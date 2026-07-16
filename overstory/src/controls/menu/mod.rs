// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained menu buttons, context menus, menu surfaces, command items, and
//! submenus.
//!
//! `MenuButton` and `ContextMenu` are invocation policies over the same
//! reusable authored menu model. Opening either realizes an ordinary retained
//! `Menu` below the shared popup root; nested menu entries become parented
//! Understory overlays rather than a menu-local stack. Menu widgets own command
//! and input policy only. Overlay lifetime, containment metadata, dismissal,
//! placement continuity, and hover-grace geometry remain owned by the shared
//! overlay runtime.
//!
//! ```ignore
//! ui.append(
//!     parent,
//!     MenuButton::new(
//!         "File",
//!         Menu::new()
//!             .item(MenuItem::new("New").on_activate(|_, _| {}))
//!             .item(MenuItem::new("Unavailable").disabled())
//!             .item(MenuItem::submenu(
//!                 "Export",
//!                 Menu::new()
//!                     .item(MenuItem::new("PDF").on_activate(|_, _| {}))
//!                     .item(MenuItem::new("SVG").on_activate(|_, _| {})),
//!             )),
//!     ),
//! );
//!
//! ui.append(
//!     parent,
//!     Panel::new().context_menu(
//!         Menu::new().item(MenuItem::new("Inspect").on_activate(|_, _| {})),
//!     ),
//! );
//! ```

mod button;
mod context;
mod item;
mod model;
mod runtime;
mod templates;

#[cfg(test)]
mod context_tests;
#[cfg(test)]
mod tests;

pub use button::MenuButton;
pub use context::{
    ContextMenu, ContextMenuRequest, ContextMenuSource, ContextMenuSpec, ContextMenuSpecExt,
};
pub use model::{Menu, MenuItem, MenuItemActivatedEvent};

pub(crate) use runtime::open_context_menu_surface;
pub(crate) use templates::{default_menu_button_template, default_menu_item_template};

use understory_style::{PartTag, TypeTag};

use crate::template::TemplateSlot;

/// Selector identity for `MenuButton`.
pub const TYPE_MENU_BUTTON: TypeTag = TypeTag(40);
/// Selector identity for an open `Menu` surface.
pub const TYPE_MENU: TypeTag = TypeTag(41);
/// Selector identity for a `MenuItem`.
pub const TYPE_MENU_ITEM: TypeTag = TypeTag(42);

/// Owner-local part identity for a menu button's label.
pub const PART_MENU_BUTTON_LABEL: PartTag = PartTag(1);
/// Owner-local part identity for a menu button's popup indicator.
pub const PART_MENU_BUTTON_INDICATOR: PartTag = PartTag(2);
/// Owner-local part identity for a menu item's label.
pub const PART_MENU_ITEM_LABEL: PartTag = PartTag(1);
/// Owner-local part identity for a submenu indicator.
pub const PART_MENU_ITEM_INDICATOR: PartTag = PartTag(2);

/// Semantic slot for a menu button's label.
pub const SLOT_MENU_BUTTON_LABEL: TemplateSlot =
    TemplateSlot::new("menu-button-label").with_part_tag(PART_MENU_BUTTON_LABEL);
/// Semantic slot for a menu button's popup indicator.
pub const SLOT_MENU_BUTTON_INDICATOR: TemplateSlot =
    TemplateSlot::new("menu-button-indicator").with_part_tag(PART_MENU_BUTTON_INDICATOR);
/// Semantic slot for a menu item's label.
pub const SLOT_MENU_ITEM_LABEL: TemplateSlot =
    TemplateSlot::new("menu-item-label").with_part_tag(PART_MENU_ITEM_LABEL);
/// Semantic slot for a submenu indicator.
pub const SLOT_MENU_ITEM_INDICATOR: TemplateSlot =
    TemplateSlot::new("menu-item-indicator").with_part_tag(PART_MENU_ITEM_INDICATOR);
