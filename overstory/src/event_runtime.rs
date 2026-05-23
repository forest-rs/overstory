// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::boxed::Box;
use alloc::vec::Vec;

use hashbrown::{HashMap, HashSet};
use kurbo::{Point, Vec2};
use ui_events::ScrollDelta;
use ui_events::keyboard::{KeyboardEvent, Modifiers};
use understory_event_state::click::{ClickResult, ClickState};
use understory_event_state::hover::{HoverEvent, HoverState};
use understory_responder::types::Phase;

use crate::element::ElementId;
use crate::ui::Ui;

pub(crate) type ClickHandler = Box<dyn FnMut(&mut Ui, ClickEvent<'_>)>;
pub(crate) type DragHandler = Box<dyn FnMut(&mut Ui, DragEvent<'_>)>;
pub(crate) type ScrollHandler = Box<dyn FnMut(&mut Ui, ScrollEvent<'_>)>;
pub(crate) type KeyHandler = Box<dyn FnMut(&mut Ui, KeyEvent<'_>)>;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum HandlerLane {
    User,
    Control,
}

/// The routed event family currently being dispatched.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RoutedEventKind {
    /// Keyboard input routed from the focused element.
    Key,
    /// Primary pointer click routed from the clicked element.
    Click,
    /// Primary pointer drag routed from the original press target.
    Drag,
    /// Pointer wheel/scroll routed from the hit element.
    Scroll,
}

/// Shared state carried by one routed event dispatch.
///
/// The state is created for a single responder route and is shared by every
/// handler invocation for that route. Calling [`Self::mark_handled`] or
/// [`Self::stop_propagation`] stops the remaining ordinary route; a future
/// handler registration API can add a "handled events too" opt-in without
/// changing event payloads.
#[derive(Debug)]
pub(crate) struct RoutedEventState {
    handled: core::cell::Cell<bool>,
    propagation_stopped: core::cell::Cell<bool>,
}

impl Default for RoutedEventState {
    fn default() -> Self {
        Self::new()
    }
}

impl RoutedEventState {
    /// Creates a fresh unhandled route state.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            handled: core::cell::Cell::new(false),
            propagation_stopped: core::cell::Cell::new(false),
        }
    }

    /// Marks this event handled.
    pub(crate) fn mark_handled(&self) {
        self.handled.set(true);
    }

    /// Returns whether this event has been marked handled.
    #[must_use]
    pub(crate) fn is_handled(&self) -> bool {
        self.handled.get()
    }

    /// Stops propagation without marking the event handled.
    pub(crate) fn stop_propagation(&self) {
        self.propagation_stopped.set(true);
    }

    /// Returns whether propagation has been explicitly stopped.
    #[must_use]
    pub(crate) fn is_propagation_stopped(&self) -> bool {
        self.propagation_stopped.get()
    }

    pub(crate) fn should_stop_route(&self) -> bool {
        self.is_handled() || self.is_propagation_stopped()
    }
}

/// Common route metadata attached to every routed event payload.
#[derive(Copy, Clone, Debug)]
pub struct RoutedEventContext<'a> {
    /// The event family being dispatched.
    pub kind: RoutedEventKind,
    /// The element the route is rooted at.
    pub source: ElementId,
    /// The element this handler is registered on.
    pub current: ElementId,
    /// Current propagation phase.
    pub phase: Phase,
    state: &'a RoutedEventState,
}

impl<'a> RoutedEventContext<'a> {
    pub(crate) fn new(
        kind: RoutedEventKind,
        source: ElementId,
        current: ElementId,
        phase: Phase,
        state: &'a RoutedEventState,
    ) -> Self {
        Self {
            kind,
            source,
            current,
            phase,
            state,
        }
    }

    /// Marks this event handled.
    ///
    /// Handled events stop the remaining ordinary route.
    pub fn mark_handled(&self) {
        self.state.mark_handled();
    }

    /// Returns whether this event has been marked handled.
    #[must_use]
    pub fn is_handled(&self) -> bool {
        self.state.is_handled()
    }

    /// Stops this event's remaining route without marking it handled.
    pub fn stop_propagation(&self) {
        self.state.stop_propagation();
    }

    /// Returns whether this event's propagation has been stopped.
    #[must_use]
    pub fn is_propagation_stopped(&self) -> bool {
        self.state.is_propagation_stopped()
    }
}

/// Result returned by routed event dispatch helpers.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct RoutedDispatchResult {
    /// Whether the routed event was consumed by a handler.
    pub handled: bool,
    /// Whether a handler explicitly stopped propagation.
    pub propagation_stopped: bool,
}

impl RoutedDispatchResult {
    pub(crate) fn from_state(state: &RoutedEventState) -> Self {
        Self {
            handled: state.is_handled(),
            propagation_stopped: state.is_propagation_stopped(),
        }
    }
}

/// Payload delivered to a key handler.
#[derive(Debug)]
pub struct KeyEvent<'a> {
    /// Shared routed-event metadata.
    pub route: RoutedEventContext<'a>,
    /// The full normalized keyboard event.
    pub key: &'a KeyboardEvent,
}

impl<'a> core::ops::Deref for KeyEvent<'a> {
    type Target = RoutedEventContext<'a>;

    fn deref(&self) -> &Self::Target {
        &self.route
    }
}

impl KeyEvent<'_> {
    /// Marks this key event handled.
    pub fn mark_handled(&self) {
        self.route.mark_handled();
    }

    /// Returns whether this key event has been marked handled.
    #[must_use]
    pub fn is_handled(&self) -> bool {
        self.route.is_handled()
    }

    /// Stops this key event's remaining route without marking it handled.
    pub fn stop_propagation(&self) {
        self.route.stop_propagation();
    }

    /// Returns whether this key event's propagation has been stopped.
    #[must_use]
    pub fn is_propagation_stopped(&self) -> bool {
        self.route.is_propagation_stopped()
    }
}

/// Result returned by [`Ui::dispatch_key`].
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyDispatchResult {
    /// Whether the key was consumed by framework policy or a key handler.
    pub handled: bool,
    /// Whether a key handler explicitly stopped propagation.
    pub propagation_stopped: bool,
    /// Whether dispatch changed the focused element.
    pub focus_changed: bool,
    /// Focused element after dispatch, if any.
    pub focused: Option<ElementId>,
}

/// Payload delivered to a drag handler as the primary pointer moves while
/// pressed.
#[derive(Copy, Clone, Debug)]
pub struct DragEvent<'a> {
    /// Shared routed-event metadata.
    pub route: RoutedEventContext<'a>,
    /// Current pointer position in physical pixels.
    pub position: Point,
    /// Pointer position when the press started, in physical pixels.
    pub press_position: Point,
    /// Movement since the previous drag dispatch in physical pixels.
    pub delta: Vec2,
}

impl<'a> core::ops::Deref for DragEvent<'a> {
    type Target = RoutedEventContext<'a>;

    fn deref(&self) -> &Self::Target {
        &self.route
    }
}

impl DragEvent<'_> {
    /// Marks this drag event handled and stops the remaining ordinary route.
    pub fn mark_handled(&self) {
        self.route.mark_handled();
    }

    /// Returns whether this drag event has been marked handled.
    #[must_use]
    pub fn is_handled(&self) -> bool {
        self.route.is_handled()
    }

    /// Stops this drag event's remaining route without marking it handled.
    pub fn stop_propagation(&self) {
        self.route.stop_propagation();
    }

    /// Returns whether this drag event's propagation has been stopped.
    #[must_use]
    pub fn is_propagation_stopped(&self) -> bool {
        self.route.is_propagation_stopped()
    }
}

/// Payload delivered to a click handler.
#[derive(Copy, Clone, Debug)]
pub struct ClickEvent<'a> {
    /// Shared routed-event metadata.
    pub route: RoutedEventContext<'a>,
    /// Pointer position at click-recognition time, in physical pixels.
    pub position: Point,
    /// Keyboard modifiers active when this click was recognized.
    pub modifiers: Modifiers,
}

impl<'a> core::ops::Deref for ClickEvent<'a> {
    type Target = RoutedEventContext<'a>;

    fn deref(&self) -> &Self::Target {
        &self.route
    }
}

impl ClickEvent<'_> {
    /// Marks this click event handled and stops the remaining ordinary route.
    pub fn mark_handled(&self) {
        self.route.mark_handled();
    }

    /// Returns whether this click event has been marked handled.
    #[must_use]
    pub fn is_handled(&self) -> bool {
        self.route.is_handled()
    }

    /// Stops this click event's remaining route without marking it handled.
    pub fn stop_propagation(&self) {
        self.route.stop_propagation();
    }

    /// Returns whether this click event's propagation has been stopped.
    #[must_use]
    pub fn is_propagation_stopped(&self) -> bool {
        self.route.is_propagation_stopped()
    }
}

/// Payload delivered to a scroll handler.
#[derive(Copy, Clone, Debug)]
pub struct ScrollEvent<'a> {
    /// Shared routed-event metadata.
    pub route: RoutedEventContext<'a>,
    /// Pointer position at scroll time, in physical pixels.
    pub position: Point,
    /// Raw scroll delta from `ui_events`.
    pub delta: ScrollDelta,
    /// Keyboard modifiers active when the scroll was requested.
    pub modifiers: Modifiers,
}

impl<'a> core::ops::Deref for ScrollEvent<'a> {
    type Target = RoutedEventContext<'a>;

    fn deref(&self) -> &Self::Target {
        &self.route
    }
}

impl ScrollEvent<'_> {
    /// Converts the event's raw delta into logical pixels.
    #[must_use]
    pub fn logical_delta(&self, line_size: Vec2, page_size: Vec2) -> Vec2 {
        match self.delta {
            ScrollDelta::PixelDelta(delta) => Vec2::new(delta.x, delta.y),
            ScrollDelta::LineDelta(x, y) => {
                Vec2::new(f64::from(x) * line_size.x, f64::from(y) * line_size.y)
            }
            ScrollDelta::PageDelta(x, y) => {
                Vec2::new(f64::from(x) * page_size.x, f64::from(y) * page_size.y)
            }
        }
    }

    /// Marks this scroll event handled and stops the remaining ordinary route.
    pub fn mark_handled(&self) {
        self.route.mark_handled();
    }

    /// Returns whether this scroll event has been marked handled.
    #[must_use]
    pub fn is_handled(&self) -> bool {
        self.route.is_handled()
    }

    /// Stops this scroll event's remaining route without marking it handled.
    pub fn stop_propagation(&self) {
        self.route.stop_propagation();
    }

    /// Returns whether this scroll event's propagation has been stopped.
    #[must_use]
    pub fn is_propagation_stopped(&self) -> bool {
        self.route.is_propagation_stopped()
    }
}

/// Routed event handler stores and in-flight removal tracking.
pub(crate) struct EventRuntime {
    click: HandlerPair<ClickHandler>,
    drag: HandlerPair<DragHandler>,
    scroll: HandlerPair<ScrollHandler>,
    key: HandlerPair<KeyHandler>,
    hover: HoverState<ElementId>,
    clicks: ClickState<ElementId>,
    press_target: Option<ElementId>,
    press_position: Option<Point>,
    last_drag_position: Option<Point>,
}

impl EventRuntime {
    pub(crate) fn new() -> Self {
        Self {
            click: HandlerPair::new(),
            drag: HandlerPair::new(),
            scroll: HandlerPair::new(),
            key: HandlerPair::new(),
            hover: HoverState::new(),
            clicks: ClickState::new(),
            press_target: None,
            press_position: None,
            last_drag_position: None,
        }
    }

    pub(crate) fn on_click(&mut self, id: ElementId, handler: ClickHandler) {
        self.click.insert(HandlerLane::User, id, handler);
    }

    pub(crate) fn on_control_click(&mut self, id: ElementId, handler: ClickHandler) {
        self.click.insert(HandlerLane::Control, id, handler);
    }

    pub(crate) fn remove_click(&mut self, id: ElementId) {
        self.click.remove(HandlerLane::User, id);
    }

    pub(crate) fn on_drag(&mut self, id: ElementId, handler: DragHandler) {
        self.drag.insert(HandlerLane::User, id, handler);
    }

    pub(crate) fn on_control_drag(&mut self, id: ElementId, handler: DragHandler) {
        self.drag.insert(HandlerLane::Control, id, handler);
    }

    pub(crate) fn remove_drag(&mut self, id: ElementId) {
        self.drag.remove(HandlerLane::User, id);
    }

    pub(crate) fn on_scroll(&mut self, id: ElementId, handler: ScrollHandler) {
        self.scroll.insert(HandlerLane::User, id, handler);
    }

    pub(crate) fn remove_scroll(&mut self, id: ElementId) {
        self.scroll.remove(HandlerLane::User, id);
    }

    pub(crate) fn on_key(&mut self, id: ElementId, handler: KeyHandler) {
        self.key.insert(HandlerLane::User, id, handler);
    }

    pub(crate) fn on_control_key(&mut self, id: ElementId, handler: KeyHandler) {
        self.key.insert(HandlerLane::Control, id, handler);
    }

    pub(crate) fn remove_key(&mut self, id: ElementId) {
        self.key.remove(HandlerLane::User, id);
    }

    pub(crate) fn remove_target(&mut self, id: ElementId) {
        self.click.remove_target(id);
        self.drag.remove_target(id);
        self.scroll.remove_target(id);
        self.key.remove_target(id);
    }

    pub(crate) fn take_click(&mut self, lane: HandlerLane, id: ElementId) -> Option<ClickHandler> {
        self.click.take(lane, id)
    }

    pub(crate) fn begin_click(&mut self, lane: HandlerLane, id: ElementId) {
        self.click.begin(lane, id);
    }

    pub(crate) fn end_click(&mut self, lane: HandlerLane, id: ElementId) {
        self.click.end(lane, id);
    }

    pub(crate) fn click_removed(&mut self, lane: HandlerLane, id: ElementId) -> bool {
        self.click.take_removed(lane, id)
    }

    pub(crate) fn insert_click_if_absent(
        &mut self,
        lane: HandlerLane,
        id: ElementId,
        handler: ClickHandler,
    ) {
        self.click.insert_if_absent(lane, id, handler);
    }

    pub(crate) fn has_click(&self, lane: HandlerLane, id: ElementId) -> bool {
        self.click.has(lane, id)
    }

    pub(crate) fn take_drag(&mut self, lane: HandlerLane, id: ElementId) -> Option<DragHandler> {
        self.drag.take(lane, id)
    }

    pub(crate) fn begin_drag(&mut self, lane: HandlerLane, id: ElementId) {
        self.drag.begin(lane, id);
    }

    pub(crate) fn end_drag(&mut self, lane: HandlerLane, id: ElementId) {
        self.drag.end(lane, id);
    }

    pub(crate) fn drag_removed(&mut self, lane: HandlerLane, id: ElementId) -> bool {
        self.drag.take_removed(lane, id)
    }

    pub(crate) fn insert_drag_if_absent(
        &mut self,
        lane: HandlerLane,
        id: ElementId,
        handler: DragHandler,
    ) {
        self.drag.insert_if_absent(lane, id, handler);
    }

    pub(crate) fn has_drag(&self, lane: HandlerLane, id: ElementId) -> bool {
        self.drag.has(lane, id)
    }

    pub(crate) fn take_scroll(
        &mut self,
        lane: HandlerLane,
        id: ElementId,
    ) -> Option<ScrollHandler> {
        self.scroll.take(lane, id)
    }

    pub(crate) fn begin_scroll(&mut self, lane: HandlerLane, id: ElementId) {
        self.scroll.begin(lane, id);
    }

    pub(crate) fn end_scroll(&mut self, lane: HandlerLane, id: ElementId) {
        self.scroll.end(lane, id);
    }

    pub(crate) fn scroll_removed(&mut self, lane: HandlerLane, id: ElementId) -> bool {
        self.scroll.take_removed(lane, id)
    }

    pub(crate) fn insert_scroll_if_absent(
        &mut self,
        lane: HandlerLane,
        id: ElementId,
        handler: ScrollHandler,
    ) {
        self.scroll.insert_if_absent(lane, id, handler);
    }

    pub(crate) fn has_scroll(&self, lane: HandlerLane, id: ElementId) -> bool {
        self.scroll.has(lane, id)
    }

    pub(crate) fn take_key(&mut self, lane: HandlerLane, id: ElementId) -> Option<KeyHandler> {
        self.key.take(lane, id)
    }

    pub(crate) fn begin_key(&mut self, lane: HandlerLane, id: ElementId) {
        self.key.begin(lane, id);
    }

    pub(crate) fn end_key(&mut self, lane: HandlerLane, id: ElementId) {
        self.key.end(lane, id);
    }

    pub(crate) fn key_removed(&mut self, lane: HandlerLane, id: ElementId) -> bool {
        self.key.take_removed(lane, id)
    }

    pub(crate) fn insert_key_if_absent(
        &mut self,
        lane: HandlerLane,
        id: ElementId,
        handler: KeyHandler,
    ) {
        self.key.insert_if_absent(lane, id, handler);
    }

    pub(crate) fn has_key(&self, lane: HandlerLane, id: ElementId) -> bool {
        self.key.has(lane, id)
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

struct HandlerPair<H> {
    user: HandlerSlot<H>,
    control: HandlerSlot<H>,
}

impl<H> HandlerPair<H> {
    fn new() -> Self {
        Self {
            user: HandlerSlot::new(),
            control: HandlerSlot::new(),
        }
    }

    fn insert(&mut self, lane: HandlerLane, id: ElementId, handler: H) {
        self.slot_mut(lane).insert(id, handler);
    }

    fn remove(&mut self, lane: HandlerLane, id: ElementId) {
        self.slot_mut(lane).remove(id);
    }

    fn remove_target(&mut self, id: ElementId) {
        self.user.remove(id);
        self.control.remove(id);
    }

    fn take(&mut self, lane: HandlerLane, id: ElementId) -> Option<H> {
        self.slot_mut(lane).take(id)
    }

    fn begin(&mut self, lane: HandlerLane, id: ElementId) {
        self.slot_mut(lane).begin(id);
    }

    fn end(&mut self, lane: HandlerLane, id: ElementId) {
        self.slot_mut(lane).end(id);
    }

    fn take_removed(&mut self, lane: HandlerLane, id: ElementId) -> bool {
        self.slot_mut(lane).take_removed(id)
    }

    fn insert_if_absent(&mut self, lane: HandlerLane, id: ElementId, handler: H) {
        self.slot_mut(lane).insert_if_absent(id, handler);
    }

    fn has(&self, lane: HandlerLane, id: ElementId) -> bool {
        self.slot(lane).has(id)
    }

    fn slot(&self, lane: HandlerLane) -> &HandlerSlot<H> {
        match lane {
            HandlerLane::User => &self.user,
            HandlerLane::Control => &self.control,
        }
    }

    fn slot_mut(&mut self, lane: HandlerLane) -> &mut HandlerSlot<H> {
        match lane {
            HandlerLane::User => &mut self.user,
            HandlerLane::Control => &mut self.control,
        }
    }
}

struct HandlerSlot<H> {
    handlers: HashMap<ElementId, H>,
    dispatch: HandlerDispatchTracker,
}

impl<H> HandlerSlot<H> {
    fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            dispatch: HandlerDispatchTracker::default(),
        }
    }

    fn insert(&mut self, id: ElementId, handler: H) {
        self.handlers.insert(id, handler);
    }

    fn remove(&mut self, id: ElementId) {
        self.handlers.remove(&id);
        self.dispatch.note_removal(id);
    }

    fn take(&mut self, id: ElementId) -> Option<H> {
        self.handlers.remove(&id)
    }

    fn begin(&mut self, id: ElementId) {
        self.dispatch.begin(id);
    }

    fn end(&mut self, id: ElementId) {
        self.dispatch.end(id);
    }

    fn take_removed(&mut self, id: ElementId) -> bool {
        self.dispatch.take_removed(id)
    }

    fn insert_if_absent(&mut self, id: ElementId, handler: H) {
        self.handlers.entry(id).or_insert(handler);
    }

    fn has(&self, id: ElementId) -> bool {
        self.handlers.contains_key(&id)
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
