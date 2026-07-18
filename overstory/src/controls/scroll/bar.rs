// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::boxed::Box;

use kurbo::{Axis, Point, Vec2};
use understory_style::TypeTag;

use crate::behavior::{ArrangeContext, ArrangeResult, WidgetEventContext};
use crate::controls::support::{
    PointAxisExt, SizeAxisExt, Vec2AxisExt, action_numeric_value, is_initial_drag_move,
};
use crate::element::{Element, ElementId};
use crate::event_runtime::{ClickEvent, DragEvent};
use crate::layouts::{Layout, LayoutSize};
use crate::scroll_runtime::{ScrollInputSource, ScrollState};
use crate::template::{TemplateContract, TemplateNode, TemplateSpec, TemplateSpecRecipe};
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, ControlTemplateSpec, ElementConfig, ElementSpec, Widget};

use super::ids::{
    DEFAULT_SCROLL_BAR_LENGTH, DEFAULT_SCROLL_BAR_THICKNESS, DEFAULT_SCROLL_BAR_THUMB_EXTENT,
    DEFAULT_SCROLL_LINE_Y, SLOT_SCROLL_BAR_THUMB, TYPE_SCROLL_BAR,
};
use super::runtime::{
    axis_scroll_offset, f64_nearly_equal, set_layout_size_if_changed, set_scroll_offset,
    state_axis_scroll_offset,
};
/// Builder + spec for a scrollbar that controls a viewport's shared scroll state.
pub struct ScrollBar {
    axis: Axis,
    viewport: ElementId,
    length: f64,
    thickness: f64,
    thumb_extent: f64,
    element: ElementConfig,
}

impl core::fmt::Debug for ScrollBar {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ScrollBar")
            .field("axis", &self.axis)
            .field("viewport", &self.viewport)
            .field("length", &self.length)
            .field("thickness", &self.thickness)
            .field("thumb_extent", &self.thumb_extent)
            .field("control_state", &self.control_state())
            .finish_non_exhaustive()
    }
}

impl ScrollBar {
    /// Starts a vertical scrollbar bound to `viewport`.
    #[must_use]
    pub fn vertical(viewport: ElementId) -> Self {
        Self::new(Axis::Vertical, viewport)
    }

    /// Starts a horizontal scrollbar bound to `viewport`.
    #[must_use]
    pub fn horizontal(viewport: ElementId) -> Self {
        Self::new(Axis::Horizontal, viewport)
    }

    /// Starts a scrollbar on `axis` bound to `viewport`.
    #[must_use]
    pub fn new(axis: Axis, viewport: ElementId) -> Self {
        let mut element = ElementConfig::default();
        element.set_control_state(ControlState::interactive().focusable(false));
        Self {
            axis,
            viewport,
            length: DEFAULT_SCROLL_BAR_LENGTH,
            thickness: DEFAULT_SCROLL_BAR_THICKNESS,
            thumb_extent: DEFAULT_SCROLL_BAR_THUMB_EXTENT,
            element,
        }
    }

    /// Sets the track length in logical pixels.
    #[must_use]
    pub fn length(mut self, length: f64) -> Self {
        self.length = length.max(1.0);
        self.thumb_extent = self.thumb_extent.min(self.length);
        self
    }

    /// Sets the track thickness in logical pixels.
    #[must_use]
    pub fn thickness(mut self, thickness: f64) -> Self {
        self.thickness = thickness.max(1.0);
        self
    }

    /// Sets the fixed thumb extent in logical pixels.
    #[must_use]
    pub fn thumb_extent(mut self, extent: f64) -> Self {
        self.thumb_extent = extent.clamp(1.0, self.length.max(1.0));
        self
    }

    /// Sets whether the scrollbar starts enabled.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.element
            .update_control_state(|state| state.enabled(enabled));
        self
    }

    /// Convenience for [`Self::enabled`] with `false`.
    #[must_use]
    pub fn disabled(self) -> Self {
        self.enabled(false)
    }

    fn control_state(&self) -> ControlState {
        self.element
            .control_state()
            .unwrap_or_else(|| ControlState::interactive().focusable(false))
    }
}

impl AppendSpec for ScrollBar {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let widget = ScrollBarWidget {
            axis: self.axis,
            viewport: self.viewport,
            state: ui
                .scroll_target(self.viewport)
                .map(|target| target.state())
                .unwrap_or_default(),
            length: self.length,
            thickness: self.thickness,
            thumb_extent: self.thumb_extent,
            drag: ScrollBarDragState::default(),
        };
        let id = ui.append_widget_deferred_template(parent, Box::new(widget));
        ui.set_layout(
            id,
            scroll_bar_layout(self.axis, self.length, self.thickness),
        );
        self.element.apply_to(ui, id);
        ui.expand_resolved_template(id);
        sync_scroll_bar_thumb_size(ui, id);
        id
    }
}

impl ControlTemplateSpec for ScrollBar {}

impl ElementSpec for ScrollBar {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

/// Per-element widget for `ScrollBar`.
#[derive(Clone, Debug)]
pub(crate) struct ScrollBarWidget {
    axis: Axis,
    viewport: ElementId,
    // A clone of the target widget's internal scroll state. Apps bind
    // scrollbars by target element; the scrollbar keeps this handle only so
    // layout and accessibility hooks can read offset/metrics without a Ui.
    state: ScrollState,
    length: f64,
    thickness: f64,
    thumb_extent: f64,
    drag: ScrollBarDragState,
}

impl Widget for ScrollBarWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_SCROLL_BAR)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::ScrollBar)
    }

    fn configure_accessibility(
        &self,
        element: &Element,
        ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        let value = state_axis_scroll_offset(&self.state, self.axis);
        let max = axis_scroll_offset(self.state.max_offset(), self.axis);
        node.set_orientation(accesskit_orientation(self.axis));
        node.set_numeric_value(value);
        node.set_min_numeric_value(0.0);
        node.set_max_numeric_value(max);
        node.set_numeric_value_step(DEFAULT_SCROLL_LINE_Y);
        node.set_numeric_value_jump(scroll_bar_page(&self.state, self.axis));
        if ctx.is_enabled(element.id()) {
            node.add_action(accesskit::Action::Increment);
            node.add_action(accesskit::Action::Decrement);
            node.add_action(accesskit::Action::SetValue);
        }
    }

    fn template_contract(&self) -> TemplateContract {
        TemplateContract::new().with_required_slot(SLOT_SCROLL_BAR_THUMB)
    }

    fn clips_subtree(&self, _element: &Element) -> bool {
        true
    }

    fn child_layout_offset(&self, _element: &Element) -> Vec2 {
        scroll_bar_thumb_offset(self.axis, &self.state, self.length, self.thumb_extent)
    }

    fn arrange(&mut self, ctx: &mut ArrangeContext<'_>) -> ArrangeResult {
        let Some(bounds) = ctx.effective_bounds(ctx.id()) else {
            return ArrangeResult::UNCHANGED;
        };
        let size = bounds.size();
        let length = size.along(self.axis).max(1.0);
        let thickness = size.along(self.axis.cross()).max(1.0);
        let thumb_extent = self.thumb_extent.clamp(1.0, length);

        let changed = !f64_nearly_equal(self.length, length)
            || !f64_nearly_equal(self.thickness, thickness)
            || !f64_nearly_equal(self.thumb_extent, thumb_extent);
        if changed {
            self.length = length;
            self.thickness = thickness;
            self.thumb_extent = thumb_extent;
        }

        let scroll_bar = ctx.id();
        ArrangeResult::from_layout_changed(
            changed || sync_scroll_bar_thumb_size(ctx.ui_mut(), scroll_bar),
        )
    }

    fn handle_click(&mut self, ctx: &mut WidgetEventContext<'_>, event: ClickEvent<'_>) {
        let scroll_bar = ctx.id();
        if !event.is_semantic_target() || !ctx.is_enabled(scroll_bar) {
            return;
        }
        if is_scroll_bar_thumb(ctx, scroll_bar, event.source()) {
            return;
        }
        let page = scroll_bar_page(&self.state, self.axis);
        let current = state_axis_scroll_offset(&self.state, self.axis);
        let thumb_start = axis_thumb_start(
            self.axis,
            ctx.effective_bounds(scroll_bar).origin(),
            self.state.offset(),
            self.state.max_offset(),
            self.length,
            self.thumb_extent,
        );
        let pointer = event.position.along(self.axis);
        let next = if pointer < thumb_start {
            current - page
        } else {
            current + page
        };
        set_axis_scroll_offset(
            ctx,
            self.viewport,
            &self.state,
            self.axis,
            next,
            ScrollInputSource::ScrollBar,
        );
        event.mark_handled();
    }

    fn handle_drag(&mut self, ctx: &mut WidgetEventContext<'_>, event: DragEvent<'_>) {
        let scroll_bar = ctx.id();
        if !event.is_semantic_target() || !ctx.is_enabled(scroll_bar) {
            return;
        }
        let snapshot = ScrollBarSnapshot::from(&*self);
        self.drag.update(
            ctx,
            scroll_bar,
            &snapshot,
            event.source(),
            event.press_position,
            event.position,
            event.delta,
        );
        event.mark_handled();
    }

    fn handle_accessibility_action(
        &mut self,
        ctx: &mut WidgetEventContext<'_>,
        request: &accesskit::ActionRequest,
    ) -> bool {
        let scroll_bar = ctx.id();
        if !ctx.is_enabled(scroll_bar) {
            return false;
        }
        let current = state_axis_scroll_offset(&self.state, self.axis);
        let next = match request.action {
            accesskit::Action::Increment => current + DEFAULT_SCROLL_LINE_Y,
            accesskit::Action::Decrement => current - DEFAULT_SCROLL_LINE_Y,
            accesskit::Action::SetValue => {
                let Some(value) = action_numeric_value(request) else {
                    return false;
                };
                value
            }
            _ => return false,
        };
        set_axis_scroll_offset(
            ctx,
            self.viewport,
            &self.state,
            self.axis,
            next,
            ScrollInputSource::Accessibility,
        )
    }
}

static SCROLL_BAR_TEMPLATE: TemplateSpecRecipe = TemplateSpecRecipe::new(scroll_bar_template_spec);

pub(crate) fn default_scroll_bar_template() -> crate::ControlTemplate {
    crate::ControlTemplate::from_static(&SCROLL_BAR_TEMPLATE)
}

fn scroll_bar_template_spec() -> TemplateSpec {
    TemplateSpec::from_nodes([
        TemplateNode::surface_slot(SLOT_SCROLL_BAR_THUMB).with_layout(
            Layout::new()
                .absolute_at(0.0, 0.0)
                .size(scroll_bar_thumb_size(
                    Axis::Vertical,
                    DEFAULT_SCROLL_BAR_THICKNESS,
                    DEFAULT_SCROLL_BAR_THUMB_EXTENT,
                )),
        ),
    ])
}
#[derive(Clone, Debug)]
struct ScrollBarSnapshot {
    axis: Axis,
    viewport: ElementId,
    state: ScrollState,
    length: f64,
    thickness: f64,
    thumb_extent: f64,
}

impl From<&ScrollBarWidget> for ScrollBarSnapshot {
    fn from(widget: &ScrollBarWidget) -> Self {
        Self {
            axis: widget.axis,
            viewport: widget.viewport,
            state: widget.state.clone(),
            length: widget.length,
            thickness: widget.thickness,
            thumb_extent: widget.thumb_extent,
        }
    }
}

#[derive(Copy, Clone, Debug, Default)]
struct ScrollBarDragState {
    offset_from_thumb_start: f64,
}

impl ScrollBarDragState {
    fn update(
        &mut self,
        ui: &mut Ui,
        scroll_bar: ElementId,
        widget: &ScrollBarSnapshot,
        source: ElementId,
        press_position: Point,
        position: Point,
        delta: Vec2,
    ) {
        let bar_origin = ui.effective_bounds(scroll_bar).origin();
        if is_initial_drag_move(press_position, position, delta) {
            let thumb_start = axis_thumb_start(
                widget.axis,
                bar_origin,
                widget.state.offset(),
                widget.state.max_offset(),
                widget.length,
                widget.thumb_extent,
            );
            if is_scroll_bar_thumb(ui, scroll_bar, source) {
                self.offset_from_thumb_start = press_position.along(widget.axis) - thumb_start;
            } else {
                self.offset_from_thumb_start = widget.thumb_extent / 2.0;
            }
        }
        let track_start = bar_origin.along(widget.axis);
        let thumb_position =
            position.along(widget.axis) - track_start - self.offset_from_thumb_start;
        let travel = (widget.length - widget.thumb_extent).max(0.0);
        if travel <= f64::EPSILON {
            return;
        }
        let max = axis_scroll_offset(widget.state.max_offset(), widget.axis);
        let next = (thumb_position / travel).clamp(0.0, 1.0) * max;
        set_axis_scroll_offset(
            ui,
            widget.viewport,
            &widget.state,
            widget.axis,
            next,
            ScrollInputSource::ScrollBar,
        );
    }
}

fn scroll_bar_widget(ui: &Ui, scroll_bar: ElementId) -> Option<ScrollBarSnapshot> {
    ui.get(scroll_bar)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<ScrollBarWidget>())
        .map(ScrollBarSnapshot::from)
}

fn scroll_bar_layout(axis: Axis, length: f64, thickness: f64) -> Layout {
    Layout::new()
        .relative()
        .size(scroll_bar_size(axis, length, thickness))
}

pub(super) fn scroll_bar_size(axis: Axis, length: f64, thickness: f64) -> LayoutSize {
    match axis {
        Axis::Horizontal => LayoutSize::logical(length, thickness),
        Axis::Vertical => LayoutSize::logical(thickness, length),
    }
}

fn scroll_bar_thumb_size(axis: Axis, thickness: f64, extent: f64) -> LayoutSize {
    match axis {
        Axis::Horizontal => LayoutSize::logical(extent, thickness),
        Axis::Vertical => LayoutSize::logical(thickness, extent),
    }
}

fn sync_scroll_bar_thumb_size(ui: &mut Ui, scroll_bar: ElementId) -> bool {
    let Some(widget) = scroll_bar_widget(ui, scroll_bar) else {
        return false;
    };
    let Some(thumb) = ui.slot(scroll_bar, SLOT_SCROLL_BAR_THUMB) else {
        return false;
    };
    set_layout_size_if_changed(
        ui,
        thumb,
        scroll_bar_thumb_size(widget.axis, widget.thickness, widget.thumb_extent),
    )
}

fn scroll_bar_thumb_offset(
    axis: Axis,
    state: &ScrollState,
    length: f64,
    thumb_extent: f64,
) -> Vec2 {
    let max = axis_scroll_offset(state.max_offset(), axis);
    if max <= f64::EPSILON {
        return Vec2::ZERO;
    }
    let travel = (length - thumb_extent).max(0.0);
    let offset = (state_axis_scroll_offset(state, axis) / max).clamp(0.0, 1.0) * travel;
    axis.pack_vec2(offset, 0.0)
}

fn accesskit_orientation(axis: Axis) -> accesskit::Orientation {
    match axis {
        Axis::Horizontal => accesskit::Orientation::Horizontal,
        Axis::Vertical => accesskit::Orientation::Vertical,
    }
}

fn is_scroll_bar_thumb(ui: &Ui, scroll_bar: ElementId, source: ElementId) -> bool {
    ui.slot(scroll_bar, SLOT_SCROLL_BAR_THUMB) == Some(source)
}

fn scroll_bar_page(state: &ScrollState, axis: Axis) -> f64 {
    state.viewport_size().along(axis).max(DEFAULT_SCROLL_LINE_Y)
}

fn set_axis_scroll_offset(
    ui: &mut Ui,
    viewport: ElementId,
    state: &ScrollState,
    axis: Axis,
    offset: f64,
    source: ScrollInputSource,
) -> bool {
    let next = axis.pack_vec2(offset, state.offset().along(axis.cross()));
    set_scroll_offset(ui, viewport, state, next, source)
}

fn axis_thumb_start(
    axis: Axis,
    track_origin: Point,
    offset: Vec2,
    max: Vec2,
    length: f64,
    thumb_extent: f64,
) -> f64 {
    track_origin.along(axis)
        + axis_scroll_fraction(offset.along(axis), max.along(axis))
            * (length - thumb_extent).max(0.0)
}

fn axis_scroll_fraction(offset: f64, max: f64) -> f64 {
    if max <= f64::EPSILON {
        0.0
    } else {
        (offset / max).clamp(0.0, 1.0)
    }
}
