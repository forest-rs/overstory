use super::presented_element_text;
use super::{COMPOSE, LAYOUT};
use crate::{
    Button, ControlTemplateSpecExt, SLOT_BUTTON_CONTENT, SLOT_SELECT_LABEL, Select, TYPE_BUTTON,
    TemplateNode, TemplateSlot, TemplateSpec, Ui, selected_select_item, set_select_index,
};
use alloc::string::String;
use understory_style::{
    ClassId, SelectorBuilder, SelectorStep, StyleBuilder, StyleCascadeBuilder, StyleOrigin,
};

#[test]
fn select_selection_updates_presented_content_label() {
    let mut ui = Ui::new();
    let root = ui.root();
    let select = ui.append(
        root,
        Select::new()
            .items(["Alpha", "Beta", "Gamma"])
            .selected_index(0),
    );
    let label = ui
        .slot(select, SLOT_SELECT_LABEL)
        .expect("default select template should expose its label slot");

    assert_eq!(
        presented_element_text(&mut ui, label).as_deref(),
        Some("Alpha")
    );

    assert!(set_select_index(&mut ui, select, 2));
    assert_eq!(selected_select_item(&ui, select).as_deref(), Some("Gamma"));
    assert_eq!(
        presented_element_text(&mut ui, label).as_deref(),
        Some("Gamma")
    );
}

#[test]
fn content_writes_invalidate_declared_content_presenter_dependents_only() {
    const EXTRA_SLOT: TemplateSlot = TemplateSlot::new("test.extra");

    let template = TemplateSpec::from_nodes([
        TemplateNode::content_presenter(SLOT_BUTTON_CONTENT),
        TemplateNode::text_slot(EXTRA_SLOT),
    ]);
    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, Button::new("Before").template(template));
    let content = ui
        .slot(button, SLOT_BUTTON_CONTENT)
        .expect("custom template should expose its content presenter");
    let extra = ui
        .slot(button, EXTRA_SLOT)
        .expect("custom template should expose its unrelated text slot");

    ui.invalidation.clear(LAYOUT);

    ui.set_local(button, ui.properties().content, String::from("After"));

    assert!(ui.invalidation().is_invalidated(content, LAYOUT));
    assert!(!ui.invalidation().is_invalidated(extra, LAYOUT));
}

#[test]
fn template_nodes_can_depend_on_non_content_owner_properties() {
    const DEPENDENT_SLOT: TemplateSlot = TemplateSlot::new("test.dependent");
    const OTHER_SLOT: TemplateSlot = TemplateSlot::new("test.other");

    let mut ui = Ui::new();
    let template = TemplateSpec::from_nodes([
        TemplateNode::text_slot(DEPENDENT_SLOT).reads_owner_property(ui.properties().opacity),
        TemplateNode::text_slot(OTHER_SLOT),
    ]);
    let root = ui.root();
    let button = ui.append(root, Button::new("Before").template(template));
    let dependent = ui
        .slot(button, DEPENDENT_SLOT)
        .expect("custom template should expose its dependent slot");
    let other = ui
        .slot(button, OTHER_SLOT)
        .expect("custom template should expose its unrelated slot");

    ui.invalidation.clear(COMPOSE);

    ui.set_local(button, ui.properties().opacity, 0.5_f32);

    assert!(ui.invalidation().is_invalidated(dependent, COMPOSE));
    assert!(!ui.invalidation().is_invalidated(other, COMPOSE));
}

#[test]
fn styled_owner_property_change_invalidates_declared_template_dependents_only() {
    const STYLED_OWNER: ClassId = ClassId(0x79);
    const DEPENDENT_SLOT: TemplateSlot = TemplateSlot::new("test.styled-dependent");
    const OTHER_SLOT: TemplateSlot = TemplateSlot::new("test.styled-other");

    let mut ui = Ui::new();
    let template = TemplateSpec::from_nodes([
        TemplateNode::text_slot(DEPENDENT_SLOT).reads_owner_property(ui.properties().opacity),
        TemplateNode::text_slot(OTHER_SLOT),
    ]);
    let selector =
        SelectorBuilder::new(SelectorStep::type_tag(TYPE_BUTTON).with_class(STYLED_OWNER)).build();
    ui.set_cascade(
        StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                selector,
                StyleBuilder::new()
                    .set(ui.properties().opacity, 0.5_f32)
                    .build(),
            )
            .build(),
    );
    let root = ui.root();
    let button = ui.append(root, Button::new("Before").template(template));
    let dependent = ui
        .slot(button, DEPENDENT_SLOT)
        .expect("custom template should expose its dependent slot");
    let other = ui
        .slot(button, OTHER_SLOT)
        .expect("custom template should expose its unrelated slot");

    ui.invalidation.clear(COMPOSE);

    ui.add_class(button, STYLED_OWNER);

    assert!(ui.invalidation().is_invalidated(dependent, COMPOSE));
    assert!(!ui.invalidation().is_invalidated(other, COMPOSE));
}
