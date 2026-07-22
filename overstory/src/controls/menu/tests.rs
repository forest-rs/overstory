// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::{rc::Rc, vec::Vec};
use core::cell::Cell;

use accesskit::{Action, ActionRequest, Node, Role, TreeId, TreeUpdate};
use kurbo::{Point, Size};
use ui_events::keyboard::{Code, Key, KeyboardEvent, Modifiers, NamedKey};
use understory_overlay::{GraceShape, OverlayBehavior, point_in_grace_shape};

use super::{Menu, MenuButton, MenuItem};
use crate::accessibility::accesskit_node_id;
use crate::element::ElementId;
use crate::layouts::Layout;
use crate::pseudos::PSEUDO_EXPANDED;
use crate::test_support::{pointer_down, pointer_move};
use crate::{Button, Ui, Visibility};

const VIEWPORT: Size = Size::new(640.0, 420.0);

#[test]
fn nested_keyboard_menus_share_overlay_focus_accessibility_and_inspection_state() {
    let (mut ui, button, _outside) = test_ui(test_menu());
    open_root_with_keyboard(&mut ui, button);

    let root = open_menu(&ui, button, OverlayBehavior::Menu);
    let root_items = menu_items(&ui, root.content);
    assert_eq!(ui.focused(), Some(root_items[0]));

    key(&mut ui, NamedKey::ArrowDown, Code::ArrowDown);
    assert_eq!(
        ui.focused(),
        Some(root_items[2]),
        "focus navigation should skip a disabled command"
    );
    key(&mut ui, NamedKey::ArrowRight, Code::ArrowRight);
    compute_layout(&mut ui);

    let child = open_menu(&ui, root_items[2], OverlayBehavior::Submenu);
    assert_expanded(&ui, root_items[2], true);
    let child_items = menu_items(&ui, child.content);
    assert_eq!(ui.focused(), Some(child_items[0]));
    key(&mut ui, NamedKey::ArrowDown, Code::ArrowDown);
    assert_eq!(ui.focused(), Some(child_items[2]));
    key(&mut ui, NamedKey::ArrowRight, Code::ArrowRight);
    compute_layout(&mut ui);

    let grandchild = open_menu(&ui, child_items[2], OverlayBehavior::Submenu);
    assert_expanded(&ui, child_items[2], true);
    let grandchild_items = menu_items(&ui, grandchild.content);
    assert_eq!(ui.focused(), Some(grandchild_items[0]));

    let snapshot = ui.inspection_snapshot();
    assert_eq!(snapshot.overlays.entries.len(), 3);
    assert_eq!(snapshot.overlays.focus_scopes.len(), 3);
    assert_eq!(snapshot.overlays.entries[0].behavior, OverlayBehavior::Menu);
    assert_eq!(snapshot.overlays.entries[0].parent, None);
    assert_eq!(snapshot.overlays.entries[0].content, root.content);
    assert_eq!(snapshot.overlays.entries[1].parent, Some(root.popup));
    assert_eq!(snapshot.overlays.entries[1].content, child.content);
    assert_eq!(snapshot.overlays.entries[2].parent, Some(child.popup));
    assert_eq!(snapshot.overlays.entries[2].content, grandchild.content);

    let update = ui.accesskit_tree_update();
    assert_menu_accessibility(&update, button, root.content, root_items[2], child.content);
    let child_item_node = node_for(&update, child_items[2]);
    assert_eq!(child_item_node.role(), Role::MenuItem);
    assert_eq!(child_item_node.is_expanded(), Some(true));
    assert_eq!(
        child_item_node.controls(),
        &[accesskit_node_id(grandchild.content)]
    );
    assert_eq!(
        node_for(&update, grandchild.content).active_descendant(),
        Some(accesskit_node_id(grandchild_items[0]))
    );
    assert_eq!(
        node_for(&update, child.content).active_descendant(),
        Some(accesskit_node_id(child_items[2]))
    );
    assert_eq!(
        node_for(&update, root.content).active_descendant(),
        Some(accesskit_node_id(root_items[2]))
    );

    key(&mut ui, NamedKey::ArrowLeft, Code::ArrowLeft);
    assert!(ui.get(grandchild.popup).is_none());
    assert!(ui.get(child.popup).is_some());
    assert_eq!(ui.focused(), Some(child_items[2]));
    assert_expanded(&ui, child_items[2], false);
    assert_expanded(&ui, root_items[2], true);

    key(&mut ui, NamedKey::Escape, Code::Escape);
    assert!(ui.get(child.popup).is_none());
    assert!(ui.get(root.popup).is_some());
    assert_eq!(ui.focused(), Some(root_items[2]));
    assert_expanded(&ui, root_items[2], false);

    key(&mut ui, NamedKey::ArrowRight, Code::ArrowRight);
    compute_layout(&mut ui);
    let reopened = open_menu(&ui, root_items[2], OverlayBehavior::Submenu);
    assert_expanded(&ui, root_items[2], true);
    ui.remove(reopened.popup);
    assert_expanded(&ui, root_items[2], false);

    key(&mut ui, NamedKey::Escape, Code::Escape);
    assert!(ui.get(root.popup).is_none());
    assert_eq!(ui.focused(), Some(button));
    assert!(ui.inspection_snapshot().overlays.entries.is_empty());
}

#[test]
fn tab_and_shift_tab_remain_inside_the_topmost_open_menu() {
    let (mut ui, button, outside) = test_ui(test_menu());
    open_root_with_keyboard(&mut ui, button);
    let root = open_menu(&ui, button, OverlayBehavior::Menu);
    let items = menu_items(&ui, root.content);

    key(&mut ui, NamedKey::Tab, Code::Tab);
    assert_eq!(ui.focused(), Some(items[2]));
    key(&mut ui, NamedKey::Tab, Code::Tab);
    assert_eq!(ui.focused(), Some(items[3]));
    key(&mut ui, NamedKey::Tab, Code::Tab);
    assert_eq!(ui.focused(), Some(items[0]));

    let mut shift_tab = KeyboardEvent::key_down(Key::Named(NamedKey::Tab), Code::Tab);
    shift_tab.modifiers = Modifiers::SHIFT;
    assert!(ui.dispatch_key(&shift_tab).handled);
    assert_eq!(ui.focused(), Some(items[3]));
    assert_ne!(ui.focused(), Some(button));
    assert_ne!(ui.focused(), Some(outside));
}

#[test]
fn command_closes_the_whole_tree_before_one_deferred_callback_and_may_remove_owner() {
    let button_id = Rc::new(Cell::new(None));
    let callback_count = Rc::new(Cell::new(0));
    let saw_open_overlay = Rc::new(Cell::new(true));
    let activated_anchor = Rc::new(Cell::new(None));
    let model = Menu::new().item(MenuItem::new("Delete owner").on_activate({
        let button_id = Rc::clone(&button_id);
        let callback_count = Rc::clone(&callback_count);
        let saw_open_overlay = Rc::clone(&saw_open_overlay);
        let activated_anchor = Rc::clone(&activated_anchor);
        move |ui, event| {
            callback_count.set(callback_count.get() + 1);
            activated_anchor.set(Some(event.anchor));
            let button = button_id.get().expect("fixture should publish the owner");
            saw_open_overlay.set(
                ui.active_overlay_for(button, OverlayBehavior::Menu)
                    .is_some(),
            );
            ui.remove(button);
        }
    }));
    let (mut ui, button, _outside) = test_ui(model);
    button_id.set(Some(button));
    open_root_with_keyboard(&mut ui, button);
    let root = open_menu(&ui, button, OverlayBehavior::Menu);
    let command = menu_items(&ui, root.content)[0];

    key(&mut ui, NamedKey::Enter, Code::Enter);

    assert_eq!(callback_count.get(), 1);
    assert_eq!(activated_anchor.get(), Some(button));
    assert!(!saw_open_overlay.get());
    assert!(ui.get(command).is_none());
    assert!(ui.get(root.popup).is_none());
    assert!(ui.get(button).is_none());
    assert!(ui.inspection_snapshot().overlays.entries.is_empty());
}

#[test]
fn reusable_menu_callbacks_survive_close_and_reopen() {
    let callback_count = Rc::new(Cell::new(0));
    let model = Menu::new().item(MenuItem::new("Run").on_activate({
        let callback_count = Rc::clone(&callback_count);
        move |_ui, _event| callback_count.set(callback_count.get() + 1)
    }));
    let (mut ui, button, _outside) = test_ui(model);

    open_root_with_keyboard(&mut ui, button);
    let first = open_menu(&ui, button, OverlayBehavior::Menu);
    key(&mut ui, NamedKey::Enter, Code::Enter);
    assert_eq!(callback_count.get(), 1);
    assert!(ui.get(first.popup).is_none());

    open_root_with_keyboard(&mut ui, button);
    let second = open_menu(&ui, button, OverlayBehavior::Menu);
    assert_ne!(second.popup, first.popup);
    key(&mut ui, NamedKey::Enter, Code::Enter);
    assert_eq!(callback_count.get(), 2);
    assert!(ui.get(second.popup).is_none());
}

#[test]
fn accesskit_actions_open_and_close_root_and_child_menus() {
    let (mut ui, button, _outside) = test_ui(test_menu());

    assert!(ui.handle_accesskit_action(&action(button, Action::Expand)));
    compute_layout(&mut ui);
    let root = open_menu(&ui, button, OverlayBehavior::Menu);
    let submenu_item = menu_items(&ui, root.content)[2];
    assert!(ui.handle_accesskit_action(&action(submenu_item, Action::Expand)));
    compute_layout(&mut ui);
    let child = open_menu(&ui, submenu_item, OverlayBehavior::Submenu);
    assert!(ui.get(child.popup).is_some());

    assert!(ui.handle_accesskit_action(&action(submenu_item, Action::Collapse)));
    assert!(ui.get(child.popup).is_none());
    assert!(ui.get(root.popup).is_some());
    assert!(ui.handle_accesskit_action(&action(button, Action::Collapse)));
    assert!(ui.get(root.popup).is_none());
}

#[test]
fn live_inspection_changes_coalesce_current_overlay_state() {
    let (mut ui, button, _outside) = test_ui(test_menu());
    let recording = ui.begin_live_inspection_recording();
    ui.clear_inspection_changes(recording);

    open_root_with_keyboard(&mut ui, button);
    let root = open_menu(&ui, button, OverlayBehavior::Menu);
    let opened = ui
        .drain_inspection_changes(recording)
        .expect("recording should be active");
    assert_eq!(
        opened
            .overlays
            .expect("opening should publish overlay state")
            .entries
            .iter()
            .map(|entry| entry.element)
            .collect::<Vec<_>>(),
        vec![root.popup]
    );

    key(&mut ui, NamedKey::Escape, Code::Escape);
    assert!(
        ui.drain_inspection_changes(recording)
            .expect("recording should be active")
            .overlays
            .expect("closing should publish overlay state")
            .entries
            .is_empty()
    );
}

#[test]
fn outside_focus_pointer_and_anchor_lifecycle_close_complete_menu_trees() {
    let (mut ui, button, outside) = test_ui(test_menu());

    let (root, child) = open_first_submenu(&mut ui, button);
    assert!(ui.set_focus(Some(outside)));
    assert!(ui.get(root.popup).is_none());
    assert!(ui.get(child.popup).is_none());
    assert_eq!(ui.focused(), Some(outside));

    assert!(ui.set_focus(Some(button)));
    let (root, child) = open_first_submenu(&mut ui, button);
    let result = ui.dispatch_pointer_event(&pointer_down(Point::new(580.0, 360.0), 1));
    assert!(
        ui.get(root.popup).is_none(),
        "outside pointer result={result:?}, overlays={:?}",
        ui.inspection_snapshot().overlays
    );
    assert!(ui.get(child.popup).is_none());
    assert_eq!(ui.focused(), Some(outside));

    let (root, child) = open_first_submenu(&mut ui, button);
    ui.set_enabled(button, false);
    assert!(ui.get(root.popup).is_none());
    assert!(ui.get(child.popup).is_none());
    ui.set_enabled(button, true);

    assert!(ui.set_focus(Some(button)));
    let (root, child) = open_first_submenu(&mut ui, button);
    ui.set_visibility(button, Visibility::Collapsed);
    assert!(ui.get(root.popup).is_none());
    assert!(ui.get(child.popup).is_none());
    ui.set_visibility(button, Visibility::Visible);

    assert!(ui.set_focus(Some(button)));
    let (root, child) = open_first_submenu(&mut ui, button);
    ui.remove(button);
    assert!(ui.get(root.popup).is_none());
    assert!(ui.get(child.popup).is_none());
    assert!(ui.inspection_snapshot().overlays.entries.is_empty());
}

#[test]
fn disabling_a_submenu_anchor_closes_only_its_descendant_tree() {
    let (mut ui, button, _outside) = test_ui(test_menu());
    let (root, child) = open_first_submenu(&mut ui, button);
    let submenu_item = menu_items(&ui, root.content)[2];

    ui.set_enabled(submenu_item, false);

    assert!(ui.get(child.popup).is_none());
    assert!(ui.get(root.popup).is_some());
    assert_eq!(
        ui.inspection_snapshot()
            .overlays
            .entries
            .iter()
            .map(|entry| entry.element)
            .collect::<Vec<_>>(),
        vec![root.popup]
    );
    assert_eq!(ui.focused(), Some(menu_items(&ui, root.content)[3]));
}

#[test]
fn hover_opens_submenus_and_understory_grace_delays_close_during_pointer_travel() {
    let (mut ui, button, _outside) = test_ui(test_menu());
    open_root_with_keyboard(&mut ui, button);
    let root = open_menu(&ui, button, OverlayBehavior::Menu);
    let submenu_item = menu_items(&ui, root.content)[2];
    let item_center = ui.effective_bounds(submenu_item).center();

    let _ = ui.dispatch_pointer_event(&pointer_move(item_center, 1, false));
    compute_layout(&mut ui);
    let child = open_menu(&ui, submenu_item, OverlayBehavior::Submenu);
    assert_eq!(ui.focused(), Some(submenu_item));
    let overlays = ui.inspection_snapshot().overlays;
    assert_eq!(overlays.grace_regions.len(), 1);
    let grace_point = point_only_in_grace(
        overlays.grace_regions[0].shape,
        overlays.entries[0].rect,
        overlays.entries[1].rect,
    );

    let _ = ui.dispatch_pointer_event(&pointer_move(grace_point, 2, false));
    assert!(ui.pointer_keeps_submenu_open(submenu_item));
    let close_deadline = ui
        .next_timer_deadline()
        .expect("leaving the submenu item should arm its close timer");
    ui.pump_timers(close_deadline);
    assert!(
        ui.get(child.popup).is_some(),
        "the Understory grace region should keep the child open through the gap"
    );

    let _ = ui.dispatch_pointer_event(&pointer_move(
        Point::new(600.0, 390.0),
        close_deadline + 1,
        false,
    ));
    assert!(
        !ui.pointer_keeps_submenu_open(submenu_item),
        "far pointer unexpectedly remained safe: {:?}",
        ui.inspection_snapshot().overlays
    );
    let poll_deadline = ui
        .next_timer_deadline()
        .expect("safe travel should rearm a short close poll");
    ui.pump_timers(poll_deadline);
    assert!(ui.get(child.popup).is_none());
    assert!(ui.get(root.popup).is_some());
}

#[test]
fn parent_hover_timer_keeps_a_branch_open_after_a_deeper_submenu_opens() {
    let model = Menu::new().item(MenuItem::submenu(
        "Export",
        Menu::new()
            .item(MenuItem::new("Scene archive"))
            .item(MenuItem::submenu(
                "Image",
                Menu::new()
                    .item(MenuItem::new("PNG"))
                    .item(MenuItem::new("OpenEXR")),
            )),
    ));
    let (mut ui, button, _outside) = test_ui(model);
    open_root_with_keyboard(&mut ui, button);
    let root = open_menu(&ui, button, OverlayBehavior::Menu);
    let export = menu_items(&ui, root.content)[0];

    let _ = ui.dispatch_pointer_event(&pointer_move(
        ui.effective_bounds(export).center(),
        1,
        false,
    ));
    compute_layout(&mut ui);
    let child = open_menu(&ui, export, OverlayBehavior::Submenu);
    let image = menu_items(&ui, child.content)[1];
    let _ = ui.dispatch_pointer_event(&pointer_move(ui.effective_bounds(image).center(), 2, false));
    compute_layout(&mut ui);
    let grandchild = open_menu(&ui, image, OverlayBehavior::Submenu);

    assert_eq!(
        ui.next_timer_deadline(),
        None,
        "entering a descendant item should cancel ancestor close polling"
    );
    ui.pump_timers(1_000_000_000);

    assert!(ui.get(root.popup).is_some());
    assert!(ui.get(child.popup).is_some());
    assert!(ui.get(grandchild.popup).is_some());
    assert_eq!(ui.focused(), Some(image));
}

#[derive(Clone, Copy)]
struct OpenMenu {
    popup: ElementId,
    content: ElementId,
}

fn test_menu() -> Menu {
    Menu::new()
        .item(MenuItem::new("Open"))
        .item(MenuItem::new("Unavailable").disabled())
        .item(MenuItem::submenu(
            "Export",
            Menu::new()
                .item(MenuItem::new("PDF"))
                .item(MenuItem::new("Unavailable export").disabled())
                .item(MenuItem::submenu(
                    "Advanced",
                    Menu::new().item(MenuItem::new("Optimized PDF")),
                )),
        ))
        .item(MenuItem::new("Quit"))
}

fn test_ui(menu: Menu) -> (Ui, ElementId, ElementId) {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(
        root,
        Layout::new()
            .relative()
            .logical_size(VIEWPORT.width, VIEWPORT.height),
    );
    let button = ui.append(root, MenuButton::new("File", menu));
    ui.set_layout(
        button,
        Layout::new()
            .absolute_at(24.0, 20.0)
            .logical_size(110.0, 34.0),
    );
    let outside = ui.append(root, Button::new("Outside"));
    ui.set_layout(
        outside,
        Layout::new()
            .absolute_at(500.0, 340.0)
            .logical_size(100.0, 34.0),
    );
    compute_layout(&mut ui);
    (ui, button, outside)
}

fn open_root_with_keyboard(ui: &mut Ui, button: ElementId) {
    if ui.focused() != Some(button) {
        assert!(ui.set_focus(Some(button)));
    }
    key(ui, NamedKey::ArrowDown, Code::ArrowDown);
    compute_layout(ui);
}

fn open_first_submenu(ui: &mut Ui, button: ElementId) -> (OpenMenu, OpenMenu) {
    open_root_with_keyboard(ui, button);
    let root = open_menu(ui, button, OverlayBehavior::Menu);
    let item = menu_items(ui, root.content)[2];
    assert!(ui.set_focus(Some(item)));
    key(ui, NamedKey::ArrowRight, Code::ArrowRight);
    compute_layout(ui);
    let child = open_menu(ui, item, OverlayBehavior::Submenu);
    (root, child)
}

fn open_menu(ui: &Ui, target: ElementId, behavior: OverlayBehavior) -> OpenMenu {
    let popup = ui
        .active_overlay_for(target, behavior)
        .expect("expected an open overlay for the target");
    let content = ui
        .active_overlay_content_for(target, behavior)
        .expect("expected semantic content for the open overlay");
    OpenMenu { popup, content }
}

fn menu_items(ui: &Ui, menu: ElementId) -> Vec<ElementId> {
    ui.get(menu)
        .expect("menu surface should be live")
        .children()
        .to_vec()
}

fn assert_expanded(ui: &Ui, item: ElementId, expected: bool) {
    assert_eq!(
        ui.get(item).unwrap().pseudos().contains(&PSEUDO_EXPANDED),
        expected
    );
}

fn compute_layout(ui: &mut Ui) {
    ui.compute_layout(VIEWPORT);
}

fn key(ui: &mut Ui, named: NamedKey, code: Code) {
    let event = KeyboardEvent::key_down(Key::Named(named), code);
    assert!(ui.dispatch_key(&event).handled);
}

fn assert_menu_accessibility(
    update: &TreeUpdate,
    button: ElementId,
    root_menu: ElementId,
    submenu_item: ElementId,
    child_menu: ElementId,
) {
    let button_node = node_for(update, button);
    assert_eq!(button_node.role(), Role::Button);
    assert_eq!(button_node.is_expanded(), Some(true));
    assert_eq!(button_node.controls(), &[accesskit_node_id(root_menu)]);
    assert!(button_node.supports_action(Action::Collapse));

    assert_eq!(node_for(update, root_menu).role(), Role::Menu);
    let item_node = node_for(update, submenu_item);
    assert_eq!(item_node.role(), Role::MenuItem);
    assert_eq!(item_node.is_expanded(), Some(true));
    assert_eq!(item_node.controls(), &[accesskit_node_id(child_menu)]);
    assert!(item_node.supports_action(Action::Collapse));
}

fn node_for(update: &TreeUpdate, element: ElementId) -> &Node {
    update
        .nodes
        .iter()
        .find(|(id, _)| *id == accesskit_node_id(element))
        .map(|(_, node)| node)
        .expect("accessibility node should exist")
}

fn action(element: ElementId, action: Action) -> ActionRequest {
    ActionRequest {
        action,
        target_tree: TreeId::ROOT,
        target_node: accesskit_node_id(element),
        data: None,
    }
}

fn point_only_in_grace(shape: GraceShape, parent: kurbo::Rect, child: kurbo::Rect) -> Point {
    assert!(
        child.x0 > parent.x1,
        "fixture needs a real pointer-travel gap: parent={parent:?}, child={child:?}"
    );
    let x = (parent.x1 + child.x0) * 0.5;
    let y0 = parent.y0.min(child.y0);
    let y1 = parent.y1.max(child.y1);
    for step in 0..=1_000 {
        let y = y0 + (y1 - y0) * f64::from(step) / 1_000.0;
        let point = Point::new(x, y);
        if !parent.contains(point) && !child.contains(point) && point_in_grace_shape(point, shape) {
            return point;
        }
    }
    panic!("expected the derived grace shape to bridge the overlay gap: {shape:?}");
}
