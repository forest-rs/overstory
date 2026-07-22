// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::boxed::Box;
use alloc::vec::Vec;

use hashbrown::HashMap;
use understory_animation::{AnimationPlayState, TimingSample};
use understory_animation_timeline::{ManualTimeline, TimelineTime};
use understory_timing::{TimerDuration, TimerInstant};

use crate::element::ElementId;
use crate::motion::{Animatable, EffectClip, PresentationValue, PresentationValueKind};

use super::cue::{TimelineCue, TimelineCueId};
use super::effect::PresentationEffect;
use super::instance::{AnimationEvent, AnimationId, next_animation_id};
use super::target::{ErasedTargetState, TimelineTargetControlResult, TimelineTargetSample};
use super::timeline::{AnimationTimelineId, DocumentTimeline, TimelineClock};
use super::types::{
    ChangeCause, PresentationRuntimeValue, TimelineAnimationRuntimeRow, TimelineEvaluationDomain,
    TimelineTrackKey, evaluation_domain, expected_value_kind,
};
use super::value::{
    AnimationInstance, NativeAnimationInstance, NativeTargetState, ResolvedTransition,
    TimelineTargetState,
};

/// Internal runtime for sampled timeline values.
///
/// Host monotonic instants enter through the document timeline adapter. Target
/// stacks and retained animation instances then operate on
/// [`TimelineTime`], keeping animation playback math independent from host
/// timer aliases. Retargeting starts from the current sampled value, not the
/// previous target value.
#[derive(Default)]
pub(crate) struct TimelineRuntime {
    inspection_revision: u64,
    document_timeline: DocumentTimeline,
    manual_timelines: HashMap<AnimationTimelineId, ManualTimeline>,
    next_timeline_id: u64,
    targets: HashMap<TimelineTrackKey, Box<dyn ErasedTargetState>>,
    pending: HashMap<TimelineTrackKey, ResolvedTransition>,
    cues: Vec<TimelineCue>,
    next_cue_id: u64,
    animation_events: Vec<AnimationEvent>,
    next_animation_id: u64,
}

impl TimelineRuntime {
    /// Creates an empty runtime.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Returns an opaque revision of the state exposed by timeline inspection.
    #[must_use]
    pub(crate) const fn inspection_revision(&self) -> u64 {
        self.inspection_revision
    }

    /// Creates a seekable manual animation timeline starting at zero.
    pub(crate) fn create_manual_timeline(&mut self) -> AnimationTimelineId {
        let id =
            AnimationTimelineId::new(self.next_timeline_id.max(AnimationTimelineId::FIRST_MANUAL));
        self.next_timeline_id = id.get().wrapping_add(1);
        self.manual_timelines
            .insert(id, ManualTimeline::at(TimelineTime::ZERO));
        id
    }

    /// Returns the current time for a manual animation timeline.
    #[must_use]
    pub(crate) fn manual_timeline_time(
        &self,
        timeline: AnimationTimelineId,
    ) -> Option<TimerDuration> {
        self.manual_timelines
            .get(&timeline)
            .and_then(|timeline| timeline.time())
            .map(TimelineTime::duration)
    }

    /// Returns whether `timeline` is a live animation timeline.
    #[must_use]
    pub(crate) fn has_timeline(&self, timeline: AnimationTimelineId) -> bool {
        timeline == AnimationTimelineId::DOCUMENT || self.manual_timelines.contains_key(&timeline)
    }

    /// Seeks a manual animation timeline and returns affected samples.
    pub(crate) fn seek_manual_timeline(
        &mut self,
        timeline: AnimationTimelineId,
        time: TimerDuration,
        now: TimerInstant,
    ) -> Option<Vec<TimelineTargetSample>> {
        let manual = self.manual_timelines.get_mut(&timeline)?;
        manual.seek(TimelineTime::from_duration(time));
        Some(self.advance(now))
    }

    /// Schedules a cue for `due_at`.
    pub(crate) fn schedule_cue(
        &mut self,
        target: Option<ElementId>,
        due_at: TimerInstant,
        payload: u64,
    ) -> TimelineCueId {
        let id = TimelineCueId(self.next_cue_id);
        self.next_cue_id = self.next_cue_id.wrapping_add(1);
        self.cues.push(TimelineCue {
            id,
            target,
            payload,
            due_at,
        });
        self.record_inspection_change();
        id
    }

    /// Cancels a pending cue by id.
    pub(crate) fn cancel_cue(&mut self, id: TimelineCueId) -> bool {
        let before = self.cues.len();
        self.cues.retain(|cue| cue.id != id);
        let canceled = self.cues.len() != before;
        if canceled {
            self.record_inspection_change();
        }
        canceled
    }

    /// Drains all app-visible cues due at or before `now`.
    pub(crate) fn drain_cues(&mut self, now: TimerInstant) -> Vec<TimelineCue> {
        let mut due = Vec::new();
        let mut pending = Vec::new();
        for cue in self.cues.drain(..) {
            if cue.due_at <= now {
                due.push(cue);
            } else {
                pending.push(cue);
            }
        }
        self.cues = pending;
        due.sort_by_key(|cue| (cue.due_at, cue.id.get()));
        if !due.is_empty() {
            self.record_inspection_change();
        }
        due
    }

    /// Requests that the next target change for `key` animate when it is caused
    /// by interaction.
    pub(crate) fn request_transition(&mut self, key: TimelineTrackKey, spec: ResolvedTransition) {
        self.pending.insert(key, spec);
        self.record_inspection_change();
    }

    /// Applies a finite visual effect clip at `now`, appending it above the
    /// current target stack.
    #[cfg(test)]
    pub(crate) fn start_effect(
        &mut self,
        key: TimelineTrackKey,
        clip: EffectClip,
        now: TimerInstant,
    ) -> AnimationId {
        self.start_effect_on_timeline(key, clip, AnimationTimelineId::DOCUMENT, now)
    }

    /// Applies a finite visual effect clip on `timeline`, appending it above
    /// the current target stack.
    pub(crate) fn start_effect_on_timeline(
        &mut self,
        key: TimelineTrackKey,
        clip: EffectClip,
        timeline: AnimationTimelineId,
        now: TimerInstant,
    ) -> AnimationId {
        if let Some(kind) = expected_value_kind(key.target()) {
            debug_assert_eq!(
                kind,
                clip.value_kind(),
                "effect clip value kind must match target metadata"
            );
        }
        debug_assert_ne!(
            evaluation_domain(key.target()),
            Some(TimelineEvaluationDomain::Layout),
            "layout-domain effects need explicit layout-animation policy"
        );
        debug_assert!(
            timeline == AnimationTimelineId::DOCUMENT
                || self.manual_timelines.contains_key(&timeline),
            "manual animation timeline must exist before starting an effect on it",
        );
        let timeline_time = self.timeline_start_time(timeline, now);
        let id = next_animation_id(&mut self.next_animation_id);
        let instance = AnimationInstance::new(
            id,
            timeline,
            PresentationEffect::effect(clip),
            timeline_time,
        );
        let state = self
            .targets
            .entry(key)
            .or_insert_with(|| Box::<TimelineTargetState>::default())
            .as_any_mut()
            .downcast_mut::<TimelineTargetState>()
            .expect("presentation timeline target must keep presentation state");
        state.set_effect(key, instance, now, &mut self.animation_events);
        self.record_inspection_change();
        id
    }

    /// Applies a native finite effect clip on `timeline`, appending it above
    /// the current native target stack.
    pub(crate) fn start_native_effect_on_timeline<T>(
        &mut self,
        key: TimelineTrackKey,
        clip: EffectClip<T>,
        timeline: AnimationTimelineId,
        now: TimerInstant,
        value_kind: Option<PresentationValueKind>,
    ) -> Option<AnimationId>
    where
        T: Animatable,
    {
        if !self.has_timeline(timeline) {
            return None;
        }
        debug_assert_ne!(
            evaluation_domain(key.target()),
            Some(TimelineEvaluationDomain::Layout),
            "layout-domain effects need explicit layout-animation policy"
        );
        let timeline_time = self.timeline_start_time(timeline, now);
        let id = next_animation_id(&mut self.next_animation_id);
        let instance =
            NativeAnimationInstance::new(id, timeline, clip.into_stack_effect(), timeline_time);
        let state = self
            .targets
            .entry(key)
            .or_insert_with(|| Box::<NativeTargetState<T>>::default())
            .as_any_mut()
            .downcast_mut::<NativeTargetState<T>>()?;
        state.set_value_kind(value_kind);
        state.set_effect(key, instance, now, &mut self.animation_events);
        self.record_inspection_change();
        Some(id)
    }

    /// Returns whether a transition spec is waiting for `key`'s next target.
    #[must_use]
    pub(crate) fn has_pending_transition(&self, key: TimelineTrackKey) -> bool {
        self.pending.contains_key(&key)
    }

    /// Returns whether `key` has a resolved target value.
    #[must_use]
    pub(crate) fn has_target(&self, key: TimelineTrackKey) -> bool {
        self.targets
            .get(&key)
            .is_some_and(|state| state.has_target())
    }

    /// Removes the target, active track, and pending transition for `key`.
    pub(crate) fn clear_target(&mut self, key: TimelineTrackKey, now: TimerInstant) -> bool {
        let had_state = self.targets.remove(&key).is_some_and(|mut state| {
            state.cancel_all(key, now, &mut self.animation_events);
            true
        });
        let had_pending = self.pending.remove(&key).is_some();
        let cleared = had_state || had_pending;
        if cleared {
            self.record_inspection_change();
        }
        cleared
    }

    /// Records the resolved target value for `key`, planning or retargeting
    /// its timeline track when policy allows.
    #[cfg(test)]
    pub(crate) fn set_target<T: PresentationRuntimeValue>(
        &mut self,
        key: TimelineTrackKey,
        target: T,
        now: TimerInstant,
        cause: ChangeCause,
    ) -> bool {
        self.set_target_value(key, target.into_presentation_value(), now, cause)
    }

    /// Records an erased resolved target value for `key`.
    pub(crate) fn set_target_value(
        &mut self,
        key: TimelineTrackKey,
        target: PresentationValue,
        now: TimerInstant,
        cause: ChangeCause,
    ) -> bool {
        if let Some(kind) = expected_value_kind(key.target()) {
            debug_assert_eq!(
                kind,
                target.kind(),
                "timeline target kind must match target metadata"
            );
        }
        debug_assert_ne!(
            evaluation_domain(key.target()),
            Some(TimelineEvaluationDomain::Layout),
            "layout-domain animation must not be sampled by the generic target setter"
        );
        let (remove_state, result) = {
            let state = self
                .targets
                .entry(key)
                .or_insert_with(|| Box::<TimelineTargetState>::default())
                .as_any_mut()
                .downcast_mut::<TimelineTargetState>()
                .expect("presentation timeline target must keep presentation state");
            let spec = if cause == ChangeCause::Interaction {
                self.pending.remove(&key)
            } else {
                self.pending.remove(&key);
                None
            };
            let result = state.set_target(target, spec, now);
            (state.is_empty(), result)
        };
        if remove_state {
            self.targets.remove(&key);
        }
        self.record_inspection_change();
        result
    }

    /// Records a native resolved target value for `key`.
    pub(crate) fn set_native_target_value<T>(
        &mut self,
        key: TimelineTrackKey,
        target: T,
        now: TimerInstant,
        cause: ChangeCause,
        value_kind: Option<PresentationValueKind>,
    ) -> Option<bool>
    where
        T: Animatable,
    {
        debug_assert_ne!(
            evaluation_domain(key.target()),
            Some(TimelineEvaluationDomain::Layout),
            "layout-domain animation must not be sampled by the generic target setter"
        );
        let (remove_state, result) = {
            let state = self
                .targets
                .entry(key)
                .or_insert_with(|| Box::<NativeTargetState<T>>::default())
                .as_any_mut()
                .downcast_mut::<NativeTargetState<T>>()?;
            state.set_value_kind(value_kind);
            let spec = if cause == ChangeCause::Interaction {
                self.pending.remove(&key)
            } else {
                self.pending.remove(&key);
                None
            };
            let result = state.set_target(target, spec, now);
            (state.is_empty(), result)
        };
        if remove_state {
            self.targets.remove(&key);
        }
        self.record_inspection_change();
        Some(result)
    }

    /// Returns the sampled timeline value for `key`.
    #[must_use]
    pub(crate) fn sample<T: PresentationRuntimeValue>(
        &self,
        key: TimelineTrackKey,
        target: T,
        now: TimerInstant,
    ) -> T {
        let value = self.sample_value(key, target.into_presentation_value(), now);
        T::from_presentation_value(value).unwrap_or(target)
    }

    /// Returns the sampled timeline value for `key`.
    #[must_use]
    pub(crate) fn sample_value(
        &self,
        key: TimelineTrackKey,
        target: PresentationValue,
        now: TimerInstant,
    ) -> PresentationValue {
        let clock = self.clock(now);
        self.presentation_state(key)
            .and_then(|state| state.sample(Some(target), clock))
            .unwrap_or(target)
    }

    /// Returns the sampled value for an active track, if one exists.
    #[must_use]
    pub(crate) fn active_sample(
        &self,
        key: TimelineTrackKey,
        now: TimerInstant,
    ) -> Option<TimelineTargetSample> {
        let clock = self.clock(now);
        self.targets
            .get(&key)
            .and_then(|state| state.active_sample(key, clock))
    }

    /// Returns the sampled presentation value for an active track, if one exists.
    #[must_use]
    pub(crate) fn active_sample_value(
        &self,
        key: TimelineTrackKey,
        now: TimerInstant,
    ) -> Option<PresentationValue> {
        self.active_sample(key, now)
            .and_then(|sample| sample.into_presentation())
            .map(|sample| sample.value())
    }

    /// Advances active targets, removing completed clips and returning
    /// affected samples that should be applied to their targets.
    pub(crate) fn advance(&mut self, now: TimerInstant) -> Vec<TimelineTargetSample> {
        let document_timeline = self.document_timeline;
        let manual_timelines = &self.manual_timelines;
        let mut samples = Vec::new();
        let mut empty = Vec::new();
        for (&key, state) in &mut self.targets {
            if !state.has_active_motion() {
                continue;
            }
            let clock = TimelineClock::new(document_timeline, now, manual_timelines);
            if let Some(sample) = state.advance(key, clock, now, &mut self.animation_events) {
                samples.push(sample);
            }
            if state.is_empty() {
                empty.push(key);
            }
        }
        for key in empty {
            self.targets.remove(&key);
        }
        if !samples.is_empty() {
            self.record_inspection_change();
        }
        samples
    }

    /// Returns the playback state for `id`, if the instance is still retained.
    #[must_use]
    pub(crate) fn animation_state(&self, id: AnimationId) -> Option<AnimationPlayState> {
        self.targets
            .values()
            .find_map(|state| state.animation_state(id))
    }

    /// Returns the local animation time for `id` at `now`.
    #[must_use]
    pub(crate) fn animation_current_time(
        &self,
        id: AnimationId,
        now: TimerInstant,
    ) -> Option<TimerDuration> {
        let clock = self.clock(now);
        self.targets
            .values()
            .find_map(|state| state.animation_current_time(id, clock))
    }

    /// Returns the timing sample for `id` at `now`.
    #[must_use]
    pub(crate) fn animation_timing_sample(
        &self,
        id: AnimationId,
        now: TimerInstant,
    ) -> Option<TimingSample> {
        let clock = self.clock(now);
        self.targets
            .values()
            .find_map(|state| state.animation_timing_sample(id, clock))
    }

    /// Returns the playback rate for `id`, if the instance is still retained.
    #[must_use]
    pub(crate) fn animation_playback_rate(&self, id: AnimationId) -> Option<f64> {
        self.targets
            .values()
            .find_map(|state| state.animation_playback_rate(id))
    }

    /// Pauses a running animation instance.
    pub(crate) fn pause_animation(
        &mut self,
        id: AnimationId,
        now: TimerInstant,
    ) -> Option<TimelineTargetControlResult> {
        self.update_animation(now, |state, key, clock, _| state.pause(id, key, clock))
    }

    /// Starts or resumes an animation instance.
    pub(crate) fn play_animation(
        &mut self,
        id: AnimationId,
        now: TimerInstant,
    ) -> Option<TimelineTargetControlResult> {
        self.update_animation(now, |state, key, clock, _| state.play(id, key, clock))
    }

    /// Seeks an animation instance to `local_time`.
    pub(crate) fn seek_animation(
        &mut self,
        id: AnimationId,
        local_time: TimerDuration,
        now: TimerInstant,
    ) -> Option<TimelineTargetControlResult> {
        self.update_animation(now, |state, key, clock, _| {
            state.seek(id, key, local_time, clock)
        })
    }

    /// Reverses an animation instance.
    pub(crate) fn reverse_animation(
        &mut self,
        id: AnimationId,
        now: TimerInstant,
    ) -> Option<TimelineTargetControlResult> {
        self.update_animation(now, |state, key, clock, _| state.reverse(id, key, clock))
    }

    /// Sets an animation instance playback rate.
    pub(crate) fn set_animation_playback_rate(
        &mut self,
        id: AnimationId,
        playback_rate: f64,
        now: TimerInstant,
    ) -> Option<TimelineTargetControlResult> {
        self.update_animation(now, |state, key, clock, _| {
            state.set_playback_rate(id, key, playback_rate, clock)
        })
    }

    /// Cancels a retained animation instance.
    pub(crate) fn cancel_animation(
        &mut self,
        id: AnimationId,
        now: TimerInstant,
    ) -> Option<TimelineTargetControlResult> {
        self.update_animation(now, |state, key, clock, events| {
            state.cancel_by_id(id, key, now, clock, events)
        })
    }

    /// Finishes a retained animation instance at its natural endpoint.
    pub(crate) fn finish_animation(
        &mut self,
        id: AnimationId,
        now: TimerInstant,
    ) -> Option<TimelineTargetControlResult> {
        self.update_animation(now, |state, key, clock, events| {
            state.finish_by_id(id, key, now, clock, events)
        })
    }

    /// Returns the next deadline a host should wake for timeline cues.
    #[must_use]
    pub(crate) fn next_cue_deadline(&self, now: TimerInstant) -> Option<TimerInstant> {
        self.cues.iter().map(|cue| cue.due_at.max(now)).min()
    }

    /// Returns the number of retained timeline targets.
    #[must_use]
    pub(crate) fn target_count(&self) -> usize {
        self.targets
            .values()
            .filter(|state| state.has_target())
            .count()
    }

    /// Returns the number of active implicit transitions.
    #[must_use]
    pub(crate) fn active_transition_count(&self) -> usize {
        self.targets
            .values()
            .map(|state| state.running_transition_count())
            .sum()
    }

    /// Returns the number of active explicit or style-triggered effects.
    #[must_use]
    pub(crate) fn active_effect_count(&self) -> usize {
        self.targets
            .values()
            .map(|state| state.running_effect_count())
            .sum()
    }

    /// Returns retained animation instances for cold-path inspection.
    pub(crate) fn animation_rows(&self, now: TimerInstant) -> Vec<TimelineAnimationRuntimeRow> {
        let clock = self.clock(now);
        let mut rows = Vec::new();
        for (&key, state) in &self.targets {
            state.animation_rows(key, clock, &mut rows);
        }
        rows.sort_by_key(|row| row.id());
        rows
    }

    /// Returns whether any transition or effect is active.
    #[must_use]
    pub(crate) fn has_active_motion(&self) -> bool {
        self.targets
            .values()
            .any(|state| state.has_running_motion_on(AnimationTimelineId::DOCUMENT))
    }

    /// Returns the number of pending transition specs.
    #[must_use]
    pub(crate) fn pending_transition_count(&self) -> usize {
        self.pending.len()
    }

    /// Returns the number of pending timeline cues.
    #[must_use]
    pub(crate) fn pending_cue_count(&self) -> usize {
        self.cues.len()
    }

    /// Drains queued animation lifecycle events in runtime order.
    pub(crate) fn drain_animation_events(&mut self) -> Vec<AnimationEvent> {
        core::mem::take(&mut self.animation_events)
    }

    /// Removes all runtime state for `element`.
    pub(crate) fn remove_element(&mut self, element: ElementId, now: TimerInstant) {
        let targets_before = self.targets.len();
        let pending_before = self.pending.len();
        let cues_before = self.cues.len();
        let removed_keys: Vec<_> = self
            .targets
            .keys()
            .copied()
            .filter(|key| key.element() == element)
            .collect();
        for key in removed_keys {
            if let Some(mut state) = self.targets.remove(&key) {
                state.cancel_all(key, now, &mut self.animation_events);
            }
        }
        self.pending.retain(|key, _| key.element() != element);
        self.cues.retain(|cue| cue.target != Some(element));
        if self.targets.len() != targets_before
            || self.pending.len() != pending_before
            || self.cues.len() != cues_before
        {
            self.record_inspection_change();
        }
    }

    /// Returns whether `key` currently has an active timeline track.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn has_active_track(&self, key: TimelineTrackKey) -> bool {
        self.targets
            .get(&key)
            .is_some_and(|state| state.has_active_motion())
    }

    fn update_animation(
        &mut self,
        now: TimerInstant,
        mut update: impl for<'a> FnMut(
            &mut dyn ErasedTargetState,
            TimelineTrackKey,
            TimelineClock<'a>,
            &mut Vec<AnimationEvent>,
        ) -> Option<TimelineTargetControlResult>,
    ) -> Option<TimelineTargetControlResult> {
        let document_timeline = self.document_timeline;
        let manual_timelines = &self.manual_timelines;
        let keys: Vec<_> = self.targets.keys().copied().collect();
        for key in keys {
            let clock = TimelineClock::new(document_timeline, now, manual_timelines);
            let (result, remove_state) = {
                let state = self.targets.get_mut(&key)?;
                let Some(result) = update(state.as_mut(), key, clock, &mut self.animation_events)
                else {
                    continue;
                };
                (result, state.is_empty())
            };
            if remove_state {
                self.targets.remove(&key);
            }
            self.record_inspection_change();
            return Some(result);
        }
        None
    }

    fn record_inspection_change(&mut self) {
        self.inspection_revision = self
            .inspection_revision
            .checked_add(1)
            .expect("timeline inspection revisions exhausted");
    }

    fn clock(&self, now: TimerInstant) -> TimelineClock<'_> {
        TimelineClock::new(self.document_timeline, now, &self.manual_timelines)
    }

    fn timeline_start_time(
        &self,
        timeline: AnimationTimelineId,
        now: TimerInstant,
    ) -> TimelineTime {
        self.clock(now).time(timeline).unwrap_or(TimelineTime::ZERO)
    }

    fn presentation_state(&self, key: TimelineTrackKey) -> Option<&TimelineTargetState> {
        self.targets
            .get(&key)?
            .as_any()
            .downcast_ref::<TimelineTargetState>()
    }
}
