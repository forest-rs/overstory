// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Runtime helpers for driving an [`overstory::Ui`] outside of a
//! particular platform windowing backend.
//!
//! This crate owns the reusable mechanics that every native Overstory
//! host needs: monotonic timer time, timer wakeup calculation, normalized
//! pointer/key dispatch, cursor intent tracking, layout, and preparation of
//! an `imaging` scene for a physical render target. Window creation, event
//! loop ownership, and GPU presentation are intentionally left to backend
//! crates such as `overstory_host_winit`.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::time::{Duration, Instant};

use imaging::record::Scene;
use kurbo::{Affine, Size};
use overstory::{
    CursorIcon, KeyDispatchResult, PointerDispatchResult, SurfaceId, TimerInstant, Ui,
};
use ui_events::keyboard::KeyboardEvent;
use ui_events::pointer::PointerEvent;

/// Drives one [`Ui`] instance between platform events and rendering.
pub struct UiHost {
    ui: Ui,
    scene: Scene,
    scaled_scene: Scene,
    surface_scenes: HashMap<SurfaceId, Scene>,
    start_instant: Instant,
    cursor_icon: CursorIcon,
}

impl fmt::Debug for UiHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UiHost")
            .field("start_instant", &self.start_instant)
            .field("cursor_icon", &self.cursor_icon)
            .finish_non_exhaustive()
    }
}

impl UiHost {
    /// Creates a host for `ui` with a fresh monotonic clock anchor.
    pub fn new(ui: Ui) -> Self {
        Self {
            ui,
            scene: Scene::new(),
            scaled_scene: Scene::new(),
            surface_scenes: HashMap::new(),
            start_instant: Instant::now(),
            cursor_icon: CursorIcon::Default,
        }
    }

    /// Returns the hosted UI.
    pub fn ui(&self) -> &Ui {
        &self.ui
    }

    /// Returns the hosted UI mutably.
    pub fn ui_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }

    /// Returns the host clock anchor.
    pub fn start_instant(&self) -> Instant {
        self.start_instant
    }

    /// Returns the host's current monotonic time as nanoseconds since
    /// [`Self::start_instant`].
    pub fn now(&self) -> TimerInstant {
        u64::try_from(self.start_instant.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }

    /// Pumps all timers that are due at the current host time.
    pub fn pump_timers_now(&mut self) {
        self.pump_timers(self.now());
    }

    /// Pumps all timers that are due at `now`.
    pub fn pump_timers(&mut self, now: TimerInstant) {
        self.ui.pump_timers(now);
    }

    /// Returns the next absolute host instant when a pending UI timer should
    /// wake, if any.
    pub fn next_wakeup(&self) -> Option<Instant> {
        self.ui
            .next_timer_deadline()
            .map(|deadline| self.instant_for_deadline(deadline))
    }

    /// Returns the duration until the next pending UI timer, if any.
    pub fn duration_until_next_wakeup(&self) -> Option<Duration> {
        self.ui.next_timer_deadline().map(|deadline| {
            let elapsed = self.start_instant.elapsed().as_nanos();
            let until = u128::from(deadline).saturating_sub(elapsed);
            Duration::from_nanos(u64::try_from(until).unwrap_or(u64::MAX))
        })
    }

    /// Dispatches a normalized pointer event into the UI.
    pub fn dispatch_pointer_event(&mut self, event: &PointerEvent) -> HostDispatchResult {
        self.ui.dispatch_pointer_event(event).into()
    }

    /// Dispatches a normalized keyboard event into the UI.
    pub fn dispatch_key(&mut self, event: &KeyboardEvent) -> HostDispatchResult {
        self.ui.dispatch_key(event).into()
    }

    /// Returns a cursor icon when the UI's current cursor intent differs from
    /// the last value observed through this host.
    pub fn cursor_update(&mut self) -> Option<CursorIcon> {
        let next = self.ui.cursor_icon().unwrap_or(CursorIcon::Default);
        if self.cursor_icon == next {
            return None;
        }
        self.cursor_icon = next;
        Some(next)
    }

    /// Lays out and paints the hosted UI, then returns a scene scaled for a
    /// physical render target.
    ///
    /// `logical_viewport` is the UI layout size in logical pixels.
    /// `scale_factor` maps those logical coordinates into the physical
    /// texture that the backend renderer will target.
    pub fn prepare_frame(&mut self, logical_viewport: Size, scale_factor: f64) -> &Scene {
        self.ui.compute_layout(logical_viewport);
        self.ui.paint(&mut self.scene);
        self.sync_surface_scenes();

        self.scene.clear();
        self.ui
            .render_composition_with(&mut self.scene, |surface| self.surface_scenes.get(&surface));

        self.scaled_scene.clear();
        self.scaled_scene
            .append_transformed(&self.scene, Affine::scale(scale_factor));
        &self.scaled_scene
    }

    fn sync_surface_scenes(&mut self) {
        let live: HashSet<SurfaceId> = self
            .ui
            .composition_surfaces()
            .map(|surface| surface.surface())
            .collect();
        self.surface_scenes
            .retain(|surface, _| live.contains(surface));
        for update in self.ui.composition_surface_updates() {
            self.surface_scenes
                .insert(update.surface(), update.scene().clone());
        }
    }

    fn instant_for_deadline(&self, deadline: TimerInstant) -> Instant {
        self.start_instant + Duration::from_nanos(deadline)
    }
}

/// Result of dispatching a platform-normalized event into a [`UiHost`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HostDispatchResult {
    /// Whether any UI handler consumed or reacted to the event.
    pub handled: bool,
    /// Whether focus changed while dispatching the event.
    pub focus_changed: bool,
    /// Whether the platform host should request a redraw.
    pub needs_redraw: bool,
}

impl From<KeyDispatchResult> for HostDispatchResult {
    fn from(result: KeyDispatchResult) -> Self {
        Self {
            handled: result.handled,
            focus_changed: result.focus_changed,
            needs_redraw: result.handled || result.focus_changed,
        }
    }
}

impl From<PointerDispatchResult> for HostDispatchResult {
    fn from(result: PointerDispatchResult) -> Self {
        Self {
            handled: result.handled,
            focus_changed: result.focus_changed,
            needs_redraw: result.changed(),
        }
    }
}

/// Converts a physical target size and scale factor into a logical viewport.
pub fn logical_viewport(width: u32, height: u32, scale_factor: f64) -> Size {
    Size::new(
        f64::from(width) / scale_factor,
        f64::from(height) / scale_factor,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_viewport_divides_physical_size_by_scale_factor() {
        assert_eq!(logical_viewport(400, 300, 2.0), Size::new(200.0, 150.0));
    }

    #[test]
    fn prepare_frame_populates_surface_scene_cache() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(
            root,
            overstory::Layout::column(0.0).logical_size(100.0, 50.0),
        );
        ui.append(root, overstory::Text::new("Hello"));

        let mut host = UiHost::new(ui);
        let frame = host.prepare_frame(Size::new(100.0, 50.0), 1.0);

        assert!(!frame.is_empty());
        assert!(!host.surface_scenes.is_empty());
    }
}
