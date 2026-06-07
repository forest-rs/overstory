// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Small shared helpers for built-in controls.

use alloc::boxed::Box;
use core::cell::{Cell, RefCell};
use core::ops::Range;

use kurbo::{Axis, Point, Rect, Size, Vec2};
use understory_property::{DependencyObjectExt, Property};
use understory_virtual_list::{FixedExtentModel, ScrollAlign, VirtualList};

use crate::behavior::{ArrangeContext, ArrangeResult};
use crate::channels::LAYOUT;
use crate::controls::scroll::ScrollBar;
use crate::controls::scroll::normalize_scroll_offset_y;
use crate::element::Element;
use crate::element::ElementId;
use crate::layouts::{Layout, LayoutPlacement, LayoutSize};
use crate::scroll_runtime::ScrollState;
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, Widget};

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

    pub(crate) fn set_option(&self, element: Option<ElementId>) {
        self.element.set(element);
    }

    pub(crate) fn clear(&self) {
        self.element.set(None);
    }
}

pub(crate) type EventHandler<E> = Box<dyn FnMut(&mut Ui, E) + 'static>;

pub(crate) struct CallbackSlot<E> {
    handler: RefCell<Option<EventHandler<E>>>,
}

impl<E> CallbackSlot<E> {
    pub(crate) const fn new(handler: Option<EventHandler<E>>) -> Self {
        Self {
            handler: RefCell::new(handler),
        }
    }

    pub(crate) fn dispatch(&self, ui: &mut Ui, event: E) {
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
    handler.dispatch(ui, event);
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

pub(crate) fn fixed_virtual_accessibility_scroll_delta(
    request: &accesskit::ActionRequest,
    direction: f64,
    item_extent: f64,
    viewport_extent: f64,
) -> Option<f64> {
    let unit = action_scroll_unit(request)?;
    let amount = match unit {
        accesskit::ScrollUnit::Item => item_extent,
        accesskit::ScrollUnit::Page => viewport_extent.max(item_extent),
    };
    Some(amount * direction)
}

#[derive(Debug, Default)]
pub(crate) struct ReservedScrollFrameWidget {
    target: ElementSlot,
    scrollbar: ElementSlot,
    gap: f32,
    scrollbar_thickness: f32,
}

impl ReservedScrollFrameWidget {
    const fn new(gap: f32, scrollbar_thickness: f32) -> Self {
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
        let height = f32_from_logical(bounds.height()).max(1.0);
        let width = f32_from_logical(bounds.width()).max(1.0);
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
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) gap: f32,
    pub(crate) scrollbar_thickness: f32,
    pub(crate) thumb_extent: f32,
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

pub(crate) fn set_fixed_virtual_scroll_offset_y(
    owner: ElementId,
    ui: &mut Ui,
    virtualizer: &mut FixedVirtualizer,
    scroll_state: &ScrollState,
    offset: f64,
) {
    virtualizer.set_scroll_offset(normalize_scroll_offset_y(offset));
    virtualizer.clamp_scroll_to_content();
    sync_fixed_virtual_scroll_target(owner, ui, virtualizer, scroll_state);
}

pub(crate) fn scroll_fixed_virtual_index_into_view(
    owner: ElementId,
    ui: &mut Ui,
    virtualizer: &mut FixedVirtualizer,
    scroll_state: &ScrollState,
    index: usize,
    len: usize,
) -> bool {
    if index >= len {
        return false;
    }
    virtualizer.scroll_to_index(index, ScrollAlign::Nearest);
    sync_fixed_virtual_scroll_target(owner, ui, virtualizer, scroll_state);
    true
}

pub(crate) fn clamp_fixed_virtual_scroll_target(
    owner: ElementId,
    ui: &mut Ui,
    virtualizer: &mut FixedVirtualizer,
    scroll_state: &ScrollState,
) {
    virtualizer.clamp_scroll_to_content();
    sync_fixed_virtual_scroll_target(owner, ui, virtualizer, scroll_state);
}

fn sync_fixed_virtual_scroll_target(
    owner: ElementId,
    ui: &mut Ui,
    virtualizer: &FixedVirtualizer,
    scroll_state: &ScrollState,
) {
    let _ = scroll_state.set_offset_y_raw(virtualizer.scroll_offset());
    // Fixed-extent virtual lists rematerialize rows when the offset changes,
    // so they still need a real layout pass. Plain `ScrollView` targets use
    // ARRANGE-only invalidation through their registered scroll target policy.
    ui.mark_channels(owner, LAYOUT.into());
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
