// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::string::String;

use accesskit::{Action, HasPopup, Role};
use understory_focus::Navigation;
use understory_overlay::OverlayBehavior;
use understory_property::DependencyObjectExt;
use understory_style::TypeTag;
use understory_timing::{TimerDuration, TimerId};

use super::model::{Menu, MenuItemActivatedEvent, MenuItemActivatedSlot};
use super::runtime::{
    MenuLevel, close_descendants_of_containing_menu, close_submenu_for_target, open_submenu,
};
use super::{SLOT_MENU_ITEM_INDICATOR, SLOT_MENU_ITEM_LABEL, TYPE_MENU, TYPE_MENU_ITEM};
use crate::activation::ActivationSource;
use crate::behavior::{ArrangeContext, ArrangeResult, WidgetEventContext};
use crate::controls::support::{
    InlineArrowDirection, inline_arrow_direction, is_descendant_or_self,
};
use crate::element::{Element, ElementId};
use crate::event_runtime::{ClickEvent, KeyEvent, PointerRoutedEvent, RoutedEventKind};
use crate::template::TemplateContract;
use crate::ui::Ui;
use crate::widget::Widget;

const SUBMENU_INDICATOR_LTR: &str = "›";
const SUBMENU_INDICATOR_RTL: &str = "‹";
const SUBMENU_CLOSE_DELAY: TimerDuration = 180_000_000;
const SUBMENU_GRACE_POLL: TimerDuration = 75_000_000;

#[derive(Debug)]
pub(super) struct MenuWidget {
    pub(super) level: MenuLevel,
}

impl Widget for MenuWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_MENU)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<Role> {
        Some(Role::Menu)
    }

    fn configure_accessibility(
        &self,
        element: &Element,
        ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        if let Some(focused) = ctx.active_descendant_for_overlay_content(element.id()) {
            node.set_active_descendant(ctx.node_id(focused));
        }
    }

    fn handle_key(&mut self, ctx: &mut WidgetEventContext<'_>, event: KeyEvent<'_>) {
        use ui_events::keyboard::{Key, KeyState, NamedKey};

        if !event.phase().is_bubble() || event.key.state != KeyState::Down {
            return;
        }
        let menu = ctx.id();
        let popup = ctx.containing_overlay_for(menu).unwrap_or(menu);
        let handled = match &event.key.key {
            Key::Named(NamedKey::ArrowDown) => ctx.focus_navigate(Navigation::Down).is_some(),
            Key::Named(NamedKey::ArrowUp) => ctx.focus_navigate(Navigation::Up).is_some(),
            Key::Named(NamedKey::Home) => ctx.focus_scope_edge(popup, Navigation::Down).is_some(),
            Key::Named(NamedKey::End) => ctx.focus_scope_edge(popup, Navigation::Up).is_some(),
            key if self.level == MenuLevel::Submenu
                && inline_arrow_direction(ctx, menu, key) == Some(InlineArrowDirection::Start) =>
            {
                ctx.close_popup(popup)
            }
            _ => false,
        };
        if handled {
            event.mark_handled();
        }
    }

    fn handle_pointer(&mut self, ctx: &mut WidgetEventContext<'_>, event: PointerRoutedEvent<'_>) {
        if event.kind == RoutedEventKind::PointerLeave && event.phase().is_bubble() {
            let menu = ctx.id();
            arm_ancestor_close_timers(ctx, menu);
        }
    }
}

pub(super) struct MenuItemWidget {
    pub(super) submenu: Option<Menu>,
    pub(super) activated: MenuItemActivatedSlot,
    pub(super) close_timer: Option<TimerId>,
}

impl core::fmt::Debug for MenuItemWidget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MenuItemWidget")
            .field("has_submenu", &self.submenu.is_some())
            .field("close_timer", &self.close_timer)
            .finish_non_exhaustive()
    }
}

impl Widget for MenuItemWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_MENU_ITEM)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<Role> {
        Some(Role::MenuItem)
    }

    fn configure_accessibility(
        &self,
        element: &Element,
        ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        let item = element.id();
        if ctx
            .active_overlay_for(item, OverlayBehavior::Submenu)
            .is_some()
        {
            node.set_has_popup(HasPopup::Menu);
            node.set_expanded(true);
            if let Some(menu) = ctx.active_overlay_content_for(item, OverlayBehavior::Submenu) {
                node.push_controlled(ctx.node_id(menu));
            }
            if ctx.is_enabled(item) {
                node.add_action(Action::Click);
                node.add_action(Action::Collapse);
            }
        } else if self.submenu.is_some() {
            node.set_has_popup(HasPopup::Menu);
            node.set_expanded(false);
            if ctx.is_enabled(item) {
                node.add_action(Action::Click);
                node.add_action(Action::Expand);
            }
        } else {
            ctx.add_click_action_if_enabled(element, node);
        }
    }

    fn template_contract(&self) -> TemplateContract {
        TemplateContract::new()
            .with_required_slot(SLOT_MENU_ITEM_LABEL)
            .with_recommended_slot(SLOT_MENU_ITEM_INDICATOR)
    }

    fn clips_subtree(&self, _element: &Element) -> bool {
        true
    }

    fn template_expanded(&mut self, item: ElementId, ui: &mut Ui) {
        self.sync_indicator(ui, item);
    }

    fn arrange(&mut self, ctx: &mut ArrangeContext<'_>) -> ArrangeResult {
        let item = ctx.id();
        ArrangeResult::from_layout_changed(self.sync_indicator(ctx.ui_mut(), item))
    }

    fn handle_key(&mut self, ctx: &mut WidgetEventContext<'_>, event: KeyEvent<'_>) {
        use ui_events::keyboard::{Key, KeyState, NamedKey};

        let item = ctx.id();
        if !event.is_target() || event.key.state != KeyState::Down || !ctx.is_enabled(item) {
            return;
        }
        let source = match &event.key.key {
            Key::Named(NamedKey::Enter) => Some(ActivationSource::Keyboard),
            Key::Character(value) if value == " " => Some(ActivationSource::Keyboard),
            _ => None,
        };
        if let Some(source) = source {
            self.activate(ctx, item, source, true);
            event.mark_handled();
        } else if self.submenu.is_some()
            && inline_arrow_direction(ctx, item, &event.key.key) == Some(InlineArrowDirection::End)
        {
            self.open_submenu(ctx, item, true);
            event.mark_handled();
        }
    }

    fn handle_click(&mut self, ctx: &mut WidgetEventContext<'_>, event: ClickEvent<'_>) {
        let item = ctx.id();
        if event.is_semantic_target() && ctx.is_enabled(item) {
            self.activate(ctx, item, ActivationSource::Pointer, true);
            event.mark_handled();
        }
    }

    fn handle_pointer(&mut self, ctx: &mut WidgetEventContext<'_>, event: PointerRoutedEvent<'_>) {
        let item = ctx.id();
        if !event.is_semantic_target() || !ctx.is_enabled(item) {
            return;
        }
        match event.kind {
            RoutedEventKind::PointerEnter => {
                self.cancel_close_timer(ctx);
                cancel_ancestor_close_timers(ctx, item);
                let _ = ctx.set_pointer_focus(Some(item));
                if self.submenu.is_some() {
                    self.open_submenu(ctx, item, false);
                    ctx.begin_submenu_pointer_grace(item);
                } else {
                    close_descendants_of_containing_menu(ctx, item);
                }
            }
            RoutedEventKind::PointerLeave if self.submenu.is_some() => {
                self.arm_close_timer(ctx, item, SUBMENU_CLOSE_DELAY);
            }
            _ => {}
        }
    }

    fn handle_accessibility_action(
        &mut self,
        ctx: &mut WidgetEventContext<'_>,
        request: &accesskit::ActionRequest,
    ) -> bool {
        let item = ctx.id();
        if !ctx.is_enabled(item) {
            return false;
        }
        match request.action {
            Action::Click => {
                self.activate(ctx, item, ActivationSource::Accessibility, true);
                true
            }
            Action::Expand if self.submenu.is_some() => {
                self.open_submenu(ctx, item, true);
                true
            }
            Action::Collapse => close_submenu_for_target(ctx, item),
            _ => false,
        }
    }
}

impl MenuItemWidget {
    fn sync_indicator(&self, ui: &mut Ui, item: ElementId) -> bool {
        let Some(indicator) = ui.slot(item, SLOT_MENU_ITEM_INDICATOR) else {
            return false;
        };
        let text = if self.submenu.is_none() {
            String::new()
        } else if ui
            .layout_direction(item)
            .is_some_and(|direction| direction.is_rtl())
        {
            String::from(SUBMENU_INDICATOR_RTL)
        } else {
            String::from(SUBMENU_INDICATOR_LTR)
        };
        if ui
            .get(indicator)
            .and_then(|element| element.get_local_value(ui.properties().text))
            == Some(&text)
        {
            return false;
        }
        ui.set_local(indicator, ui.properties().text, text);
        true
    }

    fn activate(&self, ui: &mut Ui, item: ElementId, source: ActivationSource, focus_child: bool) {
        if self.submenu.is_some() {
            self.open_submenu(ui, item, focus_child);
            return;
        }
        let Some(popup) = ui.containing_overlay_for(item) else {
            return;
        };
        let Some(callback_source) = ui.overlay_tree_anchor(popup) else {
            return;
        };
        self.activated.dispatch(
            ui,
            callback_source,
            MenuItemActivatedEvent {
                anchor: callback_source,
                source,
            },
        );
        let _ = ui.close_overlay_tree(popup);
    }

    fn open_submenu(&self, ui: &mut Ui, item: ElementId, focus_child: bool) {
        let Some(submenu) = self.submenu.clone() else {
            return;
        };
        let _ = open_submenu(ui, item, submenu, focus_child);
    }

    fn cancel_close_timer(&mut self, ui: &mut Ui) {
        if let Some(timer) = self.close_timer.take() {
            let _ = ui.cancel_timer(timer);
        }
    }

    fn arm_close_timer(&mut self, ui: &mut Ui, item: ElementId, delay: TimerDuration) {
        self.cancel_close_timer(ui);
        if ui
            .active_overlay_for(item, OverlayBehavior::Submenu)
            .is_some()
        {
            self.close_timer = Some(ui.schedule_timer_once(item, delay));
        }
    }

    pub(super) fn handle_close_timer(&mut self, ui: &mut Ui, item: ElementId, timer: TimerId) {
        if self.close_timer != Some(timer) {
            return;
        }
        self.close_timer = None;
        let pointer_in_item = ui
            .hovered()
            .is_some_and(|hovered| is_descendant_or_self(ui, hovered, item));
        if pointer_in_item || ui.pointer_keeps_submenu_open(item) {
            self.arm_close_timer(ui, item, SUBMENU_GRACE_POLL);
        } else {
            let _ = close_submenu_for_target(ui, item);
        }
    }
}

pub(super) fn with_menu_item_widget_mut(
    ui: &mut Ui,
    item: ElementId,
    f: impl FnOnce(&mut MenuItemWidget, &mut Ui),
) {
    ui.with_widget_mut(item, |widget, ui| {
        if let Some(widget) = widget.downcast_mut::<MenuItemWidget>() {
            f(widget, ui);
        }
    });
}

fn cancel_ancestor_close_timers(ui: &mut Ui, descendant: ElementId) {
    for target in ui.overlay_ancestor_targets(descendant) {
        with_menu_item_widget_mut(ui, target, |widget, ui| {
            widget.cancel_close_timer(ui);
        });
    }
}

fn arm_ancestor_close_timers(ui: &mut Ui, descendant: ElementId) {
    for target in ui.overlay_ancestor_targets(descendant) {
        with_menu_item_widget_mut(ui, target, |widget, ui| {
            widget.arm_close_timer(ui, target, SUBMENU_CLOSE_DELAY);
        });
    }
}
