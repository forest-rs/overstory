// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::boxed::Box;
use alloc::vec::Vec;

use hashbrown::{HashMap, HashSet};
use kurbo::Point;
use understory_event_state::click::{ClickResult, ClickState};
use understory_event_state::hover::{HoverEvent, HoverState};

use crate::element::ElementId;
use crate::ui::{ClickEvent, DragEvent, KeyEvent, ScrollEvent, Ui};

pub(crate) type ClickHandler = Box<dyn FnMut(&mut Ui, ClickEvent<'_>)>;
pub(crate) type DragHandler = Box<dyn FnMut(&mut Ui, DragEvent<'_>)>;
pub(crate) type ScrollHandler = Box<dyn FnMut(&mut Ui, ScrollEvent<'_>)>;
pub(crate) type KeyHandler = Box<dyn FnMut(&mut Ui, KeyEvent<'_>)>;

/// Routed event handler stores and in-flight removal tracking.
pub(crate) struct EventRuntime {
    click_handlers: HashMap<ElementId, ClickHandler>,
    click_dispatch: HandlerDispatchTracker,
    drag_handlers: HashMap<ElementId, DragHandler>,
    drag_dispatch: HandlerDispatchTracker,
    scroll_handlers: HashMap<ElementId, ScrollHandler>,
    scroll_dispatch: HandlerDispatchTracker,
    key_handlers: HashMap<ElementId, KeyHandler>,
    key_dispatch: HandlerDispatchTracker,
    hover: HoverState<ElementId>,
    clicks: ClickState<ElementId>,
    press_target: Option<ElementId>,
    press_position: Option<Point>,
    last_drag_position: Option<Point>,
}

impl EventRuntime {
    pub(crate) fn new() -> Self {
        Self {
            click_handlers: HashMap::new(),
            click_dispatch: HandlerDispatchTracker::default(),
            drag_handlers: HashMap::new(),
            drag_dispatch: HandlerDispatchTracker::default(),
            scroll_handlers: HashMap::new(),
            scroll_dispatch: HandlerDispatchTracker::default(),
            key_handlers: HashMap::new(),
            key_dispatch: HandlerDispatchTracker::default(),
            hover: HoverState::new(),
            clicks: ClickState::new(),
            press_target: None,
            press_position: None,
            last_drag_position: None,
        }
    }

    pub(crate) fn on_click(&mut self, id: ElementId, handler: ClickHandler) {
        self.click_handlers.insert(id, handler);
    }

    pub(crate) fn remove_click(&mut self, id: ElementId) {
        self.click_handlers.remove(&id);
        self.click_dispatch.note_removal(id);
    }

    pub(crate) fn on_drag(&mut self, id: ElementId, handler: DragHandler) {
        self.drag_handlers.insert(id, handler);
    }

    pub(crate) fn remove_drag(&mut self, id: ElementId) {
        self.drag_handlers.remove(&id);
        self.drag_dispatch.note_removal(id);
    }

    pub(crate) fn on_scroll(&mut self, id: ElementId, handler: ScrollHandler) {
        self.scroll_handlers.insert(id, handler);
    }

    pub(crate) fn remove_scroll(&mut self, id: ElementId) {
        self.scroll_handlers.remove(&id);
        self.scroll_dispatch.note_removal(id);
    }

    pub(crate) fn on_key(&mut self, id: ElementId, handler: KeyHandler) {
        self.key_handlers.insert(id, handler);
    }

    pub(crate) fn remove_key(&mut self, id: ElementId) {
        self.key_handlers.remove(&id);
        self.key_dispatch.note_removal(id);
    }

    pub(crate) fn remove_target(&mut self, id: ElementId) {
        self.click_handlers.remove(&id);
        self.drag_handlers.remove(&id);
        self.scroll_handlers.remove(&id);
        self.key_handlers.remove(&id);
        self.click_dispatch.note_removal(id);
        self.drag_dispatch.note_removal(id);
        self.scroll_dispatch.note_removal(id);
        self.key_dispatch.note_removal(id);
    }

    pub(crate) fn take_click(&mut self, id: ElementId) -> Option<ClickHandler> {
        self.click_handlers.remove(&id)
    }

    pub(crate) fn begin_click(&mut self, id: ElementId) {
        self.click_dispatch.begin(id);
    }

    pub(crate) fn end_click(&mut self, id: ElementId) {
        self.click_dispatch.end(id);
    }

    pub(crate) fn click_removed(&mut self, id: ElementId) -> bool {
        self.click_dispatch.take_removed(id)
    }

    pub(crate) fn insert_click_if_absent(&mut self, id: ElementId, handler: ClickHandler) {
        self.click_handlers.entry(id).or_insert(handler);
    }

    pub(crate) fn has_click(&self, id: ElementId) -> bool {
        self.click_handlers.contains_key(&id)
    }

    pub(crate) fn take_drag(&mut self, id: ElementId) -> Option<DragHandler> {
        self.drag_handlers.remove(&id)
    }

    pub(crate) fn begin_drag(&mut self, id: ElementId) {
        self.drag_dispatch.begin(id);
    }

    pub(crate) fn end_drag(&mut self, id: ElementId) {
        self.drag_dispatch.end(id);
    }

    pub(crate) fn drag_removed(&mut self, id: ElementId) -> bool {
        self.drag_dispatch.take_removed(id)
    }

    pub(crate) fn insert_drag_if_absent(&mut self, id: ElementId, handler: DragHandler) {
        self.drag_handlers.entry(id).or_insert(handler);
    }

    pub(crate) fn has_drag(&self, id: ElementId) -> bool {
        self.drag_handlers.contains_key(&id)
    }

    pub(crate) fn take_scroll(&mut self, id: ElementId) -> Option<ScrollHandler> {
        self.scroll_handlers.remove(&id)
    }

    pub(crate) fn begin_scroll(&mut self, id: ElementId) {
        self.scroll_dispatch.begin(id);
    }

    pub(crate) fn end_scroll(&mut self, id: ElementId) {
        self.scroll_dispatch.end(id);
    }

    pub(crate) fn scroll_removed(&mut self, id: ElementId) -> bool {
        self.scroll_dispatch.take_removed(id)
    }

    pub(crate) fn insert_scroll_if_absent(&mut self, id: ElementId, handler: ScrollHandler) {
        self.scroll_handlers.entry(id).or_insert(handler);
    }

    pub(crate) fn has_scroll(&self, id: ElementId) -> bool {
        self.scroll_handlers.contains_key(&id)
    }

    pub(crate) fn take_key(&mut self, id: ElementId) -> Option<KeyHandler> {
        self.key_handlers.remove(&id)
    }

    pub(crate) fn begin_key(&mut self, id: ElementId) {
        self.key_dispatch.begin(id);
    }

    pub(crate) fn end_key(&mut self, id: ElementId) {
        self.key_dispatch.end(id);
    }

    pub(crate) fn key_removed(&mut self, id: ElementId) -> bool {
        self.key_dispatch.take_removed(id)
    }

    pub(crate) fn insert_key_if_absent(&mut self, id: ElementId, handler: KeyHandler) {
        self.key_handlers.entry(id).or_insert(handler);
    }

    pub(crate) fn has_key(&self, id: ElementId) -> bool {
        self.key_handlers.contains_key(&id)
    }

    pub(crate) fn hovered(&self) -> Option<ElementId> {
        self.hover.current_path().last().copied()
    }

    pub(crate) fn update_hover_path(&mut self, path: &[ElementId]) -> Vec<HoverEvent<ElementId>> {
        self.hover.update_path(path)
    }

    pub(crate) fn clear_hover(&mut self) -> Vec<HoverEvent<ElementId>> {
        self.hover.clear()
    }

    pub(crate) fn click_down(&mut self, target: ElementId, position: Point, time_ms: u64) {
        self.clicks.on_down(None, None, target, position, time_ms);
    }

    pub(crate) fn click_up(
        &mut self,
        target: &ElementId,
        position: Point,
        time_ms: u64,
    ) -> ClickResult<ElementId> {
        self.clicks.on_up(None, None, target, position, time_ms)
    }

    pub(crate) fn cancel_click(&mut self) {
        let _ = self.clicks.cancel(None);
    }

    pub(crate) fn press_target(&self) -> Option<ElementId> {
        self.press_target
    }

    pub(crate) fn take_press_target(&mut self) -> Option<ElementId> {
        self.press_target.take()
    }

    pub(crate) fn set_press_target(&mut self, target: Option<ElementId>) {
        self.press_target = target;
    }

    pub(crate) fn start_press(&mut self, position: Point) {
        self.press_position = Some(position);
        self.last_drag_position = Some(position);
    }

    pub(crate) fn clear_press(&mut self) {
        self.press_position = None;
        self.last_drag_position = None;
    }

    pub(crate) fn press_position(&self) -> Option<Point> {
        self.press_position
    }

    pub(crate) fn last_drag_position(&self) -> Option<Point> {
        self.last_drag_position
    }

    pub(crate) fn set_last_drag_position(&mut self, position: Point) {
        self.last_drag_position = Some(position);
    }
}

#[derive(Default)]
struct HandlerDispatchTracker {
    active: Vec<ElementId>,
    removals: HashSet<ElementId>,
}

impl HandlerDispatchTracker {
    fn begin(&mut self, id: ElementId) {
        self.active.push(id);
    }

    fn end(&mut self, id: ElementId) {
        let popped = self.active.pop();
        debug_assert!(
            popped == Some(id),
            "handler dispatch tracker should end the current handler"
        );
    }

    fn note_removal(&mut self, id: ElementId) {
        if self.active.contains(&id) {
            self.removals.insert(id);
        }
    }

    fn take_removed(&mut self, id: ElementId) -> bool {
        self.removals.remove(&id)
    }
}
