// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::{rc::Rc, vec::Vec};
use core::cell::{Cell, RefCell};

use accesskit::{Action, ActionRequest, Node, TreeId};
use kurbo::{Point, Size};
use ui_events::keyboard::{Code, Key, KeyboardEvent, Modifiers, NamedKey};
use ui_events::pointer::PointerButton;
use understory_overlay::OverlayBehavior;

use super::{ContextMenu, ContextMenuSource, ContextMenuSpecExt, Menu, MenuItem};
use crate::accessibility::accesskit_node_id;
use crate::event_runtime::PointerEventKind;
use crate::inspection::OverlayAnchorInspection;
use crate::layouts::Layout;
use crate::test_support::{pointer_down, secondary_pointer_down, secondary_pointer_up};
use crate::{Button, ElementId, Panel, Tooltip, Ui, Visibility};

const VIEWPORT: Size = Size::new(640.0, 420.0);

#[test]
fn secondary_pointer_invokes_dynamic_menu_at_a_real_point_anchor() {
    let requests = Rc::new(RefCell::new(Vec::new()));
    let button_activations = Rc::new(Cell::new(0));
    let (mut ui, target, _outside) = test_ui(
        ContextMenu::dynamic({
            let requests = requests.clone();
            move |_ui, request| {
                requests.borrow_mut().push(request);
                Some(test_menu())
            }
        }),
        Some(button_activations.clone()),
    );
    let position = ui.effective_bounds(target).center();

    let down = ui.dispatch_pointer_event(&secondary_pointer_down(position, 1));
    let up = ui.dispatch_pointer_event(&secondary_pointer_up(position, 2));
    compute_layout(&mut ui);

    assert!(down.handled);
    assert!(down.overlay_changed);
    assert!(!up.overlay_changed);
    assert_eq!(button_activations.get(), 0);
    assert_eq!(requests.borrow().len(), 1);
    let request = requests.borrow()[0];
    assert_eq!(request.owner, target);
    assert_eq!(request.source, ContextMenuSource::Pointer);
    assert_eq!(request.position, position);

    let (popup, content) = open_menu(&ui, target, OverlayBehavior::ContextMenu);
    assert_eq!(ui.focused(), menu_items(&ui, content).first().copied());
    let snapshot = ui.inspection_snapshot();
    let row = snapshot
        .elements
        .iter()
        .find(|row| row.id == target)
        .expect("target should be inspectable");
    assert!(row.state.has_context_menu);
    assert_eq!(snapshot.overlays.entries[0].element, popup);
    assert_eq!(
        snapshot.overlays.entries[0].behavior,
        OverlayBehavior::ContextMenu
    );
    assert_eq!(
        snapshot.overlays.entries[0].anchor,
        OverlayAnchorInspection::Point(position)
    );
}

#[test]
fn routed_pointer_handler_can_decline_the_default_context_menu() {
    let (mut ui, target, _outside) = test_ui(ContextMenu::new(test_menu()), None);
    ui.events(target)
        .pointer(PointerEventKind::Down, |_ui, event| {
            if event.button == Some(PointerButton::Secondary) {
                event.mark_handled();
            }
        });
    let position = ui.effective_bounds(target).center();

    let result = ui.dispatch_pointer_event(&secondary_pointer_down(position, 1));

    assert!(result.handled);
    assert!(!result.overlay_changed);
    assert!(
        ui.active_overlay_for(target, OverlayBehavior::ContextMenu)
            .is_none()
    );
}

#[test]
fn context_menu_suppresses_an_outside_tooltip_until_teardown() {
    let (mut ui, target, _outside) = test_ui(ContextMenu::new(test_menu()), None);
    ui.set_tooltip(target, Tooltip::new("Target help"));
    let position = ui.effective_bounds(target).center();

    assert!(
        ui.dispatch_pointer_event(&secondary_pointer_down(position, 1))
            .handled
    );
    assert!(
        ui.inspection_snapshot()
            .overlays
            .entries
            .iter()
            .all(|entry| entry.behavior != OverlayBehavior::Tooltip)
    );
    assert!(
        ui.active_overlay_for(target, OverlayBehavior::ContextMenu)
            .is_some()
    );

    key(&mut ui, NamedKey::Escape, Code::Escape);
    assert!(
        ui.active_overlay_for(target, OverlayBehavior::ContextMenu)
            .is_none()
    );
    assert!(
        ui.inspection_snapshot()
            .overlays
            .entries
            .iter()
            .any(|entry| { entry.behavior == OverlayBehavior::Tooltip && entry.target == target })
    );
}

#[test]
fn nearest_attachment_wins_and_a_decline_falls_back_to_its_ancestor() {
    let parent_builds = Rc::new(Cell::new(0));
    let child_builds = Rc::new(Cell::new(0));
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(
        root,
        Layout::new().logical_size(VIEWPORT.width, VIEWPORT.height),
    );
    let parent = ui.append(root, Panel::new());
    ui.set_layout(
        parent,
        Layout::new()
            .absolute_at(24.0, 20.0)
            .logical_size(220.0, 90.0),
    );
    let child = ui.append(parent, Button::new("Nested target"));
    ui.set_layout(child, Layout::new().logical_size(160.0, 42.0));
    ui.set_context_menu(
        parent,
        ContextMenu::dynamic({
            let parent_builds = parent_builds.clone();
            move |_ui, _request| {
                parent_builds.set(parent_builds.get() + 1);
                Some(Menu::new().item(MenuItem::new("Parent command")))
            }
        }),
    );
    ui.set_context_menu(
        child,
        ContextMenu::dynamic({
            let child_builds = child_builds.clone();
            move |_ui, _request| {
                child_builds.set(child_builds.get() + 1);
                Some(Menu::new().item(MenuItem::new("Child command")))
            }
        }),
    );
    compute_layout(&mut ui);
    let position = ui.effective_bounds(child).center();

    assert!(
        ui.dispatch_pointer_event(&secondary_pointer_down(position, 1))
            .handled
    );
    assert_eq!(child_builds.get(), 1);
    assert_eq!(parent_builds.get(), 0);
    assert!(
        ui.active_overlay_for(child, OverlayBehavior::ContextMenu)
            .is_some()
    );
    key(&mut ui, NamedKey::Escape, Code::Escape);

    ui.set_context_menu(
        child,
        ContextMenu::dynamic({
            let child_builds = child_builds.clone();
            move |_ui, _request| {
                child_builds.set(child_builds.get() + 1);
                None
            }
        }),
    );
    assert!(
        ui.dispatch_pointer_event(&secondary_pointer_down(position, 2))
            .handled
    );
    assert_eq!(child_builds.get(), 2);
    assert_eq!(parent_builds.get(), 1);
    assert!(
        ui.active_overlay_for(parent, OverlayBehavior::ContextMenu)
            .is_some()
    );
}

#[test]
fn keyboard_and_shift_f10_invoke_from_focused_descendants() {
    let requests = Rc::new(RefCell::new(Vec::new()));
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(
        root,
        Layout::new().logical_size(VIEWPORT.width, VIEWPORT.height),
    );
    let owner = ui.append(root, Panel::new());
    ui.set_layout(
        owner,
        Layout::new()
            .absolute_at(30.0, 24.0)
            .logical_size(180.0, 70.0),
    );
    let focused = ui.append(owner, Button::new("Focused child"));
    ui.set_layout(focused, Layout::new().logical_size(140.0, 34.0));
    ui.set_context_menu(
        owner,
        ContextMenu::dynamic({
            let requests = requests.clone();
            move |_ui, request| {
                requests.borrow_mut().push(request);
                Some(test_menu())
            }
        }),
    );
    compute_layout(&mut ui);
    assert!(ui.set_focus(Some(focused)));

    let context_key = KeyboardEvent::key_down(Key::Named(NamedKey::ContextMenu), Code::ContextMenu);
    assert!(ui.dispatch_key(&context_key).handled);
    compute_layout(&mut ui);
    let expected = Point::new(
        ui.effective_bounds(focused).x0,
        ui.effective_bounds(focused).y1,
    );
    assert_eq!(requests.borrow()[0].owner, owner);
    assert_eq!(requests.borrow()[0].invoked_on, focused);
    assert_eq!(requests.borrow()[0].position, expected);
    assert_eq!(requests.borrow()[0].source, ContextMenuSource::Keyboard);

    key(&mut ui, NamedKey::Escape, Code::Escape);
    assert_eq!(ui.focused(), Some(focused));

    let mut shift_f10 = KeyboardEvent::key_down(Key::Named(NamedKey::F10), Code::F10);
    shift_f10.modifiers = Modifiers::SHIFT;
    assert!(ui.dispatch_key(&shift_f10).handled);
    assert_eq!(requests.borrow().len(), 2);
}

#[test]
fn accesskit_exposes_and_invokes_show_context_menu_without_disclosure_state() {
    let requests = Rc::new(RefCell::new(Vec::new()));
    let (mut ui, target, _outside) = test_ui(
        ContextMenu::dynamic({
            let requests = requests.clone();
            move |_ui, request| {
                requests.borrow_mut().push(request);
                Some(test_menu())
            }
        }),
        None,
    );

    let before = ui.accesskit_tree_update();
    let node = node_for(&before.nodes, target);
    assert!(node.supports_action(Action::ShowContextMenu));
    assert_eq!(node.is_expanded(), None);

    assert!(ui.handle_accesskit_action(&action(target, Action::ShowContextMenu)));
    compute_layout(&mut ui);
    assert_eq!(
        requests.borrow()[0].source,
        ContextMenuSource::Accessibility
    );
    assert!(
        ui.active_overlay_for(target, OverlayBehavior::ContextMenu)
            .is_some()
    );

    ui.clear_context_menu(target);
    assert!(
        ui.active_overlay_for(target, OverlayBehavior::ContextMenu)
            .is_none()
    );
    assert!(
        !node_for(&ui.accesskit_tree_update().nodes, target)
            .supports_action(Action::ShowContextMenu)
    );
}

#[test]
fn context_menu_reuses_nested_submenu_focus_and_command_teardown() {
    let (mut ui, target, _outside) = test_ui(
        ContextMenu::new(Menu::new().item(MenuItem::submenu(
            "Export",
            Menu::new().item(MenuItem::new("Remove owner").on_activate(|ui, event| {
                ui.remove(event.anchor);
            })),
        ))),
        None,
    );
    assert!(ui.handle_accesskit_action(&action(target, Action::ShowContextMenu)));
    compute_layout(&mut ui);
    let (root_popup, root_content) = open_menu(&ui, target, OverlayBehavior::ContextMenu);
    let submenu_item = menu_items(&ui, root_content)[0];

    key(&mut ui, NamedKey::ArrowRight, Code::ArrowRight);
    compute_layout(&mut ui);
    let (child_popup, child_content) = open_menu(&ui, submenu_item, OverlayBehavior::Submenu);
    let command = menu_items(&ui, child_content)[0];
    assert_eq!(ui.focused(), Some(command));

    assert!(ui.handle_accesskit_action(&action(command, Action::Click)));
    assert!(ui.get(target).is_none());
    assert!(ui.get(root_popup).is_none());
    assert!(ui.get(child_popup).is_none());
}

#[test]
fn repeated_direct_open_replaces_the_complete_context_tree() {
    let (mut ui, target, _outside) = test_ui(ContextMenu::new(test_menu()), None);
    let first = ui
        .open_context_menu_at(target, Point::new(80.0, 90.0), test_menu())
        .expect("first context menu should open");
    compute_layout(&mut ui);
    let first_content = ui
        .active_overlay_content_for(target, OverlayBehavior::ContextMenu)
        .expect("first menu should expose semantic content");
    let submenu = menu_items(&ui, first_content)[1];
    assert!(ui.set_focus(Some(submenu)));
    key(&mut ui, NamedKey::ArrowRight, Code::ArrowRight);
    compute_layout(&mut ui);
    let child = ui
        .active_overlay_for(submenu, OverlayBehavior::Submenu)
        .expect("first tree should include a submenu");

    let second = ui
        .open_context_menu_at(target, Point::new(260.0, 180.0), test_menu())
        .expect("second context menu should replace the first");
    compute_layout(&mut ui);

    assert_ne!(first, second);
    assert!(ui.get(first).is_none());
    assert!(ui.get(child).is_none());
    assert_eq!(ui.inspection_snapshot().overlays.entries.len(), 1);
    assert_eq!(
        ui.inspection_snapshot().overlays.entries[0].anchor,
        OverlayAnchorInspection::Point(Point::new(260.0, 180.0))
    );
}

#[test]
fn declining_factory_and_ineligible_owner_leave_no_partial_overlay() {
    let builds = Rc::new(Cell::new(0));
    let (mut ui, target, _outside) = test_ui(
        ContextMenu::dynamic({
            let builds = builds.clone();
            move |_ui, _request| {
                builds.set(builds.get() + 1);
                None
            }
        }),
        None,
    );
    let position = ui.effective_bounds(target).center();

    let declined = ui.dispatch_pointer_event(&secondary_pointer_down(position, 1));
    assert!(!declined.handled);
    assert!(!declined.overlay_changed);
    assert_eq!(builds.get(), 1);

    ui.set_enabled(target, false);
    let disabled = ui.open_context_menu_at(target, position, test_menu());
    assert_eq!(disabled, None);
    ui.set_enabled(target, true);
    ui.set_visibility(target, Visibility::Collapsed);
    assert_eq!(ui.open_context_menu_at(target, position, test_menu()), None);
}

#[test]
fn escape_outside_pointer_and_owner_lifecycle_close_and_restore_predictably() {
    let (mut ui, target, outside) = test_ui(ContextMenu::new(test_menu()), None);
    assert!(ui.set_focus(Some(target)));
    assert!(ui.handle_accesskit_action(&action(target, Action::ShowContextMenu)));
    compute_layout(&mut ui);
    let first = ui
        .active_overlay_for(target, OverlayBehavior::ContextMenu)
        .expect("context menu should open");

    key(&mut ui, NamedKey::Escape, Code::Escape);
    assert!(ui.get(first).is_none());
    assert_eq!(ui.focused(), Some(target));

    assert!(ui.handle_accesskit_action(&action(target, Action::ShowContextMenu)));
    compute_layout(&mut ui);
    let outside_result =
        ui.dispatch_pointer_event(&pointer_down(ui.effective_bounds(outside).center(), 10));
    assert!(outside_result.overlay_changed);
    assert!(
        ui.active_overlay_for(target, OverlayBehavior::ContextMenu)
            .is_none()
    );
    assert_eq!(ui.focused(), Some(outside));

    assert!(ui.set_focus(Some(target)));
    assert!(ui.handle_accesskit_action(&action(target, Action::ShowContextMenu)));
    ui.set_enabled(target, false);
    assert!(
        ui.active_overlay_for(target, OverlayBehavior::ContextMenu)
            .is_none()
    );

    ui.set_enabled(target, true);
    assert!(ui.set_focus(Some(target)));
    assert!(ui.handle_accesskit_action(&action(target, Action::ShowContextMenu)));
    ui.set_visibility(target, Visibility::Collapsed);
    assert!(
        ui.active_overlay_for(target, OverlayBehavior::ContextMenu)
            .is_none()
    );

    ui.set_visibility(target, Visibility::Visible);
    compute_layout(&mut ui);
    assert!(ui.set_focus(Some(target)));
    assert!(ui.handle_accesskit_action(&action(target, Action::ShowContextMenu)));
    ui.remove(target);
    assert!(ui.inspection_snapshot().overlays.entries.is_empty());
}

fn test_ui(
    context_menu: ContextMenu,
    activations: Option<Rc<Cell<usize>>>,
) -> (Ui, ElementId, ElementId) {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(
        root,
        Layout::new().logical_size(VIEWPORT.width, VIEWPORT.height),
    );
    let mut button = Button::new("Context target");
    if let Some(activations) = activations {
        button = button.on_activate(move |_ui, _event| {
            activations.set(activations.get() + 1);
        });
    }
    let target = ui.append(root, button.with_context_menu(context_menu));
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(24.0, 20.0)
            .logical_size(180.0, 42.0),
    );
    let outside = ui.append(root, Button::new("Outside"));
    ui.set_layout(
        outside,
        Layout::new()
            .absolute_at(500.0, 340.0)
            .logical_size(100.0, 34.0),
    );
    compute_layout(&mut ui);
    (ui, target, outside)
}

fn test_menu() -> Menu {
    Menu::new()
        .item(MenuItem::new("Inspect"))
        .item(MenuItem::submenu(
            "Priority",
            Menu::new()
                .item(MenuItem::new("High"))
                .item(MenuItem::new("Normal")),
        ))
        .item(MenuItem::new("Unavailable").disabled())
}

fn open_menu(ui: &Ui, target: ElementId, behavior: OverlayBehavior) -> (ElementId, ElementId) {
    let popup = ui
        .active_overlay_for(target, behavior)
        .expect("expected an open overlay for the target");
    let content = ui
        .active_overlay_content_for(target, behavior)
        .expect("expected semantic content for the open overlay");
    (popup, content)
}

fn menu_items(ui: &Ui, menu: ElementId) -> Vec<ElementId> {
    ui.get(menu)
        .expect("menu surface should be live")
        .children()
        .to_vec()
}

fn compute_layout(ui: &mut Ui) {
    ui.compute_layout(VIEWPORT);
}

fn key(ui: &mut Ui, named: NamedKey, code: Code) {
    let event = KeyboardEvent::key_down(Key::Named(named), code);
    assert!(ui.dispatch_key(&event).handled);
}

fn node_for(nodes: &[(accesskit::NodeId, Node)], element: ElementId) -> &Node {
    nodes
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
