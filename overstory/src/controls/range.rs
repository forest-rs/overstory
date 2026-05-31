// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared numeric range math for controls that expose a scalar value.

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ScalarRange {
    min: f64,
    max: f64,
}

impl ScalarRange {
    pub(crate) const fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    pub(crate) fn clamp(self, value: f64) -> f64 {
        value.clamp(self.lower(), self.upper())
    }

    pub(crate) fn fraction(self, value: f64) -> f64 {
        let span = self.max - self.min;
        if !span.is_finite() || span.abs() <= f64::EPSILON {
            return 0.0;
        }
        ((value - self.min) / span).clamp(0.0, 1.0)
    }

    pub(crate) fn value_at_fraction(self, fraction: f64) -> f64 {
        self.min + fraction.clamp(0.0, 1.0) * (self.max - self.min)
    }

    pub(crate) fn change_or_fraction(self, configured: f64, default_fraction: f64) -> f64 {
        if configured.is_finite() && configured > 0.0 {
            configured
        } else {
            self.span().abs() * default_fraction
        }
    }

    pub(crate) fn positive_change_or_fraction(
        self,
        configured: f64,
        default_fraction: f64,
        minimum: f64,
    ) -> f64 {
        self.change_or_fraction(configured, default_fraction)
            .max(minimum)
    }

    fn span(self) -> f64 {
        self.max - self.min
    }

    fn lower(self) -> f64 {
        self.min.min(self.max)
    }

    fn upper(self) -> f64 {
        self.min.max(self.max)
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "unit fractions fit f32 precisely enough for layout percentages"
)]
pub(crate) fn f32_from_unit_fraction(fraction: f64) -> f32 {
    fraction.clamp(0.0, 1.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_range_clamps_with_reversed_bounds() {
        let range = ScalarRange::new(10.0, 0.0);

        assert_eq!(range.clamp(-1.0), 0.0);
        assert_eq!(range.clamp(11.0), 10.0);
    }

    #[test]
    fn scalar_range_fraction_preserves_direction() {
        let range = ScalarRange::new(10.0, 0.0);

        assert_eq!(range.fraction(10.0), 0.0);
        assert_eq!(range.fraction(5.0), 0.5);
        assert_eq!(range.fraction(0.0), 1.0);
    }
}
