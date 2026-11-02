// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Reusable host runtime for driving one Overstory UI.

use core::{fmt, time::Duration};

use accesskit::{ActionRequest, TreeUpdate};
use frameclock::time::Timebase;
use frameclock::timing::FramePlan;
use frameclock::{
    ActiveFrame, FrameBeginResult, FrameDemand, FrameDriver, FrameOpportunity, FrameSubmission,
    FrameTimingSummary, HostTime, SchedulerConfig,
};
use hashbrown::{HashMap, HashSet};
use imaging::record::Scene;
use overstory::{CursorIcon, SurfaceId, TimerInstant, Ui, UiFrameTimings, UiRendererTimings};
use ui_events::keyboard::KeyboardEvent;
use ui_events::pointer::PointerEvent;
use web_time::Instant;

use crate::{FrameDiagnostics, FrameTimings, FrameViewport, HostDispatchResult};

/// Prepared scene and frameclock plan for one host frame.
#[derive(Clone, Copy, Debug)]
pub struct PreparedFrame<'a> {
    scene: &'a Scene,
    plan: Option<FramePlan>,
}

impl<'a> PreparedFrame<'a> {
    /// Returns the prepared scene to render.
    #[must_use]
    pub const fn scene(self) -> &'a Scene {
        self.scene
    }

    /// Returns the demand that selected this frame.
    #[must_use]
    pub const fn demand(self) -> FrameDemand {
        match self.plan {
            Some(plan) => plan.demand,
            None => FrameDemand::NONE,
        }
    }

    /// Returns the scheduler-selected frame plan for retained
    /// frameclock-driven frames.
    ///
    /// Ad hoc preparation paths return `None` because no scheduler plan was
    /// consumed.
    #[must_use]
    pub const fn plan(self) -> Option<FramePlan> {
        self.plan
    }
}

/// Drives one [`Ui`] instance between platform events and rendering.
pub struct UiHost {
    ui: Ui,
    scene: Scene,
    scaled_scene: Scene,
    surface_scenes: HashMap<SurfaceId, Scene>,
    start_instant: Instant,
    frame_driver: FrameDriver,
    active_frame: Option<ActiveFrame>,
    last_frame_timing: Option<FrameTimingSummary>,
    cursor_icon: CursorIcon,
    last_frame_diagnostics: FrameDiagnostics,
}

impl fmt::Debug for UiHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UiHost")
            .field("start_instant", &self.start_instant)
            .field("frame_driver", &self.frame_driver)
            .field("cursor_icon", &self.cursor_icon)
            .finish_non_exhaustive()
    }
}

impl UiHost {
    /// Creates a host for `ui` with a fresh monotonic clock anchor.
    #[must_use]
    pub fn new(ui: Ui) -> Self {
        let mut config = SchedulerConfig::pacing_only(Timebase::NANOS);
        config.initial_depth = 1;
        Self::with_scheduler_config(ui, config)
    }

    /// Creates a host for `ui` with an explicit frame scheduler
    /// configuration.
    ///
    /// Platform hosts select predictive, estimated, or pacing-only timing here.
    /// Tick-started opportunities wait for native callbacks; timed waits are
    /// exposed through [`Self::pending_frame_start`]. All supplied timing and
    /// scheduler durations must use the host's nanosecond timebase.
    #[must_use]
    pub fn with_scheduler_config(ui: Ui, scheduler_config: SchedulerConfig) -> Self {
        Self {
            ui,
            scene: Scene::new(),
            scaled_scene: Scene::new(),
            surface_scenes: HashMap::new(),
            start_instant: Instant::now(),
            frame_driver: FrameDriver::new(scheduler_config),
            active_frame: None,
            last_frame_timing: None,
            cursor_icon: CursorIcon::Default,
            last_frame_diagnostics: FrameDiagnostics::default(),
        }
    }

    /// Returns the hosted UI.
    #[must_use]
    pub fn ui(&self) -> &Ui {
        &self.ui
    }

    /// Returns the hosted UI mutably after advancing it to the current host
    /// time.
    ///
    /// This now establishes a clocked application-mutation boundary. Code that
    /// supplies an external clock domain should use [`Self::ui_mut_at`]
    /// instead; deterministic code that intentionally keeps the current time
    /// can pass [`Ui::now`] to that method.
    pub fn ui_mut(&mut self) -> &mut Ui {
        let now = self.now();
        self.ui_mut_at(now)
    }

    /// Returns the hosted UI mutably after advancing it to `now`.
    ///
    /// `now` must use the same normalized monotonic clock domain as pointer
    /// timestamps, timer pumps, and frame opportunities supplied by the
    /// embedding host.
    pub fn ui_mut_at(&mut self, now: TimerInstant) -> &mut Ui {
        self.advance_ui_time(now);
        &mut self.ui
    }

    /// Returns the host clock anchor.
    #[must_use]
    pub fn start_instant(&self) -> Instant {
        self.start_instant
    }

    /// Returns the host's current monotonic time as nanoseconds since
    /// [`Self::start_instant`].
    #[must_use]
    pub fn now(&self) -> TimerInstant {
        u64::try_from(self.start_instant.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }

    /// Returns the host's current monotonic time in `frameclock` units.
    #[must_use]
    pub fn frame_now(&self) -> HostTime {
        HostTime(self.now())
    }

    /// Pumps all timers that are due at the current host time.
    ///
    /// Returns `true` when a UI timer or timeline cue was due.
    ///
    /// Due UI wakeups are treated as animation demand because they may require
    /// another sampled frame. Active presentation motion is requeued separately
    /// through frame demand after dispatch and frame preparation.
    pub fn pump_timers_now(&mut self) -> bool {
        self.pump_timers(self.now())
    }

    /// Pumps all timers that are due at `now`.
    ///
    /// Returns `true` when a UI timer or timeline cue was due.
    pub fn pump_timers(&mut self, now: TimerInstant) -> bool {
        let now = now.max(self.ui.now());
        let due = self
            .ui
            .next_wakeup_deadline()
            .is_some_and(|deadline| deadline <= now);
        self.ui.pump_timers(now);
        if due {
            self.request_frame(FrameDemand::ANIMATION);
        }
        due
    }

    /// Returns the next absolute host instant when pending UI work should wake,
    /// if any.
    #[must_use]
    pub fn next_wakeup(&self) -> Option<Instant> {
        self.next_wakeup_deadline()
            .map(|deadline| self.instant_for_deadline(deadline))
    }

    /// Returns the duration until the next pending UI wakeup, if any.
    #[must_use]
    pub fn duration_until_next_wakeup(&self) -> Option<Duration> {
        self.duration_until_next_wakeup_at(self.now())
    }

    /// Returns the duration until the next pending UI wakeup from `now`.
    ///
    /// Hosts that use an external platform clock for frame ticks and input
    /// timestamps should call this with that clock's normalized host time.
    #[must_use]
    pub fn duration_until_next_wakeup_at(&self, now: TimerInstant) -> Option<Duration> {
        let now = now.max(self.ui.now());
        self.next_wakeup_deadline().map(|deadline| {
            let until = deadline.saturating_sub(now);
            Duration::from_nanos(until)
        })
    }

    /// Adds app-visible frame demand.
    ///
    /// Platform hosts call this when input, resize, animation, timers, or
    /// delayed UI work requires another frame. A scheduled frame-start wake
    /// should not call this; it only wakes the loop so an already planned frame
    /// can run.
    pub fn request_frame(&mut self, demand: FrameDemand) {
        self.frame_driver.request(demand);
    }

    /// Returns pending frame demand that has not yet been planned.
    #[must_use]
    pub fn pending_frame_demand(&self) -> FrameDemand {
        self.frame_driver.pending_demand()
    }

    /// Returns the timed wake for a queued frame, if one is waiting.
    ///
    /// Frames waiting for a native callback have no timed wake. Keep callbacks
    /// coming while [`Self::needs_frame_tick`] is true.
    #[must_use]
    pub fn pending_frame_start(&self) -> Option<HostTime> {
        self.frame_driver
            .pending_frame()
            .and_then(|frame| (!frame.hints.starts_at_tick()).then_some(frame.plan.frame_start))
    }

    /// Returns whether native frame callbacks should keep running.
    ///
    /// This includes queued frames waiting for a later tick and submissions
    /// awaiting feedback from a later callback.
    #[must_use]
    pub fn needs_frame_tick(&self) -> bool {
        self.frame_driver.needs_tick()
    }

    /// Returns the latest submitted frameclock timing summary.
    #[must_use]
    pub fn last_frame_timing_summary(&self) -> Option<FrameTimingSummary> {
        self.last_frame_timing
    }

    /// Dispatches a normalized pointer event into the UI.
    pub fn dispatch_pointer_event(&mut self, event: &PointerEvent) -> HostDispatchResult {
        let now = self.now();
        self.dispatch_pointer_event_at(event, now)
    }

    /// Dispatches a normalized pointer event at host dispatch time `now`.
    ///
    /// `now` advances the untimestamped dispatch boundary. A timestamp carried
    /// by the pointer state can advance the same non-regressing UI clock
    /// further, but neither source can rewind it.
    pub fn dispatch_pointer_event_at(
        &mut self,
        event: &PointerEvent,
        now: TimerInstant,
    ) -> HostDispatchResult {
        self.advance_ui_time(now);
        let result = self.ui.dispatch_pointer_event(event);
        let mut result = HostDispatchResult::from_pointer_event(event, result);
        self.include_active_presentation_motion(&mut result);
        result
    }

    /// Dispatches a normalized keyboard event into the UI.
    pub fn dispatch_key(&mut self, event: &KeyboardEvent) -> HostDispatchResult {
        let now = self.now();
        self.dispatch_key_at(event, now)
    }

    /// Dispatches a normalized keyboard event at host dispatch time `now`.
    ///
    /// External-clock hosts use this method so callbacks schedule timers and
    /// motion in the same domain as pointer input and frame opportunities.
    pub fn dispatch_key_at(
        &mut self,
        event: &KeyboardEvent,
        now: TimerInstant,
    ) -> HostDispatchResult {
        self.advance_ui_time(now);
        let mut result = self.ui.dispatch_key(event).into();
        self.include_active_presentation_motion(&mut result);
        result
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
    #[must_use]
    pub fn accesskit_tree_update(&self, scale_factor: f64) -> TreeUpdate {
        self.ui
            .accesskit_tree_update_with_scale_factor(scale_factor)
    }

    /// Dispatches an AccessKit action request into the hosted UI.
    pub fn handle_accesskit_action(&mut self, request: &ActionRequest) -> HostDispatchResult {
        let now = self.now();
        self.handle_accesskit_action_at(request, now)
    }

    /// Dispatches an AccessKit action at host dispatch time `now`.
    ///
    /// External-clock hosts use this method so semantic accessibility
    /// callbacks share the pointer, timer, and frame clock domain.
    pub fn handle_accesskit_action_at(
        &mut self,
        request: &ActionRequest,
        now: TimerInstant,
    ) -> HostDispatchResult {
        self.advance_ui_time(now);
        let focused_before = self.ui.focused();
        let handled = self.ui.handle_accesskit_action(request);
        let focus_changed = focused_before != self.ui.focused();
        let mut result = HostDispatchResult {
            handled,
            focus_changed,
            needs_redraw: handled || focus_changed,
            frame_demand: if handled || focus_changed {
                FrameDemand::INPUT
            } else {
                FrameDemand::NONE
            },
        };
        self.include_active_presentation_motion(&mut result);
        result
    }

    /// Lays out and paints the hosted UI for `viewport`, then returns a
    /// prepared scene scaled for its physical render target.
    ///
    /// The UI tree is laid out and composed in logical coordinates. Scaling is
    /// applied once at the host boundary so core Overstory geometry remains
    /// independent from device pixel density.
    pub fn prepare_frame(&mut self, viewport: FrameViewport) -> PreparedFrame<'_> {
        self.ui.reset_frame_stats();
        self.discard_active_frame();
        self.prepare_frame_inner(viewport, self.now(), None)
    }

    /// Lays out and paints without clearing existing UI frame counters first.
    ///
    /// Use this for latency probes that reset counters before dispatching or
    /// mutating input, then want the prepared-frame diagnostics to include that
    /// input's invalidation marks.
    pub fn prepare_frame_preserving_stats(&mut self, viewport: FrameViewport) -> PreparedFrame<'_> {
        self.discard_active_frame();
        self.prepare_frame_inner(viewport, self.now(), None)
    }

    /// Prepares the next requested frame when `frameclock` says it is due.
    ///
    /// Returns `None` when there is no pending demand, when a planned frame is
    /// waiting for its scheduler-selected frame start, or when `frameclock`
    /// reports an expired plan that must be replanned. Platform hosts should
    /// then arm [`Self::next_wakeup`] and avoid rendering.
    pub fn prepare_requested_frame(
        &mut self,
        viewport: FrameViewport,
        opportunity: FrameOpportunity,
    ) -> Option<PreparedFrame<'_>> {
        let planned = self.take_ready_frame(opportunity)?;
        let plan = planned.plan();
        let sample_time = planned.sample_time().ticks();
        self.active_frame = Some(planned);
        self.ui.reset_frame_stats();
        Some(self.prepare_frame_inner(viewport, sample_time, Some(plan)))
    }

    /// Discards the currently prepared frame without feeding presentation
    /// feedback into the scheduler.
    ///
    /// Platform hosts call this when rendering cannot submit the prepared frame,
    /// for example because the surface was lost or occluded after UI
    /// preparation.
    pub fn discard_prepared_frame(&mut self) {
        self.discard_active_frame();
    }

    fn prepare_frame_inner(
        &mut self,
        viewport: FrameViewport,
        sample_time: TimerInstant,
        plan: Option<FramePlan>,
    ) -> PreparedFrame<'_> {
        let prepared_demand = plan.map_or(FrameDemand::NONE, |plan| plan.demand);
        let total_start = Instant::now();

        let phase_start = Instant::now();
        self.advance_ui_time(sample_time);
        let set_now = phase_start.elapsed();

        let phase_start = Instant::now();
        let layout_outcome = self.ui.compute_layout(viewport.logical_size());
        let layout = phase_start.elapsed();
        if layout_outcome.needs_another_frame() {
            self.request_frame(if prepared_demand.is_empty() {
                FrameDemand::BACKGROUND
            } else {
                prepared_demand
            });
        }

        let phase_start = Instant::now();
        self.ui.refresh_presentation_surfaces();
        let surface_refresh = phase_start.elapsed();

        let phase_start = Instant::now();
        self.sync_surface_scenes();
        let sync_surfaces = phase_start.elapsed();

        let phase_start = Instant::now();
        self.ui
            .render_composition_with(&mut self.scene, |surface| self.surface_scenes.get(&surface));
        let compose = phase_start.elapsed();

        let phase_start = Instant::now();
        let use_scaled_scene = viewport.needs_output_scale();
        if use_scaled_scene {
            self.scaled_scene.clear();
            self.scaled_scene
                .append_transformed(&self.scene, viewport.logical_to_physical());
        }
        let scale = phase_start.elapsed();
        let output_empty = if use_scaled_scene {
            self.scaled_scene.is_empty()
        } else {
            self.scene.is_empty()
        };

        let timings = FrameTimings {
            set_now,
            layout,
            surface_refresh,
            sync_surfaces,
            compose,
            scale,
            total: total_start.elapsed(),
        };
        self.ui.set_frame_timings(ui_frame_timings(timings));
        self.ui.set_prepared_frame_demand(prepared_demand);

        self.last_frame_diagnostics = FrameDiagnostics {
            timings,
            ui: self.ui.frame_stats(),
            surface_cache_len: self.surface_scenes.len(),
            output_empty,
            frame_timing: self.last_frame_timing,
        };
        if self.ui.has_active_presentation_motion() {
            self.request_frame(FrameDemand::ANIMATION);
        }
        let scene = if use_scaled_scene {
            &self.scaled_scene
        } else {
            &self.scene
        };
        PreparedFrame { scene, plan }
    }

    /// Returns diagnostics from the most recent frame preparation call.
    #[must_use]
    pub fn last_frame_diagnostics(&self) -> FrameDiagnostics {
        self.last_frame_diagnostics
    }

    /// Records backend submission facts for the most recently prepared frame.
    ///
    /// Windowing/rendering hosts call this after they submit the prepared scene
    /// to their backend so core inspection snapshots can report renderer work,
    /// presentation feedback, and frameclock scheduling facts separately from
    /// Overstory's scene preparation pipeline. `submission.submitted_at` should
    /// use the same host-time base as the [`FrameOpportunity`] passed to
    /// [`Self::prepare_requested_frame`].
    pub fn record_frame_submission(
        &mut self,
        renderer_timings: UiRendererTimings,
        submission: FrameSubmission,
    ) {
        self.ui.set_renderer_timings(renderer_timings);
        if let Some(active) = self.active_frame.take() {
            let submit = self.frame_driver.submit_frame(active, submission);
            if let Some(summary) = submit.summary {
                self.record_frame_timing_summary(summary);
            }
        }
        self.last_frame_diagnostics.ui = self.ui.frame_stats();
        self.last_frame_diagnostics.frame_timing = self.last_frame_timing;
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

    fn include_active_presentation_motion(&self, result: &mut HostDispatchResult) {
        if self.ui.has_active_presentation_motion() {
            result.include_frame_demand(FrameDemand::ANIMATION);
        }
    }

    fn advance_ui_time(&mut self, now: TimerInstant) {
        self.ui.set_now(now);
    }

    fn instant_for_deadline(&self, deadline: TimerInstant) -> Instant {
        self.start_instant + Duration::from_nanos(deadline)
    }

    fn next_wakeup_deadline(&self) -> Option<TimerInstant> {
        min_deadline(
            self.ui.next_wakeup_deadline(),
            self.pending_frame_start().map(HostTime::ticks),
        )
    }

    fn take_ready_frame(&mut self, opportunity: FrameOpportunity) -> Option<ActiveFrame> {
        let begin = self.frame_driver.begin_frame(opportunity);
        if let Some(summary) = begin.resolved_feedback {
            self.record_frame_timing_summary(summary);
        }
        match begin.result {
            FrameBeginResult::Ready(frame) => Some(frame),
            FrameBeginResult::Expired(summary) => {
                let demand = summary.demand;
                self.record_frame_timing_summary(summary);
                self.frame_driver.request(demand);
                None
            }
            FrameBeginResult::Idle | FrameBeginResult::Wait(_) => None,
        }
    }

    fn discard_active_frame(&mut self) {
        if let Some(active) = self.active_frame.take() {
            let summary = self.frame_driver.discard_frame(active);
            self.record_frame_timing_summary(summary);
        }
    }

    fn record_frame_timing_summary(&mut self, summary: FrameTimingSummary) {
        self.last_frame_timing = Some(summary);
        self.ui.set_frame_timing_summary(self.last_frame_timing);
        self.last_frame_diagnostics.ui = self.ui.frame_stats();
        self.last_frame_diagnostics.frame_timing = self.last_frame_timing;
    }
}

fn min_deadline(a: Option<TimerInstant>, b: Option<TimerInstant>) -> Option<TimerInstant> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn ui_frame_timings(timings: FrameTimings) -> UiFrameTimings {
    UiFrameTimings {
        set_now: timings.set_now,
        layout: timings.layout,
        surface_refresh: timings.surface_refresh,
        sync_surfaces: timings.sync_surfaces,
        compose: timings.compose,
        scale: timings.scale,
        total: timings.total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use accesskit::{Action, ActionRequest, Affine, NodeId, TreeId};
    use alloc::rc::Rc;
    use core::cell::{Cell, RefCell};
    use frameclock::diagnostics::{FrameDropReason, FrameTimingBasis};
    use frameclock::{DisplayTiming, Duration as FrameDuration, OutputId};
    use kurbo::{Point, Size, Vec2};
    use overstory::motion::{
        EffectClip, EffectKeyframe, EffectRule, EffectRules, EffectTriggerId, PresentationSlot,
        PresentationTarget, PresentationValue,
    };
    use overstory::{
        Button, ChoiceOption, ElementId, Layout, SegmentedControl, ToggleSwitch, segmented_item,
        selected_segmented_index,
    };
    use ui_events::keyboard::{Code, Key, NamedKey};
    use ui_events::pointer::{
        PointerButton, PointerButtonEvent, PointerButtons, PointerEvent, PointerId, PointerInfo,
        PointerState, PointerType,
    };

    #[test]
    fn prepare_frame_populates_surface_scene_cache() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(100.0, 50.0));
        ui.append(root, overstory::Text::new("Hello"));

        let mut host = UiHost::new(ui);
        let frame = host.prepare_frame(FrameViewport::from_logical_size(
            Size::new(100.0, 50.0),
            1.0,
        ));

        assert!(!frame.scene().is_empty());
        assert!(!host.surface_scenes.is_empty());
    }

    #[test]
    fn initial_control_state_does_not_request_animation_frame() {
        let mut ui = Ui::new();
        ui.set_theme(overstory::theme::default_theme());
        ui.set_cascade(overstory::theme::default_cascade(ui.properties()));
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(100.0, 50.0));
        ui.append(root, ToggleSwitch::new().checked(true));
        ui.set_now(0);

        let mut host = UiHost::new(ui);
        let _ = host.prepare_frame(test_viewport());
        assert_eq!(host.pending_frame_demand(), FrameDemand::NONE);

        host.ui_mut_at(2_000_000_000);
        let _ = host.prepare_frame(test_viewport());
        assert_eq!(host.pending_frame_demand(), FrameDemand::NONE);
    }

    #[test]
    fn prepare_frame_records_diagnostics() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(100.0, 50.0));
        ui.append(root, overstory::Text::new("Hello"));

        let mut host = UiHost::new(ui);
        let _ = host.prepare_frame(FrameViewport::from_logical_size(
            Size::new(100.0, 50.0),
            1.0,
        ));
        let diagnostics = host.last_frame_diagnostics();

        assert!(diagnostics.timings.total >= diagnostics.timings.layout);
        assert_eq!(
            diagnostics.ui.timings.surface_refresh,
            diagnostics.timings.surface_refresh
        );
        assert_eq!(diagnostics.ui.timings.total, diagnostics.timings.total);
        assert!(diagnostics.ui.elements >= 2);
        assert!(diagnostics.ui.layout_passes >= 1);
        assert!(diagnostics.ui.painted_elements >= 1);
        assert!(!diagnostics.output_empty);
        assert_eq!(diagnostics.ui.prepared_demand, FrameDemand::NONE);
    }

    #[test]
    fn requested_frame_carries_deferred_layout_forward() {
        #[derive(Debug)]
        struct OscillatingLayout;

        impl overstory::Widget for OscillatingLayout {
            fn arrange(
                &mut self,
                _ctx: &mut overstory::ArrangeContext<'_>,
            ) -> overstory::ArrangeResult {
                overstory::ArrangeResult::LAYOUT_CHANGED
            }
        }

        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.append_widget(root, Box::new(OscillatingLayout));
        ui.set_layout(child, Layout::new().logical_size(10.0, 10.0));
        let mut host = UiHost::new(ui);
        host.request_frame(FrameDemand::INPUT);

        let _ = host.prepare_requested_frame(test_viewport(), pacing_only(HostTime(0)));

        assert_eq!(
            host.last_frame_diagnostics().ui.layout_outcome,
            overstory::LayoutOutcome::Deferred { passes: 4 }
        );
        assert_eq!(host.pending_frame_demand(), FrameDemand::INPUT);
    }

    #[test]
    fn frame_submission_records_renderer_timings_in_ui_diagnostics() {
        let mut host = UiHost::new(Ui::new());
        let timings = renderer_timings();

        host.record_frame_submission(timings, FrameSubmission::new(HostTime(0), None));

        assert_eq!(host.last_frame_diagnostics().ui.renderer_timings, timings);
        assert_eq!(host.ui().frame_stats().renderer_timings, timings);
    }

    #[test]
    fn duration_until_next_wakeup_can_use_external_clock_domain() {
        let mut host = UiHost::new(Ui::new());

        host.ui_mut_at(0).schedule_timeline_cue(10, 1);

        assert_eq!(
            host.duration_until_next_wakeup_at(4),
            Some(Duration::from_nanos(6))
        );
        assert_eq!(
            host.duration_until_next_wakeup_at(12),
            Some(Duration::from_nanos(0))
        );
    }

    #[test]
    fn requested_input_frame_submits_frameclock_summary() {
        let mut host = UiHost::new(Ui::new());

        host.request_frame(FrameDemand::INPUT);
        let prepared = host
            .prepare_requested_frame(test_viewport(), pacing_only(HostTime(0)))
            .expect("input demand should produce an immediate prepared frame");
        assert_eq!(prepared.demand(), FrameDemand::INPUT);
        assert_eq!(
            prepared.plan().map(|plan| plan.frame_interval),
            Some(default_frame_interval())
        );
        assert_eq!(
            host.last_frame_diagnostics().ui.prepared_demand,
            FrameDemand::INPUT
        );
        assert_eq!(host.ui().frame_stats().prepared_demand, FrameDemand::INPUT);
        assert_eq!(host.last_frame_timing_summary(), None);

        host.record_frame_submission(renderer_timings(), FrameSubmission::new(HostTime(1), None));

        let summary = host
            .last_frame_timing_summary()
            .expect("submitted frame should publish a timing summary");
        assert_eq!(summary.frame_index, 0);
        assert_eq!(summary.demand, FrameDemand::INPUT);
        assert_eq!(summary.submitted_at, Some(HostTime(1)));
        assert_eq!(summary.actual_present, None);
        assert_eq!(host.last_frame_diagnostics().frame_timing, Some(summary));
        assert_eq!(host.ui().frame_stats().frame_timing, Some(summary));
    }

    #[test]
    fn pacing_only_timing_can_use_reported_refresh_interval() {
        let opportunity = pacing_only_with_refresh_interval(HostTime(10), FrameDuration(8_333_333));

        assert_eq!(opportunity.tick.now, HostTime(10));
        assert_eq!(opportunity.tick.predicted_present, None);
        assert_eq!(
            opportunity.tick.refresh_interval,
            Some(FrameDuration(8_333_333))
        );
        assert_eq!(opportunity.hints.latest_commit(), HostTime(8_333_343));
        assert_eq!(
            opportunity.display_timing,
            DisplayTiming::fixed(FrameDuration(8_333_333))
        );
    }

    #[test]
    fn discarded_requested_frame_publishes_drop_timing_summary() {
        let mut host = UiHost::new(Ui::new());

        host.request_frame(FrameDemand::INPUT);
        assert!(
            host.prepare_requested_frame(test_viewport(), pacing_only(HostTime(0)))
                .is_some()
        );
        host.discard_prepared_frame();

        let summary = host
            .last_frame_timing_summary()
            .expect("discarded frame should publish drop timing");
        assert_eq!(summary.frame_index, 0);
        assert_eq!(summary.demand, FrameDemand::INPUT);
        assert_eq!(summary.submitted_at, None);
        assert_eq!(summary.actual_present, None);
        assert_eq!(summary.drop_reason, Some(FrameDropReason::Discarded));
        assert_eq!(host.last_frame_diagnostics().frame_timing, Some(summary));
        assert_eq!(host.ui().frame_stats().frame_timing, Some(summary));
    }

    #[test]
    fn tick_wait_keeps_callbacks_running_without_a_timer_wake() {
        let mut host = UiHost::new(Ui::new());
        host.request_frame(FrameDemand::BACKGROUND);
        let tick_opportunity = |now| {
            let mut opportunity = pacing_only(now);
            opportunity.hints = opportunity.hints.starting_at_tick();
            opportunity
        };

        assert!(
            host.prepare_requested_frame(test_viewport(), tick_opportunity(HostTime(0)))
                .is_none()
        );
        assert!(host.pending_frame_demand().is_empty());
        assert!(host.needs_frame_tick());
        assert_eq!(host.pending_frame_start(), None);
        assert_eq!(host.duration_until_next_wakeup_at(0), None);

        let prepared = host
            .prepare_requested_frame(
                test_viewport(),
                tick_opportunity(HostTime(default_frame_interval().ticks())),
            )
            .expect("the next native tick should release the background frame");
        assert_eq!(prepared.demand(), FrameDemand::BACKGROUND);
        host.discard_prepared_frame();
        assert!(!host.needs_frame_tick());
    }

    #[test]
    fn retained_pending_demand_survives_submitted_frame() {
        let mut host = UiHost::new(Ui::new());

        host.request_frame(FrameDemand::ANIMATION);
        assert!(
            host.prepare_requested_frame(test_viewport(), pacing_only(HostTime(0)))
                .is_none()
        );
        let frame_start = host
            .pending_frame_start()
            .expect("animation should queue a future frame start");
        assert_eq!(
            host.duration_until_next_wakeup_at(0),
            Some(Duration::from_nanos(frame_start.ticks()))
        );

        host.request_frame(FrameDemand::BACKGROUND);
        assert_eq!(host.pending_frame_demand(), FrameDemand::BACKGROUND);
        let prepared = host
            .prepare_requested_frame(test_viewport(), pacing_only(frame_start))
            .expect("queued animation should produce a prepared frame at frame start");
        assert_eq!(prepared.demand(), FrameDemand::ANIMATION);

        host.record_frame_submission(
            renderer_timings(),
            FrameSubmission::new(frame_start + FrameDuration(1), None),
        );

        let summary = host
            .last_frame_timing_summary()
            .expect("submitted animation frame should publish timing");
        assert_eq!(summary.demand, FrameDemand::ANIMATION);
        assert_eq!(host.pending_frame_demand(), FrameDemand::BACKGROUND);
    }

    #[test]
    fn active_presentation_motion_requeues_animation_demand_after_sampling() {
        const TRIGGER: EffectTriggerId = EffectTriggerId::new(7);

        let mut ui = Ui::new();
        let root = ui.root();
        let target = PresentationTarget::Slot(PresentationSlot::VisualTranslation);
        let clip = EffectClip::from_keyframes([
            EffectKeyframe::new(0, PresentationValue::Offset2d(Vec2::ZERO)),
            EffectKeyframe::new(
                100_000_000,
                PresentationValue::Offset2d(Vec2::new(12.0, 0.0)),
            ),
        ]);
        ui.set_local(
            root,
            ui.properties().effects,
            EffectRules::single(EffectRule::new(TRIGGER, target, clip)),
        );
        ui.set_now(0);
        assert!(ui.trigger_effect(root, TRIGGER));

        let mut host = UiHost::new(ui);
        host.request_frame(FrameDemand::INPUT);
        let prepared_demand = {
            let prepared = host
                .prepare_requested_frame(test_viewport(), pacing_only(HostTime(0)))
                .expect("input demand should produce an immediate prepared frame");
            prepared.demand()
        };

        assert_eq!(prepared_demand, FrameDemand::INPUT);
        assert!(
            host.pending_frame_demand().contains(FrameDemand::ANIMATION),
            "unfinished presentation motion should carry animation demand into the next frame"
        );
    }

    #[test]
    fn disabled_segment_effect_requests_animation_demand_after_dispatch() {
        let mut ui = Ui::new();
        ui.set_theme(overstory::theme::default_theme());
        ui.set_cascade(overstory::theme::default_cascade(ui.properties()));
        let root = ui.root();
        ui.set_layout(root, Layout::row(0.0).logical_size(320.0, 80.0));
        let segmented = ui.append(
            root,
            SegmentedControl::new()
                .options([
                    ChoiceOption::new("Preview"),
                    ChoiceOption::new("Code").disabled(),
                    ChoiceOption::new("Metrics"),
                ])
                .selected_index(0),
        );
        let disabled = segmented_item(&ui, segmented, 1).expect("disabled segment should exist");
        ui.compute_layout(Size::new(320.0, 80.0));
        let down = pointer_down(ui.effective_bounds(disabled).center(), 0);

        let mut host = UiHost::new(ui);
        let result = host.dispatch_pointer_event(&down);

        assert_eq!(selected_segmented_index(host.ui(), segmented), Some(0));
        assert!(result.needs_redraw);
        assert!(result.frame_demand.contains(FrameDemand::ANIMATION));
    }

    #[test]
    fn expired_queued_frame_records_drop_and_requeues_demand() {
        let mut host = UiHost::new(Ui::new());

        host.request_frame(FrameDemand::ANIMATION);
        assert!(
            host.prepare_requested_frame(test_viewport(), pacing_only(HostTime(0)))
                .is_none()
        );
        assert!(
            host.pending_frame_start().is_some(),
            "animation demand should queue a planned frame"
        );

        assert!(
            host.prepare_requested_frame(test_viewport(), pacing_only(HostTime(1_000_000_000)))
                .is_none()
        );

        let summary = host
            .last_frame_timing_summary()
            .expect("expired frame should publish drop timing");
        assert_eq!(summary.demand, FrameDemand::ANIMATION);
        assert_eq!(summary.drop_reason, Some(FrameDropReason::MissedDeadline));
        assert_eq!(host.pending_frame_demand(), FrameDemand::ANIMATION);
        assert_eq!(host.last_frame_diagnostics().frame_timing, Some(summary));
        assert_eq!(host.ui().frame_stats().frame_timing, Some(summary));
    }

    #[test]
    fn frame_submission_can_record_actual_present_feedback() {
        let mut host = UiHost::new(Ui::new());

        host.request_frame(FrameDemand::INPUT);
        assert!(
            host.prepare_requested_frame(test_viewport(), pacing_only(HostTime(0)))
                .is_some()
        );
        host.record_frame_submission(
            renderer_timings(),
            FrameSubmission::new(HostTime(1), Some(HostTime(2))),
        );

        let summary = host
            .last_frame_timing_summary()
            .expect("submitted frame should publish timing");
        assert_eq!(summary.submitted_at, Some(HostTime(1)));
        assert_eq!(summary.actual_present, Some(HostTime(2)));
        assert_eq!(summary.timing_basis, FrameTimingBasis::ActualPresent);
    }

    #[test]
    fn deferred_present_feedback_resolves_on_next_frame_tick() {
        let mut host = UiHost::new(Ui::new());

        host.request_frame(FrameDemand::INPUT);
        assert!(
            host.prepare_requested_frame(test_viewport(), pacing_only(HostTime(0)))
                .is_some()
        );
        host.record_frame_submission(renderer_timings(), FrameSubmission::deferred(HostTime(1)));
        assert_eq!(host.last_frame_timing_summary(), None);
        assert!(host.needs_frame_tick());
        assert_eq!(host.pending_frame_start(), None);

        assert!(
            host.prepare_requested_frame(
                test_viewport(),
                pacing_only_with_previous_actual_present(HostTime(3), HostTime(2))
            )
            .is_none()
        );

        let summary = host
            .last_frame_timing_summary()
            .expect("next tick should resolve deferred presentation feedback");
        assert!(!host.needs_frame_tick());
        assert_eq!(summary.submitted_at, Some(HostTime(1)));
        assert_eq!(summary.actual_present, Some(HostTime(2)));
        assert_eq!(summary.timing_basis, FrameTimingBasis::ActualPresent);
        assert_eq!(host.last_frame_diagnostics().frame_timing, Some(summary));
        assert_eq!(host.ui().frame_stats().frame_timing, Some(summary));
    }

    #[test]
    fn accesskit_focus_and_click_actions_dispatch_into_ui() {
        let clicks = Rc::new(Cell::new(0_u32));
        let clicks_for_handler = Rc::clone(&clicks);
        let mut ui = Ui::new();
        let root = ui.root();
        let button = ui.append(
            root,
            Button::new("Deploy").on_activate(move |_ui, _event| {
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
    fn keyboard_dispatch_schedules_relative_timers_from_current_host_time() {
        const DELAY: TimerInstant = 100_000_000;

        let mut ui = Ui::new();
        let root = ui.root();
        let button = ui.append(
            root,
            Button::new("Schedule").on_activate(move |ui, _event| {
                ui.schedule_timer_once(root, DELAY);
            }),
        );
        ui.set_focus(Some(button));

        let mut host = UiHost::new(ui);
        host.start_instant = Instant::now() - Duration::from_secs(1);
        let dispatch_time = host.now();
        let enter = KeyboardEvent::key_down(Key::Named(NamedKey::Enter), Code::Enter);

        assert!(host.dispatch_key(&enter).handled);
        assert!(
            host.ui()
                .next_timer_deadline()
                .is_some_and(|deadline| deadline >= dispatch_time.saturating_add(DELAY)),
            "relative timer should use current host time, not stale Ui time"
        );
    }

    #[test]
    fn explicit_dispatch_time_clocks_nested_and_host_accesskit_callbacks() {
        const KEY_TIME: TimerInstant = 2_000_000_000;
        const ACCESS_TIME: TimerInstant = 3_000_000_000;
        const DELAY: TimerInstant = 100_000_000;

        let observed_times = Rc::new(RefCell::new(Vec::new()));
        let mut ui = Ui::new();
        let root = ui.root();
        let observed_times_for_inner = Rc::clone(&observed_times);
        let inner = ui.append(
            root,
            Button::new("Inner").on_activate(move |ui, _event| {
                observed_times_for_inner.borrow_mut().push(ui.now());
                ui.schedule_timer_once(root, DELAY);
            }),
        );
        let observed_times_for_outer = Rc::clone(&observed_times);
        let outer = ui.append(
            root,
            Button::new("Outer").on_activate(move |ui, _event| {
                observed_times_for_outer.borrow_mut().push(ui.now());
                let _ = ui.handle_accesskit_action(&action(Action::Click, inner));
            }),
        );
        ui.set_focus(Some(outer));
        ui.compute_layout(Size::new(160.0, 80.0));
        let mut host = UiHost::new(ui);
        let enter = KeyboardEvent::key_down(Key::Named(NamedKey::Enter), Code::Enter);

        assert!(host.dispatch_key_at(&enter, KEY_TIME).handled);
        assert_eq!(
            host.ui().next_timer_deadline(),
            Some(KEY_TIME.saturating_add(DELAY))
        );

        assert!(
            host.handle_accesskit_action_at(&action(Action::Click, inner), ACCESS_TIME)
                .handled
        );
        assert_eq!(
            host.ui().next_timer_deadline(),
            Some(KEY_TIME.saturating_add(DELAY)),
            "the first timer remains the next deadline"
        );
        assert_eq!(host.ui().now(), ACCESS_TIME);
        assert_eq!(
            &*observed_times.borrow(),
            &[KEY_TIME, KEY_TIME, ACCESS_TIME],
            "nested dispatch must preserve its outer boundary time"
        );
    }

    #[test]
    fn mutation_boundaries_clock_relative_app_work() {
        const MUTATION_TIME: TimerInstant = 4_000_000_000;
        const DELAY: TimerInstant = 100_000_000;

        let mut host = UiHost::new(Ui::new());
        host.start_instant = Instant::now() - Duration::from_secs(1);
        let root = host.ui().root();
        let before_mutation = host.now();
        host.ui_mut().schedule_timer_once(root, DELAY);
        assert!(
            host.ui()
                .next_timer_deadline()
                .is_some_and(|deadline| deadline >= before_mutation.saturating_add(DELAY)),
            "default mutable access must sample current host time"
        );

        let mut host = UiHost::new(Ui::new());
        let root = host.ui().root();

        host.ui_mut_at(MUTATION_TIME)
            .schedule_timer_once(root, DELAY);

        assert_eq!(host.ui().now(), MUTATION_TIME);
        assert_eq!(
            host.ui().next_timer_deadline(),
            Some(MUTATION_TIME.saturating_add(DELAY))
        );
    }

    #[test]
    fn pointer_and_untimestamped_dispatch_share_a_non_regressing_clock() {
        let mut host = UiHost::new(Ui::new());
        let key = KeyboardEvent::key_down(Key::Named(NamedKey::Enter), Code::Enter);

        let _ = host.dispatch_key_at(&key, 1_000);
        let _ = host.dispatch_pointer_event_at(&pointer_down(Point::ZERO, 900), 950);
        assert_eq!(
            host.ui().now(),
            1_000,
            "an older queued pointer event must not rewind Ui time"
        );

        let _ = host.dispatch_pointer_event_at(&pointer_down(Point::ZERO, 1_200), 1_100);
        assert_eq!(
            host.ui().now(),
            1_200,
            "a newer normalized pointer timestamp may advance Ui time"
        );

        let _ = host.dispatch_pointer_event_at(&PointerEvent::Leave(pointer_info()), 1_300);
        assert_eq!(
            host.ui().now(),
            1_300,
            "pointer variants without state timestamps use host dispatch time"
        );
    }

    #[test]
    fn accesskit_tree_update_applies_host_scale_factor() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(100.0, 40.0));
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

    fn test_viewport() -> FrameViewport {
        FrameViewport::from_logical_size(Size::new(100.0, 50.0), 1.0)
    }

    const fn default_frame_interval() -> FrameDuration {
        FrameDuration(16_666_667)
    }

    fn pacing_only(now: HostTime) -> FrameOpportunity {
        pacing_only_with_refresh_interval(now, default_frame_interval())
    }

    fn pacing_only_with_refresh_interval(
        now: HostTime,
        refresh_interval: FrameDuration,
    ) -> FrameOpportunity {
        FrameOpportunity::pacing_only(now, refresh_interval, OutputId(0))
    }

    fn pacing_only_with_previous_actual_present(
        now: HostTime,
        actual_present: HostTime,
    ) -> FrameOpportunity {
        let mut opportunity = pacing_only(now);
        opportunity.tick.prev_actual_present = Some(actual_present);
        opportunity
    }

    fn pointer_down(point: Point, time: u64) -> PointerEvent {
        PointerEvent::Down(PointerButtonEvent {
            pointer: pointer_info(),
            button: Some(PointerButton::Primary),
            state: pointer_state(point, time, true),
        })
    }

    fn pointer_info() -> PointerInfo {
        PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        }
    }

    fn pointer_state(point: Point, time: u64, primary_down: bool) -> PointerState {
        let mut buttons = PointerButtons::new();
        if primary_down {
            buttons.insert(PointerButton::Primary);
        }
        let mut state = PointerState {
            time,
            buttons,
            count: 1,
            pressure: if primary_down { 0.5 } else { 0.0 },
            ..PointerState::default()
        };
        state.position.x = point.x;
        state.position.y = point.y;
        state
    }

    fn renderer_timings() -> UiRendererTimings {
        UiRendererTimings {
            scene_to_texture: Duration::from_millis(1),
            surface_acquire: Duration::from_millis(2),
            blit: Duration::from_millis(3),
            present: Duration::from_millis(4),
            total: Duration::from_millis(10),
        }
    }
}
