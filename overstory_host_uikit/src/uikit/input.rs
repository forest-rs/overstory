// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Bridge from `ui-events-uikit` responder callbacks into `UiHost`.

use alloc::boxed::Box;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use ui_events::keyboard::KeyboardEvent;
use ui_events::pointer::PointerEvent;
use ui_events_uikit::{UIKitInputResponder, UIKitInputResponderHost};

use super::state::with_view_state_cell;

pub(crate) struct InputHost;

impl UIKitInputResponderHost for InputHost {
    fn handle_keyboard_event(&self, event: KeyboardEvent) {
        dispatch_keyboard_event(&event);
    }

    fn handle_pointer_event(&self, event: PointerEvent) {
        normalize_and_dispatch_uikit_pointer_event(event);
    }

    fn pointer_scale_factor(&self) -> f64 {
        current_scale_factor()
    }
}

pub(crate) fn create_input_responder(mtm: MainThreadMarker) -> Retained<UIKitInputResponder> {
    UIKitInputResponder::new(mtm, Box::new(InputHost))
}

pub(crate) fn dispatch_pointer_event(event: &PointerEvent) {
    with_view_state_cell(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return;
        };
        state.dispatch_pointer_event(event);
    });
}

pub(crate) fn current_host_time_ns() -> u64 {
    with_view_state_cell(|cell| {
        cell.borrow()
            .as_ref()
            .map_or(0, |state| state.current_time_ns())
    })
}

fn dispatch_keyboard_event(event: &KeyboardEvent) {
    with_view_state_cell(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return;
        };
        state.dispatch_keyboard_event(event);
    });
}

fn normalize_and_dispatch_uikit_pointer_event(event: PointerEvent) {
    let event = normalize_uikit_pointer_event_time(event);
    dispatch_pointer_event(&event);
}

fn current_scale_factor() -> f64 {
    with_view_state_cell(|cell| {
        cell.borrow()
            .as_ref()
            .map_or(1.0, |state| state.scale_factor())
    })
}

fn normalize_uikit_pointer_event_time(mut event: PointerEvent) -> PointerEvent {
    let Some(time_map) = current_time_map() else {
        return event;
    };
    match &mut event {
        PointerEvent::Down(button) | PointerEvent::Up(button) => {
            button.state.time = time_map.map_apple_event_time_ns(button.state.time);
        }
        PointerEvent::Move(update) => {
            update.current.time = time_map.map_apple_event_time_ns(update.current.time);
            for state in &mut update.coalesced {
                state.time = time_map.map_apple_event_time_ns(state.time);
            }
            for state in &mut update.predicted {
                state.time = time_map.map_apple_event_time_ns(state.time);
            }
        }
        PointerEvent::Scroll(scroll) => {
            scroll.state.time = time_map.map_apple_event_time_ns(scroll.state.time);
        }
        PointerEvent::Gesture(gesture) => {
            gesture.state.time = time_map.map_apple_event_time_ns(gesture.state.time);
        }
        PointerEvent::Cancel(_) | PointerEvent::Enter(_) | PointerEvent::Leave(_) => {}
    }
    event
}

fn current_time_map() -> Option<super::time::AppleTimeMap> {
    with_view_state_cell(|cell| cell.borrow().as_ref().map(|state| state.time_map()))
}
