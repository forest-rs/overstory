// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `ScrollView`: reusable clipped viewport state and behavior.

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::Cell;

use kurbo::Vec2;
use understory_style::TypeTag;

use crate::Phase;
use crate::channels::LAYOUT;
use crate::element::{Element, ElementId};
use crate::layouts::Layout;
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, Widget};

/// Selector identity for `ScrollView`.
pub const TYPE_SCROLL_VIEW: TypeTag = TypeTag(13);

const DEFAULT_SCROLL_LINE_Y: f64 = 36.0;

/// Shared scroll offset state for viewport-like controls.
#[derive(Clone)]
pub struct ScrollState {
    inner: Rc<ScrollStateInner>,
}

struct ScrollStateInner {
    offset_x: Cell<f64>,
    offset_y: Cell<f64>,
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

    fn clips_subtree(&self, element: &Element) -> bool {
        let _ = element;
        true
    }

    fn child_layout_offset(&self, element: &Element) -> Vec2 {
        let _ = element;
        -self.state.offset()
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

pub(crate) fn install_wheel_scroll_behavior(ui: &mut Ui, viewport: ElementId, state: ScrollState) {
    ui.on_scroll(viewport, move |ui, event| {
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
    let offset = state.offset();
    let mut content_right = 0.0_f64;
    let mut content_bottom = 0.0_f64;
    if let Some(element) = ui.get(viewport) {
        for child in element.children() {
            let child_bounds = ui.effective_bounds(*child);
            content_right = content_right.max(child_bounds.x1 + offset.x - viewport_bounds.x0);
            content_bottom = content_bottom.max(child_bounds.y1 + offset.y - viewport_bounds.y0);
        }
    }
    Vec2::new(
        (content_right - viewport_bounds.width()).max(0.0),
        (content_bottom - viewport_bounds.height()).max(0.0),
    )
}

fn scroll_view_state(ui: &Ui, scroll_view: ElementId) -> Option<ScrollState> {
    ui.get(scroll_view)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<ScrollViewWidget>())
        .map(|widget| widget.state.clone())
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
        let result =
            ui.synthesize_scroll(scroll, kurbo::Point::ZERO, ScrollDelta::LineDelta(0.0, 1.0));
        assert!(result.handled);
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
}
