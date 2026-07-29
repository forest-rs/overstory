// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::any::{Any, TypeId};
use core::marker::PhantomData;

use peniko::Color;
use understory_animation::{
    AnimationPlayState, CompositeOperation, RetainedAnimationEffect, RetainedAnimationInstance,
    StackEffect, TimingSample, sample_effects,
};
use understory_animation_timeline::TimelineTime;
use understory_motion::{ColorInterpolation, TimingFunction};
use understory_property::ErasedValue;
use understory_timing::{TimerDuration, TimerInstant};
use understory_transition::{
    NoTransitionResources, TransitionDeclaration, TransitionDeclarations, TransitionRunner,
    TransitionTiming,
};

use crate::motion::{Animatable, PresentationTarget, PresentationValue, PresentationValueKind};

use super::effect::PresentationEffect;
use super::instance::{AnimationEvent, AnimationEventKind, AnimationId};
use super::target::{
    ErasedTargetState, NativeTimelineControlResult, NativeTimelineSample,
    TimelineTargetControlResult, TimelineTargetSample,
};
use super::timeline::{AnimationTimelineId, TimelineClock};
use super::types::{
    TimelineAnimationRuntimeRow, TimelineControlResult, TimelineSample, TimelineTrackKey,
    default_underlying_value, same_value,
};

/// Transition parameters after late-bound style resources are resolved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ResolvedTransition {
    duration: TimerDuration,
    delay: TimerDuration,
    timing: TimingFunction,
    color_interpolation: ColorInterpolation,
}

impl ResolvedTransition {
    pub(crate) const fn new(
        duration: TimerDuration,
        delay: TimerDuration,
        timing: TimingFunction,
        color_interpolation: ColorInterpolation,
    ) -> Self {
        Self {
            duration,
            delay,
            timing,
            color_interpolation,
        }
    }

    #[cfg(test)]
    pub(crate) const fn linear(duration: TimerDuration) -> Self {
        Self::new(
            duration,
            0,
            TimingFunction::LINEAR,
            ColorInterpolation::SRGB,
        )
    }
}

/// Effect adapter used by the retained target runtime.
///
/// The runtime owns playback and target-stack lifecycle. The effect adapter owns
/// value sampling and composition for one concrete value family.
pub(super) trait RuntimeEffect<T>: RetainedAnimationEffect + Clone + 'static {
    /// Samples this effect as a standalone replacement value.
    fn sample_value(&self, local_time: TimelineTime) -> Option<T>;

    /// Samples this effect over `underlying`.
    fn sample_over(&self, underlying: Option<T>, local_time: TimelineTime) -> Option<T>;

    /// Returns the stack composition operation.
    fn composite(&self) -> CompositeOperation;

    /// Returns presentation metadata for inspection rows, when available.
    fn value_kind(&self) -> Option<PresentationValueKind> {
        None
    }
}

/// Adapts a concrete target state into the erased runtime sample/result enums.
pub(super) trait TargetValueBridge<T>: Default + 'static {
    /// Returns whether two target values are equivalent for retargeting.
    fn values_close(a: &T, b: &T) -> bool;

    /// Captures the concrete value type and interpolation policy for a target.
    fn transition_declarations(
        target: &T,
        spec: Option<ResolvedTransition>,
    ) -> TransitionDeclarations<u8>;

    /// Erases a target value for `understory_transition`.
    fn erase_transition_value(value: T) -> ErasedValue;

    /// Restores a target value sampled by `understory_transition`.
    fn transition_value(value: ErasedValue) -> Option<T>;

    /// Returns the built-in underlying value for a target, if it has one.
    fn default_underlying(_target: PresentationTarget) -> Option<T> {
        None
    }

    /// Wraps a sampled value for runtime delivery.
    fn sample(key: TimelineTrackKey, value: T, complete: bool) -> TimelineTargetSample;

    /// Wraps a control result for runtime delivery.
    fn control_result(
        key: TimelineTrackKey,
        value: Option<T>,
        complete: bool,
    ) -> TimelineTargetControlResult;
}

/// Bridge for built-in presentation targets.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct PresentationBridge;

impl TargetValueBridge<PresentationValue> for PresentationBridge {
    fn values_close(a: &PresentationValue, b: &PresentationValue) -> bool {
        same_value(*a, *b)
    }

    fn transition_declarations(
        target: &PresentationValue,
        spec: Option<ResolvedTransition>,
    ) -> TransitionDeclarations<u8> {
        let Some(spec) = spec else {
            return TransitionDeclarations::new();
        };
        let timing = TransitionTiming::new(spec.duration)
            .with_delay(spec.delay)
            .with_timing(spec.timing);
        let declaration = match target {
            PresentationValue::Scalar32(_) => TransitionDeclaration::for_target::<f32>(0, timing),
            PresentationValue::Scalar64(_) => TransitionDeclaration::for_target::<f64>(0, timing),
            PresentationValue::Color(_) => TransitionDeclaration::for_color_target(0, timing)
                .with_color_interpolation(spec.color_interpolation),
            PresentationValue::Offset2d(_) => {
                TransitionDeclaration::for_target::<kurbo::Vec2>(0, timing)
            }
        };
        TransitionDeclarations::new().with(declaration)
    }

    fn erase_transition_value(value: PresentationValue) -> ErasedValue {
        match value {
            PresentationValue::Scalar32(value) => ErasedValue::new(value),
            PresentationValue::Scalar64(value) => ErasedValue::new(value),
            PresentationValue::Color(value) => ErasedValue::new(value),
            PresentationValue::Offset2d(value) => ErasedValue::new(value),
        }
    }

    fn transition_value(value: ErasedValue) -> Option<PresentationValue> {
        if let Some(value) = value.downcast_ref::<f32>() {
            return Some(PresentationValue::Scalar32(*value));
        }
        if let Some(value) = value.downcast_ref::<f64>() {
            return Some(PresentationValue::Scalar64(*value));
        }
        if let Some(value) = value.downcast_ref::<Color>() {
            return Some(PresentationValue::Color(*value));
        }
        value
            .downcast_ref::<kurbo::Vec2>()
            .copied()
            .map(PresentationValue::Offset2d)
    }

    fn default_underlying(target: PresentationTarget) -> Option<PresentationValue> {
        default_underlying_value(target)
    }

    fn sample(
        key: TimelineTrackKey,
        value: PresentationValue,
        complete: bool,
    ) -> TimelineTargetSample {
        TimelineTargetSample::Presentation(TimelineSample::new(key, value, complete))
    }

    fn control_result(
        key: TimelineTrackKey,
        value: Option<PresentationValue>,
        complete: bool,
    ) -> TimelineTargetControlResult {
        TimelineTargetControlResult::Presentation(TimelineControlResult::new(key, value, complete))
    }
}

/// Bridge for app-defined native animation targets.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct NativeBridge;

impl<T> TargetValueBridge<T> for NativeBridge
where
    T: Animatable + Any,
{
    fn values_close(a: &T, b: &T) -> bool {
        a.is_close(b)
    }

    fn transition_declarations(
        _target: &T,
        spec: Option<ResolvedTransition>,
    ) -> TransitionDeclarations<u8> {
        let Some(spec) = spec else {
            return TransitionDeclarations::new();
        };
        let timing = TransitionTiming::new(spec.duration)
            .with_delay(spec.delay)
            .with_timing(spec.timing);
        let declaration = if TypeId::of::<T>() == TypeId::of::<Color>() {
            TransitionDeclaration::for_color_target(0, timing)
                .with_color_interpolation(spec.color_interpolation)
        } else {
            TransitionDeclaration::for_target::<T>(0, timing)
        };
        TransitionDeclarations::new().with(declaration)
    }

    fn erase_transition_value(value: T) -> ErasedValue {
        ErasedValue::new(value)
    }

    fn transition_value(value: ErasedValue) -> Option<T> {
        value.downcast_ref::<T>().cloned()
    }

    fn sample(key: TimelineTrackKey, value: T, complete: bool) -> TimelineTargetSample {
        TimelineTargetSample::Native(NativeTimelineSample::new(key, value, complete))
    }

    fn control_result(
        key: TimelineTrackKey,
        value: Option<T>,
        complete: bool,
    ) -> TimelineTargetControlResult {
        TimelineTargetControlResult::Native(NativeTimelineControlResult::new(key, value, complete))
    }
}

/// Retained runtime state for one animation target.
#[derive(Debug)]
pub(super) struct ValueTargetState<T, E, B> {
    target: Option<T>,
    transition: TransitionRunner<u8, u8>,
    layers: Vec<RetainedValueAnimation<E>>,
    value_kind: Option<PresentationValueKind>,
    _bridge: PhantomData<B>,
}

impl<T, E, B> Default for ValueTargetState<T, E, B> {
    fn default() -> Self {
        Self {
            target: None,
            transition: TransitionRunner::new(),
            layers: Vec::new(),
            value_kind: None,
            _bridge: PhantomData,
        }
    }
}

impl<T, E, B> ValueTargetState<T, E, B>
where
    T: Clone + 'static,
    E: RuntimeEffect<T>,
    B: TargetValueBridge<T>,
{
    /// Records inspection metadata for values with a presentation kind.
    pub(super) const fn set_value_kind(&mut self, value_kind: Option<PresentationValueKind>) {
        self.value_kind = value_kind;
    }

    /// Returns whether this target has no retained target or active layers.
    pub(super) fn is_empty(&self) -> bool {
        self.target.is_none() && self.motion_is_empty()
    }

    /// Returns the sampled value over `fallback`.
    pub(super) fn sample(&self, fallback: Option<T>, clock: TimelineClock<'_>) -> Option<T> {
        let underlying = self
            .sample_transition(clock)
            .or(fallback)
            .or_else(|| self.target.clone());
        self.sample_stack(underlying.clone(), clock).or(underlying)
    }

    fn retained_underlying(&self, target: PresentationTarget) -> Option<T> {
        self.target
            .clone()
            .or_else(|| B::default_underlying(target))
    }

    /// Appends an explicit effect layer.
    pub(super) fn set_effect(
        &mut self,
        key: TimelineTrackKey,
        instance: RetainedValueAnimation<E>,
        event_at: TimerInstant,
        events: &mut Vec<AnimationEvent>,
    ) {
        events.push(AnimationEvent::new(
            *instance.id(),
            key,
            AnimationEventKind::Started,
            event_at,
        ));
        self.layers.push(instance);
    }

    /// Records a new target value and updates the implicit transition layer.
    pub(super) fn set_target(
        &mut self,
        target: T,
        spec: Option<ResolvedTransition>,
        event_at: TimerInstant,
    ) -> bool {
        let previous_target = self.target.clone();
        let target_changed = previous_target
            .as_ref()
            .is_none_or(|previous| !B::values_close(previous, &target));
        if !target_changed {
            return false;
        }
        let before = previous_target.clone().unwrap_or_else(|| target.clone());
        let declarations = B::transition_declarations(&target, spec);
        let had_transition = !self.transition.is_idle();
        let update = self
            .transition
            .set_target(
                0,
                0,
                &declarations,
                &NoTransitionResources,
                &B::erase_transition_value(before),
                B::erase_transition_value(target.clone()),
                event_at,
            )
            .expect("transition declaration type matches its target adapter");
        let (_, immediate) = update.into_parts();
        self.target = Some(target);
        had_transition || !self.transition.is_idle() || immediate.is_some()
    }

    fn motion_is_empty(&self) -> bool {
        self.transition.is_idle() && self.layers.is_empty()
    }

    fn sample_stack(&self, underlying: Option<T>, clock: TimelineClock<'_>) -> Option<T> {
        self.layers.iter().fold(underlying, |value, instance| {
            sample_instance(instance, value, timeline_time(instance, clock))
        })
    }

    fn sample_transition(&self, clock: TimelineClock<'_>) -> Option<T> {
        self.transition
            .current_value(0, 0, clock.document_instant())
            .and_then(B::transition_value)
    }

    fn animation(&self, id: AnimationId) -> Option<&RetainedValueAnimation<E>> {
        self.layers.iter().find(|instance| *instance.id() == id)
    }

    fn animation_mut(&mut self, id: AnimationId) -> Option<&mut RetainedValueAnimation<E>> {
        self.layers.iter_mut().find(|instance| *instance.id() == id)
    }

    fn control_result(
        &self,
        key: TimelineTrackKey,
        clock: TimelineClock<'_>,
    ) -> TimelineTargetControlResult {
        B::control_result(
            key,
            self.sample(self.retained_underlying(key.target()), clock),
            self.motion_is_empty(),
        )
    }

    fn remove_completed(
        &mut self,
        key: TimelineTrackKey,
        clock: TimelineClock<'_>,
        event_at: TimerInstant,
        events: &mut Vec<AnimationEvent>,
    ) {
        let mut retained = Vec::new();
        for mut instance in self.layers.drain(..) {
            let timeline_time = timeline_time(&instance, clock);
            queue_iteration_events(&mut instance, key, timeline_time, event_at, events);
            // Manual timelines are scrub sources: reaching the effect endpoint
            // should sample the endpoint, not retire the retained instance.
            if *instance.timeline() == AnimationTimelineId::DOCUMENT
                && timeline_time
                    .is_some_and(|timeline_time| instance.finish_if_complete(timeline_time))
            {
                events.push(AnimationEvent::new(
                    *instance.id(),
                    key,
                    AnimationEventKind::Finished,
                    event_at,
                ));
            } else {
                retained.push(instance);
            }
        }
        self.layers = retained;
    }

    fn remove_by_id(
        &mut self,
        id: AnimationId,
        key: TimelineTrackKey,
        now: TimerInstant,
        kind: AnimationEventKind,
        events: &mut Vec<AnimationEvent>,
    ) -> bool {
        let Some(index) = self.layers.iter().position(|instance| *instance.id() == id) else {
            return false;
        };
        let instance = self.layers.remove(index);
        events.push(AnimationEvent::new(*instance.id(), key, kind, now));
        true
    }
}

impl<T, E, B> ErasedTargetState for ValueTargetState<T, E, B>
where
    T: Clone + 'static,
    E: RuntimeEffect<T>,
    B: TargetValueBridge<T>,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn is_empty(&self) -> bool {
        Self::is_empty(self)
    }

    fn has_target(&self) -> bool {
        self.target.is_some()
    }

    fn has_active_motion(&self) -> bool {
        !self.motion_is_empty()
    }

    fn has_running_motion_on(&self, timeline: AnimationTimelineId) -> bool {
        (timeline == AnimationTimelineId::DOCUMENT && !self.transition.is_idle())
            || self
                .layers
                .iter()
                .any(|instance| *instance.timeline() == timeline && instance.is_running())
    }

    fn running_transition_count(&self) -> usize {
        usize::from(!self.transition.is_idle())
    }

    fn running_effect_count(&self) -> usize {
        self.layers
            .iter()
            .filter(|instance| instance.is_running())
            .count()
    }

    fn animation_state(&self, id: AnimationId) -> Option<AnimationPlayState> {
        self.animation(id).map(RetainedAnimationInstance::state)
    }

    fn animation_current_time(
        &self,
        id: AnimationId,
        clock: TimelineClock<'_>,
    ) -> Option<TimerDuration> {
        self.animation(id).and_then(|instance| {
            timeline_time(instance, clock)
                .and_then(|timeline_time| instance.current_time(timeline_time))
        })
    }

    fn animation_timing_sample(
        &self,
        id: AnimationId,
        clock: TimelineClock<'_>,
    ) -> Option<TimingSample> {
        self.animation(id).and_then(|instance| {
            timeline_time(instance, clock)
                .and_then(|timeline_time| instance.timing_sample(timeline_time))
        })
    }

    fn animation_playback_rate(&self, id: AnimationId) -> Option<f64> {
        self.animation(id)
            .map(RetainedAnimationInstance::playback_rate)
    }

    fn animation_rows(
        &self,
        key: TimelineTrackKey,
        clock: TimelineClock<'_>,
        rows: &mut Vec<TimelineAnimationRuntimeRow>,
    ) {
        rows.extend(
            self.layers
                .iter()
                .map(|instance| runtime_row::<T, E>(instance, key, self.value_kind, clock)),
        );
    }

    fn advance(
        &mut self,
        key: TimelineTrackKey,
        clock: TimelineClock<'_>,
        event_at: TimerInstant,
        events: &mut Vec<AnimationEvent>,
    ) -> Option<TimelineTargetSample> {
        if self.motion_is_empty() {
            return None;
        }
        let _writes = self.transition.sample(event_at);
        let value = self.sample(self.retained_underlying(key.target()), clock);
        self.remove_completed(key, clock, event_at, events);
        value.map(|value| B::sample(key, value, self.motion_is_empty()))
    }

    fn active_sample(
        &self,
        key: TimelineTrackKey,
        clock: TimelineClock<'_>,
    ) -> Option<TimelineTargetSample> {
        if self.motion_is_empty() {
            return None;
        }
        self.sample(self.retained_underlying(key.target()), clock)
            .map(|value| B::sample(key, value, false))
    }

    fn pause(
        &mut self,
        id: AnimationId,
        key: TimelineTrackKey,
        clock: TimelineClock<'_>,
    ) -> Option<TimelineTargetControlResult> {
        let timeline_time = timeline_time(self.animation(id)?, clock)?;
        let instance = self.animation_mut(id)?;
        if !instance.pause(timeline_time) {
            return None;
        }
        Some(self.control_result(key, clock))
    }

    fn play(
        &mut self,
        id: AnimationId,
        key: TimelineTrackKey,
        clock: TimelineClock<'_>,
    ) -> Option<TimelineTargetControlResult> {
        let timeline_time = timeline_time(self.animation(id)?, clock)?;
        let instance = self.animation_mut(id)?;
        if !instance.play(timeline_time) {
            return None;
        }
        Some(self.control_result(key, clock))
    }

    fn seek(
        &mut self,
        id: AnimationId,
        key: TimelineTrackKey,
        local_time: TimerDuration,
        clock: TimelineClock<'_>,
    ) -> Option<TimelineTargetControlResult> {
        let timeline_time = timeline_time(self.animation(id)?, clock)?;
        let instance = self.animation_mut(id)?;
        instance.seek(local_time, timeline_time);
        Some(self.control_result(key, clock))
    }

    fn reverse(
        &mut self,
        id: AnimationId,
        key: TimelineTrackKey,
        clock: TimelineClock<'_>,
    ) -> Option<TimelineTargetControlResult> {
        let timeline_time = timeline_time(self.animation(id)?, clock)?;
        let instance = self.animation_mut(id)?;
        if !instance.reverse(timeline_time) {
            return None;
        }
        Some(self.control_result(key, clock))
    }

    fn set_playback_rate(
        &mut self,
        id: AnimationId,
        key: TimelineTrackKey,
        playback_rate: f64,
        clock: TimelineClock<'_>,
    ) -> Option<TimelineTargetControlResult> {
        let timeline_time = timeline_time(self.animation(id)?, clock)?;
        let instance = self.animation_mut(id)?;
        if !instance.set_playback_rate(playback_rate, timeline_time) {
            return None;
        }
        Some(self.control_result(key, clock))
    }

    fn cancel_by_id(
        &mut self,
        id: AnimationId,
        key: TimelineTrackKey,
        now: TimerInstant,
        clock: TimelineClock<'_>,
        events: &mut Vec<AnimationEvent>,
    ) -> Option<TimelineTargetControlResult> {
        if !self.remove_by_id(id, key, now, AnimationEventKind::Canceled, events) {
            return None;
        }
        Some(self.control_result(key, clock))
    }

    fn finish_by_id(
        &mut self,
        id: AnimationId,
        key: TimelineTrackKey,
        now: TimerInstant,
        clock: TimelineClock<'_>,
        events: &mut Vec<AnimationEvent>,
    ) -> Option<TimelineTargetControlResult> {
        let instance = self.animation_mut(id)?;
        if !instance.finish() {
            return None;
        }
        let value = self.sample(self.retained_underlying(key.target()), clock);
        self.remove_by_id(id, key, now, AnimationEventKind::Finished, events);
        Some(B::control_result(key, value, self.motion_is_empty()))
    }

    fn cancel_all(
        &mut self,
        key: TimelineTrackKey,
        now: TimerInstant,
        events: &mut Vec<AnimationEvent>,
    ) {
        let _ = self.transition.cancel(0, 0);
        for instance in self.layers.drain(..) {
            events.push(AnimationEvent::new(
                *instance.id(),
                key,
                AnimationEventKind::Canceled,
                now,
            ));
        }
    }
}

type RetainedValueAnimation<E> = RetainedAnimationInstance<AnimationId, AnimationTimelineId, E>;

fn sample_instance<T, E>(
    instance: &RetainedValueAnimation<E>,
    underlying: Option<T>,
    timeline_time: Option<TimelineTime>,
) -> Option<T>
where
    T: Clone + 'static,
    E: RuntimeEffect<T>,
{
    let local_time = instance.local_time(timeline_time?)?;
    instance.effect().sample_over(underlying, local_time)
}

fn timeline_time<E>(
    instance: &RetainedValueAnimation<E>,
    clock: TimelineClock<'_>,
) -> Option<TimelineTime>
where
    E: RetainedAnimationEffect,
{
    clock.time(*instance.timeline())
}

fn runtime_row<T, E>(
    instance: &RetainedValueAnimation<E>,
    key: TimelineTrackKey,
    target_value_kind: Option<PresentationValueKind>,
    clock: TimelineClock<'_>,
) -> TimelineAnimationRuntimeRow
where
    T: Clone + 'static,
    E: RuntimeEffect<T>,
{
    let timeline_time = timeline_time(instance, clock);
    TimelineAnimationRuntimeRow::new(
        *instance.id(),
        key,
        instance.state(),
        timeline_time.and_then(|time| instance.current_time(time)),
        timeline_time.and_then(|time| instance.timing_sample(time)),
        instance.playback_rate(),
        instance.effect().composite(),
        instance.effect().value_kind().or(target_value_kind),
        *instance.timeline(),
    )
}

fn queue_iteration_events<E>(
    instance: &mut RetainedValueAnimation<E>,
    key: TimelineTrackKey,
    timeline_time: Option<TimelineTime>,
    event_at: TimerInstant,
    events: &mut Vec<AnimationEvent>,
) where
    E: RetainedAnimationEffect,
{
    let Some(timeline_time) = timeline_time else {
        return;
    };
    if let Some(change) = instance.take_iteration_change(timeline_time) {
        change.for_each_crossed(|iteration| {
            events.push(AnimationEvent::new_iteration(
                *instance.id(),
                key,
                event_at,
                iteration,
            ));
        });
    }
}

impl<T> RuntimeEffect<T> for StackEffect<T>
where
    T: Animatable,
{
    fn sample_value(&self, local_time: TimelineTime) -> Option<T> {
        if self.effect.composite() == CompositeOperation::Replace {
            self.sample(local_time)
        } else {
            None
        }
    }

    fn sample_over(&self, underlying: Option<T>, local_time: TimelineTime) -> Option<T> {
        match underlying {
            Some(underlying) => {
                Some(sample_effects(&underlying, core::iter::once(self), local_time).value)
            }
            None => self.sample_value(local_time),
        }
    }

    fn composite(&self) -> CompositeOperation {
        self.effect.composite()
    }
}

impl RuntimeEffect<PresentationValue> for PresentationEffect {
    fn sample_value(&self, local_time: TimelineTime) -> Option<PresentationValue> {
        Self::sample(self, local_time)
    }

    fn sample_over(
        &self,
        underlying: Option<PresentationValue>,
        local_time: TimelineTime,
    ) -> Option<PresentationValue> {
        Self::sample_over(self, underlying, local_time)
    }

    fn composite(&self) -> CompositeOperation {
        Self::composite(self)
    }

    fn value_kind(&self) -> Option<PresentationValueKind> {
        Some(Self::value_kind(self))
    }
}

/// Built-in presentation target state.
pub(super) type TimelineTargetState =
    ValueTargetState<PresentationValue, PresentationEffect, PresentationBridge>;

/// Built-in presentation animation instance.
pub(super) type AnimationInstance = RetainedValueAnimation<PresentationEffect>;

/// App-defined native target state.
pub(super) type NativeTargetState<T> = ValueTargetState<T, StackEffect<T>, NativeBridge>;

/// App-defined native animation instance.
pub(super) type NativeAnimationInstance<T> = RetainedValueAnimation<StackEffect<T>>;
