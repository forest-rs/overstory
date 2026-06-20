// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use kurbo::{Point, Vec2};
use ui_events::ScrollDelta;
use ui_events::keyboard::KeyboardEvent;
use ui_events::keyboard::Modifiers;
use understory_responder::dispatcher;
use understory_responder::types::Outcome;

use crate::activation::{ActivationDeniedEvent, ActivationDeniedReason, ActivationSource};
use crate::behavior::WidgetEventContext;
use crate::element::{Element, ElementId};
use crate::event_runtime::{
    ClickEvent, ClickHandlers, DragEvent, DragHandlers, FocusChangeReason, FocusEvent,
    FocusHandlers, HandlerFamily, KeyEvent, KeyHandlers, PointerEventKind, PointerHandlers,
    RoutedDispatchResult, RoutedEventContext, RoutedEventKind, RoutedEventOrigin, RoutedEventState,
    ScrollEvent, ScrollHandlers,
};
use crate::widget::Widget;

use super::super::Ui;
use super::super::responder::{ResponderHit, UiDispatchStep};
use super::PointerRouteData;

impl Ui {
    /// Runs `dispatch_current` across the responder route for `source`.
    ///
    /// Route construction belongs to `understory_responder` through the
    /// internal `ui::responder` adapter. This helper owns only Overstory's
    /// execution semantics: temporarily remove a handler before invoking it,
    /// share routed event state across invocations, skip ordinary handlers once
    /// an event is handled, and stop dispatch only when propagation is
    /// explicitly stopped.
    pub(in crate::ui::events) fn dispatch_routed(
        &mut self,
        kind: RoutedEventKind,
        source: ElementId,
        dispatch_current: impl FnMut(&mut Self, RoutedEventContext<'_>),
    ) -> RoutedDispatchResult {
        let dispatch = self.responder().dispatch_for(source);
        self.dispatch_responder_sequence(kind, source, dispatch, dispatch_current)
    }

    pub(in crate::ui::events) fn dispatch_responder_sequence(
        &mut self,
        kind: RoutedEventKind,
        source: ElementId,
        dispatch: Vec<UiDispatchStep>,
        mut dispatch_current: impl FnMut(&mut Self, RoutedEventContext<'_>),
    ) -> RoutedDispatchResult {
        let state = RoutedEventState::new();
        let origin = self.routed_event_origin(source);
        let _ = dispatcher::run(&dispatch, self, |step, ui| {
            let route = RoutedEventContext::new(kind, origin, step, &state);
            dispatch_current(ui, route);
            if state.should_stop_route() {
                Outcome::Stop
            } else {
                Outcome::Continue
            }
        });
        RoutedDispatchResult::from_state(&state)
    }

    fn routed_event_origin(&self, source: ElementId) -> RoutedEventOrigin {
        if let Some(owner) = self.get(source).and_then(Element::templated_parent) {
            return RoutedEventOrigin::TemplateChild {
                owner,
                child: source,
            };
        }

        if let Some(owner) = self.template_owner_for_presented_descendant(source) {
            return RoutedEventOrigin::TemplateChild {
                owner,
                child: source,
            };
        }

        RoutedEventOrigin::Element(source)
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

    fn dispatch_handler_registration<E, F>(
        &mut self,
        family: F,
        route: RoutedEventContext<'_>,
        event: E,
    ) where
        F: HandlerFamily,
        F::Handler: FnMut(&mut Self, E),
    {
        let node = route.current();
        let Some(mut registration) = self.events.take_handler(family, node, route.is_handled())
        else {
            return;
        };
        if !registration.options.scope.matches(route) {
            self.events
                .insert_handler_if_absent(family, node, registration);
            return;
        }

        self.events.begin_handler(family, node);
        (registration.handler)(self, event);
        self.events.end_handler(family, node);

        let handler_removed = self.events.handler_removed(family, node);
        if self.get(node).is_some() && !handler_removed && !self.events.has_handler(family, node) {
            self.events
                .insert_handler_if_absent(family, node, registration);
        }
    }

    pub(in crate::ui::events) fn dispatch_key_handler_lane<'a>(
        &mut self,
        route: RoutedEventContext<'a>,
        event: &'a KeyboardEvent,
    ) {
        self.dispatch_handler_registration(KeyHandlers, route, KeyEvent { route, key: event });
    }

    pub(in crate::ui::events) fn dispatch_widget_key<'a>(
        &mut self,
        route: RoutedEventContext<'a>,
        event: &'a KeyboardEvent,
    ) {
        let node = route.current();
        self.with_widget_mut(node, |widget, ui| {
            let mut ctx = WidgetEventContext::new(node, ui);
            widget.handle_key(&mut ctx, KeyEvent { route, key: event });
        });
    }

    pub(in crate::ui::events) fn dispatch_click_handler_lane(
        &mut self,
        route: RoutedEventContext<'_>,
        position: Point,
        modifiers: Modifiers,
    ) {
        let event = ClickEvent {
            route,
            position,
            modifiers,
        };
        self.dispatch_handler_registration(ClickHandlers, route, event);
    }

    pub(in crate::ui::events) fn dispatch_widget_click(
        &mut self,
        route: RoutedEventContext<'_>,
        position: Point,
        modifiers: Modifiers,
    ) {
        let node = route.current();
        self.with_widget_mut(node, |widget, ui| {
            let mut ctx = WidgetEventContext::new(node, ui);
            widget.handle_click(
                &mut ctx,
                ClickEvent {
                    route,
                    position,
                    modifiers,
                },
            );
        });
    }

    pub(in crate::ui::events) fn dispatch_drag_handler_lane(
        &mut self,
        route: RoutedEventContext<'_>,
        position: Point,
        press_position: Point,
        delta: Vec2,
    ) {
        let event = DragEvent {
            route,
            position,
            press_position,
            delta,
        };
        self.dispatch_handler_registration(DragHandlers, route, event);
    }

    pub(in crate::ui::events) fn dispatch_widget_drag(
        &mut self,
        route: RoutedEventContext<'_>,
        position: Point,
        press_position: Point,
        delta: Vec2,
    ) {
        let node = route.current();
        self.with_widget_mut(node, |widget, ui| {
            let mut ctx = WidgetEventContext::new(node, ui);
            widget.handle_drag(
                &mut ctx,
                DragEvent {
                    route,
                    position,
                    press_position,
                    delta,
                },
            );
        });
    }

    pub(in crate::ui::events) fn dispatch_scroll_handler_lane(
        &mut self,
        route: RoutedEventContext<'_>,
        position: Point,
        delta: ScrollDelta,
        modifiers: Modifiers,
    ) {
        let event = ScrollEvent {
            route,
            position,
            delta,
            modifiers,
        };
        self.dispatch_handler_registration(ScrollHandlers, route, event);
    }

    pub(in crate::ui::events) fn dispatch_widget_scroll(
        &mut self,
        route: RoutedEventContext<'_>,
        position: Point,
        delta: ScrollDelta,
        modifiers: Modifiers,
    ) {
        let node = route.current();
        self.with_widget_mut(node, |widget, ui| {
            let mut ctx = WidgetEventContext::new(node, ui);
            widget.handle_scroll(
                &mut ctx,
                ScrollEvent {
                    route,
                    position,
                    delta,
                    modifiers,
                },
            );
        });
    }

    pub(in crate::ui::events) fn dispatch_focus_handler_lane(
        &mut self,
        route: RoutedEventContext<'_>,
        previous: Option<ElementId>,
        focused: Option<ElementId>,
        focus_visible: bool,
        reason: FocusChangeReason,
    ) {
        let event = FocusEvent {
            route,
            previous,
            focused,
            focus_visible,
            reason,
        };
        self.dispatch_handler_registration(FocusHandlers, route, event);
    }

    pub(in crate::ui::events) fn dispatch_widget_focus(
        &mut self,
        route: RoutedEventContext<'_>,
        previous: Option<ElementId>,
        focused: Option<ElementId>,
        focus_visible: bool,
        reason: FocusChangeReason,
    ) {
        let node = route.current();
        self.with_widget_mut(node, |widget, ui| {
            let mut ctx = WidgetEventContext::new(node, ui);
            widget.handle_focus(
                &mut ctx,
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

    pub(in crate::ui::events) fn dispatch_pointer_handler_lane(
        &mut self,
        route: RoutedEventContext<'_>,
        data: PointerRouteData<'_>,
    ) {
        let kind = PointerEventKind::from_routed(route.kind)
            .expect("pointer dispatch route must use a pointer event kind");
        let event = data.event(route);
        self.dispatch_handler_registration(PointerHandlers::new(kind), route, event);
    }

    pub(in crate::ui::events) fn dispatch_widget_pointer(
        &mut self,
        route: RoutedEventContext<'_>,
        data: PointerRouteData<'_>,
    ) {
        let node = route.current();
        self.with_widget_mut(node, |widget, ui| {
            let mut ctx = WidgetEventContext::new(node, ui);
            widget.handle_pointer(&mut ctx, data.event(route));
        });
    }

    pub(in crate::ui::events) fn dispatch_widget_activation_denied(
        &mut self,
        visual: ElementId,
        disabled_target: ElementId,
        source: ActivationSource,
    ) {
        let event = ActivationDeniedEvent {
            target: visual,
            disabled_target,
            source,
            reason: ActivationDeniedReason::Disabled,
        };
        self.with_widget_mut(visual, |widget, ui| {
            let mut ctx = WidgetEventContext::new(visual, ui);
            widget.handle_activation_denied(&mut ctx, event);
        });
    }

    pub(in crate::ui) fn dispatch_routed_pointer_event(
        &mut self,
        kind: RoutedEventKind,
        source: ElementId,
        data: PointerRouteData<'_>,
    ) -> RoutedDispatchResult {
        self.dispatch_routed(kind, source, |ui, route| {
            ui.dispatch_pointer_handler_lane(route, data);
            if !route.is_handled() && !route.is_propagation_stopped() {
                ui.dispatch_widget_pointer(route, data);
            }
        })
    }

    pub(in crate::ui::events) fn dispatch_routed_pointer_hit(
        &mut self,
        kind: RoutedEventKind,
        hit: ResponderHit,
        data: PointerRouteData<'_>,
    ) -> RoutedDispatchResult {
        self.dispatch_responder_sequence(kind, hit.target, hit.dispatch, |ui, route| {
            ui.dispatch_pointer_handler_lane(route, data);
            if !route.is_handled() && !route.is_propagation_stopped() {
                ui.dispatch_widget_pointer(route, data);
            }
        })
    }

    pub(in crate::ui::events) fn dispatch_focus_event(
        &mut self,
        kind: RoutedEventKind,
        source: ElementId,
        previous: Option<ElementId>,
        focused: Option<ElementId>,
        focus_visible: bool,
        reason: FocusChangeReason,
    ) -> RoutedDispatchResult {
        self.dispatch_routed(kind, source, |ui, route| {
            ui.dispatch_focus_handler_lane(route, previous, focused, focus_visible, reason);
            if !route.is_handled() && !route.is_propagation_stopped() {
                ui.dispatch_widget_focus(route, previous, focused, focus_visible, reason);
            }
        })
    }
}
