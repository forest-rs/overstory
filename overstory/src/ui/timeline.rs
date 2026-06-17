// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Presentation timeline integration for [`Ui`].
//!
//! `Ui` advances the timeline from host-supplied monotonic time, converts
//! sampled tracks into ordinary invalidation, and exposes wakeup deadlines to
//! host crates. Controls can request transitions, but the runtime remains the
//! owner of active tracks.

use alloc::vec::Vec;

use kurbo::Vec2;
use understory_timing::{TimerDuration, TimerInstant};

use crate::channels::COMPOSE;
use crate::element::ElementId;
use crate::motion::{
    EffectClip, EffectTriggerId, PresentationSlot, PresentationTarget, PresentationValue,
    ResolvedTransitionSpec, TransitionTrigger,
};
use crate::timeline_runtime::{
    AnimatableValue, ChangeCause, TimelineCue, TimelineCueId, TimelineSample, TimelineTrackKey,
};

use super::Ui;
use super::property_changes::{PropertyChange, StateProjectionPolicy, TemplateRefreshPolicy};

impl Ui {
    pub(crate) fn request_presentation_transition(
        &mut self,
        element: ElementId,
        target: PresentationTarget,
        spec: ResolvedTransitionSpec,
    ) {
        self.timeline
            .request_transition(TimelineTrackKey::new(element, target), spec);
    }

    pub(crate) fn request_styled_presentation_transition(
        &mut self,
        element: ElementId,
        target: PresentationTarget,
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
        target: PresentationTarget,
    ) -> bool {
        self.timeline
            .has_pending_transition(TimelineTrackKey::new(element, target))
    }

    /// Returns whether any presentation transition or one-shot effect is still
    /// active.
    ///
    /// Host runtimes use this after dispatch or sampling a frame to keep
    /// animation frame demand alive until all presentation motion has
    /// completed.
    #[must_use]
    pub fn has_active_presentation_motion(&self) -> bool {
        self.timeline.has_active_motion()
    }

    pub(in crate::ui) fn has_presentation_target(
        &self,
        element: ElementId,
        target: PresentationTarget,
    ) -> bool {
        self.timeline
            .has_target(TimelineTrackKey::new(element, target))
    }

    pub(in crate::ui) fn sync_composition_offset_target(
        &mut self,
        element: ElementId,
        offset: Vec2,
    ) {
        let target = PresentationTarget::Slot(PresentationSlot::LayerOffset);
        let has_pending = self.has_pending_presentation_transition(element, target);
        let should_track = has_pending
            || self.has_presentation_target(element, target)
            || self
                .transition_spec_for(element, target, ChangeCause::Interaction)
                .is_some();
        if !should_track {
            return;
        }
        let cause = if has_pending {
            ChangeCause::Interaction
        } else {
            ChangeCause::Other
        };
        self.set_presentation_target(element, target, offset, cause);
    }

    pub(crate) fn clear_presentation_target(
        &mut self,
        element: ElementId,
        target: PresentationTarget,
    ) {
        let key = TimelineTrackKey::new(element, target);
        let had_runtime_state = self.timeline.clear_target(key);
        self.clear_timeline_property_sample(key);
        if had_runtime_state {
            self.mark_timeline_target_channels(element, target);
        }
    }

    pub(crate) fn set_presentation_target<T: AnimatableValue>(
        &mut self,
        element: ElementId,
        timeline_target: PresentationTarget,
        value: T,
        cause: ChangeCause,
    ) -> bool {
        self.set_presentation_target_value(element, timeline_target, value.into_value(), cause)
    }

    pub(crate) fn set_presentation_target_value(
        &mut self,
        element: ElementId,
        timeline_target: PresentationTarget,
        target: PresentationValue,
        cause: ChangeCause,
    ) -> bool {
        let key = TimelineTrackKey::new(element, timeline_target);
        if let PresentationTarget::Property(property) = timeline_target
            && let Some(adapter) = self.timeline_properties.adapter(property)
        {
            debug_assert_eq!(
                adapter.kind(),
                target.kind(),
                "timeline sample kind must match the property adapter"
            );
        }
        let needs_sample = self
            .timeline
            .set_target_value(key, target, self.now(), cause);
        self.sync_timeline_property_sample(key);
        let presented = self.timeline.sample_value(key, target, self.now());
        self.set_timeline_composition_sample(key, presented);
        if needs_sample {
            self.mark_timeline_target_channels(element, key.target());
        }
        needs_sample
    }

    pub(in crate::ui) fn transition_spec_for(
        &self,
        element: ElementId,
        target: PresentationTarget,
        cause: ChangeCause,
    ) -> Option<ResolvedTransitionSpec> {
        let trigger = match cause {
            ChangeCause::Interaction => TransitionTrigger::Interaction,
            ChangeCause::Other => return None,
        };
        let element = self.get(element)?;
        self.resolve_value_for(element, self.properties.transitions)
            .rule_for(target, trigger)
            .map(|rule| rule.spec().resolve(self.theme()))
    }

    pub(crate) fn presented_layer_opacity(&self, element: ElementId) -> f32 {
        let property = self.properties.opacity;
        let target = self
            .get(element)
            .map(|el| self.resolve_value_for(el, property))
            .unwrap_or(1.0);
        let key = TimelineTrackKey::new(element, PresentationTarget::Property(property.id()));
        self.timeline.sample(key, target, self.now())
    }

    pub(crate) fn presented_value<T: AnimatableValue>(
        &self,
        element: ElementId,
        property: PresentationTarget,
        target: T,
    ) -> T {
        self.timeline
            .sample(TimelineTrackKey::new(element, property), target, self.now())
    }

    /// Triggers target-local style-authored one-shot effects for `element`.
    ///
    /// Controls call this for semantic interaction outcomes such as rejected
    /// activation. The caller chooses the target element; the target's
    /// resolved `Effects` property decides whether anything visual happens and
    /// which presentation targets receive clips.
    pub fn trigger_effect(&mut self, element: ElementId, trigger: EffectTriggerId) -> bool {
        let Some(element_ref) = self.get(element) else {
            return false;
        };
        let rules: Vec<_> = self
            .resolve_value_for(element_ref, self.properties.effects)
            .rules_for(trigger)
            .cloned()
            .collect();
        let mut started = false;
        for rule in rules {
            started |= self.start_presentation_effect(element, rule.target(), rule.clip().clone());
        }
        started
    }

    fn start_presentation_effect(
        &mut self,
        element: ElementId,
        target: PresentationTarget,
        clip: EffectClip,
    ) -> bool {
        if let PresentationTarget::Property(property) = target {
            let Some(adapter) = self.timeline_properties.adapter(property) else {
                return false;
            };
            if adapter.kind() != clip.value_kind() {
                debug_assert_eq!(
                    adapter.kind(),
                    clip.value_kind(),
                    "effect clip kind must match the property adapter"
                );
                return false;
            }
        }
        let key = TimelineTrackKey::new(element, target);
        self.timeline.start_effect(key, clip, self.now());
        self.sync_timeline_property_sample(key);
        if let Some(value) = self.timeline.active_sample_value(key, self.now()) {
            self.set_timeline_composition_sample(key, value);
        }
        self.mark_timeline_target_channels(element, key.target());
        true
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
        self.set_timeline_composition_sample(key, sample.value());
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
        let PresentationTarget::Property(property) = key.target() else {
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
        let PresentationTarget::Property(property) = key.target() else {
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

    fn set_timeline_composition_sample(&mut self, key: TimelineTrackKey, value: PresentationValue) {
        match (key.target(), value) {
            (
                PresentationTarget::Slot(PresentationSlot::LayerOffset),
                PresentationValue::Offset2d(offset),
            ) => self.composition.set_layer_offset(key.element(), offset),
            (
                PresentationTarget::Slot(PresentationSlot::VisualTranslation),
                PresentationValue::Offset2d(_),
            ) => self.composition.set_visual_transform(
                key.element(),
                self.presented_visual_transform(key.element()),
            ),
            (PresentationTarget::Property(property), PresentationValue::Scalar32(opacity))
                if property == self.properties.opacity.id() =>
            {
                self.composition.set_opacity(key.element(), opacity);
            }
            _ => {}
        }
    }

    fn mark_timeline_target_channels(&mut self, element: ElementId, target: PresentationTarget) {
        match target {
            PresentationTarget::Property(property) => {
                // Timeline property samples are effective-value writes at the
                // animation layer. Invalidation still follows the property
                // contract: inherited properties affect descendants, and
                // template children can declare owner-property dependencies.
                self.apply_property_change(PropertyChange {
                    element,
                    property,
                    channels: self.registry.affects_channels(property),
                    template_refresh: TemplateRefreshPolicy::DoNotRefreshTemplate,
                    state_projection: StateProjectionPolicy::DoNotProjectState,
                });
            }
            PresentationTarget::Slot(
                PresentationSlot::LayerOffset | PresentationSlot::VisualTranslation,
            ) => self.mark_channels(element, COMPOSE.into_set()),
        }
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
