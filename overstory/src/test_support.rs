// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared test fixtures for Overstory's unit tests.

use alloc::vec::Vec;
use kurbo::Point;
use ui_events::ScrollDelta;
use ui_events::pointer::{
    PointerButton, PointerButtonEvent, PointerButtons, PointerEvent, PointerId, PointerInfo,
    PointerScrollEvent, PointerState, PointerType, PointerUpdate,
};

/// Builds a primary-pointer button-down event at `point`.
pub(crate) fn pointer_down(point: Point, time: u64) -> PointerEvent {
    PointerEvent::Down(PointerButtonEvent {
        pointer: pointer_info(),
        button: Some(PointerButton::Primary),
        state: pointer_state(point, time, true),
    })
}

/// Builds a primary-pointer button-up event at `point`.
pub(crate) fn pointer_up(point: Point, time: u64) -> PointerEvent {
    PointerEvent::Up(PointerButtonEvent {
        pointer: pointer_info(),
        button: Some(PointerButton::Primary),
        state: pointer_state(point, time, false),
    })
}

/// Builds a touch-contact down event at `point`.
pub(crate) fn touch_down(point: Point, time: u64) -> PointerEvent {
    PointerEvent::Down(PointerButtonEvent {
        pointer: touch_pointer_info(),
        button: None,
        state: touch_pointer_state(point, time, true),
    })
}

/// Builds a touch-contact up event at `point`.
pub(crate) fn touch_up(point: Point, time: u64) -> PointerEvent {
    PointerEvent::Up(PointerButtonEvent {
        pointer: touch_pointer_info(),
        button: None,
        state: touch_pointer_state(point, time, false),
    })
}

/// Builds a pointer-move event at `point`.
pub(crate) fn pointer_move(point: Point, time: u64, primary_down: bool) -> PointerEvent {
    PointerEvent::Move(PointerUpdate {
        pointer: pointer_info(),
        current: pointer_state(point, time, primary_down),
        coalesced: Vec::new(),
        predicted: Vec::new(),
    })
}

/// Builds a pointer-scroll event at `point`.
pub(crate) fn pointer_scroll(point: Point, time: u64, delta: ScrollDelta) -> PointerEvent {
    PointerEvent::Scroll(PointerScrollEvent {
        pointer: pointer_info(),
        delta,
        state: pointer_state(point, time, false),
    })
}

/// Builds a pointer-leave event for the primary pointer.
pub(crate) fn pointer_leave() -> PointerEvent {
    PointerEvent::Leave(pointer_info())
}

/// Builds primary mouse pointer identity.
pub(crate) fn pointer_info() -> PointerInfo {
    PointerInfo {
        pointer_id: Some(PointerId::PRIMARY),
        persistent_device_id: None,
        pointer_type: PointerType::Mouse,
    }
}

/// Builds primary touch pointer identity.
pub(crate) fn touch_pointer_info() -> PointerInfo {
    PointerInfo {
        pointer_id: Some(PointerId::PRIMARY),
        persistent_device_id: None,
        pointer_type: PointerType::Touch,
    }
}

/// Builds pointer state at `point`.
pub(crate) fn pointer_state(point: Point, time: u64, primary_down: bool) -> PointerState {
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

/// Builds touch pointer state at `point`.
pub(crate) fn touch_pointer_state(point: Point, time: u64, active: bool) -> PointerState {
    let mut state = pointer_state(point, time, false);
    state.pressure = if active { 0.5 } else { 0.0 };
    state
}
