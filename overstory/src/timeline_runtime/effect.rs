// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::Vec2;
use peniko::Color;
use understory_animation::{
    AnimationTiming, CompositeOperation, FillMode, Keyframe, KeyframeEffect,
    RetainedAnimationEffect, StackEffect, TimingPhase, TimingSample, sample_effects,
};
use understory_animation_timeline::TimelineTime;
use understory_timing::TimerDuration;

use crate::motion::{
    Animatable, ColorTransition, EffectClip, PresentationValue, PresentationValueKind,
    ResolvedTransitionSpec, normalized_progress,
};

use super::types::PresentationRuntimeValue;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum PresentationEffect {
    Scalar32(StackEffect<f32>),
    Scalar64(StackEffect<f64>),
    Color(ColorStackEffect),
    Offset(StackEffect<Vec2>),
}

impl PresentationEffect {
    pub(super) fn transition(
        from: PresentationValue,
        to: PresentationValue,
        spec: ResolvedTransitionSpec,
    ) -> Self {
        match (from, to) {
            (PresentationValue::Scalar32(from), PresentationValue::Scalar32(to)) => {
                Self::Scalar32(stack_transition(from, to, spec))
            }
            (PresentationValue::Scalar64(from), PresentationValue::Scalar64(to)) => {
                Self::Scalar64(stack_transition(from, to, spec))
            }
            (PresentationValue::Color(from), PresentationValue::Color(to)) => {
                Self::Color(ColorStackEffect::Transition(ColorTransition::new(
                    from,
                    to,
                    0,
                    spec.duration(),
                    spec.timing(),
                    spec.color_interpolation(),
                )))
            }
            (PresentationValue::Offset2d(from), PresentationValue::Offset2d(to)) => {
                Self::Offset(stack_transition(from, to, spec))
            }
            _ => unreachable!("timeline target kinds should match property metadata"),
        }
    }

    pub(super) fn effect(clip: EffectClip) -> Self {
        match clip.value_kind() {
            PresentationValueKind::Scalar32 => Self::Scalar32(stack_effect_clip::<f32>(&clip)),
            PresentationValueKind::Scalar64 => Self::Scalar64(stack_effect_clip::<f64>(&clip)),
            PresentationValueKind::Color => {
                Self::Color(ColorStackEffect::Effect(stack_effect_clip::<Color>(&clip)))
            }
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
            Self::Color(effect) => effect.sample_standalone(time).map(PresentationValue::Color),
            Self::Offset(effect) if effect.effect.composite() == CompositeOperation::Replace => {
                effect.sample(time).map(PresentationValue::Offset2d)
            }
            Self::Scalar32(_) | Self::Scalar64(_) | Self::Offset(_) => None,
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
                sample_color_stack(Some(value), self.color_effect(time))?,
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
            Self::Color(effect) => effect.total_duration(),
            Self::Offset(effect) => effect.timing.total_duration(),
        }
    }

    fn iteration_at(&self, time: TimelineTime) -> Option<u64> {
        match self {
            Self::Scalar32(effect) => effect.timing.sample(time).iteration,
            Self::Scalar64(effect) => effect.timing.sample(time).iteration,
            Self::Color(effect) => effect.iteration_at(time),
            Self::Offset(effect) => effect.timing.sample(time).iteration,
        }
    }

    fn timing_sample_at(&self, time: TimelineTime) -> Option<TimingSample> {
        Some(match self {
            Self::Scalar32(effect) => stack_timing_sample(effect, time),
            Self::Scalar64(effect) => stack_timing_sample(effect, time),
            Self::Color(effect) => effect.timing_sample_at(time),
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
            Self::Color(effect) => effect.composite(),
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

    const fn as_color(&self) -> Option<&ColorStackEffect> {
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

    fn color_effect(&self, time: TimelineTime) -> Option<LocalColorEffect<'_>> {
        Some(match self.as_color()? {
            ColorStackEffect::Transition(transition) => LocalColorEffect::Transition {
                transition,
                elapsed: time.duration(),
            },
            ColorStackEffect::Effect(effect) => {
                LocalColorEffect::Effect(LocalStackEffect { effect, time })
            }
        })
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

#[derive(Clone, Debug, PartialEq)]
pub(super) enum ColorStackEffect {
    Transition(ColorTransition),
    Effect(StackEffect<Color>),
}

impl ColorStackEffect {
    fn sample_standalone(&self, time: TimelineTime) -> Option<Color> {
        match self {
            Self::Transition(transition) => Some(transition.sample(time.duration())),
            Self::Effect(effect) if effect.effect.composite() == CompositeOperation::Replace => {
                effect.sample(time)
            }
            Self::Effect(_) => None,
        }
    }

    fn total_duration(&self) -> Option<TimerDuration> {
        match self {
            Self::Transition(transition) => Some(transition.duration()),
            Self::Effect(effect) => effect.timing.total_duration(),
        }
    }

    fn iteration_at(&self, time: TimelineTime) -> Option<u64> {
        match self {
            Self::Transition(_) => Some(0),
            Self::Effect(effect) => effect.timing.sample(time).iteration,
        }
    }

    fn timing_sample_at(&self, time: TimelineTime) -> TimingSample {
        match self {
            Self::Transition(transition) => color_transition_timing_sample(*transition, time),
            Self::Effect(effect) => stack_timing_sample(effect, time),
        }
    }

    const fn composite(&self) -> CompositeOperation {
        match self {
            Self::Transition(_) => CompositeOperation::Replace,
            Self::Effect(effect) => effect.effect.composite(),
        }
    }
}

fn stack_timing_sample<T>(effect: &StackEffect<T>, time: TimelineTime) -> TimingSample {
    if time < effect.starts_at {
        return empty_timing_sample(TimingPhase::Before);
    }
    let local = TimelineTime::from_duration(time.saturating_duration_since(effect.starts_at));
    effect.timing.sample(local)
}

fn color_transition_timing_sample(transition: ColorTransition, time: TimelineTime) -> TimingSample {
    let local = time.duration();
    if local < transition.started_at() {
        return empty_timing_sample(TimingPhase::Before);
    }

    let progress = normalized_progress(local, transition.started_at(), transition.duration());
    let phase = if transition.duration() == 0
        || local.saturating_sub(transition.started_at()) >= transition.duration()
    {
        TimingPhase::After
    } else {
        TimingPhase::Active
    };

    TimingSample {
        phase,
        iteration: Some(0),
        progress: Some(progress),
        eased_progress: Some(transition.timing().sample(progress)),
    }
}

const fn empty_timing_sample(phase: TimingPhase) -> TimingSample {
    TimingSample {
        phase,
        iteration: None,
        progress: None,
        eased_progress: None,
    }
}

fn stack_transition<T>(from: T, to: T, spec: ResolvedTransitionSpec) -> StackEffect<T>
where
    T: Animatable + Clone,
{
    StackEffect::new(
        KeyframeEffect::from_values(alloc::vec![from, to]),
        AnimationTiming {
            duration: spec.duration(),
            fill: FillMode::Forwards,
            easing: spec.timing(),
            ..AnimationTiming::new(spec.duration())
        },
    )
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

#[derive(Clone, Copy)]
enum LocalColorEffect<'a> {
    Transition {
        transition: &'a ColorTransition,
        elapsed: TimerDuration,
    },
    Effect(LocalStackEffect<'a, Color>),
}

impl LocalColorEffect<'_> {
    fn sample_over(self, underlying: Option<Color>) -> Option<Color> {
        match self {
            Self::Transition {
                transition,
                elapsed,
            } => Some(transition.sample(elapsed)),
            Self::Effect(effect) => match underlying {
                Some(underlying) => Some(
                    sample_effects(&underlying, core::iter::once(effect.effect), effect.time).value,
                ),
                None if effect.effect.effect.composite() == CompositeOperation::Replace => {
                    effect.effect.sample(effect.time)
                }
                None => None,
            },
        }
    }
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

fn sample_color_stack<'a>(
    underlying: Option<Color>,
    effects: impl IntoIterator<Item = LocalColorEffect<'a>>,
) -> Option<Color> {
    effects
        .into_iter()
        .fold(underlying, |value, effect| effect.sample_over(value))
}

fn normalized_keyframe_offset(offset: TimerDuration, duration: TimerDuration) -> f64 {
    if duration == 0 {
        0.0
    } else {
        offset as f64 / duration as f64
    }
}
