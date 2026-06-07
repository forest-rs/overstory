// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `ScrollView`: reusable clipped viewport state and behavior.
//!
//! Roadmap:
//! - Make `ScrollView` the reusable basis for lists, text fields, graph
//!   viewports, and any control that needs clipped, offset content.
//! - Support both axes consistently: horizontal and vertical bars, overlay vs
//!   reserved gutters, keyboard paging, wheel / touchpad deltas, and
//!   bring-into-view requests.
//! - Keep scroll metrics, clamping, thumb math, and shared scroll state here so
//!   controls do not each invent their own scrolling model.
//! - Integrate with virtualization and compositor-backed scrolling as separate
//!   policy layers instead of baking renderer decisions into the control.
//! - Add accessibility range metadata and platform scroll actions through the
//!   host layer.

use alloc::boxed::Box;

use kurbo::{Axis, Point, Size, Vec2};
use understory_property::DependencyObjectExt;
use understory_style::{PartTag, TypeTag};

use crate::Phase;
use crate::behavior::{ArrangeContext, ArrangeResult, WidgetEventContext};
use crate::channels::ARRANGE;
use crate::controls::support::{
    ElementSlot, PointAxisExt, RectAxisExt, SizeAxisExt, Vec2AxisExt, action_numeric_value,
    action_scroll_unit, f32_from_logical, is_initial_drag_move,
};
use crate::element::{Element, ElementId};
use crate::event_runtime::{ClickEvent, DragEvent, ScrollEvent};
use crate::layouts::{Layout, LayoutSize};
use crate::scroll_runtime::{ScrollState, normalize_scroll_axis};
use crate::template::{
    TemplateContract, TemplateNode, TemplateSlot, TemplateSpec, TemplateSpecRecipe,
};
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, ControlTemplateSpec, ElementConfig, ElementSpec, Widget};

/// Selector identity for `ScrollView`.
pub const TYPE_SCROLL_VIEW: TypeTag = TypeTag(13);
/// Selector identity for `ScrollBar`.
pub const TYPE_SCROLL_BAR: TypeTag = TypeTag(14);
/// Selector identity for `ScrollArea`.
pub const TYPE_SCROLL_AREA: TypeTag = TypeTag(20);
/// Owner-local part identity for the draggable scrollbar thumb.
pub const PART_SCROLL_BAR_THUMB: PartTag = PartTag(1);
/// Semantic slot for the draggable scrollbar thumb.
pub const SLOT_SCROLL_BAR_THUMB: TemplateSlot =
    TemplateSlot::new("scrollbar-thumb").with_part_tag(PART_SCROLL_BAR_THUMB);

pub(crate) const DEFAULT_SCROLL_LINE_Y: f64 = 36.0;
const DEFAULT_SCROLL_AREA_WIDTH: f32 = 220.0;
const DEFAULT_SCROLL_AREA_HEIGHT: f32 = 160.0;
const DEFAULT_SCROLL_AREA_GAP: f32 = 8.0;
const DEFAULT_SCROLL_BAR_LENGTH: f32 = 146.0;
const DEFAULT_SCROLL_BAR_THICKNESS: f32 = 10.0;
const DEFAULT_SCROLL_BAR_THUMB_EXTENT: f32 = 30.0;

pub(crate) fn normalize_scroll_offset_y(offset_y: f64) -> f64 {
    normalize_scroll_axis(offset_y)
}

/// Builder + spec for a clipped scroll viewport.
pub struct ScrollView {
    size: Option<(f32, f32)>,
    state: ScrollState,
    element: ElementConfig,
}

impl core::fmt::Debug for ScrollView {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ScrollView")
            .field("size", &self.size)
            .field("state", &self.state)
            .field("element", &self.element)
            .finish()
    }
}

impl ScrollView {
    /// Starts a new scroll view builder.
    #[must_use]
    pub fn new() -> Self {
        let mut element = ElementConfig::default();
        element.set_control_state(ControlState::interactive().focusable(false));
        Self {
            size: None,
            state: ScrollState::new(),
            element,
        }
    }

    /// Sets the viewport's fixed logical-pixel size.
    #[must_use]
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = Some((width, height));
        self
    }

    /// Sets the initial vertical scroll offset in logical pixels.
    #[must_use]
    pub fn scroll_offset_y(self, offset: f64) -> Self {
        self.state.set_offset_y_raw(offset);
        self
    }

    /// Sets whether the scroll view starts enabled.
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
}

impl Default for ScrollView {
    fn default() -> Self {
        Self::new()
    }
}

impl AppendSpec for ScrollView {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let state = self.state;
        let id = ui.append_widget(
            parent,
            Box::new(ScrollViewWidget {
                state: state.clone(),
            }),
        );

        let mut layout = Layout::column(0.0);
        if let Some((width, height)) = self.size {
            layout = layout.logical_size(width, height);
        }
        ui.register_scroll_target(id, state);
        ui.set_layout(id, layout);
        self.element.apply_to(ui, id);
        id
    }
}

impl ElementSpec for ScrollView {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

/// Per-element widget for `ScrollView`.
#[derive(Clone, Debug)]
pub(crate) struct ScrollViewWidget {
    state: ScrollState,
}

impl Widget for ScrollViewWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_SCROLL_VIEW)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::ScrollView)
    }

    fn configure_accessibility(
        &self,
        element: &Element,
        ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        let offset = self.state.offset();
        let max = self.state.max_offset();
        node.set_scroll_x(offset.x);
        node.set_scroll_x_min(0.0);
        node.set_scroll_x_max(max.x);
        node.set_scroll_y(offset.y);
        node.set_scroll_y_min(0.0);
        node.set_scroll_y_max(max.y);

        if !ctx.is_enabled(element.id()) {
            return;
        }
        if max.x > f64::EPSILON {
            node.add_action(accesskit::Action::ScrollLeft);
            node.add_action(accesskit::Action::ScrollRight);
        }
        if max.y > f64::EPSILON {
            node.add_action(accesskit::Action::ScrollUp);
            node.add_action(accesskit::Action::ScrollDown);
        }
        if max.x > f64::EPSILON || max.y > f64::EPSILON {
            node.add_action(accesskit::Action::SetScrollOffset);
        }
    }

    fn arrange(&mut self, ctx: &mut ArrangeContext<'_>) -> ArrangeResult {
        update_scroll_metrics_from_layout(ctx, &self.state);
        ArrangeResult::UNCHANGED
    }

    fn clips_subtree(&self, _element: &Element) -> bool {
        true
    }

    fn child_layout_offset(&self, _element: &Element) -> Vec2 {
        -self.state.offset()
    }

    fn handle_scroll(&mut self, ctx: &mut WidgetEventContext<'_>, event: ScrollEvent<'_>) {
        let viewport = ctx.id();
        handle_wheel_scroll(ctx, viewport, &self.state, event);
    }

    fn handle_accessibility_action(
        &mut self,
        ctx: &mut WidgetEventContext<'_>,
        request: &accesskit::ActionRequest,
    ) -> bool {
        let viewport = ctx.id();
        if !ctx.is_enabled(viewport) {
            return false;
        }
        match request.action {
            accesskit::Action::ScrollLeft => scroll_by_accessibility_action(
                ctx,
                viewport,
                &self.state,
                Axis::Horizontal,
                -1.0,
                request,
            ),
            accesskit::Action::ScrollRight => scroll_by_accessibility_action(
                ctx,
                viewport,
                &self.state,
                Axis::Horizontal,
                1.0,
                request,
            ),
            accesskit::Action::ScrollUp => scroll_by_accessibility_action(
                ctx,
                viewport,
                &self.state,
                Axis::Vertical,
                -1.0,
                request,
            ),
            accesskit::Action::ScrollDown => scroll_by_accessibility_action(
                ctx,
                viewport,
                &self.state,
                Axis::Vertical,
                1.0,
                request,
            ),
            accesskit::Action::SetScrollOffset => {
                let Some(accesskit::ActionData::SetScrollOffset(point)) = request.data.as_ref()
                else {
                    return false;
                };
                set_scroll_offset(ctx, viewport, &self.state, Vec2::new(point.x, point.y))
            }
            _ => false,
        }
    }
}

/// Builder + spec for a viewport with optional reserved scrollbars.
///
/// `ScrollArea` owns the scroll viewport and scrollbar elements. Append
/// content to [`scroll_area_viewport`] after inserting the area.
pub struct ScrollArea {
    size: Option<(f32, f32)>,
    vertical_scrollbar: bool,
    horizontal_scrollbar: bool,
    gap: f32,
    scrollbar_thickness: f32,
    thumb_extent: f32,
    scroll_offset_y: f64,
    element: ElementConfig,
}

impl core::fmt::Debug for ScrollArea {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ScrollArea")
            .field("size", &self.size)
            .field("vertical_scrollbar", &self.vertical_scrollbar)
            .field("horizontal_scrollbar", &self.horizontal_scrollbar)
            .field("gap", &self.gap)
            .field("scrollbar_thickness", &self.scrollbar_thickness)
            .field("thumb_extent", &self.thumb_extent)
            .field("scroll_offset_y", &self.scroll_offset_y)
            .field("element", &self.element)
            .finish()
    }
}

impl ScrollArea {
    /// Starts a scroll area with a reserved vertical scrollbar.
    #[must_use]
    pub fn new() -> Self {
        let mut element = ElementConfig::default();
        element.set_control_state(ControlState::interactive().focusable(false));
        Self {
            size: None,
            vertical_scrollbar: true,
            horizontal_scrollbar: false,
            gap: DEFAULT_SCROLL_AREA_GAP,
            scrollbar_thickness: DEFAULT_SCROLL_BAR_THICKNESS,
            thumb_extent: DEFAULT_SCROLL_BAR_THUMB_EXTENT,
            scroll_offset_y: 0.0,
            element,
        }
    }

    /// Sets the total fixed logical-pixel size of the area.
    #[must_use]
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = Some((width.max(1.0), height.max(1.0)));
        self
    }

    /// Enables or disables the reserved vertical scrollbar.
    #[must_use]
    pub fn vertical_scrollbar(mut self, enabled: bool) -> Self {
        self.vertical_scrollbar = enabled;
        self
    }

    /// Enables or disables the reserved horizontal scrollbar.
    #[must_use]
    pub fn horizontal_scrollbar(mut self, enabled: bool) -> Self {
        self.horizontal_scrollbar = enabled;
        self
    }

    /// Sets the gap between the viewport and any reserved scrollbars.
    #[must_use]
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self
    }

    /// Sets the reserved scrollbar thickness.
    #[must_use]
    pub fn scrollbar_thickness(mut self, thickness: f32) -> Self {
        self.scrollbar_thickness = thickness.max(1.0);
        self
    }

    /// Sets the fixed scrollbar thumb extent.
    #[must_use]
    pub fn thumb_extent(mut self, extent: f32) -> Self {
        self.thumb_extent = extent.max(1.0);
        self
    }

    /// Sets the viewport's initial vertical scroll offset.
    #[must_use]
    pub fn scroll_offset_y(mut self, offset: f64) -> Self {
        self.scroll_offset_y = normalize_scroll_offset_y(offset);
        self
    }

    /// Sets whether the area starts enabled.
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
}

impl Default for ScrollArea {
    fn default() -> Self {
        Self::new()
    }
}

impl AppendSpec for ScrollArea {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let Self {
            size,
            vertical_scrollbar,
            horizontal_scrollbar,
            gap,
            scrollbar_thickness,
            thumb_extent,
            scroll_offset_y,
            element,
        } = self;
        let control_state = element
            .control_state()
            .unwrap_or_else(|| ControlState::interactive().focusable(false));

        let widget = ScrollAreaWidget {
            gap,
            scrollbar_thickness,
            viewport: ElementSlot::new(),
            body: ElementSlot::new(),
            vertical_bar: ElementSlot::new(),
            horizontal_bar: ElementSlot::new(),
        };
        let area = ui.append_widget(parent, Box::new(widget));

        let (width, height) =
            size.unwrap_or((DEFAULT_SCROLL_AREA_WIDTH, DEFAULT_SCROLL_AREA_HEIGHT));
        let metrics = ScrollAreaMetrics::new(
            width,
            height,
            gap,
            scrollbar_thickness,
            vertical_scrollbar,
            horizontal_scrollbar,
        );

        ui.set_layout(area, Layout::row(gap).logical_size(width, height));
        element.apply_to(ui, area);

        let body = if horizontal_scrollbar {
            let body = ui.append_widget(area, Box::new(ScrollAreaBodyWidget));
            ui.set_layout(
                body,
                Layout::column(gap)
                    .logical_size(metrics.viewport_width, metrics.body_height)
                    .flex_shrink(0.0),
            );
            set_scroll_area_body(ui, area, body);
            body
        } else {
            area
        };

        let viewport = ui.append(
            body,
            ScrollView::new()
                .size(metrics.viewport_width, metrics.viewport_height)
                .scroll_offset_y(scroll_offset_y)
                .enabled(control_state.enabled),
        );
        if horizontal_scrollbar {
            let bar = ui.append(
                body,
                ScrollBar::horizontal(viewport)
                    .length(metrics.viewport_width)
                    .thickness(scrollbar_thickness)
                    .thumb_extent(thumb_extent)
                    .enabled(control_state.enabled),
            );
            set_scroll_area_horizontal_bar(ui, area, bar);
        }
        if vertical_scrollbar {
            let bar = ui.append(
                area,
                ScrollBar::vertical(viewport)
                    .length(metrics.viewport_height)
                    .thickness(scrollbar_thickness)
                    .thumb_extent(thumb_extent)
                    .enabled(control_state.enabled),
            );
            set_scroll_area_vertical_bar(ui, area, bar);
        }
        set_scroll_area_viewport(ui, area, viewport);
        area
    }
}

impl ElementSpec for ScrollArea {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

/// Per-element widget for `ScrollArea`.
#[derive(Debug, Default)]
pub(crate) struct ScrollAreaWidget {
    gap: f32,
    scrollbar_thickness: f32,
    viewport: ElementSlot,
    body: ElementSlot,
    vertical_bar: ElementSlot,
    horizontal_bar: ElementSlot,
}

impl Widget for ScrollAreaWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_SCROLL_AREA)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::GenericContainer)
    }

    fn arrange(&mut self, ctx: &mut ArrangeContext<'_>) -> ArrangeResult {
        let Some(bounds) = ctx.effective_bounds(ctx.id()) else {
            return ArrangeResult::UNCHANGED;
        };
        let metrics = ScrollAreaMetrics::new(
            f32_from_logical(bounds.width()).max(1.0),
            f32_from_logical(bounds.height()).max(1.0),
            self.gap,
            self.scrollbar_thickness,
            self.vertical_bar.get().is_some(),
            self.horizontal_bar.get().is_some(),
        );

        let mut changed = false;
        if let Some(body) = self.body.get() {
            changed |= set_layout_size_if_changed(
                ctx.ui_mut(),
                body,
                LayoutSize::logical(metrics.viewport_width, metrics.body_height).flex_shrink(0.0),
            );
        }
        if let Some(viewport) = self.viewport.get() {
            changed |= set_layout_size_if_changed(
                ctx.ui_mut(),
                viewport,
                LayoutSize::logical(metrics.viewport_width, metrics.viewport_height),
            );
        }
        if let Some(bar) = self.vertical_bar.get() {
            changed |= set_layout_size_if_changed(
                ctx.ui_mut(),
                bar,
                scroll_bar_size(
                    Axis::Vertical,
                    metrics.viewport_height,
                    self.scrollbar_thickness,
                ),
            );
        }
        if let Some(bar) = self.horizontal_bar.get() {
            changed |= set_layout_size_if_changed(
                ctx.ui_mut(),
                bar,
                scroll_bar_size(
                    Axis::Horizontal,
                    metrics.viewport_width,
                    self.scrollbar_thickness,
                ),
            );
        }

        ArrangeResult::from_layout_changed(changed)
    }
}

#[derive(Debug, Default)]
struct ScrollAreaBodyWidget;

impl Widget for ScrollAreaBodyWidget {}

#[derive(Copy, Clone, Debug)]
struct ScrollAreaMetrics {
    viewport_width: f32,
    viewport_height: f32,
    body_height: f32,
}

impl ScrollAreaMetrics {
    fn new(
        width: f32,
        height: f32,
        gap: f32,
        thickness: f32,
        vertical_scrollbar: bool,
        horizontal_scrollbar: bool,
    ) -> Self {
        let vertical_gutter = scrollbar_gutter(vertical_scrollbar, gap, thickness);
        let horizontal_gutter = scrollbar_gutter(horizontal_scrollbar, gap, thickness);
        let viewport_width = (width - vertical_gutter).max(1.0);
        let viewport_height = (height - horizontal_gutter).max(1.0);
        Self {
            viewport_width,
            viewport_height,
            body_height: viewport_height + horizontal_gutter,
        }
    }
}

fn scrollbar_gutter(enabled: bool, gap: f32, thickness: f32) -> f32 {
    if enabled { gap + thickness } else { 0.0 }
}

fn f32_nearly_equal(a: f32, b: f32) -> bool {
    (a - b).abs() <= f32::EPSILON
}

fn set_scroll_area_viewport(ui: &Ui, area: ElementId, viewport: ElementId) {
    if let Some(widget) = scroll_area_widget(ui, area) {
        widget.viewport.set(viewport);
    }
}

fn set_scroll_area_body(ui: &Ui, area: ElementId, body: ElementId) {
    if let Some(widget) = scroll_area_widget(ui, area) {
        widget.body.set(body);
    }
}

fn set_scroll_area_vertical_bar(ui: &Ui, area: ElementId, bar: ElementId) {
    if let Some(widget) = scroll_area_widget(ui, area) {
        widget.vertical_bar.set(bar);
    }
}

fn set_scroll_area_horizontal_bar(ui: &Ui, area: ElementId, bar: ElementId) {
    if let Some(widget) = scroll_area_widget(ui, area) {
        widget.horizontal_bar.set(bar);
    }
}

fn scroll_area_widget(ui: &Ui, area: ElementId) -> Option<&ScrollAreaWidget> {
    ui.get(area)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<ScrollAreaWidget>())
}

fn set_layout_size_if_changed(ui: &mut Ui, element: ElementId, size: LayoutSize) -> bool {
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

/// Returns the internal viewport for `area`, if it is a live [`ScrollArea`].
#[must_use]
pub fn scroll_area_viewport(ui: &Ui, area: ElementId) -> Option<ElementId> {
    scroll_area_widget(ui, area).and_then(|widget| widget.viewport.get())
}

/// Returns the reserved vertical scrollbar for `area`, if it has one.
#[must_use]
pub fn scroll_area_vertical_bar(ui: &Ui, area: ElementId) -> Option<ElementId> {
    scroll_area_widget(ui, area).and_then(|widget| widget.vertical_bar.get())
}

/// Returns the reserved horizontal scrollbar for `area`, if it has one.
#[must_use]
pub fn scroll_area_horizontal_bar(ui: &Ui, area: ElementId) -> Option<ElementId> {
    scroll_area_widget(ui, area).and_then(|widget| widget.horizontal_bar.get())
}

/// Builder + spec for a scrollbar that controls a viewport's shared
/// scroll state.
pub struct ScrollBar {
    axis: Axis,
    viewport: ElementId,
    length: f32,
    thickness: f32,
    thumb_extent: f32,
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
    pub fn length(mut self, length: f32) -> Self {
        self.length = length.max(1.0);
        self.thumb_extent = self.thumb_extent.min(self.length);
        self
    }

    /// Sets the track thickness in logical pixels.
    #[must_use]
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness.max(1.0);
        self
    }

    /// Sets the fixed thumb extent in logical pixels.
    #[must_use]
    pub fn thumb_extent(mut self, extent: f32) -> Self {
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
    length: f32,
    thickness: f32,
    thumb_extent: f32,
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
        let length = f32_from_logical(size.along(self.axis)).max(1.0);
        let thickness = f32_from_logical(size.along(self.axis.cross())).max(1.0);
        let thumb_extent = self.thumb_extent.clamp(1.0, length);

        let changed = !f32_nearly_equal(self.length, length)
            || !f32_nearly_equal(self.thickness, thickness)
            || !f32_nearly_equal(self.thumb_extent, thumb_extent);
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
        if is_scroll_bar_thumb(ctx, scroll_bar, event.source) {
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
        set_axis_scroll_offset(ctx, self.viewport, &self.state, self.axis, next);
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
            event.source,
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
        set_axis_scroll_offset(ctx, self.viewport, &self.state, self.axis, next)
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

/// Returns the current vertical scroll offset for `scroll_view`.
#[must_use]
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

pub(crate) fn handle_wheel_scroll(
    ui: &mut Ui,
    viewport: ElementId,
    state: &ScrollState,
    event: ScrollEvent<'_>,
) {
    let Some(delta) = wheel_scroll_delta(ui, viewport, event) else {
        return;
    };
    scroll_by(ui, viewport, state, delta);
    event.mark_handled();
}

pub(crate) fn wheel_scroll_delta(
    ui: &Ui,
    viewport: ElementId,
    event: ScrollEvent<'_>,
) -> Option<Vec2> {
    if !matches!(event.phase, Phase::Target | Phase::Bubble) || !ui.is_enabled(viewport) {
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

fn scroll_by_accessibility_action(
    ui: &mut Ui,
    viewport: ElementId,
    state: &ScrollState,
    axis: Axis,
    direction: f64,
    request: &accesskit::ActionRequest,
) -> bool {
    let Some(unit) = action_scroll_unit(request) else {
        return false;
    };
    let amount = match unit {
        accesskit::ScrollUnit::Item => DEFAULT_SCROLL_LINE_Y,
        accesskit::ScrollUnit::Page => state.viewport_size().along(axis).max(DEFAULT_SCROLL_LINE_Y),
    };
    set_axis_scroll_offset(
        ui,
        viewport,
        state,
        axis,
        state_axis_scroll_offset(state, axis) + (amount * direction),
    )
}

pub(crate) fn set_scroll_state_offset_y(
    ui: &mut Ui,
    viewport: ElementId,
    state: &ScrollState,
    offset: f64,
) -> bool {
    set_scroll_offset(ui, viewport, state, Vec2::new(state.offset().x, offset))
}

pub(crate) fn state_scroll_offset_y(state: &ScrollState) -> f64 {
    state.offset_y()
}

#[derive(Clone, Debug)]
struct ScrollBarSnapshot {
    axis: Axis,
    viewport: ElementId,
    state: ScrollState,
    length: f32,
    thickness: f32,
    thumb_extent: f32,
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
                self.offset_from_thumb_start = f64::from(widget.thumb_extent) / 2.0;
            }
        }
        let track_start = bar_origin.along(widget.axis);
        let thumb_position =
            position.along(widget.axis) - track_start - self.offset_from_thumb_start;
        let travel = f64::from((widget.length - widget.thumb_extent).max(0.0));
        if travel <= f64::EPSILON {
            return;
        }
        let max = axis_scroll_offset(widget.state.max_offset(), widget.axis);
        let next = (thumb_position / travel).clamp(0.0, 1.0) * max;
        set_axis_scroll_offset(ui, widget.viewport, &widget.state, widget.axis, next);
    }
}

fn scroll_by(ui: &mut Ui, viewport: ElementId, state: &ScrollState, delta: Vec2) -> bool {
    set_scroll_offset(ui, viewport, state, state.offset() + delta)
}

fn set_scroll_offset(ui: &mut Ui, viewport: ElementId, state: &ScrollState, offset: Vec2) -> bool {
    let max = max_scroll_offset(ui, viewport, state);
    let clamped = Vec2::new(
        normalize_scroll_axis(offset.x).min(max.x),
        normalize_scroll_axis(offset.y).min(max.y),
    );
    if !state.set_offset_raw(clamped) {
        return true;
    }
    let channels = ui
        .scroll_target(viewport)
        .map(|target| target.offset_channels())
        .unwrap_or_else(|| ARRANGE.into());
    ui.mark_channels(viewport, channels);
    true
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

fn update_scroll_metrics_from_layout(ctx: &ArrangeContext<'_>, state: &ScrollState) {
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

fn scroll_bar_widget(ui: &Ui, scroll_bar: ElementId) -> Option<ScrollBarSnapshot> {
    ui.get(scroll_bar)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<ScrollBarWidget>())
        .map(ScrollBarSnapshot::from)
}

fn scroll_bar_layout(axis: Axis, length: f32, thickness: f32) -> Layout {
    Layout::new()
        .relative()
        .size(scroll_bar_size(axis, length, thickness))
}

fn scroll_bar_size(axis: Axis, length: f32, thickness: f32) -> LayoutSize {
    match axis {
        Axis::Horizontal => LayoutSize::logical(length, thickness),
        Axis::Vertical => LayoutSize::logical(thickness, length),
    }
}

fn scroll_bar_thumb_size(axis: Axis, thickness: f32, extent: f32) -> LayoutSize {
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
    length: f32,
    thumb_extent: f32,
) -> Vec2 {
    let max = axis_scroll_offset(state.max_offset(), axis);
    if max <= f64::EPSILON {
        return Vec2::ZERO;
    }
    let travel = f64::from((length - thumb_extent).max(0.0));
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
) -> bool {
    let next = axis.pack_vec2(offset, state.offset().along(axis.cross()));
    set_scroll_offset(ui, viewport, state, next)
}

fn state_axis_scroll_offset(state: &ScrollState, axis: Axis) -> f64 {
    axis_scroll_offset(state.offset(), axis)
}

fn axis_scroll_offset(offset: Vec2, axis: Axis) -> f64 {
    offset.along(axis)
}

fn axis_thumb_start(
    axis: Axis,
    track_origin: Point,
    offset: Vec2,
    max: Vec2,
    length: f32,
    thumb_extent: f32,
) -> f64 {
    track_origin.along(axis)
        + axis_scroll_fraction(offset.along(axis), max.along(axis))
            * f64::from((length - thumb_extent).max(0.0))
}

fn axis_scroll_fraction(offset: f64, max: f64) -> f64 {
    if max <= f64::EPSILON {
        0.0
    } else {
        (offset / max).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accessibility::accesskit_node_id;
    use crate::test_support::{pointer_down, pointer_move, pointer_scroll, pointer_up};
    use accesskit::{
        Action, ActionData, ActionRequest, Node, Orientation, Role, TreeId, TreeUpdate,
    };
    use ui_events::ScrollDelta;

    #[test]
    fn scroll_view_offsets_and_clips_children() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 100.0));
        let scroll = ui.append(root, ScrollView::new().size(120.0, 48.0));
        let first = append_scroll_test_child(&mut ui, scroll);
        append_scroll_test_child(&mut ui, scroll);
        append_scroll_test_child(&mut ui, scroll);

        ui.compute_layout(Size::new(180.0, 100.0));
        let before = ui.effective_bounds(first);
        assert!(set_scroll_view_offset_y(&mut ui, scroll, 12.0));
        ui.compute_layout(Size::new(180.0, 100.0));
        let after = ui.effective_bounds(first);
        assert!((after.y0 - (before.y0 - 12.0)).abs() <= 0.01);
        assert_eq!(scroll_view_offset_y(&ui, scroll), Some(12.0));
    }

    #[test]
    fn scroll_view_offset_reuses_layout_results() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 100.0));
        let scroll = ui.append(root, ScrollView::new().size(120.0, 48.0));
        let first = append_scroll_test_child(&mut ui, scroll);
        for _ in 0..3 {
            append_scroll_test_child(&mut ui, scroll);
        }

        ui.compute_layout(Size::new(180.0, 100.0));
        let before = ui.effective_bounds(first);
        ui.reset_frame_stats();

        assert!(set_scroll_view_offset_y(&mut ui, scroll, 12.0));
        assert!(!ui.invalidation().has_invalidated(crate::channels::LAYOUT));
        assert!(ui.invalidation().has_invalidated(ARRANGE));

        ui.compute_layout(Size::new(180.0, 100.0));
        let stats = ui.frame_stats();
        let after = ui.effective_bounds(first);

        assert_eq!(stats.layout_passes, 0);
        assert!(!stats.layout_requested);
        assert_eq!(stats.channel_marks.layout, 0);
        assert_eq!(stats.channel_marks.arrange, 1);
        assert!(stats.bounds_written > 0);
        assert!((after.y0 - (before.y0 - 12.0)).abs() <= 0.01);
    }

    #[test]
    fn scroll_view_handles_routed_wheel_delta() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 120.0));
        let scroll = ui.append(root, ScrollView::new().size(120.0, 48.0));
        for _ in 0..4 {
            append_scroll_test_child(&mut ui, scroll);
        }

        ui.compute_layout(Size::new(180.0, 120.0));
        let result = ui.synthesize_scroll(scroll, Point::ZERO, ScrollDelta::LineDelta(0.0, -1.0));
        assert!(result.handled);
        assert!(scroll_view_offset_y(&ui, scroll).is_some_and(|offset| offset > 0.0));
    }

    #[test]
    fn pointer_wheel_scroll_uses_content_direction() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 120.0));
        let scroll = ui.append(root, ScrollView::new().size(120.0, 48.0));
        for _ in 0..4 {
            append_scroll_test_child(&mut ui, scroll);
        }

        ui.compute_layout(Size::new(180.0, 120.0));
        let position = ui.effective_bounds(scroll).center();
        let result = ui.dispatch_pointer_event(&pointer_scroll(
            position,
            1,
            ScrollDelta::LineDelta(0.0, -1.0),
        ));

        assert!(result.handled);
        assert!(scroll_view_offset_y(&ui, scroll).is_some_and(|offset| offset > 0.0));

        let before = scroll_view_offset_y(&ui, scroll).unwrap();
        let result = ui.dispatch_pointer_event(&pointer_scroll(
            position,
            2,
            ScrollDelta::LineDelta(0.0, 1.0),
        ));

        assert!(result.handled);
        assert!(scroll_view_offset_y(&ui, scroll).is_some_and(|offset| offset < before));
    }

    #[test]
    fn scroll_state_tracks_viewport_and_content_metrics() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 120.0));
        let scroll = ui.append(root, ScrollView::new().size(120.0, 48.0));
        for _ in 0..4 {
            append_scroll_test_child(&mut ui, scroll);
        }

        ui.compute_layout(Size::new(180.0, 120.0));
        let state = ui
            .scroll_target(scroll)
            .map(|target| target.state())
            .expect("scroll view should own scroll state");

        assert_eq!(state.viewport_size(), Size::new(120.0, 48.0));
        assert!(state.content_size().height > state.viewport_size().height);
        assert!(state.max_offset_y() > 0.0);
    }

    #[test]
    fn removing_scroll_view_clears_scroll_target_registration() {
        let mut ui = Ui::new();
        let root = ui.root();
        let scroll = ui.append(root, ScrollView::new().size(120.0, 48.0));

        assert!(ui.scroll_target(scroll).is_some());

        ui.remove(scroll);

        assert!(ui.scroll_target(scroll).is_none());
    }

    #[test]
    fn scroll_view_accessibility_exposes_offsets_and_handles_set_offset() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 120.0));
        let scroll = ui.append(root, ScrollView::new().size(120.0, 48.0));
        for _ in 0..4 {
            append_scroll_test_child(&mut ui, scroll);
        }

        ui.compute_layout(Size::new(180.0, 120.0));
        let update = ui.accesskit_tree_update();
        let node = accesskit_node_for(&update, scroll);
        let max = node
            .scroll_y_max()
            .expect("scroll view should expose max y");

        assert_eq!(node.role(), Role::ScrollView);
        assert_eq!(node.scroll_x(), Some(0.0));
        assert_eq!(node.scroll_y(), Some(0.0));
        assert_eq!(node.scroll_y_min(), Some(0.0));
        assert!(max > 0.0);
        assert!(node.supports_action(Action::ScrollDown));
        assert!(node.supports_action(Action::SetScrollOffset));

        let handled = ui.handle_accesskit_action(&scroll_offset_action(scroll, 0.0, max * 0.5));

        assert!(handled);
        assert!(scroll_view_offset_y(&ui, scroll).is_some_and(|offset| offset > 0.0));
    }

    #[test]
    fn scroll_view_accessibility_scroll_actions_use_units() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 160.0));
        let scroll = ui.append(root, ScrollView::new().size(120.0, 48.0));
        for _ in 0..6 {
            append_scroll_test_child(&mut ui, scroll);
        }

        ui.compute_layout(Size::new(180.0, 160.0));

        assert!(ui.handle_accesskit_action(&scroll_action(
            scroll,
            Action::ScrollDown,
            accesskit::ScrollUnit::Page,
        )));
        assert!(
            scroll_view_offset_y(&ui, scroll)
                .is_some_and(|offset| { (offset - 48.0).abs() <= 0.01 })
        );

        assert!(ui.handle_accesskit_action(&scroll_action(
            scroll,
            Action::ScrollUp,
            accesskit::ScrollUnit::Item,
        )));
        assert!(
            scroll_view_offset_y(&ui, scroll)
                .is_some_and(|offset| { (offset - 12.0).abs() <= 0.01 })
        );
    }

    #[test]
    fn scroll_area_creates_bound_viewport_and_vertical_bar() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(160.0, 100.0));
        let area = ui.append(root, ScrollArea::new().size(120.0, 80.0));
        let viewport = scroll_area_viewport(&ui, area).expect("scroll area should expose viewport");
        let bar =
            scroll_area_vertical_bar(&ui, area).expect("scroll area should expose vertical bar");
        assert_eq!(scroll_area_horizontal_bar(&ui, area), None);

        for _ in 0..5 {
            append_scroll_test_child(&mut ui, viewport);
        }

        ui.compute_layout(Size::new(160.0, 100.0));
        let viewport_bounds = ui.effective_bounds(viewport);
        let bar_bounds = ui.effective_bounds(bar);

        assert_eq!(
            viewport_bounds.width(),
            120.0 - DEFAULT_SCROLL_AREA_GAP as f64 - DEFAULT_SCROLL_BAR_THICKNESS as f64
        );
        assert_eq!(viewport_bounds.height(), 80.0);
        assert!(bar_bounds.x0 > viewport_bounds.x1);

        let target = Point::new(bar_bounds.center().x, bar_bounds.y1 - 1.0);
        assert!(
            ui.dispatch_pointer_event(&pointer_down(target, 1))
                .changed()
        );
        assert!(ui.dispatch_pointer_event(&pointer_up(target, 2)).handled);
        assert!(scroll_offset_y(&ui, viewport).is_some_and(|offset| offset > 0.0));
    }

    #[test]
    fn scroll_area_children_follow_arranged_height() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(0.0).logical_size(160.0, 140.0));
        let area = ui.append(root, ScrollArea::new().size(120.0, 80.0));
        ui.set_layout(area, Layout::new().logical_size(120.0, 80.0).fill_height());
        let viewport = scroll_area_viewport(&ui, area).expect("scroll area should expose viewport");
        let bar =
            scroll_area_vertical_bar(&ui, area).expect("scroll area should expose vertical bar");

        for _ in 0..8 {
            append_scroll_test_child(&mut ui, viewport);
        }

        ui.compute_layout(Size::new(160.0, 140.0));
        let area_bounds = ui.effective_bounds(area);
        let viewport_bounds = ui.effective_bounds(viewport);
        let bar_bounds = ui.effective_bounds(bar);

        assert_eq!(area_bounds.height(), 140.0);
        assert_eq!(viewport_bounds.height(), 140.0);
        assert_eq!(bar_bounds.height(), 140.0);
        assert_eq!(
            viewport_bounds.width(),
            120.0 - DEFAULT_SCROLL_AREA_GAP as f64 - DEFAULT_SCROLL_BAR_THICKNESS as f64
        );
        assert!(
            scroll_offset_y(&ui, viewport).is_some(),
            "arranged viewport should still be registered as scrollable"
        );
    }

    #[test]
    fn scroll_area_can_reserve_horizontal_bar_only() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(160.0, 100.0));
        let area = ui.append(
            root,
            ScrollArea::new()
                .size(120.0, 80.0)
                .vertical_scrollbar(false)
                .horizontal_scrollbar(true),
        );
        let viewport = scroll_area_viewport(&ui, area).expect("scroll area should expose viewport");
        let bar = scroll_area_horizontal_bar(&ui, area)
            .expect("scroll area should expose horizontal bar");

        assert_eq!(scroll_area_vertical_bar(&ui, area), None);

        ui.compute_layout(Size::new(160.0, 100.0));
        let viewport_bounds = ui.effective_bounds(viewport);
        let bar_bounds = ui.effective_bounds(bar);

        assert_eq!(viewport_bounds.width(), 120.0);
        assert_eq!(
            viewport_bounds.height(),
            80.0 - DEFAULT_SCROLL_AREA_GAP as f64 - DEFAULT_SCROLL_BAR_THICKNESS as f64
        );
        assert!(bar_bounds.y0 > viewport_bounds.y1);
    }

    #[test]
    fn scroll_bar_thumb_tracks_target_scroll_state() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(8.0).logical_size(180.0, 80.0));
        let scroll = ui.append(root, ScrollView::new().size(100.0, 48.0));
        for _ in 0..4 {
            append_scroll_test_child(&mut ui, scroll);
        }
        let bar = ui.append(
            root,
            ScrollBar::vertical(scroll)
                .length(48.0)
                .thickness(8.0)
                .thumb_extent(12.0),
        );
        let thumb = ui
            .slot(bar, SLOT_SCROLL_BAR_THUMB)
            .expect("scrollbar template should create a thumb slot");

        ui.compute_layout(Size::new(180.0, 80.0));
        let before = ui.effective_bounds(thumb);
        assert!(set_scroll_view_offset_y(&mut ui, scroll, 24.0));
        ui.compute_layout(Size::new(180.0, 80.0));
        let after = ui.effective_bounds(thumb);

        assert!(after.y0 > before.y0);
    }

    #[test]
    fn scroll_bar_track_click_pages_viewport() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(8.0).logical_size(180.0, 80.0));
        let scroll = ui.append(root, ScrollView::new().size(100.0, 48.0));
        for _ in 0..4 {
            append_scroll_test_child(&mut ui, scroll);
        }
        let bar = ui.append(
            root,
            ScrollBar::vertical(scroll)
                .length(48.0)
                .thickness(8.0)
                .thumb_extent(12.0),
        );

        ui.compute_layout(Size::new(180.0, 80.0));
        let bar_bounds = ui.effective_bounds(bar);
        let result =
            ui.synthesize_click(bar, Point::new(bar_bounds.center().x, bar_bounds.y1 - 1.0));

        assert!(result.handled);
        assert!(scroll_view_offset_y(&ui, scroll).is_some_and(|offset| offset > 0.0));
    }

    #[test]
    fn scroll_bar_track_pointer_click_pages_viewport() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(8.0).logical_size(180.0, 80.0));
        let scroll = ui.append(root, ScrollView::new().size(100.0, 48.0));
        for _ in 0..4 {
            append_scroll_test_child(&mut ui, scroll);
        }
        let bar = ui.append(
            root,
            ScrollBar::vertical(scroll)
                .length(48.0)
                .thickness(8.0)
                .thumb_extent(12.0),
        );

        ui.compute_layout(Size::new(180.0, 80.0));
        let bar_bounds = ui.effective_bounds(bar);
        let target = Point::new(bar_bounds.center().x, bar_bounds.y1 - 1.0);

        assert!(
            ui.dispatch_pointer_event(&pointer_down(target, 1))
                .changed()
        );
        assert!(ui.dispatch_pointer_event(&pointer_up(target, 2)).handled);
        assert!(scroll_view_offset_y(&ui, scroll).is_some_and(|offset| offset > 0.0));
    }

    #[test]
    fn scroll_bar_thumb_drag_updates_viewport_offset() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(8.0).logical_size(180.0, 80.0));
        let scroll = ui.append(root, ScrollView::new().size(100.0, 48.0));
        for _ in 0..4 {
            append_scroll_test_child(&mut ui, scroll);
        }
        let bar = ui.append(
            root,
            ScrollBar::vertical(scroll)
                .length(48.0)
                .thickness(8.0)
                .thumb_extent(12.0),
        );
        let thumb = ui
            .slot(bar, SLOT_SCROLL_BAR_THUMB)
            .expect("scrollbar template should create a thumb slot");

        ui.compute_layout(Size::new(180.0, 80.0));
        let start = ui.effective_bounds(thumb).center();
        assert!(ui.dispatch_pointer_event(&pointer_down(start, 1)).changed());
        assert!(
            ui.dispatch_pointer_event(&pointer_move(Point::new(start.x, start.y + 20.0), 2, true,))
                .changed()
        );

        assert!(scroll_view_offset_y(&ui, scroll).is_some_and(|offset| offset > 0.0));
    }

    #[test]
    fn scroll_bar_accessibility_exposes_range_and_set_value_action() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(8.0).logical_size(180.0, 80.0));
        let scroll = ui.append(root, ScrollView::new().size(100.0, 48.0));
        for _ in 0..4 {
            append_scroll_test_child(&mut ui, scroll);
        }
        let bar = ui.append(
            root,
            ScrollBar::vertical(scroll)
                .length(48.0)
                .thickness(8.0)
                .thumb_extent(12.0),
        );

        ui.compute_layout(Size::new(180.0, 80.0));
        let update = ui.accesskit_tree_update();
        let node = accesskit_node_for(&update, bar);
        let max = node
            .max_numeric_value()
            .expect("scrollbar should expose max");

        assert_eq!(node.role(), Role::ScrollBar);
        assert_eq!(node.orientation(), Some(Orientation::Vertical));
        assert_eq!(node.numeric_value(), Some(0.0));
        assert!(max > 0.0);
        assert_eq!(node.min_numeric_value(), Some(0.0));
        assert_eq!(node.numeric_value_step(), Some(DEFAULT_SCROLL_LINE_Y));
        assert!(node.supports_action(Action::Increment));
        assert!(node.supports_action(Action::Decrement));
        assert!(node.supports_action(Action::SetValue));

        let handled = ui.handle_accesskit_action(&numeric_action(bar, Action::SetValue, max * 0.5));

        assert!(handled);
        assert!(scroll_view_offset_y(&ui, scroll).is_some_and(|offset| offset > 0.0));
    }

    #[test]
    fn scroll_bar_uses_default_cursor() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(8.0).logical_size(180.0, 80.0));
        let scroll = ui.append(root, ScrollView::new().size(100.0, 48.0));
        for _ in 0..4 {
            append_scroll_test_child(&mut ui, scroll);
        }
        let bar = ui.append(
            root,
            ScrollBar::vertical(scroll)
                .length(48.0)
                .thickness(8.0)
                .thumb_extent(12.0),
        );

        ui.compute_layout(Size::new(180.0, 80.0));
        let bar_bounds = ui.effective_bounds(bar);
        let result = ui.dispatch_pointer_event(&pointer_move(bar_bounds.center(), 1, false));

        assert!(result.hover_changed);
        assert_eq!(ui.cursor_icon(), None);
    }

    fn append_scroll_test_child(ui: &mut Ui, scroll: ElementId) -> ElementId {
        let child = ui.insert_child(scroll);
        ui.set_layout(
            child,
            Layout::new().logical_size(100.0, 24.0).flex_shrink(0.0),
        );
        child
    }

    fn numeric_action(element: ElementId, action: Action, value: f64) -> ActionRequest {
        ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: accesskit_node_id(element),
            data: Some(ActionData::NumericValue(value)),
        }
    }

    fn scroll_action(
        element: ElementId,
        action: Action,
        unit: accesskit::ScrollUnit,
    ) -> ActionRequest {
        ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: accesskit_node_id(element),
            data: Some(ActionData::ScrollUnit(unit)),
        }
    }

    fn scroll_offset_action(element: ElementId, x: f64, y: f64) -> ActionRequest {
        ActionRequest {
            action: Action::SetScrollOffset,
            target_tree: TreeId::ROOT,
            target_node: accesskit_node_id(element),
            data: Some(ActionData::SetScrollOffset(accesskit::Point::new(x, y))),
        }
    }

    fn accesskit_node_for(update: &TreeUpdate, element: ElementId) -> &Node {
        update
            .nodes
            .iter()
            .find(|(id, _)| *id == accesskit_node_id(element))
            .map(|(_, node)| node)
            .expect("node should exist")
    }
}
