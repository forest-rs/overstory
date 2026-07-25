// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Select`: collapsed single-choice control backed by `Popup` + `ListBox`.
//!
//! Roadmap:
//! - Keep `Select` as a composition of ordinary Overstory controls: the
//!   collapsed owner owns the selected item and activation behavior, while the
//!   opened popup delegates list realization and selection to `ListBox`.
//! - Keep richer popup item visuals on the `ListBox` item-template path instead
//!   of inventing a separate menu-item visual model.
//! - Owner keyboard and programmatic changes commit immediately. While open,
//!   the popup mirrors that committed selection and scrolls it into view without
//!   reporting a second `ListBox` selection event. Activating a popup item commits
//!   and dismisses; there is no separate preview selection state yet.
//! - Move string/index-only selection toward shared typed option keys with
//!   `RadioGroup`, `ListBox`, and segmented selection. `Select` should not grow
//!   a private item model just because it is the first popup-backed picker.
//! - Grow mature combo-box behavior deliberately: typeahead, editable variants,
//!   async item sources, validation, accessibility roles, and explicit
//!   open/closed routed events.

use alloc::{boxed::Box, string::String, vec::Vec};

use understory_overlay::OverlayBehavior;
use understory_property::DependencyObjectExt;
use understory_style::{PartTag, TypeTag};

use crate::behavior::{ArrangeContext, ArrangeResult, WidgetEventContext};
use crate::controls::choice::{
    ChoiceOption, ResolvedChoiceSelection, collect_options, first_enabled_index,
    last_enabled_index, next_enabled_index, normalized_selection_target, option_label,
    previous_enabled_index, selected_enabled_index,
};
use crate::controls::list::{
    ListBox, ListItemContext, ListSelectionChangedEvent, SharedListItemTemplate,
    synchronize_list_selection,
};
use crate::controls::popup::Popup;
use crate::controls::support::{CallbackSlot, EventHandler};
use crate::element::{Element, ElementId};
use crate::event_runtime::{ClickEvent, KeyEvent};
use crate::layouts::{LayoutDimension, LayoutSize};
use crate::template::{
    TemplateContract, TemplateNode, TemplateSlot, TemplateSpec, TemplateSpecRecipe,
};
use crate::ui::{ChildUi, Ui};
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

const DEFAULT_SELECT_WIDTH: f64 = 220.0;
const DEFAULT_SELECT_HEIGHT: f64 = 38.0;
const DEFAULT_SELECT_POPUP_HEIGHT: f64 = 180.0;
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
    item_template: Option<SharedListItemTemplate>,
    selected_index: Option<usize>,
    placeholder: String,
    width: Option<f64>,
    popup_height: f64,
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
        element.set_focusable(true);
        Self {
            options: Vec::new(),
            item_template: None,
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

    /// Sets a retained-child template for realized popup rows.
    ///
    /// `Select` uses the same item-template path as [`ListBox`], so popup rows
    /// can have richer retained content without giving `Select` a private menu
    /// item model. Selection and commit behavior remain owned by the popup list
    /// item container.
    ///
    /// Treat this closure as a repeatable visual recipe. Row-specific mutable
    /// behavior should live in row controls, bindings, or explicit state rather
    /// than hidden mutation inside the template closure.
    #[must_use]
    pub fn item_template<F>(mut self, template: F) -> Self
    where
        F: for<'ui, 'item> Fn(&mut ChildUi<'ui>, ListItemContext<'item>) + 'static,
    {
        self.item_template = Some(SharedListItemTemplate::new(template));
        self
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
    pub fn width(mut self, width: f64) -> Self {
        self.width = Some(width.max(1.0));
        self
    }

    /// Sets the maximum popup list height.
    #[must_use]
    pub fn popup_height(mut self, height: f64) -> Self {
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
        self.element.set_focusable(focusable);
        self
    }

    /// Sets whether the select starts enabled.
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
            item_template: self.item_template,
            selected_index,
            placeholder: self.placeholder,
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
    item_template: Option<SharedListItemTemplate>,
    selected_index: Option<usize>,
    placeholder: String,
    popup_height: f64,
    item_extent: f64,
    selection_changed: SharedSelectSelectionChangedHandler,
}

impl core::fmt::Debug for SelectWidget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SelectWidget")
            .field("options_len", &self.options.len())
            .field("has_item_template", &self.item_template.is_some())
            .field("selected_index", &self.selected_index)
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
        let open = ctx.has_active_overlay_for(element.id(), OverlayBehavior::ComboboxPopup);
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

    fn template_contract(&self) -> TemplateContract {
        TemplateContract::new()
            .with_required_slot(SLOT_SELECT_LABEL)
            .with_recommended_slot(SLOT_SELECT_INDICATOR)
    }

    fn template_expanded(&mut self, select: ElementId, ui: &mut Ui) {
        if let Some(indicator) = ui.slot(select, SLOT_SELECT_INDICATOR) {
            ui.set_local(
                indicator,
                ui.properties().text,
                String::from(SELECT_INDICATOR),
            );
        }
    }

    fn arrange(&mut self, ctx: &mut ArrangeContext<'_>) -> ArrangeResult {
        let select = ctx.id();
        let requested = selected_select_index_local(ctx.ui(), select);
        ArrangeResult::from_layout_changed(self.set_selected_index(ctx.ui_mut(), select, requested))
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
            Key::Named(NamedKey::Escape) if self.close_popup(ctx, select) => {
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
                select_popup(ctx, select).is_some() || self.open_popup(ctx, select)
            }
            accesskit::Action::Collapse => self.close_popup(ctx, select),
            _ => false,
        }
    }
}

impl SelectWidget {
    fn toggle_popup(&self, ui: &mut Ui, select: ElementId) {
        if self.close_popup(ui, select) {
            return;
        }
        self.open_popup(ui, select);
    }

    fn open_popup(&self, ui: &mut Ui, select: ElementId) -> bool {
        if select_popup(ui, select).is_some() || self.options.is_empty() || ui.get(select).is_none()
        {
            return false;
        }
        let popup_height = self.popup_list_height();
        let popup_width = self.popup_width(ui, select);
        let item_extent = self.item_extent;
        let item_template = self.item_template.clone();
        let selected_index = self.selected_index;
        let options = self.options.clone();
        ui.open_popup_with_behavior(
            select,
            Popup::new().padding(0.0, 0.0).offset(0.0, 4.0),
            OverlayBehavior::ComboboxPopup,
            |ui, popup| {
                let mut list = ListBox::new()
                    .options(options)
                    .size(popup_width, popup_height)
                    .item_extent(item_extent)
                    .on_selection_changed(select_commit_handler(select));
                if let Some(template) = item_template {
                    list = list.shared_item_template(template);
                }
                let listbox = ui.append(popup, list);
                synchronize_list_selection(ui, listbox, selected_index);
            },
        )
        .is_some()
    }

    fn close_popup(&self, ui: &mut Ui, select: ElementId) -> bool {
        let Some(popup) = select_popup(ui, select) else {
            return false;
        };
        ui.close_popup(popup)
    }

    fn move_selection(&mut self, ui: &mut Ui, select: ElementId, direction: i8) {
        if self.options.is_empty() {
            return;
        }
        let current = self.selected_index;
        let next = match direction.is_positive() {
            true => next_enabled_index(&self.options, current),
            false => previous_enabled_index(&self.options, current),
        };
        let _ = self.set_selected_index(ui, select, next);
    }

    fn set_selected_index(&mut self, ui: &mut Ui, select: ElementId, index: Option<usize>) -> bool {
        let current = self.selected_index;
        let target = normalized_selection_target(&self.options, index, current);
        if target == current {
            if index != current {
                ui.set_local(select, ui.properties().selected_index, current);
            }
            return false;
        };
        let Some(selection) = ResolvedChoiceSelection::from_enabled_target(&self.options, target)
            .and_then(|selection| selection.changed_from(current))
        else {
            return false;
        };
        let content = selection
            .label
            .clone()
            .unwrap_or_else(|| self.placeholder.clone());
        let props = *ui.properties();
        self.selected_index = selection.index;
        ui.set_local(select, props.selected_index, selection.index);
        ui.set_local(select, props.content, content);
        self.synchronize_popup_selection(ui, select, selection.index);
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

    fn synchronize_popup_selection(&self, ui: &mut Ui, select: ElementId, index: Option<usize>) {
        let Some(listbox) = select_popup(ui, select)
            .and_then(|popup| ui.get(popup))
            .and_then(|popup| popup.children().first())
            .copied()
        else {
            return;
        };
        synchronize_list_selection(ui, listbox, index);
    }

    fn popup_list_height(&self) -> f64 {
        if self.options.is_empty() {
            return self.item_extent;
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "item counts that can fit in a popup preserve enough precision for height"
        )]
        let item_count = self.options.len() as f64;
        let content_height = self.item_extent * item_count;
        content_height.min(self.popup_height).max(1.0)
    }

    fn popup_width(&self, ui: &Ui, select: ElementId) -> f64 {
        let width = ui.effective_bounds(select).width();
        if width.is_finite() && width > 0.0 {
            width
        } else {
            DEFAULT_SELECT_WIDTH
        }
    }
}

fn select_popup(ui: &Ui, select: ElementId) -> Option<ElementId> {
    ui.active_overlay_for(select, OverlayBehavior::ComboboxPopup)
}

static SELECT_TEMPLATE: TemplateSpecRecipe = TemplateSpecRecipe::new(select_template_spec);

pub(crate) fn default_select_template() -> crate::ControlTemplate {
    crate::ControlTemplate::from_static(&SELECT_TEMPLATE)
}

fn select_template_spec() -> TemplateSpec {
    TemplateSpec::from_nodes([
        TemplateNode::content_presenter(SLOT_SELECT_LABEL),
        TemplateNode::text_slot(SLOT_SELECT_INDICATOR),
    ])
}

/// Selects `index` in `select`, replacing any previous selected item.
///
/// Returns `false` if `select` is stale, not a `Select`, already has that
/// selection, or if `index` is not a selectable item.
pub fn set_select_index(ui: &mut Ui, select: ElementId, index: usize) -> bool {
    with_select_widget_mut(ui, select, |widget, ui| {
        widget.set_selected_index(ui, select, Some(index))
    })
    .unwrap_or(false)
}

/// Clears the current selection for `select`.
///
/// Returns `false` if `select` is stale, not a `Select`, or already has no
/// selected item.
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
    widget.selected_index
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
            let _ = widget.close_popup(ui, select);
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
        handler.dispatch(ui, event.select, event);
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
    use crate::controls::list::{list_scroll_offset_y, realized_list_item, selected_list_index};
    use crate::pseudos::PSEUDO_SELECTED;
    use crate::test_support::pointer_down;
    use accesskit::{Action, ActionRequest, HasPopup, Node, Role, TreeId, TreeUpdate};
    use alloc::rc::Rc;
    use core::cell::{Cell, RefCell};
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
    fn direct_selected_index_write_updates_collapsed_content() {
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(root, test_select().selected_index(0));
        compute_layout(&mut ui);

        ui.set_local(select, ui.properties().selected_index, Some(2));
        compute_layout(&mut ui);

        assert_eq!(selected_select_index(&ui, select), Some(2));
        assert_eq!(
            ui.get(select)
                .and_then(|element| element.get_local_value(ui.properties().content))
                .map(String::as_str),
            Some("Ready")
        );
    }

    #[test]
    fn invalid_direct_selected_index_write_repairs_property() {
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(root, test_select().selected_index(0));
        compute_layout(&mut ui);

        ui.set_local(select, ui.properties().selected_index, Some(99));
        compute_layout(&mut ui);

        assert_eq!(selected_select_index(&ui, select), Some(0));
        assert_eq!(
            ui.get(select)
                .and_then(|element| element.get_local_value(ui.properties().selected_index))
                .copied(),
            Some(Some(0))
        );
        assert_eq!(
            ui.get(select)
                .and_then(|element| element.get_local_value(ui.properties().content))
                .map(String::as_str),
            Some("Draft")
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

        let popup = select_popup(&ui, select).unwrap();
        let listbox = ui.get(popup).unwrap().children()[0];
        assert_eq!(selected_list_index(&ui, listbox), Some(0));
        let third = realized_list_item(&ui, listbox, 2).unwrap();

        ui.synthesize_click(third, Point::ZERO);

        assert_eq!(selected_select_index(&ui, select), Some(2));
        assert!(ui.get(popup).is_none());
    }

    #[test]
    fn popup_item_template_child_click_commits_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(
            root,
            test_select().item_template(|row, item| {
                row.add(crate::Text::new(item.label()));
            }),
        );
        compute_layout(&mut ui);

        ui.synthesize_click(select, Point::ZERO);
        compute_layout(&mut ui);

        let popup = select_popup(&ui, select).unwrap();
        let listbox = ui.get(popup).unwrap().children()[0];
        let third = realized_list_item(&ui, listbox, 2).unwrap();
        let slot = ui
            .slot(third, crate::controls::list::SLOT_LIST_ITEM_CONTENT)
            .unwrap();
        let text = ui.get(slot).unwrap().children()[0];

        ui.synthesize_click(text, Point::ZERO);

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
    fn arrow_key_keeps_open_popup_selection_in_sync() {
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(root, test_select().selected_index(0));
        compute_layout(&mut ui);
        ui.synthesize_click(select, Point::ZERO);
        compute_layout(&mut ui);
        let popup = select_popup(&ui, select).unwrap();
        let listbox = ui.get(popup).unwrap().children()[0];
        ui.set_focus(Some(select));

        let down = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowDown), Code::ArrowDown);
        assert!(ui.dispatch_key(&down).handled);

        assert_eq!(selected_select_index(&ui, select), Some(1));
        assert_eq!(selected_list_index(&ui, listbox), Some(1));
    }

    #[test]
    fn escape_dismissal_clears_expansion_and_allows_reopening() {
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(root, test_select().selected_index(0));
        let (popup, _listbox) = open_test_popup(&mut ui, select);
        ui.set_focus(Some(select));

        let escape = KeyboardEvent::key_down(Key::Named(NamedKey::Escape), Code::Escape);
        assert!(ui.dispatch_key(&escape).handled);

        assert!(ui.get(popup).is_none());
        assert!(select_popup(&ui, select).is_none());
        let update = ui.accesskit_tree_update();
        assert_eq!(
            accesskit_node_for(&update, select).is_expanded(),
            Some(false)
        );

        assert!(ui.handle_accesskit_action(&action(select, Action::Expand)));
        let reopened = select_popup(&ui, select).unwrap();
        assert_ne!(reopened, popup);
        assert!(ui.get(reopened).is_some());
    }

    #[test]
    fn open_popup_tracks_programmatic_and_property_selection_writes() {
        let changes = Rc::new(Cell::new(0));
        let changes_for_handler = Rc::clone(&changes);
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(
            root,
            test_select()
                .selected_index(0)
                .on_selection_changed(move |_ui, _event| {
                    changes_for_handler.set(changes_for_handler.get() + 1);
                }),
        );
        let (popup, listbox) = open_test_popup(&mut ui, select);

        assert!(set_select_index(&mut ui, select, 2));
        assert_eq!(selected_list_index(&ui, listbox), Some(2));
        assert_eq!(changes.get(), 1);
        assert!(ui.get(popup).is_some());

        ui.set_local(select, ui.properties().selected_index, Some(3));
        compute_layout(&mut ui);
        assert_eq!(selected_select_index(&ui, select), Some(3));
        assert_eq!(selected_list_index(&ui, listbox), Some(3));
        assert_eq!(changes.get(), 2);
        assert!(ui.get(popup).is_some());
    }

    #[test]
    fn opening_long_select_realizes_and_reveals_committed_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(
            root,
            Select::new()
                .items((0..50).map(|index| alloc::format!("Item {index}")))
                .selected_index(40)
                .width(180.0)
                .popup_height(90.0)
                .item_extent(30.0),
        );

        let (_popup, listbox) = open_test_popup(&mut ui, select);

        assert_eq!(selected_list_index(&ui, listbox), Some(40));
        assert!(list_scroll_offset_y(&ui, listbox).is_some_and(|offset| offset > 0.0));
        let selected = realized_list_item(&ui, listbox, 40).unwrap();
        assert!(
            ui.get(selected)
                .unwrap()
                .pseudos()
                .contains(&PSEUDO_SELECTED)
        );
        assert!(realized_list_item(&ui, listbox, 0).is_none());
    }

    #[test]
    fn accesskit_popup_item_activation_commits_and_dismisses() {
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(root, test_select().selected_index(0));
        let (popup, listbox) = open_test_popup(&mut ui, select);
        let third = realized_list_item(&ui, listbox, 2).unwrap();

        assert!(ui.handle_accesskit_action(&action(third, Action::Click)));

        assert_eq!(selected_select_index(&ui, select), Some(2));
        assert!(ui.get(popup).is_none());
        assert!(select_popup(&ui, select).is_none());
    }

    #[test]
    fn outside_pointer_dismissal_clears_select_expansion() {
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(root, test_select().selected_index(0));
        let (popup, _listbox) = open_test_popup(&mut ui, select);

        let result = ui.dispatch_pointer_event(&pointer_down(Point::new(350.0, 250.0), 1));

        assert!(result.overlay_changed);
        assert!(ui.get(popup).is_none());
        assert!(select_popup(&ui, select).is_none());
        let update = ui.accesskit_tree_update();
        assert_eq!(
            accesskit_node_for(&update, select).is_expanded(),
            Some(false)
        );
    }

    #[test]
    fn direct_popup_close_clears_select_expansion() {
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(root, test_select().selected_index(0));
        let (popup, _listbox) = open_test_popup(&mut ui, select);

        assert!(ui.close_popup(popup));

        assert!(ui.get(popup).is_none());
        assert!(select_popup(&ui, select).is_none());
        assert!(!ui.close_popup(popup));
    }

    #[test]
    fn removing_popup_root_clears_select_expansion() {
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(root, test_select().selected_index(0));
        let (popup, _listbox) = open_test_popup(&mut ui, select);

        ui.remove(popup);

        assert!(select_popup(&ui, select).is_none());
        assert!(ui.handle_accesskit_action(&action(select, Action::Expand)));
        assert!(select_popup(&ui, select).is_some());
    }

    #[test]
    fn disabling_select_dismisses_popup_and_reenable_can_reopen() {
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(root, test_select().selected_index(0));
        let (popup, _listbox) = open_test_popup(&mut ui, select);

        ui.set_enabled(select, false);

        assert!(ui.get(popup).is_none());
        assert!(select_popup(&ui, select).is_none());
        assert!(!ui.handle_accesskit_action(&action(select, Action::Expand)));

        ui.set_enabled(select, true);
        assert!(ui.handle_accesskit_action(&action(select, Action::Expand)));
        assert!(select_popup(&ui, select).is_some());
    }

    #[test]
    fn focus_outside_dismisses_select_popup_but_owner_and_list_focus_do_not() {
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(root, test_select().selected_index(0));
        let outside = ui.append(root, crate::Button::new("Other"));
        let (popup, listbox) = open_test_popup(&mut ui, select);

        ui.set_focus(Some(select));
        assert!(ui.get(popup).is_some());
        ui.set_focus(Some(listbox));
        assert!(ui.get(popup).is_some());

        ui.set_focus(Some(outside));

        assert!(ui.get(popup).is_none());
        assert!(select_popup(&ui, select).is_none());
        assert_eq!(ui.focused(), Some(outside));
        let update = ui.accesskit_tree_update();
        assert_eq!(
            accesskit_node_for(&update, select).is_expanded(),
            Some(false)
        );
    }

    #[test]
    fn collapsing_select_dismisses_popup_and_visible_select_can_reopen() {
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(root, test_select().selected_index(0));
        let (popup, _listbox) = open_test_popup(&mut ui, select);

        ui.set_visibility(select, crate::Visibility::Collapsed);

        assert!(ui.get(popup).is_none());
        assert!(select_popup(&ui, select).is_none());
        assert!(!ui.handle_accesskit_action(&action(select, Action::Expand)));
        ui.set_visibility(select, crate::Visibility::Visible);
        assert!(ui.handle_accesskit_action(&action(select, Action::Expand)));
        assert!(select_popup(&ui, select).is_some());
    }

    #[test]
    fn collapsing_anchor_ancestor_dismisses_popup_without_restoring_hidden_focus() {
        let mut ui = Ui::new();
        let root = ui.root();
        let container = ui.insert_child(root);
        let select = ui.append(container, test_select().selected_index(0));
        let (popup, listbox) = open_test_popup(&mut ui, select);
        assert!(ui.set_focus(Some(listbox)));

        ui.set_visibility(container, crate::Visibility::Collapsed);

        assert!(ui.get(popup).is_none());
        assert!(select_popup(&ui, select).is_none());
        assert_eq!(ui.focused(), None);
        assert!(!ui.set_focus(Some(select)));
    }

    #[test]
    fn removing_select_removes_its_popup() {
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(root, test_select().selected_index(0));
        let (popup, _listbox) = open_test_popup(&mut ui, select);

        ui.remove(select);

        assert!(ui.get(select).is_none());
        assert!(ui.get(popup).is_none());
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

        compute_layout(&mut ui);
        let (_popup, listbox) = open_test_popup(&mut ui, select);
        assert!(ui.dispatch_key(&down).handled);
        assert_eq!(selected_select_index(&ui, select), Some(2));
        assert_eq!(selected_list_index(&ui, listbox), Some(2));
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
    fn selection_changed_handler_can_query_and_mutate_same_select() {
        let select_id = Rc::new(Cell::new(None));
        let select_id_for_handler = Rc::clone(&select_id);
        let callback_observation = Rc::new(RefCell::new(None));
        let callback_observation_for_handler = Rc::clone(&callback_observation);
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(
            root,
            test_select().on_selection_changed(move |ui, event| {
                if event.selected_index != Some(1) {
                    return;
                }
                let select = select_id_for_handler
                    .get()
                    .expect("select id is installed before selection changes");
                *callback_observation_for_handler.borrow_mut() = Some((
                    selected_select_index(ui, select),
                    set_select_index(ui, select, 2),
                ));
            }),
        );
        select_id.set(Some(select));

        assert!(set_select_index(&mut ui, select, 1));

        assert_eq!(*callback_observation.borrow(), Some((Some(1), true)));
        assert_eq!(selected_select_index(&ui, select), Some(2));
    }

    #[test]
    fn selection_changed_handler_can_remove_and_replace_same_select() {
        let select_id = Rc::new(Cell::new(None));
        let select_id_for_handler = Rc::clone(&select_id);
        let replacement_id = Rc::new(Cell::new(None));
        let replacement_id_for_handler = Rc::clone(&replacement_id);
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(
            root,
            test_select().on_selection_changed(move |ui, _event| {
                let select = select_id_for_handler
                    .get()
                    .expect("select id is installed before selection changes");
                assert!(ui.get(select).and_then(Element::widget).is_some());
                ui.remove(select);
                replacement_id_for_handler
                    .set(Some(ui.append(root, test_select().selected_index(2))));
            }),
        );
        select_id.set(Some(select));

        assert!(set_select_index(&mut ui, select, 1));

        let replacement = replacement_id
            .get()
            .expect("handler should append a replacement");
        assert!(ui.get(select).is_none());
        assert_ne!(replacement, select);
        assert_eq!(selected_select_index(&ui, replacement), Some(2));
    }

    #[test]
    #[should_panic(expected = "semantic callback dispatch did not converge")]
    fn cyclic_selection_callbacks_fail_at_the_semantic_budget() {
        let select_id = Rc::new(Cell::new(None));
        let select_id_for_handler = Rc::clone(&select_id);
        let mut ui = Ui::new();
        let root = ui.root();
        let select = ui.append(
            root,
            test_select().on_selection_changed(move |ui, event| {
                let select = select_id_for_handler
                    .get()
                    .expect("select id is installed before selection changes");
                let next = if event.selected_index == Some(1) {
                    2
                } else {
                    1
                };
                assert!(set_select_index(ui, select, next));
            }),
        );
        select_id.set(Some(select));

        let _ = set_select_index(&mut ui, select, 1);
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

    fn open_test_popup(ui: &mut Ui, select: ElementId) -> (ElementId, ElementId) {
        compute_layout(ui);
        ui.synthesize_click(select, Point::ZERO);
        compute_layout(ui);
        let popup = select_popup(ui, select).unwrap();
        let listbox = ui.get(popup).unwrap().children()[0];
        (popup, listbox)
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
