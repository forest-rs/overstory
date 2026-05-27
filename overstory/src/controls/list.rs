// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `ListBox` and realized list item containers.
//!
//! Roadmap:
//! - Keep lists virtualized by default, with `understory_virtual_list` owning
//!   viewport math and Overstory owning realized elements and interaction.
//! - Replace string-only items with keyed item data and item templates so the
//!   control can grow toward `ItemsControl` / `ListView` patterns.
//! - Keep selection backed by `understory_selection`: focus, current item,
//!   anchor, primary selection, and realized containers must not drift apart.
//! - Add mature keyboard behavior: typeahead, Home / End / Page movement,
//!   platform multi-select conventions, and clear focus-vs-selection semantics.
//! - Integrate reusable scrolling, scrollbars, accessibility, and eventually
//!   composition-backed virtualization without making apps assemble that by hand.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::ops::Range;

use kurbo::{Size, Vec2};
use understory_property::DependencyObjectExt;
use understory_selection::Selection;
use understory_style::{PartTag, TypeTag};
use understory_virtual_list::{FixedExtentModel, ScrollAlign, VirtualList};

use crate::Phase;
use crate::behavior::WidgetEventContext;
use crate::channels::LAYOUT;
use crate::controls::scroll::{
    ScrollState, normalize_scroll_offset_y, scroll_offset_y, wheel_scroll_delta,
};
use crate::controls::support::f32_from_logical;
use crate::controls::text::TextWidget;
use crate::element::{Element, ElementId};
use crate::event_runtime::{ClickEvent, KeyEvent, ScrollEvent};
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
/// Selector identity for realized `ListBox` item containers.
pub const TYPE_LIST_ITEM: TypeTag = TypeTag(12);
/// Owner-local part identity for the visible item label.
pub const PART_LIST_ITEM_LABEL: PartTag = PartTag(1);
/// Semantic slot for item content.
pub const SLOT_LIST_ITEM_CONTENT: TemplateSlot =
    TemplateSlot::new("content").with_part_tag(PART_LIST_ITEM_LABEL);

const DEFAULT_LIST_BOX_WIDTH: f32 = 220.0;
const DEFAULT_LIST_BOX_HEIGHT: f32 = 160.0;
const DEFAULT_LIST_ITEM_EXTENT: f64 = 28.0;
const DEFAULT_LIST_OVERSCAN: f64 = 56.0;

type SharedSelection = Rc<RefCell<Selection<usize>>>;
type ListSelectionChangedHandler = Box<dyn FnMut(&mut Ui, ListSelectionChangedEvent)>;
type SharedListSelectionChangedHandler = Rc<RefCell<Option<ListSelectionChangedHandler>>>;
type ListVirtualizer = VirtualList<FixedExtentModel<f64>>;

/// Event delivered when a [`ListBox`] selection changes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListSelectionChangedEvent {
    /// The list box whose selection changed.
    pub listbox: ElementId,
    /// The selected item indices after the change.
    pub selected_indices: Vec<usize>,
    /// The primary selected item index after the change, if any.
    pub primary_index: Option<usize>,
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

/// Builder + spec for a virtualized selectable list.
///
/// `ListBox` owns its item data and realizes only the visible/overscanned item
/// containers. Selection and keyboard navigation are expressed in stable item
/// indices rather than transient realized [`ElementId`] values.
pub struct ListBox {
    items: Vec<String>,
    size: Option<(f32, f32)>,
    item_extent: f64,
    overscan: f64,
    state: ControlState,
    selection_mode: ListSelectionMode,
    selected_index: Option<usize>,
    scroll_offset_y: f64,
    scroll_state: Option<ScrollState>,
    on_selection_changed: Option<ListSelectionChangedHandler>,
}

impl core::fmt::Debug for ListBox {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ListBox")
            .field("items_len", &self.items.len())
            .field("size", &self.size)
            .field("item_extent", &self.item_extent)
            .field("overscan", &self.overscan)
            .field("selection_mode", &self.selection_mode)
            .field("selected_index", &self.selected_index)
            .field("scroll_offset_y", &self.scroll_offset_y)
            .finish_non_exhaustive()
    }
}

impl ListBox {
    /// Starts a new empty `ListBox` builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            size: None,
            item_extent: DEFAULT_LIST_ITEM_EXTENT,
            overscan: DEFAULT_LIST_OVERSCAN,
            state: ControlState::interactive(),
            selection_mode: ListSelectionMode::Single,
            selected_index: None,
            scroll_offset_y: 0.0,
            scroll_state: None,
            on_selection_changed: None,
        }
    }

    /// Replaces the list's item strings.
    #[must_use]
    pub fn items<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.items = items.into_iter().map(Into::into).collect();
        self
    }

    /// Appends one item string to the list.
    #[must_use]
    pub fn item(mut self, item: impl Into<String>) -> Self {
        self.items.push(item.into());
        self
    }

    /// Sets the list's fixed logical-pixel size.
    #[must_use]
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = Some((width, height));
        self
    }

    /// Sets the uniform logical-pixel height of each realized item.
    #[must_use]
    pub fn item_extent(mut self, extent: f64) -> Self {
        self.item_extent = normalize_item_extent(extent);
        self
    }

    /// Sets the logical-pixel overscan before and after the viewport.
    #[must_use]
    pub fn overscan(mut self, overscan: f64) -> Self {
        self.overscan = normalize_scroll_offset_y(overscan);
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

    /// Convenience for [`Self::enabled`] with `false`.
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

    /// Convenience for [`Self::selection_mode`] with
    /// [`ListSelectionMode::Multiple`].
    #[must_use]
    pub fn multiple_selection(self) -> Self {
        self.selection_mode(ListSelectionMode::Multiple)
    }

    /// Sets the initially selected item index.
    #[must_use]
    pub fn selected_index(mut self, index: usize) -> Self {
        self.selected_index = Some(index);
        self
    }

    /// Sets the initial vertical scroll offset in logical pixels.
    #[must_use]
    pub fn scroll_offset_y(mut self, offset: f64) -> Self {
        self.scroll_offset_y = normalize_scroll_offset_y(offset);
        if let Some(state) = &self.scroll_state {
            state.set_offset_y_raw(self.scroll_offset_y);
        }
        self
    }

    /// Uses caller-provided shared scroll state.
    #[must_use]
    pub fn scroll_state(mut self, state: ScrollState) -> Self {
        state.set_offset_y_raw(self.scroll_offset_y);
        self.scroll_state = Some(state);
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
        let mut selection = Selection::new();
        if let Some(index) = self
            .selected_index
            .filter(|index| *index < self.items.len())
        {
            let _ = selection.select_only(index);
        }

        let scroll_state = self
            .scroll_state
            .unwrap_or_else(|| ScrollState::with_offset_y(self.scroll_offset_y));
        let mut virtual_list = VirtualList::new(
            FixedExtentModel::new(self.items.len(), self.item_extent),
            0.0,
            self.overscan,
        );
        virtual_list.set_scroll_offset(self.scroll_offset_y);

        let id = ui.append_widget(
            parent,
            Box::new(ListBoxWidget {
                items: self.items,
                selection: Rc::new(RefCell::new(selection)),
                scroll_state: scroll_state.clone(),
                selection_mode: self.selection_mode,
                selection_changed: Rc::new(RefCell::new(self.on_selection_changed)),
                virtual_list,
                item_extent: self.item_extent,
                realized: Vec::new(),
                last_viewport: Size::ZERO,
            }),
        );

        let (width, height) = self
            .size
            .unwrap_or((DEFAULT_LIST_BOX_WIDTH, DEFAULT_LIST_BOX_HEIGHT));
        ui.set_layout(id, Layout::new().relative().logical_size(width, height));
        ui.set_control_state(id, self.state);
        id
    }
}

/// Per-element widget for `ListBox`.
pub(crate) struct ListBoxWidget {
    items: Vec<String>,
    selection: SharedSelection,
    scroll_state: ScrollState,
    selection_mode: ListSelectionMode,
    selection_changed: SharedListSelectionChangedHandler,
    virtual_list: ListVirtualizer,
    item_extent: f64,
    realized: Vec<RealizedListItem>,
    last_viewport: Size,
}

impl core::fmt::Debug for ListBoxWidget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ListBoxWidget")
            .field("items_len", &self.items.len())
            .field("selection_mode", &self.selection_mode)
            .field("item_extent", &self.item_extent)
            .field("realized", &self.realized)
            .finish_non_exhaustive()
    }
}

impl Widget for ListBoxWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_LIST_BOX)
    }

    fn clips_subtree(&self, _element: &Element) -> bool {
        true
    }

    fn child_layout_offset(&self, _element: &Element) -> Vec2 {
        -self.scroll_state.offset()
    }

    fn post_layout(&mut self, listbox: ElementId, ui: &mut Ui) -> bool {
        self.sync_virtual_items(listbox, ui)
    }

    fn handle_scroll(&mut self, ctx: &mut WidgetEventContext<'_>, event: ScrollEvent<'_>) {
        let listbox = ctx.id();
        let Some(delta) = wheel_scroll_delta(ctx, listbox, event) else {
            return;
        };
        if delta.y != 0.0 {
            self.scroll_by(listbox, ctx, delta.y);
            event.mark_handled();
        }
    }

    fn handle_key(&mut self, ctx: &mut WidgetEventContext<'_>, event: KeyEvent<'_>) {
        use ui_events::keyboard::{Key, KeyState, NamedKey};
        let listbox = ctx.id();

        if !matches!(event.phase, Phase::Target) {
            return;
        }
        if event.key.state != KeyState::Down || !ctx.is_enabled(listbox) {
            return;
        }
        if self.items.is_empty() {
            return;
        }

        let current = self.selected_index();
        let action_mod = action_modifier(event.key.modifiers);
        let shift = event.key.modifiers.shift();

        if self.selection_mode == ListSelectionMode::Multiple
            && action_mod
            && matches!(&event.key.key, Key::Character(s) if s.eq_ignore_ascii_case("a"))
        {
            self.select_all(ctx, listbox);
            event.mark_handled();
            return;
        }

        let next = match &event.key.key {
            Key::Named(NamedKey::ArrowDown | NamedKey::ArrowRight) => {
                Some(next_index(current, self.items.len()))
            }
            Key::Named(NamedKey::ArrowUp | NamedKey::ArrowLeft) => {
                Some(previous_index(current, self.items.len()))
            }
            Key::Named(NamedKey::PageDown) => Some(page_down_index(
                current,
                self.items.len(),
                self.visible_item_count(),
            )),
            Key::Named(NamedKey::PageUp) => Some(page_up_index(current, self.visible_item_count())),
            Key::Named(NamedKey::Home) => Some(0),
            Key::Named(NamedKey::End) => Some(self.items.len() - 1),
            Key::Named(NamedKey::Enter) => current,
            Key::Character(s) if s == " " => current,
            _ => None,
        };

        let Some(next) = next else {
            return;
        };
        if self.selection_mode == ListSelectionMode::Multiple && shift {
            self.select_range(ctx, listbox, next, false);
        } else if self.selection_mode == ListSelectionMode::Multiple
            && action_mod
            && is_activation_key(&event.key.key)
        {
            self.toggle_index(ctx, listbox, next);
        } else {
            self.select_index(ctx, listbox, next);
        }
        self.scroll_index_into_view(listbox, ctx, next);
        event.mark_handled();
    }
}

impl ListBoxWidget {
    fn sync_virtual_items(&mut self, listbox: ElementId, ui: &mut Ui) -> bool {
        let sync = self.sync_virtual_metrics(listbox, ui);
        let mut changed = sync.changed;
        changed |= self.reconcile_realized_items(listbox, ui, sync.range);
        changed |= self.layout_realized_items(ui, sync.viewport);
        self.sync_realized_selected_pseudos(ui);
        changed
    }

    fn sync_virtual_metrics(&mut self, listbox: ElementId, ui: &mut Ui) -> ListVirtualSync {
        let bounds = ui.effective_bounds(listbox);
        let viewport = Size::new(bounds.width().max(0.0), bounds.height().max(0.0));
        let viewport_changed = !sizes_nearly_equal(viewport, self.last_viewport);
        self.last_viewport = viewport;

        self.virtual_list.set_len(self.items.len());
        self.virtual_list.set_viewport_extent(viewport.height);
        self.virtual_list.clamp_scroll_to_content();
        let offset_changed = self
            .scroll_state
            .set_offset_y_raw(self.virtual_list.scroll_offset());
        let content_height = self.virtual_list.total_extent();
        self.scroll_state
            .set_metrics(viewport, Size::new(viewport.width, content_height));

        let range = self.virtual_list.materialized_range();
        ListVirtualSync {
            viewport,
            range,
            changed: viewport_changed || offset_changed,
        }
    }

    fn reconcile_realized_items(
        &mut self,
        listbox: ElementId,
        ui: &mut Ui,
        range: Range<usize>,
    ) -> bool {
        let mut changed = false;
        let mut kept = Vec::with_capacity(self.realized.len());
        for item in self.realized.drain(..) {
            if range.contains(&item.index) && ui.get(item.element).is_some() {
                kept.push(item);
            } else if ui.get(item.element).is_some() {
                ui.remove(item.element);
                changed = true;
            }
        }
        self.realized = kept;

        for index in range {
            if self.realized.iter().any(|item| item.index == index) {
                continue;
            }
            let Some(content) = self.items.get(index) else {
                continue;
            };
            let element = append_realized_list_item(ui, listbox, index, content.clone());
            self.realized.push(RealizedListItem { index, element });
            changed = true;
        }
        self.realized.sort_by_key(|item| item.index);
        changed
    }

    fn layout_realized_items(&mut self, ui: &mut Ui, viewport: Size) -> bool {
        let mut changed = false;
        let width = f32_from_logical(viewport.width);
        let height = f32_from_logical(self.item_extent);
        for item in &self.realized {
            let top = f32_from_logical(self.virtual_list.offset_of(item.index));
            let layout = Layout::new()
                .absolute_at(0.0, top)
                .logical_size(width, height);
            if ui
                .get(item.element)
                .and_then(|element| element.get_local_value(ui.properties().layout))
                != Some(&layout)
            {
                ui.set_layout(item.element, layout);
                changed = true;
            }
        }
        changed
    }

    fn scroll_by(&mut self, listbox: ElementId, ui: &mut Ui, delta_y: f64) {
        let next = self.virtual_list.scroll_offset() + delta_y;
        self.set_scroll_offset_y(listbox, ui, next);
    }

    fn set_scroll_offset_y(&mut self, listbox: ElementId, ui: &mut Ui, offset: f64) {
        self.virtual_list
            .set_scroll_offset(normalize_scroll_offset_y(offset));
        self.virtual_list.clamp_scroll_to_content();
        let _ = self
            .scroll_state
            .set_offset_y_raw(self.virtual_list.scroll_offset());
        ui.mark_channels(listbox, LAYOUT.into());
        let _ = self.sync_virtual_items(listbox, ui);
    }

    fn scroll_index_into_view(&mut self, listbox: ElementId, ui: &mut Ui, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        self.virtual_list
            .scroll_to_index(index, ScrollAlign::Nearest);
        let _ = self
            .scroll_state
            .set_offset_y_raw(self.virtual_list.scroll_offset());
        ui.mark_channels(listbox, LAYOUT.into());
        let _ = self.sync_virtual_items(listbox, ui);
        true
    }

    fn selected_index(&self) -> Option<usize> {
        self.selection
            .borrow()
            .primary()
            .copied()
            .filter(|index| *index < self.items.len())
    }

    fn selected_indices(&self) -> Vec<usize> {
        self.selection
            .borrow()
            .items()
            .iter()
            .copied()
            .filter(|index| *index < self.items.len())
            .collect()
    }

    fn select_index(&mut self, ui: &mut Ui, listbox: ElementId, index: usize) -> bool {
        if !ui.is_enabled(listbox) || index >= self.items.len() {
            return false;
        }
        self.mutate_selection(ui, listbox, |selection| selection.select_only(index))
            .is_some()
    }

    fn select_range(
        &mut self,
        ui: &mut Ui,
        listbox: ElementId,
        index: usize,
        extend_existing: bool,
    ) -> bool {
        if !ui.is_enabled(listbox) || index >= self.items.len() {
            return false;
        }

        let anchor = self
            .selection
            .borrow()
            .anchor()
            .copied()
            .filter(|anchor| *anchor < self.items.len())
            .unwrap_or(index);
        let range = ordered_index_range(anchor, index);

        self.mutate_selection(ui, listbox, move |selection| {
            if !extend_existing {
                return selection.replace_with_roles(range, Some(&index), Some(&anchor));
            }

            let mut changed = selection.extend_with(range);
            changed |= selection.set_primary(&index);
            changed |= selection.set_anchor(&anchor);
            changed
        })
        .is_some()
    }

    fn toggle_index(&mut self, ui: &mut Ui, listbox: ElementId, index: usize) -> bool {
        if !ui.is_enabled(listbox) || index >= self.items.len() {
            return false;
        }

        self.mutate_selection(ui, listbox, |selection| {
            let mut changed = selection.toggle(index);
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

    fn select_all(&mut self, ui: &mut Ui, listbox: ElementId) -> bool {
        if self.items.is_empty() {
            return false;
        }
        let items: Vec<usize> = (0..self.items.len()).collect();
        self.mutate_selection(ui, listbox, move |selection| selection.replace_with(items))
            .is_some()
    }

    fn mutate_selection(
        &mut self,
        ui: &mut Ui,
        listbox: ElementId,
        mutate: impl FnOnce(&mut Selection<usize>) -> bool,
    ) -> Option<bool> {
        let changed = {
            let mut selection = self.selection.borrow_mut();
            mutate(&mut selection)
        };

        self.sync_realized_selected_pseudos(ui);
        if changed {
            dispatch_list_selection_changed(
                ui,
                Some(Rc::clone(&self.selection_changed)),
                ListSelectionChangedEvent {
                    listbox,
                    selected_indices: self.selected_indices(),
                    primary_index: self.selected_index(),
                },
            );
        }
        Some(changed)
    }

    fn sync_realized_selected_pseudos(&self, ui: &mut Ui) {
        let selection = self.selection.borrow();
        for item in &self.realized {
            if selection.contains(&item.index) {
                ui.add_pseudo(item.element, PSEUDO_SELECTED);
            } else {
                ui.remove_pseudo(item.element, PSEUDO_SELECTED);
            }
        }
    }

    fn visible_item_count(&self) -> usize {
        if self.item_extent <= f64::EPSILON || self.virtual_list.viewport_extent() <= 0.0 {
            return 1;
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "positive viewport/item ratio is clamped to at least one visible item"
        )]
        {
            (self.virtual_list.viewport_extent() / self.item_extent)
                .floor()
                .max(1.0) as usize
        }
    }
}

/// Per-element widget for a realized `ListBox` item.
#[derive(Debug)]
pub(crate) struct ListItemWidget {
    index: usize,
}

impl Widget for ListItemWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_LIST_ITEM)
    }

    fn default_template(&self) -> Option<&'static dyn crate::Template> {
        Some(&LIST_ITEM_TEMPLATE)
    }

    fn handle_click(&mut self, ctx: &mut WidgetEventContext<'_>, event: ClickEvent<'_>) {
        let item = ctx.id();
        if !matches!(event.phase, Phase::Target | Phase::Bubble) {
            return;
        }
        if !ctx.is_enabled(item) {
            return;
        }
        let Some(listbox) = listbox_parent(ctx, item) else {
            return;
        };
        select_list_index_for_click(
            ctx,
            listbox,
            self.index,
            event.modifiers.shift(),
            action_modifier(event.modifiers),
        );
        let _ = ctx.set_pointer_focus(Some(listbox));
        event.mark_handled();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RealizedListItem {
    index: usize,
    element: ElementId,
}

#[derive(Clone, Debug)]
struct ListVirtualSync {
    viewport: Size,
    range: Range<usize>,
    changed: bool,
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

fn append_realized_list_item(
    ui: &mut Ui,
    listbox: ElementId,
    index: usize,
    content: String,
) -> ElementId {
    let id = ui.append_widget(listbox, Box::new(ListItemWidget { index }));
    ui.set_local(id, ui.properties().content, content);
    ui.set_control_state(id, ControlState::interactive().focusable(false));
    id
}

/// Selects `index` in `listbox`, clearing any previous listbox selection.
///
/// Returns `false` if `listbox` is stale, disabled, not a `ListBox`, or if
/// `index` is outside the list's data.
pub fn select_list_index(ui: &mut Ui, listbox: ElementId, index: usize) -> bool {
    with_list_box_mut(ui, listbox, |widget, ui| {
        widget.select_index(ui, listbox, index)
    })
    .unwrap_or(false)
}

/// Returns the currently selected item index in `listbox`, if any.
#[must_use]
pub fn selected_list_index(ui: &Ui, listbox: ElementId) -> Option<usize> {
    list_box_widget(ui, listbox).and_then(ListBoxWidget::selected_index)
}

/// Returns all currently selected item indices in `listbox`.
#[must_use]
pub fn selected_list_indices(ui: &Ui, listbox: ElementId) -> Vec<usize> {
    list_box_widget(ui, listbox)
        .map(ListBoxWidget::selected_indices)
        .unwrap_or_default()
}

/// Returns the list's current vertical scroll offset in logical pixels.
#[must_use]
pub fn list_scroll_offset_y(ui: &Ui, listbox: ElementId) -> Option<f64> {
    list_box_widget(ui, listbox).map(|widget| scroll_offset_y(&widget.scroll_state))
}

/// Sets the list's vertical scroll offset in logical pixels.
///
/// Returns `false` if `listbox` is stale or does not own list state.
pub fn set_list_scroll_offset_y(ui: &mut Ui, listbox: ElementId, offset: f64) -> bool {
    with_list_box_mut(ui, listbox, |widget, ui| {
        widget.set_scroll_offset_y(listbox, ui, offset);
        true
    })
    .unwrap_or(false)
}

/// Scrolls `listbox` just enough to bring `index` into its current viewport.
pub fn scroll_list_index_into_view(ui: &mut Ui, listbox: ElementId, index: usize) -> bool {
    with_list_box_mut(ui, listbox, |widget, ui| {
        widget.scroll_index_into_view(listbox, ui, index)
    })
    .unwrap_or(false)
}

#[cfg(test)]
pub(crate) fn realized_list_item(ui: &Ui, listbox: ElementId, index: usize) -> Option<ElementId> {
    list_box_widget(ui, listbox)?
        .realized
        .iter()
        .find(|item| item.index == index)
        .map(|item| item.element)
        .filter(|item| ui.get(*item).is_some())
}

/// Returns the data index represented by a realized item container.
#[must_use]
pub fn realized_list_item_index(ui: &Ui, item: ElementId) -> Option<usize> {
    ui.get(item)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<ListItemWidget>())
        .map(|widget| widget.index)
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

fn select_list_index_for_click(
    ui: &mut Ui,
    listbox: ElementId,
    index: usize,
    shift: bool,
    action_mod: bool,
) -> bool {
    with_list_box_mut(ui, listbox, |widget, ui| match widget.selection_mode {
        ListSelectionMode::Single => widget.select_index(ui, listbox, index),
        ListSelectionMode::Multiple if shift => widget.select_range(ui, listbox, index, action_mod),
        ListSelectionMode::Multiple if action_mod => widget.toggle_index(ui, listbox, index),
        ListSelectionMode::Multiple => widget.select_index(ui, listbox, index),
    })
    .unwrap_or(false)
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
    list_box_widget(ui, parent).is_some().then_some(parent)
}

fn list_box_widget(ui: &Ui, listbox: ElementId) -> Option<&ListBoxWidget> {
    ui.get(listbox)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<ListBoxWidget>())
}

fn with_list_box_mut<R>(
    ui: &mut Ui,
    listbox: ElementId,
    f: impl FnOnce(&mut ListBoxWidget, &mut Ui) -> R,
) -> Option<R> {
    let mut output = None;
    ui.with_widget_mut(listbox, |widget, ui| {
        if let Some(listbox_widget) = widget.downcast_mut::<ListBoxWidget>() {
            output = Some(f(listbox_widget, ui));
        }
    });
    output
}

fn ordered_index_range(anchor: usize, index: usize) -> Vec<usize> {
    let (start, end) = if anchor <= index {
        (anchor, index)
    } else {
        (index, anchor)
    };
    (start..=end).collect()
}

fn action_modifier(modifiers: ui_events::keyboard::Modifiers) -> bool {
    modifiers.ctrl() || modifiers.meta()
}

fn is_activation_key(key: &ui_events::keyboard::Key) -> bool {
    use ui_events::keyboard::{Key, NamedKey};

    matches!(key, Key::Named(NamedKey::Enter)) || matches!(key, Key::Character(s) if s == " ")
}

fn normalize_item_extent(extent: f64) -> f64 {
    if extent.is_finite() {
        extent.max(1.0)
    } else {
        DEFAULT_LIST_ITEM_EXTENT
    }
}

fn sizes_nearly_equal(a: Size, b: Size) -> bool {
    (a.width - b.width).abs() <= f64::EPSILON && (a.height - b.height).abs() <= f64::EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pseudos::PSEUDO_SELECTED;
    use peniko::{Brush, Color};
    use ui_events::keyboard::{Code, Key, KeyboardEvent, Modifiers, NamedKey};

    const TEST_ITEMS: [&str; 7] = [
        "Vello scene graph",
        "CPU fallback raster",
        "Text shaping pass",
        "Accessibility tree",
        "Hit testing index",
        "Presentation cache",
        "Frame diagnostics",
    ];

    #[test]
    fn clicking_item_updates_listbox_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(root, test_list_box());
        realize(&mut ui);
        let first = realized_list_item(&ui, listbox, 0).unwrap();
        let second = realized_list_item(&ui, listbox, 1).unwrap();

        ui.synthesize_click(first, kurbo::Point::ZERO);
        assert_eq!(selected_list_index(&ui, listbox), Some(0));
        assert!(ui.get(first).unwrap().pseudos().contains(&PSEUDO_SELECTED));

        ui.synthesize_click(second, kurbo::Point::ZERO);
        assert_eq!(selected_list_index(&ui, listbox), Some(1));
        assert!(!ui.get(first).unwrap().pseudos().contains(&PSEUDO_SELECTED));
        assert!(ui.get(second).unwrap().pseudos().contains(&PSEUDO_SELECTED));
    }

    #[test]
    fn action_click_toggles_multiple_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(root, test_list_box().multiple_selection());
        realize(&mut ui);
        let first = realized_list_item(&ui, listbox, 0).unwrap();
        let second = realized_list_item(&ui, listbox, 1).unwrap();

        ui.synthesize_click(first, kurbo::Point::ZERO);
        ui.synthesize_click_with_modifiers(second, kurbo::Point::ZERO, Modifiers::CONTROL);
        assert_eq!(selected_list_indices(&ui, listbox), vec![0, 1]);

        ui.synthesize_click_with_modifiers(first, kurbo::Point::ZERO, Modifiers::CONTROL);
        assert_eq!(selected_list_indices(&ui, listbox), vec![1]);
    }

    #[test]
    fn selection_changed_handler_observes_semantic_indices() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let seen_for_handler = Rc::clone(&seen);
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(
            root,
            test_list_box().on_selection_changed(move |_ui, event| {
                seen_for_handler
                    .borrow_mut()
                    .push((event.primary_index, event.selected_indices));
            }),
        );

        assert!(select_list_index(&mut ui, listbox, 1));
        assert_eq!(seen.borrow().as_slice(), &[(Some(1), vec![1])]);
    }

    #[test]
    fn shift_click_selects_anchor_range() {
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(root, test_list_box().multiple_selection());
        realize(&mut ui);
        let first = realized_list_item(&ui, listbox, 0).unwrap();
        let third = realized_list_item(&ui, listbox, 2).unwrap();

        ui.synthesize_click(first, kurbo::Point::ZERO);
        ui.synthesize_click_with_modifiers(third, kurbo::Point::ZERO, Modifiers::SHIFT);
        assert_eq!(selected_list_indices(&ui, listbox), vec![0, 1, 2]);
        assert_eq!(selected_list_index(&ui, listbox), Some(2));
    }

    #[test]
    fn arrow_keys_move_listbox_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(root, test_list_box());
        ui.set_focus(Some(listbox));

        let down = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowDown), Code::ArrowDown);
        assert!(ui.dispatch_key(&down).handled);
        assert_eq!(selected_list_index(&ui, listbox), Some(0));
        assert!(ui.dispatch_key(&down).handled);
        assert_eq!(selected_list_index(&ui, listbox), Some(1));

        let end = KeyboardEvent::key_down(Key::Named(NamedKey::End), Code::End);
        assert!(ui.dispatch_key(&end).handled);
        assert_eq!(
            selected_list_index(&ui, listbox),
            Some(TEST_ITEMS.len() - 1)
        );

        let up = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowUp), Code::ArrowUp);
        assert!(ui.dispatch_key(&up).handled);
        assert_eq!(
            selected_list_index(&ui, listbox),
            Some(TEST_ITEMS.len() - 2)
        );
    }

    #[test]
    fn shift_arrow_extends_multiple_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(root, test_list_box().multiple_selection());
        ui.set_focus(Some(listbox));

        let down = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowDown), Code::ArrowDown);
        assert!(ui.dispatch_key(&down).handled);
        assert_eq!(selected_list_indices(&ui, listbox), vec![0]);

        let mut shift_down =
            KeyboardEvent::key_down(Key::Named(NamedKey::ArrowDown), Code::ArrowDown);
        shift_down.modifiers = Modifiers::SHIFT;
        assert!(ui.dispatch_key(&shift_down).handled);
        assert_eq!(selected_list_indices(&ui, listbox), vec![0, 1]);
        assert_eq!(selected_list_index(&ui, listbox), Some(1));
    }

    #[test]
    fn action_a_selects_all_items_in_multiple_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(root, test_list_box().multiple_selection());
        ui.set_focus(Some(listbox));

        let mut select_all = KeyboardEvent::key_down(Key::Character(String::from("a")), Code::KeyA);
        select_all.modifiers = Modifiers::CONTROL;
        assert!(ui.dispatch_key(&select_all).handled);
        assert_eq!(
            selected_list_indices(&ui, listbox),
            vec![0, 1, 2, 3, 4, 5, 6]
        );
    }

    #[test]
    fn listbox_realizes_only_materialized_items() {
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(
            root,
            ListBox::new()
                .items((0..100).map(|index| format!("Item {index}")))
                .size(140.0, 56.0)
                .item_extent(20.0)
                .overscan(0.0),
        );

        realize(&mut ui);

        assert!(ui.get(listbox).unwrap().children().len() < 100);
        assert!(realized_list_item(&ui, listbox, 0).is_some());
        assert!(realized_list_item(&ui, listbox, 99).is_none());
    }

    #[test]
    fn setting_scroll_offset_rematerializes_visible_items() {
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(
            root,
            ListBox::new()
                .items((0..100).map(|index| format!("Item {index}")))
                .size(140.0, 56.0)
                .item_extent(20.0)
                .overscan(0.0),
        );
        realize(&mut ui);

        assert_eq!(list_scroll_offset_y(&ui, listbox), Some(0.0));
        assert!(set_list_scroll_offset_y(&mut ui, listbox, 200.0));
        realize(&mut ui);

        assert_eq!(list_scroll_offset_y(&ui, listbox), Some(200.0));
        assert!(realized_list_item(&ui, listbox, 10).is_some());
        assert!(realized_list_item(&ui, listbox, 0).is_none());
    }

    #[test]
    fn keyboard_selection_scrolls_item_into_view() {
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(
            root,
            ListBox::new()
                .items((0..20).map(|index| format!("Item {index}")))
                .size(140.0, 56.0)
                .item_extent(20.0)
                .overscan(0.0),
        );
        realize(&mut ui);
        ui.set_focus(Some(listbox));

        let end = KeyboardEvent::key_down(Key::Named(NamedKey::End), Code::End);
        assert!(ui.dispatch_key(&end).handled);
        realize(&mut ui);

        assert_eq!(selected_list_index(&ui, listbox), Some(19));
        assert!(list_scroll_offset_y(&ui, listbox).is_some_and(|offset| offset > 0.0));
        assert!(realized_list_item(&ui, listbox, 19).is_some());
    }

    #[test]
    fn select_rejects_indices_outside_the_listbox() {
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(root, test_list_box());

        assert!(!select_list_index(&mut ui, listbox, TEST_ITEMS.len()));
        assert_eq!(selected_list_index(&ui, listbox), None);
    }

    #[test]
    fn selected_item_matches_default_theme_selected_state() {
        let mut ui = Ui::new();
        ui.set_theme(crate::theme::default_theme());
        ui.set_cascade(crate::theme::default_cascade(ui.properties()));
        let root = ui.root();
        let listbox = ui.append(root, test_list_box());
        realize(&mut ui);
        let item = realized_list_item(&ui, listbox, 0).unwrap();

        assert!(select_list_index(&mut ui, listbox, 0));
        let background = ui.resolve_value_for(ui.get(item).unwrap(), ui.properties().background);
        assert_eq!(
            background,
            Some(Brush::Solid(Color::from_rgba8(0x3a, 0x69, 0xd6, 0xff)))
        );
    }

    fn test_list_box() -> ListBox {
        ListBox::new()
            .items(TEST_ITEMS)
            .size(180.0, 128.0)
            .item_extent(24.0)
            .overscan(0.0)
    }

    fn realize(ui: &mut Ui) {
        ui.compute_layout(Size::new(400.0, 300.0));
    }
}
