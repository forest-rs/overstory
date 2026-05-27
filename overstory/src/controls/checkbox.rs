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

use crate::Phase;
use crate::behavior::WidgetEventContext;
use crate::controls::text::TextWidget;
use crate::element::{Element, ElementId};
use crate::event_runtime::{ClickEvent, KeyEvent};
use crate::pseudos::PSEUDO_CHECKED;
use crate::template::{
    Template, TemplateBinding, TemplateBuiltInProperty, TemplateNode, TemplateSlot, TemplateSpec,
    TemplateWidgetFactory,
};
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, SurfacePartWidget, TemplateOverrideWidget, Widget};

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
    state: ControlState,
    template: Option<&'static dyn Template>,
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
        Self {
            content: content.into(),
            initial_checked: false,
            font_size: None,
            state: ControlState::interactive(),
            template: None,
        }
    }

    /// Sets the initial checked state.
    #[must_use]
    pub fn checked(mut self, checked: bool) -> Self {
        self.initial_checked = checked;
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
        self.state = self.state.focusable(focusable);
        self
    }

    /// Sets whether the checkbox starts enabled.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.state = self.state.enabled(enabled);
        self
    }

    /// Convenience for [`Self::enabled`] with `false`.
    #[must_use]
    pub fn disabled(self) -> Self {
        self.enabled(false)
    }

    /// Overrides the visual template used when this checkbox is inserted.
    ///
    /// Custom templates should create [`SLOT_CHECK_BOX_BOX`],
    /// [`SLOT_CHECK_BOX_MARK`], and [`SLOT_CHECK_BOX_LABEL`] when they want
    /// default behavior and styling to keep working.
    #[must_use]
    pub fn template(mut self, template: &'static dyn Template) -> Self {
        self.template = Some(template);
        self
    }
}

impl AppendSpec for CheckBox {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let widget: Box<dyn Widget> = match self.template {
            Some(template) => Box::new(TemplateOverrideWidget::new(CheckBoxWidget, template)),
            None => Box::new(CheckBoxWidget),
        };
        let id = ui.append_widget(parent, widget);
        let content_prop = ui.properties().content;
        let font_size_prop = ui.properties().font_size;
        ui.set_local(id, content_prop, self.content);
        if let Some(font_size) = self.font_size {
            ui.set_local(id, font_size_prop, font_size);
        }
        ui.set_control_state(id, self.state);
        set_check_box_checked(ui, id, self.initial_checked);
        id
    }
}

/// Per-element widget for `CheckBox`.
#[derive(Debug, Default)]
pub(crate) struct CheckBoxWidget;

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
        ctx.add_click_action(node);
        ctx.apply_checked_state(element, node);
    }

    fn default_template(&self) -> Option<&'static dyn Template> {
        Some(&CHECK_BOX_TEMPLATE)
    }

    fn handle_click(&mut self, ctx: &mut WidgetEventContext<'_>, event: ClickEvent<'_>) {
        let checkbox = ctx.id();
        if !matches!(event.phase, Phase::Target | Phase::Bubble) {
            return;
        }
        if !ctx.is_enabled(checkbox) {
            return;
        }
        toggle_check_box(ctx, checkbox);
        event.mark_handled();
    }

    fn handle_key(&mut self, ctx: &mut WidgetEventContext<'_>, event: KeyEvent<'_>) {
        use ui_events::keyboard::{Key, KeyState, NamedKey};
        let checkbox = ctx.id();
        if !matches!(event.phase, Phase::Target) {
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
            toggle_check_box(ctx, checkbox);
            event.mark_handled();
        }
    }
}

type CheckBoxTemplate = TemplateSpec;

static CHECK_BOX_TEMPLATE: CheckBoxTemplate = TemplateSpec::new(CHECK_BOX_TEMPLATE_NODES);

const CHECK_BOX_LABEL_BINDINGS: &[TemplateBinding] = &[
    TemplateBinding::owner_to_part(
        TemplateBuiltInProperty::Content,
        TemplateBuiltInProperty::Content,
    ),
    TemplateBinding::owner_to_part(
        TemplateBuiltInProperty::FontSize,
        TemplateBuiltInProperty::FontSize,
    ),
];

const CHECK_BOX_TEMPLATE_NODES: &[TemplateNode] = &[
    TemplateNode::widget(TemplateWidgetFactory::new(check_box_box_widget))
        .with_slot(SLOT_CHECK_BOX_BOX)
        .with_children(CHECK_BOX_BOX_CHILDREN),
    TemplateNode::widget(TemplateWidgetFactory::new(check_box_label_widget))
        .with_slot(SLOT_CHECK_BOX_LABEL)
        .with_bindings(CHECK_BOX_LABEL_BINDINGS),
];

const CHECK_BOX_BOX_CHILDREN: &[TemplateNode] =
    &[
        TemplateNode::widget(TemplateWidgetFactory::new(check_box_mark_widget))
            .with_slot(SLOT_CHECK_BOX_MARK),
    ];

fn check_box_box_widget() -> Box<dyn Widget> {
    Box::new(SurfacePartWidget)
}

fn check_box_mark_widget() -> Box<dyn Widget> {
    Box::new(TextWidget)
}

fn check_box_label_widget() -> Box<dyn Widget> {
    Box::new(TextWidget)
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
        ui.set_local(mark, ui.properties().content, String::from(content));
    }
}

fn toggle_check_box(ui: &mut Ui, checkbox: ElementId) {
    let checked = ui
        .get(checkbox)
        .and_then(|el| el.get_local_value(ui.properties().is_checked).copied())
        .unwrap_or(false);
    set_check_box_checked(ui, checkbox, !checked);
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
