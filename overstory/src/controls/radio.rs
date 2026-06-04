// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `RadioButton` and `RadioGroup`: mutually exclusive checked controls.
//!
//! Roadmap:
//! - Keep `RadioButton` as the individual checked-state control and
//!   `RadioGroup` as the ordinary app-facing way to coordinate a set of radio
//!   choices.
//! - Share checked-state events, focus, accessibility, and theme state with
//!   `CheckBox` / `ToggleSwitch` rather than adding radio-specific machinery.
//! - Keep the first `RadioGroup` API index-based and string-backed until
//!   `ListBox`, `Select`, segmented selection, and future `ItemsControl`
//!   agree on typed option keys, item metadata, and templates. The likely next
//!   shape is `RadioOption<T>` where `T: Clone + Eq + 'static`, but that should
//!   be shared selection infrastructure rather than radio-only machinery.
//! - Grow toward item templates, per-option disabled state, validation, and
//!   orientation/wrap policies once the broader `ItemsControl` shape exists.

use alloc::{boxed::Box, string::String, vec::Vec};

use understory_property::DependencyObjectExt;
use understory_style::{PartTag, TypeTag};

use crate::Phase;
use crate::behavior::WidgetEventContext;
use crate::controls::choice::{
    ChoiceOption, ResolvedChoiceSelection, collect_options, last_enabled_index, option_label,
    selected_enabled_index, wrapping_next_enabled_index, wrapping_previous_enabled_index,
};
use crate::controls::support::{
    CallbackSlot, CheckedChangedEvent, CheckedChangedHandler, CheckedChangedSlot, EventHandler,
    dispatch_checked_changed,
};
use crate::controls::text::TextWidget;
use crate::element::{Element, ElementId};
use crate::event_runtime::{ClickEvent, KeyEvent};
use crate::layouts::{Layout, LayoutContainer, LayoutSpacing};
use crate::pseudos::PSEUDO_CHECKED;
use crate::template::{
    Template, TemplateBinding, TemplateBuiltInProperty, TemplateContract, TemplateNode,
    TemplateSlot, TemplateSpec, TemplateWidgetFactory,
};
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, ElementConfig, ElementSpec, TemplateOverrideWidget, Widget};

/// Selector identity for `RadioButton`.
pub const TYPE_RADIO_BUTTON: TypeTag = TypeTag(23);
/// Selector identity for `RadioGroup`.
pub const TYPE_RADIO_GROUP: TypeTag = TypeTag(24);
/// Owner-local part identity for the outer radio indicator.
pub const PART_RADIO_BUTTON_INDICATOR: PartTag = PartTag(1);
/// Owner-local part identity for the inner checked dot.
pub const PART_RADIO_BUTTON_DOT: PartTag = PartTag(2);
/// Owner-local part identity for the visible label.
pub const PART_RADIO_BUTTON_LABEL: PartTag = PartTag(3);
/// Semantic slot for the outer radio indicator.
pub const SLOT_RADIO_BUTTON_INDICATOR: TemplateSlot =
    TemplateSlot::new("radio-indicator").with_part_tag(PART_RADIO_BUTTON_INDICATOR);
/// Semantic slot for the inner checked dot.
pub const SLOT_RADIO_BUTTON_DOT: TemplateSlot =
    TemplateSlot::new("radio-dot").with_part_tag(PART_RADIO_BUTTON_DOT);
/// Semantic slot for the visible label.
pub const SLOT_RADIO_BUTTON_LABEL: TemplateSlot =
    TemplateSlot::new("radio-label").with_part_tag(PART_RADIO_BUTTON_LABEL);

const RADIO_BUTTON_TEMPLATE_REQUIRED_SLOTS: &[TemplateSlot] =
    &[SLOT_RADIO_BUTTON_INDICATOR, SLOT_RADIO_BUTTON_LABEL];
const RADIO_BUTTON_TEMPLATE_RECOMMENDED_SLOTS: &[TemplateSlot] = &[SLOT_RADIO_BUTTON_DOT];

const DEFAULT_RADIO_GROUP_GAP: f32 = 8.0;

type RadioSelectionChangedHandler = EventHandler<RadioSelectionChangedEvent>;
type RadioSelectionChangedSlot = CallbackSlot<RadioSelectionChangedEvent>;

/// Event delivered after a [`RadioGroup`] selection changes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadioSelectionChangedEvent {
    /// The radio group whose selection changed.
    pub group: ElementId,
    /// The selected item index after the change, if one is selected.
    pub selected_index: Option<usize>,
    /// The selected item text after the change, if one is selected.
    pub selected_item: Option<String>,
    /// The selected radio button element, if one is selected.
    pub selected_radio: Option<ElementId>,
}

/// Builder + spec for an individual radio button.
pub struct RadioButton {
    content: String,
    initial_checked: bool,
    font_size: Option<f64>,
    element: ElementConfig,
    template: Option<&'static dyn Template>,
    on_checked_changed: Option<CheckedChangedHandler>,
}

impl core::fmt::Debug for RadioButton {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RadioButton")
            .field("content", &self.content)
            .field("initial_checked", &self.initial_checked)
            .finish_non_exhaustive()
    }
}

impl RadioButton {
    /// Starts a new radio button with the given label.
    #[must_use]
    pub fn new(content: impl Into<String>) -> Self {
        let mut element = ElementConfig::default();
        element.set_control_state(ControlState::interactive());
        Self {
            content: content.into(),
            initial_checked: false,
            font_size: None,
            element,
            template: None,
            on_checked_changed: None,
        }
    }

    /// Sets the initial checked state.
    #[must_use]
    pub fn checked(mut self, checked: bool) -> Self {
        self.initial_checked = checked;
        self
    }

    /// Overrides the label font size.
    #[must_use]
    pub fn font_size(mut self, font_size: f64) -> Self {
        self.font_size = Some(font_size);
        self
    }

    /// Registers a handler fired after user input checks this radio button.
    #[must_use]
    pub fn on_checked_changed<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, CheckedChangedEvent) + 'static,
    {
        self.on_checked_changed = Some(Box::new(handler));
        self
    }

    /// Sets whether the radio button participates in focus traversal.
    #[must_use]
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.element
            .update_control_state(|state| state.focusable(focusable));
        self
    }

    /// Sets whether the radio button starts enabled.
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

    /// Overrides the visual template used when this radio button is inserted.
    #[must_use]
    pub fn template(mut self, template: &'static dyn Template) -> Self {
        self.template = Some(template);
        self
    }
}

impl AppendSpec for RadioButton {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let widget: Box<dyn Widget> = match self.template {
            Some(template) => Box::new(TemplateOverrideWidget::new(
                RadioButtonWidget::new(self.on_checked_changed),
                template,
            )),
            None => Box::new(RadioButtonWidget::new(self.on_checked_changed)),
        };
        let id = ui.append_widget(parent, widget);
        let content_prop = ui.properties().content;
        let font_size_prop = ui.properties().font_size;
        ui.set_local(id, content_prop, self.content);
        if let Some(font_size) = self.font_size {
            ui.set_local(id, font_size_prop, font_size);
        }
        self.element.apply_to(ui, id);
        set_radio_button_checked(ui, id, self.initial_checked);
        id
    }
}

impl ElementSpec for RadioButton {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

/// Builder + spec for a mutually-exclusive radio button group.
pub struct RadioGroup {
    options: Vec<ChoiceOption>,
    selected_index: Option<usize>,
    gap: f32,
    horizontal: bool,
    element: ElementConfig,
    on_selection_changed: Option<RadioSelectionChangedHandler>,
}

impl core::fmt::Debug for RadioGroup {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RadioGroup")
            .field("options_len", &self.options.len())
            .field("selected_index", &self.selected_index)
            .field("gap", &self.gap)
            .field("horizontal", &self.horizontal)
            .finish_non_exhaustive()
    }
}

impl RadioGroup {
    /// Starts a new empty vertical radio group.
    #[must_use]
    pub fn new() -> Self {
        let mut element = ElementConfig::default();
        element.set_control_state(ControlState::interactive().focusable(false));
        element.set_layout(Layout::column(DEFAULT_RADIO_GROUP_GAP));
        Self {
            options: Vec::new(),
            selected_index: None,
            gap: DEFAULT_RADIO_GROUP_GAP,
            horizontal: false,
            element,
            on_selection_changed: None,
        }
    }

    /// Replaces the group's options.
    #[must_use]
    pub fn options<I, O>(mut self, options: I) -> Self
    where
        I: IntoIterator<Item = O>,
        O: Into<ChoiceOption>,
    {
        self.options = collect_options(options);
        self
    }

    /// Replaces the group's item labels.
    #[must_use]
    pub fn items<I, O>(self, items: I) -> Self
    where
        I: IntoIterator<Item = O>,
        O: Into<ChoiceOption>,
    {
        self.options(items)
    }

    /// Appends one option.
    #[must_use]
    pub fn option(mut self, option: impl Into<ChoiceOption>) -> Self {
        self.options.push(option.into());
        self
    }

    /// Appends one item label.
    #[must_use]
    pub fn item(self, item: impl Into<ChoiceOption>) -> Self {
        self.option(item)
    }

    /// Sets the initially selected item index.
    #[must_use]
    pub fn selected_index(mut self, index: usize) -> Self {
        self.selected_index = Some(index);
        self
    }

    /// Lays out radio buttons horizontally.
    #[must_use]
    pub fn horizontal(mut self) -> Self {
        self.horizontal = true;
        self.sync_layout();
        self
    }

    /// Lays out radio buttons vertically.
    #[must_use]
    pub fn vertical(mut self) -> Self {
        self.horizontal = false;
        self.sync_layout();
        self
    }

    /// Sets the gap between radio buttons.
    #[must_use]
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self.sync_layout();
        self
    }

    /// Sets whether the group and its generated radio buttons start enabled.
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

    /// Registers a selection-changed handler.
    #[must_use]
    pub fn on_selection_changed<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, RadioSelectionChangedEvent) + 'static,
    {
        self.on_selection_changed = Some(Box::new(handler));
        self
    }

    fn sync_layout(&mut self) {
        let container = if self.horizontal {
            LayoutContainer::row()
        } else {
            LayoutContainer::column()
        };
        self.element.set_layout(
            Layout::new()
                .container(container)
                .spacing(LayoutSpacing::gap(self.gap)),
        );
    }
}

impl Default for RadioGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl AppendSpec for RadioGroup {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let selected_index = selected_enabled_index(&self.options, self.selected_index);
        let enabled = self
            .element
            .control_state()
            .map(|state| state.enabled)
            .unwrap_or(true);
        let id = ui.append_widget(
            parent,
            Box::new(RadioGroupWidget {
                options: self.options.clone(),
                radios: Vec::new(),
                selected_index,
                selection_changed: RadioSelectionChangedSlot::new(self.on_selection_changed),
            }),
        );
        self.element.apply_to(ui, id);

        let mut radios = Vec::with_capacity(self.options.len());
        for (index, option) in self.options.into_iter().enumerate() {
            let option_enabled = option.is_enabled();
            let radio = ui.append(
                id,
                RadioButton::new(option.into_label())
                    .checked(Some(index) == selected_radio_group_index(ui, id))
                    .enabled(enabled && option_enabled)
                    .on_checked_changed(move |ui, event| {
                        if event.checked {
                            let _ = set_radio_group_selected_index_internal(ui, id, Some(index));
                        }
                    }),
            );
            radios.push(radio);
        }
        if let Some(selected) = selected_index {
            ui.set_local(id, ui.properties().selected_index, Some(selected));
        }
        with_radio_group_mut(ui, id, |widget, _ui| {
            widget.radios = radios;
        });
        id
    }
}

impl ElementSpec for RadioGroup {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

pub(crate) struct RadioButtonWidget {
    checked_changed: CheckedChangedSlot,
}

impl RadioButtonWidget {
    fn new(checked_changed: Option<CheckedChangedHandler>) -> Self {
        Self {
            checked_changed: CheckedChangedSlot::new(checked_changed),
        }
    }

    fn check(&self, ui: &mut Ui, radio: ElementId) -> bool {
        if radio_button_checked(ui, radio) {
            return false;
        }
        set_radio_button_checked(ui, radio, true);
        dispatch_checked_changed(
            ui,
            &self.checked_changed,
            CheckedChangedEvent {
                element: radio,
                checked: true,
            },
        );
        true
    }
}

impl core::fmt::Debug for RadioButtonWidget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RadioButtonWidget").finish_non_exhaustive()
    }
}

impl Widget for RadioButtonWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_RADIO_BUTTON)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::RadioButton)
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

    fn default_template(&self) -> Option<&'static dyn Template> {
        Some(&RADIO_BUTTON_TEMPLATE)
    }

    fn template_contract(&self) -> TemplateContract {
        TemplateContract::new(
            RADIO_BUTTON_TEMPLATE_REQUIRED_SLOTS,
            RADIO_BUTTON_TEMPLATE_RECOMMENDED_SLOTS,
        )
    }

    fn handle_click(&mut self, ctx: &mut WidgetEventContext<'_>, event: ClickEvent<'_>) {
        let radio = ctx.id();
        if !event.is_semantic_target() {
            return;
        }
        if !ctx.is_enabled(radio) {
            return;
        }
        if self.check(ctx, radio) {
            event.mark_handled();
        }
    }

    fn handle_key(&mut self, ctx: &mut WidgetEventContext<'_>, event: KeyEvent<'_>) {
        use ui_events::keyboard::{Key, KeyState, NamedKey};
        let radio = ctx.id();
        if !event.is_target() {
            return;
        }
        if event.key.state != KeyState::Down || !ctx.is_enabled(radio) {
            return;
        }
        let activate = match &event.key.key {
            Key::Named(NamedKey::Enter) => true,
            Key::Character(s) if s == " " => true,
            _ => false,
        };
        if activate && self.check(ctx, radio) {
            event.mark_handled();
        }
    }
}

pub(crate) struct RadioGroupWidget {
    options: Vec<ChoiceOption>,
    radios: Vec<ElementId>,
    selected_index: Option<usize>,
    selection_changed: RadioSelectionChangedSlot,
}

impl core::fmt::Debug for RadioGroupWidget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RadioGroupWidget")
            .field("options_len", &self.options.len())
            .field("radios", &self.radios)
            .field("selected_index", &self.selected_index)
            .finish_non_exhaustive()
    }
}

impl Widget for RadioGroupWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_RADIO_GROUP)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::RadioGroup)
    }

    fn handle_key(&mut self, ctx: &mut WidgetEventContext<'_>, event: KeyEvent<'_>) {
        use ui_events::keyboard::KeyState;
        if !matches!(event.phase, Phase::Bubble) || event.key.state != KeyState::Down {
            return;
        }
        let group = ctx.id();
        if !ctx.is_enabled(group) {
            return;
        }
        let Some(current_index) = self.radio_index(event.source) else {
            return;
        };
        let Some(next_index) = self.next_radio_index(&event.key.key, current_index) else {
            return;
        };
        if self.set_selected_index(ctx, group, Some(next_index)) {
            let _ = ctx.set_focus(Some(self.radios[next_index]));
            event.mark_handled();
        }
    }
}

impl RadioGroupWidget {
    fn radio_index(&self, radio: ElementId) -> Option<usize> {
        self.radios.iter().position(|candidate| *candidate == radio)
    }

    fn set_selected_index(&mut self, ui: &mut Ui, group: ElementId, index: Option<usize>) -> bool {
        if !ui.is_enabled(group) {
            return false;
        }
        let current = self.selected_index;
        let Some(selection) = ResolvedChoiceSelection::from_enabled_target(&self.options, index)
            .and_then(|selection| selection.changed_from(current))
        else {
            return false;
        };
        let index = selection.index;
        if let Some(previous) = self.selected_index {
            set_radio_button_checked(ui, self.radios[previous], false);
        }
        self.selected_index = index;
        ui.set_local(group, ui.properties().selected_index, index);
        let selected_radio = index.map(|index| self.radios[index]);
        if let Some(selected_radio) = selected_radio {
            set_radio_button_checked(ui, selected_radio, true);
        }
        self.selection_changed.dispatch(
            ui,
            RadioSelectionChangedEvent {
                group,
                selected_index: index,
                selected_item: selection.label,
                selected_radio,
            },
        );
        true
    }

    fn next_radio_index(&self, key: &ui_events::keyboard::Key, current: usize) -> Option<usize> {
        use ui_events::keyboard::{Key, NamedKey};
        match key {
            Key::Named(NamedKey::ArrowDown | NamedKey::ArrowRight) => {
                wrapping_next_enabled_index(&self.options, Some(current))
            }
            Key::Named(NamedKey::ArrowUp | NamedKey::ArrowLeft) => {
                wrapping_previous_enabled_index(&self.options, Some(current))
            }
            Key::Named(NamedKey::Home) => selected_enabled_index(&self.options, Some(0))
                .or_else(|| wrapping_next_enabled_index(&self.options, None)),
            Key::Named(NamedKey::End) => self
                .options
                .len()
                .checked_sub(1)
                .and_then(|index| selected_enabled_index(&self.options, Some(index)))
                .or_else(|| last_enabled_index(&self.options)),
            _ => None,
        }
    }
}

type RadioButtonTemplate = TemplateSpec;

static RADIO_BUTTON_TEMPLATE: RadioButtonTemplate = TemplateSpec::new(RADIO_BUTTON_TEMPLATE_NODES);

const RADIO_BUTTON_LABEL_BINDINGS: &[TemplateBinding] = &[
    TemplateBinding::same(TemplateBuiltInProperty::Content),
    TemplateBinding::same(TemplateBuiltInProperty::FontSize),
];

const RADIO_BUTTON_TEMPLATE_NODES: &[TemplateNode] = &[
    TemplateNode::surface_slot(SLOT_RADIO_BUTTON_INDICATOR)
        .with_children(RADIO_BUTTON_INDICATOR_CHILDREN),
    TemplateNode::for_slot(
        SLOT_RADIO_BUTTON_LABEL,
        TemplateWidgetFactory::new(radio_label_widget),
    )
    .with_bindings(RADIO_BUTTON_LABEL_BINDINGS),
];

const RADIO_BUTTON_INDICATOR_CHILDREN: &[TemplateNode] =
    &[TemplateNode::surface_slot(SLOT_RADIO_BUTTON_DOT)];

fn radio_label_widget() -> Box<dyn Widget> {
    Box::new(TextWidget)
}

/// Sets `IsChecked` on `radio` and keeps the `:checked` pseudoclass in sync.
pub fn set_radio_button_checked(ui: &mut Ui, radio: ElementId, checked: bool) {
    ui.set_local(radio, ui.properties().is_checked, checked);
    if checked {
        ui.add_pseudo(radio, PSEUDO_CHECKED);
    } else {
        ui.remove_pseudo(radio, PSEUDO_CHECKED);
    }
}

/// Returns whether `radio` is currently checked.
#[must_use]
pub fn radio_button_checked(ui: &Ui, radio: ElementId) -> bool {
    ui.get(radio)
        .and_then(|el| el.get_local_value(ui.properties().is_checked).copied())
        .unwrap_or(false)
}

/// Selects `index` in `group`, replacing any previous selected item.
///
/// Returns `false` if `group` is stale, disabled, not a `RadioGroup`, already
/// has that selection, or if `index` is outside the item data.
pub fn set_radio_group_selected_index(ui: &mut Ui, group: ElementId, index: usize) -> bool {
    set_radio_group_selected_index_internal(ui, group, Some(index))
}

/// Clears the selected item in `group`.
///
/// Returns `false` if `group` is stale, disabled, not a `RadioGroup`, or if it
/// already has no selected item.
pub fn clear_radio_group_selection(ui: &mut Ui, group: ElementId) -> bool {
    set_radio_group_selected_index_internal(ui, group, None)
}

fn set_radio_group_selected_index_internal(
    ui: &mut Ui,
    group: ElementId,
    index: Option<usize>,
) -> bool {
    with_radio_group_mut(ui, group, |widget, ui| {
        widget.set_selected_index(ui, group, index)
    })
    .unwrap_or(false)
}

/// Returns the currently selected radio index in `group`, if any.
#[must_use]
pub fn selected_radio_group_index(ui: &Ui, group: ElementId) -> Option<usize> {
    radio_group_widget(ui, group).and_then(|widget| widget.selected_index)
}

/// Returns the currently selected radio item text in `group`, if any.
#[must_use]
pub fn selected_radio_group_item(ui: &Ui, group: ElementId) -> Option<String> {
    let widget = radio_group_widget(ui, group)?;
    option_label(&widget.options, widget.selected_index)
}

fn with_radio_group_mut<R>(
    ui: &mut Ui,
    group: ElementId,
    f: impl FnOnce(&mut RadioGroupWidget, &mut Ui) -> R,
) -> Option<R> {
    let mut output = None;
    ui.with_widget_mut(group, |widget, ui| {
        if let Some(radio_group) = widget.downcast_mut::<RadioGroupWidget>() {
            output = Some(f(radio_group, ui));
        }
    });
    output
}

fn radio_group_widget(ui: &Ui, group: ElementId) -> Option<&RadioGroupWidget> {
    ui.get(group)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<RadioGroupWidget>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accessibility::accesskit_node_id;
    use accesskit::Role;
    use alloc::rc::Rc;
    use core::cell::{Cell, RefCell};
    use kurbo::{Point, Size};
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

    const ITEMS: [&str; 3] = ["Design", "Build", "Ship"];

    #[test]
    fn radio_button_click_checks_but_does_not_toggle_off() {
        let mut ui = Ui::new();
        let root = ui.root();
        let radio = ui.append(root, RadioButton::new("Build"));

        ui.synthesize_click(radio, Point::ZERO);
        ui.synthesize_click(radio, Point::ZERO);

        assert!(radio_button_checked(&ui, radio));
    }

    #[test]
    fn radio_button_checked_changed_reports_new_state() {
        let observed = Rc::new(Cell::new(None));
        let observed_for_handler = Rc::clone(&observed);
        let mut ui = Ui::new();
        let root = ui.root();
        let radio = ui.append(
            root,
            RadioButton::new("Build").on_checked_changed(move |_ui, event| {
                observed_for_handler.set(Some((event.element, event.checked)));
            }),
        );

        ui.synthesize_click(radio, Point::ZERO);

        assert_eq!(observed.get(), Some((radio, true)));
    }

    #[test]
    fn radio_group_selects_one_generated_radio() {
        let mut ui = Ui::new();
        let root = ui.root();
        let group = ui.append(root, RadioGroup::new().items(ITEMS).selected_index(1));
        let radios = radio_group_widget(&ui, group).unwrap().radios.clone();

        assert_eq!(selected_radio_group_index(&ui, group), Some(1));
        assert!(!radio_button_checked(&ui, radios[0]));
        assert!(radio_button_checked(&ui, radios[1]));
        assert!(!radio_button_checked(&ui, radios[2]));

        ui.synthesize_click(radios[2], Point::ZERO);

        assert_eq!(selected_radio_group_index(&ui, group), Some(2));
        assert!(!radio_button_checked(&ui, radios[1]));
        assert!(radio_button_checked(&ui, radios[2]));
    }

    #[test]
    fn radio_group_selection_changed_reports_semantic_item() {
        let observed = Rc::new(RefCell::new(None));
        let observed_for_handler = Rc::clone(&observed);
        let mut ui = Ui::new();
        let root = ui.root();
        let group = ui.append(
            root,
            RadioGroup::new()
                .items(ITEMS)
                .on_selection_changed(move |_ui, event| {
                    *observed_for_handler.borrow_mut() = Some(event);
                }),
        );
        let radios = radio_group_widget(&ui, group).unwrap().radios.clone();

        ui.synthesize_click(radios[1], Point::ZERO);

        assert_eq!(
            *observed.borrow(),
            Some(RadioSelectionChangedEvent {
                group,
                selected_index: Some(1),
                selected_item: Some(String::from("Build")),
                selected_radio: Some(radios[1]),
            })
        );
    }

    #[test]
    fn radio_group_selection_can_be_cleared() {
        let mut ui = Ui::new();
        let root = ui.root();
        let group = ui.append(root, RadioGroup::new().items(ITEMS).selected_index(1));
        let radios = radio_group_widget(&ui, group).unwrap().radios.clone();

        assert!(clear_radio_group_selection(&mut ui, group));

        assert_eq!(selected_radio_group_index(&ui, group), None);
        assert!(!radio_button_checked(&ui, radios[0]));
        assert!(!radio_button_checked(&ui, radios[1]));
        assert!(!radio_button_checked(&ui, radios[2]));
    }

    #[test]
    fn radio_group_arrow_keys_move_selection_and_focus() {
        let mut ui = Ui::new();
        let root = ui.root();
        let group = ui.append(root, RadioGroup::new().items(ITEMS).selected_index(0));
        let radios = radio_group_widget(&ui, group).unwrap().radios.clone();
        ui.set_focus(Some(radios[0]));

        let key = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowDown), Code::ArrowDown);
        let result = ui.dispatch_key(&key);

        assert!(result.handled);
        assert_eq!(selected_radio_group_index(&ui, group), Some(1));
        assert_eq!(ui.focused(), Some(radios[1]));
    }

    #[test]
    fn radio_group_skips_disabled_options() {
        let mut ui = Ui::new();
        let root = ui.root();
        let group = ui.append(
            root,
            RadioGroup::new()
                .options([
                    ChoiceOption::new("Draft"),
                    ChoiceOption::new("Review").disabled(),
                    ChoiceOption::new("Ready"),
                ])
                .selected_index(0),
        );
        let radios = radio_group_widget(&ui, group).unwrap().radios.clone();

        assert!(!ui.is_enabled(radios[1]));
        assert!(!set_radio_group_selected_index(&mut ui, group, 1));
        assert_eq!(selected_radio_group_index(&ui, group), Some(0));

        ui.set_focus(Some(radios[0]));
        let key = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowDown), Code::ArrowDown);
        let result = ui.dispatch_key(&key);

        assert!(result.handled);
        assert_eq!(selected_radio_group_index(&ui, group), Some(2));
        assert_eq!(ui.focused(), Some(radios[2]));
    }

    #[test]
    fn radio_accessibility_uses_radio_roles_and_checked_state() {
        let mut ui = Ui::new();
        let root = ui.root();
        let group = ui.append(root, RadioGroup::new().items(ITEMS).selected_index(0));
        ui.compute_layout(Size::new(240.0, 120.0));
        let radios = radio_group_widget(&ui, group).unwrap().radios.clone();
        let update = ui.accesskit_tree_update();

        let group_node = node_for(&update, group);
        let first_radio_node = node_for(&update, radios[0]);
        assert_eq!(group_node.role(), Role::RadioGroup);
        assert_eq!(first_radio_node.role(), Role::RadioButton);
        assert_eq!(first_radio_node.toggled(), Some(accesskit::Toggled::True));
    }

    fn node_for(update: &accesskit::TreeUpdate, id: ElementId) -> &accesskit::Node {
        let node_id = accesskit_node_id(id);
        update
            .nodes
            .iter()
            .find(|(id, _)| *id == node_id)
            .map(|(_, node)| node)
            .expect("node should be emitted")
    }
}
