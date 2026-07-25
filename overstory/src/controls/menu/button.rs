// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::{boxed::Box, string::String};

use accesskit::{Action, HasPopup, Role};
use understory_focus::Navigation;
use understory_overlay::OverlayBehavior;
use understory_style::TypeTag;

use super::model::Menu;
use super::runtime::{close_root_menu, open_root_menu};
use super::{SLOT_MENU_BUTTON_INDICATOR, SLOT_MENU_BUTTON_LABEL, TYPE_MENU_BUTTON};
use crate::behavior::WidgetEventContext;
use crate::element::{Element, ElementId};
use crate::event_runtime::{ClickEvent, KeyEvent};
use crate::template::TemplateContract;
use crate::ui::Ui;
use crate::widget::{AppendSpec, ElementConfig, ElementSpec, Widget};

const MENU_BUTTON_INDICATOR: &str = "▾";

/// Button that opens a reusable [`Menu`] through the shared overlay runtime.
pub struct MenuButton {
    label: String,
    menu: Menu,
    element: ElementConfig,
}

impl core::fmt::Debug for MenuButton {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MenuButton")
            .field("label", &self.label)
            .field("menu", &self.menu)
            .finish_non_exhaustive()
    }
}

impl MenuButton {
    /// Starts a menu button with its reusable authored menu model.
    #[must_use]
    pub fn new(label: impl Into<String>, menu: Menu) -> Self {
        let mut element = ElementConfig::default();
        element.set_focusable(true);
        Self {
            label: label.into(),
            menu,
            element,
        }
    }

    /// Sets whether the menu button participates in focus traversal.
    #[must_use]
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.element.set_focusable(focusable);
        self
    }

    /// Sets whether the menu button can open its menu.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.element.set_enabled(enabled);
        self
    }

    /// Convenience for [`Self::enabled`] with `false`.
    #[must_use]
    pub fn disabled(self) -> Self {
        self.enabled(false)
    }
}

impl AppendSpec for MenuButton {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget_deferred_template(
            parent,
            Box::new(MenuButtonWidget { menu: self.menu }),
        );
        ui.set_local(id, ui.properties().content, self.label);
        self.element.apply_to(ui, id);
        ui.expand_resolved_template(id);
        id
    }
}

impl ElementSpec for MenuButton {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

#[derive(Debug)]
struct MenuButtonWidget {
    menu: Menu,
}

impl Widget for MenuButtonWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_MENU_BUTTON)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<Role> {
        Some(Role::Button)
    }

    fn configure_accessibility(
        &self,
        element: &Element,
        ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        node.set_has_popup(HasPopup::Menu);
        let popup = ctx.active_overlay_for(element.id(), OverlayBehavior::Menu);
        node.set_expanded(popup.is_some());
        if let Some(menu) = ctx.active_overlay_content_for(element.id(), OverlayBehavior::Menu) {
            node.push_controlled(ctx.node_id(menu));
        }
        if ctx.is_enabled(element.id()) {
            ctx.add_click_action(node);
            node.add_action(if popup.is_some() {
                Action::Collapse
            } else {
                Action::Expand
            });
        }
    }

    fn template_contract(&self) -> TemplateContract {
        TemplateContract::new()
            .with_required_slot(SLOT_MENU_BUTTON_LABEL)
            .with_recommended_slot(SLOT_MENU_BUTTON_INDICATOR)
    }

    fn template_expanded(&mut self, button: ElementId, ui: &mut Ui) {
        if let Some(indicator) = ui.slot(button, SLOT_MENU_BUTTON_INDICATOR) {
            ui.set_local(
                indicator,
                ui.properties().text,
                String::from(MENU_BUTTON_INDICATOR),
            );
        }
    }

    fn handle_click(&mut self, ctx: &mut WidgetEventContext<'_>, event: ClickEvent<'_>) {
        let button = ctx.id();
        if event.is_semantic_target() && ctx.is_enabled(button) {
            self.toggle(ctx, button, Navigation::Down);
            event.mark_handled();
        }
    }

    fn handle_key(&mut self, ctx: &mut WidgetEventContext<'_>, event: KeyEvent<'_>) {
        use ui_events::keyboard::{Key, KeyState, NamedKey};

        let button = ctx.id();
        if !event.is_target() || event.key.state != KeyState::Down || !ctx.is_enabled(button) {
            return;
        }
        let direction = match &event.key.key {
            Key::Named(NamedKey::ArrowDown) => Some(Navigation::Down),
            Key::Named(NamedKey::ArrowUp) => Some(Navigation::Up),
            Key::Named(NamedKey::Enter) => Some(Navigation::Down),
            Key::Character(value) if value == " " => Some(Navigation::Down),
            _ => None,
        };
        if let Some(direction) = direction {
            self.toggle(ctx, button, direction);
            event.mark_handled();
        }
    }

    fn handle_accessibility_action(
        &mut self,
        ctx: &mut WidgetEventContext<'_>,
        request: &accesskit::ActionRequest,
    ) -> bool {
        let button = ctx.id();
        if !ctx.is_enabled(button) {
            return false;
        }
        match request.action {
            Action::Click => {
                self.toggle(ctx, button, Navigation::Down);
                true
            }
            Action::Expand => open_root_menu(ctx, button, self.menu.clone(), Navigation::Down),
            Action::Collapse => close_root_menu(ctx, button),
            _ => false,
        }
    }
}

impl MenuButtonWidget {
    fn toggle(&self, ui: &mut Ui, button: ElementId, direction: Navigation) {
        if !close_root_menu(ui, button) {
            let _ = open_root_menu(ui, button, self.menu.clone(), direction);
        }
    }
}
