// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Routed input dispatch and host event synthesis for `Ui`.

use alloc::vec::Vec;

use kurbo::{Point, Vec2};
use ui_events::ScrollDelta;
use ui_events::keyboard::{KeyboardEvent, Modifiers};
use ui_events::pointer::{
    PointerButton, PointerButtonEvent, PointerEvent, PointerInfo, PointerScrollEvent, PointerState,
    PointerUpdate,
};
use understory_event_state::click::ClickResult;
use understory_focus::Navigation;
use understory_responder::router::Router;
use understory_responder::types::{ParentLookup, Phase, WidgetLookup};
use understory_timing::TimerInstant;

use crate::element::{Element, ElementId};
use crate::event_runtime::{
    ClickEvent, DragEvent, FocusChangeReason, FocusEvent, HandlerLane, KeyDispatchResult, KeyEvent,
    PointerDispatchResult, PointerRoutedEvent, RoutedDispatchResult, RoutedEventContext,
    RoutedEventKind, RoutedEventState, RoutedHandlerOptions, ScrollEvent,
};
use crate::widget::Widget;

use super::Ui;

#[derive(Copy, Clone)]
pub(in crate::ui) struct PointerRouteData<'a> {
    raw: Option<&'a PointerEvent>,
    pointer: PointerInfo,
    state: Option<&'a PointerState>,
    position: Option<Point>,
    button: Option<PointerButton>,
    modifiers: Modifiers,
}

impl<'a> PointerRouteData<'a> {
    pub(in crate::ui) fn event(self, route: RoutedEventContext<'a>) -> PointerRoutedEvent<'a> {
        PointerRoutedEvent {
            route,
            pointer: self.pointer,
            state: self.state,
            position: self.position,
            button: self.button,
            modifiers: self.modifiers,
            raw: self.raw,
        }
    }
}

/// Extracts the host-supplied monotonic time from a pointer event,
/// if the variant carries one. Pointer events all expose a
/// nanosecond timestamp on the underlying `PointerState`.
fn pointer_event_time(event: &PointerEvent) -> Option<TimerInstant> {
    match event {
        PointerEvent::Move(update) => Some(update.current.time),
        PointerEvent::Down(b) | PointerEvent::Up(b) => Some(b.state.time),
        PointerEvent::Scroll(s) => Some(s.state.time),
        PointerEvent::Enter(_) | PointerEvent::Leave(_) | PointerEvent::Cancel(_) => None,
        _ => None,
    }
}

fn pointer_route_data_for_button<'a>(
    raw: &'a PointerEvent,
    event: &'a PointerButtonEvent,
) -> PointerRouteData<'a> {
    PointerRouteData {
        raw: Some(raw),
        pointer: event.pointer,
        state: Some(&event.state),
        position: Some(event.state.logical_point()),
        button: event.button,
        modifiers: event.state.modifiers,
    }
}

fn pointer_route_data_for_move<'a>(
    raw: &'a PointerEvent,
    event: &'a PointerUpdate,
) -> PointerRouteData<'a> {
    PointerRouteData {
        raw: Some(raw),
        pointer: event.pointer,
        state: Some(&event.current),
        position: Some(event.current.logical_point()),
        button: None,
        modifiers: event.current.modifiers,
    }
}

fn pointer_route_data_for_scroll<'a>(
    raw: &'a PointerEvent,
    event: &'a PointerScrollEvent,
) -> PointerRouteData<'a> {
    PointerRouteData {
        raw: Some(raw),
        pointer: event.pointer,
        state: Some(&event.state),
        position: Some(event.state.logical_point()),
        button: None,
        modifiers: event.state.modifiers,
    }
}

fn pointer_route_data_for_info(raw: &PointerEvent, pointer: PointerInfo) -> PointerRouteData<'_> {
    PointerRouteData {
        raw: Some(raw),
        pointer,
        state: None,
        position: None,
        button: None,
        modifiers: Modifiers::default(),
    }
}

fn focus_navigation_for_key(event: &KeyboardEvent) -> Option<Navigation> {
    use ui_events::keyboard::{Key, KeyState, NamedKey};

    if event.state != KeyState::Down || !matches!(&event.key, Key::Named(NamedKey::Tab)) {
        return None;
    }
    Some(if event.modifiers.shift() {
        Navigation::Prev
    } else {
        Navigation::Next
    })
}

impl Ui {
    /// Moves focus per `direction` (e.g. [`Navigation::Next`] for
    /// Tab, [`Navigation::Prev`] for Shift+Tab). Uses
    /// [`understory_focus::DefaultPolicy`] with `WrapMode::Scope` so
    /// focus cycles within the tree. Returns the new focused element.
    pub fn focus_navigate(&mut self, direction: Navigation) -> Option<ElementId> {
        self.geometry.commit();
        let root_node = self.box_for(self.root())?;
        // Resolve the current focus's node before we borrow `tree`
        // and `focus_buf` to build the FocusSpace.
        let current_node = self.focus.focused().and_then(|id| self.box_for(id));
        let new_node =
            self.focus
                .navigate(self.geometry.tree(), root_node, current_node, direction);
        let new_element = new_node.and_then(|n| self.node_to_element(n));
        self.set_keyboard_focus(new_element);
        new_element
    }

    /// Registers a key handler on `id`. The handler runs along the
    /// responder capture → target → bubble sequence rooted at the
    /// currently focused element when [`Self::dispatch_key`] is
    /// called. One handler per element for now; subsequent calls
    /// replace the previous handler.
    pub fn on_key<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Self, KeyEvent<'_>) + 'static,
    {
        self.events.on_key(id, Box::new(handler));
    }

    /// Registers a key handler with explicit routed handler options.
    pub fn on_key_with_options<F>(
        &mut self,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: F,
    ) where
        F: FnMut(&mut Self, KeyEvent<'_>) + 'static,
    {
        self.events
            .on_key_with_options(id, options, Box::new(handler));
    }

    pub(crate) fn on_control_key<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Self, KeyEvent<'_>) + 'static,
    {
        self.events.on_control_key(id, Box::new(handler));
    }

    /// Removes a key handler previously registered with
    /// [`Self::on_key`]. No-op if none is registered.
    pub fn remove_key_handler(&mut self, id: ElementId) {
        self.events.remove_key(id);
    }

    /// Registers a focus handler on `id`.
    ///
    /// The handler receives both focus gained and focus lost events; inspect
    /// `event.kind` to distinguish them.
    pub fn on_focus<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Self, FocusEvent<'_>) + 'static,
    {
        self.events.on_focus(id, Box::new(handler));
    }

    /// Registers a focus handler with explicit routed handler options.
    pub fn on_focus_with_options<F>(
        &mut self,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: F,
    ) where
        F: FnMut(&mut Self, FocusEvent<'_>) + 'static,
    {
        self.events
            .on_focus_with_options(id, options, Box::new(handler));
    }

    /// Removes a focus handler previously registered with [`Self::on_focus`].
    pub fn remove_focus_handler(&mut self, id: ElementId) {
        self.events.remove_focus(id);
    }

    fn on_pointer_kind<F>(&mut self, kind: RoutedEventKind, id: ElementId, handler: F)
    where
        F: FnMut(&mut Self, PointerRoutedEvent<'_>) + 'static,
    {
        self.events.on_pointer(kind, id, Box::new(handler));
    }

    fn on_pointer_kind_with_options<F>(
        &mut self,
        kind: RoutedEventKind,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: F,
    ) where
        F: FnMut(&mut Self, PointerRoutedEvent<'_>) + 'static,
    {
        self.events
            .on_pointer_with_options(kind, id, options, Box::new(handler));
    }

    /// Registers a pointer-down handler on `id`.
    pub fn on_pointer_down<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Self, PointerRoutedEvent<'_>) + 'static,
    {
        self.on_pointer_kind(RoutedEventKind::PointerDown, id, handler);
    }

    /// Registers a pointer-down handler with explicit routed handler options.
    pub fn on_pointer_down_with_options<F>(
        &mut self,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: F,
    ) where
        F: FnMut(&mut Self, PointerRoutedEvent<'_>) + 'static,
    {
        self.on_pointer_kind_with_options(RoutedEventKind::PointerDown, id, options, handler);
    }

    /// Removes a pointer-down handler previously registered with
    /// [`Self::on_pointer_down`].
    pub fn remove_pointer_down_handler(&mut self, id: ElementId) {
        self.events.remove_pointer(RoutedEventKind::PointerDown, id);
    }

    /// Registers a pointer-move handler on `id`.
    pub fn on_pointer_move<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Self, PointerRoutedEvent<'_>) + 'static,
    {
        self.on_pointer_kind(RoutedEventKind::PointerMove, id, handler);
    }

    /// Registers a pointer-move handler with explicit routed handler options.
    pub fn on_pointer_move_with_options<F>(
        &mut self,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: F,
    ) where
        F: FnMut(&mut Self, PointerRoutedEvent<'_>) + 'static,
    {
        self.on_pointer_kind_with_options(RoutedEventKind::PointerMove, id, options, handler);
    }

    /// Removes a pointer-move handler previously registered with
    /// [`Self::on_pointer_move`].
    pub fn remove_pointer_move_handler(&mut self, id: ElementId) {
        self.events.remove_pointer(RoutedEventKind::PointerMove, id);
    }

    /// Registers a pointer-up handler on `id`.
    pub fn on_pointer_up<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Self, PointerRoutedEvent<'_>) + 'static,
    {
        self.on_pointer_kind(RoutedEventKind::PointerUp, id, handler);
    }

    /// Registers a pointer-up handler with explicit routed handler options.
    pub fn on_pointer_up_with_options<F>(
        &mut self,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: F,
    ) where
        F: FnMut(&mut Self, PointerRoutedEvent<'_>) + 'static,
    {
        self.on_pointer_kind_with_options(RoutedEventKind::PointerUp, id, options, handler);
    }

    /// Removes a pointer-up handler previously registered with
    /// [`Self::on_pointer_up`].
    pub fn remove_pointer_up_handler(&mut self, id: ElementId) {
        self.events.remove_pointer(RoutedEventKind::PointerUp, id);
    }

    /// Registers a pointer-enter handler on `id`.
    pub fn on_pointer_enter<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Self, PointerRoutedEvent<'_>) + 'static,
    {
        self.on_pointer_kind(RoutedEventKind::PointerEnter, id, handler);
    }

    /// Registers a pointer-enter handler with explicit routed handler options.
    pub fn on_pointer_enter_with_options<F>(
        &mut self,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: F,
    ) where
        F: FnMut(&mut Self, PointerRoutedEvent<'_>) + 'static,
    {
        self.on_pointer_kind_with_options(RoutedEventKind::PointerEnter, id, options, handler);
    }

    /// Removes a pointer-enter handler previously registered with
    /// [`Self::on_pointer_enter`].
    pub fn remove_pointer_enter_handler(&mut self, id: ElementId) {
        self.events
            .remove_pointer(RoutedEventKind::PointerEnter, id);
    }

    /// Registers a pointer-leave handler on `id`.
    pub fn on_pointer_leave<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Self, PointerRoutedEvent<'_>) + 'static,
    {
        self.on_pointer_kind(RoutedEventKind::PointerLeave, id, handler);
    }

    /// Registers a pointer-leave handler with explicit routed handler options.
    pub fn on_pointer_leave_with_options<F>(
        &mut self,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: F,
    ) where
        F: FnMut(&mut Self, PointerRoutedEvent<'_>) + 'static,
    {
        self.on_pointer_kind_with_options(RoutedEventKind::PointerLeave, id, options, handler);
    }

    /// Removes a pointer-leave handler previously registered with
    /// [`Self::on_pointer_leave`].
    pub fn remove_pointer_leave_handler(&mut self, id: ElementId) {
        self.events
            .remove_pointer(RoutedEventKind::PointerLeave, id);
    }

    /// Registers a pointer-cancel handler on `id`.
    pub fn on_pointer_cancel<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Self, PointerRoutedEvent<'_>) + 'static,
    {
        self.on_pointer_kind(RoutedEventKind::PointerCancel, id, handler);
    }

    /// Registers a pointer-cancel handler with explicit routed handler options.
    pub fn on_pointer_cancel_with_options<F>(
        &mut self,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: F,
    ) where
        F: FnMut(&mut Self, PointerRoutedEvent<'_>) + 'static,
    {
        self.on_pointer_kind_with_options(RoutedEventKind::PointerCancel, id, options, handler);
    }

    /// Removes a pointer-cancel handler previously registered with
    /// [`Self::on_pointer_cancel`].
    pub fn remove_pointer_cancel_handler(&mut self, id: ElementId) {
        self.events
            .remove_pointer(RoutedEventKind::PointerCancel, id);
    }

    /// Runs `dispatch_current` across the responder route for `source`.
    ///
    /// Route construction belongs to [`understory_responder::Router`]; this
    /// helper owns only Overstory's execution semantics: temporarily remove a
    /// handler before invoking it, share routed event state across invocations,
    /// skip ordinary handlers once an event is handled, and stop dispatch only
    /// when propagation is explicitly stopped.
    fn dispatch_routed(
        &mut self,
        kind: RoutedEventKind,
        source: ElementId,
        mut dispatch_current: impl FnMut(&mut Self, RoutedEventContext<'_>),
    ) -> RoutedDispatchResult {
        let state = RoutedEventState::new();
        for (phase, node) in self.responder_sequence(source) {
            let route = RoutedEventContext::new(kind, source, node, phase, &state);
            dispatch_current(self, route);
            if state.should_stop_route() {
                break;
            }
        }
        RoutedDispatchResult::from_state(&state)
    }

    pub(crate) fn with_widget_mut(
        &mut self,
        id: ElementId,
        dispatch: impl FnOnce(&mut dyn Widget, &mut Self),
    ) {
        let Some(mut widget) = self.elements.get_mut(id).and_then(Element::take_widget) else {
            return;
        };

        dispatch(widget.as_mut(), self);

        if let Some(element) = self.elements.get_mut(id) {
            element.restore_widget(widget);
        }
    }

    fn dispatch_key_handler_lane<'a>(
        &mut self,
        lane: HandlerLane,
        route: RoutedEventContext<'a>,
        event: &'a KeyboardEvent,
    ) {
        let node = route.current;
        let Some(mut registration) = self.events.take_key(lane, node, route.is_handled()) else {
            return;
        };
        let ev = KeyEvent { route, key: event };
        self.events.begin_key(lane, node);
        (registration.handler)(self, ev);
        self.events.end_key(lane, node);
        let handler_removed = self.events.key_removed(lane, node);
        if self.get(node).is_some() && !handler_removed && !self.events.has_key(lane, node) {
            self.events.insert_key_if_absent(lane, node, registration);
        }
    }

    fn dispatch_widget_key<'a>(&mut self, route: RoutedEventContext<'a>, event: &'a KeyboardEvent) {
        let node = route.current;
        self.with_widget_mut(node, |widget, ui| {
            widget.handle_key(node, ui, KeyEvent { route, key: event });
        });
    }

    fn dispatch_click_handler_lane(
        &mut self,
        lane: HandlerLane,
        route: RoutedEventContext<'_>,
        position: Point,
        modifiers: Modifiers,
    ) {
        let node = route.current;
        let Some(mut registration) = self.events.take_click(lane, node, route.is_handled()) else {
            return;
        };
        let event = ClickEvent {
            route,
            position,
            modifiers,
        };
        self.events.begin_click(lane, node);
        (registration.handler)(self, event);
        self.events.end_click(lane, node);
        let handler_removed = self.events.click_removed(lane, node);
        if self.get(node).is_some() && !handler_removed && !self.events.has_click(lane, node) {
            self.events.insert_click_if_absent(lane, node, registration);
        }
    }

    fn dispatch_widget_click(
        &mut self,
        route: RoutedEventContext<'_>,
        position: Point,
        modifiers: Modifiers,
    ) {
        let node = route.current;
        self.with_widget_mut(node, |widget, ui| {
            widget.handle_click(
                node,
                ui,
                ClickEvent {
                    route,
                    position,
                    modifiers,
                },
            );
        });
    }

    fn dispatch_drag_handler_lane(
        &mut self,
        lane: HandlerLane,
        route: RoutedEventContext<'_>,
        position: Point,
        press_position: Point,
        delta: Vec2,
    ) {
        let node = route.current;
        let Some(mut registration) = self.events.take_drag(lane, node, route.is_handled()) else {
            return;
        };
        let event = DragEvent {
            route,
            position,
            press_position,
            delta,
        };
        self.events.begin_drag(lane, node);
        (registration.handler)(self, event);
        self.events.end_drag(lane, node);
        let handler_removed = self.events.drag_removed(lane, node);
        if self.get(node).is_some() && !handler_removed && !self.events.has_drag(lane, node) {
            self.events.insert_drag_if_absent(lane, node, registration);
        }
    }

    fn dispatch_widget_drag(
        &mut self,
        route: RoutedEventContext<'_>,
        position: Point,
        press_position: Point,
        delta: Vec2,
    ) {
        let node = route.current;
        self.with_widget_mut(node, |widget, ui| {
            widget.handle_drag(
                node,
                ui,
                DragEvent {
                    route,
                    position,
                    press_position,
                    delta,
                },
            );
        });
    }

    fn dispatch_scroll_handler_lane(
        &mut self,
        lane: HandlerLane,
        route: RoutedEventContext<'_>,
        position: Point,
        delta: ScrollDelta,
        modifiers: Modifiers,
    ) {
        let node = route.current;
        let Some(mut registration) = self.events.take_scroll(lane, node, route.is_handled()) else {
            return;
        };
        let event = ScrollEvent {
            route,
            position,
            delta,
            modifiers,
        };
        self.events.begin_scroll(lane, node);
        (registration.handler)(self, event);
        self.events.end_scroll(lane, node);
        let handler_removed = self.events.scroll_removed(lane, node);
        if self.get(node).is_some() && !handler_removed && !self.events.has_scroll(lane, node) {
            self.events
                .insert_scroll_if_absent(lane, node, registration);
        }
    }

    fn dispatch_widget_scroll(
        &mut self,
        route: RoutedEventContext<'_>,
        position: Point,
        delta: ScrollDelta,
        modifiers: Modifiers,
    ) {
        let node = route.current;
        self.with_widget_mut(node, |widget, ui| {
            widget.handle_scroll(
                node,
                ui,
                ScrollEvent {
                    route,
                    position,
                    delta,
                    modifiers,
                },
            );
        });
    }

    fn dispatch_focus_handler_lane(
        &mut self,
        lane: HandlerLane,
        route: RoutedEventContext<'_>,
        previous: Option<ElementId>,
        focused: Option<ElementId>,
        focus_visible: bool,
        reason: FocusChangeReason,
    ) {
        let node = route.current;
        let Some(mut registration) = self.events.take_focus(lane, node, route.is_handled()) else {
            return;
        };
        let event = FocusEvent {
            route,
            previous,
            focused,
            focus_visible,
            reason,
        };
        self.events.begin_focus(lane, node);
        (registration.handler)(self, event);
        self.events.end_focus(lane, node);
        let handler_removed = self.events.focus_removed(lane, node);
        if self.get(node).is_some() && !handler_removed && !self.events.has_focus(lane, node) {
            self.events.insert_focus_if_absent(lane, node, registration);
        }
    }

    fn dispatch_widget_focus(
        &mut self,
        route: RoutedEventContext<'_>,
        previous: Option<ElementId>,
        focused: Option<ElementId>,
        focus_visible: bool,
        reason: FocusChangeReason,
    ) {
        let node = route.current;
        self.with_widget_mut(node, |widget, ui| {
            widget.handle_focus(
                node,
                ui,
                FocusEvent {
                    route,
                    previous,
                    focused,
                    focus_visible,
                    reason,
                },
            );
        });
    }

    fn dispatch_pointer_handler_lane(
        &mut self,
        lane: HandlerLane,
        route: RoutedEventContext<'_>,
        data: PointerRouteData<'_>,
    ) {
        let node = route.current;
        let Some(mut registration) =
            self.events
                .take_pointer(lane, route.kind, node, route.is_handled())
        else {
            return;
        };
        let event = data.event(route);
        self.events.begin_pointer(lane, event.kind, node);
        (registration.handler)(self, event);
        self.events.end_pointer(lane, route.kind, node);
        let handler_removed = self.events.pointer_removed(lane, route.kind, node);
        if self.get(node).is_some()
            && !handler_removed
            && !self.events.has_pointer(lane, route.kind, node)
        {
            self.events
                .insert_pointer_if_absent(lane, route.kind, node, registration);
        }
    }

    fn dispatch_widget_pointer(
        &mut self,
        route: RoutedEventContext<'_>,
        data: PointerRouteData<'_>,
    ) {
        let node = route.current;
        self.with_widget_mut(node, |widget, ui| {
            widget.handle_pointer(node, ui, data.event(route));
        });
    }

    pub(in crate::ui) fn dispatch_routed_pointer_event(
        &mut self,
        kind: RoutedEventKind,
        source: ElementId,
        data: PointerRouteData<'_>,
    ) -> RoutedDispatchResult {
        self.dispatch_routed(kind, source, |ui, route| {
            ui.dispatch_pointer_handler_lane(HandlerLane::User, route, data);
            if !route.is_handled() && !route.is_propagation_stopped() {
                ui.dispatch_widget_pointer(route, data);
            }
            if !route.is_handled() && !route.is_propagation_stopped() {
                ui.dispatch_pointer_handler_lane(HandlerLane::Control, route, data);
            }
        })
    }

    fn dispatch_focus_event(
        &mut self,
        kind: RoutedEventKind,
        source: ElementId,
        previous: Option<ElementId>,
        focused: Option<ElementId>,
        focus_visible: bool,
        reason: FocusChangeReason,
    ) -> RoutedDispatchResult {
        self.dispatch_routed(kind, source, |ui, route| {
            ui.dispatch_focus_handler_lane(
                HandlerLane::User,
                route,
                previous,
                focused,
                focus_visible,
                reason,
            );
            if !route.is_handled() && !route.is_propagation_stopped() {
                ui.dispatch_widget_focus(route, previous, focused, focus_visible, reason);
            }
            if !route.is_handled() && !route.is_propagation_stopped() {
                ui.dispatch_focus_handler_lane(
                    HandlerLane::Control,
                    route,
                    previous,
                    focused,
                    focus_visible,
                    reason,
                );
            }
        })
    }

    pub(in crate::ui) fn dispatch_focus_change(
        &mut self,
        previous: Option<ElementId>,
        focused: Option<ElementId>,
        focus_visible: bool,
        reason: FocusChangeReason,
    ) {
        if let Some(previous) = previous
            && self.get(previous).is_some()
        {
            self.dispatch_focus_event(
                RoutedEventKind::FocusLost,
                previous,
                Some(previous),
                focused,
                focus_visible,
                reason,
            );
        }
        if let Some(focused_id) = focused
            && self.get(focused_id).is_some()
        {
            self.dispatch_focus_event(
                RoutedEventKind::FocusGained,
                focused_id,
                previous,
                Some(focused_id),
                focus_visible,
                reason,
            );
        }
    }

    fn responder_sequence(&self, source: ElementId) -> Vec<(Phase, ElementId)> {
        let lookup = UiWidgetLookup;
        let parent = UiParentLookup { ui: self };
        let router: Router<ElementId, UiWidgetLookup, UiParentLookup<'_>> =
            Router::with_parent(lookup, parent);
        router
            .dispatch_for::<()>(source)
            .into_iter()
            .map(|d| (d.phase, d.node))
            .collect()
    }

    /// Routes `event` through the framework keyboard policy and,
    /// when appropriate, to the currently focused element.
    ///
    /// Tab / Shift+Tab focus navigation is handled by `Ui` before
    /// ordinary key handlers run. Other keys route along the
    /// responder capture → target → bubble path rooted at the
    /// focused element. A handler can call [`KeyEvent::mark_handled`]
    /// to skip ordinary handlers that did not opt into handled events.
    #[must_use]
    pub fn dispatch_key(&mut self, event: &KeyboardEvent) -> KeyDispatchResult {
        let previous_focus = self.focus.focused();
        if let Some(direction) = focus_navigation_for_key(event) {
            let focused = self.focus_navigate(direction);
            return KeyDispatchResult {
                handled: focused.is_some(),
                propagation_stopped: false,
                focus_changed: previous_focus != self.focus.focused(),
                focused: self.focus.focused(),
            };
        }

        let Some(focused) = self.focus.focused() else {
            return KeyDispatchResult::default();
        };
        let routed = self.dispatch_routed(RoutedEventKind::Key, focused, |ui, route| {
            ui.dispatch_key_handler_lane(HandlerLane::User, route, event);
            if !route.is_handled() && !route.is_propagation_stopped() {
                ui.dispatch_widget_key(route, event);
            }
            if !route.is_handled() && !route.is_propagation_stopped() {
                ui.dispatch_key_handler_lane(HandlerLane::Control, route, event);
            }
        });

        KeyDispatchResult {
            handled: routed.handled,
            propagation_stopped: routed.propagation_stopped,
            focus_changed: previous_focus != self.focus.focused(),
            focused: self.focus.focused(),
        }
    }

    /// Registers a click handler on `id`. The handler runs along the
    /// responder capture → target → bubble sequence when
    /// the click recognizer reports a primary-button click whose target
    /// path includes this element. One handler per element for now;
    /// subsequent calls replace the previous handler.
    pub fn on_click<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Self, ClickEvent<'_>) + 'static,
    {
        self.events.on_click(id, Box::new(handler));
    }

    /// Registers a click handler with explicit routed handler options.
    pub fn on_click_with_options<F>(
        &mut self,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: F,
    ) where
        F: FnMut(&mut Self, ClickEvent<'_>) + 'static,
    {
        self.events
            .on_click_with_options(id, options, Box::new(handler));
    }

    pub(crate) fn on_control_click<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Self, ClickEvent<'_>) + 'static,
    {
        self.events.on_control_click(id, Box::new(handler));
    }

    /// Removes a click handler previously registered with
    /// [`Self::on_click`]. No-op if no handler is registered.
    pub fn remove_click_handler(&mut self, id: ElementId) {
        self.events.remove_click(id);
    }

    /// Registers a drag handler on `id`. Fires on every
    /// primary-button `Move` while the press is active, along the
    /// responder capture → target → bubble path rooted at the
    /// press target. Useful for sliders, resizers, draggable
    /// items. One handler per element; subsequent calls replace.
    pub fn on_drag<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Self, DragEvent<'_>) + 'static,
    {
        self.events.on_drag(id, Box::new(handler));
    }

    /// Registers a drag handler with explicit routed handler options.
    pub fn on_drag_with_options<F>(
        &mut self,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: F,
    ) where
        F: FnMut(&mut Self, DragEvent<'_>) + 'static,
    {
        self.events
            .on_drag_with_options(id, options, Box::new(handler));
    }

    pub(crate) fn on_control_drag<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Self, DragEvent<'_>) + 'static,
    {
        self.events.on_control_drag(id, Box::new(handler));
    }

    /// Removes a drag handler previously registered with
    /// [`Self::on_drag`]. No-op if no handler is registered.
    pub fn remove_drag_handler(&mut self, id: ElementId) {
        self.events.remove_drag(id);
    }

    /// Registers a scroll handler on `id`. The handler runs along the
    /// responder capture → target → bubble sequence rooted at the element
    /// under the pointer when a scroll event arrives.
    pub fn on_scroll<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Self, ScrollEvent<'_>) + 'static,
    {
        self.events.on_scroll(id, Box::new(handler));
    }

    /// Registers a scroll handler with explicit routed handler options.
    pub fn on_scroll_with_options<F>(
        &mut self,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: F,
    ) where
        F: FnMut(&mut Self, ScrollEvent<'_>) + 'static,
    {
        self.events
            .on_scroll_with_options(id, options, Box::new(handler));
    }

    /// Removes a scroll handler previously registered with [`Self::on_scroll`].
    /// No-op if no handler is registered.
    pub fn remove_scroll_handler(&mut self, id: ElementId) {
        self.events.remove_scroll(id);
    }

    /// Builds the responder dispatch sequence for the current
    /// press target and runs each drag handler along it. Called
    /// from `dispatch_pointer_event` on every `Move` while a press
    /// is active.
    pub(in crate::ui) fn dispatch_drag(&mut self, position: Point) -> RoutedDispatchResult {
        let Some(source) = self.events.pointer_capture() else {
            return RoutedDispatchResult::default();
        };
        let Some(press_position) = self.events.press_position() else {
            return RoutedDispatchResult::default();
        };
        let last = self.events.last_drag_position().unwrap_or(press_position);
        let delta = position - last;
        self.events.set_last_drag_position(position);

        self.dispatch_routed(RoutedEventKind::Drag, source, |ui, route| {
            ui.dispatch_drag_handler_lane(
                HandlerLane::User,
                route,
                position,
                press_position,
                delta,
            );
            if !route.is_handled() && !route.is_propagation_stopped() {
                ui.dispatch_widget_drag(route, position, press_position, delta);
            }
            if !route.is_handled() && !route.is_propagation_stopped() {
                ui.dispatch_drag_handler_lane(
                    HandlerLane::Control,
                    route,
                    position,
                    press_position,
                    delta,
                );
            }
        })
    }

    /// Programmatically synthesize a click on `target` at
    /// `position`. Used by widgets that translate non-pointer input
    /// (e.g. Button's Space / Enter key activation) into the same
    /// click dispatch the pointer pipeline would produce.
    pub fn synthesize_click(&mut self, target: ElementId, position: Point) -> RoutedDispatchResult {
        self.dispatch_click(target, position)
    }

    /// Programmatically synthesize a click with explicit keyboard
    /// modifiers.
    pub fn synthesize_click_with_modifiers(
        &mut self,
        target: ElementId,
        position: Point,
        modifiers: Modifiers,
    ) -> RoutedDispatchResult {
        self.dispatch_click_with_modifiers(target, position, modifiers)
    }

    /// Builds the responder dispatch sequence for `target` and runs
    /// each element's click handler along it (capture, then target,
    /// then bubble). Called when the click recognizer returns
    /// [`ClickResult::Click`] or by [`Self::synthesize_click`].
    pub(in crate::ui) fn dispatch_click(
        &mut self,
        target: ElementId,
        position: Point,
    ) -> RoutedDispatchResult {
        self.dispatch_click_with_modifiers(target, position, Modifiers::default())
    }

    fn dispatch_click_with_modifiers(
        &mut self,
        target: ElementId,
        position: Point,
        modifiers: Modifiers,
    ) -> RoutedDispatchResult {
        if !self.is_enabled(self.press_visual_target(target)) {
            return RoutedDispatchResult::default();
        }

        self.dispatch_routed(RoutedEventKind::Click, target, |ui, route| {
            ui.dispatch_click_handler_lane(HandlerLane::User, route, position, modifiers);
            if !route.is_handled() && !route.is_propagation_stopped() {
                ui.dispatch_widget_click(route, position, modifiers);
            }
            if !route.is_handled() && !route.is_propagation_stopped() {
                ui.dispatch_click_handler_lane(HandlerLane::Control, route, position, modifiers);
            }
        })
    }

    /// Programmatically synthesize a scroll routed at `target`.
    pub fn synthesize_scroll(
        &mut self,
        target: ElementId,
        position: Point,
        delta: ScrollDelta,
    ) -> RoutedDispatchResult {
        self.dispatch_scroll_with_modifiers(target, position, delta, Modifiers::default())
    }

    fn dispatch_scroll_with_modifiers(
        &mut self,
        target: ElementId,
        position: Point,
        delta: ScrollDelta,
        modifiers: Modifiers,
    ) -> RoutedDispatchResult {
        if !self.is_enabled(self.press_visual_target(target)) {
            return RoutedDispatchResult::default();
        }

        self.dispatch_routed(RoutedEventKind::Scroll, target, |ui, route| {
            ui.dispatch_scroll_handler_lane(HandlerLane::User, route, position, delta, modifiers);
            if !route.is_handled() && !route.is_propagation_stopped() {
                ui.dispatch_widget_scroll(route, position, delta, modifiers);
            }
            if !route.is_handled() && !route.is_propagation_stopped() {
                ui.dispatch_scroll_handler_lane(
                    HandlerLane::Control,
                    route,
                    position,
                    delta,
                    modifiers,
                );
            }
        })
    }

    /// Dispatches a normalized pointer event.
    ///
    /// Handles routed pointer down/move/up/enter/leave/cancel, hover,
    /// primary-button press/click/drag, and routed scroll. Gesture events are
    /// ignored for now.
    ///
    /// `:pressed` follows WPF semantics: a primary-button press marks the
    /// control visual pressed and that visual state survives even if the cursor
    /// leaves it. The original hit element is pointer-captured for routed drag
    /// events until the matching Up or Cancel unless a pointer-down handler
    /// captures another element first.
    ///
    /// Returns what changed or was handled while dispatching the event.
    pub fn dispatch_pointer_event(&mut self, event: &PointerEvent) -> PointerDispatchResult {
        // Update the cached `now` from whichever PointerState the
        // event carries — pointer events all expose a `time` in
        // nanoseconds. Handlers and templates can use `Ui::now()`
        // without threading it through every call.
        if let Some(t) = pointer_event_time(event) {
            self.timers.set_now(t);
        }
        let focused_before = self.focus.focused();
        let capture_before = self.events.pointer_capture();

        let mut result = match event {
            PointerEvent::Move(update) => {
                let data = pointer_route_data_for_move(event, update);
                let pos = update.current.logical_point();
                let mut result = PointerDispatchResult::default();
                self.update_hover_at_with_pointer(pos, data, &mut result);
                let source = self.events.pointer_capture().or_else(|| self.hit_test(pos));
                let pointer_move = source
                    .filter(|source| self.is_enabled(self.press_visual_target(*source)))
                    .map(|source| {
                        self.dispatch_routed_pointer_event(
                            RoutedEventKind::PointerMove,
                            source,
                            data,
                        )
                    })
                    .unwrap_or_default();
                result.include_routed(pointer_move);
                // If a primary press is captured, fan the move out to drag
                // handlers along the captured source's responder path after
                // ordinary pointer-move handlers have had first refusal.
                if self.events.pointer_capture().is_some()
                    && !pointer_move.handled
                    && !pointer_move.propagation_stopped
                {
                    result.include_routed(self.dispatch_drag(pos));
                }
                result
            }
            PointerEvent::Down(button_event) => {
                let data = pointer_route_data_for_button(event, button_event);
                let pos = button_event.state.logical_point();
                let time_ms = button_event.state.time / 1_000_000;
                let mut result = PointerDispatchResult::default();
                // A Down without a preceding Move can arrive on some
                // platforms; refresh hover first so any newly-entered
                // path is set up before we press.
                self.update_hover_at_with_pointer(pos, data, &mut result);
                if let Some(target) = self.hit_test(pos) {
                    let visual = self.press_visual_target(target);
                    if !self.is_enabled(visual) {
                        self.events.cancel_click();
                        result.press_changed = self.set_press_target(None);
                        result.capture_changed = self.set_pointer_capture(None);
                        self.events.clear_press();
                    } else {
                        if self.is_focusable(visual) {
                            result.focus_changed = self.set_pointer_focus(Some(visual));
                        }
                        let pointer_down = self.dispatch_routed_pointer_event(
                            RoutedEventKind::PointerDown,
                            target,
                            data,
                        );
                        result.include_routed(pointer_down);
                        if button_event.button == Some(PointerButton::Primary)
                            && !pointer_down.handled
                            && !pointer_down.propagation_stopped
                        {
                            self.events.click_down(target, pos, time_ms);
                            result.press_changed = self.set_press_target(Some(visual));
                            if self.events.pointer_capture().is_none() {
                                result.capture_changed = self.set_pointer_capture(Some(target));
                            }
                            self.events.start_press(pos);
                        }
                    }
                } else {
                    result.press_changed = self.set_press_target(None);
                    result.capture_changed = self.set_pointer_capture(None);
                    self.events.clear_press();
                }
                result
            }
            PointerEvent::Up(button_event) => {
                let data = pointer_route_data_for_button(event, button_event);
                let pos = button_event.state.logical_point();
                let time_ms = button_event.state.time / 1_000_000;
                let hit = self.hit_test(pos);
                let source = self.events.pointer_capture().or(hit);
                let pointer_up = source
                    .filter(|source| self.is_enabled(self.press_visual_target(*source)))
                    .map(|source| {
                        self.dispatch_routed_pointer_event(RoutedEventKind::PointerUp, source, data)
                    })
                    .unwrap_or_default();
                let mut result = PointerDispatchResult::default();
                result.include_routed(pointer_up);
                if button_event.button == Some(PointerButton::Primary) {
                    // Hand the up to the click recognizer — produces a
                    // `ClickResult::Click(target)` when the press is
                    // recognized; dispatch handlers along the responder
                    // path for that target.
                    let click_result = if pointer_up.handled || pointer_up.propagation_stopped {
                        self.events.cancel_click();
                        ClickResult::Suppressed(None)
                    } else {
                        match hit {
                            Some(target) => self.events.click_up(&target, pos, time_ms),
                            None => {
                                // Up off any element — cancel any open press
                                // so it doesn't dangle and pretend to click
                                // later.
                                self.events.cancel_click();
                                ClickResult::Suppressed(None)
                            }
                        }
                    };
                    result.press_changed = self.set_press_target(None);
                    result.capture_changed = self.set_pointer_capture(None);
                    self.events.clear_press();
                    if let ClickResult::Click(target) = click_result {
                        result.include_routed(self.dispatch_click_with_modifiers(
                            target,
                            pos,
                            button_event.state.modifiers,
                        ));
                    }
                }
                result
            }
            PointerEvent::Scroll(scroll_event) => {
                let data = pointer_route_data_for_scroll(event, scroll_event);
                let pos = scroll_event.state.logical_point();
                let mut result = PointerDispatchResult::default();
                self.update_hover_at_with_pointer(pos, data, &mut result);
                let routed = self
                    .hit_test(pos)
                    .map(|target| {
                        self.dispatch_scroll_with_modifiers(
                            target,
                            pos,
                            scroll_event.delta,
                            scroll_event.state.modifiers,
                        )
                    })
                    .unwrap_or_default();
                result.include_routed(routed);
                result
            }
            PointerEvent::Leave(pointer) => {
                let data = pointer_route_data_for_info(event, *pointer);
                let mut result = PointerDispatchResult::default();
                self.clear_hover_with_pointer(data, &mut result);
                result
            }
            PointerEvent::Cancel(pointer) => {
                let data = pointer_route_data_for_info(event, *pointer);
                let mut result = PointerDispatchResult::default();
                let source = self.events.pointer_capture().or_else(|| self.hovered());
                if let Some(source) = source.filter(|source| self.get(*source).is_some()) {
                    result.include_routed(self.dispatch_routed_pointer_event(
                        RoutedEventKind::PointerCancel,
                        source,
                        data,
                    ));
                }
                self.events.cancel_click();
                result.press_changed = self.set_press_target(None);
                result.capture_changed = self.set_pointer_capture(None);
                self.events.clear_press();
                self.clear_hover_with_pointer(data, &mut result);
                result
            }
            PointerEvent::Enter(_) | PointerEvent::Gesture(_) => PointerDispatchResult::default(),
        };

        result.focus_changed |= focused_before != self.focus.focused();
        result.capture_changed |= capture_before != self.events.pointer_capture();
        result
    }
}

struct UiWidgetLookup;

impl WidgetLookup<ElementId> for UiWidgetLookup {
    type WidgetId = ElementId;
    fn widget_of(&self, node: &ElementId) -> Option<ElementId> {
        Some(*node)
    }
}

struct UiParentLookup<'a> {
    ui: &'a Ui,
}

impl<'a> ParentLookup<ElementId> for UiParentLookup<'a> {
    fn parent_of(&self, node: &ElementId) -> Option<ElementId> {
        self.ui.get(*node).and_then(Element::parent)
    }
}
