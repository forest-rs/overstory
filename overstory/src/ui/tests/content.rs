// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn button_label_content_presenter_reads_owner_content() {
    // Regression guard for two sibling buttons: a template label is a
    // presenter over its owner content, so each generated label must display
    // its own templated owner rather than copied or shared state.
    use crate::Button;

    let mut ui = Ui::new();
    let root = ui.root();
    let b1 = ui.append(root, Button::new("Click me"));
    let b2 = ui.append(root, Button::new("Or me"));

    assert_eq!(
        presented_button_label_text(&mut ui, b1).as_deref(),
        Some("Click me")
    );
    assert_eq!(
        presented_button_label_text(&mut ui, b2).as_deref(),
        Some("Or me")
    );
}

#[test]
fn content_presenter_hosts_retained_children_over_text_fallback() {
    use crate::{Button, SLOT_BUTTON_CONTENT, Text};

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, Button::new("Fallback"));
    let presenter = ui
        .slot(button, SLOT_BUTTON_CONTENT)
        .expect("button template should expose content slot");

    assert_eq!(
        presented_element_text(&mut ui, presenter).as_deref(),
        Some("Fallback")
    );

    let child = ui
        .slot_children(button, SLOT_BUTTON_CONTENT, |content| {
            content.add(Text::new("Child"))
        })
        .expect("button template should expose content slot");

    assert_eq!(ui.get(child).unwrap().parent(), Some(presenter));
    assert_eq!(presented_element_text(&mut ui, presenter), None);
    assert_eq!(
        presented_element_text(&mut ui, child).as_deref(),
        Some("Child")
    );

    ui.remove(child);
    assert_eq!(
        presented_element_text(&mut ui, presenter).as_deref(),
        Some("Fallback")
    );
}

#[test]
fn button_uses_shared_content_slot_authoring() {
    use crate::{Button, Text, TooltipSpecExt};

    let mut ui = Ui::new();
    let root = ui.root();
    let (button, child) = ui.content(
        root,
        Button::new("Fallback").tooltip("Runs the deploy workflow"),
        |content| content.add(Text::new("Deploy")),
    );
    let child = child.expect("button template should expose content slot");
    let presenter = ui
        .slot(button, crate::SLOT_BUTTON_CONTENT)
        .expect("button template should expose content slot");

    assert_eq!(ui.get(child).unwrap().parent(), Some(presenter));
    assert_eq!(presented_element_text(&mut ui, presenter), None);
    assert_eq!(
        presented_element_text(&mut ui, child).as_deref(),
        Some("Deploy")
    );
}

#[test]
fn label_controls_use_shared_content_slot_authoring() {
    use crate::{
        CheckBox, RadioButton, SLOT_CHECK_BOX_LABEL, SLOT_RADIO_BUTTON_LABEL, SLOT_TOOLTIP_CONTENT,
        Text, Tooltip,
    };

    let mut ui = Ui::new();
    let root = ui.root();

    let (checkbox, checkbox_child) = ui.content(root, CheckBox::new(""), |content| {
        content.add(Text::new("Check"))
    });
    let checkbox_child = checkbox_child.expect("checkbox template should expose label slot");
    assert_eq!(
        ui.get(checkbox_child).unwrap().parent(),
        ui.slot(checkbox, SLOT_CHECK_BOX_LABEL)
    );

    let (radio, radio_child) = ui.content(root, RadioButton::new(""), |content| {
        content.add(Text::new("Radio"))
    });
    let radio_child = radio_child.expect("radio template should expose label slot");
    assert_eq!(
        ui.get(radio_child).unwrap().parent(),
        ui.slot(radio, SLOT_RADIO_BUTTON_LABEL)
    );

    let (tooltip, tooltip_child) = ui.content(root, Tooltip::new(""), |content| {
        content.add(Text::new("Tooltip"))
    });
    let tooltip_child = tooltip_child.expect("tooltip template should expose content slot");
    assert_eq!(
        ui.get(tooltip_child).unwrap().parent(),
        ui.slot(tooltip, SLOT_TOOLTIP_CONTENT)
    );
}

#[test]
fn app_bound_button_content_updates_template_label() {
    use crate::{BindingSpecExt, Button};
    use alloc::format;

    let mut ui = Ui::new();
    let root = ui.root();
    let activations = ui.local_state(0_u32);
    let button = ui.append(
        root,
        Button::new("").bind_content(activations.map(|count| {
            if *count == 0 {
                String::from("Deploy")
            } else {
                format!("Clicked {count}")
            }
        })),
    );

    ui.flush_bindings();

    assert_eq!(
        presented_button_label_text(&mut ui, button).as_deref(),
        Some("Deploy")
    );

    assert!(activations.set(&mut ui, 2));
    ui.flush_bindings();
    assert_eq!(
        presented_button_label_text(&mut ui, button).as_deref(),
        Some("Clicked 2")
    );
}
