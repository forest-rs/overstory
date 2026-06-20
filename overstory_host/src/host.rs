// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Reusable host runtime for driving one Overstory UI.

use core::{fmt, time::Duration};

use accesskit::{ActionRequest, TreeUpdate};
use frameclock::{
    ActiveFrame, DisplayTiming, Duration as FrameDuration, FrameBeginResult, FrameDemand,
    FrameDriver, FrameOpportunity, FrameSubmission, FrameTick, FrameTimingSummary, HostTime,
    OutputId, PresentHints, PresentationObservation, SchedulerConfig,
};
use hashbrown::{HashMap, HashSet};
use imaging::record::Scene;
use overstory::{CursorIcon, SurfaceId, TimerInstant, Ui, UiFrameTimings, UiRendererTimings};
use ui_events::keyboard::KeyboardEvent;
use ui_events::pointer::PointerEvent;
use web_time::Instant;

use crate::{FrameDiagnostics, FrameTimings, FrameViewport, HostDispatchResult};

const DEFAULT_FRAME_INTERVAL: FrameDuration = FrameDuration(16_666_667);

/// Timing facts used by [`UiHost`] to ask `frameclock` for one frame plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostFrameTiming {
    /// Host time at which the frame opportunity is being evaluated.
    pub now: HostTime,
    /// Predicted time when pixels will be presented, if the platform exposes
    /// one.
    pub predicted_present: Option<HostTime>,
    /// Display refresh interval in host-time ticks, if known.
    pub refresh_interval: Option<u64>,
    /// Actual presentation time of the previous frame, if the platform reports
    /// it on this tick.
    pub prev_actual_present: Option<HostTime>,
    /// Backend submission constraints paired with this opportunity.
    pub hints: PresentHints,
    /// Display timing constraints for the target output.
    pub display_timing: DisplayTiming,
}

impl HostFrameTiming {
    /// Creates pacing-only timing facts for hosts that do not expose predicted
    /// presentation timestamps.
    #[must_use]
    pub fn pacing_only(now: HostTime) -> Self {
        Self::pacing_only_with_refresh_interval(now, DEFAULT_FRAME_INTERVAL)
    }

    /// Creates pacing-only timing facts with a host-supplied refresh interval.
    ///
    /// Use this when the backend does not expose predicted presentation
    /// timestamps, but can still report the target display cadence.
    #[must_use]
    pub fn pacing_only_with_refresh_interval(
        now: HostTime,
        refresh_interval: FrameDuration,
    ) -> Self {
        Self {
            now,
            predicted_present: None,
            refresh_interval: Some(refresh_interval.ticks()),
            prev_actual_present: None,
            hints: PresentHints::pacing_only(now + refresh_interval),
            display_timing: DisplayTiming::fixed(refresh_interval),
        }
    }

    /// Sets the actual presentation time of the previous submitted frame.
    ///
    /// Use this for platforms that report actual presentation one tick later.
    /// `frameclock` resolves any deferred submission summary before deciding
    /// what to do with the current frame opportunity.
    #[must_use]
    pub const fn with_previous_actual_present(mut self, actual_present: HostTime) -> Self {
        self.prev_actual_present = Some(actual_present);
        self
    }
}

/// Host-side submission facts for a prepared frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostFrameSubmission {
    /// CPU-side backend renderer timings for the submitted frame.
    pub renderer_timings: UiRendererTimings,
    /// Host time when the backend submitted or committed the frame.
    ///
    /// When omitted, [`UiHost`] samples its monotonic clock while recording the
    /// submission.
    pub submitted_at: Option<HostTime>,
    /// Presentation evidence available for the submitted frame.
    pub presentation: PresentationObservation,
}

impl HostFrameSubmission {
    /// Creates submission facts with renderer timings and no platform
    /// presentation timestamp.
    #[must_use]
    pub const fn new(renderer_timings: UiRendererTimings) -> Self {
        Self {
            renderer_timings,
            submitted_at: None,
            presentation: PresentationObservation::Unavailable,
        }
    }

    /// Sets the host time when the frame was submitted or committed.
    #[must_use]
    pub const fn with_submitted_at(mut self, submitted_at: HostTime) -> Self {
        self.submitted_at = Some(submitted_at);
        self
    }

    /// Sets the actual presentation time reported by the platform.
    ///
    /// The timestamp must use the same host-time base as [`HostFrameTiming`].
    #[must_use]
    pub const fn with_actual_present(mut self, actual_present: HostTime) -> Self {
        self.presentation = PresentationObservation::Actual(actual_present);
        self
    }

    /// Marks this submission as waiting for actual-present feedback on a later
    /// frame tick.
    #[must_use]
    pub const fn with_deferred_present_feedback(mut self) -> Self {
        self.presentation = PresentationObservation::Deferred;
        self
    }
}

impl Default for HostFrameSubmission {
    fn default() -> Self {
        Self::new(UiRendererTimings::default())
    }
}

/// Prepared scene and scheduling demand for one host frame.
#[derive(Clone, Copy, Debug)]
pub struct PreparedFrame<'a> {
    scene: &'a Scene,
    demand: FrameDemand,
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
        self.demand
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
    frame_output: OutputId,
    next_frame_index: u64,
    active_frame: Option<ActiveFrame>,
    last_frame_timing: Option<FrameTimingSummary>,
    cursor_icon: CursorIcon,
    last_frame_diagnostics: FrameDiagnostics,
}

impl fmt::Debug for UiHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UiHost")
            .field("start_instant", &self.start_instant)
            .field("next_frame_index", &self.next_frame_index)
            .field("frame_driver", &self.frame_driver)
            .field("cursor_icon", &self.cursor_icon)
            .finish_non_exhaustive()
    }
}

impl UiHost {
    /// Creates a host for `ui` with a fresh monotonic clock anchor.
    #[must_use]
    pub fn new(ui: Ui) -> Self {
        let mut config = SchedulerConfig::pacing_only();
        config.initial_depth = 1;
        Self::with_scheduler_config(ui, config)
    }

    /// Creates a host for `ui` with an explicit frame scheduler
    /// configuration.
    ///
    /// Platform hosts use this when their native wake source changes how
    /// [`FrameBeginResult::WaitUntil`] should be handled. Event-loop hosts can
    /// mirror frame-start waits into a timer queue; display-link-only hosts may
    /// instead choose a fixed one-deep predictive plan that uses each display
    /// callback as the frame-start wake.
    #[must_use]
    pub fn with_scheduler_config(ui: Ui, scheduler_config: SchedulerConfig) -> Self {
        Self {
            ui,
            scene: Scene::new(),
            scaled_scene: Scene::new(),
            surface_scenes: HashMap::new(),
            start_instant: Instant::now(),
            frame_driver: FrameDriver::new(scheduler_config),
            frame_output: OutputId(0),
            next_frame_index: 0,
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

    /// Returns the hosted UI mutably.
    pub fn ui_mut(&mut self) -> &mut Ui {
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
    /// Returns `true` when a UI timer, timeline cue, or active presentation
    /// track was due. Due UI wakeups are treated as animation demand because
    /// they require another sampled frame.
    pub fn pump_timers_now(&mut self) -> bool {
        self.pump_timers(self.now())
    }

    /// Pumps all timers that are due at `now`.
    ///
    /// Returns `true` when a UI timer, timeline cue, or active presentation
    /// track was due.
    pub fn pump_timers(&mut self, now: TimerInstant) -> bool {
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

    /// Returns the start time for the queued frame, if one is waiting.
    #[must_use]
    pub fn pending_frame_start(&self) -> Option<HostTime> {
        self.frame_driver.next_frame_start()
    }

    /// Returns the latest submitted frameclock timing summary.
    #[must_use]
    pub fn last_frame_timing_summary(&self) -> Option<FrameTimingSummary> {
        self.last_frame_timing
    }

    /// Dispatches a normalized pointer event into the UI.
    pub fn dispatch_pointer_event(&mut self, event: &PointerEvent) -> HostDispatchResult {
        let result = self.ui.dispatch_pointer_event(event);
        let mut result = HostDispatchResult::from_pointer_event(event, result);
        self.include_active_presentation_motion(&mut result);
        result
    }

    /// Dispatches a normalized keyboard event into the UI.
    pub fn dispatch_key(&mut self, event: &KeyboardEvent) -> HostDispatchResult {
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
        self.prepare_frame_inner(viewport, self.now(), FrameDemand::NONE)
    }

    /// Lays out and paints without clearing existing UI frame counters first.
    ///
    /// Use this for latency probes that reset counters before dispatching or
    /// mutating input, then want the prepared-frame diagnostics to include that
    /// input's invalidation marks.
    pub fn prepare_frame_preserving_stats(&mut self, viewport: FrameViewport) -> PreparedFrame<'_> {
        self.discard_active_frame();
        self.prepare_frame_inner(viewport, self.now(), FrameDemand::NONE)
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
        timing: HostFrameTiming,
    ) -> Option<PreparedFrame<'_>> {
        let planned = self.take_ready_frame(timing)?;
        let demand = planned.plan().demand;
        let sample_time = planned.sample_time().ticks();
        self.active_frame = Some(planned);
        self.ui.reset_frame_stats();
        Some(self.prepare_frame_inner(viewport, sample_time, demand))
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
        prepared_demand: FrameDemand,
    ) -> PreparedFrame<'_> {
        let total_start = Instant::now();

        let phase_start = Instant::now();
        self.ui.set_now(sample_time);
        let set_now = phase_start.elapsed();

        let phase_start = Instant::now();
        self.ui.compute_layout(viewport.logical_size());
        let layout = phase_start.elapsed();

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
        PreparedFrame {
            scene,
            demand: prepared_demand,
        }
    }

    /// Returns diagnostics from the most recent frame preparation call.
    #[must_use]
    pub fn last_frame_diagnostics(&self) -> FrameDiagnostics {
        self.last_frame_diagnostics
    }

    /// Records backend submission facts for the most recently submitted frame.
    ///
    /// Windowing/rendering hosts call this after they submit the prepared scene
    /// to their backend so core inspection snapshots can report renderer work,
    /// presentation feedback, and frameclock scheduling facts separately from
    /// Overstory's scene preparation pipeline.
    pub fn record_frame_submission(&mut self, submission: HostFrameSubmission) {
        self.ui.set_renderer_timings(submission.renderer_timings);
        if let Some(active) = self.active_frame.take() {
            let submitted_at = submission.submitted_at.unwrap_or_else(|| self.frame_now());
            let submit = self.frame_driver.submit_frame(
                active,
                FrameSubmission {
                    submitted_at,
                    presentation: submission.presentation,
                },
            );
            if let Some(summary) = submit.summary {
                self.record_frame_timing_summary(summary);
            }
            self.next_frame_index = self.next_frame_index.wrapping_add(1);
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

    fn instant_for_deadline(&self, deadline: TimerInstant) -> Instant {
        self.start_instant + Duration::from_nanos(deadline)
    }

    fn next_wakeup_deadline(&self) -> Option<TimerInstant> {
        min_deadline(
            self.ui.next_wakeup_deadline(),
            self.frame_driver.next_frame_start().map(HostTime::ticks),
        )
    }

    fn take_ready_frame(&mut self, timing: HostFrameTiming) -> Option<ActiveFrame> {
        let tick = FrameTick {
            now: timing.now,
            predicted_present: timing.predicted_present,
            refresh_interval: timing.refresh_interval,
            frame_index: self.next_frame_index,
            output: self.frame_output,
            prev_actual_present: timing.prev_actual_present,
        };
        let begin = self.frame_driver.begin_frame(FrameOpportunity::new(
            tick,
            timing.hints,
            timing.display_timing,
        ));
        if let Some(summary) = begin.resolved_feedback {
            self.record_frame_timing_summary(summary);
        }
        match begin.result {
            FrameBeginResult::Ready(frame) => Some(frame),
            FrameBeginResult::Expired(summary) => {
                let demand = summary.demand;
                self.record_frame_timing_summary(summary);
                self.next_frame_index = self.next_frame_index.wrapping_add(1);
                self.frame_driver.request(demand);
                None
            }
            FrameBeginResult::Idle | FrameBeginResult::WaitUntil(_) => None,
        }
    }

    fn discard_active_frame(&mut self) {
        if let Some(active) = self.active_frame.take() {
            let summary = self.frame_driver.discard_frame(active);
            self.record_frame_timing_summary(summary);
            self.next_frame_index = self.next_frame_index.wrapping_add(1);
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
    use core::cell::Cell;
    use frameclock::diagnostics::{FrameDropReason, FrameTimingBasis};
    use kurbo::{Point, Size, Vec2};
    use overstory::motion::{
        EffectClip, EffectKeyframe, EffectRule, EffectRules, EffectTriggerId, PresentationSlot,
        PresentationTarget, PresentationValue,
    };
    use overstory::{
        Button, ChoiceOption, ElementId, Layout, SegmentedControl, segmented_item,
        selected_segmented_index,
    };
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
    fn frame_submission_records_renderer_timings_in_ui_diagnostics() {
        let mut host = UiHost::new(Ui::new());
        let timings = renderer_timings();

        host.record_frame_submission(HostFrameSubmission::new(timings));

        assert_eq!(host.last_frame_diagnostics().ui.renderer_timings, timings);
        assert_eq!(host.ui().frame_stats().renderer_timings, timings);
    }

    #[test]
    fn duration_until_next_wakeup_can_use_external_clock_domain() {
        let mut host = UiHost::new(Ui::new());

        host.ui_mut().schedule_timeline_cue(10, 1);

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
            .prepare_requested_frame(test_viewport(), HostFrameTiming::pacing_only(HostTime(0)))
            .expect("input demand should produce an immediate prepared frame");
        assert_eq!(prepared.demand(), FrameDemand::INPUT);
        assert_eq!(
            host.last_frame_diagnostics().ui.prepared_demand,
            FrameDemand::INPUT
        );
        assert_eq!(host.ui().frame_stats().prepared_demand, FrameDemand::INPUT);
        assert_eq!(host.last_frame_timing_summary(), None);

        host.record_frame_submission(
            HostFrameSubmission::new(renderer_timings()).with_submitted_at(HostTime(1)),
        );

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
        let timing = HostFrameTiming::pacing_only_with_refresh_interval(
            HostTime(10),
            FrameDuration(8_333_333),
        );

        assert_eq!(timing.now, HostTime(10));
        assert_eq!(timing.predicted_present, None);
        assert_eq!(timing.refresh_interval, Some(8_333_333));
        assert_eq!(timing.hints.latest_commit(), HostTime(8_333_343));
        assert_eq!(
            timing.display_timing,
            DisplayTiming::fixed(FrameDuration(8_333_333))
        );
    }

    #[test]
    fn discarded_requested_frame_publishes_drop_timing_summary() {
        let mut host = UiHost::new(Ui::new());

        host.request_frame(FrameDemand::INPUT);
        assert!(
            host.prepare_requested_frame(
                test_viewport(),
                HostFrameTiming::pacing_only(HostTime(0))
            )
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
    fn retained_pending_demand_survives_submitted_frame() {
        let mut host = UiHost::new(Ui::new());

        host.request_frame(FrameDemand::ANIMATION);
        assert!(
            host.prepare_requested_frame(
                test_viewport(),
                HostFrameTiming::pacing_only(HostTime(0))
            )
            .is_none()
        );
        let frame_start = host
            .pending_frame_start()
            .expect("animation should queue a future frame start");

        host.request_frame(FrameDemand::BACKGROUND);
        assert_eq!(host.pending_frame_demand(), FrameDemand::BACKGROUND);
        let prepared = host
            .prepare_requested_frame(test_viewport(), HostFrameTiming::pacing_only(frame_start))
            .expect("queued animation should produce a prepared frame at frame start");
        assert_eq!(prepared.demand(), FrameDemand::ANIMATION);

        host.record_frame_submission(
            HostFrameSubmission::new(renderer_timings())
                .with_submitted_at(frame_start + FrameDuration(1)),
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
                .prepare_requested_frame(test_viewport(), HostFrameTiming::pacing_only(HostTime(0)))
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
            host.prepare_requested_frame(
                test_viewport(),
                HostFrameTiming::pacing_only(HostTime(0))
            )
            .is_none()
        );
        assert!(
            host.pending_frame_start().is_some(),
            "animation demand should queue a planned frame"
        );

        assert!(
            host.prepare_requested_frame(
                test_viewport(),
                HostFrameTiming::pacing_only(HostTime(1_000_000_000))
            )
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
            host.prepare_requested_frame(
                test_viewport(),
                HostFrameTiming::pacing_only(HostTime(0))
            )
            .is_some()
        );
        host.record_frame_submission(
            HostFrameSubmission::new(renderer_timings())
                .with_submitted_at(HostTime(1))
                .with_actual_present(HostTime(2)),
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
            host.prepare_requested_frame(
                test_viewport(),
                HostFrameTiming::pacing_only(HostTime(0))
            )
            .is_some()
        );
        host.record_frame_submission(
            HostFrameSubmission::new(renderer_timings())
                .with_submitted_at(HostTime(1))
                .with_deferred_present_feedback(),
        );
        assert_eq!(host.last_frame_timing_summary(), None);

        assert!(
            host.prepare_requested_frame(
                test_viewport(),
                HostFrameTiming::pacing_only(HostTime(3)).with_previous_actual_present(HostTime(2))
            )
            .is_none()
        );

        let summary = host
            .last_frame_timing_summary()
            .expect("next tick should resolve deferred presentation feedback");
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
