// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Internal presentation timeline runtime.
//!
//! This is intentionally tiny: one implicit track per element/property and one
//! scalar transition clip kind. The shape leaves room for a future shared
//! timeline scheduler without making built-in controls own local timers.

use alloc::vec::Vec;

use hashbrown::HashMap;
use invalidation::ChannelSet;
use understory_timing::{TimerDuration, TimerInstant};

use crate::element::ElementId;

/// Nominal frame interval hosts can use while presentation tracks are active.
pub(crate) const PRESENTATION_FRAME_INTERVAL: TimerDuration = 16_666_667;

/// Presentation property whose sampled value may differ from its target value.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum PresentationProperty {
    /// Layer-local x offset sampled by composition while layout keeps the
    /// resolved target geometry.
    CompositionOffsetX,
}

/// Why a target value changed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChangeCause {
    /// User interaction changed semantic control state.
    Interaction,
    /// Any change that should snap for the first animation slice.
    Other,
}

/// Timing function for a scalar transition.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Easing {
    /// Linear interpolation.
    #[default]
    Linear,
}

impl Easing {
    fn sample(self, t: f32) -> f32 {
        match self {
            Self::Linear => t,
        }
    }
}

/// Transition policy for an implicit scalar presentation track.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TransitionSpec {
    duration: TimerDuration,
    easing: Easing,
}

impl TransitionSpec {
    /// Creates a linear transition over `duration` nanoseconds.
    #[must_use]
    pub(crate) const fn linear(duration: TimerDuration) -> Self {
        Self {
            duration,
            easing: Easing::Linear,
        }
    }
}

/// Key for one implicit presentation track.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PresentationTrackKey {
    element: ElementId,
    property: PresentationProperty,
}

impl PresentationTrackKey {
    /// Creates a presentation track key.
    #[must_use]
    pub(crate) const fn new(element: ElementId, property: PresentationProperty) -> Self {
        Self { element, property }
    }

    /// Returns the element this track targets.
    #[must_use]
    pub(crate) const fn element(self) -> ElementId {
        self.element
    }
}

/// Active clip carried by one presentation track.
#[derive(Clone, Copy, Debug, PartialEq)]
enum MotionClip {
    ScalarTransition(ScalarTransition),
}

impl MotionClip {
    fn sample_scalar(self, now: TimerInstant) -> f32 {
        match self {
            Self::ScalarTransition(transition) => transition.sample(now),
        }
    }

    fn is_complete(self, now: TimerInstant) -> bool {
        match self {
            Self::ScalarTransition(transition) => transition.is_complete(now),
        }
    }

    fn end_time(self) -> TimerInstant {
        match self {
            Self::ScalarTransition(transition) => transition.end_time(),
        }
    }
}

/// One scalar transition clip.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ScalarTransition {
    from: f32,
    to: f32,
    started_at: TimerInstant,
    duration: TimerDuration,
    easing: Easing,
}

impl ScalarTransition {
    fn new(from: f32, to: f32, started_at: TimerInstant, spec: TransitionSpec) -> Self {
        Self {
            from,
            to,
            started_at,
            duration: spec.duration,
            easing: spec.easing,
        }
    }

    fn sample(self, now: TimerInstant) -> f32 {
        if self.duration == 0 {
            return self.to;
        }
        let elapsed = now.saturating_sub(self.started_at);
        if elapsed >= self.duration {
            return self.to;
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "transition progress is a bounded f32 sample"
        )]
        let t = elapsed as f32 / self.duration as f32;
        self.from + ((self.to - self.from) * self.easing.sample(t))
    }

    fn is_complete(self, now: TimerInstant) -> bool {
        now.saturating_sub(self.started_at) >= self.duration
    }

    fn end_time(self) -> TimerInstant {
        self.started_at.saturating_add(self.duration)
    }
}

/// Active implicit presentation track.
#[derive(Clone, Copy, Debug, PartialEq)]
struct PresentationTrack {
    clip: MotionClip,
    affects: ChannelSet,
}

/// Internal animation runtime for sampled presentation values.
#[derive(Debug, Default)]
pub(crate) struct AnimationRuntime {
    targets: HashMap<PresentationTrackKey, f32>,
    tracks: HashMap<PresentationTrackKey, PresentationTrack>,
    pending: HashMap<PresentationTrackKey, TransitionSpec>,
}

impl AnimationRuntime {
    /// Creates an empty runtime.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Requests that the next target change for `key` animate when it is caused
    /// by interaction.
    pub(crate) fn request_scalar_transition(
        &mut self,
        key: PresentationTrackKey,
        spec: TransitionSpec,
    ) {
        self.pending.insert(key, spec);
    }

    /// Returns whether a transition spec is waiting for `key`'s next target.
    #[must_use]
    pub(crate) fn has_pending_transition(&self, key: PresentationTrackKey) -> bool {
        self.pending.contains_key(&key)
    }

    /// Records the resolved target value for `key`, planning or retargeting its
    /// presentation track when policy allows.
    pub(crate) fn set_scalar_target(
        &mut self,
        key: PresentationTrackKey,
        target: f32,
        now: TimerInstant,
        cause: ChangeCause,
        affects: ChannelSet,
    ) -> bool {
        let previous_target = self.targets.insert(key, target);
        let target_changed = previous_target.is_none_or(|previous| !same_scalar(previous, target));
        if !target_changed {
            if cause == ChangeCause::Interaction {
                self.pending.remove(&key);
            }
            return false;
        }

        let had_track = self.tracks.contains_key(&key);
        let spec = (cause == ChangeCause::Interaction)
            .then(|| self.pending.remove(&key))
            .flatten();
        let Some(spec) = spec else {
            self.tracks.remove(&key);
            return had_track;
        };

        let previous_target = previous_target.unwrap_or(target);
        let from = self.sample_scalar(key, previous_target, now);
        if same_scalar(from, target) || spec.duration == 0 {
            self.tracks.remove(&key);
            return had_track;
        }

        self.tracks.insert(
            key,
            PresentationTrack {
                clip: MotionClip::ScalarTransition(ScalarTransition::new(from, target, now, spec)),
                affects,
            },
        );
        true
    }

    /// Returns the sampled scalar presentation value for `key`.
    #[must_use]
    pub(crate) fn sample_scalar(
        &self,
        key: PresentationTrackKey,
        target: f32,
        now: TimerInstant,
    ) -> f32 {
        self.tracks
            .get(&key)
            .map_or(target, |track| track.clip.sample_scalar(now))
    }

    /// Advances active tracks, removing completed clips and returning affected
    /// elements/channels that should be invalidated for this sample.
    pub(crate) fn advance(&mut self, now: TimerInstant) -> Vec<(ElementId, ChannelSet)> {
        let mut affected = Vec::new();
        self.tracks.retain(|key, track| {
            affected.push((key.element(), track.affects));
            !track.clip.is_complete(now)
        });
        affected
    }

    /// Returns the next deadline a host should wake for active presentation
    /// tracks.
    #[must_use]
    pub(crate) fn next_deadline(&self, now: TimerInstant) -> Option<TimerInstant> {
        if self.tracks.is_empty() {
            return None;
        }
        let next_frame = now.saturating_add(PRESENTATION_FRAME_INTERVAL);
        let next_end = self
            .tracks
            .values()
            .map(|track| track.clip.end_time())
            .min();
        Some(next_end.map_or(next_frame, |end| next_frame.min(end).max(now)))
    }

    /// Removes all runtime state for `element`.
    pub(crate) fn remove_element(&mut self, element: ElementId) {
        self.targets.retain(|key, _| key.element() != element);
        self.tracks.retain(|key, _| key.element() != element);
        self.pending.retain(|key, _| key.element() != element);
    }

    /// Returns whether `key` currently has an active presentation track.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn has_active_track(&self, key: PresentationTrackKey) -> bool {
        self.tracks.contains_key(&key)
    }
}

fn same_scalar(a: f32, b: f32) -> bool {
    (a - b).abs() <= f32::EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::PAINT;

    const SPEC: TransitionSpec = TransitionSpec::linear(100);

    fn key() -> PresentationTrackKey {
        PresentationTrackKey::new(
            ElementId::new(7, 0),
            PresentationProperty::CompositionOffsetX,
        )
    }

    #[test]
    fn scalar_transition_samples_linearly() {
        let key = key();
        let mut runtime = AnimationRuntime::new();
        runtime.set_scalar_target(key, 0.0, 0, ChangeCause::Other, PAINT.into_set());
        runtime.request_scalar_transition(key, SPEC);
        runtime.set_scalar_target(key, 10.0, 10, ChangeCause::Interaction, PAINT.into_set());

        assert_eq!(runtime.sample_scalar(key, 10.0, 10), 0.0);
        assert_eq!(runtime.sample_scalar(key, 10.0, 60), 5.0);
        assert_eq!(runtime.sample_scalar(key, 10.0, 110), 10.0);
    }

    #[test]
    fn scalar_transition_retargets_from_sampled_value() {
        let key = key();
        let mut runtime = AnimationRuntime::new();
        runtime.set_scalar_target(key, 0.0, 0, ChangeCause::Other, PAINT.into_set());
        runtime.request_scalar_transition(key, SPEC);
        runtime.set_scalar_target(key, 10.0, 0, ChangeCause::Interaction, PAINT.into_set());
        runtime.request_scalar_transition(key, SPEC);
        runtime.set_scalar_target(key, 2.0, 50, ChangeCause::Interaction, PAINT.into_set());

        assert_eq!(runtime.sample_scalar(key, 2.0, 50), 5.0);
        assert_eq!(runtime.sample_scalar(key, 2.0, 100), 3.5);
        assert_eq!(runtime.sample_scalar(key, 2.0, 150), 2.0);
    }

    #[test]
    fn completed_transition_is_removed_by_advance() {
        let key = key();
        let mut runtime = AnimationRuntime::new();
        runtime.set_scalar_target(key, 0.0, 0, ChangeCause::Other, PAINT.into_set());
        runtime.request_scalar_transition(key, SPEC);
        runtime.set_scalar_target(key, 10.0, 0, ChangeCause::Interaction, PAINT.into_set());

        assert!(runtime.has_active_track(key));
        let affected = runtime.advance(100);

        assert_eq!(affected, alloc::vec![(key.element(), PAINT.into_set())]);
        assert!(!runtime.has_active_track(key));
        assert_eq!(runtime.sample_scalar(key, 10.0, 150), 10.0);
    }

    #[test]
    fn other_changes_snap_to_target() {
        let key = key();
        let mut runtime = AnimationRuntime::new();
        runtime.set_scalar_target(key, 0.0, 0, ChangeCause::Other, PAINT.into_set());
        runtime.request_scalar_transition(key, SPEC);
        runtime.set_scalar_target(key, 10.0, 0, ChangeCause::Other, PAINT.into_set());

        assert!(!runtime.has_active_track(key));
        assert_eq!(runtime.sample_scalar(key, 10.0, 50), 10.0);
    }

    #[test]
    fn next_deadline_tracks_frame_and_completion() {
        let key = key();
        let mut runtime = AnimationRuntime::new();
        let long_spec = TransitionSpec::linear(PRESENTATION_FRAME_INTERVAL * 4);
        runtime.set_scalar_target(key, 0.0, 0, ChangeCause::Other, PAINT.into_set());
        runtime.request_scalar_transition(key, long_spec);
        runtime.set_scalar_target(key, 10.0, 0, ChangeCause::Interaction, PAINT.into_set());

        assert_eq!(runtime.next_deadline(0), Some(PRESENTATION_FRAME_INTERVAL));
        assert_eq!(
            runtime.next_deadline(PRESENTATION_FRAME_INTERVAL * 4 - 10),
            Some(PRESENTATION_FRAME_INTERVAL * 4)
        );
    }

    #[test]
    fn remove_element_drops_tracks_targets_and_pending_specs() {
        let key = key();
        let mut runtime = AnimationRuntime::new();
        runtime.set_scalar_target(key, 0.0, 0, ChangeCause::Other, PAINT.into_set());
        runtime.request_scalar_transition(key, SPEC);
        runtime.set_scalar_target(key, 10.0, 0, ChangeCause::Interaction, PAINT.into_set());

        runtime.remove_element(key.element());

        assert!(!runtime.has_active_track(key));
        assert_eq!(runtime.next_deadline(0), None);
        assert_eq!(runtime.sample_scalar(key, 25.0, 50), 25.0);
    }
}
