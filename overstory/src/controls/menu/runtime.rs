// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::boxed::Box;

use understory_focus::Navigation;
use understory_overlay::OverlayBehavior;

use super::item::{MenuItemWidget, MenuWidget, with_menu_item_widget_mut};
use super::model::{DEFAULT_MENU_MIN_WIDTH, Menu, MenuItem};
use crate::SurfaceSpecExt;
use crate::controls::popup::Popup;
use crate::element::ElementId;
use crate::layouts::{Layout, LayoutAlign, LayoutAlignment, LayoutDimension, LayoutSize};
use crate::overlay::Placement;
use crate::ui::Ui;

const DEFAULT_MENU_ITEM_HEIGHT: f64 = 34.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MenuLevel {
    Root,
    Submenu,
}

pub(super) fn open_root_menu(
    ui: &mut Ui,
    button: ElementId,
    menu: Menu,
    direction: Navigation,
) -> bool {
    if ui
        .active_overlay_for(button, OverlayBehavior::Menu)
        .is_some()
    {
        return false;
    }
    let popup = ui.open_semantic_popup_with_behavior(
        button,
        None,
        menu_popup_host().offset(0.0, 4.0),
        OverlayBehavior::Menu,
        move |ui, popup| append_menu_surface(ui, popup, menu, MenuLevel::Root),
    );
    let Some(popup) = popup else {
        return false;
    };
    let _ = ui.focus_scope_edge(popup, direction);
    true
}

pub(super) fn close_root_menu(ui: &mut Ui, button: ElementId) -> bool {
    let Some(popup) = ui.active_overlay_for(button, OverlayBehavior::Menu) else {
        return false;
    };
    ui.close_overlay_tree(popup)
}

pub(super) fn open_submenu(ui: &mut Ui, item: ElementId, submenu: Menu, focus_child: bool) -> bool {
    let Some(parent) = ui.containing_overlay_for(item) else {
        return false;
    };
    if let Some(popup) = ui.active_overlay_for(item, OverlayBehavior::Submenu) {
        if focus_child {
            let _ = ui.focus_scope_edge(popup, Navigation::Down);
        }
        return true;
    }
    let _ = ui.close_overlay_descendants(parent);
    let popup = ui.open_semantic_popup_with_behavior(
        item,
        Some(parent),
        menu_popup_host()
            .placement(Placement::RIGHT_START)
            .offset(10.0, 0.0),
        OverlayBehavior::Submenu,
        move |ui, popup| append_menu_surface(ui, popup, submenu, MenuLevel::Submenu),
    );
    let Some(popup) = popup else {
        return false;
    };
    if focus_child {
        let _ = ui.focus_scope_edge(popup, Navigation::Down);
    }
    true
}

fn menu_popup_host() -> Popup {
    Popup::new()
        .padding(0.0, 0.0)
        .background(None)
        .border_width(0.0)
}

pub(super) fn close_submenu_for_target(ui: &mut Ui, item: ElementId) -> bool {
    let Some(popup) = ui.active_overlay_for(item, OverlayBehavior::Submenu) else {
        return false;
    };
    ui.close_popup(popup)
}

pub(super) fn close_descendants_of_containing_menu(ui: &mut Ui, item: ElementId) {
    if let Some(parent) = ui.containing_overlay_for(item) {
        let _ = ui.close_overlay_descendants(parent);
    }
}

fn append_menu_surface(ui: &mut Ui, parent: ElementId, menu: Menu, level: MenuLevel) -> ElementId {
    let surface = ui.append_widget(parent, Box::new(MenuWidget { level }));
    let size = match menu.width {
        Some(width) => LayoutSize::new().width(LayoutDimension::Logical(width)),
        None => LayoutSize::new().min_width(LayoutDimension::Logical(DEFAULT_MENU_MIN_WIDTH)),
    };
    ui.set_layout(surface, Layout::column(0.0).size(size));
    for item in menu.items {
        append_menu_item(ui, surface, item);
    }
    surface
}

fn append_menu_item(ui: &mut Ui, parent: ElementId, item: MenuItem) -> ElementId {
    let id = ui.append_widget_deferred_template(
        parent,
        Box::new(MenuItemWidget {
            submenu: item.submenu,
            activated: item.activated,
            close_timer: None,
        }),
    );
    let props = *ui.properties();
    ui.set_local(id, props.content, item.label);
    ui.set_layout(
        id,
        Layout::row(8.0)
            .size(LayoutSize::new().height(LayoutDimension::Logical(DEFAULT_MENU_ITEM_HEIGHT)))
            .alignment(LayoutAlignment::new().align_items(LayoutAlign::Center)),
    );
    item.element.apply_to(ui, id);
    ui.expand_resolved_template(id);
    ui.on_timer(id, move |ui, event| {
        with_menu_item_widget_mut(ui, id, |widget, ui| {
            widget.handle_close_timer(ui, id, event.timer);
        });
    });
    id
}
