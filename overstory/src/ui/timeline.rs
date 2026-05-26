// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Presentation timeline integration for [`Ui`].
//!
//! `Ui` advances the timeline from host-supplied monotonic time, converts
//! sampled tracks into ordinary invalidation, and exposes wakeup deadlines to
//! host crates. Controls can request transitions, but the runtime remains the
//! owner of active tracks.

use alloc::vec::Vec;

use understory_timing::{TimerDuration, TimerInstant};

use crate::channels::COMPOSE;
use crate::element::ElementId;
use crate::motion::{TransitionSpec, TransitionTrigger};
use crate::timeline_runtime::{
    AnimatableValue, ChangeCause, PresentationValue, TimelineCue, TimelineCueId, TimelineSample,
    TimelineTarget, TimelineTrackKey, VisualOutput,
};

use super::Ui;

impl Ui {
    pub(crate) fn request_presentation_transition(
        &mut self,
        element: ElementId,
        target: TimelineTarget,
        spec: TransitionSpec,
    ) {
        self.timeline
            .request_transition(TimelineTrackKey::new(element, target), spec);
    }

    pub(crate) fn request_styled_presentation_transition(
        &mut self,
        element: ElementId,
        target: TimelineTarget,
    ) -> bool {
        let Some(spec) = self.transition_spec_for(element, target, ChangeCause::Interaction) else {
            return false;
        };
        self.request_presentation_transition(element, target, spec);
        true
    }

    pub(crate) fn has_pending_presentation_transition(
        &self,
        element: ElementId,
        target: TimelineTarget,
    ) -> bool {
        self.timeline
            .has_pending_transition(TimelineTrackKey::new(element, target))
    }

    pub(crate) fn set_presentation_target<T: AnimatableValue>(
        &mut self,
        element: ElementId,
        timeline_target: TimelineTarget,
        value: T,
        cause: ChangeCause,
    ) -> bool {
        let key = TimelineTrackKey::new(element, timeline_target);
        if let TimelineTarget::Property(property) = timeline_target
            && let Some(adapter) = self.timeline_properties.adapter(property)
        {
            debug_assert_eq!(
                adapter.kind(),
                T::KIND,
                "timeline sample kind must match the property adapter"
            );
        }
        let needs_sample = self.timeline.set_target(key, value, self.now(), cause);
        self.sync_timeline_property_sample(key);
        if needs_sample {
            self.mark_timeline_target_channels(element, key.target());
        }
        needs_sample
    }

    fn transition_spec_for(
        &self,
        element: ElementId,
        target: TimelineTarget,
        cause: ChangeCause,
    ) -> Option<TransitionSpec> {
        let trigger = match cause {
            ChangeCause::Interaction => TransitionTrigger::Interaction,
            ChangeCause::Other => return None,
        };
        let element = self.get(element)?;
        self.effective_value_for(element, self.properties.transitions)
            .rule_for(target.transition_target(), trigger)
            .map(|rule| rule.spec())
    }

    pub(crate) fn presented_layer_opacity(&self, element: ElementId) -> f32 {
        let property = self.properties.opacity;
        let target = self
            .get(element)
            .map(|el| self.effective_value_for(el, property))
            .unwrap_or(1.0);
        let key = TimelineTrackKey::new(element, TimelineTarget::Property(property.id()));
        self.timeline.sample(key, target, self.now())
    }

    pub(crate) fn presented_value<T: AnimatableValue>(
        &self,
        element: ElementId,
        property: TimelineTarget,
        target: T,
    ) -> T {
        self.timeline
            .sample(TimelineTrackKey::new(element, property), target, self.now())
    }

    pub(in crate::ui) fn advance_timeline(&mut self, now: TimerInstant) {
        let samples = self.timeline.advance(now);
        for sample in samples {
            self.apply_timeline_sample(sample);
        }
    }

    fn apply_timeline_sample(&mut self, sample: TimelineSample) {
        let key = sample.key();
        if sample.is_complete() {
            self.clear_timeline_property_sample(key);
        } else {
            self.set_timeline_property_sample(key, sample.value());
        }
        self.mark_timeline_target_channels(key.element(), key.target());
    }

    fn sync_timeline_property_sample(&mut self, key: TimelineTrackKey) {
        if let Some(value) = self.timeline.active_sample_value(key, self.now()) {
            self.set_timeline_property_sample(key, value);
        } else {
            self.clear_timeline_property_sample(key);
        }
    }

    fn set_timeline_property_sample(&mut self, key: TimelineTrackKey, value: PresentationValue) {
        let TimelineTarget::Property(property) = key.target() else {
            return;
        };
        let Some(adapter) = self.timeline_properties.adapter(property) else {
            return;
        };
        let Some(element) = self.get_mut(key.element()) else {
            return;
        };
        adapter.set_sample(element, property, value);
    }

    pub(in crate::ui) fn clear_timeline_property_sample(&mut self, key: TimelineTrackKey) {
        let TimelineTarget::Property(property) = key.target() else {
            return;
        };
        let Some(adapter) = self.timeline_properties.adapter(property) else {
            return;
        };
        let Some(element) = self.get_mut(key.element()) else {
            return;
        };
        adapter.clear_sample(element, property);
    }

    fn mark_timeline_target_channels(&mut self, element: ElementId, target: TimelineTarget) {
        let channels = match target {
            TimelineTarget::Property(property) => self.registry.affects_channels(property),
            TimelineTarget::VisualOutput(VisualOutput::LayerOffset) => COMPOSE.into_set(),
        };
        self.mark_channels(element, channels);
    }

    /// Schedules an app-level timeline cue relative to [`Self::now`].
    ///
    /// Cues are deterministic wakeup points for side effects that are not
    /// themselves property animations. Overstory stores the opaque `payload`
    /// token and returns it when the cue is drained; callers decide what that
    /// token means.
    pub fn schedule_timeline_cue(&mut self, delay: TimerDuration, payload: u64) -> TimelineCueId {
        self.timeline
            .schedule_cue(None, self.now().saturating_add(delay), payload)
    }

    /// Schedules an element-scoped timeline cue relative to [`Self::now`].
    ///
    /// Returns `None` if `target` is stale. Element-scoped cues are removed
    /// automatically when the target element is removed.
    pub fn schedule_element_timeline_cue(
        &mut self,
        target: ElementId,
        delay: TimerDuration,
        payload: u64,
    ) -> Option<TimelineCueId> {
        self.get(target)?;
        Some(
            self.timeline
                .schedule_cue(Some(target), self.now().saturating_add(delay), payload),
        )
    }

    /// Cancels a pending timeline cue.
    ///
    /// Returns `true` if the cue was still pending and is now removed.
    pub fn cancel_timeline_cue(&mut self, cue: TimelineCueId) -> bool {
        self.timeline.cancel_cue(cue)
    }

    /// Drains timeline cues due at or before `now`.
    ///
    /// This updates [`Self::now`] first, so due cue delivery and animation
    /// sampling observe the same host time.
    pub fn drain_timeline_cues(&mut self, now: TimerInstant) -> Vec<TimelineCue> {
        self.set_now(now);
        self.timeline.drain_cues(now)
    }

    /// Returns the next timer, presentation-track, or timeline-cue deadline the
    /// host should wake for.
    #[must_use]
    pub fn next_wakeup_deadline(&self) -> Option<TimerInstant> {
        min_deadline(
            self.next_timer_deadline(),
            self.timeline.next_deadline(self.now()),
        )
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
