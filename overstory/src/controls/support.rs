// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Small shared helpers for built-in controls.

use core::ops::Range;

use kurbo::{Axis, Point, Rect, Size, Vec2};
use understory_property::{DependencyObjectExt, Property};
use understory_virtual_list::{FixedExtentModel, VirtualList};

use crate::element::ElementId;
use crate::layouts::{LayoutPlacement, LayoutSize};
use crate::scroll_runtime::ScrollState;
use crate::ui::Ui;

pub(crate) type FixedVirtualizer = VirtualList<FixedExtentModel<f64>>;

#[derive(Clone, Debug)]
pub(crate) struct FixedVirtualRowsSync {
    pub(crate) viewport: Size,
    pub(crate) range: Range<usize>,
    pub(crate) changed: bool,
}

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

pub(crate) fn action_numeric_value(request: &accesskit::ActionRequest) -> Option<f64> {
    match request.data.as_ref()? {
        accesskit::ActionData::NumericValue(value) => Some(*value),
        accesskit::ActionData::Value(value) => value.parse().ok(),
        _ => None,
    }
}

pub(crate) fn action_scroll_unit(
    request: &accesskit::ActionRequest,
) -> Option<accesskit::ScrollUnit> {
    match request.data.as_ref() {
        None => Some(accesskit::ScrollUnit::Item),
        Some(accesskit::ActionData::ScrollUnit(unit)) => Some(*unit),
        _ => None,
    }
}

pub(crate) fn fixed_virtualizer(
    len: usize,
    item_extent: f64,
    scroll_offset: f64,
    overscan: f64,
) -> FixedVirtualizer {
    let mut virtualizer = VirtualList::new(FixedExtentModel::new(len, item_extent), 0.0, overscan);
    virtualizer.set_scroll_offset(scroll_offset);
    virtualizer
}

pub(crate) fn sync_fixed_virtual_rows(
    owner: ElementId,
    ui: &Ui,
    virtualizer: &mut FixedVirtualizer,
    scroll_state: &ScrollState,
    len: usize,
    last_viewport: &mut Size,
) -> FixedVirtualRowsSync {
    let bounds = ui.effective_bounds(owner);
    let viewport = Size::new(bounds.width().max(0.0), bounds.height().max(0.0));
    let viewport_changed = !sizes_nearly_equal(viewport, *last_viewport);
    *last_viewport = viewport;

    virtualizer.set_len(len);
    virtualizer.set_viewport_extent(viewport.height);
    virtualizer.set_scroll_offset(scroll_state.offset_y());
    virtualizer.clamp_scroll_to_content();
    let offset_changed = scroll_state.set_offset_y_raw(virtualizer.scroll_offset());
    let content_height = virtualizer.total_extent();
    scroll_state.set_metrics(viewport, Size::new(viewport.width, content_height));

    FixedVirtualRowsSync {
        viewport,
        range: virtualizer.materialized_range(),
        changed: viewport_changed || offset_changed,
    }
}

pub(crate) fn layout_fixed_virtual_rows<'a>(
    ui: &mut Ui,
    virtualizer: &mut FixedVirtualizer,
    item_extent: f64,
    rows: impl IntoIterator<Item = (usize, ElementId)> + 'a,
    viewport: Size,
) -> bool {
    let mut changed = false;
    let width = f32_from_logical(viewport.width);
    let height = f32_from_logical(item_extent);
    for (index, element) in rows {
        let top = f32_from_logical(virtualizer.offset_of(index));
        let size = LayoutSize::logical(width, height);
        let placement = LayoutPlacement::absolute_at(0.0, top);
        let size_changed = ui
            .get(element)
            .and_then(|element| element.get_local_value(ui.properties().layout_size))
            != Some(&size);
        let placement_changed = ui
            .get(element)
            .and_then(|element| element.get_local_value(ui.properties().layout_placement))
            != Some(&placement);
        if size_changed || placement_changed {
            let props = *ui.properties();
            ui.set_local(element, props.layout_size, size);
            ui.set_local(element, props.layout_placement, placement);
            changed = true;
        }
    }
    changed
}

pub(crate) fn fixed_visible_item_count(item_extent: f64, viewport_extent: f64) -> usize {
    if item_extent <= f64::EPSILON || viewport_extent <= 0.0 {
        return 1;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "positive viewport/item ratio is clamped to at least one visible item"
    )]
    {
        (viewport_extent / item_extent).floor().max(1.0) as usize
    }
}

pub(crate) fn sizes_nearly_equal(a: Size, b: Size) -> bool {
    (a.width - b.width).abs() <= f64::EPSILON && (a.height - b.height).abs() <= f64::EPSILON
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
