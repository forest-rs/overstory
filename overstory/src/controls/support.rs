// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Small shared helpers for built-in controls.

use kurbo::{Axis, Point, Rect, Size, Vec2};
use understory_property::{DependencyObjectExt, Property};

use crate::element::ElementId;
use crate::ui::Ui;

pub(crate) fn read_local_or<T: Clone + 'static>(
    ui: &Ui,
    element: ElementId,
    property: Property<T>,
    default: T,
) -> T {
    ui.get(element)
        .and_then(|el| el.get_local_value(property).cloned())
        .unwrap_or(default)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "control-local logical coordinates fit f32"
)]
pub(crate) fn f32_from_logical(value: f64) -> f32 {
    value as f32
}

pub(crate) fn is_initial_drag_move(press_position: Point, position: Point, delta: Vec2) -> bool {
    delta == position - press_position
}

// These receiver-style axis helpers are local stand-ins for methods we may want
// to upstream to kurbo once the usage settles. In particular, calls like
// `rect.min_along(axis)` read better than free functions at the call site.
pub(crate) trait PointAxisExt {
    fn along(self, axis: Axis) -> f64;
}

impl PointAxisExt for Point {
    fn along(self, axis: Axis) -> f64 {
        match axis {
            Axis::Horizontal => self.x,
            Axis::Vertical => self.y,
        }
    }
}

pub(crate) trait Vec2AxisExt {
    fn along(self, axis: Axis) -> f64;
}

impl Vec2AxisExt for Vec2 {
    fn along(self, axis: Axis) -> f64 {
        match axis {
            Axis::Horizontal => self.x,
            Axis::Vertical => self.y,
        }
    }
}

pub(crate) trait SizeAxisExt {
    fn along(self, axis: Axis) -> f64;
}

impl SizeAxisExt for Size {
    fn along(self, axis: Axis) -> f64 {
        match axis {
            Axis::Horizontal => self.width,
            Axis::Vertical => self.height,
        }
    }
}

pub(crate) trait RectAxisExt {
    fn min_along(self, axis: Axis) -> f64;

    fn max_along(self, axis: Axis) -> f64;

    fn size_along(self, axis: Axis) -> f64;
}

impl RectAxisExt for Rect {
    fn min_along(self, axis: Axis) -> f64 {
        match axis {
            Axis::Horizontal => self.x0,
            Axis::Vertical => self.y0,
        }
    }

    fn max_along(self, axis: Axis) -> f64 {
        match axis {
            Axis::Horizontal => self.x1,
            Axis::Vertical => self.y1,
        }
    }

    fn size_along(self, axis: Axis) -> f64 {
        match axis {
            Axis::Horizontal => self.width(),
            Axis::Vertical => self.height(),
        }
    }
}
