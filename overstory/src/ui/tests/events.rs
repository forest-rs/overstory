// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use alloc::borrow::ToOwned;

#[test]
fn set_focus_toggles_pseudoclass() {
    let mut ui = Ui::new();
    let root = ui.root();
    let a = ui.insert_child(root);
    let b = ui.insert_child(root);

    assert_eq!(ui.focused(), None);
    assert!(ui.set_focus(Some(a)));
    assert_eq!(ui.focused(), Some(a));
    assert!(ui.get(a).unwrap().pseudos().contains(&PSEUDO_FOCUSED));
    assert!(ui.get(a).unwrap().pseudos().contains(&PSEUDO_FOCUS_VISIBLE));

    // Move focus to b — a should lose focus pseudos, b should gain them.
    assert!(ui.set_focus(Some(b)));
    assert!(!ui.get(a).unwrap().pseudos().contains(&PSEUDO_FOCUSED));
    assert!(!ui.get(a).unwrap().pseudos().contains(&PSEUDO_FOCUS_VISIBLE));
    assert!(ui.get(b).unwrap().pseudos().contains(&PSEUDO_FOCUSED));
    assert!(ui.get(b).unwrap().pseudos().contains(&PSEUDO_FOCUS_VISIBLE));

    // Clear focus.
    assert!(ui.set_focus(None));
    assert!(!ui.get(b).unwrap().pseudos().contains(&PSEUDO_FOCUSED));
    assert!(!ui.get(b).unwrap().pseudos().contains(&PSEUDO_FOCUS_VISIBLE));
    assert_eq!(ui.focused(), None);
}

#[test]
fn pointer_focus_does_not_set_focus_visible() {
    use crate::Button;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(200.0, 80.0));
    let button = ui.append(root, Button::new("Click"));
    ui.on_click(button, |_ui, event| event.mark_handled());
    ui.compute_layout(Size::new(200.0, 80.0));

    let center = ui.effective_bounds(button).center();
    assert!(
        ui.dispatch_pointer_event(&pointer_down(center, 1))
            .changed()
    );
    assert_eq!(ui.focused(), Some(button));
    assert!(ui.dispatch_pointer_event(&pointer_up(center, 2)).changed());

    let pseudos = ui.get(button).unwrap().pseudos();
    assert_eq!(ui.focused(), Some(button));
    assert!(pseudos.contains(&PSEUDO_FOCUSED));
    assert!(!pseudos.contains(&PSEUDO_FOCUS_VISIBLE));
}

#[test]
fn focus_navigate_walks_focusables() {
    // Lay out three focusable children. `Navigation::Next`
    // cycles through them in document order with WrapMode::Scope.
    let mut ui = Ui::new();
    let root = ui.root();
    let layout_prop = ui.properties().layout;
    ui.set_local(root, layout_prop, Layout::flex().logical_size(300.0, 50.0));
    let a = ui.insert_child(root);
    let b = ui.insert_child(root);
    let c = ui.insert_child(root);
    for id in [a, b, c] {
        ui.set_local(
            id,
            layout_prop,
            Layout::new().flex_grow(1.0).align_self_stretch(),
        );
        ui.set_focusable(id, true);
    }
    ui.compute_layout(Size::new(300.0, 50.0));

    // Starting unfocused, Next picks the initial element.
    let first = ui.focus_navigate(Navigation::Next).unwrap();
    assert_eq!(first, a);
    assert_eq!(ui.focus_navigate(Navigation::Next), Some(b));
    assert_eq!(ui.focus_navigate(Navigation::Next), Some(c));
    // WrapMode::Scope → back to the first.
    assert_eq!(ui.focus_navigate(Navigation::Next), Some(a));
    // Prev cycles backwards.
    assert_eq!(ui.focus_navigate(Navigation::Prev), Some(c));
}

#[test]
fn dispatch_key_runs_target_handler() {
    use alloc::rc::Rc;
    use core::cell::Cell;
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

    let mut ui = Ui::new();
    let root = ui.root();
    let mid = ui.insert_child(root);
    let child = ui.insert_child(mid);
    ui.set_focusable(child, true);
    ui.set_focus(Some(child));

    let fired = Rc::new(Cell::new(false));
    let inc = fired.clone();
    ui.on_key(child, move |_ui, event| {
        if matches!(event.phase, Phase::Target) {
            assert_eq!(event.kind, RoutedEventKind::Key);
            inc.set(true);
        }
    });

    let key = KeyboardEvent::key_down(Key::Named(NamedKey::Enter), Code::Enter);
    let result = ui.dispatch_key(&key);
    assert!(fired.get(), "target-phase key handler should fire");
    assert!(!result.handled, "handler did not mark the key handled");
}

#[test]
fn dispatch_key_noop_without_focus() {
    use alloc::rc::Rc;
    use core::cell::Cell;
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

    let mut ui = Ui::new();
    let root = ui.root();
    let fired = Rc::new(Cell::new(false));
    let inc = fired.clone();
    ui.on_key(root, move |_ui, _event| inc.set(true));

    // No focus set → dispatch should be a no-op even though root
    // has a handler.
    let key = KeyboardEvent::key_down(Key::Named(NamedKey::Enter), Code::Enter);
    let result = ui.dispatch_key(&key);
    assert!(!fired.get(), "key handler should not fire without focus");
    assert_eq!(result, KeyDispatchResult::default());
}

#[test]
fn dispatch_key_stops_route_when_handler_marks_handled() {
    use alloc::rc::Rc;
    use core::cell::Cell;
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

    let mut ui = Ui::new();
    let root = ui.root();
    let mid = ui.insert_child(root);
    let child = ui.insert_child(mid);
    ui.set_focusable(child, true);
    ui.set_focus(Some(child));

    let child_fired = Rc::new(Cell::new(false));
    let child_fired_for_handler = Rc::clone(&child_fired);
    ui.on_key(root, move |_ui, event| {
        if matches!(event.phase, Phase::Capture) {
            event.mark_handled();
        }
    });
    ui.on_key(child, move |_ui, _event| {
        child_fired_for_handler.set(true);
    });

    let key = KeyboardEvent::key_down(Key::Named(NamedKey::Enter), Code::Enter);
    let result = ui.dispatch_key(&key);

    assert!(result.handled);
    assert!(!result.propagation_stopped);
    assert!(
        !child_fired.get(),
        "handled capture-phase key should stop before target"
    );
}

#[test]
fn handled_events_too_key_handler_observes_handled_route() {
    use alloc::rc::Rc;
    use core::cell::Cell;
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

    let mut ui = Ui::new();
    let root = ui.root();
    let mid = ui.insert_child(root);
    let child = ui.insert_child(mid);
    ui.set_focusable(child, true);
    ui.set_focus(Some(child));

    ui.on_key(root, |_ui, event| {
        if matches!(event.phase, Phase::Capture) {
            event.mark_handled();
        }
    });

    let ordinary_fired = Rc::new(Cell::new(false));
    let ordinary_fired_for_handler = Rc::clone(&ordinary_fired);
    ui.on_key(child, move |_ui, _event| {
        ordinary_fired_for_handler.set(true);
    });

    let observer_fired = Rc::new(Cell::new(false));
    let observer_fired_for_handler = Rc::clone(&observer_fired);
    ui.on_key_with_options(
        mid,
        RoutedHandlerOptions::new().handled_events_too(true),
        move |_ui, event| {
            if matches!(event.phase, Phase::Bubble) {
                observer_fired_for_handler.set(event.is_handled());
            }
        },
    );

    let key = KeyboardEvent::key_down(Key::Named(NamedKey::Enter), Code::Enter);
    let result = ui.dispatch_key(&key);

    assert!(result.handled);
    assert!(!ordinary_fired.get());
    assert!(observer_fired.get());
}

#[test]
fn dispatch_key_stops_route_when_handler_stops_propagation() {
    use alloc::rc::Rc;
    use core::cell::Cell;
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

    let mut ui = Ui::new();
    let root = ui.root();
    let child = ui.insert_child(root);
    ui.set_focusable(child, true);
    ui.set_focus(Some(child));

    let child_fired = Rc::new(Cell::new(false));
    let child_fired_for_handler = Rc::clone(&child_fired);
    ui.on_key(root, move |_ui, event| {
        if matches!(event.phase, Phase::Capture) {
            event.stop_propagation();
        }
    });
    ui.on_key(child, move |_ui, _event| {
        child_fired_for_handler.set(true);
    });

    let key = KeyboardEvent::key_down(Key::Named(NamedKey::Enter), Code::Enter);
    let result = ui.dispatch_key(&key);

    assert!(!result.handled);
    assert!(result.propagation_stopped);
    assert!(
        !child_fired.get(),
        "stopped capture-phase key should stop before target"
    );
}

#[test]
fn focus_events_route_lost_and_gained_notifications() {
    use alloc::rc::Rc;
    use core::cell::RefCell;

    let mut ui = Ui::new();
    let root = ui.root();
    let a = ui.insert_child(root);
    let b = ui.insert_child(root);
    ui.set_focusable(a, true);
    ui.set_focusable(b, true);

    let seen = Rc::new(RefCell::new(Vec::new()));
    let seen_for_a = Rc::clone(&seen);
    ui.on_focus(a, move |_ui, event| {
        if matches!(event.phase, Phase::Target) {
            seen_for_a.borrow_mut().push((
                event.kind,
                event.previous,
                event.focused,
                event.focus_visible,
                event.reason,
            ));
        }
    });
    let seen_for_b = Rc::clone(&seen);
    ui.on_focus(b, move |_ui, event| {
        if matches!(event.phase, Phase::Target) {
            seen_for_b.borrow_mut().push((
                event.kind,
                event.previous,
                event.focused,
                event.focus_visible,
                event.reason,
            ));
        }
    });

    assert!(ui.set_focus(Some(a)));
    assert!(ui.set_focus(Some(b)));

    assert_eq!(
        seen.borrow().as_slice(),
        &[
            (
                RoutedEventKind::FocusGained,
                None,
                Some(a),
                true,
                FocusChangeReason::Programmatic,
            ),
            (
                RoutedEventKind::FocusLost,
                Some(a),
                Some(b),
                true,
                FocusChangeReason::Programmatic,
            ),
            (
                RoutedEventKind::FocusGained,
                Some(a),
                Some(b),
                true,
                FocusChangeReason::Programmatic,
            ),
        ]
    );
}

#[test]
fn dispatch_key_handles_tab_focus_navigation() {
    use ui_events::keyboard::{Code, Key, KeyboardEvent, Modifiers, NamedKey};

    let mut ui = Ui::new();
    let root = ui.root();
    let layout_prop = ui.properties().layout;
    ui.set_local(root, layout_prop, Layout::flex().logical_size(300.0, 50.0));
    let a = ui.insert_child(root);
    let b = ui.insert_child(root);
    for id in [a, b] {
        ui.set_local(
            id,
            layout_prop,
            Layout::new().flex_grow(1.0).align_self_stretch(),
        );
        ui.set_focusable(id, true);
    }
    ui.compute_layout(Size::new(300.0, 50.0));

    let tab = KeyboardEvent::key_down(Key::Named(NamedKey::Tab), Code::Tab);
    let result = ui.dispatch_key(&tab);
    assert!(result.handled);
    assert!(result.focus_changed);
    assert_eq!(result.focused, Some(a));
    assert_eq!(ui.focused(), Some(a));

    let result = ui.dispatch_key(&tab);
    assert!(result.handled);
    assert!(result.focus_changed);
    assert_eq!(result.focused, Some(b));
    assert_eq!(ui.focused(), Some(b));

    let mut shift_tab = KeyboardEvent::key_down(Key::Named(NamedKey::Tab), Code::Tab);
    shift_tab.modifiers = Modifiers::SHIFT;
    let result = ui.dispatch_key(&shift_tab);
    assert!(result.handled);
    assert!(result.focus_changed);
    assert_eq!(result.focused, Some(a));
    assert_eq!(ui.focused(), Some(a));
}

#[test]
fn built_in_controls_mark_consumed_keys_handled() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    use crate::{Button, Slider, ToggleSwitch};
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

    let mut ui = Ui::new();
    let root = ui.root();
    let clicked = Rc::new(Cell::new(false));
    let clicked_for_handler = Rc::clone(&clicked);
    let button = ui.append(
        root,
        Button::new("Run").on_click(move |_ui, _event| clicked_for_handler.set(true)),
    );

    assert!(ui.set_focus(Some(button)));
    let enter = KeyboardEvent::key_down(Key::Named(NamedKey::Enter), Code::Enter);
    let result = ui.dispatch_key(&enter);
    assert!(result.handled);
    assert!(
        clicked.get(),
        "button Enter activation should synthesize a click"
    );

    let toggle = ui.append(root, ToggleSwitch::new());
    assert!(ui.set_focus(Some(toggle)));
    let result = ui.dispatch_key(&enter);
    assert!(result.handled);
    assert_eq!(
        ui.get(toggle)
            .and_then(|el| el.get_local_value(ui.properties().is_checked).copied()),
        Some(true)
    );

    let slider = ui.append(root, Slider::new().value(0.4));
    assert!(ui.set_focus(Some(slider)));
    let right = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
    let result = ui.dispatch_key(&right);
    assert!(result.handled);
    assert!(
        ui.get(slider)
            .and_then(|el| el.get_local_value(ui.properties().value).copied())
            .is_some_and(|value| value > 0.4),
        "slider ArrowRight should increase Value"
    );
}

#[test]
fn user_click_handler_does_not_replace_control_default_handler() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    use crate::ToggleSwitch;

    let mut ui = Ui::new();
    let root = ui.root();
    let toggle = ui.append(root, ToggleSwitch::new());
    let fired = Rc::new(Cell::new(false));
    let fired_for_handler = Rc::clone(&fired);
    ui.on_click(toggle, move |_ui, _event| fired_for_handler.set(true));

    let result = ui.synthesize_click(toggle, Point::ZERO);

    assert!(result.handled, "control default should handle the click");
    assert!(fired.get(), "user handler should still run");
    assert_eq!(
        ui.get(toggle)
            .and_then(|el| el.get_local_value(ui.properties().is_checked).copied()),
        Some(true),
        "control default handler should still toggle"
    );
}

#[test]
fn user_click_handler_can_prevent_control_default_handler() {
    use crate::ToggleSwitch;

    let mut ui = Ui::new();
    let root = ui.root();
    let toggle = ui.append(root, ToggleSwitch::new());
    ui.on_click(toggle, |_ui, event| event.mark_handled());

    let result = ui.synthesize_click(toggle, Point::ZERO);

    assert!(result.handled);
    assert_eq!(
        ui.get(toggle)
            .and_then(|el| el.get_local_value(ui.properties().is_checked).copied()),
        None,
        "handled user event should skip the control default handler"
    );
}

#[test]
fn dispatch_click_runs_target_handler() {
    // Register a click handler on a child element; dispatch a
    // click against it; the handler should run with phase Target
    // and source == child.
    use alloc::rc::Rc;
    use core::cell::Cell;

    let mut ui = Ui::new();
    let root = ui.root();
    let child = ui.insert_child(root);

    let fired_with = Rc::new(Cell::new(None::<(Phase, ElementId, ElementId)>));
    let fired = fired_with.clone();
    ui.on_click(child, move |_ui, event| {
        if matches!(event.phase, Phase::Target) {
            assert_eq!(event.kind, RoutedEventKind::Click);
            fired.set(Some((event.phase, event.source, event.current)));
        }
    });

    ui.dispatch_click(child, Point::ZERO);

    let (phase, source, current) = fired_with.get().expect("handler should have fired");
    assert!(matches!(phase, Phase::Target));
    assert_eq!(source, child);
    assert_eq!(current, child);
}

#[test]
fn dispatch_click_walks_capture_target_bubble() {
    // Register handlers on root, child, and grandchild; click the
    // grandchild. Expected dispatch order: root capture, child
    // capture, grandchild target, child bubble, root bubble.
    use alloc::rc::Rc;
    use alloc::string::String;
    use core::cell::RefCell;

    let mut ui = Ui::new();
    let root = ui.root();
    let child = ui.insert_child(root);
    let grand = ui.insert_child(child);

    let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    for (name, id) in [("root", root), ("child", child), ("grand", grand)] {
        let log = log.clone();
        ui.on_click(id, move |_ui, event| {
            let phase = match event.phase {
                Phase::Capture => "capture",
                Phase::Target => "target",
                Phase::Bubble => "bubble",
            };
            log.borrow_mut().push(alloc::format!("{name}:{phase}"));
        });
    }

    ui.dispatch_click(grand, Point::ZERO);

    let log = log.borrow().clone();
    assert_eq!(
        log,
        vec![
            "root:capture".to_owned(),
            "child:capture".to_owned(),
            "grand:target".to_owned(),
            "child:bubble".to_owned(),
            "root:bubble".to_owned(),
        ]
    );
}

#[test]
fn dispatch_click_stops_route_when_handler_stops_propagation() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    let mut ui = Ui::new();
    let root = ui.root();
    let child = ui.insert_child(root);

    let child_fired = Rc::new(Cell::new(false));
    let child_fired_for_handler = Rc::clone(&child_fired);
    ui.on_click(root, move |_ui, event| {
        if matches!(event.phase, Phase::Capture) {
            event.stop_propagation();
        }
    });
    ui.on_click(child, move |_ui, _event| {
        child_fired_for_handler.set(true);
    });

    let result = ui.dispatch_click(child, Point::ZERO);

    assert!(!result.handled);
    assert!(result.propagation_stopped);
    assert!(
        !child_fired.get(),
        "stopped capture-phase click should stop before target"
    );
}

#[test]
fn dispatch_scroll_walks_capture_target_bubble() {
    use alloc::rc::Rc;
    use alloc::string::String;
    use core::cell::RefCell;
    use ui_events::ScrollDelta;

    let mut ui = Ui::new();
    let root = ui.root();
    let child = ui.insert_child(root);
    let grand = ui.insert_child(child);

    let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let target_delta = Rc::new(RefCell::new(Vec2::ZERO));
    for (name, id) in [("root", root), ("child", child), ("grand", grand)] {
        let log = Rc::clone(&log);
        let target_delta = Rc::clone(&target_delta);
        ui.on_scroll(id, move |_ui, event| {
            let phase = match event.phase {
                Phase::Capture => "capture",
                Phase::Target => "target",
                Phase::Bubble => "bubble",
            };
            if matches!(event.phase, Phase::Target) {
                assert_eq!(event.kind, RoutedEventKind::Scroll);
                *target_delta.borrow_mut() =
                    event.logical_delta(Vec2::new(16.0, 20.0), Vec2::new(160.0, 200.0));
                event.mark_handled();
            }
            log.borrow_mut().push(alloc::format!("{name}:{phase}"));
        });
    }

    let result = ui.synthesize_scroll(grand, Point::ZERO, ScrollDelta::LineDelta(0.0, 2.0));

    assert!(result.handled);
    assert_eq!(*target_delta.borrow(), Vec2::new(0.0, 40.0));
    assert_eq!(
        log.borrow().as_slice(),
        &[
            "root:capture".to_owned(),
            "child:capture".to_owned(),
            "grand:target".to_owned(),
        ]
    );
}

#[test]
fn pointer_scroll_hit_tests_and_routes_to_target() {
    use alloc::rc::Rc;
    use core::cell::Cell;
    use ui_events::ScrollDelta;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(200.0, 200.0));
    let child = ui.insert_child(root);
    ui.set_layout(child, Layout::new().logical_size(80.0, 80.0));
    ui.compute_layout(Size::new(200.0, 200.0));

    let fired = Rc::new(Cell::new(false));
    let fired_for_handler = Rc::clone(&fired);
    ui.on_scroll(child, move |_ui, event| {
        if matches!(event.phase, Phase::Target) {
            assert_eq!(event.source, child);
            assert_eq!(event.position, Point::new(20.0, 20.0));
            fired_for_handler.set(true);
            event.mark_handled();
        }
    });

    let event = pointer_scroll(
        Point::new(20.0, 20.0),
        123,
        ScrollDelta::LineDelta(0.0, 1.0),
    );
    assert!(ui.dispatch_pointer_event(&event).handled);
    assert!(fired.get());
    assert_eq!(ui.now(), 123);
}

#[test]
fn pointer_down_walks_capture_target_bubble() {
    use alloc::rc::Rc;
    use alloc::string::String;
    use core::cell::RefCell;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(200.0, 200.0));
    let child = ui.insert_child(root);
    ui.set_layout(child, Layout::new().logical_size(100.0, 100.0));
    let grand = ui.insert_child(child);
    ui.set_layout(grand, Layout::new().logical_size(50.0, 50.0));
    ui.compute_layout(Size::new(200.0, 200.0));

    let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    for (name, id) in [("root", root), ("child", child), ("grand", grand)] {
        let log = Rc::clone(&log);
        ui.on_pointer(id, PointerEventKind::Down, move |_ui, event| {
            let phase = match event.phase {
                Phase::Capture => "capture",
                Phase::Target => "target",
                Phase::Bubble => "bubble",
            };
            assert_eq!(event.kind, RoutedEventKind::PointerDown);
            assert_eq!(event.position, Some(Point::new(20.0, 20.0)));
            log.borrow_mut().push(alloc::format!("{name}:{phase}"));
        });
    }

    let result = ui.dispatch_pointer_event(&pointer_down(Point::new(20.0, 20.0), 1));

    assert!(result.capture_changed);
    assert_eq!(
        log.borrow().as_slice(),
        &[
            "root:capture".to_owned(),
            "child:capture".to_owned(),
            "grand:target".to_owned(),
            "child:bubble".to_owned(),
            "root:bubble".to_owned(),
        ]
    );
}

#[test]
fn handled_pointer_down_suppresses_derived_click() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(200.0, 80.0));
    let child = ui.insert_child(root);
    ui.set_layout(child, Layout::new().logical_size(100.0, 60.0));
    ui.compute_layout(Size::new(200.0, 80.0));

    ui.on_pointer(child, PointerEventKind::Down, |_ui, event| {
        if matches!(event.phase, Phase::Target) {
            event.mark_handled();
        }
    });

    let click_fired = Rc::new(Cell::new(false));
    let click_fired_for_handler = Rc::clone(&click_fired);
    ui.on_click(child, move |_ui, event| {
        if matches!(event.phase, Phase::Target) {
            click_fired_for_handler.set(true);
        }
    });

    let point = Point::new(20.0, 20.0);
    assert!(ui.dispatch_pointer_event(&pointer_down(point, 1)).handled);
    let up = ui.dispatch_pointer_event(&pointer_up(point, 2));

    assert!(!click_fired.get());
    assert!(!up.handled);
}

#[test]
fn explicit_pointer_capture_routes_move_outside_hit_target() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(100.0, 100.0));
    let child = ui.insert_child(root);
    ui.set_layout(child, Layout::new().logical_size(40.0, 40.0));
    ui.compute_layout(Size::new(100.0, 100.0));

    ui.on_pointer(root, PointerEventKind::Down, move |ui, event| {
        if matches!(event.phase, Phase::Capture) {
            assert!(ui.capture_pointer(root));
            event.mark_handled();
        }
    });

    let move_seen = Rc::new(Cell::new(None::<(ElementId, Phase)>));
    let move_seen_for_handler = Rc::clone(&move_seen);
    ui.on_pointer(root, PointerEventKind::Move, move |_ui, event| {
        if matches!(event.phase, Phase::Target) {
            move_seen_for_handler.set(Some((event.source, event.phase)));
            event.mark_handled();
        }
    });

    let down = ui.dispatch_pointer_event(&pointer_down(Point::new(10.0, 10.0), 1));
    assert!(down.capture_changed);
    assert_eq!(ui.pointer_capture(), Some(root));

    let moved = ui.dispatch_pointer_event(&pointer_move(Point::new(160.0, 160.0), 2, true));
    assert!(moved.handled);
    assert_eq!(move_seen.get(), Some((root, Phase::Target)));

    let up = ui.dispatch_pointer_event(&pointer_up(Point::new(160.0, 160.0), 3));
    assert!(up.capture_changed);
    assert_eq!(ui.pointer_capture(), None);
}

#[test]
fn public_pointer_capture_release_updates_capture_state() {
    let mut ui = Ui::new();
    let root = ui.root();
    let child = ui.insert_child(root);

    assert!(ui.capture_pointer(child));
    assert_eq!(ui.pointer_capture(), Some(child));
    assert!(!ui.release_pointer_capture(root));
    assert_eq!(ui.pointer_capture(), Some(child));
    assert!(ui.release_pointer_capture(child));
    assert_eq!(ui.pointer_capture(), None);

    ui.set_enabled(child, false);
    assert!(!ui.capture_pointer(child));
    assert_eq!(ui.pointer_capture(), None);
}

#[test]
fn pointer_hover_changes_route_enter_and_leave_from_leaf() {
    use alloc::rc::Rc;
    use alloc::string::String;
    use core::cell::RefCell;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(200.0, 200.0));
    let child = ui.insert_child(root);
    ui.set_layout(child, Layout::new().logical_size(80.0, 80.0));
    ui.compute_layout(Size::new(200.0, 200.0));

    let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    for (name, id) in [("root", root), ("child", child)] {
        let enter_log = Rc::clone(&log);
        ui.on_pointer(id, PointerEventKind::Enter, move |_ui, event| {
            if matches!(event.phase, Phase::Target | Phase::Bubble) {
                enter_log
                    .borrow_mut()
                    .push(alloc::format!("{name}:enter:{:?}", event.phase));
            }
        });
        let leave_log = Rc::clone(&log);
        ui.on_pointer(id, PointerEventKind::Leave, move |_ui, event| {
            if matches!(event.phase, Phase::Target | Phase::Bubble) {
                leave_log
                    .borrow_mut()
                    .push(alloc::format!("{name}:leave:{:?}", event.phase));
            }
        });
    }

    let enter = ui.dispatch_pointer_event(&pointer_move(Point::new(20.0, 20.0), 1, false));
    let leave = ui.dispatch_pointer_event(&pointer_move(Point::new(120.0, 120.0), 2, false));

    assert!(enter.hover_changed);
    assert!(leave.hover_changed);
    assert_eq!(
        log.borrow().as_slice(),
        &[
            "child:enter:Target".to_owned(),
            "root:enter:Bubble".to_owned(),
            "child:leave:Target".to_owned(),
            "root:leave:Bubble".to_owned(),
            "root:enter:Target".to_owned(),
        ]
    );
}

#[test]
fn dispatch_drag_stops_route_when_handler_marks_handled() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    let mut ui = Ui::new();
    let root = ui.root();
    let child = ui.insert_child(root);
    ui.events.set_press_target(Some(child));
    ui.events.set_pointer_capture(Some(child));
    ui.events.start_press(Point::ZERO);

    let child_fired = Rc::new(Cell::new(false));
    let child_fired_for_handler = Rc::clone(&child_fired);
    ui.on_drag(root, move |_ui, event| {
        if matches!(event.phase, Phase::Capture) {
            event.mark_handled();
        }
    });
    ui.on_drag(child, move |_ui, _event| {
        assert_eq!(_event.kind, RoutedEventKind::Drag);
        child_fired_for_handler.set(true);
    });

    let result = ui.dispatch_drag(Point::new(4.0, 0.0));

    assert!(result.handled);
    assert!(!result.propagation_stopped);
    assert!(
        !child_fired.get(),
        "handled capture-phase drag should stop before target"
    );
}

#[test]
fn pointer_capture_preserves_template_part_as_drag_source() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    use crate::{Button, SLOT_BUTTON_CONTENT};

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, Button::new("Drag"));
    ui.compute_layout(Size::new(220.0, 80.0));

    let label = ui.slot(button, SLOT_BUTTON_CONTENT).unwrap();
    let press = ui.effective_bounds(label).center();

    let seen = Rc::new(Cell::new(None::<(ElementId, Phase)>));
    let seen_for_handler = Rc::clone(&seen);
    ui.on_drag(button, move |_ui, event| {
        if matches!(event.phase, Phase::Bubble) {
            seen_for_handler.set(Some((event.source, event.phase)));
            event.mark_handled();
        }
    });

    let down = ui.dispatch_pointer_event(&pointer_down(press, 1));
    assert!(down.capture_changed);
    assert_eq!(ui.pointer_capture(), Some(label));

    let moved =
        ui.dispatch_pointer_event(&pointer_move(Point::new(press.x + 8.0, press.y), 2, true));
    assert!(moved.handled);
    assert_eq!(seen.get(), Some((label, Phase::Bubble)));

    let up = ui.dispatch_pointer_event(&pointer_up(Point::new(press.x + 8.0, press.y), 3));
    assert!(up.capture_changed);
    assert_eq!(ui.pointer_capture(), None);
}

#[test]
fn remove_click_handler_stops_firing() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    let mut ui = Ui::new();
    let root = ui.root();
    let child = ui.insert_child(root);

    let count = Rc::new(Cell::new(0_u32));
    let inc = count.clone();
    ui.on_click(child, move |_ui, _event| {
        inc.set(inc.get() + 1);
    });

    ui.dispatch_click(child, Point::ZERO);
    assert_eq!(count.get(), 1);

    ui.remove_click_handler(child);
    ui.dispatch_click(child, Point::ZERO);
    assert_eq!(count.get(), 1, "removed handler should not fire");
}

#[test]
fn click_handler_can_remove_itself_during_dispatch() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    let mut ui = Ui::new();
    let root = ui.root();
    let child = ui.insert_child(root);

    let count = Rc::new(Cell::new(0_u32));
    let count_for_handler = Rc::clone(&count);
    ui.on_click(child, move |ui, event| {
        if matches!(event.phase, Phase::Target) {
            count_for_handler.set(count_for_handler.get() + 1);
            ui.remove_click_handler(child);
        }
    });

    ui.dispatch_click(child, Point::ZERO);
    ui.dispatch_click(child, Point::ZERO);

    assert_eq!(count.get(), 1, "self-removed handler should stay removed");
}

#[test]
fn key_handler_can_remove_itself_during_dispatch() {
    use alloc::rc::Rc;
    use core::cell::Cell;
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

    let mut ui = Ui::new();
    let root = ui.root();
    let child = ui.insert_child(root);
    ui.set_focusable(child, true);
    ui.set_focus(Some(child));

    let count = Rc::new(Cell::new(0_u32));
    let count_for_handler = Rc::clone(&count);
    ui.on_key(child, move |ui, event| {
        if matches!(event.phase, Phase::Target) {
            count_for_handler.set(count_for_handler.get() + 1);
            ui.remove_key_handler(child);
        }
    });

    let key = KeyboardEvent::key_down(Key::Named(NamedKey::Enter), Code::Enter);
    let _ = ui.dispatch_key(&key);
    let _ = ui.dispatch_key(&key);

    assert_eq!(count.get(), 1, "self-removed handler should stay removed");
}

#[test]
fn drag_handler_can_remove_itself_during_dispatch() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    let mut ui = Ui::new();
    let root = ui.root();
    let child = ui.insert_child(root);
    ui.events.set_press_target(Some(child));
    ui.events.set_pointer_capture(Some(child));
    ui.events.start_press(Point::ZERO);

    let count = Rc::new(Cell::new(0_u32));
    let count_for_handler = Rc::clone(&count);
    ui.on_drag(child, move |ui, event| {
        if matches!(event.phase, Phase::Target) {
            count_for_handler.set(count_for_handler.get() + 1);
            ui.remove_drag_handler(child);
        }
    });

    ui.dispatch_drag(Point::new(1.0, 0.0));
    ui.dispatch_drag(Point::new(2.0, 0.0));

    assert_eq!(count.get(), 1, "self-removed handler should stay removed");
}
