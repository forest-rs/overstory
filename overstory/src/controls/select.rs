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
//! - Grow mature combo-box behavior deliberately: typeahead, editable variants,
//!   async item sources, validation, accessibility roles, and explicit
//!   open/closed routed events.

use alloc::{boxed::Box, rc::Rc, string::String, vec::Vec};
use core::cell::{Cell, RefCell};

use understory_property::DependencyObjectExt;
use understory_style::{PartTag, TypeTag};

use crate::Phase;
use crate::behavior::WidgetEventContext;
use crate::controls::list::{ListBox, ListSelectionChangedEvent};
use crate::controls::popup::Popup;
use crate::controls::text::TextWidget;
use crate::element::ElementId;
use crate::event_runtime::{ClickEvent, KeyEvent};
use crate::layouts::{LayoutDimension, LayoutSize};
use crate::template::{
    Template, TemplateBinding, TemplateBuiltInProperty, TemplateNode, TemplateSlot, TemplateSpec,
    TemplateWidgetFactory,
};
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, Widget};

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

const DEFAULT_SELECT_WIDTH: f32 = 220.0;
const DEFAULT_SELECT_HEIGHT: f32 = 38.0;
const DEFAULT_SELECT_POPUP_HEIGHT: f32 = 180.0;
const DEFAULT_SELECT_ITEM_EXTENT: f64 = 30.0;
const DEFAULT_SELECT_PLACEHOLDER: &str = "Select...";
const SELECT_INDICATOR: &str = "▾";

type SelectSelectionChangedHandler = Box<dyn FnMut(&mut Ui, SelectSelectionChangedEvent) + 'static>;
type SharedSelectSelectionChangedHandler = RefCell<Option<SelectSelectionChangedHandler>>;

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
    items: Vec<String>,
    selected_index: Option<usize>,
    placeholder: String,
    width: Option<f32>,
    popup_height: f32,
    item_extent: f64,
    state: ControlState,
    on_selection_changed: Option<SelectSelectionChangedHandler>,
}

impl core::fmt::Debug for Select {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Select")
            .field("items_len", &self.items.len())
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
        Self {
            items: Vec::new(),
            selected_index: None,
            placeholder: String::from(DEFAULT_SELECT_PLACEHOLDER),
            width: None,
            popup_height: DEFAULT_SELECT_POPUP_HEIGHT,
            item_extent: DEFAULT_SELECT_ITEM_EXTENT,
            state: ControlState::interactive(),
            on_selection_changed: None,
        }
    }

    /// Replaces the select's item strings.
    #[must_use]
    pub fn items<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.items = items.into_iter().map(Into::into).collect();
        self
    }

    /// Appends one item string.
    #[must_use]
    pub fn item(mut self, item: impl Into<String>) -> Self {
        self.items.push(item.into());
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
        self.state = self.state.focusable(focusable);
        self
    }

    /// Sets whether the select starts enabled.
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
        let selected_index = self
            .selected_index
            .filter(|index| *index < self.items.len());
        let content = item_label(&self.items, selected_index, &self.placeholder);
        let width = self.width.unwrap_or(DEFAULT_SELECT_WIDTH);
        let widget = SelectWidget {
            items: self.items,
            placeholder: self.placeholder,
            popup: Rc::new(Cell::new(None)),
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
        ui.set_control_state(id, self.state);
        id
    }
}

/// Per-element widget for `Select`.
pub(crate) struct SelectWidget {
    items: Vec<String>,
    placeholder: String,
    popup: Rc<Cell<Option<ElementId>>>,
    popup_height: f32,
    item_extent: f64,
    selection_changed: SharedSelectSelectionChangedHandler,
}

impl core::fmt::Debug for SelectWidget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SelectWidget")
            .field("items_len", &self.items.len())
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

    fn default_template(&self) -> Option<&'static dyn Template> {
        Some(&SELECT_TEMPLATE)
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
        if !matches!(event.phase, Phase::Target | Phase::Bubble) {
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
        if !matches!(event.phase, Phase::Target) {
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
                let _ = self.set_selected_index(ctx, select, Some(0));
                event.mark_handled();
            }
            Key::Named(NamedKey::End) => {
                let last = self.items.len().checked_sub(1);
                let _ = self.set_selected_index(ctx, select, last);
                event.mark_handled();
            }
            _ => {}
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
        if self.items.is_empty() || ui.get(select).is_none() {
            return;
        }
        let popup_cell = Rc::clone(&self.popup);
        let popup_height = self.popup_list_height();
        let popup_width = self.popup_width(ui, select);
        let item_extent = self.item_extent;
        let selected_index =
            selected_select_index_local(ui, select).filter(|index| *index < self.items.len());
        let items = self.items.clone();
        let popup = ui.open_popup(
            select,
            Popup::new().padding(0.0, 0.0).offset(0.0, 4.0),
            |ui, popup| {
                let _list = ui.append(
                    popup,
                    ListBox::new()
                        .items(items)
                        .size(popup_width, popup_height)
                        .item_extent(item_extent)
                        .selected_index(selected_index.unwrap_or(usize::MAX))
                        .on_selection_changed(select_commit_handler(select, popup_cell)),
                );
            },
        );
        self.popup.set(popup);
    }

    fn close_popup(&self, ui: &mut Ui) -> bool {
        let Some(popup) = self.popup.get() else {
            return false;
        };
        self.popup.set(None);
        ui.close_popup(popup)
    }

    fn move_selection(&mut self, ui: &mut Ui, select: ElementId, direction: i8) {
        if self.items.is_empty() {
            return;
        }
        let current =
            selected_select_index_local(ui, select).filter(|index| *index < self.items.len());
        let next = match (current, direction.is_positive()) {
            (Some(index), true) => (index + 1).min(self.items.len() - 1),
            (Some(index), false) => index.saturating_sub(1),
            (None, true) => 0,
            (None, false) => self.items.len() - 1,
        };
        let _ = self.set_selected_index(ui, select, Some(next));
    }

    fn set_selected_index(&mut self, ui: &mut Ui, select: ElementId, index: Option<usize>) -> bool {
        if !ui.is_enabled(select) {
            return false;
        }
        let index = index.filter(|index| *index < self.items.len());
        let current =
            selected_select_index_local(ui, select).filter(|index| *index < self.items.len());
        if current == index {
            return false;
        }
        let item = index.and_then(|index| self.items.get(index).cloned());
        let content = item.clone().unwrap_or_else(|| self.placeholder.clone());
        let props = *ui.properties();
        ui.set_local(select, props.selected_index, index);
        ui.set_local(select, props.content, content);
        dispatch_select_selection_changed(
            ui,
            Some(&self.selection_changed),
            SelectSelectionChangedEvent {
                select,
                selected_index: index,
                selected_item: item,
            },
        );
        true
    }

    fn popup_list_height(&self) -> f32 {
        if self.items.is_empty() {
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
        let item_count = self.items.len() as f64;
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
    selected_select_index_local(ui, select).filter(|index| *index < widget.items.len())
}

/// Returns the currently selected item text in `select`, if any.
#[must_use]
pub fn selected_select_item(ui: &Ui, select: ElementId) -> Option<String> {
    let index = selected_select_index(ui, select)?;
    select_widget(ui, select)?.items.get(index).cloned()
}

fn select_commit_handler(
    select: ElementId,
    popup_cell: Rc<Cell<Option<ElementId>>>,
) -> impl FnMut(&mut Ui, ListSelectionChangedEvent) + 'static {
    move |ui, event| {
        let Some(index) = event.primary_index else {
            return;
        };
        if set_select_index(ui, select, index) {
            if let Some(popup) = popup_cell.get() {
                popup_cell.set(None);
                let _ = ui.close_popup(popup);
            }
            let _ = ui.set_focus(Some(select));
        }
    }
}

fn dispatch_select_selection_changed(
    ui: &mut Ui,
    handler: Option<&SharedSelectSelectionChangedHandler>,
    event: SelectSelectionChangedEvent,
) {
    let Some(handler) = handler else {
        return;
    };
    let Some(mut callback) = handler.borrow_mut().take() else {
        return;
    };
    callback(ui, event);
    let should_restore = handler.borrow().is_none();
    if should_restore {
        *handler.borrow_mut() = Some(callback);
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

fn item_label(items: &[String], index: Option<usize>, placeholder: &str) -> String {
    index
        .and_then(|index| items.get(index))
        .cloned()
        .unwrap_or_else(|| String::from(placeholder))
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
    use crate::controls::list::{realized_list_item, selected_list_index};
    use alloc::rc::Rc;
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
        realize(&mut ui);

        ui.synthesize_click(select, Point::ZERO);
        realize(&mut ui);

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

    fn test_select() -> Select {
        Select::new().items(ITEMS).width(180.0).popup_height(92.0)
    }

    fn realize(ui: &mut Ui) {
        ui.compute_layout(Size::new(400.0, 300.0));
    }
}
