// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Small shared helpers for built-in controls.

use kurbo::{Axis, Point, Rect, Vec2};
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

pub(crate) fn axis_point(axis: Axis, point: Point) -> f64 {
    match axis {
        Axis::Horizontal => point.x,
        Axis::Vertical => point.y,
    }
}

pub(crate) fn axis_vec(axis: Axis, vec: Vec2) -> f64 {
    match axis {
        Axis::Horizontal => vec.x,
        Axis::Vertical => vec.y,
    }
}

pub(crate) fn axis_rect_size(axis: Axis, rect: Rect) -> f64 {
    match axis {
        Axis::Horizontal => rect.width(),
        Axis::Vertical => rect.height(),
    }
}
