// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::Any;

use understory_animation::{AnimationPlayState, TimingSample};
use understory_timing::{TimerDuration, TimerInstant};

use super::instance::{AnimationEvent, AnimationId};
use super::timeline::AnimationTimelineId;
use super::timeline::TimelineClock;
use super::types::{
    TimelineAnimationRuntimeRow, TimelineControlResult, TimelineSample, TimelineTrackKey,
};

/// Type-erased sample emitted by a native target stack.
pub(crate) struct NativeTimelineSample {
    key: TimelineTrackKey,
    value: Box<dyn Any>,
    complete: bool,
}

impl NativeTimelineSample {
    pub(super) fn new<T>(key: TimelineTrackKey, value: T, complete: bool) -> Self
    where
        T: Any,
    {
        Self {
            key,
            value: Box::new(value),
            complete,
        }
    }

    /// Returns the target key this sample targets.
    #[must_use]
    pub(crate) const fn key(&self) -> TimelineTrackKey {
        self.key
    }

    /// Returns the sampled value.
    #[must_use]
    pub(crate) fn value(&self) -> &dyn Any {
        self.value.as_ref()
    }

    /// Converts this sample into its concrete native value.
    pub(crate) fn into_value<T>(self) -> Option<T>
    where
        T: Any,
    {
        self.value.downcast::<T>().ok().map(|value| *value)
    }

    /// Returns whether this is the final sample for the track.
    #[must_use]
    pub(crate) const fn is_complete(&self) -> bool {
        self.complete
    }
}

/// Result from controlling a native retained animation instance.
pub(crate) struct NativeTimelineControlResult {
    key: TimelineTrackKey,
    value: Option<Box<dyn Any>>,
    complete: bool,
}

impl NativeTimelineControlResult {
    pub(super) fn new<T>(key: TimelineTrackKey, value: Option<T>, complete: bool) -> Self
    where
        T: Any,
    {
        Self {
            key,
            value: value.map(|value| Box::new(value) as Box<dyn Any>),
            complete,
        }
    }

    /// Returns the track key this control operation affected.
    #[must_use]
    pub(crate) const fn key(&self) -> TimelineTrackKey {
        self.key
    }

    /// Returns whether the affected track has no remaining active motion.
    #[must_use]
    pub(crate) const fn is_complete(&self) -> bool {
        self.complete
    }

    /// Converts this result into a native timeline sample, when it has a value.
    #[must_use]
    pub(crate) fn sample(self) -> Option<NativeTimelineSample> {
        self.value.map(|value| NativeTimelineSample {
            key: self.key,
            value,
            complete: self.complete,
        })
    }
}

/// Sample emitted by a retained animation target state.
pub(crate) enum TimelineTargetSample {
    /// Sample for a built-in presentation value target.
    Presentation(TimelineSample),
    /// Sample for a native target.
    Native(NativeTimelineSample),
}

impl TimelineTargetSample {
    /// Returns this sample as a built-in presentation sample, when applicable.
    pub(crate) fn into_presentation(self) -> Option<TimelineSample> {
        match self {
            Self::Presentation(sample) => Some(sample),
            Self::Native(_) => None,
        }
    }

    /// Returns this sample as a native target sample, when applicable.
    pub(crate) fn into_native<T>(self) -> Option<T>
    where
        T: Any,
    {
        match self {
            Self::Presentation(_) => None,
            Self::Native(sample) => sample.into_value(),
        }
    }
}

/// Result from controlling an animation on a retained target state.
pub(crate) enum TimelineTargetControlResult {
    /// Result for a built-in presentation value target.
    Presentation(TimelineControlResult),
    /// Result for a native target.
    Native(NativeTimelineControlResult),
}

/// Type-erased retained state for one animation target.
///
/// The runtime owns scheduling and lifecycle orchestration through this trait.
/// Concrete target states own value storage and sampling details, including
/// Overstory's built-in color interpolation policy.
pub(super) trait ErasedTargetState {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn is_empty(&self) -> bool;
    fn has_target(&self) -> bool;
    fn has_active_motion(&self) -> bool;
    fn has_running_motion_on(&self, timeline: AnimationTimelineId) -> bool;
    fn running_transition_count(&self) -> usize;
    fn running_effect_count(&self) -> usize;
    fn animation_state(&self, id: AnimationId) -> Option<AnimationPlayState>;
    fn animation_current_time(
        &self,
        id: AnimationId,
        clock: TimelineClock<'_>,
    ) -> Option<TimerDuration>;
    fn animation_timing_sample(
        &self,
        id: AnimationId,
        clock: TimelineClock<'_>,
    ) -> Option<TimingSample>;
    fn animation_playback_rate(&self, id: AnimationId) -> Option<f64>;
    fn animation_rows(
        &self,
        key: TimelineTrackKey,
        clock: TimelineClock<'_>,
        rows: &mut Vec<TimelineAnimationRuntimeRow>,
    );
    fn advance(
        &mut self,
        key: TimelineTrackKey,
        clock: TimelineClock<'_>,
        event_at: TimerInstant,
        events: &mut Vec<AnimationEvent>,
    ) -> Option<TimelineTargetSample>;
    fn active_sample(
        &self,
        key: TimelineTrackKey,
        clock: TimelineClock<'_>,
    ) -> Option<TimelineTargetSample>;
    fn pause(
        &mut self,
        id: AnimationId,
        key: TimelineTrackKey,
        clock: TimelineClock<'_>,
    ) -> Option<TimelineTargetControlResult>;
    fn play(
        &mut self,
        id: AnimationId,
        key: TimelineTrackKey,
        clock: TimelineClock<'_>,
    ) -> Option<TimelineTargetControlResult>;
    fn seek(
        &mut self,
        id: AnimationId,
        key: TimelineTrackKey,
        local_time: TimerDuration,
        clock: TimelineClock<'_>,
    ) -> Option<TimelineTargetControlResult>;
    fn reverse(
        &mut self,
        id: AnimationId,
        key: TimelineTrackKey,
        clock: TimelineClock<'_>,
    ) -> Option<TimelineTargetControlResult>;
    fn set_playback_rate(
        &mut self,
        id: AnimationId,
        key: TimelineTrackKey,
        playback_rate: f64,
        clock: TimelineClock<'_>,
    ) -> Option<TimelineTargetControlResult>;
    fn cancel_by_id(
        &mut self,
        id: AnimationId,
        key: TimelineTrackKey,
        now: TimerInstant,
        clock: TimelineClock<'_>,
        events: &mut Vec<AnimationEvent>,
    ) -> Option<TimelineTargetControlResult>;
    fn finish_by_id(
        &mut self,
        id: AnimationId,
        key: TimelineTrackKey,
        now: TimerInstant,
        clock: TimelineClock<'_>,
        events: &mut Vec<AnimationEvent>,
    ) -> Option<TimelineTargetControlResult>;
    fn cancel_all(
        &mut self,
        key: TimelineTrackKey,
        now: TimerInstant,
        events: &mut Vec<AnimationEvent>,
    );
}
