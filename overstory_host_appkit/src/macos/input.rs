// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Bridge from `ui-events-appkit` responder callbacks into `UiHost`.

#![expect(
    unsafe_code,
    reason = "AppKit responder chain wiring uses Objective-C FFI"
)]

use alloc::boxed::Box;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSEvent, NSWindow};
use ui_events::keyboard::KeyboardEvent;
use ui_events::pointer::PointerEvent;
use ui_events_appkit::{AppKitInputResponder, AppKitInputResponderHost};

use super::state::with_window_state_cell;
use super::time::AppleTimeMap;
use super::window::ContentView;

pub(crate) struct InputHost;

impl AppKitInputResponderHost for InputHost {
    fn handle_keyboard_event(&self, event: KeyboardEvent) {
        dispatch_keyboard_event(&event);
    }

    fn handle_pointer_event(&self, event: PointerEvent) {
        normalize_and_dispatch_appkit_pointer_event(event);
    }

    fn pointer_position(&self, event: &NSEvent) -> (f64, f64) {
        with_window_state_cell(|cell| {
            cell.borrow()
                .as_ref()
                .map_or((0.0, 0.0), |state| state.pointer_position(event))
        })
    }

    fn pointer_scale_factor(&self) -> f64 {
        current_scale_factor()
    }
}

pub(crate) fn install_input_responder(
    mtm: MainThreadMarker,
    window: &NSWindow,
    content_view: &ContentView,
) -> Retained<AppKitInputResponder> {
    let input_responder = AppKitInputResponder::new(mtm, Box::new(InputHost));
    unsafe {
        let following_responder = content_view.nextResponder();
        input_responder.set_following_responder(following_responder.as_deref());
        content_view.setNextResponder(Some(input_responder.as_ref()));
    }
    window.setAcceptsMouseMovedEvents(true);
    let _ = window.makeFirstResponder(Some(input_responder.as_ref()));
    input_responder
}

fn dispatch_keyboard_event(event: &KeyboardEvent) {
    with_window_state_cell(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return;
        };
        state.dispatch_keyboard_event(event);
    });
}

fn dispatch_pointer_event(event: &PointerEvent) {
    with_window_state_cell(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return;
        };
        state.dispatch_pointer_event(event);
    });
}

fn normalize_and_dispatch_appkit_pointer_event(event: PointerEvent) {
    let event = normalize_appkit_pointer_event_time(event);
    dispatch_pointer_event(&event);
}

fn current_scale_factor() -> f64 {
    with_window_state_cell(|cell| {
        cell.borrow()
            .as_ref()
            .map_or(1.0, |state| state.scale_factor())
    })
}

fn normalize_appkit_pointer_event_time(mut event: PointerEvent) -> PointerEvent {
    let Some(time_map) = current_time_map() else {
        return event;
    };
    match &mut event {
        PointerEvent::Down(button) | PointerEvent::Up(button) => {
            button.state.time = time_map.map_appkit_event_time_ns(button.state.time);
        }
        PointerEvent::Move(update) => {
            update.current.time = time_map.map_appkit_event_time_ns(update.current.time);
            for state in &mut update.coalesced {
                state.time = time_map.map_appkit_event_time_ns(state.time);
            }
            for state in &mut update.predicted {
                state.time = time_map.map_appkit_event_time_ns(state.time);
            }
        }
        PointerEvent::Scroll(scroll) => {
            scroll.state.time = time_map.map_appkit_event_time_ns(scroll.state.time);
        }
        PointerEvent::Gesture(gesture) => {
            gesture.state.time = time_map.map_appkit_event_time_ns(gesture.state.time);
        }
        PointerEvent::Cancel(_) | PointerEvent::Enter(_) | PointerEvent::Leave(_) => {}
    }
    event
}

fn current_time_map() -> Option<AppleTimeMap> {
    with_window_state_cell(|cell| cell.borrow().as_ref().map(|state| state.time_map()))
}
