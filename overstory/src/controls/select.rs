// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Select`: collapsed single-choice control backed by `Popup` + `ListBox`.
//!
//! Roadmap:
//! - Keep `Select` as a composition of ordinary Overstory controls: the
//!   collapsed owner owns the selected item and activation behavior, while the
//!   opened popup delegates list realization and selection to `ListBox`.
//! - Add richer item templating through the existing template/slot system
//!   instead of inventing a separate menu-item visual model.
//! - Move string/index-only selection toward shared typed option keys with
//!   `RadioGroup`, `ListBox`, and segmented selection. `Select` should not grow
//!   a private item model just because it is the first popup-backed picker.
//! - Grow mature combo-box behavior deliberately: typeahead, editable variants,
//!   async item sources, validation, accessibility roles, and explicit
//!   open/closed routed events.

use alloc::{boxed::Box, string::String, vec::Vec};

use understory_property::DependencyObjectExt;
use understory_style::{PartTag, TypeTag};

use crate::behavior::WidgetEventContext;
use crate::controls::choice::{
    ChoiceOption, ResolvedChoiceSelection, collect_options, first_enabled_index,
    last_enabled_index, next_enabled_index, option_label, previous_enabled_index,
    selected_enabled_index,
};
use crate::controls::list::{ListBox, ListSelectionChangedEvent};
use crate::controls::popup::Popup;
use crate::controls::support::{CallbackSlot, ElementSlot, EventHandler};
use crate::controls::text::TextWidget;
use crate::element::{Element, ElementId};
use crate::event_runtime::{ClickEvent, KeyEvent};
use crate::layouts::{LayoutDimension, LayoutSize};
use crate::template::{
    Template, TemplateBinding, TemplateBuiltInProperty, TemplateContract, TemplateNode,
    TemplateSlot, TemplateSpec, TemplateWidgetFactory,
};
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, ElementConfig, ElementSpec, Widget};

/// Selector identity for `Select`.
pub const TYPE_SELECT: TypeTag = TypeTag(18);
/// Owner-local part identity for the collapsed selected-item label.
pub const PART_SELECT_LABEL: PartTag = PartTag(1);
/// Owner-local part identity for the collapsed dropdown indicator.
pub const PART_SELECT_INDICATOR: PartTag = PartTag(2);
/// Semantic slot for the collapsed selected-item label.
pub const SLOT_SELECT_LABEL: TemplateSlot =
    TemplateSlot::new("select-label").with_part_tag(PART_SELECT_LABEL);
/// Semantic slot for the collapsed dropdown indicator.
pub const SLOT_SELECT_INDICATOR: TemplateSlot =
    TemplateSlot::new("select-indicator").with_part_tag(PART_SELECT_INDICATOR);

const SELECT_TEMPLATE_REQUIRED_SLOTS: &[TemplateSlot] = &[SLOT_SELECT_LABEL];
const SELECT_TEMPLATE_RECOMMENDED_SLOTS: &[TemplateSlot] = &[SLOT_SELECT_INDICATOR];

const DEFAULT_SELECT_WIDTH: f32 = 220.0;
const DEFAULT_SELECT_HEIGHT: f32 = 38.0;
const DEFAULT_SELECT_POPUP_HEIGHT: f32 = 180.0;
const DEFAULT_SELECT_ITEM_EXTENT: f64 = 30.0;
const DEFAULT_SELECT_PLACEHOLDER: &str = "Select...";
const SELECT_INDICATOR: &str = "▾";

type SelectSelectionChangedHandler = EventHandler<SelectSelectionChangedEvent>;
type SharedSelectSelectionChangedHandler = CallbackSlot<SelectSelectionChangedEvent>;

/// Selection change payload emitted by [`Select`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectSelectionChangedEvent {
    /// The `Select` element whose primary selection changed.
    pub select: ElementId,
    /// The selected item index, or `None` when selection is cleared.
    pub selected_index: Option<usize>,
    /// The selected item text, or `None` when selection is cleared.
    pub selected_item: Option<String>,
}

/// Builder + spec for a collapsed single-choice `Select`.
pub struct Select {
    options: Vec<ChoiceOption>,
    selected_index: Option<usize>,
    placeholder: String,
    width: Option<f32>,
    popup_height: f32,
    item_extent: f64,
    element: ElementConfig,
    on_selection_changed: Option<SelectSelectionChangedHandler>,
}

impl core::fmt::Debug for Select {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Select")
            .field("options_len", &self.options.len())
            .field("selected_index", &self.selected_index)
            .field("width", &self.width)
            .field("popup_height", &self.popup_height)
            .field("item_extent", &self.item_extent)
            .finish_non_exhaustive()
    }
}

impl Select {
    /// Starts a new empty `Select` builder.
    #[must_use]
    pub fn new() -> Self {
        let mut element = ElementConfig::default();
        element.set_control_state(ControlState::interactive());
        Self {
            options: Vec::new(),
            selected_index: None,
            placeholder: String::from(DEFAULT_SELECT_PLACEHOLDER),
            width: None,
            popup_height: DEFAULT_SELECT_POPUP_HEIGHT,
            item_extent: DEFAULT_SELECT_ITEM_EXTENT,
            element,
            on_selection_changed: None,
        }
    }

    /// Replaces the select's options.
    #[must_use]
    pub fn options<I, O>(mut self, options: I) -> Self
    where
        I: IntoIterator<Item = O>,
        O: Into<ChoiceOption>,
    {
        self.options = collect_options(options);
        self
    }

    /// Replaces the select's item labels.
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

    /// Sets the placeholder displayed when no item is selected.
    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Sets the collapsed select width.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width.max(1.0));
        self
    }

    /// Sets the maximum popup list height.
    #[must_use]
    pub fn popup_height(mut self, height: f32) -> Self {
        self.popup_height = height.max(1.0);
        self
    }

    /// Sets the uniform height for popup list items.
    #[must_use]
    pub fn item_extent(mut self, extent: f64) -> Self {
        self.item_extent = normalize_item_extent(extent);
        self
    }

    /// Sets whether the select participates in focus traversal.
    #[must_use]
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.element
            .update_control_state(|state| state.focusable(focusable));
        self
    }

    /// Sets whether the select starts enabled.
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
        F: FnMut(&mut Ui, SelectSelectionChangedEvent) + 'static,
    {
        self.on_selection_changed = Some(Box::new(handler));
        self
    }
}

impl Default for Select {
    fn default() -> Self {
        Self::new()
    }
}

impl AppendSpec for Select {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let selected_index = selected_enabled_index(&self.options, self.selected_index);
        let content = item_label(&self.options, selected_index, &self.placeholder);
        let width = self.width.unwrap_or(DEFAULT_SELECT_WIDTH);
        let widget = SelectWidget {
            options: self.options,
            placeholder: self.placeholder,
            popup: ElementSlot::new(),
            popup_height: self.popup_height,
            item_extent: self.item_extent,
            selection_changed: SharedSelectSelectionChangedHandler::new(self.on_selection_changed),
        };

        let id = ui.append_widget(parent, Box::new(widget));
        let props = *ui.properties();
        ui.set_local(id, props.content, content);
        ui.set_local(id, props.selected_index, selected_index);
        if self.width.is_some() {
            ui.set_local(
                id,
                props.layout_size,
                LayoutSize::new()
                    .width(LayoutDimension::Logical(width))
                    .height(LayoutDimension::Logical(DEFAULT_SELECT_HEIGHT)),
            );
        }
        self.element.apply_to(ui, id);
        id
    }
}

impl ElementSpec for Select {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

/// Per-element widget for `Select`.
pub(crate) struct SelectWidget {
    options: Vec<ChoiceOption>,
    placeholder: String,
    popup: ElementSlot,
    popup_height: f32,
    item_extent: f64,
    selection_changed: SharedSelectSelectionChangedHandler,
}

impl core::fmt::Debug for SelectWidget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SelectWidget")
            .field("options_len", &self.options.len())
            .field("popup", &self.popup.get())
            .field("popup_height", &self.popup_height)
            .field("item_extent", &self.item_extent)
            .finish_non_exhaustive()
    }
}

impl Widget for SelectWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_SELECT)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::ComboBox)
    }

    fn configure_accessibility(
        &self,
        element: &Element,
        ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        node.set_value(ctx.content(element));
        node.set_has_popup(accesskit::HasPopup::Listbox);
        let open = self.popup.get().is_some();
        node.set_expanded(open);
        if ctx.is_enabled(element.id()) {
            ctx.add_click_action_if_enabled(element, node);
            if open {
                node.add_action(accesskit::Action::Collapse);
            } else {
                node.add_action(accesskit::Action::Expand);
            }
        }
    }

    fn default_template(&self) -> Option<&'static dyn Template> {
        Some(&SELECT_TEMPLATE)
    }

    fn template_contract(&self) -> TemplateContract {
        TemplateContract::new(
            SELECT_TEMPLATE_REQUIRED_SLOTS,
            SELECT_TEMPLATE_RECOMMENDED_SLOTS,
        )
    }

    fn template_expanded(&mut self, select: ElementId, ui: &mut Ui) {
        if let Some(indicator) = ui.slot(select, SLOT_SELECT_INDICATOR) {
            ui.set_local(
                indicator,
                ui.properties().content,
                String::from(SELECT_INDICATOR),
            );
        }
    }

    fn handle_click(&mut self, ctx: &mut WidgetEventContext<'_>, event: ClickEvent<'_>) {
        let select = ctx.id();
        if !event.is_semantic_target() {
            return;
        }
        if !ctx.is_enabled(select) {
            return;
        }
        self.toggle_popup(ctx, select);
        event.mark_handled();
    }

    fn handle_key(&mut self, ctx: &mut WidgetEventContext<'_>, event: KeyEvent<'_>) {
        use ui_events::keyboard::{Key, KeyState, NamedKey};

        let select = ctx.id();
        if !event.is_target() {
            return;
        }
        if event.key.state != KeyState::Down || !ctx.is_enabled(select) {
            return;
        }

        let activate = matches!(&event.key.key, Key::Named(NamedKey::Enter))
            || matches!(&event.key.key, Key::Character(s) if s == " ");

        if activate {
            self.toggle_popup(ctx, select);
            event.mark_handled();
            return;
        }

        match &event.key.key {
            Key::Named(NamedKey::Escape) if self.close_popup(ctx) => {
                event.mark_handled();
            }
            Key::Named(NamedKey::Escape) => {}
            Key::Named(NamedKey::ArrowDown | NamedKey::ArrowRight) => {
                self.move_selection(ctx, select, 1);
                event.mark_handled();
            }
            Key::Named(NamedKey::ArrowUp | NamedKey::ArrowLeft) => {
                self.move_selection(ctx, select, -1);
                event.mark_handled();
            }
            Key::Named(NamedKey::Home) => {
                let _ = self.set_selected_index(ctx, select, first_enabled_index(&self.options));
                event.mark_handled();
            }
            Key::Named(NamedKey::End) => {
                let _ = self.set_selected_index(ctx, select, last_enabled_index(&self.options));
                event.mark_handled();
            }
            _ => {}
        }
    }

    fn handle_accessibility_action(
        &mut self,
        ctx: &mut WidgetEventContext<'_>,
        request: &accesskit::ActionRequest,
    ) -> bool {
        let select = ctx.id();
        if !ctx.is_enabled(select) {
            return false;
        }
        match request.action {
            accesskit::Action::Expand => {
                if self.popup.get().is_none() {
                    self.open_popup(ctx, select);
                }
                true
            }
            accesskit::Action::Collapse => self.close_popup(ctx),
            _ => false,
        }
    }
}

impl SelectWidget {
    fn toggle_popup(&self, ui: &mut Ui, select: ElementId) {
        if self.close_popup(ui) {
            return;
        }
        self.open_popup(ui, select);
    }

    fn open_popup(&self, ui: &mut Ui, select: ElementId) {
        if self.options.is_empty() || ui.get(select).is_none() {
            return;
        }
        let popup_height = self.popup_list_height();
        let popup_width = self.popup_width(ui, select);
        let item_extent = self.item_extent;
        let selected_index =
            selected_enabled_index(&self.options, selected_select_index_local(ui, select));
        let options = self.options.clone();
        let popup = ui.open_popup(
            select,
            Popup::new().padding(0.0, 0.0).offset(0.0, 4.0),
            |ui, popup| {
                let _list = ui.append(
                    popup,
                    ListBox::new()
                        .options(options)
                        .size(popup_width, popup_height)
                        .item_extent(item_extent)
                        .selected_index(selected_index.unwrap_or(usize::MAX))
                        .on_selection_changed(select_commit_handler(select)),
                );
            },
        );
        self.popup.set_option(popup);
    }

    fn close_popup(&self, ui: &mut Ui) -> bool {
        let Some(popup) = self.popup.get() else {
            return false;
        };
        self.popup.clear();
        ui.close_popup(popup)
    }

    fn move_selection(&mut self, ui: &mut Ui, select: ElementId, direction: i8) {
        if self.options.is_empty() {
            return;
        }
        let current =
            selected_enabled_index(&self.options, selected_select_index_local(ui, select));
        let next = match direction.is_positive() {
            true => next_enabled_index(&self.options, current),
            false => previous_enabled_index(&self.options, current),
        };
        let _ = self.set_selected_index(ui, select, next);
    }

    fn set_selected_index(&mut self, ui: &mut Ui, select: ElementId, index: Option<usize>) -> bool {
        if !ui.is_enabled(select) {
            return false;
        }
        let current =
            selected_enabled_index(&self.options, selected_select_index_local(ui, select));
        let Some(selection) = ResolvedChoiceSelection::from_enabled_target(&self.options, index)
            .and_then(|selection| selection.changed_from(current))
        else {
            return false;
        };
        let content = selection
            .label
            .clone()
            .unwrap_or_else(|| self.placeholder.clone());
        let props = *ui.properties();
        ui.set_local(select, props.selected_index, selection.index);
        ui.set_local(select, props.content, content);
        dispatch_select_selection_changed(
            ui,
            Some(&self.selection_changed),
            SelectSelectionChangedEvent {
                select,
                selected_index: selection.index,
                selected_item: selection.label,
            },
        );
        true
    }

    fn popup_list_height(&self) -> f32 {
        if self.options.is_empty() {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "normalized list item extent is used only as a fallback f32 height"
            )]
            {
                return self.item_extent as f32;
            }
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "item counts that can fit in a popup preserve enough precision for height"
        )]
        let item_count = self.options.len() as f64;
        let content_height = self.item_extent * item_count;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "popup list height is clamped to the configured f32 size"
        )]
        {
            content_height.min(f64::from(self.popup_height)).max(1.0) as f32
        }
    }

    fn popup_width(&self, ui: &Ui, select: ElementId) -> f32 {
        let width = ui.effective_bounds(select).width();
        if width.is_finite() && width > 0.0 {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "select bounds are layout-space f64 values that fit f32 style inputs"
            )]
            {
                width as f32
            }
        } else {
            DEFAULT_SELECT_WIDTH
        }
    }
}

type SelectTemplate = TemplateSpec;

static SELECT_TEMPLATE: SelectTemplate = TemplateSpec::new(SELECT_TEMPLATE_NODES);

const SELECT_LABEL_BINDINGS: &[TemplateBinding] = &[
    TemplateBinding::owner_to_part(
        TemplateBuiltInProperty::Content,
        TemplateBuiltInProperty::Content,
    ),
    TemplateBinding::owner_to_part(
        TemplateBuiltInProperty::FontSize,
        TemplateBuiltInProperty::FontSize,
    ),
];

const SELECT_TEMPLATE_NODES: &[TemplateNode] = &[
    TemplateNode::widget(TemplateWidgetFactory::new(select_label_widget))
        .with_slot(SLOT_SELECT_LABEL)
        .with_bindings(SELECT_LABEL_BINDINGS),
    TemplateNode::widget(TemplateWidgetFactory::new(select_indicator_widget))
        .with_slot(SLOT_SELECT_INDICATOR),
];

fn select_label_widget() -> Box<dyn Widget> {
    Box::new(TextWidget)
}

fn select_indicator_widget() -> Box<dyn Widget> {
    Box::new(TextWidget)
}

/// Selects `index` in `select`, replacing any previous selected item.
///
/// Returns `false` if `select` is stale, disabled, not a `Select`, or if the
/// index is outside the item data.
pub fn set_select_index(ui: &mut Ui, select: ElementId, index: usize) -> bool {
    with_select_widget_mut(ui, select, |widget, ui| {
        widget.set_selected_index(ui, select, Some(index))
    })
    .unwrap_or(false)
}

/// Clears the current selection for `select`.
///
/// Returns `false` if `select` is stale, disabled, not a `Select`, or already
/// has no selected item.
pub fn clear_select_selection(ui: &mut Ui, select: ElementId) -> bool {
    with_select_widget_mut(ui, select, |widget, ui| {
        widget.set_selected_index(ui, select, None)
    })
    .unwrap_or(false)
}

/// Returns the currently selected item index in `select`, if any.
#[must_use]
pub fn selected_select_index(ui: &Ui, select: ElementId) -> Option<usize> {
    let widget = select_widget(ui, select)?;
    selected_enabled_index(&widget.options, selected_select_index_local(ui, select))
}

/// Returns the currently selected item text in `select`, if any.
#[must_use]
pub fn selected_select_item(ui: &Ui, select: ElementId) -> Option<String> {
    let widget = select_widget(ui, select)?;
    option_label(&widget.options, selected_select_index(ui, select))
}

fn select_commit_handler(
    select: ElementId,
) -> impl FnMut(&mut Ui, ListSelectionChangedEvent) + 'static {
    move |ui, event| {
        let Some(index) = event.primary_index else {
            return;
        };
        commit_select_popup_selection(ui, select, index);
    }
}

fn commit_select_popup_selection(ui: &mut Ui, select: ElementId, index: usize) -> bool {
    with_select_widget_mut(ui, select, |widget, ui| {
        let changed = widget.set_selected_index(ui, select, Some(index));
        if changed {
            let _ = widget.close_popup(ui);
            let _ = ui.set_focus(Some(select));
        }
        changed
    })
    .unwrap_or(false)
}

fn dispatch_select_selection_changed(
    ui: &mut Ui,
    handler: Option<&SharedSelectSelectionChangedHandler>,
    event: SelectSelectionChangedEvent,
) {
    if let Some(handler) = handler {
        handler.dispatch(ui, event);
    }
}

fn select_widget(ui: &Ui, select: ElementId) -> Option<&SelectWidget> {
    ui.get(select)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<SelectWidget>())
}

fn selected_select_index_local(ui: &Ui, select: ElementId) -> Option<usize> {
    ui.get(select)
        .and_then(|element| {
            element
                .get_local_value(ui.properties().selected_index)
                .copied()
        })
        .flatten()
}

fn with_select_widget_mut<R>(
    ui: &mut Ui,
    select: ElementId,
    f: impl FnOnce(&mut SelectWidget, &mut Ui) -> R,
) -> Option<R> {
    let mut output = None;
    ui.with_widget_mut(select, |widget, ui| {
        if let Some(select_widget) = widget.downcast_mut::<SelectWidget>() {
            output = Some(f(select_widget, ui));
        }
    });
    output
}

fn item_label(options: &[ChoiceOption], index: Option<usize>, placeholder: &str) -> String {
    option_label(options, index).unwrap_or_else(|| String::from(placeholder))
}

fn normalize_item_extent(extent: f64) -> f64 {
    if extent.is_finite() {
        extent.max(1.0)
    } else {
        DEFAULT_SELECT_ITEM_EXTENT
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accessibility::accesskit_node_id;
    use crate::controls::list::{realized_list_item, selected_list_index};
    use accesskit::{Action, ActionRequest, HasPopup, Node, Role, TreeId, TreeUpdate};
    use alloc::rc::Rc;
    use core::cell::RefCell;
    use kurbo::{Point, Size};
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

    const ITEMS: [&str; 4] = ["Draft", "Review", "Ready", "Archived"];

    #[test]
    fn select_stores_initial_selected_index_as_semantic_state() {
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(root, test_select().selected_index(1));

        assert_eq!(selected_select_index(&ui, select), Some(1));
        assert_eq!(selected_select_item(&ui, select).as_deref(), Some("Review"));
        assert_eq!(
            ui.get(select)
                .and_then(|element| element.get_local_value(ui.properties().content))
                .map(String::as_str),
            Some("Review")
        );
    }

    #[test]
    fn clicking_popup_item_commits_selection_and_closes_popup() {
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(root, test_select().selected_index(0));
        compute_layout(&mut ui);

        ui.synthesize_click(select, Point::ZERO);
        compute_layout(&mut ui);

        let popup = select_widget(&ui, select).unwrap().popup.get().unwrap();
        let listbox = ui.get(popup).unwrap().children()[0];
        assert_eq!(selected_list_index(&ui, listbox), Some(0));
        let third = realized_list_item(&ui, listbox, 2).unwrap();

        ui.synthesize_click(third, Point::ZERO);

        assert_eq!(selected_select_index(&ui, select), Some(2));
        assert!(ui.get(popup).is_none());
    }

    #[test]
    fn select_arrow_keys_change_collapsed_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(root, test_select());
        ui.set_focus(Some(select));

        let down = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowDown), Code::ArrowDown);
        assert!(ui.dispatch_key(&down).handled);
        assert_eq!(selected_select_index(&ui, select), Some(0));
        assert!(ui.dispatch_key(&down).handled);
        assert_eq!(selected_select_index(&ui, select), Some(1));

        let end = KeyboardEvent::key_down(Key::Named(NamedKey::End), Code::End);
        assert!(ui.dispatch_key(&end).handled);
        assert_eq!(selected_select_index(&ui, select), Some(ITEMS.len() - 1));
    }

    #[test]
    fn disabled_options_are_skipped_by_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(
            root,
            Select::new()
                .options([
                    ChoiceOption::new("Draft"),
                    ChoiceOption::new("Review").disabled(),
                    ChoiceOption::new("Ready"),
                ])
                .selected_index(1),
        );

        assert_eq!(selected_select_index(&ui, select), None);
        assert!(!set_select_index(&mut ui, select, 1));

        ui.set_focus(Some(select));
        let down = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowDown), Code::ArrowDown);
        assert!(ui.dispatch_key(&down).handled);
        assert_eq!(selected_select_index(&ui, select), Some(0));
        assert!(ui.dispatch_key(&down).handled);
        assert_eq!(selected_select_index(&ui, select), Some(2));
    }

    #[test]
    fn select_selection_changed_handler_receives_item_text() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let seen_for_handler = Rc::clone(&seen);
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(
            root,
            test_select().on_selection_changed(move |_ui, event| {
                seen_for_handler
                    .borrow_mut()
                    .push((event.selected_index, event.selected_item));
            }),
        );

        assert!(set_select_index(&mut ui, select, 2));

        assert_eq!(
            seen.borrow().as_slice(),
            &[(Some(2), Some(String::from("Ready")))]
        );
    }

    #[test]
    fn select_accessibility_exposes_value_popup_state_and_expand_actions() {
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(root, test_select().selected_index(1));
        compute_layout(&mut ui);

        let update = ui.accesskit_tree_update();
        let node = accesskit_node_for(&update, select);
        assert_eq!(node.role(), Role::ComboBox);
        assert_eq!(node.value(), Some("Review"));
        assert_eq!(node.has_popup(), Some(HasPopup::Listbox));
        assert_eq!(node.is_expanded(), Some(false));
        assert!(node.supports_action(Action::Click));
        assert!(node.supports_action(Action::Expand));

        let expanded = ui.handle_accesskit_action(&action(select, Action::Expand));
        compute_layout(&mut ui);
        let update = ui.accesskit_tree_update();
        let node = accesskit_node_for(&update, select);
        assert!(expanded);
        assert_eq!(node.is_expanded(), Some(true));
        assert!(node.supports_action(Action::Collapse));

        let collapsed = ui.handle_accesskit_action(&action(select, Action::Collapse));
        compute_layout(&mut ui);
        let update = ui.accesskit_tree_update();
        let node = accesskit_node_for(&update, select);
        assert!(collapsed);
        assert_eq!(node.is_expanded(), Some(false));
    }

    fn test_select() -> Select {
        Select::new().items(ITEMS).width(180.0).popup_height(92.0)
    }

    fn compute_layout(ui: &mut Ui) {
        ui.compute_layout(Size::new(400.0, 300.0));
    }

    fn action(element: ElementId, action: Action) -> ActionRequest {
        ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: accesskit_node_id(element),
            data: None,
        }
    }

    fn accesskit_node_for(update: &TreeUpdate, element: ElementId) -> &Node {
        update
            .nodes
            .iter()
            .find(|(id, _)| *id == accesskit_node_id(element))
            .map(|(_, node)| node)
            .expect("node should exist")
    }
}
