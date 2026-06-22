// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Presentation timeline integration for [`Ui`].
//!
//! `Ui` advances the timeline from host-supplied monotonic time, converts
//! sampled tracks into ordinary invalidation, and exposes wakeup deadlines to
//! host crates. Controls can request transitions, but the runtime remains the
//! owner of active tracks.

use alloc::boxed::Box;
use alloc::vec::Vec;

use kurbo::{Affine, Vec2};
use understory_animation::{AnimationPlayState, TimingSample};
use understory_property::Property;
use understory_timing::{TimerDuration, TimerInstant};

use crate::channels::COMPOSE;
use crate::element::ElementId;
use crate::motion::{
    Animatable, AnimationTarget, EffectClip, EffectTriggerId, PresentationAnimationValue,
    PresentationSlot, PresentationTarget, PresentationValue, PresentationValueKind,
    ResolvedTransitionSpec, TransitionTrigger,
};
use crate::timeline_properties::NativeTimelinePropertyAdapter;
use crate::timeline_runtime::{
    AnimationEvent, AnimationId, AnimationTimelineId, ChangeCause, NativeTimelineControlResult,
    NativeTimelineSample, PresentationRuntimeValue, TimelineControlResult, TimelineCue,
    TimelineCueId, TimelineSample, TimelineTargetControlResult, TimelineTargetSample,
    TimelineTrackKey,
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

    /// Returns whether any presentation transition or effect is still active.
    ///
    /// Host runtimes use this after dispatch or sampling a frame to keep
    /// animation frame demand alive until all presentation motion has
    /// completed.
    #[must_use]
    pub fn has_active_presentation_motion(&self) -> bool {
        self.timeline.has_active_motion()
    }

    /// Registers a dependency property as a presentation animation target.
    ///
    /// App and control code should first register the property metadata through
    /// [`Self::registry_mut`], then pass the returned property here. This does
    /// not create or alter property metadata; it lets lower-level
    /// [`PresentationTarget`] APIs read and write that property through its
    /// animation layer.
    ///
    /// [`Self::animate_property`] and [`Self::animate_target`] register their
    /// property sample adapter when the animation starts.
    pub fn register_animatable_property<T>(&mut self, property: Property<T>)
    where
        T: PresentationAnimationValue,
    {
        self.timeline_properties.register(property);
    }

    /// Registers a dependency property as a native animation target.
    ///
    /// This is the long-term registration path for custom property values:
    /// transition planning and retained animations read and sample the
    /// property's own value type instead of requiring it to lower through
    /// [`PresentationValue`].
    pub fn register_animation_target_property<T>(&mut self, property: Property<T>)
    where
        T: Animatable,
    {
        self.timeline_properties.register_native(property);
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
        let had_runtime_state = self.timeline.clear_target(key, self.now());
        self.clear_timeline_property_sample(key);
        self.clear_native_timeline_property_sample(key);
        self.clear_timeline_composition_sample(key);
        if had_runtime_state {
            self.mark_timeline_target_channels(element, target);
        }
    }

    pub(crate) fn set_presentation_target<T: PresentationRuntimeValue>(
        &mut self,
        element: ElementId,
        timeline_target: PresentationTarget,
        value: T,
        cause: ChangeCause,
    ) -> bool {
        self.set_presentation_target_value(
            element,
            timeline_target,
            value.into_presentation_value(),
            cause,
        )
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

    pub(in crate::ui) fn set_native_presentation_property_target_value(
        &mut self,
        element: ElementId,
        property: understory_property::PropertyId,
        adapter: NativeTimelinePropertyAdapter,
        target: Box<dyn core::any::Any>,
        cause: ChangeCause,
    ) -> bool {
        let presentation_target = PresentationTarget::Property(property);
        let key = TimelineTrackKey::new(element, presentation_target);
        let now = self.now();
        let value_kind = self.native_target_value_kind(presentation_target);
        let needs_sample = adapter
            .set_target(&mut self.timeline, key, target, now, cause, value_kind)
            .unwrap_or(false);
        self.sync_timeline_target_sample(key);
        if needs_sample {
            self.mark_timeline_target_channels(element, presentation_target);
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

    pub(crate) fn presented_value<T>(
        &self,
        element: ElementId,
        property: PresentationTarget,
        target: T,
    ) -> T
    where
        T: PresentationRuntimeValue + Animatable,
    {
        let key = TimelineTrackKey::new(element, property);
        if let Some(value) = self
            .timeline
            .active_sample(key, self.now())
            .and_then(TimelineTargetSample::into_native::<T>)
        {
            return value;
        }
        self.timeline.sample(key, target, self.now())
    }

    /// Triggers target-local style-authored effects for `element`.
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
            started |= self
                .start_presentation_effect(element, rule.target(), rule.clip().clone())
                .is_some();
        }
        started
    }

    /// Starts an explicit retained presentation animation.
    ///
    /// The returned [`AnimationId`] can be used with [`Self::pause_animation`],
    /// [`Self::seek_animation`], [`Self::cancel_animation`], and the other
    /// playback-control APIs. The animation is sampled immediately, queues a
    /// started lifecycle event, and is appended above existing animation
    /// layers on the same target.
    ///
    /// Returns `None` if `element` is stale, if `target` is an unregistered
    /// property target, or if the clip's value kind does not match the target.
    pub fn animate(
        &mut self,
        element: ElementId,
        target: PresentationTarget,
        clip: EffectClip,
    ) -> Option<AnimationId> {
        self.get(element)?;
        self.start_presentation_effect(element, target, clip)
    }

    /// Starts an explicit retained presentation animation on `timeline`.
    ///
    /// Use this when a devtool, test, or app-controlled scrubber should drive
    /// progress manually. Animations on manual timelines are sampled by
    /// [`Self::seek_manual_animation_timeline`] instead of by document-frame
    /// animation demand. Finite animations remain retained at their endpoint so
    /// callers can seek the manual timeline backward without recreating them.
    pub fn animate_on_timeline(
        &mut self,
        element: ElementId,
        target: PresentationTarget,
        clip: EffectClip,
        timeline: AnimationTimelineId,
    ) -> Option<AnimationId> {
        self.get(element)?;
        self.start_presentation_effect_on_timeline(element, target, clip, timeline)
    }

    /// Starts an explicit retained animation for a presentation target.
    ///
    /// Keyframes are written in the target value's Rust type, so ordinary
    /// property and presentation-slot animations are checked at compile time
    /// before Overstory stores them in the retained runtime.
    ///
    /// ```ignore
    /// let id = ui.animate_target(
    ///     panel,
    ///     AnimationTarget::visual_translation(),
    ///     EffectClip::from_keyframes([
    ///         EffectKeyframe::new(millis(0), Vec2::ZERO),
    ///         EffectKeyframe::new(millis(180), Vec2::new(12.0, 0.0)),
    ///     ]),
    /// );
    /// ```
    pub fn animate_target<T>(
        &mut self,
        element: ElementId,
        target: AnimationTarget<T>,
        clip: EffectClip<T>,
    ) -> Option<AnimationId>
    where
        T: Animatable,
    {
        self.animate_target_on_timeline(element, target, clip, AnimationTimelineId::DOCUMENT)
    }

    /// Starts an explicit retained animation on `timeline`.
    ///
    /// Use this when a devtool, test, or app-controlled scrubber should drive
    /// progress manually. Animations on manual timelines are sampled by
    /// [`Self::seek_manual_animation_timeline`] instead of by document-frame
    /// animation demand. Finite animations remain retained at their endpoint so
    /// callers can seek the manual timeline backward without recreating them.
    pub fn animate_target_on_timeline<T>(
        &mut self,
        element: ElementId,
        target: AnimationTarget<T>,
        clip: EffectClip<T>,
        timeline: AnimationTimelineId,
    ) -> Option<AnimationId>
    where
        T: Animatable,
    {
        self.get(element)?;
        let target = target.presentation_target();
        if !self.timeline.has_timeline(timeline) {
            return None;
        }
        if let PresentationTarget::Property(property) = target {
            self.register_animation_target_property(Property::<T>::from_id(property));
        }
        let key = TimelineTrackKey::new(element, target);
        let value_kind = self.native_target_value_kind(target);
        let id = self.timeline.start_native_effect_on_timeline(
            key,
            clip,
            timeline,
            self.now(),
            value_kind,
        )?;
        if let Some(sample) = self.timeline.active_sample(key, self.now()) {
            self.apply_timeline_target_sample(sample);
        }
        self.mark_timeline_target_channels(element, target);
        Some(id)
    }

    /// Starts an explicit retained animation for a dependency property.
    ///
    /// This convenience constructor is equivalent to passing
    /// [`AnimationTarget::property`] to [`Self::animate_target`].
    pub fn animate_property<T>(
        &mut self,
        element: ElementId,
        property: Property<T>,
        clip: EffectClip<T>,
    ) -> Option<AnimationId>
    where
        T: Animatable,
    {
        self.animate_target(element, AnimationTarget::property(property), clip)
    }

    /// Starts an explicit retained dependency-property animation on `timeline`.
    ///
    /// This convenience constructor is equivalent to passing
    /// [`AnimationTarget::property`] to [`Self::animate_target_on_timeline`].
    pub fn animate_property_on_timeline<T>(
        &mut self,
        element: ElementId,
        property: Property<T>,
        clip: EffectClip<T>,
        timeline: AnimationTimelineId,
    ) -> Option<AnimationId>
    where
        T: Animatable,
    {
        self.animate_target_on_timeline(
            element,
            AnimationTarget::property(property),
            clip,
            timeline,
        )
    }

    fn start_presentation_effect(
        &mut self,
        element: ElementId,
        target: PresentationTarget,
        clip: EffectClip,
    ) -> Option<AnimationId> {
        self.start_presentation_effect_on_timeline(
            element,
            target,
            clip,
            AnimationTimelineId::DOCUMENT,
        )
    }

    fn start_presentation_effect_on_timeline(
        &mut self,
        element: ElementId,
        target: PresentationTarget,
        clip: EffectClip,
        timeline: AnimationTimelineId,
    ) -> Option<AnimationId> {
        if !self.timeline.has_timeline(timeline) {
            return None;
        }
        if !self.clip_matches_target(target, clip.value_kind()) {
            return None;
        }
        let key = TimelineTrackKey::new(element, target);
        let id = self
            .timeline
            .start_effect_on_timeline(key, clip, timeline, self.now());
        self.sync_timeline_target_sample(key);
        self.mark_timeline_target_channels(element, key.target());
        Some(id)
    }

    /// Creates a seekable animation timeline starting at zero.
    ///
    /// Manual timelines are for deterministic tests, devtools scrubbing, and
    /// app-controlled playback. They do not by themselves request animation
    /// frame demand; callers advance them by seeking. Endpoint seeks sample the
    /// endpoint but keep finite animations retained for later reverse scrubs.
    ///
    /// ```ignore
    /// let timeline = ui.create_manual_animation_timeline();
    /// let id = ui.animate_target_on_timeline(element, target, clip, timeline);
    /// ui.seek_manual_animation_timeline(timeline, millis(120));
    /// ```
    pub fn create_manual_animation_timeline(&mut self) -> AnimationTimelineId {
        self.timeline.create_manual_timeline()
    }

    /// Returns the current time for a manual animation timeline.
    #[must_use]
    pub fn manual_animation_timeline_time(
        &self,
        timeline: AnimationTimelineId,
    ) -> Option<TimerDuration> {
        self.timeline.manual_timeline_time(timeline)
    }

    /// Seeks a manual animation timeline and applies the resulting samples.
    ///
    /// Returns `false` when `timeline` does not name a live manual timeline.
    pub fn seek_manual_animation_timeline(
        &mut self,
        timeline: AnimationTimelineId,
        time: TimerDuration,
    ) -> bool {
        let Some(samples) = self
            .timeline
            .seek_manual_timeline(timeline, time, self.now())
        else {
            return false;
        };
        for sample in samples {
            self.apply_timeline_target_sample(sample);
        }
        true
    }

    fn clip_matches_target(&self, target: PresentationTarget, kind: PresentationValueKind) -> bool {
        match target {
            PresentationTarget::Property(property) => self
                .timeline_properties
                .adapter(property)
                .is_some_and(|adapter| adapter.kind() == kind),
            PresentationTarget::Slot(
                PresentationSlot::LayerOffset | PresentationSlot::VisualTranslation,
            ) => kind == PresentationValueKind::Offset2d,
        }
    }

    fn native_target_value_kind(
        &self,
        target: PresentationTarget,
    ) -> Option<PresentationValueKind> {
        match target {
            PresentationTarget::Property(property) => self
                .timeline_properties
                .adapter(property)
                .map(|adapter| adapter.kind()),
            PresentationTarget::Slot(
                PresentationSlot::LayerOffset | PresentationSlot::VisualTranslation,
            ) => Some(PresentationValueKind::Offset2d),
        }
    }

    pub(in crate::ui) fn advance_timeline(&mut self, now: TimerInstant) {
        let samples = self.timeline.advance(now);
        for sample in samples {
            self.apply_timeline_target_sample(sample);
        }
    }

    /// Returns the playback state for a retained presentation animation.
    #[must_use]
    pub fn animation_state(&self, animation: AnimationId) -> Option<AnimationPlayState> {
        self.timeline.animation_state(animation)
    }

    /// Returns the local playback time for a retained presentation animation.
    #[must_use]
    pub fn animation_current_time(&self, animation: AnimationId) -> Option<TimerDuration> {
        self.timeline.animation_current_time(animation, self.now())
    }

    /// Returns the current timing sample for a retained presentation animation.
    ///
    /// The returned [`TimingSample`] carries the phase, iteration, raw
    /// direction-adjusted progress, and eased progress used by the animation's
    /// effect sampling. It is `None` for stale ids, idle animations, or
    /// animations that are not currently producing a timing sample.
    #[must_use]
    pub fn animation_timing_sample(&self, animation: AnimationId) -> Option<TimingSample> {
        self.timeline.animation_timing_sample(animation, self.now())
    }

    /// Returns the playback rate for a retained presentation animation.
    #[must_use]
    pub fn animation_playback_rate(&self, animation: AnimationId) -> Option<f64> {
        self.timeline.animation_playback_rate(animation)
    }

    /// Pauses a running presentation animation at the current UI time.
    ///
    /// Returns `true` when `animation` was retained and running. Paused
    /// animations remain inspectable and can be resumed with
    /// [`Self::play_animation`], but they no longer contribute animation frame
    /// demand.
    pub fn pause_animation(&mut self, animation: AnimationId) -> bool {
        let Some(result) = self.timeline.pause_animation(animation, self.now()) else {
            return false;
        };
        self.apply_timeline_target_control_result(result);
        true
    }

    /// Starts or resumes a retained presentation animation.
    ///
    /// Returns `true` when `animation` was retained and not already running.
    pub fn play_animation(&mut self, animation: AnimationId) -> bool {
        let Some(result) = self.timeline.play_animation(animation, self.now()) else {
            return false;
        };
        self.apply_timeline_target_control_result(result);
        true
    }

    /// Seeks a retained presentation animation to `local_time`.
    ///
    /// The seek is sampled immediately at the current UI time, so properties
    /// and composition layers reflect the new local time before the next frame.
    pub fn seek_animation(&mut self, animation: AnimationId, local_time: TimerDuration) -> bool {
        let Some(result) = self
            .timeline
            .seek_animation(animation, local_time, self.now())
        else {
            return false;
        };
        self.apply_timeline_target_control_result(result);
        true
    }

    /// Reverses a retained presentation animation at the current UI time.
    ///
    /// The current sampled value is preserved as the new playback anchor.
    pub fn reverse_animation(&mut self, animation: AnimationId) -> bool {
        let Some(result) = self.timeline.reverse_animation(animation, self.now()) else {
            return false;
        };
        self.apply_timeline_target_control_result(result);
        true
    }

    /// Sets the playback rate for a retained presentation animation.
    ///
    /// Non-finite rates are rejected. A rate of `0.0` freezes local time while
    /// the animation remains in the running state; use [`Self::pause_animation`]
    /// when the animation should stop contributing frame demand.
    pub fn set_animation_playback_rate(
        &mut self,
        animation: AnimationId,
        playback_rate: f64,
    ) -> bool {
        let Some(result) =
            self.timeline
                .set_animation_playback_rate(animation, playback_rate, self.now())
        else {
            return false;
        };
        self.apply_timeline_target_control_result(result);
        true
    }

    /// Cancels a retained presentation animation.
    ///
    /// Canceling removes the animation instance and queues a canceled
    /// lifecycle event. The affected target is resampled immediately from the
    /// remaining stack or its underlying value.
    pub fn cancel_animation(&mut self, animation: AnimationId) -> bool {
        let Some(result) = self.timeline.cancel_animation(animation, self.now()) else {
            return false;
        };
        self.apply_timeline_target_control_result(result);
        true
    }

    /// Finishes a retained presentation animation at its natural endpoint.
    ///
    /// Returns `false` for stale ids or indefinite animations.
    pub fn finish_animation(&mut self, animation: AnimationId) -> bool {
        let Some(result) = self.timeline.finish_animation(animation, self.now()) else {
            return false;
        };
        self.apply_timeline_target_control_result(result);
        true
    }

    /// Drains timeline animation lifecycle events in runtime order.
    ///
    /// Events are queued when retained presentation animation instances start,
    /// cross iteration boundaries, finish, or are canceled by
    /// replacement/removal. Draining does not advance time or sample
    /// animations.
    pub fn drain_animation_events(&mut self) -> Vec<AnimationEvent> {
        self.timeline.drain_animation_events()
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

    fn apply_native_timeline_sample(&mut self, sample: NativeTimelineSample) {
        let key = sample.key();
        if sample.is_complete() {
            self.clear_native_timeline_property_sample(key);
        } else {
            self.set_native_timeline_property_sample(key, sample.value());
        }
        self.set_native_timeline_composition_sample(key, sample.value());
        self.mark_timeline_target_channels(key.element(), key.target());
    }

    fn apply_timeline_target_sample(&mut self, sample: TimelineTargetSample) {
        match sample {
            TimelineTargetSample::Presentation(sample) => self.apply_timeline_sample(sample),
            TimelineTargetSample::Native(sample) => self.apply_native_timeline_sample(sample),
        }
    }

    fn apply_timeline_control_result(&mut self, result: TimelineControlResult) {
        if let Some(sample) = result.sample() {
            self.apply_timeline_sample(sample);
            return;
        }

        let key = result.key();
        if result.is_complete() {
            self.clear_timeline_property_sample(key);
        }
        self.clear_timeline_composition_sample(key);
        self.mark_timeline_target_channels(key.element(), key.target());
    }

    fn apply_native_timeline_control_result(&mut self, result: NativeTimelineControlResult) {
        let key = result.key();
        let is_complete = result.is_complete();
        if let Some(sample) = result.sample() {
            self.apply_native_timeline_sample(sample);
            return;
        }

        if is_complete {
            self.clear_native_timeline_property_sample(key);
        }
        self.clear_native_timeline_composition_sample(key);
        self.mark_timeline_target_channels(key.element(), key.target());
    }

    fn apply_timeline_target_control_result(&mut self, result: TimelineTargetControlResult) {
        match result {
            TimelineTargetControlResult::Presentation(result) => {
                self.apply_timeline_control_result(result);
            }
            TimelineTargetControlResult::Native(result) => {
                self.apply_native_timeline_control_result(result);
            }
        }
    }

    fn sync_timeline_property_sample(&mut self, key: TimelineTrackKey) {
        if let Some(value) = self.timeline.active_sample_value(key, self.now()) {
            self.set_timeline_property_sample(key, value);
        } else {
            self.clear_timeline_property_sample(key);
        }
    }

    fn sync_timeline_target_sample(&mut self, key: TimelineTrackKey) {
        if let Some(sample) = self.timeline.active_sample(key, self.now()) {
            self.apply_timeline_target_sample(sample);
            return;
        }
        self.clear_timeline_property_sample(key);
        self.clear_timeline_composition_sample(key);
        self.clear_native_timeline_property_sample(key);
        self.clear_native_timeline_composition_sample(key);
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

    fn set_native_timeline_property_sample(
        &mut self,
        key: TimelineTrackKey,
        value: &dyn core::any::Any,
    ) {
        let PresentationTarget::Property(property) = key.target() else {
            return;
        };
        let Some(adapter) = self.timeline_properties.native_adapter(property) else {
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

    fn clear_native_timeline_property_sample(&mut self, key: TimelineTrackKey) {
        let PresentationTarget::Property(property) = key.target() else {
            return;
        };
        let Some(adapter) = self.timeline_properties.native_adapter(property) else {
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

    fn set_native_timeline_composition_sample(
        &mut self,
        key: TimelineTrackKey,
        value: &dyn core::any::Any,
    ) {
        match key.target() {
            PresentationTarget::Slot(PresentationSlot::LayerOffset) => {
                let Some(offset) = value.downcast_ref::<Vec2>() else {
                    return;
                };
                self.composition.set_layer_offset(key.element(), *offset);
            }
            PresentationTarget::Slot(PresentationSlot::VisualTranslation) => {
                let Some(translation) = value.downcast_ref::<Vec2>() else {
                    return;
                };
                self.composition.set_visual_transform(
                    key.element(),
                    Affine::translate((translation.x, translation.y))
                        * self.effective_visual_transform(key.element()),
                );
            }
            PresentationTarget::Property(property) if property == self.properties.opacity.id() => {
                let Some(opacity) = value.downcast_ref::<f32>() else {
                    return;
                };
                self.composition.set_opacity(key.element(), *opacity);
            }
            PresentationTarget::Property(_) => {}
        }
    }

    fn clear_timeline_composition_sample(&mut self, key: TimelineTrackKey) {
        match key.target() {
            PresentationTarget::Slot(PresentationSlot::LayerOffset) => {
                self.composition.set_layer_offset(key.element(), Vec2::ZERO);
            }
            PresentationTarget::Slot(PresentationSlot::VisualTranslation) => {
                self.composition.set_visual_transform(
                    key.element(),
                    self.presented_visual_transform(key.element()),
                );
            }
            PresentationTarget::Property(property) if property == self.properties.opacity.id() => {
                self.composition
                    .set_opacity(key.element(), self.presented_layer_opacity(key.element()));
            }
            PresentationTarget::Property(_) => {}
        }
    }

    fn clear_native_timeline_composition_sample(&mut self, key: TimelineTrackKey) {
        self.clear_timeline_composition_sample(key);
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

    /// Returns the next timer or timeline-cue deadline the host should wake for.
    ///
    /// Active presentation motion is driven by frame demand, not by this timer
    /// wakeup path.
    #[must_use]
    pub fn next_wakeup_deadline(&self) -> Option<TimerInstant> {
        min_deadline(
            self.next_timer_deadline(),
            self.timeline.next_cue_deadline(self.now()),
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
