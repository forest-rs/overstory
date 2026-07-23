// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn scroll_view_clip_applies_to_child_surfaces() {
    use imaging::record::Command;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(0.0).logical_size(120.0, 80.0));
    let scroll = ui.append(root, crate::ScrollView::new().size(80.0, 32.0));
    let child = ui.insert_child(scroll);
    ui.set_layout(
        child,
        Layout::new().logical_size(80.0, 64.0).flex_shrink(0.0),
    );
    ui.set_local(
        child,
        ui.properties().surface.background,
        Some(Brush::Solid(Color::from_rgba8(0x00, 0x00, 0xff, 0xff))),
    );

    ui.compute_layout(Size::new(120.0, 80.0));
    let mut output = Scene::new();
    ui.paint(&mut output);

    let scroll_layer = ui.composition.layer_for(scroll).unwrap();
    let child_layer = ui.composition.layer_for(child).unwrap();
    assert!(ui.composition_layers().clip(scroll_layer).is_some());
    assert!(ui.composition_layers().content(child_layer).is_some());
    assert!(
        output
            .commands()
            .iter()
            .any(|command| matches!(command, Command::PushClip(_)))
    );
}

#[test]
fn press_marks_templated_control_not_inner_part() {
    // Pressing on a templated control's part (Button's label
    // `Text`) should put `:pressed` on the Button itself, not
    // on the label. Without this, the cascade rule keyed on
    // `TYPE_BUTTON:pressed` never matches and the button shows
    // no press feedback. Use `press_visual_target` directly so
    // the test doesn't depend on layout + hit-test plumbing.
    use crate::{Button, Text};

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, Button::new("Click me"));
    let label = ui.part(button, crate::PART_BUTTON_LABEL).unwrap();
    let retained_label = ui
        .slot_children(button, crate::SLOT_BUTTON_CONTENT, |content| {
            content.add(Text::new("Retained"))
        })
        .expect("button template should expose content slot");

    assert_eq!(ui.press_visual_target(label), button);
    assert_eq!(ui.press_visual_target(retained_label), button);
    assert_eq!(ui.press_visual_target(button), button);
    assert_eq!(ui.press_visual_target(root), root);
}

#[test]
fn disabled_button_carries_state_and_ignores_builder_click() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    use crate::Button;

    let mut ui = Ui::new();
    let root = ui.root();
    let fired = Rc::new(Cell::new(false));
    let fired_for_handler = Rc::clone(&fired);
    let button = ui.append(
        root,
        Button::new("No")
            .disabled()
            .on_activate(move |_ui, _event| fired_for_handler.set(true)),
    );

    assert!(!ui.is_enabled(button));
    assert!(ui.get(button).unwrap().pseudos().contains(&PSEUDO_DISABLED));

    ui.synthesize_click(button, Point::ZERO);
    assert!(
        !fired.get(),
        "disabled button click handler should not fire"
    );

    assert!(!ui.set_focus(Some(button)));
    assert_eq!(ui.focused(), None, "disabled button should reject focus");
}

#[test]
fn set_enabled_false_clears_focus_and_pressed_state() {
    use crate::Button;

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, Button::new("Run"));

    assert!(ui.set_focus(Some(button)));
    ui.set_press_target(Some(button));
    ui.set_pointer_capture(Some(button));
    ui.set_enabled(button, false);

    assert!(!ui.is_enabled(button));
    assert_eq!(ui.focused(), None);
    assert_eq!(ui.events.press_target(), None);
    assert_eq!(ui.pointer_capture(), None);
    let pseudos = ui.get(button).unwrap().pseudos();
    assert!(pseudos.contains(&PSEUDO_DISABLED));
    assert!(!pseudos.contains(&PSEUDO_FOCUSED));
    assert!(!pseudos.contains(&PSEUDO_FOCUS_VISIBLE));
    assert!(!pseudos.contains(&PSEUDO_PRESSED));

    ui.set_enabled(button, true);
    assert!(ui.is_enabled(button));
    assert!(
        ui.is_focusable(button),
        "re-enabling should restore the stored focusable preference"
    );
    assert!(ui.set_focus(Some(button)));
}

#[test]
fn ancestor_visibility_controls_focus_eligibility_and_restores_preference() {
    let mut ui = Ui::new();
    let root = ui.root();
    let container = ui.insert_child(root);
    let button = ui.append(container, crate::Button::new("Run"));
    assert!(ui.set_focus(Some(button)));

    ui.set_visibility(container, crate::Visibility::Hidden);

    assert_eq!(ui.focused(), None);
    assert!(!ui.is_focusable(button));
    assert!(!ui.set_focus(Some(button)));

    ui.set_visibility(container, crate::Visibility::Visible);

    assert!(ui.is_focusable(button));
    assert!(ui.set_focus(Some(button)));
}

#[test]
fn direct_is_checked_write_updates_checked_pseudo() {
    use crate::CheckBox;

    let mut ui = Ui::new();
    let root = ui.root();
    let checkbox = ui.append(root, CheckBox::new("Enabled"));

    ui.set_local(checkbox, ui.properties().is_checked, true);

    assert!(
        ui.get(checkbox)
            .unwrap()
            .pseudos()
            .contains(&PSEUDO_CHECKED)
    );
}

#[test]
fn direct_is_enabled_write_clears_focus_and_pressed_state() {
    use crate::Button;

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, Button::new("Run"));

    assert!(ui.set_focus(Some(button)));
    ui.set_press_target(Some(button));
    ui.set_pointer_capture(Some(button));
    ui.set_local(button, ui.properties().is_enabled, false);

    assert!(!ui.is_enabled(button));
    assert_eq!(ui.focused(), None);
    assert_eq!(ui.events.press_target(), None);
    assert_eq!(ui.pointer_capture(), None);
    let pseudos = ui.get(button).unwrap().pseudos();
    assert!(pseudos.contains(&PSEUDO_DISABLED));
    assert!(!pseudos.contains(&PSEUDO_FOCUSED));
    assert!(!pseudos.contains(&PSEUDO_FOCUS_VISIBLE));
    assert!(!pseudos.contains(&PSEUDO_PRESSED));
}

#[test]
fn disabled_toggle_and_slider_share_control_state_policy() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    use crate::{Slider, ToggleSwitch};

    let mut ui = Ui::new();
    let root = ui.root();

    let toggle = ui.append(root, ToggleSwitch::new().disabled());
    assert!(!ui.is_enabled(toggle));
    assert!(!ui.is_focusable(toggle));
    assert!(ui.get(toggle).unwrap().pseudos().contains(&PSEUDO_DISABLED));
    ui.synthesize_click(toggle, Point::ZERO);
    let checked = ui
        .get(toggle)
        .and_then(|el| el.get_local_value(ui.properties().is_checked).copied())
        .unwrap_or(false);
    assert!(!checked, "disabled toggle should ignore user activation");

    let fired = Rc::new(Cell::new(false));
    let fired_for_handler = Rc::clone(&fired);
    let slider = ui.append(root, Slider::new().disabled());
    ui.events(slider)
        .click(move |_ui, _event| fired_for_handler.set(true));
    assert!(!ui.is_enabled(slider));
    assert!(!ui.is_focusable(slider));
    assert!(ui.get(slider).unwrap().pseudos().contains(&PSEUDO_DISABLED));
    ui.synthesize_click(slider, Point::ZERO);
    assert!(
        !fired.get(),
        "disabled slider should suppress click dispatch"
    );
}

#[test]
fn pointer_leave_clears_hover() {
    // Move events require constructing a PointerUpdate with platform
    // position types; we exercise that end-to-end via the demo. Here we
    // verify the dispatch shape: a Leave event clears whatever was
    // currently hovered.

    let mut ui = Ui::new();
    let root = ui.root();
    // Manually mark something hovered (this is what dispatch_pointer_event
    // does internally via add_pseudo on a Move).
    ui.add_pseudo(root, PSEUDO_HOVER);
    // Match the internal state. (Phase 1d's dispatch is the only writer
    // of `hovered` in real code, so this is a deliberate test-only nudge.)
    // We test the Leave path which clears via the public API.
    // Even if hovered is None (we never set it via dispatch), Leave
    // should be a no-op rather than panic.
    let _ = ui.dispatch_pointer_event(&pointer_leave());
    assert_eq!(ui.hovered(), None);
}

#[test]
fn add_class_restyles_subtree() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.add_class(root, ClassId(1));
    // `add_class` restyles synchronously — the class is on the
    // element afterward and any cascade-driven channel changes
    // are already marked via `restyle_subtree`. With an empty
    // cascade (this test's case) no channels move.
    assert!(ui.get(root).unwrap().classes().contains(&ClassId(1)));
}

#[test]
fn add_and_remove_class_by_name_use_style_vocabulary() {
    let mut ui = Ui::new();
    let root = ui.root();

    ui.add_class(root, ".runtime-class");
    let class = ui
        .style_vocabulary()
        .class_id_by_name(".runtime-class")
        .expect("adding by name should intern the class");
    assert!(ui.get(root).unwrap().classes().contains(&class));

    ui.remove_class(root, ".runtime-class");
    assert!(!ui.get(root).unwrap().classes().contains(&class));
    ui.remove_class(root, ".not-registered");
    assert_eq!(
        ui.style_vocabulary().class_id_by_name(".not-registered"),
        None,
        "removing an unknown name should not intern it",
    );
}

#[test]
fn text_builder_attaches_classes() {
    let mut ui = Ui::new();
    let root = ui.root();
    let text = ui.append(root, crate::Text::new("Styled").class(ClassId(42)));

    assert!(ui.get(text).unwrap().classes().contains(&ClassId(42)));
}

#[test]
fn text_builder_resolves_named_classes() {
    let mut ui = Ui::new();
    let root = ui.root();
    let text = ui.append(root, crate::Text::new("Styled").class(".named-label"));

    let class = ui
        .style_vocabulary()
        .class_id_by_name(".named-label")
        .expect("appending a named class should intern it");
    assert!(ui.get(text).unwrap().classes().contains(&class));
}

#[test]
fn element_spec_ext_applies_shared_settings() {
    use crate::ElementSpecExt;

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(
        root,
        crate::Button::new("Run")
            .class(ClassId(77))
            .layout(Layout::new().logical_size(80.0, 30.0))
            .disabled(),
    );

    assert!(ui.get(button).unwrap().classes().contains(&ClassId(77)));
    assert!(!ui.is_enabled(button));
    assert!(!ui.is_focusable(button));
    assert_eq!(
        ui.effective_layout(button),
        Layout::new()
            .container(crate::LayoutContainer::row())
            .logical_size(80.0, 30.0)
            .align_center()
            .justify_center()
    );
    assert_eq!(
        ui.resolve_value_for(
            ui.get(button).unwrap(),
            ui.properties().surface.padding_widths.top
        ),
        18.0
    );
    assert_eq!(
        ui.resolve_value_for(
            ui.get(button).unwrap(),
            ui.properties().surface.padding_widths.left
        ),
        32.0
    );
}

#[test]
fn inspection_snapshot_reports_effective_property_summaries() {
    use crate::inspection::{
        ElementPropertyInspectionGroup, ElementPropertyInspectionOrigin,
        ElementPropertyInspectionValue,
    };
    use crate::{Language, SurfaceSpecExt, TextSpecExt};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout_direction(root, crate::LayoutDirection::Rtl);
    let background = Color::from_rgba8(0x20, 0x40, 0x80, 0xff);
    let language = Language::parse("AR-arab-sa").unwrap();
    let panel = ui.append(
        root,
        crate::Panel::new()
            .background(Some(background.into()))
            .padding_xy(12.0, 6.0)
            .font_size(18.0)
            .text_language(language)
            .layout(Layout::row(8.0).logical_size(120.0, 64.0))
            .opacity(0.75),
    );

    let snapshot = ui.inspection_snapshot();
    let row = snapshot
        .elements
        .iter()
        .find(|row| row.id == panel)
        .expect("configured panel should be inspectable");

    assert!(
        row.properties.iter().any(|property| {
            property.name == "LayoutDirection"
                && property.group == ElementPropertyInspectionGroup::Layout
                && property.origin == ElementPropertyInspectionOrigin::Inherited
                && property.value == ElementPropertyInspectionValue::Token(String::from("rtl"))
        }),
        "snapshot should expose inherited application direction and provenance"
    );
    assert!(
        row.properties.iter().any(|property| {
            property.name == "ParagraphDirection"
                && property.group == ElementPropertyInspectionGroup::Text
                && matches!(
                    &property.value,
                    ElementPropertyInspectionValue::Record { summary, .. }
                        if summary == "match-layout → rtl"
                )
        }),
        "snapshot should expose both paragraph policy and its resolved direction"
    );
    assert!(
        row.properties.iter().any(|property| {
            property.name == "Background"
                && property.group == ElementPropertyInspectionGroup::Appearance
                && property.value == ElementPropertyInspectionValue::Color(background)
        }),
        "snapshot should expose effective appearance properties"
    );
    assert!(
        row.properties.iter().any(|property| {
            property.name == "Padding left"
                && property.group == ElementPropertyInspectionGroup::Box
                && property.value == ElementPropertyInspectionValue::Number(12.0)
        }),
        "snapshot should expose box model properties separately from appearance"
    );
    assert!(
        row.properties.iter().any(|property| {
            property.name == "FontSize"
                && property.group == ElementPropertyInspectionGroup::Text
                && property.value == ElementPropertyInspectionValue::Number(18.0)
        }),
        "snapshot should expose effective text properties"
    );
    assert!(
        row.properties.iter().any(|property| {
            property.name == "TextLanguage"
                && property.group == ElementPropertyInspectionGroup::Text
                && property.value
                    == ElementPropertyInspectionValue::Token(String::from("ar-Arab-SA"))
        }),
        "snapshot should expose the canonical content-language tag"
    );
    assert!(
        row.properties.iter().any(|property| {
            property.name == "Opacity"
                && property.group == ElementPropertyInspectionGroup::Motion
                && property.value == ElementPropertyInspectionValue::Number(0.75)
        }),
        "snapshot should expose effective motion properties"
    );
    assert!(
        row.properties.iter().any(|property| {
            property.name == "LayoutContainer"
                && property.group == ElementPropertyInspectionGroup::Layout
                && matches!(
                    &property.value,
                    ElementPropertyInspectionValue::Record { summary, fields }
                        if summary == "display flex, direction row"
                            && fields.iter().any(|field| field.name == "Display"
                                && field.value == ElementPropertyInspectionValue::Token(String::from("flex")))
                            && fields.iter().any(|field| field.name == "Direction"
                                && field.value == ElementPropertyInspectionValue::Token(String::from("row")))
                )
        }),
        "snapshot should expose readable container layout properties"
    );
    assert!(
        row.properties.iter().any(|property| {
            property.name == "LayoutSize"
                && property.group == ElementPropertyInspectionGroup::Layout
                && matches!(
                    &property.value,
                    ElementPropertyInspectionValue::Record { summary, fields }
                        if summary == "width 120px, height 64px"
                            && fields.iter().any(|field| field.name == "Width"
                                && field.value == ElementPropertyInspectionValue::Token(String::from("120px")))
                            && fields.iter().any(|field| field.name == "Height"
                                && field.value == ElementPropertyInspectionValue::Token(String::from("64px")))
                )
        }),
        "snapshot should expose readable size layout properties"
    );
    assert!(
        row.properties.iter().any(|property| {
            property.name == "LayoutSpacing"
                && property.group == ElementPropertyInspectionGroup::Layout
                && matches!(
                    &property.value,
                    ElementPropertyInspectionValue::Record { summary, fields }
                        if summary == "gap 8px"
                            && fields.iter().any(|field| field.name == "Gap"
                                && field.value == ElementPropertyInspectionValue::Token(String::from("8px")))
                )
        }),
        "snapshot should expose readable spacing layout properties"
    );
}

#[test]
fn inspection_snapshot_reports_style_vocabulary_names() {
    let mut ui = Ui::new();
    assert_eq!(
        ui.style_vocabulary().resource_name(crate::theme::ACCENT_BG),
        Some("accent.bg")
    );
    let class = ui.class_id(".named-class");
    let pseudo = ui.pseudo_class_id(":named-pseudo");
    let type_tag = ui.type_tag("NamedWidget");
    let resource = ui.resource_key("named.resource");
    let part_tag = ui.part_tag(type_tag, "NamedWidget.Part");

    let snapshot = ui.inspection_snapshot();

    assert_eq!(
        snapshot.style_vocabulary.class_name(class),
        Some(".named-class")
    );
    assert_eq!(
        snapshot.style_vocabulary.pseudo_name(pseudo),
        Some(":named-pseudo")
    );
    assert_eq!(
        snapshot.style_vocabulary.type_name(type_tag),
        Some("NamedWidget")
    );
    assert_eq!(
        snapshot.style_vocabulary.resource_name(resource),
        Some("named.resource")
    );
    assert_eq!(
        snapshot.style_vocabulary.part_name(type_tag, part_tag),
        Some("NamedWidget.Part")
    );
}

#[test]
fn ui_resolves_style_token_sets() {
    struct TestTokens;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct TestClasses {
        class: ClassId,
        resource: ResourceKey,
    }

    impl StyleTokenSet for TestTokens {
        type Resolved = TestClasses;

        fn resolve(vocabulary: &mut StyleVocabulary) -> Self::Resolved {
            TestClasses {
                class: vocabulary.class_id(".ui-token"),
                resource: vocabulary.resource_key("ui.token.resource"),
            }
        }
    }

    let mut ui = Ui::new();

    let tokens = ui.style_tokens::<TestTokens>();

    assert_eq!(
        ui.style_vocabulary().class_name(tokens.class),
        Some(".ui-token")
    );
    assert_eq!(
        ui.style_vocabulary().resource_name(tokens.resource),
        Some("ui.token.resource")
    );
    assert_eq!(ui.style_tokens::<TestTokens>(), tokens);
}

#[test]
fn element_spec_ext_applies_initial_local_properties() {
    use crate::{SurfaceSpecExt, TextSpecExt};

    let mut ui = Ui::new();
    let root = ui.root();
    let background = Brush::from(Color::from_rgba8(0x22, 0x33, 0x44, 0xff));
    let foreground = Brush::from(Color::from_rgba8(0xee, 0xdd, 0xcc, 0xff));
    let corner_radius = crate::CornerRadius::circular(6.0);
    let panel = ui.append(
        root,
        crate::Panel::new()
            .background(background.clone())
            .foreground(foreground.clone())
            .border_width(2.0)
            .corner_radius(6.0)
            .padding_xy(14.0, 8.0)
            .opacity(0.5)
            .content("Configured"),
    );
    let element = ui.get(panel).unwrap();

    assert_eq!(
        element.get_local_value(ui.properties().surface.background),
        Some(&Some(background))
    );
    assert_eq!(
        element.get_local_value(ui.properties().foreground),
        Some(&foreground)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.border_widths.top),
        Some(&2.0)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.border_widths.right),
        Some(&2.0)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.border_styles.top),
        Some(&crate::BorderStyle::Solid)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.corner_radii.top_left),
        Some(&corner_radius)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.corner_radii.bottom_right),
        Some(&corner_radius)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.padding_widths.top),
        Some(&8.0)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.padding_widths.left),
        Some(&14.0)
    );
    assert_eq!(element.get_local_value(ui.properties().opacity), Some(&0.5));
    assert_eq!(
        element.get_local_value(ui.properties().content),
        Some(&String::from("Configured"))
    );
}

#[test]
fn surface_border_style_shorthand_writes_surface_border_styles() {
    use crate::SurfaceSpecExt;

    let mut ui = Ui::new();
    let root = ui.root();
    let panel = ui.append(
        root,
        crate::Panel::new()
            .border_style(crate::BorderStyle::Dashed)
            .border_width(2.0)
            .border_brush(Brush::from(Color::from_rgba8(0x22, 0x33, 0x44, 0xff))),
    );
    let element = ui.get(panel).unwrap();

    assert_eq!(
        element.get_local_value(ui.properties().surface.border_styles.top),
        Some(&crate::BorderStyle::Dashed)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.border_styles.bottom),
        Some(&crate::BorderStyle::Dashed)
    );

    let panel = ui.append(
        root,
        crate::Panel::new().border_styles(understory_presentation::Edges::new(
            crate::BorderStyle::Dotted,
            crate::BorderStyle::Double,
            crate::BorderStyle::Groove,
            crate::BorderStyle::Ridge,
        )),
    );
    let element = ui.get(panel).unwrap();

    assert_eq!(
        element.get_local_value(ui.properties().surface.border_styles.top),
        Some(&crate::BorderStyle::Dotted)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.border_styles.right),
        Some(&crate::BorderStyle::Double)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.border_styles.bottom),
        Some(&crate::BorderStyle::Groove)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.border_styles.left),
        Some(&crate::BorderStyle::Ridge)
    );
}

#[test]
fn surface_padding_shorthand_writes_surface_padding() {
    let mut ui = Ui::new();
    let root = ui.root();
    let panel = ui.append(
        root,
        crate::Panel::new()
            .padding_xy(12.0, 8.0)
            .layout(Layout::row(4.0)),
    );
    let element = ui.get(panel).unwrap();

    assert_eq!(
        element.get_local_value(ui.properties().surface.padding_widths.top),
        Some(&8.0)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.padding_widths.right),
        Some(&12.0)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.padding_widths.bottom),
        Some(&8.0)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.padding_widths.left),
        Some(&12.0)
    );
    assert_eq!(
        element.get_local_value(ui.properties().layout_spacing),
        Some(&crate::LayoutSpacing::gap(4.0))
    );
}

#[test]
fn element_spec_ext_applies_text_and_range_property_helpers() {
    use crate::{RangeSpecExt, TextSpecExt};

    let mut ui = Ui::new();
    let root = ui.root();
    let feature = parley::FontFeature::parse_css_list("\"tnum\" on")
        .next()
        .expect("feature should parse")
        .expect("feature should be valid");
    let text = ui.append(
        root,
        crate::Text::new("0000")
            .text("1234")
            .font_features(Box::from([feature]))
            .letter_spacing(1.25)
            .text_wrap_mode(crate::TextWrapMode::Wrap),
    );
    let slider = ui.append(
        root,
        crate::Panel::new()
            .range(-1.0, 1.0)
            .value(0.25)
            .small_change(0.1)
            .large_change(0.5),
    );

    assert_eq!(
        ui.get(text).unwrap().get_local_value(ui.properties().text),
        Some(&String::from("1234"))
    );
    assert_eq!(
        ui.get(text)
            .unwrap()
            .get_local_value(ui.properties().font_features)
            .map(|features| features.as_ref()),
        Some(&[feature][..])
    );
    assert_eq!(
        ui.get(text)
            .unwrap()
            .get_local_value(ui.properties().letter_spacing),
        Some(&1.25)
    );
    assert_eq!(
        ui.get(text)
            .unwrap()
            .get_local_value(ui.properties().text_wrap_mode),
        Some(&crate::TextWrapMode::Wrap)
    );
    assert_eq!(
        ui.get(slider)
            .unwrap()
            .get_local_value(ui.properties().minimum),
        Some(&-1.0)
    );
    assert_eq!(
        ui.get(slider)
            .unwrap()
            .get_local_value(ui.properties().maximum),
        Some(&1.0)
    );
    assert_eq!(
        ui.get(slider)
            .unwrap()
            .get_local_value(ui.properties().value),
        Some(&0.25)
    );
    assert_eq!(
        ui.get(slider)
            .unwrap()
            .get_local_value(ui.properties().small_change),
        Some(&0.1)
    );
    assert_eq!(
        ui.get(slider)
            .unwrap()
            .get_local_value(ui.properties().large_change),
        Some(&0.5)
    );
}

#[test]
fn element_spec_ext_local_property_accepts_registered_property_handles() {
    use crate::ElementSpecExt;

    let mut ui = Ui::new();
    let root = ui.root();
    let letter_spacing = ui.properties().letter_spacing;
    let text = ui.append(root, crate::Text::new("Tracked").local(letter_spacing, 2.5));

    assert_eq!(
        ui.get(text)
            .unwrap()
            .get_local_value(ui.properties().letter_spacing),
        Some(&2.5)
    );
}

#[test]
fn element_spec_ext_replaces_repeated_initial_property_writes() {
    use crate::SurfaceSpecExt;

    let mut ui = Ui::new();
    let root = ui.root();
    let panel = ui.append(root, crate::Panel::new().opacity(0.25).opacity(0.75));

    assert_eq!(
        ui.get(panel)
            .unwrap()
            .get_local_value(ui.properties().opacity),
        Some(&0.75)
    );
}

#[test]
fn more_control_specs_share_element_settings() {
    use crate::ElementSpecExt;

    let mut ui = Ui::new();
    let root = ui.root();
    let class = ClassId(88);
    let layout_size = crate::LayoutSize::logical(72.0, 24.0);
    let layout = Layout::new().size(layout_size);

    let divider = ui.append(
        root,
        crate::Divider::horizontal()
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );
    let spinner = ui.append(
        root,
        crate::Spinner::new()
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );
    let viewport = ui.append(
        root,
        crate::Viewport::new()
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );
    let splitter = ui.append(
        root,
        crate::Splitter::vertical()
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );
    let list = ui.append(
        root,
        crate::ListBox::new()
            .items(["A", "B"])
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );
    let select = ui.append(
        root,
        crate::Select::new()
            .items(["A", "B"])
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );
    let outline = ui.append(
        root,
        crate::OutlineView::new()
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );
    let scroll_view = ui.append(
        root,
        crate::ScrollView::new()
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );
    let scroll_area = ui.append(
        root,
        crate::ScrollArea::new()
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );
    let scroll_bar = ui.append(
        root,
        crate::ScrollBar::vertical(scroll_view)
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );
    let popup = ui.append(
        root,
        crate::Popup::new()
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );
    let tooltip = ui.append(
        root,
        crate::Tooltip::new("Details")
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );

    for id in [
        divider,
        spinner,
        viewport,
        splitter,
        list,
        select,
        outline,
        scroll_view,
        scroll_area,
        scroll_bar,
        popup,
        tooltip,
    ] {
        assert!(ui.get(id).unwrap().classes().contains(&class));
        assert!(!ui.is_enabled(id));
        assert_eq!(
            ui.get(id)
                .unwrap()
                .get_local_value(ui.properties().layout_size),
            Some(&layout_size)
        );
    }
}
