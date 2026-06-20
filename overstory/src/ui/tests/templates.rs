// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn template_overrides_replace_visuals_not_control_behavior() {
    use crate::{
        Button, CheckBox, SLOT_BUTTON_CONTENT, SLOT_CHECK_BOX_BOX, SLOT_CHECK_BOX_LABEL,
        SLOT_CHECK_BOX_MARK, SLOT_SLIDER_THUMB, SLOT_TOGGLE_THUMB, Slider, Template, TemplateChild,
        Text, TextWidget, ToggleSwitch, set_slider_value,
    };

    const OUTER_FRAME: PartTag = PartTag(0x40);
    const INNER_FRAME: PartTag = PartTag(0x41);

    struct DoubleFrameButtonTemplate;
    impl Template for DoubleFrameButtonTemplate {
        fn instantiate(&self, cx: &mut TemplateContext<'_>) {
            let outer = cx.insert(TemplateChild::surface_part(OUTER_FRAME));
            let inner = cx.insert_under(outer, TemplateChild::surface_part(INNER_FRAME));
            let _content =
                cx.insert_under(inner, TemplateChild::content_presenter(SLOT_BUTTON_CONTENT));
        }
    }

    struct SingleSlotTemplate(TemplateSlot);
    impl Template for SingleSlotTemplate {
        fn instantiate(&self, cx: &mut TemplateContext<'_>) {
            let _part = cx.insert(TemplateChild::surface_slot(self.0));
        }
    }

    static BUTTON_TEMPLATE: DoubleFrameButtonTemplate = DoubleFrameButtonTemplate;
    static TOGGLE_TEMPLATE: SingleSlotTemplate = SingleSlotTemplate(SLOT_TOGGLE_THUMB);
    static SLIDER_TEMPLATE: SingleSlotTemplate = SingleSlotTemplate(SLOT_SLIDER_THUMB);

    struct CheckBoxTemplate;
    impl Template for CheckBoxTemplate {
        fn instantiate(&self, cx: &mut TemplateContext<'_>) {
            let box_part = cx.insert(TemplateChild::surface_slot(SLOT_CHECK_BOX_BOX));
            let _mark = cx.insert_under(
                box_part,
                TemplateChild::slot(SLOT_CHECK_BOX_MARK, Box::new(TextWidget)),
            );
            let _label = cx.insert(TemplateChild::content_presenter(SLOT_CHECK_BOX_LABEL));
        }
    }

    static CHECK_BOX_TEMPLATE: CheckBoxTemplate = CheckBoxTemplate;

    let mut ui = Ui::new();
    let root = ui.root();

    let button = ui.append(root, Button::new("Run").template(&BUTTON_TEMPLATE));
    let outer = ui.part(button, OUTER_FRAME).unwrap();
    let inner = ui.part(button, INNER_FRAME).unwrap();
    let content = ui.slot(button, SLOT_BUTTON_CONTENT).unwrap();
    assert_eq!(ui.get(outer).unwrap().parent(), Some(button));
    assert_eq!(ui.get(inner).unwrap().parent(), Some(outer));
    assert_eq!(ui.get(content).unwrap().parent(), Some(inner));
    assert_eq!(
        presented_button_label_text(&mut ui, button).as_deref(),
        Some("Run")
    );
    let structured_label = ui
        .slot_children(button, SLOT_BUTTON_CONTENT, |content| {
            content.add(Text::new("Retained"))
        })
        .expect("custom button template should expose its content slot");
    assert_eq!(ui.get(structured_label).unwrap().parent(), Some(content));
    assert_eq!(presented_element_text(&mut ui, content), None);
    assert_eq!(
        presented_element_text(&mut ui, structured_label).as_deref(),
        Some("Retained")
    );

    let toggle = ui.append(root, ToggleSwitch::new().template(&TOGGLE_TEMPLATE));
    ui.synthesize_click(toggle, Point::ZERO);
    let checked = ui
        .get(toggle)
        .and_then(|el| el.get_local_value(ui.properties().is_checked).copied())
        .unwrap_or(false);
    assert!(
        checked,
        "custom toggle template should keep toggle behavior"
    );

    let slider = ui.append(root, Slider::new().template(&SLIDER_TEMPLATE).value(0.75));
    assert!(ui.slot(slider, SLOT_SLIDER_THUMB).is_some());
    assert_eq!(
        ui.get(slider)
            .unwrap()
            .get_local_value(ui.properties().value)
            .copied(),
        Some(0.75)
    );
    set_slider_value(&mut ui, slider, 0.25);
    assert_eq!(
        ui.get(slider)
            .unwrap()
            .get_local_value(ui.properties().value)
            .copied(),
        Some(0.25)
    );

    let checkbox = ui.append(root, CheckBox::new("Confirm").template(&CHECK_BOX_TEMPLATE));
    assert!(ui.slot(checkbox, SLOT_CHECK_BOX_BOX).is_some());
    assert!(ui.slot(checkbox, SLOT_CHECK_BOX_MARK).is_some());
    let checkbox_label = ui.slot(checkbox, SLOT_CHECK_BOX_LABEL).unwrap();
    assert_eq!(
        presented_element_text(&mut ui, checkbox_label).as_deref(),
        Some("Confirm")
    );
    ui.synthesize_click(checkbox, Point::ZERO);
    assert_eq!(
        ui.get(checkbox)
            .unwrap()
            .get_local_value(ui.properties().is_checked)
            .copied(),
        Some(true)
    );
}

#[test]
fn owned_template_specs_work_through_control_builders() {
    use crate::{Button, SLOT_BUTTON_CONTENT, TemplateNode, TemplateSpec};

    const OWNED_FRAME: PartTag = PartTag(0x44);
    let owned_class = ClassId(0x45);

    let template = TemplateSpec::from_nodes([TemplateNode::surface_part(OWNED_FRAME)
        .with_class(owned_class)
        .with_child(TemplateNode::content_presenter(SLOT_BUTTON_CONTENT))]);

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, Button::new("Owned").template(template));

    let frame = ui
        .part(button, OWNED_FRAME)
        .expect("owned template should create its surface part");
    let content = ui
        .slot(button, SLOT_BUTTON_CONTENT)
        .expect("owned template should create its content slot");

    assert!(ui.get(frame).unwrap().classes().contains(&owned_class));
    assert!(
        ui.get(button)
            .unwrap()
            .get_local_value(ui.properties().control_template)
            .is_some()
    );
    assert_eq!(ui.get(content).unwrap().parent(), Some(frame));
    assert_eq!(
        presented_element_text(&mut ui, content).as_deref(),
        Some("Owned")
    );
}

#[test]
fn style_can_supply_initial_control_template() {
    use crate::{
        Button, ControlTemplate, SLOT_BUTTON_CONTENT, TYPE_BUTTON, TemplateNode, TemplateSpec,
    };

    const TEMPLATE_CLASS: ClassId = ClassId(0x46);
    const STYLE_FRAME: PartTag = PartTag(0x47);
    let style_template =
        ControlTemplate::from(TemplateSpec::from_nodes([TemplateNode::surface_part(
            STYLE_FRAME,
        )
        .with_child(TemplateNode::content_presenter(SLOT_BUTTON_CONTENT))]));

    let mut ui = Ui::new();
    let root = ui.root();
    let style = StyleBuilder::new()
        .set(ui.properties().control_template, Some(style_template))
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            SelectorBuilder::new(SelectorStep::type_tag(TYPE_BUTTON).with_class(TEMPLATE_CLASS))
                .build(),
            style,
        )
        .build();
    ui.set_cascade(cascade);

    let button = ui.append(root, Button::new("Styled").class(TEMPLATE_CLASS));
    let frame = ui
        .part(button, STYLE_FRAME)
        .expect("style-authored template should create its frame");
    let content = ui
        .slot(button, SLOT_BUTTON_CONTENT)
        .expect("style-authored template should create its content slot");

    assert_eq!(ui.get(frame).unwrap().parent(), Some(button));
    assert_eq!(ui.get(content).unwrap().parent(), Some(frame));
    assert_eq!(
        presented_element_text(&mut ui, content).as_deref(),
        Some("Styled")
    );
}

#[test]
fn ui_new_supplies_framework_control_template_property() {
    use crate::controls::button::default_button_template;
    use crate::{Button, SLOT_BUTTON_CONTENT};

    let mut ui = Ui::new();

    let button = ui.append(ui.root(), Button::new("Default"));
    let element = ui.get(button).unwrap();
    let resolved = ui.resolve_value_for(element, ui.properties().control_template);

    assert_eq!(
        resolved.as_ref().map(crate::ControlTemplate::id),
        Some(default_button_template().id())
    );
    assert_eq!(
        ui.template_instance(button).unwrap().template_id(),
        default_button_template().id()
    );
    assert!(
        ui.slot(button, SLOT_BUTTON_CONTENT).is_some(),
        "default template should still expose button content"
    );
}

#[test]
fn empty_app_cascade_preserves_framework_control_template_property() {
    use crate::controls::button::default_button_template;
    use crate::{Button, SLOT_BUTTON_CONTENT};

    let mut ui = Ui::new();
    ui.set_cascade(StyleCascadeBuilder::new().build());

    let button = ui.append(ui.root(), Button::new("Default"));
    let element = ui.get(button).unwrap();
    let resolved = ui.resolve_value_for(element, ui.properties().control_template);

    assert_eq!(
        resolved.as_ref().map(crate::ControlTemplate::id),
        Some(default_button_template().id())
    );
    assert_eq!(
        ui.template_instance(button).unwrap().template_id(),
        default_button_template().id()
    );
    assert!(
        ui.slot(button, SLOT_BUTTON_CONTENT).is_some(),
        "framework template should still expose button content"
    );
}

#[test]
fn sheet_style_control_template_replaces_default_cascade_template() {
    use crate::controls::button::TYPE_BUTTON;
    use crate::{Button, ControlTemplate, SLOT_BUTTON_CONTENT, TemplateNode, TemplateSpec};

    const STYLE_FRAME: PartTag = PartTag(0x51);
    let style_template =
        ControlTemplate::from(TemplateSpec::from_nodes([TemplateNode::surface_part(
            STYLE_FRAME,
        )
        .with_child(TemplateNode::content_presenter(SLOT_BUTTON_CONTENT))]));
    let style_template_id = style_template.id();

    let mut ui = Ui::new();
    let style = StyleBuilder::new()
        .set(ui.properties().control_template, Some(style_template))
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Sheet,
            crate::selectors::type_tag(TYPE_BUTTON),
            style,
        )
        .build();
    ui.set_cascade(cascade);

    let button = ui.append(ui.root(), Button::new("Styled"));

    assert_eq!(
        ui.template_instance(button).unwrap().template_id(),
        style_template_id
    );
    let frame = ui.part(button, STYLE_FRAME).unwrap();
    let content = ui.slot(button, SLOT_BUTTON_CONTENT).unwrap();
    assert_eq!(
        ui.get(content).unwrap().parent(),
        Some(frame),
        "the stronger style template should own the realized visual structure"
    );
}

#[test]
fn style_change_can_materialize_template_after_insert() {
    use crate::{ControlTemplate, TemplateNode, TemplateSpec, Widget};

    const TYPE_LATE_TEMPLATE: understory_style::TypeTag = understory_style::TypeTag(0x52);
    const LATE_PART: PartTag = PartTag(0x53);

    #[derive(Debug)]
    struct LateTemplateWidget;

    impl Widget for LateTemplateWidget {
        fn type_tag(&self) -> Option<understory_style::TypeTag> {
            Some(TYPE_LATE_TEMPLATE)
        }
    }

    let mut ui = Ui::new();
    let owner = ui.append_widget(ui.root(), Box::new(LateTemplateWidget));
    assert!(
        ui.template_instance(owner).is_none(),
        "the widget starts without a resolved ControlTemplate"
    );

    let template = ControlTemplate::from(TemplateSpec::from_nodes([TemplateNode::surface_part(
        LATE_PART,
    )]));
    let template_id = template.id();
    let style = StyleBuilder::new()
        .set(ui.properties().control_template, Some(template))
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Sheet,
            crate::selectors::type_tag(TYPE_LATE_TEMPLATE),
            style,
        )
        .build();

    ui.set_cascade(cascade);

    assert_eq!(
        ui.template_instance(owner).unwrap().template_id(),
        template_id
    );
    assert!(ui.part(owner, LATE_PART).is_some());
}

#[test]
fn changing_control_template_replaces_parts_and_preserves_slot_content() {
    use crate::{Button, ControlTemplate, SLOT_BUTTON_CONTENT, TemplateNode, TemplateSpec, Text};

    const OLD_FRAME: PartTag = PartTag(0x48);
    const NEW_FRAME: PartTag = PartTag(0x49);
    let old_template =
        ControlTemplate::from(TemplateSpec::from_nodes([TemplateNode::surface_part(
            OLD_FRAME,
        )
        .with_child(TemplateNode::content_presenter(SLOT_BUTTON_CONTENT))]));
    let new_template =
        ControlTemplate::from(TemplateSpec::from_nodes([TemplateNode::surface_part(
            NEW_FRAME,
        )
        .with_child(TemplateNode::content_presenter(SLOT_BUTTON_CONTENT))]));

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, Button::new("Button").template(old_template));
    let old_frame = ui
        .part(button, OLD_FRAME)
        .expect("old template should create its frame");
    let authored = ui
        .slot_children(button, SLOT_BUTTON_CONTENT, |content| {
            content.add(Text::new("Retained child"))
        })
        .expect("old template should expose the content slot");

    ui.set_local(button, ui.properties().control_template, Some(new_template));

    let new_frame = ui
        .part(button, NEW_FRAME)
        .expect("new template should create its frame");
    let content = ui
        .slot(button, SLOT_BUTTON_CONTENT)
        .expect("new template should expose the content slot");

    assert!(
        ui.get(old_frame).is_none(),
        "old generated frame is removed"
    );
    assert_eq!(ui.part(button, OLD_FRAME), None);
    assert_eq!(ui.get(new_frame).unwrap().parent(), Some(button));
    assert_eq!(ui.get(content).unwrap().parent(), Some(new_frame));
    assert_eq!(ui.get(authored).unwrap().parent(), Some(content));
    assert_eq!(
        presented_element_text(&mut ui, authored).as_deref(),
        Some("Retained child")
    );
}

#[test]
fn clearing_control_template_reexpands_default_template() {
    use crate::{Button, ControlTemplate, SLOT_BUTTON_CONTENT, TemplateNode, TemplateSpec, Text};

    const LOCAL_FRAME: PartTag = PartTag(0x4B);
    let local_template =
        ControlTemplate::from(TemplateSpec::from_nodes([TemplateNode::surface_part(
            LOCAL_FRAME,
        )
        .with_child(TemplateNode::content_presenter(SLOT_BUTTON_CONTENT))]));

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, Button::new("Button").template(local_template));
    let local_frame = ui
        .part(button, LOCAL_FRAME)
        .expect("local template should create its frame");
    let authored = ui
        .slot_children(button, SLOT_BUTTON_CONTENT, |content| {
            content.add(Text::new("Retained child"))
        })
        .expect("local template should expose the content slot");

    assert!(ui.clear_local(button, ui.properties().control_template));

    let content = ui
        .slot(button, SLOT_BUTTON_CONTENT)
        .expect("default button template should expose the content slot");
    assert!(
        ui.get(local_frame).is_none(),
        "local template frame is removed"
    );
    assert_eq!(ui.part(button, LOCAL_FRAME), None);
    assert_eq!(ui.get(content).unwrap().parent(), Some(button));
    assert_eq!(ui.get(authored).unwrap().parent(), Some(content));
    assert_eq!(
        presented_element_text(&mut ui, authored).as_deref(),
        Some("Retained child")
    );
}

#[test]
fn binding_to_control_template_reexpands_template() {
    use crate::{Button, ControlTemplate, SLOT_BUTTON_CONTENT, TemplateNode, TemplateSpec, Text};
    use understory_property_binding::PropertyEndpoint;

    const BOUND_FRAME: PartTag = PartTag(0x4A);
    let bound_template =
        ControlTemplate::from(TemplateSpec::from_nodes([TemplateNode::surface_part(
            BOUND_FRAME,
        )
        .with_child(TemplateNode::content_presenter(SLOT_BUTTON_CONTENT))]));

    let mut ui = Ui::new();
    let root = ui.root();
    let template = ui.local_state(Some(bound_template));
    let button = ui.append(root, Button::new("Button"));
    let authored = ui
        .slot_children(button, SLOT_BUTTON_CONTENT, |content| {
            content.add(Text::new("Retained child"))
        })
        .expect("default template should expose the content slot");

    ui.bind(
        template.endpoint(),
        PropertyEndpoint::new(
            BindingOwner::element(button),
            ui.properties().control_template,
        ),
    )
    .expect("control template binding registers");
    ui.flush_bindings();

    let frame = ui
        .part(button, BOUND_FRAME)
        .expect("bound template should create its frame");
    let content = ui
        .slot(button, SLOT_BUTTON_CONTENT)
        .expect("bound template should expose the content slot");

    assert_eq!(ui.get(frame).unwrap().parent(), Some(button));
    assert_eq!(ui.get(content).unwrap().parent(), Some(frame));
    assert_eq!(ui.get(authored).unwrap().parent(), Some(content));
    assert_eq!(
        presented_element_text(&mut ui, authored).as_deref(),
        Some("Retained child")
    );
}

#[test]
fn inspection_reports_optional_and_recommended_template_slots() {
    use crate::{
        SLOT_SLIDER_AXIS, SLOT_SLIDER_THUMB, SLOT_SLIDER_TICK, SLOT_SLIDER_TICK_LABEL,
        SLOT_SLIDER_TRACK, Slider, Template, TemplateChild,
    };

    struct MinimalSliderTemplate;
    impl Template for MinimalSliderTemplate {
        fn instantiate(&self, cx: &mut TemplateContext<'_>) {
            let _thumb = cx.insert(TemplateChild::surface_slot(SLOT_SLIDER_THUMB));
        }
    }

    static MINIMAL_SLIDER_TEMPLATE: MinimalSliderTemplate = MinimalSliderTemplate;

    let mut ui = Ui::new();
    let root = ui.root();
    let slider = ui.append(root, Slider::new().template(&MINIMAL_SLIDER_TEMPLATE));

    let report = ui
        .template_expansion_report(slider)
        .expect("slider should expose a template expansion report");
    assert_eq!(report.required_slots()[0].slot(), SLOT_SLIDER_THUMB);
    assert!(report.required_slots()[0].is_present());
    assert_eq!(report.recommended_slots()[0].slot(), SLOT_SLIDER_TRACK);
    assert!(!report.recommended_slots()[0].is_present());
    assert!(!report.has_missing_required_slots());
    assert!(report.has_missing_recommended_slots());
    assert!(!report.has_issues());

    let snapshot = ui.inspection_snapshot();
    let contract = snapshot
        .elements
        .iter()
        .find(|row| row.id == slider)
        .and_then(|row| row.template_contract.as_ref())
        .expect("slider should expose a template contract");

    assert_eq!(contract.required_slots[0].name, "slider-thumb");
    assert_eq!(
        contract.required_slots[0].element,
        ui.slot(slider, SLOT_SLIDER_THUMB)
    );
    assert!(!contract.has_missing_required_slots());

    assert_eq!(contract.recommended_slots.len(), 1);
    assert_eq!(contract.recommended_slots[0].name, "slider-track");
    assert_eq!(contract.recommended_slots[0].element, None);
    assert!(contract.has_missing_recommended_slots());
    assert!(
        !contract.has_issues(),
        "missing recommended slots should be a quality signal, not a contract failure"
    );

    assert_eq!(contract.optional_slots.len(), 3);
    assert_eq!(contract.optional_slots[0].name, SLOT_SLIDER_AXIS.name());
    assert_eq!(contract.optional_slots[1].name, SLOT_SLIDER_TICK.name());
    assert_eq!(
        contract.optional_slots[2].name,
        SLOT_SLIDER_TICK_LABEL.name()
    );
    assert!(
        contract
            .optional_slots
            .iter()
            .all(|slot| slot.element.is_none())
    );
    assert_eq!(ui.slot(slider, SLOT_SLIDER_TRACK), None);
}

#[test]
fn checkbox_template_contract_allows_optional_mark_slot() {
    use crate::{
        CheckBox, SLOT_CHECK_BOX_BOX, SLOT_CHECK_BOX_LABEL, SLOT_CHECK_BOX_MARK, Template,
        TemplateChild,
    };

    struct MinimalCheckBoxTemplate;
    impl Template for MinimalCheckBoxTemplate {
        fn instantiate(&self, cx: &mut TemplateContext<'_>) {
            let _box = cx.insert(TemplateChild::surface_slot(SLOT_CHECK_BOX_BOX));
            let _label = cx.insert(TemplateChild::content_presenter(SLOT_CHECK_BOX_LABEL));
        }
    }

    static MINIMAL_CHECK_BOX_TEMPLATE: MinimalCheckBoxTemplate = MinimalCheckBoxTemplate;

    let mut ui = Ui::new();
    let root = ui.root();
    let checkbox = ui.append(
        root,
        CheckBox::new("Confirm").template(&MINIMAL_CHECK_BOX_TEMPLATE),
    );

    let report = ui
        .template_expansion_report(checkbox)
        .expect("checkbox should expose a template expansion report");
    assert_eq!(report.required_slots()[0].slot(), SLOT_CHECK_BOX_BOX);
    assert_eq!(report.required_slots()[1].slot(), SLOT_CHECK_BOX_LABEL);
    assert!(report.required_slots().iter().all(|slot| slot.is_present()));
    assert_eq!(report.optional_slots()[0].slot(), SLOT_CHECK_BOX_MARK);
    assert!(!report.optional_slots()[0].is_present());
    assert!(!report.has_issues());

    ui.synthesize_click(checkbox, Point::ZERO);
    assert_eq!(
        ui.get(checkbox)
            .unwrap()
            .get_local_value(ui.properties().is_checked)
            .copied(),
        Some(true),
        "the optional visual mark slot should not be required for checkbox behavior",
    );
}

#[test]
fn inspection_reports_template_contract_slot_status() {
    use crate::{
        Button, SLOT_BUTTON_CONTENT, Template, TemplateChild, TemplateDiagnostic, TextWidget,
    };

    struct EmptyButtonTemplate;
    impl Template for EmptyButtonTemplate {
        fn instantiate(&self, _cx: &mut TemplateContext<'_>) {}
    }

    struct ContentButtonTemplate;
    impl Template for ContentButtonTemplate {
        fn instantiate(&self, cx: &mut TemplateContext<'_>) {
            let _content = cx.insert(TemplateChild::slot(
                SLOT_BUTTON_CONTENT,
                Box::new(TextWidget),
            ));
        }
    }

    struct DuplicateButtonTemplate;
    impl Template for DuplicateButtonTemplate {
        fn instantiate(&self, cx: &mut TemplateContext<'_>) {
            let _first = cx.insert(TemplateChild::slot(
                SLOT_BUTTON_CONTENT,
                Box::new(TextWidget),
            ));
            let _duplicate = cx.insert(TemplateChild::slot(
                SLOT_BUTTON_CONTENT,
                Box::new(TextWidget),
            ));
        }
    }

    static EMPTY_TEMPLATE: EmptyButtonTemplate = EmptyButtonTemplate;
    static CONTENT_TEMPLATE: ContentButtonTemplate = ContentButtonTemplate;
    static DUPLICATE_TEMPLATE: DuplicateButtonTemplate = DuplicateButtonTemplate;

    let mut ui = Ui::new();
    let root = ui.root();
    let default_button = ui.append(root, Button::new("Default"));
    let empty_button = ui.append(root, Button::new("Empty").template(&EMPTY_TEMPLATE));
    let custom_button = ui.append(root, Button::new("Custom").template(&CONTENT_TEMPLATE));
    let duplicate_button = ui.append(root, Button::new("Duplicate").template(&DUPLICATE_TEMPLATE));

    let snapshot = ui.inspection_snapshot();
    let default_contract = snapshot
        .elements
        .iter()
        .find(|row| row.id == default_button)
        .and_then(|row| row.template_contract.as_ref())
        .expect("default button should expose a template contract");
    assert_eq!(default_contract.required_slots[0].name, "content");
    assert_eq!(
        default_contract.required_slots[0].element,
        ui.slot(default_button, SLOT_BUTTON_CONTENT)
    );
    assert!(!default_contract.has_missing_required_slots());

    let empty_contract = snapshot
        .elements
        .iter()
        .find(|row| row.id == empty_button)
        .and_then(|row| row.template_contract.as_ref())
        .expect("custom button should expose the same control contract");
    assert_eq!(empty_contract.required_slots[0].name, "content");
    assert_eq!(empty_contract.required_slots[0].element, None);
    assert!(empty_contract.has_missing_required_slots());

    let custom_contract = snapshot
        .elements
        .iter()
        .find(|row| row.id == custom_button)
        .and_then(|row| row.template_contract.as_ref())
        .expect("custom template with content should expose contract status");
    assert_eq!(
        custom_contract.required_slots[0].element,
        ui.slot(custom_button, SLOT_BUTTON_CONTENT)
    );
    assert!(!custom_contract.has_missing_required_slots());

    let duplicate_contract = snapshot
        .elements
        .iter()
        .find(|row| row.id == duplicate_button)
        .and_then(|row| row.template_contract.as_ref())
        .expect("duplicate template should expose contract diagnostics");
    let content_part = SLOT_BUTTON_CONTENT.part_tag().unwrap();
    assert!(!duplicate_contract.has_missing_required_slots());
    assert!(duplicate_contract.has_issues());
    assert_eq!(duplicate_contract.diagnostics.len(), 2);
    assert!(
        duplicate_contract
            .diagnostics
            .iter()
            .any(|diagnostic| matches!(
                *diagnostic,
                TemplateDiagnostic::DuplicateSlot {
                    slot,
                    key: None,
                    ..
                } if slot == SLOT_BUTTON_CONTENT
            ))
    );
    assert!(
        duplicate_contract
            .diagnostics
            .iter()
            .any(|diagnostic| matches!(
                *diagnostic,
                TemplateDiagnostic::DuplicatePart {
                    part_tag,
                    key: None,
                    ..
                } if part_tag == content_part
            ))
    );
}

#[test]
fn surface_template_parts_do_not_match_generic_type_selector() {
    use crate::Slider;

    let mut ui = Ui::new();
    let root = ui.root();
    let slider = ui.append(root, Slider::new());
    let track = ui.part(slider, crate::PART_SLIDER_TRACK).unwrap();

    let red = Some(Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff)));
    let style = StyleBuilder::new()
        .set(ui.properties().surface.background, red)
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            crate::selectors::type_tag(TYPE_GENERIC),
            style,
        )
        .build();
    ui.set_cascade(cascade);

    let background =
        ui.resolve_value_for(ui.get(track).unwrap(), ui.properties().surface.background);
    assert_eq!(
        background, None,
        "surface-only template parts should not inherit TYPE_GENERIC as a fake type"
    );
}

#[test]
fn slider_template_parts_match_owner_local_part_selectors() {
    use crate::{PART_SLIDER_THUMB, PART_SLIDER_TRACK, SLOT_SLIDER_THUMB, Slider, TYPE_SLIDER};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_local(
        root,
        ui.properties().layout,
        Layout::flex().logical_size(300.0, 80.0),
    );

    let slider = ui.append(root, Slider::new());
    let track = ui.part(slider, PART_SLIDER_TRACK).unwrap();
    let thumb = ui
        .slot(slider, SLOT_SLIDER_THUMB)
        .expect("slider template should expose a thumb slot");
    let red = Some(Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff)));
    let style = StyleBuilder::new()
        .set(ui.properties().surface.background, red.clone())
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            crate::selectors::part(crate::selectors::template_part(
                TYPE_SLIDER,
                PART_SLIDER_TRACK,
            )),
            style,
        )
        .build();
    ui.set_cascade(cascade);

    let background =
        ui.resolve_value_for(ui.get(track).unwrap(), ui.properties().surface.background);
    assert_eq!(
        background, red,
        "owner-local part rule should match the nested slider track"
    );

    let snapshot = ui.inspection_snapshot();
    let slider_row = snapshot
        .elements
        .iter()
        .find(|row| row.id == slider)
        .expect("slider should be inspectable");
    assert_eq!(slider_row.type_tag, Some(TYPE_SLIDER));
    assert!(slider_row.has_widget_identity);

    let track_row = snapshot
        .elements
        .iter()
        .find(|row| row.id == track)
        .expect("track should be inspectable");
    assert_eq!(track_row.templated_parent, Some(slider));
    assert_eq!(track_row.type_tag, None);
    assert!(track_row.has_widget_identity);
    assert_eq!(track_row.part_tag, Some(PART_SLIDER_TRACK));
    assert_eq!(track_row.template_slot, Some("slider-track"));

    let thumb_row = snapshot
        .elements
        .iter()
        .find(|row| row.id == thumb)
        .expect("thumb should be inspectable");
    assert_eq!(thumb_row.templated_parent, Some(slider));
    assert_eq!(thumb_row.type_tag, None);
    assert!(thumb_row.has_widget_identity);
    assert_eq!(thumb_row.part_tag, Some(PART_SLIDER_THUMB));
    assert_eq!(thumb_row.template_slot, Some("slider-thumb"));

    ui.compute_layout(Size::new(300.0, 80.0));
    let mut output = Scene::new();
    ui.paint(&mut output);
    assert!(
        !output.is_empty(),
        "styled slider track should contribute painted output"
    );
}

#[test]
fn slider_template_creates_keyed_major_and_minor_tick_parts() {
    use crate::{
        CLASS_SLIDER_MAJOR_TICK, CLASS_SLIDER_MINOR_TICK, PART_SLIDER_TICK, PART_SLIDER_TICK_LABEL,
        SLOT_SLIDER_TICK, SLOT_SLIDER_TICK_LABEL, Slider, TemplateItemKey,
    };

    let mut ui = Ui::new();
    let root = ui.root();
    let slider = ui.append(root, Slider::new().min(10.0).max(20.0));
    let ticks: Vec<_> = ui.parts(slider, PART_SLIDER_TICK).collect();
    let labels: Vec<_> = ui.parts(slider, PART_SLIDER_TICK_LABEL).collect();

    assert_eq!(ticks.len(), 5);
    assert_eq!(labels.len(), 5);

    let instance = ui.template_instance(slider).unwrap();
    let first_tick = instance
        .keyed_part(PART_SLIDER_TICK, TemplateItemKey::new(0))
        .unwrap();
    assert_eq!(
        instance.keyed_slot(SLOT_SLIDER_TICK, TemplateItemKey::new(0)),
        Some(first_tick)
    );
    let second_tick = instance
        .keyed_part(PART_SLIDER_TICK, TemplateItemKey::new(1))
        .unwrap();
    assert!(
        ui.get(first_tick)
            .unwrap()
            .classes()
            .contains(&CLASS_SLIDER_MAJOR_TICK)
    );
    assert!(
        ui.get(second_tick)
            .unwrap()
            .classes()
            .contains(&CLASS_SLIDER_MINOR_TICK)
    );

    let middle_label = instance
        .keyed_part(PART_SLIDER_TICK_LABEL, TemplateItemKey::new(2))
        .unwrap();
    assert_eq!(
        instance.keyed_slot(SLOT_SLIDER_TICK_LABEL, TemplateItemKey::new(2)),
        Some(middle_label)
    );
    let text = ui.properties().text;
    let label = ui.get(middle_label).unwrap().get_local_value(text).cloned();
    assert_eq!(label.as_deref(), Some("15"));
}

#[test]
fn slider_keyboard_uses_small_and_large_changes() {
    use crate::Slider;
    use ui_events::keyboard::{Code, Key, KeyboardEvent, Modifiers, NamedKey};

    let mut ui = Ui::new();
    let root = ui.root();
    let slider = ui.append(
        root,
        Slider::new()
            .value(0.5)
            .small_change(0.1)
            .large_change(0.25),
    );
    assert!(ui.set_focus(Some(slider)));

    let right = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
    assert!(ui.dispatch_key(&right).handled);
    assert_eq!(slider_value(&ui, slider), Some(0.6));

    let mut shift_right =
        KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
    shift_right.modifiers = Modifiers::SHIFT;
    assert!(ui.dispatch_key(&shift_right).handled);
    assert_eq!(slider_value(&ui, slider), Some(0.85));

    let page_down = KeyboardEvent::key_down(Key::Named(NamedKey::PageDown), Code::PageDown);
    assert!(ui.dispatch_key(&page_down).handled);
    assert_eq!(slider_value(&ui, slider), Some(0.6));

    let home = KeyboardEvent::key_down(Key::Named(NamedKey::Home), Code::Home);
    assert!(ui.dispatch_key(&home).handled);
    assert_eq!(slider_value(&ui, slider), Some(0.0));

    let end = KeyboardEvent::key_down(Key::Named(NamedKey::End), Code::End);
    assert!(ui.dispatch_key(&end).handled);
    assert_eq!(slider_value(&ui, slider), Some(1.0));
}

#[test]
fn slider_thumb_click_and_drag_preserve_thumb_grab_offset() {
    use crate::{SLOT_SLIDER_THUMB, Slider};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_local(
        root,
        ui.properties().layout,
        Layout::flex().logical_size(300.0, 80.0),
    );
    let slider = ui.append(root, Slider::new().value(0.5));
    ui.compute_layout(Size::new(300.0, 80.0));

    let thumb = ui.slot(slider, SLOT_SLIDER_THUMB).unwrap();
    let before = ui.effective_bounds(thumb);
    let press = Point::new(before.x0 + 3.0, before.center().y);

    assert!(ui.dispatch_pointer_event(&pointer_down(press, 1)).changed());
    assert!(ui.dispatch_pointer_event(&pointer_up(press, 2)).changed());
    assert_eq!(
        slider_value(&ui, slider),
        Some(0.5),
        "clicking the thumb itself should not recenter it under the pointer"
    );

    assert!(ui.dispatch_pointer_event(&pointer_down(press, 3)).changed());
    assert!(
        ui.dispatch_pointer_event(&pointer_move(Point::new(press.x + 20.0, press.y), 4, true,))
            .changed()
    );
    ui.compute_layout(Size::new(300.0, 80.0));

    let after = ui.effective_bounds(thumb);
    assert!(
        ((after.center().x - before.center().x) - 20.0).abs() < 0.001,
        "thumb should move by the drag delta instead of snapping its center to the pointer"
    );
}

#[test]
fn default_cascade_styles_template_parts_by_owner_part() {
    use crate::{PART_SLIDER_TICK, Slider, TemplateItemKey, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    let cascade = theme::default_cascade(ui.properties());
    ui.set_cascade(cascade);

    let root = ui.root();
    let slider = ui.append(root, Slider::new());
    let track = ui.part(slider, crate::PART_SLIDER_TRACK).unwrap();
    let major_tick = ui
        .template_instance(slider)
        .unwrap()
        .keyed_part(PART_SLIDER_TICK, TemplateItemKey::new(0))
        .unwrap();
    let expected = Some(Brush::Solid(Color::from_rgba8(0x44, 0x44, 0x4a, 0xff)));
    let background =
        ui.resolve_value_for(ui.get(track).unwrap(), ui.properties().surface.background);
    let tick_background = ui.resolve_value_for(
        ui.get(major_tick).unwrap(),
        ui.properties().surface.background,
    );

    assert_eq!(background, expected);
    assert_eq!(
        tick_background,
        Some(Brush::Solid(Color::from_rgba8(0xc8, 0xc8, 0xd2, 0xff)))
    );
}

#[test]
fn default_cascade_leaves_generic_panels_transparent() {
    use crate::{Panel, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));

    let panel = ui.append(ui.root(), Panel::new());
    let background =
        ui.resolve_value_for(ui.get(panel).unwrap(), ui.properties().surface.background);

    assert_eq!(background, None);
}

#[test]
fn owner_pseudo_selector_can_style_template_descendant() {
    use crate::{Button, PART_BUTTON_LABEL, TYPE_BUTTON};

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, Button::new("Run"));
    let label = ui.part(button, PART_BUTTON_LABEL).unwrap();

    let red = Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff));
    let style = StyleBuilder::new()
        .set(ui.properties().foreground, red.clone())
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            crate::selectors::part_when(
                SelectorStep::type_tag(TYPE_BUTTON).with_pseudo(PSEUDO_PRESSED),
                crate::selectors::template_part(TYPE_BUTTON, PART_BUTTON_LABEL),
            ),
            style,
        )
        .build();
    ui.set_cascade(cascade);
    ui.add_pseudo(button, PSEUDO_PRESSED);

    let mut output = Scene::new();
    ui.paint(&mut output);

    let label_node = ui.box_for(label).expect("label should have a box node");
    let foreground = ui
        .presentation
        .presentation(label_node)
        .expect("label should have presentation")
        .plain_text()
        .expect("label should present text")
        .foreground
        .clone();
    assert_eq!(foreground, Some(red));
}
