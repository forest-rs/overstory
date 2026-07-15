// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::{Axis, Size, Vec2};
use understory_property::DependencyObjectExt;

use crate::behavior::ArrangeContext;
use crate::channels::ARRANGE;
use crate::controls::support::{RectAxisExt, SizeAxisExt, Vec2AxisExt, action_scroll_unit};
use crate::element::{Element, ElementId};
use crate::event_runtime::ScrollEvent;
use crate::layouts::LayoutSize;
use crate::scroll_runtime::{ScrollState, normalize_scroll_axis};
use crate::ui::Ui;

use super::events::{ScrollOffsetChangeSource, ScrollOffsetChangedEvent};
use super::ids::DEFAULT_SCROLL_LINE_Y;
use super::view::ScrollViewWidget;
/// Normalizes a vertical scroll offset for public builder and setter APIs.
pub(crate) fn normalize_scroll_offset_y(offset_y: f64) -> f64 {
    normalize_scroll_axis(offset_y)
}

/// Returns whether two layout scalars are close enough to avoid a rewrite.
pub(super) fn f64_nearly_equal(a: f64, b: f64) -> bool {
    (a - b).abs() <= f64::EPSILON
}

/// Returns the current vertical scroll offset for `scroll_view`.
pub fn scroll_view_offset_y(ui: &Ui, scroll_view: ElementId) -> Option<f64> {
    scroll_offset_y(ui, scroll_view)
}

/// Sets the vertical scroll offset for `scroll_view`.
pub fn set_scroll_view_offset_y(ui: &mut Ui, scroll_view: ElementId, offset: f64) -> bool {
    set_scroll_offset_y(ui, scroll_view, offset)
}

/// Returns the current vertical scroll offset for a scrollable target.
#[must_use]
pub fn scroll_offset_y(ui: &Ui, target: ElementId) -> Option<f64> {
    ui.scroll_target(target).map(|target| target.offset_y())
}

/// Sets the vertical scroll offset for a scrollable target.
pub fn set_scroll_offset_y(ui: &mut Ui, target: ElementId, offset: f64) -> bool {
    let Some(state) = ui.scroll_target(target).map(|target| target.state()) else {
        return false;
    };
    set_scroll_state_offset_y(ui, target, &state, offset)
}

pub(crate) fn wheel_scroll_delta(
    ui: &Ui,
    viewport: ElementId,
    event: ScrollEvent<'_>,
) -> Option<Vec2> {
    if !(event.phase().is_target() || event.phase().is_bubble()) || !ui.is_enabled(viewport) {
        return None;
    }
    let bounds = ui.effective_bounds(viewport);
    let page = Vec2::new(bounds.width(), bounds.height());
    let raw_delta = event.logical_delta(
        Vec2::new(DEFAULT_SCROLL_LINE_Y, DEFAULT_SCROLL_LINE_Y),
        page,
    );
    // Wheel deltas describe device movement; scroll offsets move content in
    // the opposite direction.
    let delta = -raw_delta;
    if delta == Vec2::ZERO {
        None
    } else {
        Some(delta)
    }
}

pub(crate) fn scroll_by_accessibility_action(
    ui: &mut Ui,
    viewport: ElementId,
    state: &ScrollState,
    axis: Axis,
    direction: f64,
    request: &accesskit::ActionRequest,
) -> Option<ScrollOffsetChange> {
    let unit = action_scroll_unit(request)?;
    let amount = match unit {
        accesskit::ScrollUnit::Item => DEFAULT_SCROLL_LINE_Y,
        accesskit::ScrollUnit::Page => state.viewport_size().along(axis).max(DEFAULT_SCROLL_LINE_Y),
    };
    let next = axis.pack_vec2(
        state_axis_scroll_offset(state, axis) + (amount * direction),
        state.offset().along(axis.cross()),
    );
    set_scroll_offset_raw(ui, viewport, state, next)
}

pub(crate) fn set_scroll_state_offset_y(
    ui: &mut Ui,
    viewport: ElementId,
    state: &ScrollState,
    offset: f64,
) -> bool {
    set_scroll_offset(
        ui,
        viewport,
        state,
        Vec2::new(state.offset().x, offset),
        ScrollOffsetChangeSource::Programmatic,
    )
}

pub(crate) fn state_scroll_offset_y(state: &ScrollState) -> f64 {
    state.offset_y()
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ScrollOffsetChange {
    pub(super) previous: Vec2,
    pub(super) offset: Vec2,
}

pub(crate) fn scroll_by(
    ui: &mut Ui,
    viewport: ElementId,
    state: &ScrollState,
    delta: Vec2,
) -> Option<ScrollOffsetChange> {
    set_scroll_offset_raw(ui, viewport, state, state.offset() + delta)
}

pub(super) fn set_scroll_offset(
    ui: &mut Ui,
    viewport: ElementId,
    state: &ScrollState,
    offset: Vec2,
    source: ScrollOffsetChangeSource,
) -> bool {
    let Some(change) = set_scroll_offset_raw(ui, viewport, state, offset) else {
        return false;
    };
    dispatch_scroll_offset_changed(ui, viewport, change, source);
    true
}

pub(crate) fn set_scroll_offset_raw(
    ui: &mut Ui,
    viewport: ElementId,
    state: &ScrollState,
    offset: Vec2,
) -> Option<ScrollOffsetChange> {
    let previous = state.offset();
    let max = max_scroll_offset(ui, viewport, state);
    let clamped = Vec2::new(
        normalize_scroll_axis(offset.x).min(max.x),
        normalize_scroll_axis(offset.y).min(max.y),
    );
    if !state.set_offset_raw(clamped) {
        return None;
    }
    let channels = ui
        .scroll_target(viewport)
        .map(|target| target.offset_channels())
        .unwrap_or_else(|| ARRANGE.into());
    ui.mark_channels(viewport, channels);
    Some(ScrollOffsetChange {
        previous,
        offset: clamped,
    })
}

fn dispatch_scroll_offset_changed(
    ui: &mut Ui,
    scroll_view: ElementId,
    change: ScrollOffsetChange,
    source: ScrollOffsetChangeSource,
) {
    let event = ScrollOffsetChangedEvent {
        scroll_view,
        previous_offset: change.previous,
        offset: change.offset,
        source,
    };
    ui.with_widget_mut(scroll_view, |widget, ui| {
        let Some(widget) = widget.downcast_mut::<ScrollViewWidget>() else {
            return;
        };
        widget.offset_changed.dispatch(ui, scroll_view, event);
    });
}

fn max_scroll_offset(ui: &Ui, viewport: ElementId, state: &ScrollState) -> Vec2 {
    let viewport_bounds = ui.effective_bounds(viewport);
    if viewport_bounds.width() <= 0.0 || viewport_bounds.height() <= 0.0 {
        return Vec2::ZERO;
    }
    if let Some(element) = ui.get(viewport)
        && element
            .widget()
            .and_then(|widget| widget.downcast_ref::<ScrollViewWidget>())
            .is_some()
    {
        // `ScrollView` derives its extent from child geometry. Virtualized
        // controls, such as `ListBox`, own their metrics and must not be
        // clamped to only the currently realized children.
        update_scroll_view_metrics_from_ui_bounds(ui, state, viewport_bounds, element);
    }
    state.max_offset()
}

pub(crate) fn update_scroll_metrics_from_layout(ctx: &ArrangeContext<'_>, state: &ScrollState) {
    let Some(viewport_bounds) = ctx.effective_bounds(ctx.id()) else {
        return;
    };
    let Some(element) = ctx.ui().get(ctx.id()) else {
        return;
    };
    let child_bounds = element
        .children()
        .iter()
        .filter_map(|child| ctx.effective_bounds(*child));
    update_scroll_metrics_from_bounds(viewport_bounds, state, child_bounds);
}

fn update_scroll_view_metrics_from_ui_bounds(
    ui: &Ui,
    state: &ScrollState,
    viewport_bounds: kurbo::Rect,
    element: &Element,
) {
    let child_bounds = element
        .children()
        .iter()
        .map(|child| ui.effective_bounds(*child));
    update_scroll_metrics_from_bounds(viewport_bounds, state, child_bounds);
}

fn update_scroll_metrics_from_bounds(
    viewport_bounds: kurbo::Rect,
    state: &ScrollState,
    child_bounds: impl IntoIterator<Item = kurbo::Rect>,
) {
    if viewport_bounds.width() <= 0.0 || viewport_bounds.height() <= 0.0 {
        state.set_metrics(Size::ZERO, Size::ZERO);
        return;
    }
    let offset = state.offset();
    let mut content_right = 0.0_f64;
    let mut content_bottom = 0.0_f64;
    for child_bounds in child_bounds {
        content_right = content_right.max(
            child_bounds.max_along(Axis::Horizontal) + offset.x
                - viewport_bounds.min_along(Axis::Horizontal),
        );
        content_bottom = content_bottom.max(
            child_bounds.max_along(Axis::Vertical) + offset.y
                - viewport_bounds.min_along(Axis::Vertical),
        );
    }
    state.set_metrics(
        Size::new(viewport_bounds.width(), viewport_bounds.height()),
        Size::new(content_right, content_bottom),
    );
}

pub(super) fn set_layout_size_if_changed(
    ui: &mut Ui,
    element: ElementId,
    size: LayoutSize,
) -> bool {
    if ui
        .get(element)
        .and_then(|element| element.get_local_value(ui.properties().layout_size))
        == Some(&size)
    {
        return false;
    }
    ui.set_layout_size(element, size);
    true
}

pub(super) fn state_axis_scroll_offset(state: &ScrollState, axis: Axis) -> f64 {
    axis_scroll_offset(state.offset(), axis)
}

pub(super) fn axis_scroll_offset(offset: Vec2, axis: Axis) -> f64 {
    offset.along(axis)
}
