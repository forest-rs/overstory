// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Application-owned dimensions for a twelve-module field instrument case.

use kurbo::{Point, Rect, Size};

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

    pub(super) fn color(self) -> peniko::Color {
        match self {
            Self::Columns => peniko::Color::from_rgba8(0x75, 0xdc, 0xb3, 0xff),
            Self::Spacing => peniko::Color::from_rgba8(0x7b, 0xbe, 0xf3, 0xff),
            Self::Radius => peniko::Color::from_rgba8(0xc4, 0xa3, 0xef, 0xff),
        }
    }

    pub(super) fn range(self) -> (f64, f64) {
        match self {
            Self::Columns => (2., 6.),
            Self::Spacing => (4., 24.),
            Self::Radius => (0., 16.),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CaseParameters {
    columns: f64,
    spacing: f64,
    radius: f64,
}

impl Default for CaseParameters {
    fn default() -> Self {
        Self {
            columns: 4.,
            spacing: 12.,
            radius: 10.,
        }
    }
}

impl CaseParameters {
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

    pub(super) const CASE: Size = Size::new(300., 240.);
    pub(super) const MODULE: Size = Size::new(44., 32.);

    pub(super) fn columns(self) -> usize {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "columns are integral and bounded to 2..=6"
        )]
        {
            self.columns as usize
        }
    }
    pub(super) fn rows(self) -> usize {
        12_usize.div_ceil(self.columns())
    }
    pub(super) fn footprint(self) -> Size {
        Size::new(
            self.columns * Self::MODULE.width + (self.columns - 1.) * self.spacing,
            self.rows() as f64 * Self::MODULE.height + (self.rows() - 1) as f64 * self.spacing,
        )
    }
    pub(super) fn clearance(self) -> Size {
        let footprint = self.footprint();
        Size::new(
            Self::CASE.width - footprint.width,
            Self::CASE.height - footprint.height,
        )
    }
    pub(super) fn scale(self, bounds: Rect) -> f64 {
        let footprint = self.footprint();
        (bounds.width() / Self::CASE.width.max(footprint.width))
            .min(bounds.height() / Self::CASE.height.max(footprint.height))
    }
    pub(super) fn case_rect(self, bounds: Rect) -> Rect {
        Rect::from_origin_size(bounds.origin(), Self::CASE * self.scale(bounds))
    }
    pub(super) fn preview(self, bounds: Rect) -> Vec<(Rect, f64)> {
        let columns = self.columns();
        let scale = self.scale(bounds);
        (0..12)
            .map(|index| {
                let x = bounds.x0
                    + (index % columns) as f64 * (Self::MODULE.width + self.spacing) * scale;
                let y = bounds.y0
                    + (index / columns) as f64 * (Self::MODULE.height + self.spacing) * scale;
                (
                    Rect::from_origin_size(Point::new(x, y), Self::MODULE * scale),
                    self.radius * scale,
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
        let mut values = CaseParameters::default();
        let bounds = Rect::new(0., 0., 220., 120.);
        let before = values.preview(bounds);
        assert!(!values.set(Parameter::Columns, f64::NAN));
        assert!(values.set(Parameter::Columns, 999.));
        assert_eq!(values.value(Parameter::Columns), 6.);
        assert_ne!(values.preview(bounds), before);
        values.set(Parameter::Spacing, 24.);
        values.set(Parameter::Radius, 24.);
        for (rect, radius) in values.preview(bounds) {
            assert!(bounds.inflate(0.001, 0.001).contains_rect(rect));
            assert!(radius <= rect.width().min(rect.height()) * 0.5 + 0.001);
        }
    }
    #[test]
    fn fixed_module_dimensions_reveal_case_overflow_instead_of_shrinking_modules() {
        let mut values = CaseParameters::default();
        assert_eq!(values.footprint(), Size::new(212., 120.));
        values.set(Parameter::Columns, 6.);
        values.set(Parameter::Spacing, 24.);
        assert_eq!(values.footprint(), Size::new(384., 88.));
        assert_eq!(values.clearance().width, -84.);
        values.set(Parameter::Columns, 2.);
        assert_eq!(values.footprint().height, 312.);
    }
}
