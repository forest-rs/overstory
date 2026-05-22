// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Splitter`: draggable resize grip for adjacent panes.

use alloc::{boxed::Box, rc::Rc};
use core::cell::RefCell;

use cursor_icon::CursorIcon;
use understory_style::TypeTag;

use crate::Phase;
use crate::element::{Element, ElementId};
use crate::layouts::Layout;
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, Widget};

/// Selector identity for `Splitter`.
pub const TYPE_SPLITTER: TypeTag = TypeTag(10);

const DEFAULT_THICKNESS: f32 = 8.0;
const DEFAULT_LENGTH: f32 = 160.0;
const DEFAULT_KEYBOARD_STEP: f64 = 8.0;
const DEFAULT_KEYBOARD_LARGE_STEP: f64 = 24.0;

/// Orientation for a splitter grip.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum SplitterAxis {
    /// Horizontal rule between top/bottom panes. Dragging changes pane heights.
    Horizontal,
    /// Vertical rule between left/right panes. Dragging changes pane widths.
    #[default]
    Vertical,
}

/// Payload produced when a splitter is dragged or moved from the keyboard.
#[derive(Copy, Clone, Debug)]
pub struct SplitterResizeEvent {
    /// Splitter element that produced this resize request.
    pub splitter: ElementId,
    /// Splitter orientation.
    pub axis: SplitterAxis,
    /// Incremental resize delta in logical pixels.
    pub delta: f64,
    /// Distance from the original press position for pointer drags.
    pub total_delta: f64,
    /// Current pointer position, or the splitter's current origin for keyboard input.
    pub position: kurbo::Point,
    /// Original pointer press position, or the splitter's current origin for keyboard input.
    pub press_position: kurbo::Point,
}

impl SplitterResizeEvent {
    /// Resizes two fixed-size panes adjacent to this splitter.
    ///
    /// Vertical splitters resize pane widths; horizontal splitters resize pane
    /// heights. The helper intentionally only covers the common fixed-pane
    /// case; richer split layouts can use [`Self::delta`] directly.
    pub fn resize_pair(
        self,
        ui: &mut Ui,
        before: ElementId,
        after: ElementId,
        min_before: f64,
        min_after: f64,
    ) -> bool {
        resize_split_pair(
            ui, before, after, self.axis, self.delta, min_before, min_after,
        )
    }
}

type SplitterResizeHandler = Box<dyn FnMut(&mut Ui, SplitterResizeEvent)>;

/// Builder + spec for a draggable splitter.
pub struct Splitter {
    axis: SplitterAxis,
    thickness: f32,
    length: f32,
    keyboard_step: f64,
    keyboard_large_step: f64,
    state: ControlState,
    on_resize: Option<SplitterResizeHandler>,
}

impl core::fmt::Debug for Splitter {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Splitter")
            .field("axis", &self.axis)
            .field("thickness", &self.thickness)
            .field("length", &self.length)
            .field("keyboard_step", &self.keyboard_step)
            .field("keyboard_large_step", &self.keyboard_large_step)
            .finish_non_exhaustive()
    }
}

impl Splitter {
    /// Creates a vertical splitter between left/right panes.
    #[must_use]
    pub fn vertical() -> Self {
        Self::new(SplitterAxis::Vertical)
    }

    /// Creates a horizontal splitter between top/bottom panes.
    #[must_use]
    pub fn horizontal() -> Self {
        Self::new(SplitterAxis::Horizontal)
    }

    /// Creates a splitter with the supplied orientation.
    #[must_use]
    pub fn new(axis: SplitterAxis) -> Self {
        Self {
            axis,
            thickness: DEFAULT_THICKNESS,
            length: DEFAULT_LENGTH,
            keyboard_step: DEFAULT_KEYBOARD_STEP,
            keyboard_large_step: DEFAULT_KEYBOARD_LARGE_STEP,
            state: ControlState::interactive(),
            on_resize: None,
        }
    }

    /// Sets the splitter thickness in logical pixels.
    #[must_use]
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness.max(1.0);
        self
    }

    /// Sets the splitter's main-axis length in logical pixels.
    #[must_use]
    pub fn length(mut self, length: f32) -> Self {
        self.length = length.max(1.0);
        self
    }

    /// Sets the unmodified arrow-key resize step.
    #[must_use]
    pub fn keyboard_step(mut self, step: f64) -> Self {
        if step.is_finite() && step > 0.0 {
            self.keyboard_step = step;
        }
        self
    }

    /// Sets the Shift+arrow resize step.
    #[must_use]
    pub fn keyboard_large_step(mut self, step: f64) -> Self {
        if step.is_finite() && step > 0.0 {
            self.keyboard_large_step = step;
        }
        self
    }

    /// Sets whether the splitter participates in focus traversal.
    #[must_use]
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.state = self.state.focusable(focusable);
        self
    }

    /// Sets whether the splitter starts enabled.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.state = self.state.enabled(enabled);
        self
    }

    /// Convenience for [`Self::enabled(false)`].
    #[must_use]
    pub fn disabled(self) -> Self {
        self.enabled(false)
    }

    /// Registers a resize handler.
    ///
    /// The splitter owns input policy and focus. The handler owns layout
    /// policy, which keeps custom split views from being forced into one pane
    /// sizing model.
    #[must_use]
    pub fn on_resize<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, SplitterResizeEvent) + 'static,
    {
        self.on_resize = Some(Box::new(handler));
        self
    }

    fn layout(&self) -> Layout {
        match self.axis {
            SplitterAxis::Horizontal => Layout::new().logical_size(self.length, self.thickness),
            SplitterAxis::Vertical => Layout::new().logical_size(self.thickness, self.length),
        }
    }
}

impl Default for Splitter {
    fn default() -> Self {
        Self::vertical()
    }
}

impl AppendSpec for Splitter {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget(parent, Box::new(SplitterWidget { axis: self.axis }));
        ui.set_layout(id, self.layout());
        ui.set_control_state(id, self.state);
        install_splitter_behavior(
            ui,
            id,
            self.axis,
            self.keyboard_step,
            self.keyboard_large_step,
            self.on_resize,
        );
        id
    }
}

/// Per-element widget for `Splitter`.
#[derive(Debug, Default)]
pub struct SplitterWidget {
    axis: SplitterAxis,
}

impl Widget for SplitterWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_SPLITTER)
    }

    fn cursor_icon(&self, element: &Element) -> Option<CursorIcon> {
        let _ = element;
        Some(match self.axis {
            SplitterAxis::Horizontal => CursorIcon::RowResize,
            SplitterAxis::Vertical => CursorIcon::ColResize,
        })
    }
}

fn install_splitter_behavior(
    ui: &mut Ui,
    splitter: ElementId,
    axis: SplitterAxis,
    keyboard_step: f64,
    keyboard_large_step: f64,
    on_resize: Option<SplitterResizeHandler>,
) {
    let on_resize = Rc::new(RefCell::new(on_resize));
    ui.on_click(splitter, move |ui, event| {
        if !matches!(event.phase, Phase::Target | Phase::Bubble) {
            return;
        }
        if !ui.is_enabled(splitter) {
            return;
        }
        event.mark_handled();
    });

    let drag_handler = Rc::clone(&on_resize);
    ui.on_drag(splitter, move |ui, event| {
        if !matches!(event.phase, Phase::Target | Phase::Bubble) {
            return;
        }
        if !ui.is_enabled(splitter) {
            return;
        }
        let delta = axis_delta(axis, event.delta);
        if delta != 0.0
            && let Some(handler) = drag_handler.borrow_mut().as_mut()
        {
            handler(
                ui,
                SplitterResizeEvent {
                    splitter,
                    axis,
                    delta,
                    total_delta: point_axis_delta(axis, event.position, event.press_position),
                    position: event.position,
                    press_position: event.press_position,
                },
            );
        }
        event.mark_handled();
    });

    let key_handler = Rc::clone(&on_resize);
    ui.on_key(splitter, move |ui, event| {
        use ui_events::keyboard::{Key, KeyState, NamedKey};
        if !matches!(event.phase, Phase::Target) {
            return;
        }
        if event.key.state != KeyState::Down {
            return;
        }
        if !ui.is_enabled(splitter) {
            return;
        }
        let step = if event.key.modifiers.shift() {
            keyboard_large_step
        } else {
            keyboard_step
        };
        let delta = match (&event.key.key, axis) {
            (Key::Named(NamedKey::ArrowLeft), SplitterAxis::Vertical)
            | (Key::Named(NamedKey::ArrowUp), SplitterAxis::Horizontal) => -step,
            (Key::Named(NamedKey::ArrowRight), SplitterAxis::Vertical)
            | (Key::Named(NamedKey::ArrowDown), SplitterAxis::Horizontal) => step,
            _ => return,
        };
        if let Some(handler) = key_handler.borrow_mut().as_mut() {
            let position = ui.effective_bounds(splitter).origin();
            handler(
                ui,
                SplitterResizeEvent {
                    splitter,
                    axis,
                    delta,
                    total_delta: delta,
                    position,
                    press_position: position,
                },
            );
        }
        event.mark_handled();
    });
}

fn axis_delta(axis: SplitterAxis, delta: kurbo::Vec2) -> f64 {
    match axis {
        SplitterAxis::Horizontal => delta.y,
        SplitterAxis::Vertical => delta.x,
    }
}

fn point_axis_delta(axis: SplitterAxis, position: kurbo::Point, origin: kurbo::Point) -> f64 {
    match axis {
        SplitterAxis::Horizontal => position.y - origin.y,
        SplitterAxis::Vertical => position.x - origin.x,
    }
}

/// Resizes two fixed-size panes as if a splitter between them moved by `delta`.
///
/// Vertical splitters adjust widths. Horizontal splitters adjust heights.
/// Returns `false` when either pane is stale, has no current size, or the
/// requested minimums cannot fit in the current combined span.
pub fn resize_split_pair(
    ui: &mut Ui,
    before: ElementId,
    after: ElementId,
    axis: SplitterAxis,
    delta: f64,
    min_before: f64,
    min_after: f64,
) -> bool {
    let before_bounds = ui.effective_bounds(before);
    let after_bounds = ui.effective_bounds(after);
    let before_size = axis_size(axis, before_bounds);
    let after_size = axis_size(axis, after_bounds);
    if before_size <= 0.0 || after_size <= 0.0 {
        return false;
    }

    let min_before = min_before.max(1.0);
    let min_after = min_after.max(1.0);
    let total = before_size + after_size;
    if total <= min_before + min_after {
        return false;
    }

    let next_before = (before_size + delta).clamp(min_before, total - min_after);
    let next_after = total - next_before;
    if (next_before - before_size).abs() <= f64::EPSILON {
        return false;
    }

    let before_layout = ui.effective_layout(before);
    let after_layout = ui.effective_layout(after);
    ui.set_layout(
        before,
        resized_layout(axis, before_bounds, next_before, before_layout),
    );
    ui.set_layout(
        after,
        resized_layout(axis, after_bounds, next_after, after_layout),
    );
    true
}

fn axis_size(axis: SplitterAxis, bounds: kurbo::Rect) -> f64 {
    match axis {
        SplitterAxis::Horizontal => bounds.height(),
        SplitterAxis::Vertical => bounds.width(),
    }
}

fn resized_layout(
    axis: SplitterAxis,
    bounds: kurbo::Rect,
    axis_size: f64,
    layout: Layout,
) -> Layout {
    match axis {
        SplitterAxis::Horizontal => {
            layout.logical_size(f32_from_f64(bounds.width()), f32_from_f64(axis_size))
        }
        SplitterAxis::Vertical => {
            layout.logical_size(f32_from_f64(axis_size), f32_from_f64(bounds.height()))
        }
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "control-local logical coordinates fit f32"
)]
fn f32_from_f64(value: f64) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::rc::Rc;
    use alloc::vec::Vec;
    use core::cell::Cell;
    use kurbo::{Point, Size};

    #[test]
    fn splitter_drag_resizes_pair() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(0.0).logical_size(260.0, 80.0));
        let before = ui.append_widget(root, Box::new(crate::PanelWidget));
        ui.set_layout(before, Layout::new().logical_size(120.0, 80.0));
        let after = ui.append_widget(root, Box::new(crate::PanelWidget));
        ui.set_layout(after, Layout::new().logical_size(120.0, 80.0));
        let splitter = ui.append(
            root,
            Splitter::vertical()
                .length(80.0)
                .on_resize(move |ui, event| {
                    event.resize_pair(ui, before, after, 40.0, 40.0);
                }),
        );
        ui.compute_layout(Size::new(260.0, 80.0));

        let press = ui.effective_bounds(splitter).center();
        assert!(ui.dispatch_pointer_event(&pointer_down(press, 1)));
        assert!(ui.dispatch_pointer_event(&pointer_move(
            Point::new(press.x + 20.0, press.y),
            2,
            true
        )));
        ui.compute_layout(Size::new(260.0, 80.0));

        assert_eq!(ui.effective_bounds(before).width(), 140.0);
        assert_eq!(ui.effective_bounds(after).width(), 100.0);
    }

    #[test]
    fn splitter_keyboard_resize_marks_key_handled() {
        use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

        let called = Rc::new(Cell::new(false));
        let called_for_handler = Rc::clone(&called);
        let mut ui = Ui::new();
        let root = ui.root();
        let splitter = ui.append(
            root,
            Splitter::vertical().on_resize(move |_ui, event| {
                called_for_handler.set(event.delta > 0.0);
            }),
        );
        ui.set_focus(Some(splitter));

        let key = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
        let result = ui.dispatch_key(&key);

        assert!(result.handled);
        assert!(called.get());
    }

    #[test]
    fn splitter_keyboard_uses_small_and_large_steps() {
        use core::cell::RefCell;
        use ui_events::keyboard::{Code, Key, KeyboardEvent, Modifiers, NamedKey};

        let deltas = Rc::new(RefCell::new(Vec::new()));
        let deltas_for_handler = Rc::clone(&deltas);
        let mut ui = Ui::new();
        let root = ui.root();
        let splitter = ui.append(
            root,
            Splitter::vertical()
                .keyboard_step(5.0)
                .keyboard_large_step(17.0)
                .on_resize(move |_ui, event| {
                    deltas_for_handler.borrow_mut().push(event.delta);
                }),
        );
        ui.set_focus(Some(splitter));

        let right = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
        assert!(ui.dispatch_key(&right).handled);

        let mut shift_left =
            KeyboardEvent::key_down(Key::Named(NamedKey::ArrowLeft), Code::ArrowLeft);
        shift_left.modifiers = Modifiers::SHIFT;
        assert!(ui.dispatch_key(&shift_left).handled);

        assert_eq!(deltas.borrow().as_slice(), &[5.0, -17.0]);
    }

    #[test]
    fn splitter_hover_exposes_axis_resize_cursor() {
        use cursor_icon::CursorIcon;

        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(0.0).logical_size(40.0, 180.0));
        let vertical = ui.append(root, Splitter::vertical().length(160.0));
        ui.compute_layout(Size::new(40.0, 180.0));

        assert!(ui.dispatch_pointer_event(&pointer_move(
            ui.effective_bounds(vertical).center(),
            1,
            false,
        )));
        assert_eq!(ui.cursor_icon(), Some(CursorIcon::ColResize));

        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 40.0));
        let horizontal = ui.append(root, Splitter::horizontal().length(160.0));
        ui.compute_layout(Size::new(180.0, 40.0));

        assert!(ui.dispatch_pointer_event(&pointer_move(
            ui.effective_bounds(horizontal).center(),
            1,
            false,
        )));
        assert_eq!(ui.cursor_icon(), Some(CursorIcon::RowResize));
    }

    #[test]
    fn pressed_splitter_keeps_resize_cursor_off_hover() {
        use cursor_icon::CursorIcon;

        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(0.0).logical_size(40.0, 180.0));
        let splitter = ui.append(root, Splitter::vertical().length(160.0));
        ui.compute_layout(Size::new(40.0, 180.0));

        let center = ui.effective_bounds(splitter).center();
        assert!(ui.dispatch_pointer_event(&pointer_down(center, 1)));
        assert!(ui.dispatch_pointer_event(&pointer_move(Point::new(300.0, 300.0), 2, true,)));

        assert_eq!(ui.hovered(), None);
        assert_eq!(ui.cursor_icon(), Some(CursorIcon::ColResize));
    }

    #[test]
    fn splitter_pointer_focus_is_not_focus_visible() {
        use crate::pseudos::{PSEUDO_FOCUS_VISIBLE, PSEUDO_FOCUSED};

        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(0.0).logical_size(40.0, 180.0));
        let splitter = ui.append(root, Splitter::vertical().length(160.0));
        ui.compute_layout(Size::new(40.0, 180.0));

        assert!(
            ui.dispatch_pointer_event(&pointer_down(ui.effective_bounds(splitter).center(), 1,))
        );

        let pseudos = ui.get(splitter).unwrap().pseudos();
        assert_eq!(ui.focused(), Some(splitter));
        assert!(pseudos.contains(&PSEUDO_FOCUSED));
        assert!(!pseudos.contains(&PSEUDO_FOCUS_VISIBLE));
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
