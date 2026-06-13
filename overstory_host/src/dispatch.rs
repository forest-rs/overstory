// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Event dispatch result types shared by platform hosts.

use frameclock::FrameDemand;
use overstory::{KeyDispatchResult, PointerDispatchResult};
use ui_events::pointer::PointerEvent;

/// Result of dispatching a platform-normalized event into a [`crate::UiHost`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HostDispatchResult {
    /// Whether any UI handler consumed or reacted to the event.
    pub handled: bool,
    /// Whether focus changed while dispatching the event.
    pub focus_changed: bool,
    /// Whether the platform host should request a redraw.
    pub needs_redraw: bool,
    /// Demand to request when [`Self::needs_redraw`] is true.
    ///
    /// Empty demand means the dispatch did not change visible UI state. Pointer
    /// movement, scroll, and gestures use continuous-input demand; discrete
    /// input such as keys, clicks, cancellation, and focus changes use input
    /// demand.
    pub frame_demand: FrameDemand,
}

impl From<KeyDispatchResult> for HostDispatchResult {
    fn from(result: KeyDispatchResult) -> Self {
        let needs_redraw = result.handled || result.focus_changed;
        Self {
            handled: result.handled,
            focus_changed: result.focus_changed,
            needs_redraw,
            frame_demand: if needs_redraw {
                FrameDemand::INPUT
            } else {
                FrameDemand::NONE
            },
        }
    }
}

impl HostDispatchResult {
    /// Creates a host dispatch result from a normalized pointer event and the
    /// corresponding core Overstory dispatch result.
    #[must_use]
    pub fn from_pointer_event(event: &PointerEvent, result: PointerDispatchResult) -> Self {
        let needs_redraw = result.changed();
        Self {
            handled: result.handled,
            focus_changed: result.focus_changed,
            needs_redraw,
            frame_demand: if needs_redraw {
                pointer_frame_demand(event)
            } else {
                FrameDemand::NONE
            },
        }
    }
}

fn pointer_frame_demand(event: &PointerEvent) -> FrameDemand {
    match event {
        PointerEvent::Move(_) | PointerEvent::Scroll(_) | PointerEvent::Gesture(_) => {
            FrameDemand::CONTINUOUS_INPUT
        }
        PointerEvent::Down(_)
        | PointerEvent::Up(_)
        | PointerEvent::Cancel(_)
        | PointerEvent::Enter(_)
        | PointerEvent::Leave(_) => FrameDemand::INPUT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ui_events::ScrollDelta;
    use ui_events::pointer::{
        PointerButton, PointerButtonEvent, PointerId, PointerInfo, PointerScrollEvent,
        PointerState, PointerType, PointerUpdate,
    };

    fn changed_pointer_result() -> PointerDispatchResult {
        PointerDispatchResult {
            hover_changed: true,
            ..PointerDispatchResult::default()
        }
    }

    fn pointer_info() -> PointerInfo {
        PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        }
    }

    fn pointer_state(button: Option<PointerButton>) -> PointerState {
        let mut state = PointerState::default();
        if let Some(button) = button {
            state.buttons.insert(button);
        }
        state
    }

    #[test]
    fn key_dispatch_uses_input_demand_when_it_needs_redraw() {
        let result = HostDispatchResult::from(KeyDispatchResult {
            handled: true,
            ..KeyDispatchResult::default()
        });

        assert!(result.needs_redraw);
        assert_eq!(result.frame_demand, FrameDemand::INPUT);
    }

    #[test]
    fn pointer_down_dispatch_uses_input_demand() {
        let event = PointerEvent::Down(PointerButtonEvent {
            button: Some(PointerButton::Primary),
            pointer: pointer_info(),
            state: pointer_state(Some(PointerButton::Primary)),
        });
        let result = HostDispatchResult::from_pointer_event(&event, changed_pointer_result());

        assert!(result.needs_redraw);
        assert_eq!(result.frame_demand, FrameDemand::INPUT);
    }

    #[test]
    fn pointer_drag_and_scroll_dispatch_use_continuous_input_demand() {
        let drag = PointerEvent::Move(PointerUpdate {
            pointer: pointer_info(),
            current: pointer_state(Some(PointerButton::Primary)),
            coalesced: Vec::new(),
            predicted: Vec::new(),
        });
        let scroll = PointerEvent::Scroll(PointerScrollEvent {
            pointer: pointer_info(),
            delta: ScrollDelta::LineDelta(0.0, 1.0),
            state: pointer_state(None),
        });

        assert_eq!(
            HostDispatchResult::from_pointer_event(&drag, changed_pointer_result()).frame_demand,
            FrameDemand::CONTINUOUS_INPUT
        );
        assert_eq!(
            HostDispatchResult::from_pointer_event(&scroll, changed_pointer_result()).frame_demand,
            FrameDemand::CONTINUOUS_INPUT
        );
    }

    #[test]
    fn unchanged_pointer_dispatch_has_no_frame_demand() {
        let event = PointerEvent::Move(PointerUpdate {
            pointer: pointer_info(),
            current: pointer_state(None),
            coalesced: Vec::new(),
            predicted: Vec::new(),
        });
        let result =
            HostDispatchResult::from_pointer_event(&event, PointerDispatchResult::default());

        assert!(!result.needs_redraw);
        assert_eq!(result.frame_demand, FrameDemand::NONE);
    }
}
