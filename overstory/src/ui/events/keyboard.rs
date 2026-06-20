// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use ui_events::keyboard::KeyboardEvent;
use understory_focus::Navigation;

use crate::event_runtime::{KeyDispatchResult, RoutedEventKind};

use super::super::Ui;

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
        if self.dismiss_popup_for_escape(event) {
            return KeyDispatchResult {
                handled: true,
                propagation_stopped: false,
                focus_changed: previous_focus != self.focus.focused(),
                focused: self.focus.focused(),
            };
        }
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
            ui.dispatch_key_handler_lane(route, event);
            if !route.is_handled() && !route.is_propagation_stopped() {
                ui.dispatch_widget_key(route, event);
            }
        });

        KeyDispatchResult {
            handled: routed.handled,
            propagation_stopped: routed.propagation_stopped,
            focus_changed: previous_focus != self.focus.focused(),
            focused: self.focus.focused(),
        }
    }
}
