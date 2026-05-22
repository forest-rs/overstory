// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `ListBox` and `ListItem`: a selectable list control.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use kurbo::Vec2;
use understory_selection::Selection;
use understory_style::{PartTag, TypeTag};

use crate::Phase;
use crate::controls::text::TextWidget;
use crate::element::{Element, ElementId};
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
type SharedScrollOffset = Rc<Cell<f64>>;
type ListSelectionChangedHandler = Box<dyn FnMut(&mut Ui, ListSelectionChangedEvent)>;
type SharedListSelectionChangedHandler = Rc<RefCell<Option<ListSelectionChangedHandler>>>;

/// Event delivered when a [`ListBox`] selection changes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListSelectionChangedEvent {
    /// The list box whose selection changed.
    pub listbox: ElementId,
    /// The selected direct list items after the change.
    pub selected: Vec<ElementId>,
    /// The primary selected item after the change, if any.
    pub primary: Option<ElementId>,
}

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
    scroll_offset_y: f64,
    on_selection_changed: Option<ListSelectionChangedHandler>,
}

impl core::fmt::Debug for ListBox {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ListBox")
            .field("size", &self.size)
            .field("selection_mode", &self.selection_mode)
            .field("scroll_offset_y", &self.scroll_offset_y)
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
            scroll_offset_y: 0.0,
            on_selection_changed: None,
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

    /// Sets the initial vertical scroll offset in logical pixels.
    #[must_use]
    pub fn scroll_offset_y(mut self, offset: f64) -> Self {
        self.scroll_offset_y = normalize_scroll_offset(offset);
        self
    }

    /// Registers a handler fired after this list's selection changes.
    #[must_use]
    pub fn on_selection_changed<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, ListSelectionChangedEvent) + 'static,
    {
        self.on_selection_changed = Some(Box::new(handler));
        self
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
        let scroll_offset_y = Rc::new(Cell::new(self.scroll_offset_y));
        let selection_changed = Rc::new(RefCell::new(self.on_selection_changed));
        let id = ui.append_widget(
            parent,
            Box::new(ListBoxWidget {
                selection,
                scroll_offset_y,
                selection_mode: self.selection_mode,
                selection_changed,
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
    scroll_offset_y: SharedScrollOffset,
    selection_mode: ListSelectionMode,
    selection_changed: SharedListSelectionChangedHandler,
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

    fn clips_subtree(&self, element: &Element) -> bool {
        let _ = element;
        true
    }

    fn child_layout_offset(&self, element: &Element) -> Vec2 {
        let _ = element;
        Vec2::new(0.0, -self.scroll_offset_y.get())
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

/// Returns the list's current vertical scroll offset in logical pixels.
#[must_use]
pub fn list_scroll_offset_y(ui: &Ui, listbox: ElementId) -> Option<f64> {
    list_box_scroll_offset(ui, listbox).map(|scroll| scroll.get())
}

/// Sets the list's vertical scroll offset in logical pixels.
///
/// Returns `false` if `listbox` is stale or does not own list state.
pub fn set_list_scroll_offset_y(ui: &mut Ui, listbox: ElementId, offset: f64) -> bool {
    let Some(scroll) = list_box_scroll_offset(ui, listbox) else {
        return false;
    };
    let offset = normalize_scroll_offset(offset);
    if (scroll.get() - offset).abs() <= f64::EPSILON {
        return true;
    }
    scroll.set(offset);
    ui.mark_channels(listbox, crate::channels::LAYOUT.into());
    true
}

/// Scrolls `listbox` just enough to bring `item` into its current viewport.
///
/// The operation uses the most recently computed bounds, so callers should run
/// [`Ui::compute_layout`] before calling this for geometry-driven scrolling.
pub fn scroll_list_item_into_view(ui: &mut Ui, listbox: ElementId, item: ElementId) -> bool {
    if !is_direct_list_item(ui, listbox, item) {
        return false;
    }
    let Some(current) = list_scroll_offset_y(ui, listbox) else {
        return false;
    };
    let list_bounds = ui.effective_bounds(listbox);
    let item_bounds = ui.effective_bounds(item);
    if list_bounds.width() <= 0.0 || list_bounds.height() <= 0.0 || item_bounds.height() <= 0.0 {
        return false;
    }

    let next = if item_bounds.y0 < list_bounds.y0 {
        current - (list_bounds.y0 - item_bounds.y0)
    } else if item_bounds.y1 > list_bounds.y1 {
        current + (item_bounds.y1 - list_bounds.y1)
    } else {
        current
    };
    set_list_scroll_offset_y(ui, listbox, next)
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
            Key::Named(NamedKey::PageDown) => Some(page_down_index(
                current_index,
                items.len(),
                visible_item_count(ui, listbox, &items),
            )),
            Key::Named(NamedKey::PageUp) => Some(page_up_index(
                current_index,
                visible_item_count(ui, listbox, &items),
            )),
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
        scroll_list_item_into_view(ui, listbox, item);
        event.mark_handled();
    });
}

fn next_index(current: Option<usize>, len: usize) -> usize {
    current.map(|idx| (idx + 1).min(len - 1)).unwrap_or(0)
}

fn previous_index(current: Option<usize>, len: usize) -> usize {
    current.map(|idx| idx.saturating_sub(1)).unwrap_or(len - 1)
}

fn page_down_index(current: Option<usize>, len: usize, page: usize) -> usize {
    current.map(|idx| (idx + page).min(len - 1)).unwrap_or(0)
}

fn page_up_index(current: Option<usize>, page: usize) -> usize {
    current.map(|idx| idx.saturating_sub(page)).unwrap_or(0)
}

fn visible_item_count(ui: &Ui, listbox: ElementId, items: &[ElementId]) -> usize {
    let list_height = ui.effective_bounds(listbox).height();
    let item_height = items
        .iter()
        .map(|item| ui.effective_bounds(*item).height())
        .find(|height| *height > 0.0)
        .unwrap_or(0.0);
    if list_height <= 0.0 || item_height <= 0.0 {
        return 1;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "positive viewport/item ratio is clamped to at least one visible item"
    )]
    {
        (list_height / item_height).floor().max(1.0) as usize
    }
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
        if !extend_existing {
            return selection.replace_with_roles(range, Some(&item), Some(&anchor));
        }

        let mut changed = selection.extend_with(range);
        changed |= selection.set_primary(&item);
        changed |= selection.set_anchor(&anchor);
        changed
    })
    .is_some()
}

fn toggle_list_item_selection(ui: &mut Ui, listbox: ElementId, item: ElementId) -> bool {
    if !ui.is_enabled(listbox) || !ui.is_enabled(item) || !is_direct_list_item(ui, listbox, item) {
        return false;
    }

    mutate_list_selection(ui, listbox, |selection| {
        let mut changed = selection.toggle(item);
        if selection.primary().is_none()
            && let Some(first) = selection.items().first().copied()
        {
            changed |= selection.set_primary(&first);
        }
        if selection.anchor().is_none()
            && let Some(primary) = selection.primary().copied()
        {
            changed |= selection.set_anchor(&primary);
        }
        changed
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
    mutate: impl FnOnce(&mut Selection<ElementId>) -> bool,
) -> Option<bool> {
    let selection = list_box_selection(ui, listbox)?;
    let selection_changed = list_box_selection_changed_handler(ui, listbox);
    let previous = selection.borrow().items().to_vec();

    let changed = {
        let mut selection = selection.borrow_mut();
        mutate(&mut selection)
    };

    let current = selection.borrow().items().to_vec();
    sync_selected_pseudos(ui, listbox, &previous, &current);
    if changed {
        let selected = current
            .iter()
            .copied()
            .filter(|item| is_direct_list_item(ui, listbox, *item))
            .collect();
        let primary = selected_list_item(ui, listbox);
        dispatch_list_selection_changed(
            ui,
            selection_changed,
            ListSelectionChangedEvent {
                listbox,
                selected,
                primary,
            },
        );
    }
    Some(changed)
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

fn normalize_scroll_offset(offset: f64) -> f64 {
    if offset.is_finite() {
        offset.max(0.0)
    } else {
        0.0
    }
}

fn is_activation_key(key: &ui_events::keyboard::Key) -> bool {
    use ui_events::keyboard::{Key, NamedKey};

    matches!(key, Key::Named(NamedKey::Enter)) || matches!(key, Key::Character(s) if s == " ")
}

fn dispatch_list_selection_changed(
    ui: &mut Ui,
    handler: Option<SharedListSelectionChangedHandler>,
    event: ListSelectionChangedEvent,
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

fn list_box_scroll_offset(ui: &Ui, listbox: ElementId) -> Option<SharedScrollOffset> {
    ui.get(listbox)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<ListBoxWidget>())
        .map(|widget| Rc::clone(&widget.scroll_offset_y))
}

fn list_box_selection_mode(ui: &Ui, listbox: ElementId) -> Option<ListSelectionMode> {
    ui.get(listbox)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<ListBoxWidget>())
        .map(|widget| widget.selection_mode)
}

fn list_box_selection_changed_handler(
    ui: &Ui,
    listbox: ElementId,
) -> Option<SharedListSelectionChangedHandler> {
    ui.get(listbox)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<ListBoxWidget>())
        .map(|widget| Rc::clone(&widget.selection_changed))
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
    fn selection_changed_handler_observes_semantic_changes() {
        let mut ui = Ui::new();
        let root = ui.root();
        let observed: Rc<RefCell<Vec<ListSelectionChangedEvent>>> =
            Rc::new(RefCell::new(Vec::new()));
        let observed_for_handler = Rc::clone(&observed);
        let listbox = ui.append(
            root,
            ListBox::new()
                .multiple_selection()
                .on_selection_changed(move |_ui, event| {
                    observed_for_handler.borrow_mut().push(event);
                }),
        );
        let first = ui.append(listbox, ListItem::new("First"));
        let second = ui.append(listbox, ListItem::new("Second"));

        ui.synthesize_click(first, kurbo::Point::ZERO);
        ui.synthesize_click(first, kurbo::Point::ZERO);
        ui.synthesize_click_with_modifiers(second, kurbo::Point::ZERO, Modifiers::CONTROL);

        let observed = observed.borrow();
        assert_eq!(observed.len(), 2, "reselecting the same item is a no-op");
        assert_eq!(observed[0].listbox, listbox);
        assert_eq!(observed[0].selected, vec![first]);
        assert_eq!(observed[0].primary, Some(first));
        assert_eq!(observed[1].selected, vec![first, second]);
        assert_eq!(observed[1].primary, Some(second));
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
    fn list_scroll_offset_translates_children_and_clips_paint() {
        let mut ui = Ui::new();
        ui.set_theme(crate::theme::default_theme());
        ui.set_cascade(crate::theme::default_cascade(ui.properties()));
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 96.0));
        let listbox = ui.append(root, ListBox::new().size(140.0, 48.0));
        let first = ui.append(listbox, ListItem::new("First"));
        ui.append(listbox, ListItem::new("Second"));
        ui.append(listbox, ListItem::new("Third"));
        let fourth = ui.append(listbox, ListItem::new("Fourth"));

        ui.compute_layout(kurbo::Size::new(180.0, 96.0));
        let list_bounds = ui.effective_bounds(listbox);
        let first_before = ui.effective_bounds(first);
        let fourth_center = ui.effective_bounds(fourth).center();
        assert!(fourth_center.y > list_bounds.y1);
        assert_ne!(ui.hit_test(fourth_center), Some(fourth));
        assert_eq!(list_scroll_offset_y(&ui, listbox), Some(0.0));

        assert!(set_list_scroll_offset_y(&mut ui, listbox, 18.0));
        ui.compute_layout(kurbo::Size::new(180.0, 96.0));
        let first_after = ui.effective_bounds(first);
        assert!((first_after.y0 - (first_before.y0 - 18.0)).abs() <= 0.01);

        let mut scene = imaging::record::Scene::new();
        ui.paint(&mut scene);
        assert!(
            scene
                .commands()
                .iter()
                .any(|command| matches!(command, imaging::record::Command::PushClip(_)))
        );
        scene.validate().expect("list viewport clip should balance");
    }

    #[test]
    fn keyboard_selection_scrolls_item_into_view() {
        let mut ui = Ui::new();
        ui.set_theme(crate::theme::default_theme());
        ui.set_cascade(crate::theme::default_cascade(ui.properties()));
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 120.0));
        let listbox = ui.append(root, ListBox::new().size(140.0, 56.0));
        let mut items = Vec::new();
        for label in ["One", "Two", "Three", "Four", "Five", "Six"] {
            items.push(ui.append(listbox, ListItem::new(label)));
        }
        assert!(ui.set_focus(Some(listbox)));

        ui.compute_layout(kurbo::Size::new(180.0, 120.0));
        let end = KeyboardEvent::key_down(Key::Named(NamedKey::End), Code::End);
        assert!(ui.dispatch_key(&end).handled);
        assert!(list_scroll_offset_y(&ui, listbox).is_some_and(|offset| offset > 0.0));

        ui.compute_layout(kurbo::Size::new(180.0, 120.0));
        let list_bounds = ui.effective_bounds(listbox);
        let last_bounds = ui.effective_bounds(*items.last().unwrap());
        assert!(last_bounds.y0 >= list_bounds.y0 - 0.01);
        assert!(last_bounds.y1 <= list_bounds.y1 + 0.01);
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
