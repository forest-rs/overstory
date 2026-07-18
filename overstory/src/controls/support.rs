// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Small shared helpers for built-in controls.

use alloc::{boxed::Box, rc::Rc};
use core::cell::{Cell, RefCell};

use kurbo::{Axis, Point, Rect, Size, Vec2};
use understory_presentation::Edges;
use understory_property::{DependencyObjectExt, Property};

use crate::behavior::{ArrangeContext, ArrangeResult};
use crate::controls::scroll::ScrollBar;
use crate::element::Element;
use crate::element::ElementId;
use crate::layouts::{Layout, LayoutDimension, LayoutSize};
use crate::scroll_runtime::ScrollState;
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, Widget};

/// Resolves a control's outer size: either a fixed logical size, or a
/// fill-width size with a fixed logical height. Controls swap only the width
/// dimension so they keep their own layout flavor (relative positioning,
/// centered rows) while stretching to fill their container.
pub(crate) fn control_size(fill_width: bool, width: f64, height: f64) -> LayoutSize {
    if fill_width {
        LayoutSize::new()
            .width(LayoutDimension::Percent(1.0))
            .height(LayoutDimension::Logical(height))
    } else {
        LayoutSize::logical(width, height)
    }
}

#[derive(Debug, Default)]
pub(crate) struct ElementSlot {
    element: Cell<Option<ElementId>>,
}

impl ElementSlot {
    pub(crate) const fn new() -> Self {
        Self {
            element: Cell::new(None),
        }
    }

    pub(crate) fn get(&self) -> Option<ElementId> {
        self.element.get()
    }

    pub(crate) fn set(&self, element: ElementId) {
        self.element.set(Some(element));
    }
}

pub(crate) type EventHandler<E> = Box<dyn FnMut(&mut Ui, E) + 'static>;

pub(crate) struct CallbackSlot<E> {
    handler: Rc<RefCell<Option<EventHandler<E>>>>,
}

impl<E> Clone for CallbackSlot<E> {
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
        }
    }
}

impl<E: 'static> CallbackSlot<E> {
    pub(crate) fn new(handler: Option<EventHandler<E>>) -> Self {
        Self {
            handler: Rc::new(RefCell::new(handler)),
        }
    }

    pub(crate) fn has_handler(&self) -> bool {
        self.handler.borrow().is_some()
    }

    pub(crate) fn dispatch(&self, ui: &mut Ui, source: ElementId, event: E) {
        let slot = self.clone();
        ui.enqueue_semantic_callback(source, move |ui| slot.dispatch_now(ui, event));
    }

    fn dispatch_now(&self, ui: &mut Ui, event: E) {
        let Some(mut callback) = self.handler.borrow_mut().take() else {
            return;
        };
        callback(ui, event);
        let should_restore = self.handler.borrow().is_none();
        if should_restore {
            *self.handler.borrow_mut() = Some(callback);
        }
    }
}

/// Event delivered after a checked-state control changes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedChangedEvent {
    /// The control whose checked state changed.
    pub element: ElementId,
    /// The checked state after the change.
    pub checked: bool,
}

pub(crate) type CheckedChangedHandler = EventHandler<CheckedChangedEvent>;
pub(crate) type CheckedChangedSlot = CallbackSlot<CheckedChangedEvent>;

pub(crate) fn dispatch_checked_changed(
    ui: &mut Ui,
    handler: &CheckedChangedSlot,
    event: CheckedChangedEvent,
) {
    handler.dispatch(ui, event.element, event);
}

pub(crate) fn is_descendant_or_self(ui: &Ui, descendant: ElementId, ancestor: ElementId) -> bool {
    let mut current = Some(descendant);
    while let Some(id) = current {
        if id == ancestor {
            return true;
        }
        current = ui.get(id).and_then(Element::parent);
    }
    false
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

pub(crate) fn box_padding_xy(horizontal: f64, vertical: f64) -> Edges<f64> {
    let horizontal = horizontal.max(0.0);
    let vertical = vertical.max(0.0);
    Edges::new(vertical, horizontal, vertical, horizontal)
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

pub(crate) fn configure_vertical_scroll_accessibility(
    element: &Element,
    ctx: &crate::AccessibilityContext<'_>,
    node: &mut accesskit::Node,
    scroll_state: &ScrollState,
) {
    let offset = scroll_state.offset();
    let max = scroll_state.max_offset();
    node.set_scroll_y(offset.y);
    node.set_scroll_y_min(0.0);
    node.set_scroll_y_max(max.y);
    if ctx.is_enabled(element.id()) && max.y > f64::EPSILON {
        node.add_action(accesskit::Action::ScrollUp);
        node.add_action(accesskit::Action::ScrollDown);
        node.add_action(accesskit::Action::SetScrollOffset);
    }
}

#[derive(Debug, Default)]
pub(crate) struct ReservedScrollFrameWidget {
    target: ElementSlot,
    scrollbar: ElementSlot,
    gap: f64,
    scrollbar_thickness: f64,
}

impl ReservedScrollFrameWidget {
    const fn new(gap: f64, scrollbar_thickness: f64) -> Self {
        Self {
            target: ElementSlot::new(),
            scrollbar: ElementSlot::new(),
            gap,
            scrollbar_thickness,
        }
    }

    pub(crate) fn target(&self) -> Option<ElementId> {
        self.target.get()
    }

    pub(crate) fn scrollbar(&self) -> Option<ElementId> {
        self.scrollbar.get()
    }

    fn set_parts(&self, target: ElementId, scrollbar: ElementId) {
        self.target.set(target);
        self.scrollbar.set(scrollbar);
    }
}

impl Widget for ReservedScrollFrameWidget {
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
        let height = bounds.height().max(1.0);
        let width = bounds.width().max(1.0);
        let target_width = (width - self.gap - self.scrollbar_thickness).max(1.0);

        let mut changed = false;
        if let Some(target) = self.target.get() {
            changed |= set_layout_size_if_changed(
                ctx.ui_mut(),
                target,
                LayoutSize::logical(target_width, height),
            );
        }
        if let Some(scrollbar) = self.scrollbar.get() {
            changed |= set_layout_size_if_changed(
                ctx.ui_mut(),
                scrollbar,
                LayoutSize::logical(self.scrollbar_thickness, height),
            );
        }

        ArrangeResult::from_layout_changed(changed)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ReservedVerticalScrollArea {
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) gap: f64,
    pub(crate) scrollbar_thickness: f64,
    pub(crate) thumb_extent: f64,
    pub(crate) enabled: bool,
}

pub(crate) fn append_reserved_vertical_scroll_area(
    ui: &mut Ui,
    parent: ElementId,
    target: impl AppendSpec,
    spec: ReservedVerticalScrollArea,
) -> (ElementId, ElementId, ElementId) {
    let frame_width = spec.width + spec.gap + spec.scrollbar_thickness;
    let frame = ui.append_widget(
        parent,
        Box::new(ReservedScrollFrameWidget::new(
            spec.gap,
            spec.scrollbar_thickness,
        )),
    );
    ui.set_layout(
        frame,
        Layout::row(spec.gap).logical_size(frame_width, spec.height),
    );
    ui.set_control_state(frame, ControlState::interactive().focusable(false));

    let target = ui.append(frame, target);
    let scrollbar = ui.append(
        frame,
        ScrollBar::vertical(target)
            .length(spec.height)
            .thickness(spec.scrollbar_thickness)
            .thumb_extent(spec.thumb_extent)
            .enabled(spec.enabled),
    );

    if let Some(widget) = reserved_scroll_frame_widget(ui, frame) {
        widget.set_parts(target, scrollbar);
    }
    (frame, target, scrollbar)
}

pub(crate) fn reserved_scroll_frame_widget(
    ui: &Ui,
    frame: ElementId,
) -> Option<&ReservedScrollFrameWidget> {
    ui.get(frame)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<ReservedScrollFrameWidget>())
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

#[cfg(test)]
mod fill_width_tests {
    use kurbo::Size;

    use crate::widget::AppendSpec;
    use crate::{Layout, ListBox, ProgressBar, Slider, TextInput, Ui};

    fn filled_width(spec: impl AppendSpec) -> f64 {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(300.0, 200.0));
        let id = ui.append(root, spec);
        ui.compute_layout(Size::new(300.0, 200.0));
        ui.effective_bounds(id).width()
    }

    #[test]
    fn controls_fill_width_stretch_to_container() {
        assert!(
            (filled_width(ListBox::new().fill_width(120.0)) - 300.0).abs() < 0.5,
            "list box should fill its container width",
        );
        assert!(
            (filled_width(TextInput::new().fill_width()) - 300.0).abs() < 0.5,
            "text input should fill its container width",
        );
        assert!(
            (filled_width(ProgressBar::new().fill_width(18.0)) - 300.0).abs() < 0.5,
            "progress bar should fill its container width",
        );
        assert!(
            (filled_width(Slider::new().fill_width()) - 300.0).abs() < 0.5,
            "slider should fill its container width",
        );
    }
}
