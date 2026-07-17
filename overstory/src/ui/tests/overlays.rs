// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn tooltip_shows_as_overlay_for_hovered_target() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().logical_size(220.0, 120.0));
    let target = ui.insert_child(root);
    ui.set_layout(target, Layout::new().logical_size(80.0, 30.0));
    ui.set_tooltip(target, crate::Tooltip::new("Deploys the service"));

    ui.compute_layout(Size::new(220.0, 120.0));
    assert_eq!(ui.overlays.active_tooltip(), None);

    let result = ui.dispatch_pointer_event(&pointer_move(Point::new(20.0, 12.0), 1, false));
    assert!(result.changed());
    let active = ui
        .overlays
        .active_tooltip()
        .expect("hovering a tooltip target should create an overlay tooltip");
    assert_eq!(active.target, target);
    assert_eq!(ui.get(active.element).unwrap().parent(), Some(root));

    ui.compute_layout(Size::new(220.0, 120.0));
    let target_bounds = ui.effective_bounds(target);
    let tooltip_bounds = ui.effective_bounds(active.element);
    assert_eq!(
        tooltip_bounds.origin(),
        Point::new(target_bounds.x0, target_bounds.y1 + 8.0)
    );
    assert!(tooltip_bounds.width() > 0.0);
    assert!(tooltip_bounds.height() > 0.0);

    let overlay_root = ui
        .composition
        .overlay_root()
        .expect("tooltip should create an overlay composition root");
    let tooltip_layer = ui.composition.layer_for(active.element).unwrap();
    assert_eq!(
        ui.composition_layers().parent(tooltip_layer),
        Some(overlay_root)
    );
    assert_ne!(ui.hit_test(tooltip_bounds.center()), Some(active.element));

    let leave = ui.dispatch_pointer_event(&pointer_move(Point::new(180.0, 100.0), 2, false));
    assert!(leave.changed());
    assert_eq!(ui.overlays.active_tooltip(), None);
    assert!(ui.get(active.element).is_none());
}

#[test]
fn tooltip_padding_writes_surface_padding_not_layout_padding() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().logical_size(220.0, 120.0));
    let target = ui.insert_child(root);
    ui.set_layout(target, Layout::new().logical_size(80.0, 30.0));
    ui.set_tooltip(target, crate::Tooltip::new("Padded").padding(13.0, 7.0));

    ui.compute_layout(Size::new(220.0, 120.0));
    let _ = ui.dispatch_pointer_event(&pointer_move(Point::new(20.0, 12.0), 1, false));
    let active = ui
        .overlays
        .active_tooltip()
        .expect("hovering a tooltip target should create an overlay tooltip");

    assert_surface_padding(&ui, active.element, 7.0, 13.0, 7.0, 13.0);
    assert_layout_spacing_has_no_padding(&ui, active.element);
}

#[test]
fn default_cascade_styles_tooltip_surface_and_label() {
    use crate::{SLOT_TOOLTIP_CONTENT, Tooltip, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::new().logical_size(220.0, 120.0));
    let target = ui.insert_child(root);
    ui.set_layout(target, Layout::new().logical_size(80.0, 30.0));
    ui.set_tooltip(target, Tooltip::new("Deploys the service"));

    ui.compute_layout(Size::new(220.0, 120.0));
    let result = ui.dispatch_pointer_event(&pointer_move(Point::new(20.0, 12.0), 1, false));
    assert!(result.changed());
    let active = ui
        .overlays
        .active_tooltip()
        .expect("hovering a tooltip target should create an overlay tooltip");

    ui.compute_layout(Size::new(220.0, 120.0));
    let mut output = Scene::new();
    ui.paint(&mut output);

    let tooltip_node = ui
        .box_for(active.element)
        .expect("tooltip should have a box node");
    let surface = ui
        .presentation
        .presentation(tooltip_node)
        .expect("tooltip should have presentation")
        .surface()
        .expect("tooltip should present a surface");
    assert!(
        !surface.backgrounds.is_empty(),
        "default cascade should give tooltips an opaque background"
    );
    assert!(
        !surface.border.is_empty(),
        "default cascade should give tooltips a border"
    );

    let label = ui
        .slot(active.element, SLOT_TOOLTIP_CONTENT)
        .expect("tooltip should create a content label");
    let label_node = ui.box_for(label).expect("label should have a box node");
    let foreground = ui
        .presentation
        .presentation(label_node)
        .expect("label should have presentation")
        .plain_text()
        .expect("label should present text")
        .foreground
        .clone();
    assert_eq!(
        foreground,
        Some(Brush::Solid(Color::from_rgba8(0xf4, 0xf4, 0xf7, 0xff))),
        "tooltip label should inherit the styled tooltip foreground"
    );
}

#[test]
fn tooltip_attached_to_owner_activates_from_template_part_hover() {
    let mut ui = Ui::new();
    ui.set_cascade(crate::theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::new().logical_size(220.0, 120.0));
    let button = ui.append(root, crate::Button::new("Deploy").font_size(12.0));
    ui.set_layout(button, Layout::new().logical_size(92.0, 36.0));
    ui.set_tooltip(button, crate::Tooltip::new("Runs the deploy workflow"));

    ui.compute_layout(Size::new(220.0, 120.0));
    let center = ui.effective_bounds(button).center();
    let _ = ui.dispatch_pointer_event(&pointer_move(center, 1, false));

    assert_eq!(
        ui.overlays.active_tooltip().map(|active| active.target),
        Some(button),
        "tooltip lookup should walk from hovered template parts to the owning control"
    );
}

#[test]
fn tooltip_follows_keyboard_focus_and_clears_with_focus() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().logical_size(220.0, 120.0));
    let target = ui.insert_child(root);
    ui.set_layout(target, Layout::new().logical_size(80.0, 30.0));
    ui.set_focusable(target, true);
    ui.set_tooltip(target, crate::Tooltip::new("Focusable helper"));

    ui.compute_layout(Size::new(220.0, 120.0));
    assert!(ui.set_focus(Some(target)));
    let active = ui
        .overlays
        .active_tooltip()
        .expect("focused tooltip target should create an overlay tooltip");
    assert_eq!(active.target, target);

    assert!(ui.set_focus(None));
    assert_eq!(ui.overlays.active_tooltip(), None);
    assert!(ui.get(active.element).is_none());
}

#[test]
fn collapsing_tooltip_anchor_ancestor_closes_hover_tooltip() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().logical_size(220.0, 120.0));
    let container = ui.insert_child(root);
    let target = ui.insert_child(container);
    ui.set_layout(target, Layout::new().logical_size(80.0, 30.0));
    ui.set_tooltip(target, crate::Tooltip::new("Anchored helper"));

    ui.compute_layout(Size::new(220.0, 120.0));
    let _ = ui.dispatch_pointer_event(&pointer_move(Point::new(20.0, 12.0), 1, false));
    let active = ui
        .overlays
        .active_tooltip()
        .expect("hovering the target should open its tooltip");

    ui.set_visibility(container, crate::Visibility::Collapsed);

    assert_eq!(ui.overlays.active_tooltip(), None);
    assert!(ui.get(active.element).is_none());
}

#[test]
fn pointer_leave_does_not_restore_previous_focused_tooltip() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::row(20.0).logical_size(240.0, 120.0));
    let first = ui.insert_child(root);
    ui.set_layout(first, Layout::new().logical_size(70.0, 30.0));
    ui.set_focusable(first, true);
    ui.set_tooltip(first, crate::Tooltip::new("First"));
    let second = ui.insert_child(root);
    ui.set_layout(second, Layout::new().logical_size(70.0, 30.0));
    ui.set_tooltip(second, crate::Tooltip::new("Second"));

    ui.compute_layout(Size::new(240.0, 120.0));
    assert!(ui.set_focus(Some(first)));
    assert_eq!(
        ui.overlays.active_tooltip().map(|active| active.target),
        Some(first)
    );

    let second_center = ui.effective_bounds(second).center();
    let _ = ui.dispatch_pointer_event(&pointer_move(second_center, 1, false));
    assert_eq!(
        ui.overlays.active_tooltip().map(|active| active.target),
        Some(second)
    );

    let _ = ui.dispatch_pointer_event(&pointer_move(Point::new(220.0, 90.0), 2, false));
    assert_eq!(
        ui.overlays.active_tooltip(),
        None,
        "leaving a hover tooltip should hide it instead of restoring a focused target"
    );
}

#[test]
fn tooltip_placement_flips_when_bottom_would_overflow() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(220.0, 90.0));
    let target = ui.insert_child(root);
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(10.0, 50.0)
            .logical_size(70.0, 30.0),
    );
    ui.set_tooltip(target, crate::Tooltip::new("Near the bottom edge"));

    ui.compute_layout(Size::new(220.0, 90.0));
    let _ = ui.dispatch_pointer_event(&pointer_move(
        ui.effective_bounds(target).center(),
        1,
        false,
    ));
    ui.compute_layout(Size::new(220.0, 90.0));

    let active = ui.overlays.active_tooltip().unwrap();
    let target_bounds = ui.effective_bounds(target);
    let tooltip_bounds = ui.effective_bounds(active.element);
    assert!(
        tooltip_bounds.y1 <= target_bounds.y0,
        "bottom placement should flip above the target when it would overflow"
    );
}

#[test]
fn tooltip_can_be_placed_to_the_right_of_target() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(260.0, 100.0));
    let target = ui.insert_child(root);
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(20.0, 20.0)
            .logical_size(60.0, 24.0),
    );
    ui.set_tooltip(
        target,
        crate::Tooltip::new("Side")
            .placement(crate::Placement::RIGHT_START)
            .offset(6.0, 0.0),
    );

    ui.compute_layout(Size::new(260.0, 100.0));
    let _ = ui.dispatch_pointer_event(&pointer_move(
        ui.effective_bounds(target).center(),
        1,
        false,
    ));
    ui.compute_layout(Size::new(260.0, 100.0));

    let active = ui.overlays.active_tooltip().unwrap();
    let target_bounds = ui.effective_bounds(target);
    let tooltip_bounds = ui.effective_bounds(active.element);
    assert_eq!(
        tooltip_bounds.origin(),
        Point::new(target_bounds.x1 + 6.0, target_bounds.y0)
    );
}

#[test]
fn popup_opens_as_overlay_container_with_content() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(260.0, 120.0));
    let target = ui.insert_child(root);
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(20.0, 20.0)
            .logical_size(80.0, 28.0),
    );
    ui.compute_layout(Size::new(260.0, 120.0));

    let popup = ui
        .open_popup(target, crate::Popup::new(), |ui, popup| {
            ui.append(popup, crate::Text::new("Popup content").font_size(12.0));
        })
        .expect("live target should open popup");
    ui.compute_layout(Size::new(260.0, 120.0));

    assert_eq!(ui.get(popup).unwrap().parent(), Some(root));
    assert_eq!(ui.get(popup).unwrap().children().len(), 1);
    let target_bounds = ui.effective_bounds(target);
    let popup_bounds = ui.effective_bounds(popup);
    assert_eq!(
        popup_bounds.origin(),
        Point::new(target_bounds.x0, target_bounds.y1 + 6.0)
    );

    let overlay_root = ui.composition.overlay_root().unwrap();
    let popup_layer = ui.composition.layer_for(popup).unwrap();
    assert_eq!(
        ui.composition_layers().parent(popup_layer),
        Some(overlay_root)
    );
}

#[test]
fn menu_popup_surface_owns_chrome_bounds_indicators_and_input() {
    use crate::{
        Menu, MenuButton, MenuItem, SLOT_MENU_ITEM_INDICATOR, SLOT_MENU_ITEM_LABEL, theme,
    };
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};
    use understory_overlay::OverlayBehavior;

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(640.0, 420.0));
    let underlay = ui.insert_child(root);
    ui.set_layout(
        underlay,
        Layout::new()
            .absolute_at(0.0, 40.0)
            .logical_size(300.0, 180.0),
    );
    let mut descendant = underlay;
    for _ in 0..8 {
        descendant = ui.insert_child(descendant);
        ui.set_layout(descendant, Layout::new().logical_size(300.0, 180.0));
    }
    let button = ui.append(
        root,
        MenuButton::new(
            "Actions",
            Menu::new()
                .item(MenuItem::new(
                    "Copy status: Selected renderer queue item with a deliberately long name",
                ))
                .item(MenuItem::submenu(
                    "Set priority",
                    Menu::new().item(MenuItem::new("High")),
                )),
        ),
    );
    ui.set_layout(
        button,
        Layout::new()
            .absolute_at(0.0, 0.0)
            .logical_size(120.0, 36.0),
    );
    ui.compute_layout(Size::new(640.0, 420.0));
    assert!(ui.set_focus(Some(button)));
    let open = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowDown), Code::ArrowDown);
    assert!(ui.dispatch_key(&open).handled);
    ui.compute_layout(Size::new(640.0, 420.0));

    let popup = ui
        .active_overlay_for(button, OverlayBehavior::Menu)
        .expect("menu button should open a popup");
    let menu = ui
        .active_overlay_content_for(button, OverlayBehavior::Menu)
        .expect("menu popup should expose its semantic surface");
    let items = ui
        .get(menu)
        .expect("menu surface should be live")
        .children()
        .to_vec();
    let submenu = items[1];
    let long_label = ui
        .slot(items[0], SLOT_MENU_ITEM_LABEL)
        .expect("menu item should realize its label slot");
    let indicator = ui
        .slot(submenu, SLOT_MENU_ITEM_INDICATOR)
        .expect("submenu item should realize its indicator slot");

    assert!(
        ui.effective_bounds(menu).width() > 220.0,
        "a menu without an explicit width should grow to preserve its labels"
    );
    assert!(ui.effective_bounds(long_label).width() > 220.0);
    assert!(ui.effective_bounds(long_label).x1 <= ui.effective_bounds(items[0]).x1);
    assert!(ui.effective_bounds(items[0]).x1 <= ui.effective_bounds(menu).x1);
    assert!(
        ui.effective_bounds(indicator).width() > 0.0,
        "indicator={:?}, submenu={:?}, layout={:?}, text={:?}",
        ui.effective_bounds(indicator),
        ui.effective_bounds(submenu),
        ui.effective_layout(indicator),
        ui.get(indicator)
            .and_then(|element| element.get_local_value(ui.properties().text)),
    );
    assert!(ui.effective_bounds(indicator).x1 <= ui.effective_bounds(submenu).x1);
    assert_eq!(
        ui.get(indicator)
            .and_then(|element| element.get_local_value(ui.properties().text))
            .map(String::as_str),
        Some("›")
    );

    let submenu_point = Point::new(
        ui.effective_bounds(submenu).x0 + 8.0,
        ui.effective_bounds(submenu).center().y,
    );
    assert!(ui.effective_bounds(underlay).contains(submenu_point));
    let _ = ui.dispatch_pointer_event(&pointer_move(submenu_point, 2, false));
    assert_eq!(
        ui.focused(),
        Some(submenu),
        "overlay input should win over deeper retained content beneath it"
    );
    let open_child = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
    assert!(ui.dispatch_key(&open_child).handled);
    ui.compute_layout(Size::new(640.0, 420.0));
    assert_ne!(ui.focused(), Some(submenu));
    assert!(
        ui.get(submenu)
            .unwrap()
            .pseudos()
            .contains(&crate::pseudos::PSEUDO_EXPANDED)
    );
    assert_ne!(
        ui.resolve_value_for(ui.get(submenu).unwrap(), ui.properties().surface.background),
        ui.resolve_value_for(
            ui.get(items[0]).unwrap(),
            ui.properties().surface.background
        ),
        "an expanded submenu ancestor should retain the active visual trail"
    );

    let item_layer = ui
        .composition
        .layer_for(items[0])
        .expect("menu item should own a composition layer");
    assert!(ui.composition_layers().clip(item_layer).is_some());

    let mut output = Scene::new();
    ui.paint(&mut output);
    let popup_surface = ui
        .presentation
        .presentation(ui.box_for(popup).expect("popup should own a box"))
        .and_then(|presentation| presentation.surface())
        .expect("popup host should have a surface record");
    assert!(popup_surface.backgrounds.is_empty());
    assert!(popup_surface.border.is_empty());
    let menu_surface = ui
        .presentation
        .presentation(ui.box_for(menu).expect("menu should own a box"))
        .and_then(|presentation| presentation.surface())
        .expect("menu should have a surface record");
    assert!(!menu_surface.backgrounds.is_empty());
    assert!(!menu_surface.border.is_empty());
}

#[test]
fn tooltip_inside_an_active_popup_remains_available() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(320.0, 180.0));
    let target = ui.insert_child(root);
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(20.0, 20.0)
            .logical_size(80.0, 28.0),
    );
    ui.compute_layout(Size::new(320.0, 180.0));

    let mut content = None;
    let _popup = ui
        .open_popup(target, crate::Popup::new(), |ui, popup| {
            let child = ui.insert_child(popup);
            ui.set_layout(child, Layout::new().logical_size(100.0, 32.0));
            ui.set_tooltip(child, crate::Tooltip::new("Popup help"));
            content = Some(child);
        })
        .expect("live target should open popup");
    ui.compute_layout(Size::new(320.0, 180.0));
    let content = content.expect("popup builder should create tooltip content");

    let _ = ui.dispatch_pointer_event(&pointer_move(
        ui.effective_bounds(content).center(),
        1,
        false,
    ));

    assert_eq!(
        ui.overlays.active_tooltip().map(|active| active.target),
        Some(content)
    );
}

#[test]
fn popup_padding_writes_surface_padding_not_layout_padding() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(260.0, 120.0));
    let target = ui.insert_child(root);
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(20.0, 20.0)
            .logical_size(80.0, 28.0),
    );
    ui.compute_layout(Size::new(260.0, 120.0));

    let popup = ui
        .open_popup(
            target,
            crate::Popup::new().padding(17.0, 11.0),
            |ui, popup| {
                ui.append(popup, crate::Text::new("Popup content").font_size(12.0));
            },
        )
        .expect("live target should open popup");

    assert_surface_padding(&ui, popup, 11.0, 17.0, 11.0, 17.0);
    assert_layout_spacing_has_no_padding(&ui, popup);
}

#[test]
fn popup_repositions_with_target_and_closes_by_root_element() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(260.0, 140.0));
    let target = ui.insert_child(root);
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(20.0, 20.0)
            .logical_size(70.0, 24.0),
    );
    ui.compute_layout(Size::new(260.0, 140.0));

    let popup = ui
        .open_popup(target, crate::Popup::new(), |ui, popup| {
            ui.append(popup, crate::Text::new("Popup").font_size(12.0));
        })
        .unwrap();
    ui.compute_layout(Size::new(260.0, 140.0));
    let original = ui.effective_bounds(popup).origin();

    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(120.0, 40.0)
            .logical_size(70.0, 24.0),
    );
    ui.compute_layout(Size::new(260.0, 140.0));
    let moved = ui.effective_bounds(popup).origin();
    assert_ne!(moved, original);
    assert_eq!(moved, Point::new(120.0, 70.0));

    assert!(ui.close_popup(popup));
    assert!(ui.get(popup).is_none());
    assert!(!ui.close_popup(popup));
}

#[test]
fn popup_placement_keeps_a_viable_incumbent_near_a_viewport_edge() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(200.0, 100.0));
    let target = ui.insert_child(root);
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(120.0, 65.0)
            .logical_size(60.0, 20.0),
    );
    ui.compute_layout(Size::new(200.0, 100.0));

    let popup = ui
        .open_popup(target, crate::Popup::new(), |ui, popup| {
            ui.set_layout(popup, Layout::new().logical_size(80.0, 30.0));
        })
        .unwrap();
    ui.compute_layout(Size::new(200.0, 100.0));
    let first_popup = ui.effective_bounds(popup);
    let first_target = ui.effective_bounds(target);
    assert!(
        first_popup.y1 <= first_target.y0,
        "the first measured placement should flip above the crowded anchor: popup={first_popup:?}, target={first_target:?}"
    );

    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(120.0, 55.0)
            .logical_size(60.0, 20.0),
    );
    ui.compute_layout(Size::new(200.0, 100.0));

    assert!(
        ui.effective_bounds(popup).y1 <= ui.effective_bounds(target).y0,
        "a small geometry change should retain the viable previous placement"
    );

    ui.compute_layout(Size::new(160.0, 100.0));
    let resized = ui.effective_bounds(popup);
    assert!(resized.x0 >= 0.0 && resized.x1 <= 160.0);
}

#[test]
fn nested_popups_reposition_with_arrange_only_anchor_movement() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().logical_size(200.0, 120.0));
    let scroll = ui.append(root, crate::ScrollView::new().size(120.0, 48.0));
    let spacer = ui.insert_child(scroll);
    ui.set_layout(
        spacer,
        Layout::new().logical_size(120.0, 60.0).flex_shrink(0.0),
    );
    let target = ui.insert_child(scroll);
    ui.set_layout(
        target,
        Layout::new().logical_size(80.0, 24.0).flex_shrink(0.0),
    );
    ui.compute_layout(Size::new(200.0, 120.0));

    let mut nested_target = None;
    let popup = ui
        .open_popup(target, crate::Popup::new(), |ui, popup| {
            ui.set_layout(popup, Layout::new().logical_size(90.0, 30.0));
            let target = ui.insert_child(popup);
            ui.set_layout(target, Layout::new().logical_size(40.0, 16.0));
            nested_target = Some(target);
        })
        .unwrap();
    ui.compute_layout(Size::new(200.0, 120.0));
    let nested_target = nested_target.unwrap();
    let nested = ui
        .open_semantic_popup_with_behavior(
            nested_target,
            Some(popup),
            crate::Popup::new()
                .placement(crate::Placement::RIGHT_START)
                .offset(4.0, 0.0),
            understory_overlay::OverlayBehavior::Submenu,
            |ui, popup| {
                ui.set_layout(popup, Layout::new().logical_size(50.0, 20.0));
                popup
            },
        )
        .unwrap();
    ui.compute_layout(Size::new(200.0, 120.0));
    let moving = [target, popup, nested_target, nested];
    let before = moving.map(|element| ui.effective_bounds(element));
    assert!(
        before[0].y0 >= ui.effective_bounds(scroll).y1,
        "ancestor clipping does not implicitly close an explicitly opened overlay"
    );
    ui.reset_frame_stats();

    assert!(crate::set_scroll_view_offset_y(&mut ui, scroll, 12.0));
    assert!(!ui.invalidation().has_invalidated(LAYOUT));
    ui.compute_layout(Size::new(200.0, 120.0));

    assert_eq!(ui.frame_stats().layout_passes, 0);
    for (element, before) in moving.into_iter().zip(before) {
        let after = ui.effective_bounds(element);
        assert!((after.y0 - (before.y0 - 12.0)).abs() <= 0.01);
    }
    let after_nested = ui.effective_bounds(nested);
    assert_eq!(layer_origin(&ui, nested), after_nested.origin());
    assert_eq!(ui.hit_test(after_nested.center()), Some(nested));
    assert_eq!(
        ui.inspection_snapshot()
            .overlays
            .entries
            .iter()
            .find(|entry| entry.element == nested)
            .unwrap()
            .rect,
        after_nested
    );
}

#[test]
fn escape_closes_topmost_popup_and_restores_focus() {
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(260.0, 140.0));
    let target = ui.append(root, crate::Button::new("Open"));
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(20.0, 20.0)
            .logical_size(70.0, 28.0),
    );
    ui.compute_layout(Size::new(260.0, 140.0));
    assert!(ui.set_focus(Some(target)));

    let mut focused_inside = None;
    let popup = ui
        .open_popup(target, crate::Popup::new(), |ui, popup| {
            let child = ui.insert_child(popup);
            ui.set_layout(child, Layout::new().logical_size(80.0, 24.0));
            ui.set_focusable(child, true);
            focused_inside = Some(child);
        })
        .unwrap();
    ui.compute_layout(Size::new(260.0, 140.0));
    assert!(ui.set_focus(focused_inside));

    let escape = KeyboardEvent::key_down(Key::Named(NamedKey::Escape), Code::Escape);
    let result = ui.dispatch_key(&escape);

    assert!(result.handled);
    assert!(result.focus_changed);
    assert_eq!(result.focused, Some(target));
    assert!(ui.get(popup).is_none());
}

#[test]
fn focus_outside_closes_popup_without_stealing_new_focus() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(320.0, 140.0));
    let target = ui.append(root, crate::Button::new("Open"));
    let outside = ui.append(root, crate::Button::new("Other"));
    ui.compute_layout(Size::new(320.0, 140.0));
    assert!(ui.set_focus(Some(target)));

    let mut focused_inside = None;
    let popup = ui
        .open_popup(target, crate::Popup::new(), |ui, popup| {
            let child = ui.insert_child(popup);
            ui.set_focusable(child, true);
            focused_inside = Some(child);
        })
        .unwrap();
    assert!(ui.set_focus(focused_inside));
    assert!(
        ui.get(popup).is_some(),
        "focus inside should keep the popup open"
    );

    assert!(ui.set_focus(Some(outside)));

    assert!(ui.get(popup).is_none());
    assert_eq!(ui.focused(), Some(outside));
}

#[test]
fn overlay_resolution_observes_focus_after_routed_handlers_settle() {
    let mut ui = Ui::new();
    let root = ui.root();
    let target = ui.append(root, crate::Button::new("Open"));
    let redirect = ui.append(root, crate::Button::new("Redirect"));
    ui.compute_layout(Size::new(320.0, 140.0));
    assert!(ui.set_focus(Some(target)));
    let popup = ui
        .open_popup(target, crate::Popup::new(), |ui, popup| {
            ui.append(popup, crate::Text::new("Popup"));
        })
        .unwrap();

    ui.events(redirect).focus(move |ui, event| {
        if event.phase().is_target() && event.kind == RoutedEventKind::FocusGained {
            let _ = ui.set_focus(Some(target));
        }
    });

    assert!(ui.set_focus(Some(redirect)));

    assert_eq!(ui.focused(), Some(target));
    assert!(ui.get(popup).is_some());
}

#[test]
fn outside_pointer_down_closes_popup_without_consuming_click() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(320.0, 120.0));
    let target = ui.append(root, crate::Button::new("Open"));
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(20.0, 20.0)
            .logical_size(70.0, 28.0),
    );
    let outside = ui.append(root, crate::Button::new("Other"));
    ui.set_layout(
        outside,
        Layout::new()
            .absolute_at(210.0, 20.0)
            .logical_size(80.0, 28.0),
    );
    let clicked = Rc::new(Cell::new(false));
    let clicked_for_handler = Rc::clone(&clicked);
    ui.events(outside).click(move |_ui, _event| {
        clicked_for_handler.set(true);
    });
    ui.compute_layout(Size::new(320.0, 120.0));

    let popup = ui
        .open_popup(target, crate::Popup::new(), |ui, popup| {
            ui.append(popup, crate::Text::new("Popup").font_size(12.0));
        })
        .unwrap();
    ui.compute_layout(Size::new(320.0, 120.0));
    assert!(ui.get(popup).is_some());

    let outside_center = ui.effective_bounds(outside).center();
    let down = ui.dispatch_pointer_event(&pointer_down(outside_center, 1));
    assert!(down.changed());
    assert!(ui.get(popup).is_none());

    let up = ui.dispatch_pointer_event(&pointer_up(outside_center, 2));
    assert!(up.changed());
    assert!(
        clicked.get(),
        "outside click should continue after dismissal"
    );
}
