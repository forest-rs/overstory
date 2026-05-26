// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Renderer-neutral motion primitives.
//!
//! This module owns the reusable pieces of time-sampled value motion:
//! interpolation, timing functions, and single-value transition clips. It
//! deliberately does not know about elements, dependency properties, style,
//! invalidation, layout, or composition backends.

use core::fmt;

use kurbo::Vec2;
use peniko::color::{self, ColorSpaceTag, DynamicColor, HueDirection, Srgb};
use peniko::{Color, InterpolationAlphaSpace};
use understory_timing::{TimerDuration, TimerInstant};

/// Value that can be interpolated between two endpoints.
pub trait Interpolate: Copy {
    /// Returns the value at normalized progress `t` between `self` and `to`.
    ///
    /// Most transitions pass values in the inclusive `0.0..=1.0` range, but
    /// timing functions may intentionally overshoot for future motion styles.
    fn interpolate(self, to: Self, t: f32) -> Self;
}

/// Value suitable for timeline sampling.
pub trait Animatable: Interpolate + PartialEq {
    /// Returns whether two values are close enough to snap instead of animate.
    fn is_close(self, other: Self) -> bool;
}

impl Interpolate for f32 {
    fn interpolate(self, to: Self, t: f32) -> Self {
        self + ((to - self) * t)
    }
}

impl Animatable for f32 {
    fn is_close(self, other: Self) -> bool {
        (self - other).abs() <= Self::EPSILON
    }
}

impl Interpolate for f64 {
    fn interpolate(self, to: Self, t: f32) -> Self {
        self + ((to - self) * Self::from(t))
    }
}

impl Animatable for f64 {
    fn is_close(self, other: Self) -> bool {
        (self - other).abs() <= Self::EPSILON
    }
}

impl Interpolate for Vec2 {
    fn interpolate(self, to: Self, t: f32) -> Self {
        let t = f64::from(t);
        self + ((to - self) * t)
    }
}

impl Animatable for Vec2 {
    fn is_close(self, other: Self) -> bool {
        (self.x - other.x).abs() <= f64::EPSILON && (self.y - other.y).abs() <= f64::EPSILON
    }
}

/// Data-driven timing function for normalized transition progress.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimingFunction {
    kind: TimingFunctionKind,
}

impl TimingFunction {
    /// Constant progress rate.
    pub const LINEAR: Self = Self {
        kind: TimingFunctionKind::Linear,
    };

    /// Creates a cubic Bezier timing function from CSS-style control points.
    ///
    /// The curve always starts at `(0, 0)` and ends at `(1, 1)`. `x1` and `x2`
    /// should be finite values in `0.0..=1.0` so the curve can be inverted from
    /// input progress to output progress.
    #[must_use]
    pub const fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self {
            kind: TimingFunctionKind::CubicBezier(CubicBezierTimingFunction { x1, y1, x2, y2 }),
        }
    }

    /// Samples the timing function at normalized input progress `t`.
    #[must_use]
    pub fn sample(self, t: f32) -> f32 {
        match self.kind {
            TimingFunctionKind::Linear => t.clamp(0.0, 1.0),
            TimingFunctionKind::CubicBezier(curve) => curve.sample(t),
        }
    }
}

impl Default for TimingFunction {
    fn default() -> Self {
        Self::LINEAR
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum TimingFunctionKind {
    Linear,
    CubicBezier(CubicBezierTimingFunction),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CubicBezierTimingFunction {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
}

impl CubicBezierTimingFunction {
    fn sample(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        if t <= 0.0 || t >= 1.0 {
            return t;
        }
        cubic_value(self.solve_x(t), self.y1, self.y2)
    }

    fn solve_x(self, x: f32) -> f32 {
        const DERIVATIVE_EPSILON: f32 = 1.0e-6;
        const NEWTON_STEPS: usize = 8;
        const BISECTION_STEPS: usize = 12;

        let mut t = x;
        for _ in 0..NEWTON_STEPS {
            let error = cubic_value(t, self.x1, self.x2) - x;
            let derivative = cubic_derivative(t, self.x1, self.x2);
            if derivative.abs() <= DERIVATIVE_EPSILON {
                break;
            }
            let next = t - (error / derivative);
            if !(0.0..=1.0).contains(&next) {
                break;
            }
            t = next;
        }

        let mut low = 0.0;
        let mut high = 1.0;
        for _ in 0..BISECTION_STEPS {
            let value = cubic_value(t, self.x1, self.x2);
            if (value - x).abs() <= DERIVATIVE_EPSILON {
                return t;
            }
            if value < x {
                low = t;
            } else {
                high = t;
            }
            t = (low + high) * 0.5;
        }
        t
    }
}

fn cubic_value(t: f32, p1: f32, p2: f32) -> f32 {
    let inverse = 1.0 - t;
    (3.0 * inverse * inverse * t * p1) + (3.0 * inverse * t * t * p2) + (t * t * t)
}

fn cubic_derivative(t: f32, p1: f32, p2: f32) -> f32 {
    let inverse = 1.0 - t;
    (3.0 * inverse * inverse * p1) + (6.0 * inverse * t * (p2 - p1)) + (3.0 * t * t * (1.0 - p2))
}

/// Policy for interpolating colors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorInterpolation {
    color_space: ColorSpaceTag,
    hue_direction: HueDirection,
    alpha_space: InterpolationAlphaSpace,
}

impl ColorInterpolation {
    /// Interpolate in sRGB with premultiplied alpha.
    pub const SRGB: Self = Self::new(
        ColorSpaceTag::Srgb,
        HueDirection::Shorter,
        InterpolationAlphaSpace::Premultiplied,
    );

    /// Interpolate in Oklab with premultiplied alpha.
    pub const OKLAB: Self = Self::new(
        ColorSpaceTag::Oklab,
        HueDirection::Shorter,
        InterpolationAlphaSpace::Premultiplied,
    );

    /// Creates a color interpolation policy.
    #[must_use]
    pub const fn new(
        color_space: ColorSpaceTag,
        hue_direction: HueDirection,
        alpha_space: InterpolationAlphaSpace,
    ) -> Self {
        Self {
            color_space,
            hue_direction,
            alpha_space,
        }
    }

    /// Returns the color space used for interpolation.
    #[must_use]
    pub const fn color_space(self) -> ColorSpaceTag {
        self.color_space
    }

    /// Returns the hue direction used for cylindrical color spaces.
    #[must_use]
    pub const fn hue_direction(self) -> HueDirection {
        self.hue_direction
    }

    /// Returns how alpha participates in interpolation.
    #[must_use]
    pub const fn alpha_space(self) -> InterpolationAlphaSpace {
        self.alpha_space
    }
}

impl Default for ColorInterpolation {
    fn default() -> Self {
        Self::SRGB
    }
}

/// A single-value transition clip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition<T: Animatable> {
    from: T,
    to: T,
    started_at: TimerInstant,
    duration: TimerDuration,
    timing: TimingFunction,
}

impl<T: Animatable> Transition<T> {
    /// Creates a transition from `from` to `to`.
    #[must_use]
    pub const fn new(
        from: T,
        to: T,
        started_at: TimerInstant,
        duration: TimerDuration,
        timing: TimingFunction,
    ) -> Self {
        Self {
            from,
            to,
            started_at,
            duration,
            timing,
        }
    }

    /// Returns the transition start value.
    #[must_use]
    pub const fn from(&self) -> T {
        self.from
    }

    /// Returns the transition target value.
    #[must_use]
    pub const fn to(&self) -> T {
        self.to
    }

    /// Returns the transition start instant.
    #[must_use]
    pub const fn started_at(&self) -> TimerInstant {
        self.started_at
    }

    /// Returns the transition duration.
    #[must_use]
    pub const fn duration(&self) -> TimerDuration {
        self.duration
    }

    /// Returns the timing function.
    #[must_use]
    pub const fn timing(&self) -> TimingFunction {
        self.timing
    }

    /// Samples this transition at `now`.
    #[must_use]
    pub fn sample(self, now: TimerInstant) -> T {
        let t = self
            .timing
            .sample(normalized_progress(now, self.started_at, self.duration));
        self.from.interpolate(self.to, t)
    }

    /// Returns whether this transition has reached its target by `now`.
    #[must_use]
    pub fn is_complete(self, now: TimerInstant) -> bool {
        now.saturating_sub(self.started_at) >= self.duration
    }

    /// Returns the instant this transition reaches its target.
    #[must_use]
    pub fn end_time(self) -> TimerInstant {
        self.started_at.saturating_add(self.duration)
    }
}

/// A color transition clip with explicit interpolation policy.
#[derive(Clone, Copy)]
pub struct ColorTransition {
    from: Color,
    to: Color,
    started_at: TimerInstant,
    duration: TimerDuration,
    timing: TimingFunction,
    interpolation: ColorInterpolation,
    ramp: ColorRamp,
}

impl ColorTransition {
    /// Creates a color transition.
    #[must_use]
    pub fn new(
        from: Color,
        to: Color,
        started_at: TimerInstant,
        duration: TimerDuration,
        timing: TimingFunction,
        interpolation: ColorInterpolation,
    ) -> Self {
        Self {
            from,
            to,
            started_at,
            duration,
            timing,
            interpolation,
            ramp: ColorRamp::new(from, to, interpolation),
        }
    }

    /// Returns the transition start color.
    #[must_use]
    pub const fn from(&self) -> Color {
        self.from
    }

    /// Returns the transition target color.
    #[must_use]
    pub const fn to(&self) -> Color {
        self.to
    }

    /// Returns the transition start instant.
    #[must_use]
    pub const fn started_at(&self) -> TimerInstant {
        self.started_at
    }

    /// Returns the transition duration.
    #[must_use]
    pub const fn duration(&self) -> TimerDuration {
        self.duration
    }

    /// Returns the timing function.
    #[must_use]
    pub const fn timing(&self) -> TimingFunction {
        self.timing
    }

    /// Returns the color interpolation policy.
    #[must_use]
    pub const fn interpolation(&self) -> ColorInterpolation {
        self.interpolation
    }

    /// Samples this color transition at `now`.
    #[must_use]
    pub fn sample(self, now: TimerInstant) -> Color {
        self.ramp.sample(self.timing.sample(normalized_progress(
            now,
            self.started_at,
            self.duration,
        )))
    }

    /// Returns whether this transition has reached its target by `now`.
    #[must_use]
    pub fn is_complete(self, now: TimerInstant) -> bool {
        now.saturating_sub(self.started_at) >= self.duration
    }

    /// Returns the instant this transition reaches its target.
    #[must_use]
    pub fn end_time(self) -> TimerInstant {
        self.started_at.saturating_add(self.duration)
    }
}

impl fmt::Debug for ColorTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ColorTransition")
            .field("from", &self.from)
            .field("to", &self.to)
            .field("started_at", &self.started_at)
            .field("duration", &self.duration)
            .field("timing", &self.timing)
            .field("interpolation", &self.interpolation)
            .finish_non_exhaustive()
    }
}

impl PartialEq for ColorTransition {
    fn eq(&self, other: &Self) -> bool {
        self.from == other.from
            && self.to == other.to
            && self.started_at == other.started_at
            && self.duration == other.duration
            && self.timing == other.timing
            && self.interpolation == other.interpolation
    }
}

#[derive(Clone, Copy)]
enum ColorRamp {
    Premultiplied(color::Interpolator),
    Unpremultiplied(color::UnpremultipliedInterpolator),
}

impl ColorRamp {
    fn new(from: Color, to: Color, interpolation: ColorInterpolation) -> Self {
        let from = DynamicColor::from_alpha_color(from);
        let to = DynamicColor::from_alpha_color(to);
        match interpolation.alpha_space() {
            InterpolationAlphaSpace::Premultiplied => Self::Premultiplied(from.interpolate(
                to,
                interpolation.color_space(),
                interpolation.hue_direction(),
            )),
            InterpolationAlphaSpace::Unpremultiplied => {
                Self::Unpremultiplied(from.interpolate_unpremultiplied(
                    to,
                    interpolation.color_space(),
                    interpolation.hue_direction(),
                ))
            }
        }
    }

    fn sample(self, t: f32) -> Color {
        match self {
            Self::Premultiplied(ramp) => ramp.eval(t),
            Self::Unpremultiplied(ramp) => ramp.eval(t),
        }
        .to_alpha_color::<Srgb>()
    }
}

/// Returns normalized progress through a duration.
#[must_use]
pub fn normalized_progress(
    now: TimerInstant,
    started_at: TimerInstant,
    duration: TimerDuration,
) -> f32 {
    if duration == 0 {
        return 1.0;
    }
    let elapsed = now.saturating_sub(started_at);
    if elapsed >= duration {
        return 1.0;
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "transition progress is a bounded f32 sample"
    )]
    {
        elapsed as f32 / duration as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_function_samples_expected_shape() {
        let slow_start = TimingFunction::cubic_bezier(0.55, 0.055, 0.675, 0.19);
        let slow_finish = TimingFunction::cubic_bezier(0.215, 0.61, 0.355, 1.0);

        assert_eq!(TimingFunction::LINEAR.sample(0.5), 0.5);
        assert!(slow_start.sample(0.5) < 0.5);
        assert!(slow_finish.sample(0.5) > 0.5);
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
