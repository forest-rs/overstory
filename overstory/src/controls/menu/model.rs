// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::{boxed::Box, string::String, vec::Vec};

use crate::activation::ActivationSource;
use crate::controls::support::CallbackSlot;
use crate::element::ElementId;
use crate::ui::{ControlState, Ui};
use crate::widget::ElementConfig;

pub(super) const DEFAULT_MENU_MIN_WIDTH: f64 = 220.0;

pub(super) type MenuItemActivatedSlot = CallbackSlot<MenuItemActivatedEvent>;

/// Event delivered after a command menu item is activated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MenuItemActivatedEvent {
    /// Stable root anchor that owned the open menu tree.
    pub anchor: ElementId,
    /// Input path that requested activation.
    pub source: ActivationSource,
}

/// Reusable authored menu model.
#[derive(Clone)]
pub struct Menu {
    pub(super) items: Vec<MenuItem>,
    pub(super) width: Option<f64>,
}

impl core::fmt::Debug for Menu {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Menu")
            .field("items_len", &self.items.len())
            .field("width", &self.width)
            .finish_non_exhaustive()
    }
}

impl Menu {
    /// Starts an empty menu model.
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            width: None,
        }
    }

    /// Appends one command or submenu entry.
    #[must_use]
    pub fn item(mut self, item: MenuItem) -> Self {
        self.items.push(item);
        self
    }

    /// Sets a fixed logical width for each realized menu surface.
    ///
    /// Without an explicit width, a menu grows to fit its content and keeps a
    /// minimum width of 220 logical pixels.
    #[must_use]
    pub fn width(mut self, width: f64) -> Self {
        self.width = Some(normalized_extent(width, DEFAULT_MENU_MIN_WIDTH));
        self
    }
}

impl Default for Menu {
    fn default() -> Self {
        Self::new()
    }
}

/// Authored command or submenu entry.
#[derive(Clone)]
pub struct MenuItem {
    pub(super) label: String,
    pub(super) submenu: Option<Menu>,
    pub(super) activated: MenuItemActivatedSlot,
    pub(super) element: ElementConfig,
}

impl core::fmt::Debug for MenuItem {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MenuItem")
            .field("label", &self.label)
            .field("has_submenu", &self.submenu.is_some())
            .finish_non_exhaustive()
    }
}

impl MenuItem {
    /// Starts a command menu item.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        let mut element = ElementConfig::default();
        element.set_control_state(ControlState::interactive());
        Self {
            label: label.into(),
            submenu: None,
            activated: MenuItemActivatedSlot::new(None),
            element,
        }
    }

    /// Starts an item that opens `submenu`.
    #[must_use]
    pub fn submenu(label: impl Into<String>, submenu: Menu) -> Self {
        Self {
            submenu: Some(submenu),
            ..Self::new(label)
        }
    }

    /// Sets whether this item can receive focus or activation.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.element
            .update_control_state(|state| state.enabled(enabled));
        self
    }

    /// Convenience for [`Self::enabled`] with `false`.
    #[must_use]
    pub fn disabled(self) -> Self {
        self.enabled(false)
    }

    /// Registers the command invoked after this item closes its menu tree.
    ///
    /// Submenu entries use activation to open their child surface and do not
    /// dispatch command callbacks.
    #[must_use]
    pub fn on_activate<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, MenuItemActivatedEvent) + 'static,
    {
        self.activated = MenuItemActivatedSlot::new(Some(Box::new(handler)));
        self
    }
}

fn normalized_extent(value: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value.max(1.0)
    } else {
        fallback
    }
}
