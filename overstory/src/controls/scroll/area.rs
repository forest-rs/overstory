// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::boxed::Box;

use kurbo::Axis;
use understory_style::TypeTag;

use crate::behavior::{ArrangeContext, ArrangeResult};
use crate::controls::support::ElementSlot;
use crate::element::{Element, ElementId};
use crate::layouts::{Layout, LayoutSize};
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, ElementConfig, ElementSpec, Widget};

use super::bar::{ScrollBar, scroll_bar_size};
use super::events::{ScrollOffsetChangedEvent, ScrollOffsetChangedHandler};
use super::ids::{
    DEFAULT_SCROLL_AREA_GAP, DEFAULT_SCROLL_AREA_HEIGHT, DEFAULT_SCROLL_AREA_WIDTH,
    DEFAULT_SCROLL_BAR_THICKNESS, DEFAULT_SCROLL_BAR_THUMB_EXTENT, TYPE_SCROLL_AREA,
};
use super::runtime::{normalize_scroll_offset_y, set_layout_size_if_changed};
use super::view::ScrollView;
/// Builder + spec for a viewport with optional reserved scrollbars.
///
/// `ScrollArea` owns the scroll viewport and scrollbar elements. Append
/// content to the viewport returned by [`scroll_area_viewport`].
pub struct ScrollArea {
    size: Option<(f64, f64)>,
    vertical_scrollbar: bool,
    horizontal_scrollbar: bool,
    gap: f64,
    scrollbar_thickness: f64,
    thumb_extent: f64,
    scroll_offset_y: f64,
    on_offset_changed: Option<ScrollOffsetChangedHandler>,
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
            .field(
                "has_offset_changed_handler",
                &self.on_offset_changed.is_some(),
            )
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
            on_offset_changed: None,
            element,
        }
    }

    /// Sets the total fixed logical-pixel size of the area.
    #[must_use]
    pub fn size(mut self, width: f64, height: f64) -> Self {
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
    pub fn gap(mut self, gap: f64) -> Self {
        self.gap = gap.max(0.0);
        self
    }

    /// Sets the reserved scrollbar thickness.
    #[must_use]
    pub fn scrollbar_thickness(mut self, thickness: f64) -> Self {
        self.scrollbar_thickness = thickness.max(1.0);
        self
    }

    /// Sets the fixed scrollbar thumb extent.
    #[must_use]
    pub fn thumb_extent(mut self, extent: f64) -> Self {
        self.thumb_extent = extent.max(1.0);
        self
    }

    /// Sets the viewport's initial vertical scroll offset.
    #[must_use]
    pub fn scroll_offset_y(mut self, offset: f64) -> Self {
        self.scroll_offset_y = normalize_scroll_offset_y(offset);
        self
    }

    /// Registers a handler for the internal viewport's scroll offset changes.
    #[must_use]
    pub fn on_offset_changed<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, ScrollOffsetChangedEvent) + 'static,
    {
        self.on_offset_changed = Some(Box::new(handler));
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
            on_offset_changed,
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
                .on_offset_changed_option(on_offset_changed)
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
    gap: f64,
    scrollbar_thickness: f64,
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
            bounds.width().max(1.0),
            bounds.height().max(1.0),
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
    viewport_width: f64,
    viewport_height: f64,
    body_height: f64,
}

impl ScrollAreaMetrics {
    fn new(
        width: f64,
        height: f64,
        gap: f64,
        thickness: f64,
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

fn scrollbar_gutter(enabled: bool, gap: f64, thickness: f64) -> f64 {
    if enabled { gap + thickness } else { 0.0 }
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
