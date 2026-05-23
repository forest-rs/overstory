// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `ScrollView`: reusable clipped viewport state and behavior.

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::Cell;

use cursor_icon::CursorIcon;
use kurbo::Point;
use kurbo::Vec2;
use understory_style::{PartTag, TypeTag};

use crate::Phase;
use crate::behavior::LayoutContext;
use crate::channels::LAYOUT;
use crate::element::{Element, ElementId};
use crate::layouts::Layout;
use crate::template::{Template, TemplateContext, TemplateSlot};
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, SurfacePartWidget, TemplateOverrideWidget, Widget};

/// Selector identity for `ScrollView`.
pub const TYPE_SCROLL_VIEW: TypeTag = TypeTag(13);
/// Selector identity for `ScrollBar`.
pub const TYPE_SCROLL_BAR: TypeTag = TypeTag(14);
/// Owner-local part identity for the draggable scrollbar thumb.
pub const PART_SCROLL_BAR_THUMB: PartTag = PartTag(1);
/// Semantic slot for the draggable scrollbar thumb.
pub const SLOT_SCROLL_BAR_THUMB: TemplateSlot =
    TemplateSlot::new("scrollbar-thumb").with_part_tag(PART_SCROLL_BAR_THUMB);

const DEFAULT_SCROLL_LINE_Y: f64 = 36.0;
const DEFAULT_SCROLL_BAR_LENGTH: f32 = 146.0;
const DEFAULT_SCROLL_BAR_THICKNESS: f32 = 10.0;
const DEFAULT_SCROLL_BAR_THUMB_EXTENT: f32 = 30.0;

/// Axis a scroll bar manipulates.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ScrollAxis {
    /// Horizontal scrolling.
    Horizontal,
    /// Vertical scrolling.
    #[default]
    Vertical,
}

/// Shared scroll offset state for viewport-like controls.
#[derive(Clone)]
pub struct ScrollState {
    inner: Rc<ScrollStateInner>,
}

struct ScrollStateInner {
    offset_x: Cell<f64>,
    offset_y: Cell<f64>,
    viewport_width: Cell<f64>,
    viewport_height: Cell<f64>,
    content_width: Cell<f64>,
    content_height: Cell<f64>,
}

impl core::fmt::Debug for ScrollState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ScrollState")
            .field("offset", &self.offset())
            .finish()
    }
}

impl Default for ScrollState {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollState {
    /// Creates shared scroll state with a zero offset.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Rc::new(ScrollStateInner {
                offset_x: Cell::new(0.0),
                offset_y: Cell::new(0.0),
                viewport_width: Cell::new(0.0),
                viewport_height: Cell::new(0.0),
                content_width: Cell::new(0.0),
                content_height: Cell::new(0.0),
            }),
        }
    }

    /// Creates shared scroll state with an initial vertical offset.
    #[must_use]
    pub fn with_offset_y(offset_y: f64) -> Self {
        let state = Self::new();
        state.set_offset_y_raw(offset_y);
        state
    }

    /// Returns the current scroll offset in logical pixels.
    #[must_use]
    pub fn offset(&self) -> Vec2 {
        Vec2::new(self.inner.offset_x.get(), self.inner.offset_y.get())
    }

    /// Returns the current vertical scroll offset in logical pixels.
    #[must_use]
    pub fn offset_y(&self) -> f64 {
        self.inner.offset_y.get()
    }

    /// Returns the current maximum scroll offset in logical pixels.
    #[must_use]
    pub fn max_offset(&self) -> Vec2 {
        Vec2::new(
            (self.inner.content_width.get() - self.inner.viewport_width.get()).max(0.0),
            (self.inner.content_height.get() - self.inner.viewport_height.get()).max(0.0),
        )
    }

    /// Returns the current maximum vertical scroll offset in logical pixels.
    #[must_use]
    pub fn max_offset_y(&self) -> f64 {
        self.max_offset().y
    }

    /// Returns the latest viewport size in logical pixels.
    #[must_use]
    pub fn viewport_size(&self) -> kurbo::Size {
        kurbo::Size::new(
            self.inner.viewport_width.get(),
            self.inner.viewport_height.get(),
        )
    }

    /// Returns the latest scrollable content size in logical pixels.
    #[must_use]
    pub fn content_size(&self) -> kurbo::Size {
        kurbo::Size::new(
            self.inner.content_width.get(),
            self.inner.content_height.get(),
        )
    }

    pub(crate) fn set_offset_raw(&self, offset: Vec2) -> bool {
        let offset = Vec2::new(
            normalize_scroll_axis(offset.x),
            normalize_scroll_axis(offset.y),
        );
        if (self.inner.offset_x.get() - offset.x).abs() <= f64::EPSILON
            && (self.inner.offset_y.get() - offset.y).abs() <= f64::EPSILON
        {
            return false;
        }
        self.inner.offset_x.set(offset.x);
        self.inner.offset_y.set(offset.y);
        true
    }

    pub(crate) fn set_offset_y_raw(&self, offset_y: f64) -> bool {
        self.set_offset_raw(Vec2::new(self.inner.offset_x.get(), offset_y))
    }

    fn set_metrics(&self, viewport: kurbo::Size, content: kurbo::Size) {
        self.inner
            .viewport_width
            .set(normalize_scroll_axis(viewport.width));
        self.inner
            .viewport_height
            .set(normalize_scroll_axis(viewport.height));
        self.inner
            .content_width
            .set(normalize_scroll_axis(content.width));
        self.inner
            .content_height
            .set(normalize_scroll_axis(content.height));
    }
}

pub(crate) fn normalize_scroll_offset_y(offset_y: f64) -> f64 {
    normalize_scroll_axis(offset_y)
}

/// Builder + spec for a clipped scroll viewport.
pub struct ScrollView {
    size: Option<(f32, f32)>,
    state: ScrollState,
    control_state: ControlState,
}

impl core::fmt::Debug for ScrollView {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ScrollView")
            .field("size", &self.size)
            .field("state", &self.state)
            .field("control_state", &self.control_state)
            .finish()
    }
}

impl ScrollView {
    /// Starts a new scroll view builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            size: None,
            state: ScrollState::new(),
            control_state: ControlState::interactive().focusable(false),
        }
    }

    /// Sets the viewport's fixed logical-pixel size.
    #[must_use]
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = Some((width, height));
        self
    }

    /// Uses caller-provided shared scroll state.
    #[must_use]
    pub fn state(mut self, state: ScrollState) -> Self {
        self.state = state;
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
        self.control_state = self.control_state.enabled(enabled);
        self
    }

    /// Convenience for [`Self::enabled(false)`].
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
        ui.set_layout(id, layout);
        ui.set_control_state(id, self.control_state);
        install_wheel_scroll_behavior(ui, id, state);
        id
    }
}

/// Per-element widget for `ScrollView`.
#[derive(Clone, Debug)]
pub struct ScrollViewWidget {
    state: ScrollState,
}

impl Widget for ScrollViewWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_SCROLL_VIEW)
    }

    fn layout_updated(&self, element: &Element, ctx: &LayoutContext<'_>) {
        update_scroll_metrics_from_layout(element, ctx, &self.state);
    }

    fn clips_subtree(&self, element: &Element) -> bool {
        let _ = element;
        true
    }

    fn child_layout_offset(&self, element: &Element) -> Vec2 {
        let _ = element;
        -self.state.offset()
    }
}

/// Builder + spec for a scrollbar that controls a viewport's shared
/// [`ScrollState`].
pub struct ScrollBar {
    axis: ScrollAxis,
    viewport: ElementId,
    state: ScrollState,
    length: f32,
    thickness: f32,
    thumb_extent: f32,
    control_state: ControlState,
    template: Option<&'static dyn Template>,
}

impl core::fmt::Debug for ScrollBar {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ScrollBar")
            .field("axis", &self.axis)
            .field("viewport", &self.viewport)
            .field("state", &self.state)
            .field("length", &self.length)
            .field("thickness", &self.thickness)
            .field("thumb_extent", &self.thumb_extent)
            .field("control_state", &self.control_state)
            .finish_non_exhaustive()
    }
}

impl ScrollBar {
    /// Starts a vertical scrollbar bound to `viewport` and `state`.
    #[must_use]
    pub fn vertical(viewport: ElementId, state: ScrollState) -> Self {
        Self::new(ScrollAxis::Vertical, viewport, state)
    }

    /// Starts a horizontal scrollbar bound to `viewport` and `state`.
    #[must_use]
    pub fn horizontal(viewport: ElementId, state: ScrollState) -> Self {
        Self::new(ScrollAxis::Horizontal, viewport, state)
    }

    /// Starts a scrollbar on `axis` bound to `viewport` and `state`.
    #[must_use]
    pub fn new(axis: ScrollAxis, viewport: ElementId, state: ScrollState) -> Self {
        Self {
            axis,
            viewport,
            state,
            length: DEFAULT_SCROLL_BAR_LENGTH,
            thickness: DEFAULT_SCROLL_BAR_THICKNESS,
            thumb_extent: DEFAULT_SCROLL_BAR_THUMB_EXTENT,
            control_state: ControlState::interactive().focusable(false),
            template: None,
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
        self.control_state = self.control_state.enabled(enabled);
        self
    }

    /// Convenience for [`Self::enabled(false)`].
    #[must_use]
    pub fn disabled(self) -> Self {
        self.enabled(false)
    }

    /// Overrides the visual template used when this scrollbar is inserted.
    ///
    /// Custom templates should create [`SLOT_SCROLL_BAR_THUMB`] when they want
    /// the built-in thumb positioning behavior.
    #[must_use]
    pub fn template(mut self, template: &'static dyn Template) -> Self {
        self.template = Some(template);
        self
    }
}

impl AppendSpec for ScrollBar {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let widget = ScrollBarWidget {
            axis: self.axis,
            viewport: self.viewport,
            state: self.state,
            length: self.length,
            thickness: self.thickness,
            thumb_extent: self.thumb_extent,
        };
        let boxed: Box<dyn Widget> = match self.template {
            Some(template) => Box::new(TemplateOverrideWidget::new(widget, template)),
            None => Box::new(widget),
        };
        let id = ui.append_widget_deferred_template(parent, boxed);
        ui.set_layout(
            id,
            scroll_bar_layout(self.axis, self.length, self.thickness),
        );
        ui.set_control_state(id, self.control_state);
        ui.expand_default_template(id);
        sync_scroll_bar_thumb_size(ui, id);
        install_scroll_bar_behavior(ui, id);
        id
    }
}

/// Per-element widget for `ScrollBar`.
#[derive(Clone, Debug)]
pub struct ScrollBarWidget {
    axis: ScrollAxis,
    viewport: ElementId,
    state: ScrollState,
    length: f32,
    thickness: f32,
    thumb_extent: f32,
}

impl Widget for ScrollBarWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_SCROLL_BAR)
    }

    fn default_template(&self) -> Option<&'static dyn Template> {
        Some(&SCROLL_BAR_TEMPLATE)
    }

    fn cursor_icon(&self, element: &Element) -> Option<CursorIcon> {
        let _ = element;
        Some(CursorIcon::AllScroll)
    }

    fn clips_subtree(&self, element: &Element) -> bool {
        let _ = element;
        true
    }

    fn child_layout_offset(&self, element: &Element) -> Vec2 {
        let _ = element;
        scroll_bar_thumb_offset(self.axis, &self.state, self.length, self.thumb_extent)
    }
}

static SCROLL_BAR_TEMPLATE: ScrollBarTemplate = ScrollBarTemplate;

#[derive(Debug, Default)]
struct ScrollBarTemplate;

impl Template for ScrollBarTemplate {
    fn instantiate(&self, cx: &mut TemplateContext<'_>) {
        let thumb = cx.slot_widget(SLOT_SCROLL_BAR_THUMB, Box::new(SurfacePartWidget));
        cx.set_local(
            thumb,
            cx.properties().layout,
            scroll_bar_thumb_layout(
                ScrollAxis::Vertical,
                DEFAULT_SCROLL_BAR_THICKNESS,
                DEFAULT_SCROLL_BAR_THUMB_EXTENT,
            ),
        );
    }
}

/// Returns the current vertical scroll offset for `scroll_view`.
#[must_use]
pub fn scroll_view_offset_y(ui: &Ui, scroll_view: ElementId) -> Option<f64> {
    scroll_view_state(ui, scroll_view).map(|state| state.offset_y())
}

/// Sets the vertical scroll offset for `scroll_view`.
pub fn set_scroll_view_offset_y(ui: &mut Ui, scroll_view: ElementId, offset: f64) -> bool {
    let Some(state) = scroll_view_state(ui, scroll_view) else {
        return false;
    };
    set_scroll_offset_y(ui, scroll_view, &state, offset)
}

/// Returns a cloned scroll state for `scroll_view`.
#[must_use]
pub fn scroll_view_state(ui: &Ui, scroll_view: ElementId) -> Option<ScrollState> {
    ui.get(scroll_view)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<ScrollViewWidget>())
        .map(|widget| widget.state.clone())
}

pub(crate) fn install_wheel_scroll_behavior(ui: &mut Ui, viewport: ElementId, state: ScrollState) {
    ui.on_control_scroll(viewport, move |ui, event| {
        if !matches!(event.phase, Phase::Target | Phase::Bubble) || !ui.is_enabled(viewport) {
            return;
        }
        let bounds = ui.effective_bounds(viewport);
        let page = Vec2::new(bounds.width(), bounds.height());
        let delta = event.logical_delta(
            Vec2::new(DEFAULT_SCROLL_LINE_Y, DEFAULT_SCROLL_LINE_Y),
            page,
        );
        if delta != Vec2::ZERO {
            scroll_by(ui, viewport, &state, delta);
            event.mark_handled();
        }
    });
}

pub(crate) fn set_scroll_offset_y(
    ui: &mut Ui,
    viewport: ElementId,
    state: &ScrollState,
    offset: f64,
) -> bool {
    set_scroll_offset(ui, viewport, state, Vec2::new(state.offset().x, offset))
}

pub(crate) fn scroll_child_into_view(
    ui: &mut Ui,
    viewport: ElementId,
    state: &ScrollState,
    child: ElementId,
) -> bool {
    if !is_direct_child(ui, viewport, child) {
        return false;
    }
    let current = state.offset_y();
    let viewport_bounds = ui.effective_bounds(viewport);
    let child_bounds = ui.effective_bounds(child);
    if viewport_bounds.width() <= 0.0
        || viewport_bounds.height() <= 0.0
        || child_bounds.height() <= 0.0
    {
        return false;
    }

    let next = if child_bounds.y0 < viewport_bounds.y0 {
        current - (viewport_bounds.y0 - child_bounds.y0)
    } else if child_bounds.y1 > viewport_bounds.y1 {
        current + (child_bounds.y1 - viewport_bounds.y1)
    } else {
        current
    };
    set_scroll_offset_y(ui, viewport, state, next)
}

pub(crate) fn scroll_offset_y(state: &ScrollState) -> f64 {
    state.offset_y()
}

pub(crate) fn update_viewport_metrics_from_layout(
    element: &Element,
    ctx: &LayoutContext<'_>,
    state: &ScrollState,
) {
    update_scroll_metrics_from_layout(element, ctx, state);
}

fn install_scroll_bar_behavior(ui: &mut Ui, scroll_bar: ElementId) {
    ui.on_control_click(scroll_bar, move |ui, event| {
        if !matches!(event.phase, Phase::Target | Phase::Bubble) || !ui.is_enabled(scroll_bar) {
            return;
        }
        let Some(widget) = scroll_bar_widget(ui, scroll_bar) else {
            return;
        };
        if is_scroll_bar_thumb(ui, scroll_bar, event.source) {
            return;
        }
        let page = scroll_bar_page(&widget.state, widget.axis);
        let current = state_axis_scroll_offset(&widget.state, widget.axis);
        let thumb_start = axis_thumb_start(
            widget.axis,
            ui.effective_bounds(scroll_bar).origin(),
            widget.state.offset(),
            widget.state.max_offset(),
            widget.length,
            widget.thumb_extent,
        );
        let pointer = axis_point(widget.axis, event.position);
        let next = if pointer < thumb_start {
            current - page
        } else {
            current + page
        };
        set_axis_scroll_offset(ui, widget.viewport, &widget.state, widget.axis, next);
        event.mark_handled();
    });

    let mut drag = ScrollBarDragState::default();
    ui.on_control_drag(scroll_bar, move |ui, event| {
        if !matches!(event.phase, Phase::Target | Phase::Bubble) || !ui.is_enabled(scroll_bar) {
            return;
        }
        let Some(widget) = scroll_bar_widget(ui, scroll_bar) else {
            return;
        };
        drag.update(
            ui,
            scroll_bar,
            &widget,
            event.source,
            event.press_position,
            event.position,
            event.delta,
        );
        event.mark_handled();
    });
}

#[derive(Clone, Debug)]
struct ScrollBarSnapshot {
    axis: ScrollAxis,
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
        if is_first_drag_move(press_position, position, delta) {
            let thumb_start = axis_thumb_start(
                widget.axis,
                bar_origin,
                widget.state.offset(),
                widget.state.max_offset(),
                widget.length,
                widget.thumb_extent,
            );
            if is_scroll_bar_thumb(ui, scroll_bar, source) {
                self.offset_from_thumb_start =
                    axis_point(widget.axis, press_position) - thumb_start;
            } else {
                self.offset_from_thumb_start = f64::from(widget.thumb_extent) / 2.0;
            }
        }
        let track_start = axis_point(widget.axis, bar_origin);
        let thumb_position =
            axis_point(widget.axis, position) - track_start - self.offset_from_thumb_start;
        let travel = f64::from((widget.length - widget.thumb_extent).max(0.0));
        if travel <= f64::EPSILON {
            return;
        }
        let max = axis_scroll_offset(widget.state.max_offset(), widget.axis);
        let next = (thumb_position / travel).clamp(0.0, 1.0) * max;
        set_axis_scroll_offset(ui, widget.viewport, &widget.state, widget.axis, next);
    }
}

fn is_first_drag_move(press_position: Point, position: Point, delta: Vec2) -> bool {
    delta == position - press_position
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
    ui.mark_channels(viewport, LAYOUT.into());
    true
}

fn max_scroll_offset(ui: &Ui, viewport: ElementId, state: &ScrollState) -> Vec2 {
    let viewport_bounds = ui.effective_bounds(viewport);
    if viewport_bounds.width() <= 0.0 || viewport_bounds.height() <= 0.0 {
        return Vec2::ZERO;
    }
    if let Some(element) = ui.get(viewport) {
        let child_bounds = element
            .children()
            .iter()
            .map(|child| ui.effective_bounds(*child));
        update_scroll_metrics_from_bounds(viewport_bounds, state, child_bounds);
    }
    state.max_offset()
}

fn update_scroll_metrics_from_layout(
    element: &Element,
    ctx: &LayoutContext<'_>,
    state: &ScrollState,
) {
    let Some(viewport_bounds) = ctx.effective_bounds(element.id()) else {
        return;
    };
    let child_bounds = element
        .children()
        .iter()
        .filter_map(|child| ctx.effective_bounds(*child));
    update_scroll_metrics_from_bounds(viewport_bounds, state, child_bounds);
}

fn update_scroll_metrics_from_bounds(
    viewport_bounds: kurbo::Rect,
    state: &ScrollState,
    child_bounds: impl IntoIterator<Item = kurbo::Rect>,
) {
    if viewport_bounds.width() <= 0.0 || viewport_bounds.height() <= 0.0 {
        state.set_metrics(kurbo::Size::ZERO, kurbo::Size::ZERO);
        return;
    }
    let offset = state.offset();
    let mut content_right = 0.0_f64;
    let mut content_bottom = 0.0_f64;
    for child_bounds in child_bounds {
        content_right = content_right.max(child_bounds.x1 + offset.x - viewport_bounds.x0);
        content_bottom = content_bottom.max(child_bounds.y1 + offset.y - viewport_bounds.y0);
    }
    state.set_metrics(
        kurbo::Size::new(viewport_bounds.width(), viewport_bounds.height()),
        kurbo::Size::new(content_right, content_bottom),
    );
}

fn scroll_bar_widget(ui: &Ui, scroll_bar: ElementId) -> Option<ScrollBarSnapshot> {
    ui.get(scroll_bar)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<ScrollBarWidget>())
        .map(ScrollBarSnapshot::from)
}

fn scroll_bar_layout(axis: ScrollAxis, length: f32, thickness: f32) -> Layout {
    match axis {
        ScrollAxis::Horizontal => Layout::new().relative().logical_size(length, thickness),
        ScrollAxis::Vertical => Layout::new().relative().logical_size(thickness, length),
    }
}

fn scroll_bar_thumb_layout(axis: ScrollAxis, thickness: f32, extent: f32) -> Layout {
    match axis {
        ScrollAxis::Horizontal => Layout::new()
            .absolute_at(0.0, 0.0)
            .logical_size(extent, thickness),
        ScrollAxis::Vertical => Layout::new()
            .absolute_at(0.0, 0.0)
            .logical_size(thickness, extent),
    }
}

fn sync_scroll_bar_thumb_size(ui: &mut Ui, scroll_bar: ElementId) {
    let Some(widget) = scroll_bar_widget(ui, scroll_bar) else {
        return;
    };
    let Some(thumb) = ui.slot(scroll_bar, SLOT_SCROLL_BAR_THUMB) else {
        return;
    };
    ui.set_local(
        thumb,
        ui.properties().layout,
        scroll_bar_thumb_layout(widget.axis, widget.thickness, widget.thumb_extent),
    );
}

fn scroll_bar_thumb_offset(
    axis: ScrollAxis,
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
    match axis {
        ScrollAxis::Horizontal => Vec2::new(offset, 0.0),
        ScrollAxis::Vertical => Vec2::new(0.0, offset),
    }
}

fn is_scroll_bar_thumb(ui: &Ui, scroll_bar: ElementId, source: ElementId) -> bool {
    ui.slot(scroll_bar, SLOT_SCROLL_BAR_THUMB) == Some(source)
}

fn scroll_bar_page(state: &ScrollState, axis: ScrollAxis) -> f64 {
    let viewport = state.viewport_size();
    match axis {
        ScrollAxis::Horizontal => viewport.width,
        ScrollAxis::Vertical => viewport.height,
    }
    .max(DEFAULT_SCROLL_LINE_Y)
}

fn set_axis_scroll_offset(
    ui: &mut Ui,
    viewport: ElementId,
    state: &ScrollState,
    axis: ScrollAxis,
    offset: f64,
) -> bool {
    match axis {
        ScrollAxis::Horizontal => {
            set_scroll_offset(ui, viewport, state, Vec2::new(offset, state.offset().y))
        }
        ScrollAxis::Vertical => set_scroll_offset_y(ui, viewport, state, offset),
    }
}

fn state_axis_scroll_offset(state: &ScrollState, axis: ScrollAxis) -> f64 {
    axis_scroll_offset(state.offset(), axis)
}

fn axis_scroll_offset(offset: Vec2, axis: ScrollAxis) -> f64 {
    match axis {
        ScrollAxis::Horizontal => offset.x,
        ScrollAxis::Vertical => offset.y,
    }
}

fn axis_thumb_start(
    axis: ScrollAxis,
    track_origin: Point,
    offset: Vec2,
    max: Vec2,
    length: f32,
    thumb_extent: f32,
) -> f64 {
    axis_point(axis, track_origin)
        + axis_scroll_fraction(
            axis_scroll_offset(offset, axis),
            axis_scroll_offset(max, axis),
        ) * f64::from((length - thumb_extent).max(0.0))
}

fn axis_scroll_fraction(offset: f64, max: f64) -> f64 {
    if max <= f64::EPSILON {
        0.0
    } else {
        (offset / max).clamp(0.0, 1.0)
    }
}

fn axis_point(axis: ScrollAxis, point: Point) -> f64 {
    match axis {
        ScrollAxis::Horizontal => point.x,
        ScrollAxis::Vertical => point.y,
    }
}

fn is_direct_child(ui: &Ui, parent: ElementId, child: ElementId) -> bool {
    ui.get(child)
        .is_some_and(|element| element.parent() == Some(parent))
}

fn normalize_scroll_axis(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

        ui.compute_layout(kurbo::Size::new(180.0, 100.0));
        let before = ui.effective_bounds(first);
        assert!(set_scroll_view_offset_y(&mut ui, scroll, 12.0));
        ui.compute_layout(kurbo::Size::new(180.0, 100.0));
        let after = ui.effective_bounds(first);
        assert!((after.y0 - (before.y0 - 12.0)).abs() <= 0.01);
        assert_eq!(scroll_view_offset_y(&ui, scroll), Some(12.0));
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

        ui.compute_layout(kurbo::Size::new(180.0, 120.0));
        let result = ui.synthesize_scroll(scroll, Point::ZERO, ScrollDelta::LineDelta(0.0, 1.0));
        assert!(result.handled);
        assert!(scroll_view_offset_y(&ui, scroll).is_some_and(|offset| offset > 0.0));
    }

    #[test]
    fn scroll_state_tracks_viewport_and_content_metrics() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 120.0));
        let state = ScrollState::new();
        let scroll = ui.append(
            root,
            ScrollView::new().size(120.0, 48.0).state(state.clone()),
        );
        for _ in 0..4 {
            append_scroll_test_child(&mut ui, scroll);
        }

        ui.compute_layout(kurbo::Size::new(180.0, 120.0));

        assert_eq!(state.viewport_size(), kurbo::Size::new(120.0, 48.0));
        assert!(state.content_size().height > state.viewport_size().height);
        assert!(state.max_offset_y() > 0.0);
    }

    #[test]
    fn scroll_bar_thumb_tracks_shared_scroll_state() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(8.0).logical_size(180.0, 80.0));
        let state = ScrollState::new();
        let scroll = ui.append(
            root,
            ScrollView::new().size(100.0, 48.0).state(state.clone()),
        );
        for _ in 0..4 {
            append_scroll_test_child(&mut ui, scroll);
        }
        let bar = ui.append(
            root,
            ScrollBar::vertical(scroll, state)
                .length(48.0)
                .thickness(8.0)
                .thumb_extent(12.0),
        );
        let thumb = ui
            .slot(bar, SLOT_SCROLL_BAR_THUMB)
            .expect("scrollbar template should create a thumb slot");

        ui.compute_layout(kurbo::Size::new(180.0, 80.0));
        let before = ui.effective_bounds(thumb);
        assert!(set_scroll_view_offset_y(&mut ui, scroll, 24.0));
        ui.compute_layout(kurbo::Size::new(180.0, 80.0));
        let after = ui.effective_bounds(thumb);

        assert!(after.y0 > before.y0);
    }

    #[test]
    fn scroll_bar_track_click_pages_viewport() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(8.0).logical_size(180.0, 80.0));
        let state = ScrollState::new();
        let scroll = ui.append(
            root,
            ScrollView::new().size(100.0, 48.0).state(state.clone()),
        );
        for _ in 0..4 {
            append_scroll_test_child(&mut ui, scroll);
        }
        let bar = ui.append(
            root,
            ScrollBar::vertical(scroll, state)
                .length(48.0)
                .thickness(8.0)
                .thumb_extent(12.0),
        );

        ui.compute_layout(kurbo::Size::new(180.0, 80.0));
        let bar_bounds = ui.effective_bounds(bar);
        let result =
            ui.synthesize_click(bar, Point::new(bar_bounds.center().x, bar_bounds.y1 - 1.0));

        assert!(result.handled);
        assert!(scroll_view_offset_y(&ui, scroll).is_some_and(|offset| offset > 0.0));
    }

    #[test]
    fn scroll_bar_thumb_drag_updates_viewport_offset() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(8.0).logical_size(180.0, 80.0));
        let state = ScrollState::new();
        let scroll = ui.append(
            root,
            ScrollView::new().size(100.0, 48.0).state(state.clone()),
        );
        for _ in 0..4 {
            append_scroll_test_child(&mut ui, scroll);
        }
        let bar = ui.append(
            root,
            ScrollBar::vertical(scroll, state)
                .length(48.0)
                .thickness(8.0)
                .thumb_extent(12.0),
        );
        let thumb = ui
            .slot(bar, SLOT_SCROLL_BAR_THUMB)
            .expect("scrollbar template should create a thumb slot");

        ui.compute_layout(kurbo::Size::new(180.0, 80.0));
        let start = ui.effective_bounds(thumb).center();
        assert!(ui.dispatch_pointer_event(&pointer_down(start, 1)));
        assert!(ui.dispatch_pointer_event(&pointer_move(
            Point::new(start.x, start.y + 20.0),
            2,
            true,
        )));

        assert!(scroll_view_offset_y(&ui, scroll).is_some_and(|offset| offset > 0.0));
    }

    fn append_scroll_test_child(ui: &mut Ui, scroll: ElementId) -> ElementId {
        let child = ui.insert_child(scroll);
        ui.set_layout(
            child,
            Layout::new().logical_size(100.0, 24.0).flex_shrink(0.0),
        );
        child
    }

    fn pointer_down(point: Point, time: u64) -> ui_events::pointer::PointerEvent {
        use ui_events::pointer::{PointerButton, PointerButtonEvent, PointerEvent};
        PointerEvent::Down(PointerButtonEvent {
            pointer: pointer_info(),
            button: Some(PointerButton::Primary),
            state: pointer_state(point, time, true),
        })
    }

    fn pointer_move(
        point: Point,
        time: u64,
        primary_down: bool,
    ) -> ui_events::pointer::PointerEvent {
        use ui_events::pointer::{PointerEvent, PointerUpdate};
        PointerEvent::Move(PointerUpdate {
            pointer: pointer_info(),
            current: pointer_state(point, time, primary_down),
            coalesced: Vec::new(),
            predicted: Vec::new(),
        })
    }

    fn pointer_info() -> ui_events::pointer::PointerInfo {
        use ui_events::pointer::{PointerId, PointerInfo, PointerType};
        PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        }
    }

    fn pointer_state(
        point: Point,
        time: u64,
        primary_down: bool,
    ) -> ui_events::pointer::PointerState {
        use ui_events::pointer::{PointerButton, PointerButtons, PointerState};

        let mut buttons = PointerButtons::new();
        if primary_down {
            buttons.insert(PointerButton::Primary);
        }
        let mut state = PointerState {
            time,
            buttons,
            count: 1,
            pressure: if primary_down { 0.5 } else { 0.0 },
            ..PointerState::default()
        };
        state.position.x = point.x;
        state.position.y = point.y;
        state
    }
}
