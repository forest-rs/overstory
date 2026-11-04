// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Application-owned values for the inline surface study.

use kurbo::Rect;

#[derive(Clone, Copy, Debug)]
pub(super) enum Parameter {
    Columns,
    Spacing,
    Radius,
}

impl Parameter {
    pub(super) const ALL: [Self; 3] = [Self::Columns, Self::Spacing, Self::Radius];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Columns => "Columns",
            Self::Spacing => "Spacing",
            Self::Radius => "Radius",
        }
    }

    pub(super) fn range(self) -> (f64, f64) {
        match self {
            Self::Columns => (2., 6.),
            Self::Spacing => (4., 24.),
            Self::Radius => (0., 24.),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SurfaceParameters {
    columns: f64,
    spacing: f64,
    radius: f64,
}

impl Default for SurfaceParameters {
    fn default() -> Self {
        Self {
            columns: 4.,
            spacing: 12.,
            radius: 10.,
        }
    }
}

impl SurfaceParameters {
    pub(super) fn value(self, parameter: Parameter) -> f64 {
        match parameter {
            Parameter::Columns => self.columns,
            Parameter::Spacing => self.spacing,
            Parameter::Radius => self.radius,
        }
    }

    pub(super) fn set(&mut self, parameter: Parameter, value: f64) -> bool {
        if !value.is_finite() {
            return false;
        }
        let (min, max) = parameter.range();
        let value = value.round().clamp(min, max);
        let current = match parameter {
            Parameter::Columns => &mut self.columns,
            Parameter::Spacing => &mut self.spacing,
            Parameter::Radius => &mut self.radius,
        };
        if *current == value {
            return false;
        }
        *current = value;
        true
    }

    /// Twelve cards in a small, application-owned layout. This is not graph execution.
    pub(super) fn preview(self, bounds: Rect) -> Vec<(Rect, f64)> {
        // Defaults and set() keep this integral and within 2..=6.
        #[allow(
            clippy::cast_possible_truncation,
            reason = "columns are integral and bounded to 2..=6"
        )]
        let columns = self.columns as usize;
        let rows = 12_usize.div_ceil(columns);
        let gap = self.spacing * 0.3;
        let width = ((bounds.width() - gap * (columns - 1) as f64) / columns as f64).max(1.);
        let height = ((bounds.height() - gap * (rows - 1) as f64) / rows as f64).max(1.);
        (0..12)
            .map(|index| {
                let x = bounds.x0 + (index % columns) as f64 * (width + gap);
                let y = bounds.y0 + (index / columns) as f64 * (height + gap);
                (
                    Rect::new(x, y, x + width, y + height),
                    (self.radius * 0.45).min(width.min(height) * 0.5),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_parameters_change_the_preview_with_bounded_values() {
        let mut values = SurfaceParameters::default();
        let bounds = Rect::new(0., 0., 220., 120.);
        let before = values.preview(bounds);
        assert!(!values.set(Parameter::Columns, f64::NAN));
        assert!(values.set(Parameter::Columns, 999.));
        assert_eq!(values.value(Parameter::Columns), 6.);
        assert_ne!(values.preview(bounds), before);
        values.set(Parameter::Spacing, 24.);
        values.set(Parameter::Radius, 24.);
        for (rect, radius) in values.preview(bounds) {
            assert!(bounds.contains_rect(rect));
            assert!(radius <= rect.width().min(rect.height()) * 0.5);
        }
    }
}
