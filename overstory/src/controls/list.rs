// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `ListBox` and `ListItem`: a selectable list control.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use understory_selection::Selection;
use understory_style::{PartTag, TypeTag};

use crate::Phase;
use crate::controls::text::TextWidget;
use crate::element::ElementId;
use crate::layouts::Layout;
use crate::pseudos::PSEUDO_SELECTED;
use crate::template::{
    TemplateBinding, TemplateBuiltInProperty, TemplateNode, TemplateSlot, TemplateSpec,
    TemplateWidgetFactory,
};
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, Widget};

/// Selector identity for `ListBox`.
pub const TYPE_LIST_BOX: TypeTag = TypeTag(11);
/// Selector identity for `ListItem`.
pub const TYPE_LIST_ITEM: TypeTag = TypeTag(12);
/// Owner-local part identity for the visible item label.
pub const PART_LIST_ITEM_LABEL: PartTag = PartTag(1);
/// Semantic slot for item content.
pub const SLOT_LIST_ITEM_CONTENT: TemplateSlot =
    TemplateSlot::new("content").with_part_tag(PART_LIST_ITEM_LABEL);

type SharedSelection = Rc<RefCell<Selection<ElementId>>>;

/// Selection behavior for a [`ListBox`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ListSelectionMode {
    /// Plain single-item selection.
    #[default]
    Single,
    /// Multiple selection with platform action-modifier toggles and
    /// Shift-based range extension.
    Multiple,
}

/// Builder + spec for a selectable list.
pub struct ListBox {
    size: Option<(f32, f32)>,
    state: ControlState,
    selection_mode: ListSelectionMode,
}

impl core::fmt::Debug for ListBox {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ListBox")
            .field("size", &self.size)
            .field("selection_mode", &self.selection_mode)
            .finish_non_exhaustive()
    }
}

impl ListBox {
    /// Starts a new `ListBox` builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            size: None,
            state: ControlState::interactive(),
            selection_mode: ListSelectionMode::Single,
        }
    }

    /// Sets the list's fixed logical-pixel size.
    #[must_use]
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = Some((width, height));
        self
    }

    /// Sets whether the list participates in focus traversal.
    #[must_use]
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.state = self.state.focusable(focusable);
        self
    }

    /// Sets whether the list starts enabled.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.state = self.state.enabled(enabled);
        self
    }

    /// Convenience for [`Self::enabled(false)`].
    #[must_use]
    pub fn disabled(self) -> Self {
        self.enabled(false)
    }

    /// Sets the list's selection mode.
    #[must_use]
    pub fn selection_mode(mut self, mode: ListSelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    /// Convenience for [`Self::selection_mode(ListSelectionMode::Multiple)`].
    #[must_use]
    pub fn multiple_selection(self) -> Self {
        self.selection_mode(ListSelectionMode::Multiple)
    }
}

impl Default for ListBox {
    fn default() -> Self {
        Self::new()
    }
}

impl AppendSpec for ListBox {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let selection = Rc::new(RefCell::new(Selection::new()));
        let id = ui.append_widget(
            parent,
            Box::new(ListBoxWidget {
                selection,
                selection_mode: self.selection_mode,
            }),
        );

        let mut layout = Layout::column(2.0).padding_all(4.0);
        if let Some((width, height)) = self.size {
            layout = layout.logical_size(width, height);
        }
        ui.set_layout(id, layout);
        ui.set_control_state(id, self.state);
        install_list_box_key_behavior(ui, id);
        id
    }
}

/// Per-element widget for `ListBox`.
pub struct ListBoxWidget {
    selection: SharedSelection,
    selection_mode: ListSelectionMode,
}

impl core::fmt::Debug for ListBoxWidget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ListBoxWidget").finish_non_exhaustive()
    }
}

impl Widget for ListBoxWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_LIST_BOX)
    }
}

/// Builder + spec for a selectable list item.
pub struct ListItem {
    content: String,
    font_size: f64,
    state: ControlState,
}

impl core::fmt::Debug for ListItem {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ListItem")
            .field("content", &self.content)
            .field("font_size", &self.font_size)
            .finish_non_exhaustive()
    }
}

impl ListItem {
    /// Starts a new list item with visible text content.
    #[must_use]
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            font_size: 14.0,
            state: ControlState::interactive().focusable(false),
        }
    }

    /// Overrides the item label font size.
    #[must_use]
    pub fn font_size(mut self, font_size: f64) -> Self {
        self.font_size = font_size;
        self
    }

    /// Sets whether the item starts enabled.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.state = self.state.enabled(enabled);
        self
    }

    /// Convenience for [`Self::enabled(false)`].
    #[must_use]
    pub fn disabled(self) -> Self {
        self.enabled(false)
    }
}

impl AppendSpec for ListItem {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget(parent, Box::new(ListItemWidget));
        ui.set_local(id, ui.properties().content, self.content);
        ui.set_local(id, ui.properties().font_size, self.font_size);
        ui.set_control_state(id, self.state);
        install_list_item_click_behavior(ui, id);
        id
    }
}

/// Per-element widget for `ListItem`.
#[derive(Debug, Default)]
pub struct ListItemWidget;

impl Widget for ListItemWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_LIST_ITEM)
    }

    fn default_template(&self) -> Option<&'static dyn crate::Template> {
        Some(&LIST_ITEM_TEMPLATE)
    }
}

type ListItemTemplate = TemplateSpec;

static LIST_ITEM_TEMPLATE: ListItemTemplate = TemplateSpec::new(LIST_ITEM_TEMPLATE_NODES);

const LIST_ITEM_LABEL_BINDINGS: &[TemplateBinding] = &[
    TemplateBinding::owner_to_part(
        TemplateBuiltInProperty::Content,
        TemplateBuiltInProperty::Content,
    ),
    TemplateBinding::owner_to_part(
        TemplateBuiltInProperty::FontSize,
        TemplateBuiltInProperty::FontSize,
    ),
];

const LIST_ITEM_TEMPLATE_NODES: &[TemplateNode] =
    &[
        TemplateNode::widget(TemplateWidgetFactory::new(list_item_label_widget))
            .with_slot(SLOT_LIST_ITEM_CONTENT)
            .with_bindings(LIST_ITEM_LABEL_BINDINGS),
    ];

fn list_item_label_widget() -> Box<dyn Widget> {
    Box::new(TextWidget)
}

/// Selects `item` in `listbox`, clearing any previous listbox selection.
///
/// Returns `false` if either id is stale, `item` is not a direct child of
/// `listbox`, either element is disabled, or the listbox does not own
/// selection state.
pub fn select_list_item(ui: &mut Ui, listbox: ElementId, item: ElementId) -> bool {
    if !ui.is_enabled(listbox) || !ui.is_enabled(item) || !is_direct_list_item(ui, listbox, item) {
        return false;
    }

    mutate_list_selection(ui, listbox, |selection| selection.select_only(item)).is_some()
}

/// Returns the currently selected item in `listbox`, if one is still live.
#[must_use]
pub fn selected_list_item(ui: &Ui, listbox: ElementId) -> Option<ElementId> {
    let selection = list_box_selection(ui, listbox)?;
    let selected = selection.borrow().primary().copied()?;
    is_direct_list_item(ui, listbox, selected).then_some(selected)
}

/// Returns all currently selected direct list items in `listbox`.
#[must_use]
pub fn selected_list_items(ui: &Ui, listbox: ElementId) -> Vec<ElementId> {
    let Some(selection) = list_box_selection(ui, listbox) else {
        return Vec::new();
    };

    selection
        .borrow()
        .items()
        .iter()
        .copied()
        .filter(|item| is_direct_list_item(ui, listbox, *item))
        .collect()
}

fn install_list_item_click_behavior(ui: &mut Ui, item: ElementId) {
    ui.on_click(item, move |ui, event| {
        if !matches!(event.phase, Phase::Target | Phase::Bubble) {
            return;
        }
        if !ui.is_enabled(item) {
            return;
        }
        let Some(listbox) = listbox_parent(ui, item) else {
            return;
        };
        select_list_item_for_click(
            ui,
            listbox,
            item,
            event.modifiers.shift(),
            action_modifier(event.modifiers),
        );
        let _ = ui.set_pointer_focus(Some(listbox));
        event.mark_handled();
    });
}

fn install_list_box_key_behavior(ui: &mut Ui, listbox: ElementId) {
    ui.on_key(listbox, move |ui, event| {
        use ui_events::keyboard::{Key, KeyState, NamedKey};

        if !matches!(event.phase, Phase::Target) {
            return;
        }
        if event.key.state != KeyState::Down || !ui.is_enabled(listbox) {
            return;
        }

        let items = list_items(ui, listbox);
        if items.is_empty() {
            return;
        }

        let mode = list_box_selection_mode(ui, listbox).unwrap_or_default();
        let current = selected_list_item(ui, listbox);
        let current_index =
            current.and_then(|selected| items.iter().position(|id| *id == selected));
        let action_mod = action_modifier(event.key.modifiers);
        let shift = event.key.modifiers.shift();

        if mode == ListSelectionMode::Multiple
            && action_mod
            && matches!(&event.key.key, Key::Character(s) if s.eq_ignore_ascii_case("a"))
        {
            select_all_list_items(ui, listbox);
            event.mark_handled();
            return;
        }

        let next = match &event.key.key {
            Key::Named(NamedKey::ArrowDown | NamedKey::ArrowRight) => {
                Some(next_index(current_index, items.len()))
            }
            Key::Named(NamedKey::ArrowUp | NamedKey::ArrowLeft) => {
                Some(previous_index(current_index, items.len()))
            }
            Key::Named(NamedKey::Home) => Some(0),
            Key::Named(NamedKey::End) => Some(items.len() - 1),
            Key::Named(NamedKey::Enter) => current_index,
            Key::Character(s) if s == " " => current_index,
            _ => None,
        };

        let Some(next) = next else {
            return;
        };
        let item = items[next];
        if mode == ListSelectionMode::Multiple && shift {
            select_list_item_range(ui, listbox, item, false);
        } else if mode == ListSelectionMode::Multiple
            && action_mod
            && is_activation_key(&event.key.key)
        {
            toggle_list_item_selection(ui, listbox, item);
        } else {
            select_list_item(ui, listbox, item);
        }
        event.mark_handled();
    });
}

fn next_index(current: Option<usize>, len: usize) -> usize {
    current.map(|idx| (idx + 1).min(len - 1)).unwrap_or(0)
}

fn previous_index(current: Option<usize>, len: usize) -> usize {
    current.map(|idx| idx.saturating_sub(1)).unwrap_or(len - 1)
}

fn select_list_item_for_click(
    ui: &mut Ui,
    listbox: ElementId,
    item: ElementId,
    shift: bool,
    action_mod: bool,
) -> bool {
    match list_box_selection_mode(ui, listbox).unwrap_or_default() {
        ListSelectionMode::Single => select_list_item(ui, listbox, item),
        ListSelectionMode::Multiple if shift => {
            select_list_item_range(ui, listbox, item, action_mod)
        }
        ListSelectionMode::Multiple if action_mod => toggle_list_item_selection(ui, listbox, item),
        ListSelectionMode::Multiple => select_list_item(ui, listbox, item),
    }
}

fn select_list_item_range(
    ui: &mut Ui,
    listbox: ElementId,
    item: ElementId,
    extend_existing: bool,
) -> bool {
    if !ui.is_enabled(listbox) || !ui.is_enabled(item) || !is_direct_list_item(ui, listbox, item) {
        return false;
    }

    let items = list_items(ui, listbox);
    let anchor = selection_anchor(ui, listbox)
        .filter(|anchor| items.contains(anchor))
        .unwrap_or(item);
    let Some(range) = ordered_item_range(&items, anchor, item) else {
        return false;
    };

    mutate_list_selection(ui, listbox, move |selection| {
        if extend_existing {
            selection.extend_with(range);
        } else {
            selection.replace_with(range);
        }
        selection.set_primary(&item);
        selection.set_anchor(&anchor);
    })
    .is_some()
}

fn toggle_list_item_selection(ui: &mut Ui, listbox: ElementId, item: ElementId) -> bool {
    if !ui.is_enabled(listbox) || !ui.is_enabled(item) || !is_direct_list_item(ui, listbox, item) {
        return false;
    }

    mutate_list_selection(ui, listbox, |selection| {
        selection.toggle(item);
        if selection.primary().is_none()
            && let Some(first) = selection.items().first().copied()
        {
            selection.set_primary(&first);
        }
        if selection.anchor().is_none()
            && let Some(primary) = selection.primary().copied()
        {
            selection.set_anchor(&primary);
        }
    })
    .is_some()
}

fn select_all_list_items(ui: &mut Ui, listbox: ElementId) -> bool {
    let items = list_items(ui, listbox);
    if items.is_empty() {
        return false;
    }

    mutate_list_selection(ui, listbox, move |selection| selection.replace_with(items)).is_some()
}

fn mutate_list_selection(
    ui: &mut Ui,
    listbox: ElementId,
    mutate: impl FnOnce(&mut Selection<ElementId>),
) -> Option<bool> {
    let selection = list_box_selection(ui, listbox)?;
    let (previous, previous_revision) = {
        let selection = selection.borrow();
        (selection.items().to_vec(), selection.revision())
    };

    {
        let mut selection = selection.borrow_mut();
        mutate(&mut selection);
    }

    let (current, current_revision) = {
        let selection = selection.borrow();
        (selection.items().to_vec(), selection.revision())
    };
    sync_selected_pseudos(ui, listbox, &previous, &current);
    Some(current_revision != previous_revision)
}

fn sync_selected_pseudos(
    ui: &mut Ui,
    listbox: ElementId,
    previous: &[ElementId],
    current: &[ElementId],
) {
    for selected in previous {
        if !current.contains(selected) {
            ui.remove_pseudo(*selected, PSEUDO_SELECTED);
        }
    }
    for selected in current {
        if is_direct_list_item(ui, listbox, *selected) {
            ui.add_pseudo(*selected, PSEUDO_SELECTED);
        }
    }
}

fn ordered_item_range(
    items: &[ElementId],
    anchor: ElementId,
    item: ElementId,
) -> Option<Vec<ElementId>> {
    let anchor = items.iter().position(|candidate| *candidate == anchor)?;
    let item = items.iter().position(|candidate| *candidate == item)?;
    let (start, end) = if anchor <= item {
        (anchor, item)
    } else {
        (item, anchor)
    };
    Some(items[start..=end].to_vec())
}

fn selection_anchor(ui: &Ui, listbox: ElementId) -> Option<ElementId> {
    list_box_selection(ui, listbox).and_then(|selection| selection.borrow().anchor().copied())
}

fn action_modifier(modifiers: ui_events::keyboard::Modifiers) -> bool {
    modifiers.ctrl() || modifiers.meta()
}

fn is_activation_key(key: &ui_events::keyboard::Key) -> bool {
    use ui_events::keyboard::{Key, NamedKey};

    matches!(key, Key::Named(NamedKey::Enter)) || matches!(key, Key::Character(s) if s == " ")
}

fn listbox_parent(ui: &Ui, item: ElementId) -> Option<ElementId> {
    let parent = ui.get(item)?.parent()?;
    list_box_selection(ui, parent).is_some().then_some(parent)
}

fn list_items(ui: &Ui, listbox: ElementId) -> Vec<ElementId> {
    ui.get(listbox)
        .map(|element| {
            element
                .children()
                .iter()
                .copied()
                .filter(|child| is_list_item(ui, *child) && ui.is_enabled(*child))
                .collect()
        })
        .unwrap_or_default()
}

fn is_direct_list_item(ui: &Ui, listbox: ElementId, item: ElementId) -> bool {
    ui.get(item)
        .is_some_and(|element| element.parent() == Some(listbox))
        && is_list_item(ui, item)
}

fn is_list_item(ui: &Ui, id: ElementId) -> bool {
    ui.get(id)
        .and_then(|element| element.widget())
        .and_then(Widget::type_tag)
        == Some(TYPE_LIST_ITEM)
}

fn list_box_selection(ui: &Ui, listbox: ElementId) -> Option<SharedSelection> {
    ui.get(listbox)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<ListBoxWidget>())
        .map(|widget| Rc::clone(&widget.selection))
}

fn list_box_selection_mode(ui: &Ui, listbox: ElementId) -> Option<ListSelectionMode> {
    ui.get(listbox)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<ListBoxWidget>())
        .map(|widget| widget.selection_mode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pseudos::PSEUDO_SELECTED;
    use peniko::{Brush, Color};
    use ui_events::keyboard::{Code, Key, KeyboardEvent, Modifiers, NamedKey};

    #[test]
    fn clicking_item_updates_listbox_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(root, ListBox::new());
        let first = ui.append(listbox, ListItem::new("First"));
        let second = ui.append(listbox, ListItem::new("Second"));

        ui.synthesize_click(first, kurbo::Point::ZERO);
        assert_eq!(selected_list_item(&ui, listbox), Some(first));
        assert!(ui.get(first).unwrap().pseudos().contains(&PSEUDO_SELECTED));

        ui.synthesize_click(second, kurbo::Point::ZERO);
        assert_eq!(selected_list_item(&ui, listbox), Some(second));
        assert!(!ui.get(first).unwrap().pseudos().contains(&PSEUDO_SELECTED));
        assert!(ui.get(second).unwrap().pseudos().contains(&PSEUDO_SELECTED));
    }

    #[test]
    fn action_click_toggles_multiple_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(root, ListBox::new().multiple_selection());
        let first = ui.append(listbox, ListItem::new("First"));
        let second = ui.append(listbox, ListItem::new("Second"));

        ui.synthesize_click(first, kurbo::Point::ZERO);
        ui.synthesize_click_with_modifiers(second, kurbo::Point::ZERO, Modifiers::CONTROL);
        assert_eq!(selected_list_items(&ui, listbox), vec![first, second]);
        assert!(ui.get(first).unwrap().pseudos().contains(&PSEUDO_SELECTED));
        assert!(ui.get(second).unwrap().pseudos().contains(&PSEUDO_SELECTED));

        ui.synthesize_click_with_modifiers(first, kurbo::Point::ZERO, Modifiers::CONTROL);
        assert_eq!(selected_list_items(&ui, listbox), vec![second]);
        assert!(!ui.get(first).unwrap().pseudos().contains(&PSEUDO_SELECTED));
        assert!(ui.get(second).unwrap().pseudos().contains(&PSEUDO_SELECTED));
    }

    #[test]
    fn shift_click_selects_anchor_range() {
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(root, ListBox::new().multiple_selection());
        let first = ui.append(listbox, ListItem::new("First"));
        let second = ui.append(listbox, ListItem::new("Second"));
        let third = ui.append(listbox, ListItem::new("Third"));

        ui.synthesize_click(first, kurbo::Point::ZERO);
        ui.synthesize_click_with_modifiers(third, kurbo::Point::ZERO, Modifiers::SHIFT);

        assert_eq!(
            selected_list_items(&ui, listbox),
            vec![first, second, third]
        );
        assert_eq!(selected_list_item(&ui, listbox), Some(third));
        for item in [first, second, third] {
            assert!(ui.get(item).unwrap().pseudos().contains(&PSEUDO_SELECTED));
        }
    }

    #[test]
    fn arrow_keys_move_listbox_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(root, ListBox::new());
        let first = ui.append(listbox, ListItem::new("First"));
        let second = ui.append(listbox, ListItem::new("Second"));
        let third = ui.append(listbox, ListItem::new("Third"));
        assert!(ui.set_focus(Some(listbox)));

        let down = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowDown), Code::ArrowDown);
        assert!(ui.dispatch_key(&down).handled);
        assert_eq!(selected_list_item(&ui, listbox), Some(first));

        assert!(ui.dispatch_key(&down).handled);
        assert_eq!(selected_list_item(&ui, listbox), Some(second));

        let end = KeyboardEvent::key_down(Key::Named(NamedKey::End), Code::End);
        assert!(ui.dispatch_key(&end).handled);
        assert_eq!(selected_list_item(&ui, listbox), Some(third));

        let up = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowUp), Code::ArrowUp);
        assert!(ui.dispatch_key(&up).handled);
        assert_eq!(selected_list_item(&ui, listbox), Some(second));
    }

    #[test]
    fn shift_arrow_extends_multiple_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(root, ListBox::new().multiple_selection());
        let first = ui.append(listbox, ListItem::new("First"));
        let second = ui.append(listbox, ListItem::new("Second"));
        let _third = ui.append(listbox, ListItem::new("Third"));
        assert!(ui.set_focus(Some(listbox)));

        let down = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowDown), Code::ArrowDown);
        assert!(ui.dispatch_key(&down).handled);
        assert_eq!(selected_list_items(&ui, listbox), vec![first]);

        let mut shift_down =
            KeyboardEvent::key_down(Key::Named(NamedKey::ArrowDown), Code::ArrowDown);
        shift_down.modifiers = Modifiers::SHIFT;
        assert!(ui.dispatch_key(&shift_down).handled);
        assert_eq!(selected_list_items(&ui, listbox), vec![first, second]);
        assert_eq!(selected_list_item(&ui, listbox), Some(second));
    }

    #[test]
    fn action_a_selects_all_items_in_multiple_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(root, ListBox::new().multiple_selection());
        let first = ui.append(listbox, ListItem::new("First"));
        let second = ui.append(listbox, ListItem::new("Second"));
        let third = ui.append(listbox, ListItem::new("Third"));
        assert!(ui.set_focus(Some(listbox)));

        let mut select_all = KeyboardEvent::key_down(Key::Character(String::from("a")), Code::KeyA);
        select_all.modifiers = Modifiers::CONTROL;
        assert!(ui.dispatch_key(&select_all).handled);
        assert_eq!(
            selected_list_items(&ui, listbox),
            vec![first, second, third]
        );
    }

    #[test]
    fn select_rejects_items_outside_the_listbox() {
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(root, ListBox::new());
        let outside = ui.append(root, ListItem::new("Outside"));

        assert!(!select_list_item(&mut ui, listbox, outside));
        assert_eq!(selected_list_item(&ui, listbox), None);
    }

    #[test]
    fn selected_item_matches_default_theme_selected_state() {
        let mut ui = Ui::new();
        ui.set_theme(crate::theme::default_theme());
        let root = ui.root();
        let listbox = ui.append(root, ListBox::new());
        let item = ui.append(listbox, ListItem::new("Renderer"));
        ui.set_cascade(crate::theme::default_cascade(ui.properties()));

        assert!(select_list_item(&mut ui, listbox, item));

        let background = ui.effective_value_for(ui.get(item).unwrap(), ui.properties().background);
        assert_eq!(
            background,
            Some(Brush::Solid(Color::from_rgba8(0x3a, 0x69, 0xd6, 0xff)))
        );
    }
}
