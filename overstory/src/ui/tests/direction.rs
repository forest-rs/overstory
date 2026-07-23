// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use accesskit::TextDirection as AccessTextDirection;
use imaging::record::Scene;
use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};
use understory_presentation::{BaseDirection, TextAlign};

use crate::{
    Button, Expander, LayoutDirection, Panel, ParagraphDirection, Popup, SLOT_BUTTON_CONTENT,
    SLOT_EXPANDER_DISCLOSURE, SLOT_SLIDER_THUMB, SegmentedControl, Slider, Text, TextSpecExt,
    accessibility::accesskit_node_id,
};

fn presented_base_direction(ui: &Ui, text: ElementId) -> BaseDirection {
    let node = ui.box_for(text).expect("text should own a layout node");
    ui.presentation
        .presentation(node)
        .and_then(|presentation| presentation.plain_text())
        .expect("text should have plain-text presentation")
        .layout
        .base_direction
}

#[test]
fn runtime_direction_change_reflows_retained_row_without_losing_focus() {
    let mut ui = Ui::new();
    let root = ui.root();
    let (row, (first, second)) = ui.children(
        root,
        Panel::row(0.0).layout(Layout::new().logical_size(200.0, 40.0)),
        |children| {
            let first = children.add(
                Panel::new()
                    .focusable(true)
                    .layout(Layout::new().logical_size(40.0, 20.0)),
            );
            let second = children.add(Panel::new().layout(Layout::new().logical_size(60.0, 20.0)));
            (first, second)
        },
    );

    assert!(ui.set_focus(Some(first)));
    ui.compute_layout(Size::new(200.0, 40.0));
    let ltr_first = ui.effective_bounds(first);
    let ltr_second = ui.effective_bounds(second);
    assert!(ltr_first.x0 < ltr_second.x0);

    ui.set_layout_direction(row, LayoutDirection::Rtl);
    ui.compute_layout(Size::new(200.0, 40.0));

    let rtl_first = ui.effective_bounds(first);
    let rtl_second = ui.effective_bounds(second);
    assert!(rtl_first.x0 > rtl_second.x0);
    assert_eq!(ui.focused(), Some(first));
    assert!(ui.get(first).is_some());
    assert!(ui.get(second).is_some());
}

#[test]
fn flex_reverse_composes_with_inline_direction_without_reordering_children() {
    let mut ui = Ui::new();
    let root = ui.root();
    let (row, (first, second)) = ui.children(
        root,
        Panel::new()
            .layout_direction(LayoutDirection::Rtl)
            .layout(Layout::row_reverse(0.0).logical_size(200.0, 40.0)),
        |children| {
            let first = children.add(Panel::new().layout(Layout::new().logical_size(40.0, 20.0)));
            let second = children.add(Panel::new().layout(Layout::new().logical_size(60.0, 20.0)));
            (first, second)
        },
    );
    ui.compute_layout(Size::new(200.0, 40.0));

    assert!(ui.effective_bounds(first).x0 < ui.effective_bounds(second).x0);
    assert_eq!(ui.get(row).unwrap().children(), &[first, second]);
}

#[test]
fn inherited_direction_reaches_text_presentation_and_accessibility() {
    let mut ui = Ui::new();
    let root = ui.root();
    let text = ui.append(
        root,
        Text::new("الواجهة العربية English").text_align(TextAlign::Start),
    );

    ui.compute_layout(Size::new(300.0, 80.0));
    ui.paint(&mut Scene::new());
    let node = ui.box_for(text).expect("text should own a layout node");
    let primitive = ui
        .presentation
        .presentation(node)
        .and_then(|presentation| presentation.plain_text())
        .expect("text should have plain-text presentation");
    assert_eq!(primitive.layout.align, TextAlign::Start);
    assert_eq!(primitive.layout.base_direction, BaseDirection::Ltr);

    ui.set_layout_direction(root, LayoutDirection::Rtl);
    ui.compute_layout(Size::new(300.0, 80.0));
    ui.paint(&mut Scene::new());
    let primitive = ui
        .presentation
        .presentation(node)
        .and_then(|presentation| presentation.plain_text())
        .expect("text presentation should survive the direction change");
    assert_eq!(primitive.layout.align, TextAlign::Start);
    assert_eq!(primitive.layout.base_direction, BaseDirection::Rtl);

    let update = ui.accesskit_tree_update();
    let text_node = update
        .nodes
        .iter()
        .find(|(id, _)| *id == accesskit_node_id(text))
        .map(|(_, node)| node)
        .expect("text should be represented in the accessibility tree");
    assert_eq!(
        text_node.text_direction(),
        Some(AccessTextDirection::RightToLeft)
    );
}

#[test]
fn paragraph_policy_is_independent_from_application_flow() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout_direction(root, LayoutDirection::Rtl);
    let (row, (matching, automatic, identifier)) = ui.children(
        root,
        Panel::row(8.0).layout(Layout::new().logical_size(420.0, 40.0)),
        |children| {
            let matching = children.add(
                Text::new("الواجهة العربية")
                    .layout(Layout::new().logical_size(120.0, 30.0))
                    .paragraph_direction(ParagraphDirection::MatchLayout),
            );
            let automatic = children.add(
                Text::new("123 / 456")
                    .layout(Layout::new().logical_size(120.0, 30.0))
                    .paragraph_direction(ParagraphDirection::Auto),
            );
            let identifier = children.add(
                Text::new("AR-1082-B")
                    .layout(Layout::new().logical_size(120.0, 30.0))
                    .paragraph_direction(ParagraphDirection::Ltr),
            );
            (matching, automatic, identifier)
        },
    );

    ui.compute_layout(Size::new(420.0, 80.0));
    ui.paint(&mut Scene::new());
    assert_eq!(presented_base_direction(&ui, matching), BaseDirection::Rtl);
    assert_eq!(
        presented_base_direction(&ui, automatic),
        BaseDirection::Auto
    );
    assert_eq!(
        presented_base_direction(&ui, identifier),
        BaseDirection::Ltr
    );
    assert!(ui.effective_bounds(matching).x0 > ui.effective_bounds(automatic).x0);
    assert!(ui.effective_bounds(automatic).x0 > ui.effective_bounds(identifier).x0);

    let update = ui.accesskit_tree_update();
    let direction_for = |element| {
        update
            .nodes
            .iter()
            .find(|(id, _)| *id == accesskit_node_id(element))
            .and_then(|(_, node)| node.text_direction())
    };
    assert_eq!(
        direction_for(matching),
        Some(AccessTextDirection::RightToLeft)
    );
    assert_eq!(direction_for(automatic), None);
    assert_eq!(
        direction_for(identifier),
        Some(AccessTextDirection::LeftToRight)
    );

    let before = [
        ui.effective_bounds(matching),
        ui.effective_bounds(automatic),
        ui.effective_bounds(identifier),
    ];
    let property = ui.properties().paragraph_direction;
    ui.set_local(identifier, property, ParagraphDirection::Rtl);
    ui.compute_layout(Size::new(420.0, 80.0));
    ui.paint(&mut Scene::new());

    assert_eq!(
        [
            ui.effective_bounds(matching),
            ui.effective_bounds(automatic),
            ui.effective_bounds(identifier),
        ],
        before,
        "paragraph policy must not change surrounding retained layout"
    );
    assert_eq!(
        presented_base_direction(&ui, identifier),
        BaseDirection::Rtl
    );
    assert_eq!(ui.layout_direction(row), Some(LayoutDirection::Rtl));
    assert!(ui.get(identifier).is_some());
}

#[test]
fn content_presenter_inherits_owner_paragraph_policy() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout_direction(root, LayoutDirection::Rtl);
    let button = ui.append(
        root,
        Button::new("AR-1082-B").paragraph_direction(ParagraphDirection::Ltr),
    );
    let label = ui
        .slot(button, SLOT_BUTTON_CONTENT)
        .expect("button should realize its content presenter");

    ui.compute_layout(Size::new(240.0, 80.0));
    ui.paint(&mut Scene::new());

    assert_eq!(presented_base_direction(&ui, label), BaseDirection::Ltr);
    assert_eq!(ui.layout_direction(button), Some(LayoutDirection::Rtl));
    assert_eq!(ui.layout_direction(label), Some(LayoutDirection::Rtl));
}

#[test]
fn paragraph_policy_participates_in_styles_and_bindings() {
    let mut ui = Ui::new();
    let property = ui.properties().paragraph_direction;
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Sheet,
            SelectorBuilder::new(SelectorStep::type_tag(crate::TYPE_TEXT)).build(),
            StyleBuilder::new()
                .set(property, ParagraphDirection::Rtl)
                .build(),
        )
        .build();
    ui.set_cascade(cascade);

    let source = ui.append(ui.root(), Panel::new());
    let text = ui.append(ui.root(), Text::new("123 / 456"));
    assert_eq!(
        ui.resolve_value_for(ui.get(text).unwrap(), property),
        ParagraphDirection::Rtl
    );

    ui.bind(
        Ui::element_endpoint(source, property),
        Ui::element_endpoint(text, property),
    )
    .expect("paragraph policy binding should register");
    ui.set_local(source, property, ParagraphDirection::Auto);
    ui.flush_bindings();

    assert_eq!(
        ui.resolve_value_for(ui.get(text).unwrap(), property),
        ParagraphDirection::Auto
    );
}

#[test]
fn visual_direction_does_not_reorder_accessibility_children() {
    let mut ui = Ui::new();
    let root = ui.root();
    let (row, (first, second)) = ui.children(
        root,
        Panel::row(0.0)
            .layout_direction(LayoutDirection::Rtl)
            .layout(Layout::new().logical_size(200.0, 40.0)),
        |children| {
            let first = children.add(Panel::new().layout(Layout::new().logical_size(40.0, 20.0)));
            let second = children.add(Panel::new().layout(Layout::new().logical_size(60.0, 20.0)));
            (first, second)
        },
    );
    ui.compute_layout(Size::new(200.0, 40.0));

    assert!(ui.effective_bounds(first).x0 > ui.effective_bounds(second).x0);
    let update = ui.accesskit_tree_update();
    let row_node = update
        .nodes
        .iter()
        .find(|(id, _)| *id == accesskit_node_id(row))
        .map(|(_, node)| node)
        .expect("row should be represented in the accessibility tree");
    assert_eq!(
        row_node.children(),
        &[accesskit_node_id(first), accesskit_node_id(second)]
    );
}

#[test]
fn directional_controls_mirror_geometry_and_inline_arrow_semantics() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(8.0).logical_size(320.0, 180.0));
    let slider = ui.append(root, Slider::new().value(0.25));
    let thumb = ui.slot(slider, SLOT_SLIDER_THUMB).unwrap();
    let segmented = ui.append(
        root,
        SegmentedControl::new()
            .items(["One", "Two", "Three"])
            .selected_index(0),
    );

    ui.compute_layout(Size::new(320.0, 180.0));
    let ltr_thumb = ui.effective_bounds(thumb);
    let segments = ui
        .get(segmented)
        .unwrap()
        .children()
        .iter()
        .copied()
        .filter(|segment| {
            ui.get(*segment)
                .is_some_and(|element| element.templated_parent().is_none())
        })
        .collect::<Vec<_>>();
    assert!(ui.effective_bounds(segments[0]).x0 < ui.effective_bounds(segments[1]).x0);

    ui.set_layout_direction(root, LayoutDirection::Rtl);
    ui.compute_layout(Size::new(320.0, 180.0));
    assert!(ui.effective_bounds(thumb).x0 > ltr_thumb.x0);
    assert!(ui.effective_bounds(segments[0]).x0 > ui.effective_bounds(segments[1]).x0);

    assert!(ui.set_focus(Some(slider)));
    let left = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowLeft), Code::ArrowLeft);
    assert!(ui.dispatch_key(&left).handled);
    assert!(
        ui.resolve_value_for(ui.get(slider).unwrap(), ui.properties().value) > 0.25,
        "ArrowLeft should move toward the RTL inline end and increase the value"
    );

    assert!(ui.set_focus(Some(segmented)));
    assert!(ui.dispatch_key(&left).handled);
    assert_eq!(
        ui.resolve_value_for(ui.get(segmented).unwrap(), ui.properties().selected_index),
        Some(1)
    );
}

#[test]
fn disclosure_chrome_and_keys_follow_runtime_direction() {
    let mut ui = Ui::new();
    let root = ui.root();
    let expander = ui.append(root, Expander::new("Details"));
    let disclosure = ui.slot(expander, SLOT_EXPANDER_DISCLOSURE).unwrap();
    assert!(ui.set_focus(Some(expander)));

    ui.compute_layout(Size::new(240.0, 120.0));
    assert_eq!(
        ui.resolve_value_for(ui.get(disclosure).unwrap(), ui.properties().text),
        "▸"
    );

    ui.set_layout_direction(root, LayoutDirection::Rtl);
    ui.compute_layout(Size::new(240.0, 120.0));
    assert_eq!(
        ui.resolve_value_for(ui.get(disclosure).unwrap(), ui.properties().text),
        "◂"
    );

    let left = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowLeft), Code::ArrowLeft);
    assert!(ui.dispatch_key(&left).handled);
    assert!(ui.resolve_value_for(ui.get(expander).unwrap(), ui.properties().is_expanded));

    let right = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
    assert!(ui.dispatch_key(&right).handled);
    assert!(!ui.resolve_value_for(ui.get(expander).unwrap(), ui.properties().is_expanded));
}

#[test]
fn default_popup_placement_tracks_target_direction_while_open() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(480.0, 240.0));
    let target = ui.append(
        root,
        Panel::new().layout(
            Layout::new()
                .absolute_at(180.0, 30.0)
                .logical_size(100.0, 32.0),
        ),
    );
    ui.compute_layout(Size::new(480.0, 240.0));
    let popup = ui
        .open_popup(target, Popup::new(), |ui, popup| {
            ui.append(
                popup,
                Panel::new().layout(Layout::new().logical_size(140.0, 40.0)),
            );
        })
        .expect("eligible target should open a popup");
    ui.compute_layout(Size::new(480.0, 240.0));
    let target_bounds = ui.effective_bounds(target);
    assert_eq!(ui.effective_bounds(popup).x0, target_bounds.x0);

    ui.set_layout_direction(target, LayoutDirection::Rtl);
    ui.compute_layout(Size::new(480.0, 240.0));

    assert_eq!(ui.effective_bounds(popup).x1, target_bounds.x1);
    assert_eq!(ui.layout_direction(popup), Some(LayoutDirection::Rtl));
    assert!(ui.get(popup).is_some());
}
