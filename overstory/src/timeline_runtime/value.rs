// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::any::Any;
use core::marker::PhantomData;

use understory_animation::{
    AnimationPlayState, AnimationTiming, CompositeOperation, FillMode, Keyframe, KeyframeEffect,
    RetainedAnimationEffect, RetainedAnimationInstance, StackEffect, TimingSample, sample_effects,
};
use understory_animation_timeline::TimelineTime;
use understory_timing::{TimerDuration, TimerInstant};

use crate::motion::{
    Animatable, PresentationTarget, PresentationValue, PresentationValueKind,
    ResolvedTransitionSpec,
};

use super::effect::PresentationEffect;
use super::instance::{AnimationEvent, AnimationEventKind, AnimationId, next_animation_id};
use super::target::{
    ErasedTargetState, NativeTimelineControlResult, NativeTimelineSample,
    TimelineTargetControlResult, TimelineTargetSample,
};
use super::timeline::{AnimationTimelineId, TimelineClock};
use super::types::{
    TimelineAnimationLayerKind, TimelineAnimationRuntimeRow, TimelineControlResult, TimelineSample,
    TimelineTrackKey, default_underlying_value, same_value,
};

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

/// Runtime effect adapter that can synthesize an implicit transition.
pub(super) trait RuntimeTransitionEffect<T>: RuntimeEffect<T> {
    /// Creates a replacement transition from `from` to `to`.
    fn transition(from: T, to: T, spec: ResolvedTransitionSpec) -> Self;
}

/// Adapts a concrete target state into the erased runtime sample/result enums.
pub(super) trait TargetValueBridge<T>: Default + 'static {
    /// Returns whether two target values are equivalent for retargeting.
    fn values_close(a: &T, b: &T) -> bool;

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
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ValueTargetState<T, E, B> {
    target: Option<T>,
    layers: Vec<ValueAnimationLayer<T, E>>,
    value_kind: Option<PresentationValueKind>,
    _bridge: PhantomData<B>,
}

impl<T, E, B> Default for ValueTargetState<T, E, B> {
    fn default() -> Self {
        Self {
            target: None,
            layers: Vec::new(),
            value_kind: None,
            _bridge: PhantomData,
        }
    }
}

impl<T, E, B> ValueTargetState<T, E, B>
where
    T: Clone + 'static,
    E: RuntimeTransitionEffect<T>,
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
        let underlying = fallback.or_else(|| self.target.clone());
        self.sample_stack(underlying.clone(), clock).or(underlying)
    }

    /// Returns the sampled transition layer, excluding explicit effects.
    pub(super) fn sample_transition_layer(
        &self,
        fallback: Option<T>,
        clock: TimelineClock<'_>,
    ) -> Option<T> {
        let underlying = fallback.or_else(|| self.target.clone());
        self.sample_stack_until_effect(underlying.clone(), clock)
            .or(underlying)
    }

    /// Appends an explicit effect layer.
    pub(super) fn set_effect(
        &mut self,
        key: TimelineTrackKey,
        instance: ValueAnimationInstance<T, E>,
        event_at: TimerInstant,
        events: &mut Vec<AnimationEvent>,
    ) {
        events.push(AnimationEvent::new(
            instance.id(),
            key,
            AnimationEventKind::Started,
            event_at,
        ));
        self.layers.push(ValueAnimationLayer::new(instance));
    }

    /// Records a new target value and updates the implicit transition layer.
    #[expect(
        clippy::too_many_arguments,
        reason = "retargeting needs runtime context"
    )]
    pub(super) fn set_target(
        &mut self,
        key: TimelineTrackKey,
        target: T,
        spec: Option<ResolvedTransitionSpec>,
        clock: TimelineClock<'_>,
        timeline_time: TimelineTime,
        event_at: TimerInstant,
        next_animation: &mut u64,
        events: &mut Vec<AnimationEvent>,
    ) -> bool {
        let previous_target = self.target.clone();
        let current_presented = self
            .sample_transition_layer(
                previous_target.clone().or_else(|| Some(target.clone())),
                clock,
            )
            .unwrap_or_else(|| target.clone());
        self.target = Some(target.clone());
        let target_changed = previous_target
            .as_ref()
            .is_none_or(|previous| !B::values_close(previous, &target));
        if !target_changed {
            return false;
        }

        let had_transition = self.has_transition();
        match spec {
            Some(spec) if !B::values_close(&current_presented, &target) && spec.duration() != 0 => {
                let id = next_animation_id(next_animation);
                let instance = ValueAnimationInstance::new(
                    id,
                    AnimationTimelineId::DOCUMENT,
                    E::transition(current_presented, target, spec),
                    timeline_time,
                );
                self.set_transition(key, instance, event_at, events);
                true
            }
            Some(_) | None => {
                self.clear_transition(key, event_at, events);
                had_transition
            }
        }
    }

    /// Returns whether an implicit transition layer is retained.
    pub(super) fn has_transition(&self) -> bool {
        self.layers
            .iter()
            .any(|layer| layer.kind == TimelineAnimationLayerKind::Transition)
    }

    /// Replaces the implicit transition layer.
    pub(super) fn set_transition(
        &mut self,
        key: TimelineTrackKey,
        instance: ValueAnimationInstance<T, E>,
        event_at: TimerInstant,
        events: &mut Vec<AnimationEvent>,
    ) {
        self.cancel_layers(
            key,
            event_at,
            TimelineAnimationLayerKind::Transition,
            events,
        );
        events.push(AnimationEvent::new(
            instance.id(),
            key,
            AnimationEventKind::Started,
            event_at,
        ));
        let insertion = self
            .layers
            .iter()
            .position(|layer| layer.kind == TimelineAnimationLayerKind::Effect)
            .unwrap_or(self.layers.len());
        self.layers.insert(
            insertion,
            ValueAnimationLayer::with_kind(TimelineAnimationLayerKind::Transition, instance),
        );
    }

    /// Clears the implicit transition layer.
    pub(super) fn clear_transition(
        &mut self,
        key: TimelineTrackKey,
        now: TimerInstant,
        events: &mut Vec<AnimationEvent>,
    ) {
        self.cancel_layers(key, now, TimelineAnimationLayerKind::Transition, events);
    }

    fn motion_is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    fn sample_stack(&self, underlying: Option<T>, clock: TimelineClock<'_>) -> Option<T> {
        self.layers.iter().fold(underlying, |value, layer| {
            layer
                .instance
                .sample_over(value, layer.instance.timeline_time(clock))
        })
    }

    fn sample_stack_until_effect(
        &self,
        underlying: Option<T>,
        clock: TimelineClock<'_>,
    ) -> Option<T> {
        let effect_start = self
            .layers
            .iter()
            .position(|layer| layer.kind == TimelineAnimationLayerKind::Effect)
            .unwrap_or(self.layers.len());
        self.layers[..effect_start]
            .iter()
            .fold(underlying, |value, layer| {
                layer
                    .instance
                    .sample_over(value, layer.instance.timeline_time(clock))
            })
    }

    fn animation(&self, id: AnimationId) -> Option<&ValueAnimationInstance<T, E>> {
        self.layers
            .iter()
            .find(|layer| layer.instance.id() == id)
            .map(|layer| &layer.instance)
    }

    fn animation_mut(&mut self, id: AnimationId) -> Option<&mut ValueAnimationInstance<T, E>> {
        self.layers
            .iter_mut()
            .find(|layer| layer.instance.id() == id)
            .map(|layer| &mut layer.instance)
    }

    fn control_result(
        &self,
        key: TimelineTrackKey,
        clock: TimelineClock<'_>,
    ) -> TimelineTargetControlResult {
        B::control_result(
            key,
            self.sample(B::default_underlying(key.target()), clock),
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
        for mut layer in self.layers.drain(..) {
            let timeline_time = layer.instance.timeline_time(clock);
            layer
                .instance
                .queue_iteration_events(key, timeline_time, event_at, events);
            // Manual timelines are scrub sources: reaching the effect endpoint
            // should sample the endpoint, not retire the retained instance.
            if layer.instance.timeline() == AnimationTimelineId::DOCUMENT
                && timeline_time
                    .is_some_and(|timeline_time| layer.instance.finish_if_complete(timeline_time))
            {
                events.push(AnimationEvent::new(
                    layer.instance.id(),
                    key,
                    AnimationEventKind::Finished,
                    event_at,
                ));
            } else {
                retained.push(layer);
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
        let Some(index) = self
            .layers
            .iter()
            .position(|layer| layer.instance.id() == id)
        else {
            return false;
        };
        let layer = self.layers.remove(index);
        events.push(AnimationEvent::new(layer.instance.id(), key, kind, now));
        true
    }

    fn cancel_layers(
        &mut self,
        key: TimelineTrackKey,
        now: TimerInstant,
        kind: TimelineAnimationLayerKind,
        events: &mut Vec<AnimationEvent>,
    ) {
        let mut retained = Vec::new();
        for layer in self.layers.drain(..) {
            if layer.kind == kind {
                events.push(AnimationEvent::new(
                    layer.instance.id(),
                    key,
                    AnimationEventKind::Canceled,
                    now,
                ));
            } else {
                retained.push(layer);
            }
        }
        self.layers = retained;
    }
}

impl<T, E, B> ErasedTargetState for ValueTargetState<T, E, B>
where
    T: Clone + 'static,
    E: RuntimeTransitionEffect<T>,
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
        self.layers
            .iter()
            .any(|layer| layer.instance.timeline() == timeline && layer.instance.is_running())
    }

    fn running_transition_count(&self) -> usize {
        self.layers
            .iter()
            .filter(|layer| layer.kind == TimelineAnimationLayerKind::Transition)
            .filter(|layer| layer.instance.is_running())
            .count()
    }

    fn running_effect_count(&self) -> usize {
        self.layers
            .iter()
            .filter(|layer| layer.kind == TimelineAnimationLayerKind::Effect)
            .filter(|layer| layer.instance.is_running())
            .count()
    }

    fn animation_state(&self, id: AnimationId) -> Option<AnimationPlayState> {
        self.animation(id).map(ValueAnimationInstance::state)
    }

    fn animation_current_time(
        &self,
        id: AnimationId,
        clock: TimelineClock<'_>,
    ) -> Option<TimerDuration> {
        self.animation(id).and_then(|instance| {
            instance
                .timeline_time(clock)
                .and_then(|timeline_time| instance.current_time(timeline_time))
        })
    }

    fn animation_timing_sample(
        &self,
        id: AnimationId,
        clock: TimelineClock<'_>,
    ) -> Option<TimingSample> {
        self.animation(id).and_then(|instance| {
            instance
                .timeline_time(clock)
                .and_then(|timeline_time| instance.timing_sample(timeline_time))
        })
    }

    fn animation_playback_rate(&self, id: AnimationId) -> Option<f64> {
        self.animation(id)
            .map(ValueAnimationInstance::playback_rate)
    }

    fn animation_rows(
        &self,
        key: TimelineTrackKey,
        clock: TimelineClock<'_>,
        rows: &mut Vec<TimelineAnimationRuntimeRow>,
    ) {
        rows.extend(self.layers.iter().map(|layer| {
            layer
                .instance
                .runtime_row(key, layer.kind, self.value_kind, clock)
        }));
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
        let value = self.sample(B::default_underlying(key.target()), clock);
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
        self.sample(B::default_underlying(key.target()), clock)
            .map(|value| B::sample(key, value, false))
    }

    fn pause(
        &mut self,
        id: AnimationId,
        key: TimelineTrackKey,
        clock: TimelineClock<'_>,
    ) -> Option<TimelineTargetControlResult> {
        let timeline_time = self.animation(id)?.timeline_time(clock)?;
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
        let timeline_time = self.animation(id)?.timeline_time(clock)?;
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
        let timeline_time = self.animation(id)?.timeline_time(clock)?;
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
        let timeline_time = self.animation(id)?.timeline_time(clock)?;
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
        let timeline_time = self.animation(id)?.timeline_time(clock)?;
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
        let value = self.sample(B::default_underlying(key.target()), clock);
        self.remove_by_id(id, key, now, AnimationEventKind::Finished, events);
        Some(B::control_result(key, value, self.motion_is_empty()))
    }

    fn cancel_all(
        &mut self,
        key: TimelineTrackKey,
        now: TimerInstant,
        events: &mut Vec<AnimationEvent>,
    ) {
        for layer in self.layers.drain(..) {
            events.push(AnimationEvent::new(
                layer.instance.id(),
                key,
                AnimationEventKind::Canceled,
                now,
            ));
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ValueAnimationLayer<T, E> {
    kind: TimelineAnimationLayerKind,
    instance: ValueAnimationInstance<T, E>,
}

impl<T, E> ValueAnimationLayer<T, E> {
    const fn new(instance: ValueAnimationInstance<T, E>) -> Self {
        Self::with_kind(TimelineAnimationLayerKind::Effect, instance)
    }

    const fn with_kind(
        kind: TimelineAnimationLayerKind,
        instance: ValueAnimationInstance<T, E>,
    ) -> Self {
        Self { kind, instance }
    }
}

/// Retained animation instance for one native effect adapter.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ValueAnimationInstance<T, E> {
    inner: RetainedAnimationInstance<AnimationId, AnimationTimelineId, E>,
    _value: PhantomData<T>,
}

impl<T, E> ValueAnimationInstance<T, E>
where
    T: Clone + 'static,
    E: RuntimeEffect<T>,
{
    /// Creates a retained animation instance.
    pub(super) fn new(
        id: AnimationId,
        timeline: AnimationTimelineId,
        effect: E,
        started_at: TimelineTime,
    ) -> Self {
        Self {
            inner: RetainedAnimationInstance::new(id, timeline, effect, started_at),
            _value: PhantomData,
        }
    }

    fn id(&self) -> AnimationId {
        *self.inner.id()
    }

    fn timeline(&self) -> AnimationTimelineId {
        *self.inner.timeline()
    }

    fn state(&self) -> AnimationPlayState {
        self.inner.state()
    }

    fn playback_rate(&self) -> f64 {
        self.inner.playback_rate()
    }

    fn is_running(&self) -> bool {
        self.inner.is_running()
    }

    fn runtime_row(
        &self,
        key: TimelineTrackKey,
        layer: TimelineAnimationLayerKind,
        target_value_kind: Option<PresentationValueKind>,
        clock: TimelineClock<'_>,
    ) -> TimelineAnimationRuntimeRow {
        let timeline_time = self.timeline_time(clock);
        TimelineAnimationRuntimeRow::new(
            self.id(),
            key,
            layer,
            self.state(),
            timeline_time.and_then(|timeline_time| self.current_time(timeline_time)),
            timeline_time.and_then(|timeline_time| self.timing_sample(timeline_time)),
            self.playback_rate(),
            self.composite(),
            self.inner.effect().value_kind().or(target_value_kind),
            self.timeline(),
        )
    }

    fn composite(&self) -> CompositeOperation {
        self.inner.effect().composite()
    }

    fn current_time(&self, timeline_time: TimelineTime) -> Option<TimerDuration> {
        self.inner.current_time(timeline_time)
    }

    fn timing_sample(&self, timeline_time: TimelineTime) -> Option<TimingSample> {
        self.inner.timing_sample(timeline_time)
    }

    fn timeline_time(&self, clock: TimelineClock<'_>) -> Option<TimelineTime> {
        clock.time(self.timeline())
    }

    fn sample_over(&self, underlying: Option<T>, timeline_time: Option<TimelineTime>) -> Option<T> {
        let time = self.inner.local_time(timeline_time?)?;
        self.inner.effect().sample_over(underlying, time)
    }

    fn play(&mut self, timeline_time: TimelineTime) -> bool {
        self.inner.play(timeline_time)
    }

    fn pause(&mut self, timeline_time: TimelineTime) -> bool {
        self.inner.pause(timeline_time)
    }

    fn seek(&mut self, local_time: TimerDuration, timeline_time: TimelineTime) {
        self.inner.seek(local_time, timeline_time);
    }

    fn reverse(&mut self, timeline_time: TimelineTime) -> bool {
        self.inner.reverse(timeline_time)
    }

    fn set_playback_rate(&mut self, playback_rate: f64, timeline_time: TimelineTime) -> bool {
        self.inner.set_playback_rate(playback_rate, timeline_time)
    }

    fn finish(&mut self) -> bool {
        self.inner.finish()
    }

    fn finish_if_complete(&mut self, timeline_time: TimelineTime) -> bool {
        self.inner.finish_if_complete(timeline_time)
    }

    fn queue_iteration_events(
        &mut self,
        key: TimelineTrackKey,
        timeline_time: Option<TimelineTime>,
        event_at: TimerInstant,
        events: &mut Vec<AnimationEvent>,
    ) {
        let Some(timeline_time) = timeline_time else {
            return;
        };
        if let Some(change) = self.inner.take_iteration_change(timeline_time) {
            change.for_each_crossed(|iteration| {
                events.push(AnimationEvent::new_iteration(
                    self.id(),
                    key,
                    event_at,
                    iteration,
                ));
            });
        }
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

impl<T> RuntimeTransitionEffect<T> for StackEffect<T>
where
    T: Animatable,
{
    fn transition(from: T, to: T, spec: ResolvedTransitionSpec) -> Self {
        Self::new(
            KeyframeEffect::new(Vec::from([
                Keyframe::new(0.0, from),
                Keyframe::new(1.0, to),
            ]))
            .with_composite(CompositeOperation::Replace),
            AnimationTiming {
                duration: spec.duration(),
                easing: spec.timing(),
                fill: FillMode::Forwards,
                ..AnimationTiming::new(spec.duration())
            },
        )
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

impl RuntimeTransitionEffect<PresentationValue> for PresentationEffect {
    fn transition(
        from: PresentationValue,
        to: PresentationValue,
        spec: ResolvedTransitionSpec,
    ) -> Self {
        Self::transition(from, to, spec)
    }
}

/// Built-in presentation target state.
pub(super) type TimelineTargetState =
    ValueTargetState<PresentationValue, PresentationEffect, PresentationBridge>;

/// Built-in presentation animation instance.
pub(super) type AnimationInstance = ValueAnimationInstance<PresentationValue, PresentationEffect>;

/// App-defined native target state.
pub(super) type NativeTargetState<T> = ValueTargetState<T, StackEffect<T>, NativeBridge>;

/// App-defined native animation instance.
pub(super) type NativeAnimationInstance<T> = ValueAnimationInstance<T, StackEffect<T>>;
