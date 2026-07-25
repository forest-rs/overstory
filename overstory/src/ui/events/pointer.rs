// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::Point;
use ui_events::keyboard::Modifiers;
use ui_events::pointer::{
    PointerButton, PointerButtonEvent, PointerEvent, PointerInfo, PointerScrollEvent, PointerState,
    PointerType, PointerUpdate,
};
use understory_event_state::click::ClickResult;
use understory_timing::TimerInstant;

use crate::activation::ActivationSource;
use crate::event_runtime::{
    PointerDispatchResult, PointerRoutedEvent, RoutedDispatchResult, RoutedEventContext,
    RoutedEventKind,
};

use super::super::Ui;

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
        PointerEvent::Gesture(g) => Some(g.state.time),
        PointerEvent::Enter(_) | PointerEvent::Leave(_) | PointerEvent::Cancel(_) => None,
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

fn is_primary_activation_contact(event: &PointerButtonEvent) -> bool {
    event.button == Some(PointerButton::Primary)
        || (event.button.is_none() && matches!(event.pointer.pointer_type, PointerType::Touch))
}

fn is_context_menu_contact(event: &PointerButtonEvent) -> bool {
    event.button == Some(PointerButton::Secondary)
}

impl Ui {
    fn dispatch_pointer_hit(
        &mut self,
        kind: RoutedEventKind,
        pos: Point,
        data: PointerRouteData<'_>,
    ) -> RoutedDispatchResult {
        let Some(hit) = self.responder().hit_dispatch(pos) else {
            return RoutedDispatchResult::default();
        };
        if !self.is_enabled(self.press_visual_target(hit.target)) {
            return RoutedDispatchResult::default();
        }
        self.dispatch_routed_pointer_hit(kind, hit, data)
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
            self.set_now(t);
        }
        // Pointer dispatch may follow tree mutations that do not require a
        // full layout pass before they are query-safe (for example hiding a
        // passive overlay tooltip). Commit pending box-tree structure changes
        // before any hit test.
        self.geometry.commit();
        let focused_before = self.focus.focused();
        let capture_before = self.events.pointer_capture();

        let mut result = match event {
            PointerEvent::Move(update) => {
                let data = pointer_route_data_for_move(event, update);
                let pos = update.current.logical_point();
                self.update_overlay_pointer(pos);
                let mut result = PointerDispatchResult::default();
                self.update_hover_at_with_pointer(pos, data, &mut result);
                // Pointer-enter/leave handlers may retain or remove structure
                // (for example, opening a submenu). The following hit test is
                // a new routing phase and must observe a committed tree.
                self.geometry.commit();
                let pointer_move = if let Some(source) = self.events.pointer_capture() {
                    if self.is_enabled(self.press_visual_target(source)) {
                        self.dispatch_routed_pointer_event(
                            RoutedEventKind::PointerMove,
                            source,
                            data,
                        )
                    } else {
                        RoutedDispatchResult::default()
                    }
                } else {
                    self.dispatch_pointer_hit(RoutedEventKind::PointerMove, pos, data)
                };
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
                self.update_overlay_pointer(pos);
                let time_ms = button_event.state.time / 1_000_000;
                let mut result = PointerDispatchResult::default();
                if is_primary_activation_contact(button_event)
                    || is_context_menu_contact(button_event)
                {
                    result.overlay_changed |= self.dismiss_popups_for_pointer_down(pos);
                    if result.overlay_changed {
                        self.geometry.commit();
                    }
                }
                // A Down without a preceding Move can arrive on some
                // platforms; refresh hover first so any newly-entered
                // path is set up before we press.
                self.update_hover_at_with_pointer(pos, data, &mut result);
                self.geometry.commit();
                if let Some(hit) = self.responder().hit_dispatch(pos) {
                    let target = hit.target;
                    let visual = self.press_visual_target(target);
                    if !self.is_enabled(visual) {
                        self.dispatch_widget_activation_denied(
                            visual,
                            target,
                            ActivationSource::Pointer,
                        );
                        self.events.cancel_click();
                        result.press_changed = self.set_press_target(None);
                        result.capture_changed = self.set_pointer_capture(None);
                        self.events.clear_press();
                    } else {
                        if self.is_focusable(visual) {
                            result.focus_changed = self.set_pointer_focus(Some(visual));
                        }
                        let pointer_down = self.dispatch_routed_pointer_hit(
                            RoutedEventKind::PointerDown,
                            hit,
                            data,
                        );
                        result.include_routed(pointer_down);
                        if is_context_menu_contact(button_event)
                            && !pointer_down.handled
                            && !pointer_down.propagation_stopped
                        {
                            let opened = self.request_pointer_context_menu(target, pos);
                            result.handled |= opened;
                            result.overlay_changed |= opened;
                            if opened {
                                self.geometry.commit();
                            }
                        }
                        if is_primary_activation_contact(button_event)
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
                self.update_overlay_pointer(pos);
                let time_ms = button_event.state.time / 1_000_000;
                let hit = self.responder().hit_dispatch(pos);
                let hit_target = hit.as_ref().map(|hit| hit.target);
                let pointer_up = if let Some(source) = self.events.pointer_capture() {
                    if self.is_enabled(self.press_visual_target(source)) {
                        self.dispatch_routed_pointer_event(RoutedEventKind::PointerUp, source, data)
                    } else {
                        RoutedDispatchResult::default()
                    }
                } else if let Some(hit) = hit {
                    if self.is_enabled(self.press_visual_target(hit.target)) {
                        self.dispatch_routed_pointer_hit(RoutedEventKind::PointerUp, hit, data)
                    } else {
                        RoutedDispatchResult::default()
                    }
                } else {
                    RoutedDispatchResult::default()
                };
                let mut result = PointerDispatchResult::default();
                result.include_routed(pointer_up);
                if is_primary_activation_contact(button_event) {
                    // Hand the up to the click recognizer — produces a
                    // `ClickResult::Click(target)` when the press is
                    // recognized; dispatch handlers along the responder
                    // path for that target.
                    let click_result = if pointer_up.handled || pointer_up.propagation_stopped {
                        self.events.cancel_click();
                        ClickResult::Suppressed(None)
                    } else {
                        match hit_target {
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
                if matches!(button_event.pointer.pointer_type, PointerType::Touch) {
                    self.clear_hover_with_pointer(data, &mut result);
                }
                result
            }
            PointerEvent::Scroll(scroll_event) => {
                let data = pointer_route_data_for_scroll(event, scroll_event);
                let pos = scroll_event.state.logical_point();
                let mut result = PointerDispatchResult::default();
                self.update_hover_at_with_pointer(pos, data, &mut result);
                self.geometry.commit();
                let routed = self
                    .responder()
                    .hit_dispatch(pos)
                    .map(|hit| {
                        self.dispatch_scroll_hit_with_modifiers(
                            hit,
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
        result.hover_changed |= self.refresh_tooltip_from_pointer();
        result
    }
}
