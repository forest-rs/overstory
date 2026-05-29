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

extern crate alloc;

use core::{fmt, time::Duration};

use accesskit::{ActionRequest, TreeUpdate};
use hashbrown::{HashMap, HashSet};
use imaging::record::Scene;
use kurbo::{Affine, Size};
use overstory::{
    CursorIcon, KeyDispatchResult, PointerDispatchResult, SurfaceId, TimerInstant, Ui, UiFrameStats,
};
use ui_events::keyboard::KeyboardEvent;
use ui_events::pointer::PointerEvent;
use web_time::Instant;

/// Drives one [`Ui`] instance between platform events and rendering.
pub struct UiHost {
    ui: Ui,
    scene: Scene,
    scaled_scene: Scene,
    surface_scenes: HashMap<SurfaceId, Scene>,
    start_instant: Instant,
    cursor_icon: CursorIcon,
    last_frame_diagnostics: FrameDiagnostics,
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
            last_frame_diagnostics: FrameDiagnostics::default(),
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

    /// Returns the next absolute host instant when pending UI work should wake,
    /// if any.
    pub fn next_wakeup(&self) -> Option<Instant> {
        self.ui
            .next_wakeup_deadline()
            .map(|deadline| self.instant_for_deadline(deadline))
    }

    /// Returns the duration until the next pending UI wakeup, if any.
    pub fn duration_until_next_wakeup(&self) -> Option<Duration> {
        self.ui.next_wakeup_deadline().map(|deadline| {
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

    /// Builds a full AccessKit tree update for the hosted UI.
    ///
    /// This is intentionally a pass-through to core Overstory. Platform hosts
    /// own adapter activation and scheduling; the UI owns semantic lowering.
    pub fn accesskit_tree_update(&self, scale_factor: f64) -> TreeUpdate {
        self.ui
            .accesskit_tree_update_with_scale_factor(scale_factor)
    }

    /// Dispatches an AccessKit action request into the hosted UI.
    pub fn handle_accesskit_action(&mut self, request: &ActionRequest) -> HostDispatchResult {
        let focused_before = self.ui.focused();
        let handled = self.ui.handle_accesskit_action(request);
        let focus_changed = focused_before != self.ui.focused();
        HostDispatchResult {
            handled,
            focus_changed,
            needs_redraw: handled || focus_changed,
        }
    }

    /// Lays out and paints the hosted UI, then returns a scene scaled for a
    /// physical render target.
    ///
    /// `logical_viewport` is the UI layout size in logical pixels.
    /// `scale_factor` maps those logical coordinates into the physical
    /// texture that the backend renderer will target.
    pub fn prepare_frame(&mut self, logical_viewport: Size, scale_factor: f64) -> &Scene {
        self.ui.reset_frame_stats();
        self.prepare_frame_inner(logical_viewport, scale_factor)
    }

    /// Lays out and paints without clearing existing UI frame counters first.
    ///
    /// Use this for latency probes that reset counters before dispatching or
    /// mutating input, then want the prepared-frame diagnostics to include that
    /// input's invalidation marks.
    pub fn prepare_frame_preserving_stats(
        &mut self,
        logical_viewport: Size,
        scale_factor: f64,
    ) -> &Scene {
        self.prepare_frame_inner(logical_viewport, scale_factor)
    }

    fn prepare_frame_inner(&mut self, logical_viewport: Size, scale_factor: f64) -> &Scene {
        let total_start = Instant::now();

        let phase_start = Instant::now();
        self.ui.set_now(self.now());
        let set_now = phase_start.elapsed();

        let phase_start = Instant::now();
        self.ui.compute_layout(logical_viewport);
        let layout = phase_start.elapsed();

        let phase_start = Instant::now();
        self.ui.paint(&mut self.scene);
        let paint = phase_start.elapsed();

        let phase_start = Instant::now();
        self.sync_surface_scenes();
        let sync_surfaces = phase_start.elapsed();

        let phase_start = Instant::now();
        self.scene.clear();
        self.ui
            .render_composition_with(&mut self.scene, |surface| self.surface_scenes.get(&surface));
        let compose = phase_start.elapsed();

        let phase_start = Instant::now();
        self.scaled_scene.clear();
        self.scaled_scene
            .append_transformed(&self.scene, Affine::scale(scale_factor));
        let scale = phase_start.elapsed();

        self.last_frame_diagnostics = FrameDiagnostics {
            timings: FrameTimings {
                set_now,
                layout,
                paint,
                sync_surfaces,
                compose,
                scale,
                total: total_start.elapsed(),
            },
            ui: self.ui.frame_stats(),
            surface_cache_len: self.surface_scenes.len(),
            output_empty: self.scaled_scene.is_empty(),
        };
        &self.scaled_scene
    }

    /// Returns diagnostics from the most recent [`Self::prepare_frame`] call.
    #[must_use]
    pub fn last_frame_diagnostics(&self) -> FrameDiagnostics {
        self.last_frame_diagnostics
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

/// Wall-clock timings for one host frame preparation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FrameTimings {
    /// Time spent advancing the UI clock and presentation timeline.
    pub set_now: Duration,
    /// Time spent in layout, including bindings, post-layout hooks, and
    /// composition-layer geometry synchronization.
    pub layout: Duration,
    /// Time spent refreshing presentation and Overstory-owned composition
    /// surfaces.
    pub paint: Duration,
    /// Time spent copying updated surface scenes into the host cache.
    pub sync_surfaces: Duration,
    /// Time spent composing cached surfaces into the logical output scene.
    pub compose: Duration,
    /// Time spent scaling the logical scene for the physical render target.
    pub scale: Duration,
    /// Total time spent preparing the frame.
    pub total: Duration,
}

/// Diagnostics captured for one host frame preparation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FrameDiagnostics {
    /// Host-side wall-clock timings by stage.
    pub timings: FrameTimings,
    /// Core UI counters captured after the frame was prepared.
    pub ui: UiFrameStats,
    /// Number of Overstory-owned surface scenes retained by the host cache.
    pub surface_cache_len: usize,
    /// Whether the scaled output scene is empty.
    pub output_empty: bool,
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
    use accesskit::{Action, ActionRequest, Affine, NodeId, TreeId};
    use alloc::rc::Rc;
    use core::cell::Cell;
    use overstory::{Button, ElementId};

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

    #[test]
    fn prepare_frame_records_diagnostics() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(
            root,
            overstory::Layout::column(0.0).logical_size(100.0, 50.0),
        );
        ui.append(root, overstory::Text::new("Hello"));

        let mut host = UiHost::new(ui);
        let _ = host.prepare_frame(Size::new(100.0, 50.0), 1.0);
        let diagnostics = host.last_frame_diagnostics();

        assert!(diagnostics.timings.total >= diagnostics.timings.layout);
        assert!(diagnostics.ui.elements >= 2);
        assert!(diagnostics.ui.layout_passes >= 1);
        assert!(diagnostics.ui.painted_elements >= 1);
        assert!(!diagnostics.output_empty);
    }

    #[test]
    fn accesskit_focus_and_click_actions_dispatch_into_ui() {
        let clicks = Rc::new(Cell::new(0_u32));
        let clicks_for_handler = Rc::clone(&clicks);
        let mut ui = Ui::new();
        let root = ui.root();
        let button = ui.append(
            root,
            Button::new("Deploy").on_click(move |_ui, _event| {
                clicks_for_handler.set(clicks_for_handler.get() + 1);
            }),
        );
        ui.compute_layout(Size::new(160.0, 80.0));

        let mut host = UiHost::new(ui);
        let focus = host.handle_accesskit_action(&action(Action::Focus, button));
        assert!(focus.handled);
        assert_eq!(host.ui().focused(), Some(button));

        let click = host.handle_accesskit_action(&action(Action::Click, button));
        assert!(click.handled);
        assert_eq!(clicks.get(), 1);
    }

    #[test]
    fn accesskit_tree_update_applies_host_scale_factor() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(
            root,
            overstory::Layout::column(0.0).logical_size(100.0, 40.0),
        );
        ui.compute_layout(Size::new(100.0, 40.0));

        let host = UiHost::new(ui);
        let update = host.accesskit_tree_update(2.0);
        let root_node = update
            .nodes
            .iter()
            .find(|(id, _)| *id == node_id(root))
            .map(|(_, node)| node)
            .expect("root node should be emitted");

        assert_eq!(root_node.transform(), Some(&Affine::scale(2.0)));
    }

    fn action(action: Action, element: ElementId) -> ActionRequest {
        ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: node_id(element),
            data: None,
        }
    }

    fn node_id(element: ElementId) -> NodeId {
        NodeId((u64::from(element.generation()) << 32) | u64::from(element.slot()))
    }
}
