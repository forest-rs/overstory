// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use crate::accessibility::accesskit_node_id;
use crate::test_support::{pointer_down, pointer_move, pointer_up};
use accesskit::{Action, ActionData, ActionRequest, TreeId};
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use kurbo::{Point, Size};
use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

#[test]
fn slider_range_and_step_builders_configure_properties() {
    let mut ui = Ui::new();
    let root = ui.root();
    let slider = ui.append(root, Slider::new().range(-10.0, 10.0).step(0.25).value(5.0));
    let element = ui.get(slider).unwrap();

    assert_eq!(
        ui.resolve_value_for(element, ui.properties().minimum),
        -10.0
    );
    assert_eq!(ui.resolve_value_for(element, ui.properties().maximum), 10.0);
    assert_eq!(
        ui.resolve_value_for(element, ui.properties().small_change),
        0.25
    );
    assert_eq!(ui.resolve_value_for(element, ui.properties().value), 5.0);
}

#[test]
fn slider_value_changed_reports_keyboard_step() {
    let observed = Rc::new(Cell::new(None));
    let observed_for_handler = Rc::clone(&observed);
    let mut ui = Ui::new();
    let root = ui.root();
    let slider = ui.append(
        root,
        Slider::new()
            .range(0.0, 10.0)
            .step(2.0)
            .value(4.0)
            .on_value_changed(move |_ui, event| {
                observed_for_handler.set(Some(event));
            }),
    );
    ui.set_focus(Some(slider));

    let right = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
    assert!(ui.dispatch_key(&right).handled);

    assert_eq!(
        observed.get(),
        Some(SliderValueChangedEvent {
            element: slider,
            previous_value: 4.0,
            value: 6.0,
            phase: SliderValueChangePhase::Committed,
        })
    );
    assert_eq!(
        ui.resolve_value_for(ui.get(slider).unwrap(), ui.properties().value),
        6.0
    );
}

#[test]
fn slider_value_changed_reports_accessibility_set_value() {
    let observed = Rc::new(Cell::new(None));
    let observed_for_handler = Rc::clone(&observed);
    let mut ui = Ui::new();
    let root = ui.root();
    let slider = ui.append(
        root,
        Slider::new()
            .range(0.0, 10.0)
            .value(4.0)
            .on_value_changed(move |_ui, event| {
                observed_for_handler.set(Some(event));
            }),
    );

    let handled = ui.handle_accesskit_action(&numeric_action(slider, Action::SetValue, 8.0));

    assert!(handled);
    assert_eq!(
        observed.get(),
        Some(SliderValueChangedEvent {
            element: slider,
            previous_value: 4.0,
            value: 8.0,
            phase: SliderValueChangePhase::Committed,
        })
    );
    assert_eq!(
        ui.resolve_value_for(ui.get(slider).unwrap(), ui.properties().value),
        8.0
    );
}

#[test]
fn slider_value_changed_reports_drag_live_and_commit() {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let observed_for_handler = Rc::clone(&observed);
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(300.0, 80.0));
    let slider = ui.append(
        root,
        Slider::new()
            .range(0.0, 1.0)
            .value(0.5)
            .on_value_changed(move |_ui, event| {
                observed_for_handler.borrow_mut().push(event);
            }),
    );
    ui.compute_layout(Size::new(300.0, 80.0));

    let thumb = ui.slot(slider, SLOT_SLIDER_THUMB).unwrap();
    let press = ui.effective_bounds(thumb).center();
    let drag = Point::new(press.x + 20.0, press.y);

    assert!(ui.dispatch_pointer_event(&pointer_down(press, 1)).changed());
    assert!(
        ui.dispatch_pointer_event(&pointer_move(drag, 2, true))
            .changed()
    );
    assert!(ui.dispatch_pointer_event(&pointer_up(drag, 3)).changed());

    let events = observed.borrow().clone();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].element, slider);
    assert_eq!(events[0].previous_value, 0.5);
    assert!(events[0].value > 0.5);
    assert_eq!(events[0].phase, SliderValueChangePhase::Changing);
    assert_eq!(
        events[1],
        SliderValueChangedEvent {
            element: slider,
            previous_value: 0.5,
            value: events[0].value,
            phase: SliderValueChangePhase::Committed,
        }
    );
}

fn numeric_action(element: ElementId, action: Action, value: f64) -> ActionRequest {
    ActionRequest {
        action,
        target_tree: TreeId::ROOT,
        target_node: accesskit_node_id(element),
        data: Some(ActionData::NumericValue(value)),
    }
}
