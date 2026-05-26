// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Renderer-neutral motion primitives.
//!
//! This module owns the reusable pieces of time-sampled value motion:
//! interpolation, easing, and a single-value transition clip. It deliberately
//! does not know about elements, dependency properties, style, invalidation,
//! layout, or composition backends.

use kurbo::Vec2;
use peniko::Color;
use understory_timing::{TimerDuration, TimerInstant};

/// Value that can be interpolated between two endpoints.
pub trait Interpolate: Copy {
    /// Returns the value at normalized progress `t` between `self` and `to`.
    ///
    /// Implementations may assume callers pass values in the inclusive
    /// `0.0..=1.0` range.
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

impl Interpolate for Color {
    fn interpolate(self, to: Self, t: f32) -> Self {
        self.lerp_rect(to, t)
    }
}

impl Animatable for Color {
    fn is_close(self, other: Self) -> bool {
        self.components
            .into_iter()
            .zip(other.components)
            .all(|(a, b)| a.is_close(b))
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

/// Timing curve for normalized transition progress.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Easing {
    /// Constant progress rate.
    #[default]
    Linear,
    /// Slow start, fast finish.
    EaseInCubic,
    /// Fast start, slow finish.
    EaseOutCubic,
    /// Slow start and finish with faster middle motion.
    EaseInOutCubic,
}

impl Easing {
    /// Samples the easing curve at normalized progress `t`.
    #[must_use]
    pub fn sample(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseInCubic => t * t * t,
            Self::EaseOutCubic => {
                let p = 1.0 - t;
                1.0 - (p * p * p)
            }
            Self::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    let p = -2.0 * t + 2.0;
                    1.0 - (p * p * p / 2.0)
                }
            }
        }
    }
}

/// A single-value transition clip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition<T: Animatable> {
    from: T,
    to: T,
    started_at: TimerInstant,
    duration: TimerDuration,
    easing: Easing,
}

impl<T: Animatable> Transition<T> {
    /// Creates a transition from `from` to `to`.
    #[must_use]
    pub const fn new(
        from: T,
        to: T,
        started_at: TimerInstant,
        duration: TimerDuration,
        easing: Easing,
    ) -> Self {
        Self {
            from,
            to,
            started_at,
            duration,
            easing,
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

    /// Returns the easing curve.
    #[must_use]
    pub const fn easing(&self) -> Easing {
        self.easing
    }

    /// Samples this transition at `now`.
    #[must_use]
    pub fn sample(self, now: TimerInstant) -> T {
        let t = self
            .easing
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
    fn cubic_easing_samples_expected_shape() {
        assert_eq!(Easing::Linear.sample(0.5), 0.5);
        assert!(Easing::EaseInCubic.sample(0.5) < 0.5);
        assert!(Easing::EaseOutCubic.sample(0.5) > 0.5);
        assert_eq!(Easing::EaseInOutCubic.sample(0.5), 0.5);
    }

    #[test]
    fn transition_samples_value_with_easing() {
        let transition = Transition::new(0.0_f32, 10.0, 10, 100, Easing::EaseOutCubic);

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
}
