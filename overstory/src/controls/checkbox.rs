// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `CheckBox`: labeled two-state check control.
//!
//! Roadmap:
//! - Keep `CheckBox` as a form-style checked state control, separate from
//!   `ToggleSwitch` even though both share the same checked property and pseudo.
//! - Add tri-state support only when the property model and templates can
//!   represent indeterminate state cleanly.
//! - Move from string-only labels toward content slots so labels can contain
//!   richer text or small inline elements.
//! - Tighten selection, validation, form binding, and change notification so
//!   checked state works as data, not just as a visual toggle.
//! - Add accessibility metadata for role, checked / mixed state, label
//!   association, focus, disabled state, and keyboard activation.

use alloc::{boxed::Box, string::String};

use understory_property::DependencyObjectExt;
use understory_style::{PartTag, TypeTag};

use crate::behavior::WidgetEventContext;
use crate::controls::support::{
    CheckedChangedEvent, CheckedChangedHandler, CheckedChangedSlot, dispatch_checked_changed,
};
use crate::element::{Element, ElementId};
use crate::event_runtime::{ClickEvent, KeyEvent};
use crate::pseudos::PSEUDO_CHECKED;
use crate::template::{TemplateContract, TemplateNode, TemplateSlot, TemplateSpec};
use crate::ui::{ControlState, Ui};
use crate::widget::{
    AppendSpec, ContentSlotSpec, ControlTemplateSpec, ElementConfig, ElementSpec, Widget,
};

/// Selector identity for `CheckBox`.
pub const TYPE_CHECK_BOX: TypeTag = TypeTag(8);
/// Owner-local part identity for the square check box.
pub const PART_CHECK_BOX_BOX: PartTag = PartTag(1);
/// Owner-local part identity for the check mark.
pub const PART_CHECK_BOX_MARK: PartTag = PartTag(2);
/// Owner-local part identity for the visible label.
pub const PART_CHECK_BOX_LABEL: PartTag = PartTag(3);
/// Semantic slot for the square check box.
pub const SLOT_CHECK_BOX_BOX: TemplateSlot =
    TemplateSlot::new("check-box").with_part_tag(PART_CHECK_BOX_BOX);
/// Semantic slot for the check mark.
pub const SLOT_CHECK_BOX_MARK: TemplateSlot =
    TemplateSlot::new("check-mark").with_part_tag(PART_CHECK_BOX_MARK);
/// Semantic slot for the visible label.
pub const SLOT_CHECK_BOX_LABEL: TemplateSlot =
    TemplateSlot::new("check-label").with_part_tag(PART_CHECK_BOX_LABEL);

/// Builder + spec for a `CheckBox`.
pub struct CheckBox {
    content: String,
    initial_checked: bool,
    font_size: Option<f64>,
    element: ElementConfig,
    on_checked_changed: Option<CheckedChangedHandler>,
}

impl core::fmt::Debug for CheckBox {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CheckBox")
            .field("content", &self.content)
            .field("initial_checked", &self.initial_checked)
            .finish_non_exhaustive()
    }
}

impl CheckBox {
    /// Starts a new labeled checkbox.
    #[must_use]
    pub fn new(content: impl Into<String>) -> Self {
        let mut element = ElementConfig::default();
        element.set_control_state(ControlState::interactive());
        Self {
            content: content.into(),
            initial_checked: false,
            font_size: None,
            element,
            on_checked_changed: None,
        }
    }

    /// Sets the initial checked state.
    #[must_use]
    pub fn checked(mut self, checked: bool) -> Self {
        self.initial_checked = checked;
        self
    }

    /// Registers a handler fired after user input changes the checked state.
    #[must_use]
    pub fn on_checked_changed<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, CheckedChangedEvent) + 'static,
    {
        self.on_checked_changed = Some(Box::new(handler));
        self
    }

    /// Overrides the label font size.
    ///
    /// When unset, the value comes from the active style cascade.
    #[must_use]
    pub fn font_size(mut self, font_size: f64) -> Self {
        self.font_size = Some(font_size);
        self
    }

    /// Sets whether the checkbox participates in focus traversal.
    #[must_use]
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.element
            .update_control_state(|state| state.focusable(focusable));
        self
    }

    /// Sets whether the checkbox starts enabled.
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

    /// Sets the shared construction-time control state.
    #[must_use]
    pub fn control_state(mut self, state: ControlState) -> Self {
        self.element.set_control_state(state);
        self
    }
}

impl AppendSpec for CheckBox {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget_deferred_template(
            parent,
            Box::new(CheckBoxWidget::new(self.on_checked_changed)),
        );
        let content_prop = ui.properties().content;
        let font_size_prop = ui.properties().font_size;
        ui.set_local(id, content_prop, self.content);
        if let Some(font_size) = self.font_size {
            ui.set_local(id, font_size_prop, font_size);
        }
        self.element.apply_to(ui, id);
        ui.expand_default_template(id);
        set_check_box_checked(ui, id, self.initial_checked);
        id
    }
}

impl ControlTemplateSpec for CheckBox {}

impl ContentSlotSpec for CheckBox {
    const CONTENT_SLOT: TemplateSlot = SLOT_CHECK_BOX_LABEL;
}

impl ElementSpec for CheckBox {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

/// Per-element widget for `CheckBox`.
pub(crate) struct CheckBoxWidget {
    checked_changed: CheckedChangedSlot,
}

impl CheckBoxWidget {
    fn new(checked_changed: Option<CheckedChangedHandler>) -> Self {
        Self {
            checked_changed: CheckedChangedSlot::new(checked_changed),
        }
    }

    fn toggle_checked(&self, ui: &mut Ui, checkbox: ElementId) {
        let next = toggle_check_box(ui, checkbox);
        dispatch_checked_changed(
            ui,
            &self.checked_changed,
            CheckedChangedEvent {
                element: checkbox,
                checked: next,
            },
        );
    }
}

impl core::fmt::Debug for CheckBoxWidget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CheckBoxWidget").finish_non_exhaustive()
    }
}

impl Widget for CheckBoxWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_CHECK_BOX)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::CheckBox)
    }

    fn configure_accessibility(
        &self,
        element: &Element,
        ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        ctx.add_click_action_if_enabled(element, node);
        ctx.apply_checked_state(element, node);
    }

    fn default_template(&self) -> Option<crate::ControlTemplate> {
        Some(crate::ControlTemplate::from_spec_factory(
            check_box_template_spec,
        ))
    }

    fn template_contract(&self) -> TemplateContract {
        TemplateContract::new()
            .with_required_slot(SLOT_CHECK_BOX_BOX)
            .with_required_slot(SLOT_CHECK_BOX_LABEL)
            .with_optional_slot(SLOT_CHECK_BOX_MARK)
    }

    fn handle_click(&mut self, ctx: &mut WidgetEventContext<'_>, event: ClickEvent<'_>) {
        let checkbox = ctx.id();
        if !event.is_semantic_target() {
            return;
        }
        if !ctx.is_enabled(checkbox) {
            return;
        }
        self.toggle_checked(ctx, checkbox);
        event.mark_handled();
    }

    fn handle_key(&mut self, ctx: &mut WidgetEventContext<'_>, event: KeyEvent<'_>) {
        use ui_events::keyboard::{Key, KeyState, NamedKey};
        let checkbox = ctx.id();
        if !event.is_target() {
            return;
        }
        if event.key.state != KeyState::Down {
            return;
        }
        if !ctx.is_enabled(checkbox) {
            return;
        }
        let activate = match &event.key.key {
            Key::Named(NamedKey::Enter) => true,
            Key::Character(s) if s == " " => true,
            _ => false,
        };
        if activate {
            self.toggle_checked(ctx, checkbox);
            event.mark_handled();
        }
    }
}

fn check_box_template_spec() -> TemplateSpec {
    TemplateSpec::from_nodes([
        TemplateNode::surface_slot(SLOT_CHECK_BOX_BOX)
            .with_child(TemplateNode::text_slot(SLOT_CHECK_BOX_MARK)),
        TemplateNode::content_presenter(SLOT_CHECK_BOX_LABEL),
    ])
}

/// Sets `IsChecked` on `checkbox` and keeps the `:checked` pseudoclass in sync.
pub fn set_check_box_checked(ui: &mut Ui, checkbox: ElementId, checked: bool) {
    ui.set_local(checkbox, ui.properties().is_checked, checked);
    if checked {
        ui.add_pseudo(checkbox, PSEUDO_CHECKED);
    } else {
        ui.remove_pseudo(checkbox, PSEUDO_CHECKED);
    }

    if let Some(mark) = ui.slot(checkbox, SLOT_CHECK_BOX_MARK) {
        let content = if checked { "✓" } else { "" };
        ui.set_local(mark, ui.properties().text, String::from(content));
    }
}

fn toggle_check_box(ui: &mut Ui, checkbox: ElementId) -> bool {
    let checked = ui
        .get(checkbox)
        .and_then(|el| el.get_local_value(ui.properties().is_checked).copied())
        .unwrap_or(false);
    let next = !checked;
    set_check_box_checked(ui, checkbox, next);
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::rc::Rc;
    use core::cell::Cell;
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

    #[test]
    fn checkbox_click_toggles_checked_state() {
        let mut ui = Ui::new();
        let root = ui.root();
        let checkbox = ui.append(root, CheckBox::new("Ship it"));

        ui.synthesize_click(checkbox, kurbo::Point::ZERO);

        assert_eq!(
            ui.get(checkbox)
                .and_then(|el| el.get_local_value(ui.properties().is_checked).copied()),
            Some(true)
        );
    }

    #[test]
    fn checkbox_keyboard_activation_is_handled() {
        let mut ui = Ui::new();
        let root = ui.root();
        let checkbox = ui.append(root, CheckBox::new("Ship it"));
        ui.set_focus(Some(checkbox));

        let key = KeyboardEvent::key_down(Key::Named(NamedKey::Enter), Code::Enter);
        let result = ui.dispatch_key(&key);

        assert!(result.handled);
        assert_eq!(
            ui.get(checkbox)
                .and_then(|el| el.get_local_value(ui.properties().is_checked).copied()),
            Some(true)
        );
    }

    #[test]
    fn checkbox_checked_changed_reports_new_state() {
        let observed = Rc::new(Cell::new(None));
        let observed_for_handler = Rc::clone(&observed);
        let mut ui = Ui::new();
        let root = ui.root();
        let checkbox = ui.append(
            root,
            CheckBox::new("Ship it").on_checked_changed(move |_ui, event| {
                observed_for_handler.set(Some((event.element, event.checked)));
            }),
        );

        ui.synthesize_click(checkbox, kurbo::Point::ZERO);

        assert_eq!(observed.get(), Some((checkbox, true)));
    }
}
