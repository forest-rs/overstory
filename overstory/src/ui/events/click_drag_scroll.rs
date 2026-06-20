// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::Point;
use ui_events::ScrollDelta;
use ui_events::keyboard::Modifiers;

use crate::element::ElementId;
use crate::event_runtime::{RoutedDispatchResult, RoutedEventKind};

use super::super::Ui;
use super::super::responder::ResponderHit;

impl Ui {
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
            ui.dispatch_drag_handler_lane(route, position, press_position, delta);
            if !route.is_handled() && !route.is_propagation_stopped() {
                ui.dispatch_widget_drag(route, position, press_position, delta);
            }
        })
    }

    /// Programmatically synthesize a routed click on `target` at `position`.
    ///
    /// This is useful for tests and bridges that need raw click routing without
    /// constructing a full pointer down/up sequence. Semantic control
    /// activation should use control-specific callbacks instead.
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

    pub(in crate::ui::events) fn dispatch_click_with_modifiers(
        &mut self,
        target: ElementId,
        position: Point,
        modifiers: Modifiers,
    ) -> RoutedDispatchResult {
        if !self.is_enabled(self.press_visual_target(target)) {
            return RoutedDispatchResult::default();
        }

        self.dispatch_routed(RoutedEventKind::Click, target, |ui, route| {
            ui.dispatch_click_handler_lane(route, position, modifiers);
            if !route.is_handled() && !route.is_propagation_stopped() {
                ui.dispatch_widget_click(route, position, modifiers);
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
            ui.dispatch_scroll_handler_lane(route, position, delta, modifiers);
            if !route.is_handled() && !route.is_propagation_stopped() {
                ui.dispatch_widget_scroll(route, position, delta, modifiers);
            }
        })
    }

    pub(in crate::ui::events) fn dispatch_scroll_hit_with_modifiers(
        &mut self,
        hit: ResponderHit,
        position: Point,
        delta: ScrollDelta,
        modifiers: Modifiers,
    ) -> RoutedDispatchResult {
        if !self.is_enabled(self.press_visual_target(hit.target)) {
            return RoutedDispatchResult::default();
        }

        self.dispatch_responder_sequence(
            RoutedEventKind::Scroll,
            hit.target,
            hit.dispatch,
            |ui, route| {
                ui.dispatch_scroll_handler_lane(route, position, delta, modifiers);
                if !route.is_handled() && !route.is_propagation_stopped() {
                    ui.dispatch_widget_scroll(route, position, delta, modifiers);
                }
            },
        )
    }
}
