// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::Vec2;
use peniko::Color;
use understory_animation::{
    CompositeOperation, Keyframe, KeyframeEffect, RetainedAnimationEffect, StackEffect,
    TimingPhase, TimingSample, sample_effects,
};
use understory_animation_timeline::TimelineTime;
use understory_timing::TimerDuration;

use crate::motion::{Animatable, EffectClip, PresentationValue, PresentationValueKind};

use super::types::PresentationRuntimeValue;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum PresentationEffect {
    Scalar32(StackEffect<f32>),
    Scalar64(StackEffect<f64>),
    Color(StackEffect<Color>),
    Offset(StackEffect<Vec2>),
}

impl PresentationEffect {
    pub(super) fn effect(clip: EffectClip) -> Self {
        match clip.value_kind() {
            PresentationValueKind::Scalar32 => Self::Scalar32(stack_effect_clip::<f32>(&clip)),
            PresentationValueKind::Scalar64 => Self::Scalar64(stack_effect_clip::<f64>(&clip)),
            PresentationValueKind::Color => Self::Color(stack_effect_clip::<Color>(&clip)),
            PresentationValueKind::Offset2d => Self::Offset(stack_effect_clip::<Vec2>(&clip)),
        }
    }

    pub(super) fn sample(&self, time: TimelineTime) -> Option<PresentationValue> {
        match self {
            Self::Scalar32(effect) if effect.effect.composite() == CompositeOperation::Replace => {
                effect.sample(time).map(PresentationValue::Scalar32)
            }
            Self::Scalar64(effect) if effect.effect.composite() == CompositeOperation::Replace => {
                effect.sample(time).map(PresentationValue::Scalar64)
            }
            Self::Color(effect) if effect.effect.composite() == CompositeOperation::Replace => {
                effect.sample(time).map(PresentationValue::Color)
            }
            Self::Offset(effect) if effect.effect.composite() == CompositeOperation::Replace => {
                effect.sample(time).map(PresentationValue::Offset2d)
            }
            Self::Scalar32(_) | Self::Scalar64(_) | Self::Color(_) | Self::Offset(_) => None,
        }
    }

    pub(super) fn sample_over(
        &self,
        underlying: Option<PresentationValue>,
        time: TimelineTime,
    ) -> Option<PresentationValue> {
        match underlying {
            Some(PresentationValue::Scalar32(value)) => Some(PresentationValue::Scalar32(
                sample_native_stack(value, self.scalar32_effect(time)),
            )),
            Some(PresentationValue::Scalar64(value)) => Some(PresentationValue::Scalar64(
                sample_native_stack(value, self.scalar64_effect(time)),
            )),
            Some(PresentationValue::Color(value)) => Some(PresentationValue::Color(
                sample_native_stack(value, self.color_effect(time)),
            )),
            Some(PresentationValue::Offset2d(value)) => Some(PresentationValue::Offset2d(
                sample_native_stack(value, self.offset_effect(time)),
            )),
            None => self.sample(time),
        }
    }

    fn total_duration(&self) -> Option<TimerDuration> {
        match self {
            Self::Scalar32(effect) => effect.timing.total_duration(),
            Self::Scalar64(effect) => effect.timing.total_duration(),
            Self::Color(effect) => effect.timing.total_duration(),
            Self::Offset(effect) => effect.timing.total_duration(),
        }
    }

    fn iteration_at(&self, time: TimelineTime) -> Option<u64> {
        match self {
            Self::Scalar32(effect) => effect.timing.sample(time).iteration,
            Self::Scalar64(effect) => effect.timing.sample(time).iteration,
            Self::Color(effect) => effect.timing.sample(time).iteration,
            Self::Offset(effect) => effect.timing.sample(time).iteration,
        }
    }

    fn timing_sample_at(&self, time: TimelineTime) -> Option<TimingSample> {
        Some(match self {
            Self::Scalar32(effect) => stack_timing_sample(effect, time),
            Self::Scalar64(effect) => stack_timing_sample(effect, time),
            Self::Color(effect) => stack_timing_sample(effect, time),
            Self::Offset(effect) => stack_timing_sample(effect, time),
        })
    }

    pub(super) const fn value_kind(&self) -> PresentationValueKind {
        match self {
            Self::Scalar32(_) => PresentationValueKind::Scalar32,
            Self::Scalar64(_) => PresentationValueKind::Scalar64,
            Self::Color(_) => PresentationValueKind::Color,
            Self::Offset(_) => PresentationValueKind::Offset2d,
        }
    }

    pub(super) const fn composite(&self) -> CompositeOperation {
        match self {
            Self::Scalar32(effect) => effect.effect.composite(),
            Self::Scalar64(effect) => effect.effect.composite(),
            Self::Color(effect) => effect.effect.composite(),
            Self::Offset(effect) => effect.effect.composite(),
        }
    }

    const fn as_scalar32(&self) -> Option<&StackEffect<f32>> {
        match self {
            Self::Scalar32(effect) => Some(effect),
            Self::Scalar64(_) | Self::Color(_) | Self::Offset(_) => None,
        }
    }

    const fn as_scalar64(&self) -> Option<&StackEffect<f64>> {
        match self {
            Self::Scalar64(effect) => Some(effect),
            Self::Scalar32(_) | Self::Color(_) | Self::Offset(_) => None,
        }
    }

    const fn as_color(&self) -> Option<&StackEffect<Color>> {
        match self {
            Self::Color(effect) => Some(effect),
            Self::Scalar32(_) | Self::Scalar64(_) | Self::Offset(_) => None,
        }
    }

    const fn as_offset(&self) -> Option<&StackEffect<Vec2>> {
        match self {
            Self::Offset(effect) => Some(effect),
            Self::Scalar32(_) | Self::Scalar64(_) | Self::Color(_) => None,
        }
    }

    fn scalar32_effect(&self, time: TimelineTime) -> Option<LocalStackEffect<'_, f32>> {
        self.local_stack_effect(self.as_scalar32()?, time)
    }

    fn scalar64_effect(&self, time: TimelineTime) -> Option<LocalStackEffect<'_, f64>> {
        self.local_stack_effect(self.as_scalar64()?, time)
    }

    fn color_effect(&self, time: TimelineTime) -> Option<LocalStackEffect<'_, Color>> {
        self.local_stack_effect(self.as_color()?, time)
    }

    fn offset_effect(&self, time: TimelineTime) -> Option<LocalStackEffect<'_, Vec2>> {
        self.local_stack_effect(self.as_offset()?, time)
    }

    const fn local_stack_effect<'a, T>(
        &self,
        effect: &'a StackEffect<T>,
        time: TimelineTime,
    ) -> Option<LocalStackEffect<'a, T>> {
        Some(LocalStackEffect { effect, time })
    }
}

impl RetainedAnimationEffect for PresentationEffect {
    fn timing_sample_at(&self, local_time: TimelineTime) -> Option<TimingSample> {
        Self::timing_sample_at(self, local_time)
    }

    fn iteration_at(&self, local_time: TimelineTime) -> Option<u64> {
        Self::iteration_at(self, local_time)
    }

    fn total_duration(&self) -> Option<TimerDuration> {
        Self::total_duration(self)
    }
}

fn stack_timing_sample<T>(effect: &StackEffect<T>, time: TimelineTime) -> TimingSample {
    if time < effect.starts_at {
        return empty_timing_sample(TimingPhase::Before);
    }
    let local = TimelineTime::from_duration(time.saturating_duration_since(effect.starts_at));
    effect.timing.sample(local)
}

const fn empty_timing_sample(phase: TimingPhase) -> TimingSample {
    TimingSample {
        phase,
        iteration: None,
        progress: None,
        eased_progress: None,
    }
}

fn stack_effect_clip<T>(clip: &EffectClip) -> StackEffect<T>
where
    T: PresentationRuntimeValue + Animatable,
{
    let duration = clip.keyframe_duration();
    let keyframes = clip
        .keyframes()
        .iter()
        .map(|keyframe| {
            let offset = normalized_keyframe_offset(keyframe.offset(), duration);
            let value = T::from_presentation_value(*keyframe.value())
                .expect("effect clip value kind should match requested stack type");
            Keyframe::new(offset, value)
        })
        .collect();
    StackEffect::new(
        KeyframeEffect::new(keyframes).with_composite(clip.composite()),
        clip.timing(),
    )
}

#[derive(Clone, Copy)]
struct LocalStackEffect<'a, T> {
    effect: &'a StackEffect<T>,
    time: TimelineTime,
}

fn sample_native_stack<'a, T>(
    underlying: T,
    effects: impl IntoIterator<Item = LocalStackEffect<'a, T>>,
) -> T
where
    T: Animatable + 'a,
{
    effects.into_iter().fold(underlying, |value, effect| {
        sample_effects(&value, core::iter::once(effect.effect), effect.time).value
    })
}

fn normalized_keyframe_offset(offset: TimerDuration, duration: TimerDuration) -> f64 {
    if duration == 0 {
        0.0
    } else {
        offset as f64 / duration as f64
    }
}
