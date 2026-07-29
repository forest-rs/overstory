// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Typed motion authoring for styles and ordinary retained mutations.
//!
//! This module owns target-independent transition intent and the typed bridge
//! from dependency properties to presentation targets. It explicitly does not
//! own clocks, sampling, active tracks, or retained playback control.

use alloc::vec::Vec;

use peniko::color::{ColorSpaceTag, HueDirection};
use peniko::{Brush, Color, InterpolationAlphaSpace};
use understory_motion::TimingFunction;
use understory_property::Property;
use understory_style::ResourceKey;
use understory_timing::TimerDuration;

use super::{Animatable, AnimationTarget, PresentationTarget};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MotionValue<T> {
    Literal(T),
    Resource(ResourceKey),
}

/// Target-independent timing and interpolation policy for a transition.
///
/// The same specification can be attached to style-authored property
/// transitions or supplied to [`Ui::with_motion`](crate::Ui::with_motion) for
/// an ordinary retained mutation. Theme resource references are resolved when
/// the transition starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionSpec {
    pub(crate) duration: MotionValue<TimerDuration>,
    pub(crate) delay: MotionValue<TimerDuration>,
    pub(crate) easing: MotionValue<TimingFunction>,
    pub(crate) color_space: MotionValue<ColorSpaceTag>,
    pub(crate) hue_direction: MotionValue<HueDirection>,
    pub(crate) alpha_space: MotionValue<InterpolationAlphaSpace>,
}

impl MotionSpec {
    /// Creates a linear transition lasting `duration`.
    #[must_use]
    pub const fn new(duration: TimerDuration) -> Self {
        Self {
            duration: MotionValue::Literal(duration),
            delay: MotionValue::Literal(0),
            easing: MotionValue::Literal(TimingFunction::LINEAR),
            color_space: MotionValue::Literal(ColorSpaceTag::Srgb),
            hue_direction: MotionValue::Literal(HueDirection::Shorter),
            alpha_space: MotionValue::Literal(InterpolationAlphaSpace::Premultiplied),
        }
    }

    /// Creates a transition whose duration comes from a theme resource.
    ///
    /// A missing resource resolves to zero duration, matching style-authored
    /// transition resource semantics.
    #[must_use]
    pub const fn from_duration_resource(resource: ResourceKey) -> Self {
        Self {
            duration: MotionValue::Resource(resource),
            delay: MotionValue::Literal(0),
            easing: MotionValue::Literal(TimingFunction::LINEAR),
            color_space: MotionValue::Literal(ColorSpaceTag::Srgb),
            hue_direction: MotionValue::Literal(HueDirection::Shorter),
            alpha_space: MotionValue::Literal(InterpolationAlphaSpace::Premultiplied),
        }
    }

    /// Resolves the duration from a theme resource.
    #[must_use]
    pub const fn with_duration_resource(mut self, resource: ResourceKey) -> Self {
        self.duration = MotionValue::Resource(resource);
        self
    }

    /// Delays the transition by `delay`.
    #[must_use]
    pub const fn with_delay(mut self, delay: TimerDuration) -> Self {
        self.delay = MotionValue::Literal(delay);
        self
    }

    /// Resolves the delay from a theme resource.
    #[must_use]
    pub const fn with_delay_resource(mut self, resource: ResourceKey) -> Self {
        self.delay = MotionValue::Resource(resource);
        self
    }

    /// Applies `easing` to transition progress.
    #[must_use]
    pub const fn with_easing(mut self, easing: TimingFunction) -> Self {
        self.easing = MotionValue::Literal(easing);
        self
    }

    /// Resolves the easing function from a theme resource.
    #[must_use]
    pub const fn with_easing_resource(mut self, resource: ResourceKey) -> Self {
        self.easing = MotionValue::Resource(resource);
        self
    }

    /// Selects the complete interpolation method for color-valued targets.
    ///
    /// The arguments are the native `color`/Peniko vocabulary used by the
    /// renderer and gradient system. Non-color targets ignore these fields.
    #[must_use]
    pub const fn with_color_interpolation(
        mut self,
        color_space: ColorSpaceTag,
        hue_direction: HueDirection,
        alpha_space: InterpolationAlphaSpace,
    ) -> Self {
        self.color_space = MotionValue::Literal(color_space);
        self.hue_direction = MotionValue::Literal(hue_direction);
        self.alpha_space = MotionValue::Literal(alpha_space);
        self
    }

    /// Selects the color space used for color-valued targets.
    ///
    /// Hue direction and alpha interpolation retain their current values.
    #[must_use]
    pub const fn with_color_space(mut self, color_space: ColorSpaceTag) -> Self {
        self.color_space = MotionValue::Literal(color_space);
        self
    }

    /// Resolves the color space from a theme resource.
    #[must_use]
    pub const fn with_color_space_resource(mut self, resource: ResourceKey) -> Self {
        self.color_space = MotionValue::Resource(resource);
        self
    }

    /// Selects how hue angles travel in cylindrical color spaces.
    #[must_use]
    pub const fn with_hue_direction(mut self, direction: HueDirection) -> Self {
        self.hue_direction = MotionValue::Literal(direction);
        self
    }

    /// Resolves hue direction from a theme resource.
    #[must_use]
    pub const fn with_hue_direction_resource(mut self, resource: ResourceKey) -> Self {
        self.hue_direction = MotionValue::Resource(resource);
        self
    }

    /// Selects premultiplied or unpremultiplied alpha interpolation.
    #[must_use]
    pub const fn with_alpha_space(mut self, alpha_space: InterpolationAlphaSpace) -> Self {
        self.alpha_space = MotionValue::Literal(alpha_space);
        self
    }

    /// Resolves alpha interpolation from a theme resource.
    #[must_use]
    pub const fn with_alpha_space_resource(mut self, resource: ResourceKey) -> Self {
        self.alpha_space = MotionValue::Resource(resource);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AuthoredTransition {
    target: PresentationTarget,
    motion: MotionSpec,
}

impl AuthoredTransition {
    const fn new(target: PresentationTarget, motion: MotionSpec) -> Self {
        Self { target, motion }
    }

    pub(crate) const fn target(self) -> PresentationTarget {
        self.target
    }

    pub(crate) const fn motion(self) -> MotionSpec {
        self.motion
    }
}

/// Typed style-authored transitions for one element.
///
/// The winning value replaces the complete set, matching CSS transition
/// declarations. Repeating a target is permitted; the last declaration wins.
#[derive(Clone, Debug, PartialEq)]
pub struct Transitions {
    declarations: Vec<AuthoredTransition>,
}

impl Default for Transitions {
    fn default() -> Self {
        Self::new()
    }
}

impl Transitions {
    /// Creates an empty transition set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            declarations: Vec::new(),
        }
    }

    /// Adds a transition for a property whose stored and animated types match.
    /// Custom properties must also be registered through
    /// [`Ui::register_animation_target_property`](crate::Ui::register_animation_target_property)
    /// before they can receive timeline samples.
    ///
    /// Non-interpolatable properties are rejected at compile time:
    ///
    /// ```compile_fail
    /// use overstory::{MotionSpec, Transitions, Ui, millis};
    ///
    /// let ui = Ui::new();
    /// let _ = Transitions::new().property(
    ///     ui.properties().is_checked,
    ///     MotionSpec::new(millis(120)),
    /// );
    /// ```
    #[must_use]
    pub fn property<T>(mut self, property: Property<T>, motion: MotionSpec) -> Self
    where
        T: Animatable,
    {
        self.declarations.push(AuthoredTransition::new(
            PresentationTarget::property(property.id()),
            motion,
        ));
        self
    }

    /// Adds a color transition for a supported color-presenting property.
    ///
    /// Built-in color-presenting properties are registered automatically.
    /// Register custom properties once through
    /// [`Ui::register_color_animation_target_property`](crate::Ui::register_color_animation_target_property).
    /// Using an unregistered property fails at the transition boundary rather
    /// than silently snapping.
    #[must_use]
    pub fn color(mut self, property: impl ColorTransitionProperty, motion: MotionSpec) -> Self {
        self.declarations.push(AuthoredTransition::new(
            property.presentation_target(),
            motion,
        ));
        self
    }

    /// Adds a transition for a typed presentation target.
    #[must_use]
    pub fn target<T>(mut self, target: AnimationTarget<T>, motion: MotionSpec) -> Self
    where
        T: Animatable,
    {
        self.declarations.push(AuthoredTransition::new(
            target.presentation_target(),
            motion,
        ));
        self
    }

    /// Returns the number of authored declarations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.declarations.len()
    }

    /// Returns whether no transitions are declared.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }

    pub(crate) fn find(&self, target: &PresentationTarget) -> Option<&AuthoredTransition> {
        self.declarations
            .iter()
            .rev()
            .find(|declaration| declaration.target == *target)
    }

    pub(crate) fn declarations(&self) -> impl Iterator<Item = &AuthoredTransition> {
        self.declarations.iter()
    }
}

mod private {
    #[expect(
        unnameable_types,
        reason = "the private supertrait deliberately seals color transition targets"
    )]
    pub trait Sealed {}
}

/// Property handle accepted by [`Transitions::color`].
///
/// The trait is sealed because each accepted storage representation requires a
/// specific presentation adapter. Built-in properties install theirs during
/// [`Ui`](crate::Ui) construction; custom properties must be registered once.
pub trait ColorTransitionProperty: private::Sealed + Copy {
    /// Returns the erased presentation target used by the transition runtime.
    #[doc(hidden)]
    fn presentation_target(self) -> PresentationTarget;

    /// Installs this property's color presentation adapter.
    #[doc(hidden)]
    fn register_adapter(self, ui: &mut crate::Ui);
}

impl private::Sealed for Property<Color> {}
impl private::Sealed for Property<Brush> {}
impl private::Sealed for Property<Option<Brush>> {}

impl ColorTransitionProperty for Property<Color> {
    fn presentation_target(self) -> PresentationTarget {
        PresentationTarget::property(self.id())
    }

    fn register_adapter(self, ui: &mut crate::Ui) {
        ui.register_animation_target_property(self);
    }
}

impl ColorTransitionProperty for Property<Brush> {
    fn presentation_target(self) -> PresentationTarget {
        PresentationTarget::property(self.id())
    }

    fn register_adapter(self, ui: &mut crate::Ui) {
        ui.register_brush_color_animation_target(self);
    }
}

impl ColorTransitionProperty for Property<Option<Brush>> {
    fn presentation_target(self) -> PresentationTarget {
        PresentationTarget::property(self.id())
    }

    fn register_adapter(self, ui: &mut crate::Ui) {
        ui.register_optional_brush_color_animation_target(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use understory_property::{PropertyMetadataBuilder, PropertyRegistry};

    #[test]
    fn typed_properties_become_presentation_targets() {
        let mut registry = PropertyRegistry::new();
        let opacity = registry.register("Opacity", PropertyMetadataBuilder::new(1.0_f32).build());
        let foreground = registry.register(
            "Foreground",
            PropertyMetadataBuilder::new(Color::BLACK).build(),
        );

        let transitions = Transitions::new()
            .property(opacity, MotionSpec::new(90))
            .color(foreground, MotionSpec::new(140));
        let targets: Vec<_> = transitions
            .declarations()
            .map(|declaration| declaration.target())
            .collect();

        assert_eq!(
            targets,
            [
                PresentationTarget::property(opacity.id()),
                PresentationTarget::property(foreground.id()),
            ]
        );
    }

    #[test]
    fn motion_resources_remain_native_until_transition_resolution() {
        const DURATION: ResourceKey = ResourceKey::new(7);
        const EASING: ResourceKey = ResourceKey::new(8);
        const COLOR_SPACE: ResourceKey = ResourceKey::new(9);
        const HUE_DIRECTION: ResourceKey = ResourceKey::new(10);
        const ALPHA_SPACE: ResourceKey = ResourceKey::new(11);

        let mut registry = PropertyRegistry::new();
        let foreground = registry.register(
            "Foreground",
            PropertyMetadataBuilder::new(Color::BLACK).build(),
        );
        let transitions = Transitions::new().color(
            foreground,
            MotionSpec::from_duration_resource(DURATION)
                .with_easing_resource(EASING)
                .with_color_space_resource(COLOR_SPACE)
                .with_hue_direction_resource(HUE_DIRECTION)
                .with_alpha_space_resource(ALPHA_SPACE),
        );
        let declaration = transitions.declarations().next().expect("one declaration");
        let motion = declaration.motion();

        assert_eq!(motion.duration, MotionValue::Resource(DURATION));
        assert_eq!(motion.easing, MotionValue::Resource(EASING));
        assert_eq!(motion.color_space, MotionValue::Resource(COLOR_SPACE));
        assert_eq!(motion.hue_direction, MotionValue::Resource(HUE_DIRECTION));
        assert_eq!(motion.alpha_space, MotionValue::Resource(ALPHA_SPACE));
    }
}
