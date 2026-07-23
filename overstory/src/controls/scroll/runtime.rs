// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::{Axis, Size, Vec2};
use understory_property::DependencyObjectExt;

use crate::behavior::ArrangeContext;
use crate::controls::support::{RectAxisExt, SizeAxisExt, Vec2AxisExt, action_scroll_unit};
use crate::element::{Element, ElementId};
use crate::event_runtime::ScrollEvent;
use crate::layouts::{LayoutDirection, LayoutSize};
use crate::scroll_runtime::{
    ScrollCommandResult, ScrollInputSource, ScrollState, normalize_scroll_axis,
};
use crate::ui::Ui;

use super::events::ScrollOffsetChangedEvent;
use super::ids::DEFAULT_SCROLL_LINE_Y;
use super::view::ScrollViewWidget;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ScrollLayoutState {
    normalization: Vec2,
    direction: LayoutDirection,
}

impl ScrollLayoutState {
    pub(crate) fn child_offset(self, state: &ScrollState) -> Vec2 {
        self.normalization - state.offset()
    }
}
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
    let delta = Vec2::new(
        if delta.x.is_finite() { delta.x } else { 0.0 },
        if delta.y.is_finite() { delta.y } else { 0.0 },
    );
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
) -> Option<ScrollCommandResult> {
    let unit = action_scroll_unit(request)?;
    let amount = match unit {
        accesskit::ScrollUnit::Item => DEFAULT_SCROLL_LINE_Y,
        accesskit::ScrollUnit::Page => state.viewport_size().along(axis).max(DEFAULT_SCROLL_LINE_Y),
    };
    let next = axis.pack_vec2(
        state_axis_scroll_offset(state, axis) + (amount * direction),
        state.offset().along(axis.cross()),
    );
    let change = apply_scroll_offset(ui, viewport, state, next, ScrollInputSource::Accessibility);
    change.changed().then_some(change)
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
        ScrollInputSource::Programmatic,
    )
}

pub(crate) fn scroll_by(
    ui: &mut Ui,
    viewport: ElementId,
    state: &ScrollState,
    delta: Vec2,
    source: ScrollInputSource,
) -> ScrollCommandResult {
    apply_scroll_offset(ui, viewport, state, state.offset() + delta, source)
}

pub(super) fn set_scroll_offset(
    ui: &mut Ui,
    viewport: ElementId,
    state: &ScrollState,
    offset: Vec2,
    source: ScrollInputSource,
) -> bool {
    let change = apply_scroll_offset(ui, viewport, state, offset, source);
    if !change.changed() {
        return false;
    }
    dispatch_scroll_offset_changed(ui, viewport, change);
    true
}

pub(crate) fn apply_scroll_offset(
    ui: &mut Ui,
    viewport: ElementId,
    state: &ScrollState,
    offset: Vec2,
    source: ScrollInputSource,
) -> ScrollCommandResult {
    refresh_scroll_metrics(ui, viewport, state);
    ui.apply_scroll_command(viewport, offset, source)
        .expect("scroll commands require a registered target")
}

fn dispatch_scroll_offset_changed(
    ui: &mut Ui,
    scroll_view: ElementId,
    change: ScrollCommandResult,
) {
    let event = ScrollOffsetChangedEvent {
        scroll_view,
        previous_offset: change.previous_offset,
        offset: change.offset,
        source: change.source,
    };
    ui.with_widget_mut(scroll_view, |widget, ui| {
        let Some(widget) = widget.downcast_mut::<ScrollViewWidget>() else {
            return;
        };
        widget.offset_changed.dispatch(ui, scroll_view, event);
    });
}

fn refresh_scroll_metrics(ui: &Ui, viewport: ElementId, state: &ScrollState) {
    let viewport_bounds = ui.effective_bounds(viewport);
    if viewport_bounds.width() <= 0.0 || viewport_bounds.height() <= 0.0 {
        return;
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
}

pub(crate) fn update_scroll_metrics_from_layout(
    ctx: &ArrangeContext<'_>,
    state: &ScrollState,
    layout_state: &mut ScrollLayoutState,
) -> bool {
    let Some(viewport_bounds) = ctx.effective_bounds(ctx.id()) else {
        return false;
    };
    let Some(element) = ctx.ui().get(ctx.id()) else {
        return false;
    };
    let child_bounds = element
        .children()
        .iter()
        .filter_map(|child| ctx.effective_bounds(*child));
    let previous_offset = state.offset();
    let previous_max_offset = state.max_offset();
    let previous_layout = *layout_state;
    let normalization = update_scroll_metrics_from_bounds(
        viewport_bounds,
        state,
        layout_state.normalization,
        child_bounds,
    );
    let direction = ctx.ui().layout_direction(ctx.id()).unwrap_or_default();
    let logical_x = if layout_state.direction.is_rtl() {
        previous_max_offset.x - previous_offset.x
    } else {
        previous_offset.x
    };
    let physical_x = if direction.is_rtl() {
        state.max_offset().x - logical_x
    } else {
        logical_x
    };
    state.set_offset_raw(Vec2::new(physical_x, state.offset().y));
    *layout_state = ScrollLayoutState {
        normalization,
        direction,
    };
    previous_layout.child_offset(state) != layout_state.child_offset(state)
        || previous_offset != state.offset()
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
    let layout_state = element
        .widget()
        .and_then(|widget| widget.downcast_ref::<ScrollViewWidget>())
        .map_or_else(ScrollLayoutState::default, |widget| widget.layout_state);
    update_scroll_metrics_from_bounds(
        viewport_bounds,
        state,
        layout_state.normalization,
        child_bounds,
    );
}

fn update_scroll_metrics_from_bounds(
    viewport_bounds: kurbo::Rect,
    state: &ScrollState,
    normalization: Vec2,
    child_bounds: impl IntoIterator<Item = kurbo::Rect>,
) -> Vec2 {
    if viewport_bounds.width() <= 0.0 || viewport_bounds.height() <= 0.0 {
        state.set_metrics(Size::ZERO, Size::ZERO);
        return Vec2::ZERO;
    }
    let offset = state.offset();
    let translation = normalization - offset;
    let mut content_left = 0.0_f64;
    let mut content_top = 0.0_f64;
    let mut content_right = viewport_bounds.width();
    let mut content_bottom = viewport_bounds.height();
    for child_bounds in child_bounds {
        content_left = content_left.min(
            child_bounds.min_along(Axis::Horizontal)
                - viewport_bounds.min_along(Axis::Horizontal)
                - translation.x,
        );
        content_top = content_top.min(
            child_bounds.min_along(Axis::Vertical)
                - viewport_bounds.min_along(Axis::Vertical)
                - translation.y,
        );
        content_right = content_right.max(
            child_bounds.max_along(Axis::Horizontal)
                - viewport_bounds.min_along(Axis::Horizontal)
                - translation.x,
        );
        content_bottom = content_bottom.max(
            child_bounds.max_along(Axis::Vertical)
                - viewport_bounds.min_along(Axis::Vertical)
                - translation.y,
        );
    }
    state.set_metrics(
        Size::new(viewport_bounds.width(), viewport_bounds.height()),
        Size::new(content_right - content_left, content_bottom - content_top),
    );
    Vec2::new(-content_left, -content_top)
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
