// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Renderer-neutral motion primitives.
//!
//! This module owns the reusable pieces of time-sampled value motion:
//! interpolation, timing functions, and single-value transition clips. It
//! deliberately does not know about elements, dependency properties, style,
//! invalidation, layout, or composition backends.
//!
//! Overstory exposes three authoring tiers over one retained runtime:
//!
//! 1. [`Transitions`] attaches typed, theme-aware motion to style changes.
//! 2. [`Ui::with_motion`](crate::Ui::with_motion) applies one policy to
//!    synchronous retained mutations without requiring a controller.
//! 3. [`EffectClip`] and [`Ui::animate_target`](crate::Ui::animate_target)
//!    retain explicit keyframes, timelines, composition, and playback control.
//!
//! The first two tiers lower into the same transition tracks. They do not
//! create a second state system or a second sampler.

mod authoring;

use alloc::vec;
use alloc::vec::Vec;
use core::marker::PhantomData;

use kurbo::Vec2;
use peniko::Color;
pub use understory_animation::{
    AnimationTiming, CompositeOperation, FillMode, Keyframe, KeyframeEffect, PlaybackDirection,
    StackEffect, TimingPhase, TimingSample,
};
pub use understory_motion::{
    AnimatableValue as Animatable, Interpolate, TimingFunction, Transform2d, Transition, millis,
    normalized_progress,
};
use understory_property::{Property, PropertyId};
use understory_timing::TimerDuration;

pub(crate) use authoring::MotionValue;
pub use authoring::{ColorTransitionProperty, MotionSpec, Transitions};

/// Presentation-only output sampled by the motion runtime.
///
/// Slots are intentionally not dependency properties. They are for values
/// produced or consumed by the presentation pipeline when there is no durable
/// authored target value in the property system.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PresentationSlot {
    /// Layer-local placement offset sampled during composition.
    ///
    /// This is for targets whose real geometry has a layout-derived local
    /// position, such as a slider thumb, toggle thumb, or selection indicator.
    /// Layout resolves the target position, and composition can present motion
    /// between old and new positions without relaying out siblings.
    ///
    /// Do not use this for transient feedback like shake or denial motion.
    /// Those effects should use [`Self::VisualTranslation`] so they compose
    /// additively on top of resolved layer placement and disappear without
    /// changing target geometry.
    LayerOffset,
    /// Additive visual translation sampled during composition.
    ///
    /// Unlike [`Self::LayerOffset`], this is not layout's resolved local
    /// position and should not become the target geometry for a real part. It is
    /// a transient presentation nudge composed on top of layout, which makes it
    /// suitable for feedback effects such as shake or press response.
    VisualTranslation,
}

/// Destination whose sampled presentation value may differ from its target
/// value for a period of time.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PresentationTarget {
    /// Dependency property whose animation value participates in normal
    /// property resolution, invalidation, inheritance, and template owner
    /// dependency tracking.
    Property(PropertyId),
    /// Presentation-only slot that does not have an authoritative dependency
    /// property value.
    Slot(PresentationSlot),
}

impl PresentationTarget {
    /// Creates a target for dependency property `property`.
    #[must_use]
    pub const fn property(property: PropertyId) -> Self {
        Self::Property(property)
    }

    /// Creates a target for a presentation-only `slot`.
    #[must_use]
    pub const fn slot(slot: PresentationSlot) -> Self {
        Self::Slot(slot)
    }

    /// Creates a target for layout-derived layer-local composition offset.
    #[must_use]
    pub const fn layer_offset() -> Self {
        Self::Slot(PresentationSlot::LayerOffset)
    }

    /// Creates a target for additive transient visual translation.
    #[must_use]
    pub const fn visual_translation() -> Self {
        Self::Slot(PresentationSlot::VisualTranslation)
    }
}

/// Animation destination with a concrete value type.
///
/// Use this with [`EffectClip<T>`] when the target value type is known at
/// the call site. Overstory can then reject mismatched target and keyframe
/// types at compile time for ordinary property and presentation-slot
/// animations.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AnimationTarget<T> {
    target: PresentationTarget,
    _marker: PhantomData<fn() -> T>,
}

impl<T> AnimationTarget<T> {
    /// Creates a target from a presentation target.
    const fn new(target: PresentationTarget) -> Self {
        Self {
            target,
            _marker: PhantomData,
        }
    }

    /// Returns the presentation target identity for lower-level APIs.
    #[must_use]
    pub const fn presentation_target(self) -> PresentationTarget {
        self.target
    }
}

impl<T> AnimationTarget<T> {
    /// Creates a target for dependency property `property`.
    #[must_use]
    pub const fn property(property: Property<T>) -> Self {
        Self::new(PresentationTarget::Property(property.id()))
    }
}

impl AnimationTarget<Vec2> {
    /// Creates a target for layout-derived layer-local composition offset.
    #[must_use]
    pub const fn layer_offset() -> Self {
        Self::new(PresentationTarget::Slot(PresentationSlot::LayerOffset))
    }

    /// Creates a target for additive transient visual translation.
    #[must_use]
    pub const fn visual_translation() -> Self {
        Self::new(PresentationTarget::Slot(
            PresentationSlot::VisualTranslation,
        ))
    }
}

/// Storage/interpolation kind for a sampled presentation value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentationValueKind {
    /// A single `f32` value.
    Scalar32,
    /// A single `f64` value.
    Scalar64,
    /// A straight-alpha color.
    Color,
    /// A two-dimensional logical offset.
    Offset2d,
}

/// Value carried by a presentation track or effect keyframe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PresentationValue {
    /// A single `f32` value.
    Scalar32(f32),
    /// A single `f64` value.
    Scalar64(f64),
    /// A straight-alpha color.
    Color(Color),
    /// A two-dimensional logical offset.
    Offset2d(Vec2),
}

impl PresentationValue {
    /// Returns the storage/interpolation kind for this value.
    #[must_use]
    pub const fn kind(self) -> PresentationValueKind {
        match self {
            Self::Scalar32(_) => PresentationValueKind::Scalar32,
            Self::Scalar64(_) => PresentationValueKind::Scalar64,
            Self::Color(_) => PresentationValueKind::Color,
            Self::Offset2d(_) => PresentationValueKind::Offset2d,
        }
    }
}

/// Property value type that can be animated through Overstory presentation samples.
///
/// This is the bridge between a dependency property value and Overstory's
/// current presentation sample storage. A type may implement this directly when
/// it can losslessly map to one of the built-in sample kinds, such as a domain
/// wrapper around `f64` or `Vec2`. The interpolation and composition math then
/// happens in that presentation representation, while the final sampled value
/// is written back to the original property type.
pub trait PresentationAnimationValue: Copy + PartialEq + 'static {
    /// Runtime storage kind for this value.
    const KIND: PresentationValueKind;

    /// Converts this value into erased presentation storage.
    fn into_presentation_value(self) -> PresentationValue;

    /// Converts erased presentation storage back into this value.
    fn from_presentation_value(value: PresentationValue) -> Option<Self>;
}

impl PresentationAnimationValue for f32 {
    const KIND: PresentationValueKind = PresentationValueKind::Scalar32;

    fn into_presentation_value(self) -> PresentationValue {
        PresentationValue::Scalar32(self)
    }

    fn from_presentation_value(value: PresentationValue) -> Option<Self> {
        match value {
            PresentationValue::Scalar32(value) => Some(value),
            PresentationValue::Scalar64(_)
            | PresentationValue::Color(_)
            | PresentationValue::Offset2d(_) => None,
        }
    }
}

impl PresentationAnimationValue for f64 {
    const KIND: PresentationValueKind = PresentationValueKind::Scalar64;

    fn into_presentation_value(self) -> PresentationValue {
        PresentationValue::Scalar64(self)
    }

    fn from_presentation_value(value: PresentationValue) -> Option<Self> {
        match value {
            PresentationValue::Scalar64(value) => Some(value),
            PresentationValue::Scalar32(_)
            | PresentationValue::Color(_)
            | PresentationValue::Offset2d(_) => None,
        }
    }
}

impl PresentationAnimationValue for Color {
    const KIND: PresentationValueKind = PresentationValueKind::Color;

    fn into_presentation_value(self) -> PresentationValue {
        PresentationValue::Color(self)
    }

    fn from_presentation_value(value: PresentationValue) -> Option<Self> {
        match value {
            PresentationValue::Color(value) => Some(value),
            PresentationValue::Scalar32(_)
            | PresentationValue::Scalar64(_)
            | PresentationValue::Offset2d(_) => None,
        }
    }
}

impl PresentationAnimationValue for Vec2 {
    const KIND: PresentationValueKind = PresentationValueKind::Offset2d;

    fn into_presentation_value(self) -> PresentationValue {
        PresentationValue::Offset2d(self)
    }

    fn from_presentation_value(value: PresentationValue) -> Option<Self> {
        match value {
            PresentationValue::Offset2d(value) => Some(value),
            PresentationValue::Scalar32(_)
            | PresentationValue::Scalar64(_)
            | PresentationValue::Color(_) => None,
        }
    }
}

/// Stable semantic trigger for a style-authored presentation effect.
///
/// Controls emit effect triggers to report interaction outcomes. Style rules
/// decide whether that trigger produces visual feedback and which presentation
/// target receives it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct EffectTriggerId(u32);

impl EffectTriggerId {
    /// Creates an effect trigger id from a stable numeric value.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns this trigger id as a compact integer token.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// One keyframe in an effect clip.
///
/// Keyframes for [`EffectClip<T>`] let app and control code author animations
/// in the property or presentation-slot value's own Rust type.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectKeyframe<T = PresentationValue> {
    offset: TimerDuration,
    value: T,
}

impl<T> EffectKeyframe<T> {
    /// Creates a keyframe at `offset` relative to clip start.
    #[must_use]
    pub const fn new(offset: TimerDuration, value: T) -> Self {
        Self { offset, value }
    }

    /// Returns the offset relative to clip start.
    #[must_use]
    pub const fn offset(&self) -> TimerDuration {
        self.offset
    }

    /// Returns the keyframe value.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }
}

/// Keyframed effect over one target value.
///
/// Effects can be authored as style-triggered feedback, such as a short
/// denied-action nudge, or started explicitly through
/// [`Ui::animate_target`](crate::Ui::animate_target). Keyframe offsets describe
/// one effect iteration; [`AnimationTiming`] controls playback duration, delay,
/// fill, direction, iteration count, and easing.
///
/// Use `EffectClip<T>` with an [`AnimationTarget<T>`] when the value type is
/// known. Plain `EffectClip` is the lower-level form whose keyframes contain
/// [`PresentationValue`] directly.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectClip<T = PresentationValue> {
    keyframes: Vec<EffectKeyframe<T>>,
    timing: AnimationTiming,
    composite: CompositeOperation,
}

impl<T> EffectClip<T> {
    /// Builds a clip from keyframes.
    ///
    /// The first keyframe supplies the sample before its timestamp. The
    /// default timing uses the last keyframe offset as one linear iteration and
    /// fills forwards, matching the original finite feedback behavior. Use
    /// [`Self::with_timing`] to repeat, delay, reverse, or retime the same
    /// keyframe curve.
    #[must_use]
    pub fn from_keyframes(keyframes: impl IntoIterator<Item = EffectKeyframe<T>>) -> Self {
        let mut keyframes: Vec<_> = keyframes.into_iter().collect();
        keyframes.sort_by_key(|keyframe| keyframe.offset);
        assert!(
            !keyframes.is_empty(),
            "effect clips require at least one keyframe"
        );
        let duration = keyframe_duration(&keyframes);
        Self {
            keyframes,
            timing: AnimationTiming {
                duration,
                fill: FillMode::Forwards,
                ..AnimationTiming::new(duration)
            },
            composite: CompositeOperation::Replace,
        }
    }

    /// Sets the playback timing for this clip.
    #[must_use]
    pub fn with_timing(mut self, timing: AnimationTiming) -> Self {
        self.timing = timing;
        self
    }

    /// Returns the playback timing for this clip.
    #[must_use]
    pub const fn timing(&self) -> AnimationTiming {
        self.timing
    }

    /// Sets how this clip combines with the value beneath it in the target
    /// stack.
    ///
    /// Unsupported additive or accumulative combinations are ignored by the
    /// stack sampler, matching [`understory_animation`] composition semantics.
    #[must_use]
    pub fn with_composite(mut self, composite: CompositeOperation) -> Self {
        self.composite = composite;
        self
    }

    /// Returns how this clip combines with the value beneath it.
    #[must_use]
    pub const fn composite(&self) -> CompositeOperation {
        self.composite
    }

    /// Returns the keyframes in sorted order.
    #[must_use]
    pub fn keyframes(&self) -> &[EffectKeyframe<T>] {
        &self.keyframes
    }

    pub(crate) fn into_presentation_values(self) -> EffectClip
    where
        T: PresentationAnimationValue,
    {
        EffectClip::from_keyframes(self.keyframes.into_iter().map(|keyframe| {
            EffectKeyframe::new(keyframe.offset, keyframe.value.into_presentation_value())
        }))
        .with_timing(self.timing)
        .with_composite(self.composite)
    }

    pub(crate) fn into_stack_effect(self) -> StackEffect<T>
    where
        T: Animatable,
    {
        let duration = keyframe_duration(&self.keyframes);
        let keyframes = self
            .keyframes
            .into_iter()
            .map(|keyframe| {
                Keyframe::new(
                    normalized_keyframe_offset(keyframe.offset, duration),
                    keyframe.value,
                )
            })
            .collect();
        StackEffect::new(
            KeyframeEffect::new(keyframes).with_composite(self.composite),
            self.timing,
        )
    }

    pub(crate) fn keyframe_duration(&self) -> TimerDuration {
        keyframe_duration(&self.keyframes)
    }
}

impl EffectClip<PresentationValue> {
    pub(crate) fn value_kind(&self) -> PresentationValueKind {
        let kind = self.keyframes[0].value.kind();
        debug_assert!(
            self.keyframes
                .iter()
                .all(|keyframe| keyframe.value.kind() == kind),
            "presentation-value effect keyframes must use one value kind"
        );
        kind
    }
}

/// One style-authored effect rule.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectRule {
    trigger: EffectTriggerId,
    target: PresentationTarget,
    clip: EffectClip,
}

impl EffectRule {
    /// Creates an effect rule for `trigger`, `target`, and `clip`.
    ///
    /// Prefer [`Self::for_target`] or [`Self::property`] when the target value
    /// type is known. This constructor is for lower-level code that already has
    /// keyframes in [`PresentationValue`] form.
    #[must_use]
    pub fn new(trigger: EffectTriggerId, target: PresentationTarget, clip: EffectClip) -> Self {
        Self {
            trigger,
            target,
            clip,
        }
    }

    /// Creates an effect rule for an animation target with a concrete value type.
    ///
    /// This is the preferred constructor when the effect target's value type is
    /// known at the call site. The clip values are authored as `T`, then
    /// lowered once for storage in the style rule.
    #[must_use]
    pub fn for_target<T>(
        trigger: EffectTriggerId,
        target: AnimationTarget<T>,
        clip: EffectClip<T>,
    ) -> Self
    where
        T: PresentationAnimationValue,
    {
        Self::new(
            trigger,
            target.presentation_target(),
            clip.into_presentation_values(),
        )
    }

    /// Creates an effect rule for a dependency property with a concrete value type.
    ///
    /// This convenience constructor is equivalent to passing
    /// [`AnimationTarget::property`] to [`Self::for_target`].
    #[must_use]
    pub fn property<T>(trigger: EffectTriggerId, property: Property<T>, clip: EffectClip<T>) -> Self
    where
        T: PresentationAnimationValue,
    {
        Self::for_target(trigger, AnimationTarget::property(property), clip)
    }

    /// Returns the trigger this rule responds to.
    #[must_use]
    pub const fn trigger(&self) -> EffectTriggerId {
        self.trigger
    }

    /// Returns the target this rule samples.
    #[must_use]
    pub const fn target(&self) -> PresentationTarget {
        self.target
    }

    /// Returns the effect clip.
    #[must_use]
    pub const fn clip(&self) -> &EffectClip {
        &self.clip
    }
}

/// Target-local style value containing effect rules for one element.
///
/// The cascade can attach different rule lists to different elements with
/// ordinary selectors. The trigger source is deliberately not modeled here:
/// controls, routed event handlers, or future effect-cascade machinery choose
/// which target element receives a trigger.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EffectRules {
    rules: Vec<EffectRule>,
}

impl EffectRules {
    /// Creates an empty effect rule list.
    #[must_use]
    pub const fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Creates an effect rule list containing `rule`.
    #[must_use]
    pub fn single(rule: EffectRule) -> Self {
        Self { rules: vec![rule] }
    }

    /// Appends `rule` and returns the updated list.
    #[must_use]
    pub fn with_rule(mut self, rule: EffectRule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Appends `rule`.
    pub fn push(&mut self, rule: EffectRule) {
        self.rules.push(rule);
    }

    /// Returns all effect rules in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &EffectRule> {
        self.rules.iter()
    }

    /// Returns rules that respond to `trigger`, preserving insertion order.
    pub fn rules_for(&self, trigger: EffectTriggerId) -> impl Iterator<Item = &EffectRule> {
        self.rules
            .iter()
            .filter(move |rule| rule.trigger == trigger)
    }

    /// Returns whether this rule list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

impl From<EffectRule> for EffectRules {
    fn from(rule: EffectRule) -> Self {
        Self::single(rule)
    }
}

fn keyframe_duration<T>(keyframes: &[EffectKeyframe<T>]) -> TimerDuration {
    keyframes
        .last()
        .map(|keyframe| keyframe.offset)
        .unwrap_or(0)
}

fn normalized_keyframe_offset(offset: TimerDuration, duration: TimerDuration) -> f64 {
    if duration == 0 {
        0.0
    } else {
        offset as f64 / duration as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use peniko::InterpolationAlphaSpace;
    use peniko::color::{ColorSpaceTag, DynamicColor, HueDirection, Srgb};
    use understory_motion::{ColorInterpolation, ColorTransition};

    #[test]
    fn timing_function_samples_expected_shape() {
        let slow_start = TimingFunction::cubic_bezier(0.55, 0.055, 0.675, 0.19);
        let slow_finish = TimingFunction::cubic_bezier(0.215, 0.61, 0.355, 1.0);

        assert_eq!(TimingFunction::LINEAR.sample(0.5), 0.5);
        assert!(slow_start.sample(0.5) < 0.5);
        assert!(slow_finish.sample(0.5) > 0.5);
    }

    #[test]
    fn typed_effect_clip_keeps_domain_values_until_erased() {
        #[derive(Clone, Debug, PartialEq)]
        struct DomainValue {
            label: String,
        }

        let clip = EffectClip::from_keyframes([
            EffectKeyframe::new(
                100,
                DomainValue {
                    label: String::from("end"),
                },
            ),
            EffectKeyframe::new(
                0,
                DomainValue {
                    label: String::from("start"),
                },
            ),
        ]);

        assert_eq!(clip.timing().duration, 100);
        assert_eq!(clip.keyframes()[0].offset(), 0);
        assert_eq!(clip.keyframes()[0].value().label, "start");
        assert_eq!(clip.keyframes()[1].offset(), 100);
        assert_eq!(clip.keyframes()[1].value().label, "end");
    }

    #[test]
    fn transition_samples_value_with_timing_function() {
        let transition = Transition::new(
            0.0_f32,
            10.0,
            10,
            100,
            TimingFunction::cubic_bezier(0.215, 0.61, 0.355, 1.0),
        );

        assert_eq!(transition.sample(10), 0.0);
        assert!(transition.sample(60) > 5.0);
        assert_eq!(transition.sample(110), 10.0);
        assert!(transition.is_complete(110));
        assert_eq!(transition.end_time(), 110);
    }

    #[test]
    fn normalized_progress_clamps_to_unit_interval() {
        assert_eq!(normalized_progress(0, 10, 100), 0.0);
        assert_eq!(normalized_progress(10, 10, 0), 1.0);
        assert_eq!(normalized_progress(110, 10, 100), 1.0);
    }

    #[test]
    fn color_transition_uses_color_interpolation_policy() {
        let from = Color::from_rgba8(0xff, 0x00, 0x00, 0xff);
        let to = Color::from_rgba8(0x00, 0xff, 0x00, 0xff);
        let transition = ColorTransition::new(
            from,
            to,
            0,
            100,
            TimingFunction::LINEAR,
            ColorInterpolation::new(
                ColorSpaceTag::Hsl,
                HueDirection::Increasing,
                InterpolationAlphaSpace::Premultiplied,
            ),
        );
        let expected = DynamicColor::from_alpha_color(from)
            .interpolate(
                DynamicColor::from_alpha_color(to),
                ColorSpaceTag::Hsl,
                HueDirection::Increasing,
            )
            .eval(0.5)
            .to_alpha_color::<Srgb>();

        assert_eq!(transition.sample(50), expected);
    }
}
