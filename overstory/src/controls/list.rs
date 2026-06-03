// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `ListBox` and realized list item containers.
//!
//! Roadmap:
//! - Keep lists virtualized by default, with `understory_virtual_list` owning
//!   viewport math and Overstory owning realized elements and interaction.
//! - Replace string-only items with keyed item data and item templates so the
//!   control can grow toward `ItemsControl` / `ListView` patterns. This should
//!   also provide the typed option-key shape used by `RadioGroup`, `Select`,
//!   segmented controls, and other single-selection controls.
//! - Keep selection backed by `understory_selection`: focus, current item,
//!   anchor, primary selection, and realized containers must not drift apart.
//! - Add mature keyboard behavior: typeahead, Home / End / Page movement,
//!   platform multi-select conventions, and clear focus-vs-selection semantics.
//! - Integrate reusable scrolling, scrollbars, accessibility, and eventually
//!   composition-backed virtualization without making apps assemble that by hand.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::ops::Range;

use kurbo::{Size, Vec2};
use understory_selection::Selection;
use understory_style::{PartTag, TypeTag};

use crate::Phase;
use crate::behavior::{ElementRealizeContext, ElementRealizeResult, WidgetEventContext};
use crate::controls::choice::{
    ChoiceOption, collect_options, enabled_index_at_or_after, enabled_index_at_or_before,
    first_enabled_index, last_enabled_index, next_enabled_index, option_enabled,
    previous_enabled_index, selected_enabled_index,
};
use crate::controls::scroll::{
    normalize_scroll_offset_y, state_scroll_offset_y, wheel_scroll_delta,
};
use crate::controls::support::{
    CallbackSlot, EventHandler, FixedVirtualizer, ReservedVerticalScrollArea,
    append_reserved_vertical_scroll_area, configure_vertical_scroll_accessibility,
    fixed_virtual_accessibility_scroll_delta, fixed_virtualizer, fixed_visible_item_count,
    layout_fixed_virtual_rows, reserved_scroll_frame_widget, scroll_fixed_virtual_index_into_view,
    set_fixed_virtual_scroll_offset_y, sync_fixed_virtual_rows,
};
use crate::controls::text::TextWidget;
use crate::element::{Element, ElementId};
use crate::event_runtime::{ClickEvent, KeyEvent, ScrollEvent};
use crate::layouts::Layout;
use crate::pseudos::PSEUDO_SELECTED;
use crate::scroll_runtime::ScrollState;
use crate::template::{
    TemplateBinding, TemplateBuiltInProperty, TemplateNode, TemplateSlot, TemplateSpec,
    TemplateWidgetFactory,
};
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, ElementConfig, ElementSpec, SurfacePartWidget, Widget};

/// Selector identity for `ListBox`.
pub const TYPE_LIST_BOX: TypeTag = TypeTag(11);
/// Selector identity for realized `ListBox` item containers.
pub const TYPE_LIST_ITEM: TypeTag = TypeTag(12);
/// Owner-local part identity for the styled list item content surface.
pub const PART_LIST_ITEM_CONTENT: PartTag = PartTag(1);
/// Owner-local part identity for the visible item label.
pub const PART_LIST_ITEM_LABEL: PartTag = PartTag(2);
/// Semantic slot for item content.
pub const SLOT_LIST_ITEM_CONTENT: TemplateSlot =
    TemplateSlot::new("content").with_part_tag(PART_LIST_ITEM_LABEL);

const DEFAULT_LIST_BOX_WIDTH: f32 = 220.0;
const DEFAULT_LIST_BOX_HEIGHT: f32 = 160.0;
const DEFAULT_LIST_ITEM_EXTENT: f64 = 28.0;
const DEFAULT_LIST_OVERSCAN: f64 = 56.0;
const DEFAULT_LIST_BOX_SCROLLBAR_GAP: f32 = 8.0;
const DEFAULT_LIST_BOX_SCROLLBAR_THICKNESS: f32 = 10.0;
const DEFAULT_LIST_BOX_SCROLLBAR_THUMB_EXTENT: f32 = 30.0;

type ListSelectionChangedHandler = EventHandler<ListSelectionChangedEvent>;
type ListSelectionChangedSlot = CallbackSlot<ListSelectionChangedEvent>;

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
    options: Vec<ChoiceOption>,
    size: Option<(f32, f32)>,
    item_extent: f64,
    overscan: f64,
    element: ElementConfig,
    selection_mode: ListSelectionMode,
    selected_index: Option<usize>,
    scroll_offset_y: f64,
    on_selection_changed: Option<ListSelectionChangedHandler>,
}

impl core::fmt::Debug for ListBox {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ListBox")
            .field("options_len", &self.options.len())
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
        let mut element = ElementConfig::default();
        element.set_control_state(ControlState::interactive());
        Self {
            options: Vec::new(),
            size: None,
            item_extent: DEFAULT_LIST_ITEM_EXTENT,
            overscan: DEFAULT_LIST_OVERSCAN,
            element,
            selection_mode: ListSelectionMode::Single,
            selected_index: None,
            scroll_offset_y: 0.0,
            on_selection_changed: None,
        }
    }

    /// Replaces the list's options.
    #[must_use]
    pub fn options<I, O>(mut self, options: I) -> Self
    where
        I: IntoIterator<Item = O>,
        O: Into<ChoiceOption>,
    {
        self.options = collect_options(options);
        self
    }

    /// Replaces the list's item labels.
    #[must_use]
    pub fn items<I, O>(self, items: I) -> Self
    where
        I: IntoIterator<Item = O>,
        O: Into<ChoiceOption>,
    {
        self.options(items)
    }

    /// Appends one option to the list.
    #[must_use]
    pub fn option(mut self, option: impl Into<ChoiceOption>) -> Self {
        self.options.push(option.into());
        self
    }

    /// Appends one item label to the list.
    #[must_use]
    pub fn item(self, item: impl Into<ChoiceOption>) -> Self {
        self.option(item)
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
        self.element
            .update_control_state(|state| state.focusable(focusable));
        self
    }

    /// Sets whether the list starts enabled.
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

    /// Wraps this list in a reserved vertical scrollbar frame.
    #[must_use]
    pub fn with_vertical_scrollbar(self) -> ListBoxScrollArea {
        ListBoxScrollArea::new(self)
    }

    fn control_state(&self) -> ControlState {
        self.element
            .control_state()
            .unwrap_or_else(ControlState::interactive)
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
            .filter(|index| option_enabled(&self.options, *index))
        {
            let _ = selection.select_only(index);
        }

        let scroll_state = ScrollState::with_offset_y(self.scroll_offset_y);
        let virtual_list = fixed_virtualizer(
            self.options.len(),
            self.item_extent,
            self.scroll_offset_y,
            self.overscan,
        );

        let id = ui.append_widget(
            parent,
            Box::new(ListBoxWidget {
                options: self.options,
                selection,
                scroll_state: scroll_state.clone(),
                selection_mode: self.selection_mode,
                selection_changed: ListSelectionChangedSlot::new(self.on_selection_changed),
                virtual_list,
                item_extent: self.item_extent,
                realized: Vec::new(),
                last_viewport: Size::ZERO,
            }),
        );

        let (width, height) = self
            .size
            .unwrap_or((DEFAULT_LIST_BOX_WIDTH, DEFAULT_LIST_BOX_HEIGHT));
        ui.register_scroll_target(id, scroll_state);
        ui.set_layout(id, Layout::new().relative().logical_size(width, height));
        self.element.apply_to(ui, id);
        id
    }
}

impl ElementSpec for ListBox {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

/// Builder + spec for a [`ListBox`] with a reserved vertical scrollbar.
pub struct ListBoxScrollArea {
    list: ListBox,
    gap: f32,
    scrollbar_thickness: f32,
    thumb_extent: f32,
    element: ElementConfig,
}

impl core::fmt::Debug for ListBoxScrollArea {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ListBoxScrollArea")
            .field("list", &self.list)
            .field("gap", &self.gap)
            .field("scrollbar_thickness", &self.scrollbar_thickness)
            .field("thumb_extent", &self.thumb_extent)
            .field("element", &self.element)
            .finish()
    }
}

impl ListBoxScrollArea {
    /// Starts a scrollbar frame around `list`.
    #[must_use]
    pub fn new(list: ListBox) -> Self {
        Self {
            list,
            gap: DEFAULT_LIST_BOX_SCROLLBAR_GAP,
            scrollbar_thickness: DEFAULT_LIST_BOX_SCROLLBAR_THICKNESS,
            thumb_extent: DEFAULT_LIST_BOX_SCROLLBAR_THUMB_EXTENT,
            element: ElementConfig::default(),
        }
    }

    /// Sets the gap between the list viewport and the reserved scrollbar.
    #[must_use]
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self
    }

    /// Sets the reserved scrollbar thickness.
    #[must_use]
    pub fn scrollbar_thickness(mut self, thickness: f32) -> Self {
        self.scrollbar_thickness = thickness.max(1.0);
        self
    }

    /// Sets the fixed scrollbar thumb extent.
    #[must_use]
    pub fn thumb_extent(mut self, extent: f32) -> Self {
        self.thumb_extent = extent.max(1.0);
        self
    }
}

impl AppendSpec for ListBoxScrollArea {
    fn append_to(mut self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let (width, height) = self
            .list
            .size
            .unwrap_or((DEFAULT_LIST_BOX_WIDTH, DEFAULT_LIST_BOX_HEIGHT));
        self.list.size = Some((width, height));
        let enabled = self.list.control_state().enabled;

        let (frame, _, _) = append_reserved_vertical_scroll_area(
            ui,
            parent,
            self.list,
            ReservedVerticalScrollArea {
                width,
                height,
                gap: self.gap,
                scrollbar_thickness: self.scrollbar_thickness,
                thumb_extent: self.thumb_extent,
                enabled,
            },
        );
        self.element.apply_to(ui, frame);
        frame
    }
}

impl ElementSpec for ListBoxScrollArea {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

/// Returns the inner list target for a [`ListBoxScrollArea`].
#[must_use]
pub fn list_box_scroll_area_list(ui: &Ui, frame: ElementId) -> Option<ElementId> {
    reserved_scroll_frame_widget(ui, frame).and_then(|widget| widget.target())
}

/// Returns the reserved scrollbar for a [`ListBoxScrollArea`].
#[must_use]
pub fn list_box_scroll_area_scrollbar(ui: &Ui, frame: ElementId) -> Option<ElementId> {
    reserved_scroll_frame_widget(ui, frame).and_then(|widget| widget.scrollbar())
}

/// Per-element widget for `ListBox`.
pub(crate) struct ListBoxWidget {
    options: Vec<ChoiceOption>,
    selection: Selection<usize>,
    scroll_state: ScrollState,
    selection_mode: ListSelectionMode,
    selection_changed: ListSelectionChangedSlot,
    virtual_list: FixedVirtualizer,
    item_extent: f64,
    realized: Vec<RealizedListItem>,
    last_viewport: Size,
}

impl core::fmt::Debug for ListBoxWidget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ListBoxWidget")
            .field("options_len", &self.options.len())
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

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::ListBox)
    }

    fn configure_accessibility(
        &self,
        element: &Element,
        ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        if self.selection_mode == ListSelectionMode::Multiple {
            node.set_multiselectable();
        }
        node.set_size_of_set(self.options.len());
        configure_vertical_scroll_accessibility(element, ctx, node, &self.scroll_state);
        if let Some(primary) = self.selected_index()
            && let Some(item) = self.realized.iter().find(|item| item.index == primary)
        {
            node.set_active_descendant(ctx.node_id(item.element));
        }
    }

    fn clips_subtree(&self, _element: &Element) -> bool {
        true
    }

    fn child_layout_offset(&self, _element: &Element) -> Vec2 {
        -self.scroll_state.offset()
    }

    fn realize_elements(&mut self, ctx: &mut ElementRealizeContext<'_>) -> ElementRealizeResult {
        let listbox = ctx.id();
        ElementRealizeResult::from_changed(self.sync_virtual_items(listbox, ctx.ui_mut()))
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
        if self.options.is_empty() {
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
                next_enabled_index(&self.options, current)
            }
            Key::Named(NamedKey::ArrowUp | NamedKey::ArrowLeft) => {
                previous_enabled_index(&self.options, current)
            }
            Key::Named(NamedKey::PageDown) => {
                self.page_down_index(current, self.visible_item_count())
            }
            Key::Named(NamedKey::PageUp) => self.page_up_index(current, self.visible_item_count()),
            Key::Named(NamedKey::Home) => first_enabled_index(&self.options),
            Key::Named(NamedKey::End) => last_enabled_index(&self.options),
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

    fn handle_accessibility_action(
        &mut self,
        ctx: &mut WidgetEventContext<'_>,
        request: &accesskit::ActionRequest,
    ) -> bool {
        let listbox = ctx.id();
        if !ctx.is_enabled(listbox) {
            return false;
        }
        match request.action {
            accesskit::Action::ScrollUp => {
                self.scroll_by_accessibility_action(listbox, ctx, -1.0, request)
            }
            accesskit::Action::ScrollDown => {
                self.scroll_by_accessibility_action(listbox, ctx, 1.0, request)
            }
            accesskit::Action::SetScrollOffset => {
                let Some(accesskit::ActionData::SetScrollOffset(point)) = request.data.as_ref()
                else {
                    return false;
                };
                self.set_scroll_offset_y(listbox, ctx, point.y);
                true
            }
            _ => false,
        }
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

    fn sync_virtual_metrics(
        &mut self,
        listbox: ElementId,
        ui: &mut Ui,
    ) -> crate::controls::support::FixedVirtualRowsSync {
        sync_fixed_virtual_rows(
            listbox,
            ui,
            &mut self.virtual_list,
            &self.scroll_state,
            self.options.len(),
            &mut self.last_viewport,
        )
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
            let Some(option) = self.options.get(index) else {
                continue;
            };
            let element = append_realized_list_item(ui, listbox, index, option);
            self.realized.push(RealizedListItem { index, element });
            changed = true;
        }
        self.realized.sort_by_key(|item| item.index);
        changed
    }

    fn layout_realized_items(&mut self, ui: &mut Ui, viewport: Size) -> bool {
        layout_fixed_virtual_rows(
            ui,
            &mut self.virtual_list,
            self.item_extent,
            self.realized.iter().map(|item| (item.index, item.element)),
            viewport,
        )
    }

    fn scroll_by(&mut self, listbox: ElementId, ui: &mut Ui, delta_y: f64) {
        let next = self.virtual_list.scroll_offset() + delta_y;
        self.set_scroll_offset_y(listbox, ui, next);
    }

    fn scroll_by_accessibility_action(
        &mut self,
        listbox: ElementId,
        ui: &mut Ui,
        direction: f64,
        request: &accesskit::ActionRequest,
    ) -> bool {
        let Some(delta) = fixed_virtual_accessibility_scroll_delta(
            request,
            direction,
            self.item_extent,
            self.virtual_list.viewport_extent(),
        ) else {
            return false;
        };
        self.scroll_by(listbox, ui, delta);
        true
    }

    fn set_scroll_offset_y(&mut self, listbox: ElementId, ui: &mut Ui, offset: f64) {
        set_fixed_virtual_scroll_offset_y(
            listbox,
            ui,
            &mut self.virtual_list,
            &self.scroll_state,
            offset,
        );
        let _ = self.sync_virtual_items(listbox, ui);
    }

    fn scroll_index_into_view(&mut self, listbox: ElementId, ui: &mut Ui, index: usize) -> bool {
        if !scroll_fixed_virtual_index_into_view(
            listbox,
            ui,
            &mut self.virtual_list,
            &self.scroll_state,
            index,
            self.options.len(),
        ) {
            return false;
        }
        let _ = self.sync_virtual_items(listbox, ui);
        true
    }

    fn selected_index(&self) -> Option<usize> {
        self.selection
            .primary()
            .copied()
            .filter(|index| option_enabled(&self.options, *index))
    }

    fn selected_indices(&self) -> Vec<usize> {
        self.selection
            .items()
            .iter()
            .copied()
            .filter(|index| option_enabled(&self.options, *index))
            .collect()
    }

    fn select_index(&mut self, ui: &mut Ui, listbox: ElementId, index: usize) -> bool {
        if !ui.is_enabled(listbox) || !option_enabled(&self.options, index) {
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
        if !ui.is_enabled(listbox) || !option_enabled(&self.options, index) {
            return false;
        }

        let anchor = self
            .selection
            .anchor()
            .copied()
            .filter(|anchor| option_enabled(&self.options, *anchor))
            .unwrap_or(index);
        let range = ordered_index_range(anchor, index)
            .into_iter()
            .filter(|index| option_enabled(&self.options, *index))
            .collect::<Vec<_>>();

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
        if !ui.is_enabled(listbox) || !option_enabled(&self.options, index) {
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
        if self.options.is_empty() {
            return false;
        }
        let items: Vec<usize> = (0..self.options.len())
            .filter(|index| option_enabled(&self.options, *index))
            .collect();
        if items.is_empty() {
            return false;
        }
        self.mutate_selection(ui, listbox, move |selection| selection.replace_with(items))
            .is_some()
    }

    fn mutate_selection(
        &mut self,
        ui: &mut Ui,
        listbox: ElementId,
        mutate: impl FnOnce(&mut Selection<usize>) -> bool,
    ) -> Option<bool> {
        let changed = mutate(&mut self.selection);

        self.sync_realized_selected_pseudos(ui);
        if changed {
            self.selection_changed.dispatch(
                ui,
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
        for item in &self.realized {
            if self.selection.contains(&item.index) {
                ui.add_pseudo(item.element, PSEUDO_SELECTED);
            } else {
                ui.remove_pseudo(item.element, PSEUDO_SELECTED);
            }
        }
    }

    fn visible_item_count(&self) -> usize {
        fixed_visible_item_count(self.item_extent, self.virtual_list.viewport_extent())
    }

    fn page_down_index(&self, current: Option<usize>, page: usize) -> Option<usize> {
        let current = selected_enabled_index(&self.options, current);
        let start = current.unwrap_or(0);
        let target = start.saturating_add(page).min(self.options.len() - 1);
        enabled_index_at_or_after(&self.options, target)
            .or_else(|| enabled_index_at_or_before(&self.options, target))
    }

    fn page_up_index(&self, current: Option<usize>, page: usize) -> Option<usize> {
        let current = selected_enabled_index(&self.options, current);
        let start = current.unwrap_or(0);
        let target = start.saturating_sub(page);
        enabled_index_at_or_before(&self.options, target)
            .or_else(|| enabled_index_at_or_after(&self.options, target))
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

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::ListBoxOption)
    }

    fn configure_accessibility(
        &self,
        element: &Element,
        ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        ctx.add_click_action_if_enabled(element, node);
        ctx.apply_selected_state(element, node);
        node.set_position_in_set(self.index + 1);
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

const LIST_ITEM_CONTENT_CHILDREN: &[TemplateNode] =
    &[
        TemplateNode::widget(TemplateWidgetFactory::new(list_item_label_widget))
            .with_slot(SLOT_LIST_ITEM_CONTENT)
            .with_bindings(LIST_ITEM_LABEL_BINDINGS),
    ];

const LIST_ITEM_TEMPLATE_NODES: &[TemplateNode] =
    &[
        TemplateNode::widget(TemplateWidgetFactory::new(list_item_content_widget))
            .with_part(PART_LIST_ITEM_CONTENT)
            .with_children(LIST_ITEM_CONTENT_CHILDREN),
    ];

fn list_item_content_widget() -> Box<dyn Widget> {
    Box::new(SurfacePartWidget)
}

fn list_item_label_widget() -> Box<dyn Widget> {
    Box::new(TextWidget)
}

fn append_realized_list_item(
    ui: &mut Ui,
    listbox: ElementId,
    index: usize,
    option: &ChoiceOption,
) -> ElementId {
    let id = ui.append_widget(listbox, Box::new(ListItemWidget { index }));
    ui.set_local(id, ui.properties().content, String::from(option.label()));
    ui.set_control_state(
        id,
        ControlState::interactive()
            .focusable(false)
            .enabled(option.is_enabled()),
    );
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
    list_box_widget(ui, listbox).map(|widget| state_scroll_offset_y(&widget.scroll_state))
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

fn listbox_parent(ui: &Ui, item: ElementId) -> Option<ElementId> {
    let parent = ui.get(item)?.parent()?;
    direct_list_box_widget(ui, parent)
        .is_some()
        .then_some(parent)
}

fn list_box_widget(ui: &Ui, listbox: ElementId) -> Option<&ListBoxWidget> {
    direct_list_box_widget(ui, list_box_target(ui, listbox)?)
}

fn direct_list_box_widget(ui: &Ui, listbox: ElementId) -> Option<&ListBoxWidget> {
    ui.get(listbox)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<ListBoxWidget>())
}

fn list_box_target(ui: &Ui, id: ElementId) -> Option<ElementId> {
    if direct_list_box_widget(ui, id).is_some() {
        return Some(id);
    }
    list_box_scroll_area_list(ui, id)
}

fn with_list_box_mut<R>(
    ui: &mut Ui,
    listbox: ElementId,
    f: impl FnOnce(&mut ListBoxWidget, &mut Ui) -> R,
) -> Option<R> {
    let listbox = list_box_target(ui, listbox)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ElementSpecExt;
    use crate::accessibility::accesskit_node_id;
    use crate::controls::scroll::{SLOT_SCROLL_BAR_THUMB, ScrollBar};
    use crate::pseudos::PSEUDO_SELECTED;
    use crate::test_support::{pointer_down, pointer_move, pointer_up};
    use accesskit::{Action, ActionData, ActionRequest, Node, Role, TreeId, TreeUpdate};
    use alloc::rc::Rc;
    use core::cell::RefCell;
    use kurbo::Point;
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
        compute_layout(&mut ui);
        let first = realized_list_item(&ui, listbox, 0).unwrap();
        let second = realized_list_item(&ui, listbox, 1).unwrap();

        ui.synthesize_click(first, Point::ZERO);
        assert_eq!(selected_list_index(&ui, listbox), Some(0));
        assert!(ui.get(first).unwrap().pseudos().contains(&PSEUDO_SELECTED));

        ui.synthesize_click(second, Point::ZERO);
        assert_eq!(selected_list_index(&ui, listbox), Some(1));
        assert!(!ui.get(first).unwrap().pseudos().contains(&PSEUDO_SELECTED));
        assert!(ui.get(second).unwrap().pseudos().contains(&PSEUDO_SELECTED));
    }

    #[test]
    fn action_click_toggles_multiple_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(root, test_list_box().multiple_selection());
        compute_layout(&mut ui);
        let first = realized_list_item(&ui, listbox, 0).unwrap();
        let second = realized_list_item(&ui, listbox, 1).unwrap();

        ui.synthesize_click(first, Point::ZERO);
        ui.synthesize_click_with_modifiers(second, Point::ZERO, Modifiers::CONTROL);
        assert_eq!(selected_list_indices(&ui, listbox), vec![0, 1]);

        ui.synthesize_click_with_modifiers(first, Point::ZERO, Modifiers::CONTROL);
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
        compute_layout(&mut ui);
        let first = realized_list_item(&ui, listbox, 0).unwrap();
        let third = realized_list_item(&ui, listbox, 2).unwrap();

        ui.synthesize_click(first, Point::ZERO);
        ui.synthesize_click_with_modifiers(third, Point::ZERO, Modifiers::SHIFT);
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
    fn disabled_options_are_not_selectable() {
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(
            root,
            ListBox::new()
                .options([
                    ChoiceOption::new("Draft"),
                    ChoiceOption::new("Review").disabled(),
                    ChoiceOption::new("Ready"),
                ])
                .selected_index(1)
                .size(180.0, 90.0)
                .item_extent(24.0)
                .overscan(0.0),
        );

        assert_eq!(selected_list_index(&ui, listbox), None);
        assert!(!select_list_index(&mut ui, listbox, 1));

        compute_layout(&mut ui);
        let disabled = realized_list_item(&ui, listbox, 1).unwrap();
        assert!(!ui.is_enabled(disabled));

        ui.synthesize_click(disabled, Point::ZERO);
        assert_eq!(selected_list_index(&ui, listbox), None);

        ui.set_focus(Some(listbox));
        let down = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowDown), Code::ArrowDown);
        assert!(ui.dispatch_key(&down).handled);
        assert_eq!(selected_list_index(&ui, listbox), Some(0));
        assert!(ui.dispatch_key(&down).handled);
        assert_eq!(selected_list_index(&ui, listbox), Some(2));
    }

    #[test]
    fn listbox_accessibility_exposes_item_set_and_selection_metadata() {
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(root, test_list_box().multiple_selection());
        assert!(select_list_index(&mut ui, listbox, 2));
        compute_layout(&mut ui);
        let item = realized_list_item(&ui, listbox, 2).unwrap();

        let update = ui.accesskit_tree_update();
        let listbox_node = accesskit_node_for(&update, listbox);
        let item_node = accesskit_node_for(&update, item);

        assert_eq!(listbox_node.role(), Role::ListBox);
        assert!(listbox_node.is_multiselectable());
        assert_eq!(listbox_node.size_of_set(), Some(TEST_ITEMS.len()));
        assert_eq!(
            listbox_node.active_descendant(),
            Some(accesskit_node_id(item))
        );
        assert_eq!(item_node.role(), Role::ListBoxOption);
        assert_eq!(item_node.is_selected(), Some(true));
        assert_eq!(item_node.position_in_set(), Some(3));
        assert!(item_node.supports_action(Action::Click));
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

        compute_layout(&mut ui);

        assert!(ui.get(listbox).unwrap().children().len() < 100);
        assert!(realized_list_item(&ui, listbox, 0).is_some());
        assert!(realized_list_item(&ui, listbox, 99).is_none());
    }

    #[test]
    fn listbox_accessibility_reports_scroll_range_from_virtual_model() {
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(root, scrolling_test_list_box());

        compute_layout(&mut ui);
        let update = ui.accesskit_tree_update();
        let node = accesskit_node_for(&update, listbox);

        assert!(ui.get(listbox).unwrap().children().len() < 100);
        assert_eq!(node.role(), Role::ListBox);
        assert_eq!(node.size_of_set(), Some(100));
        assert_eq!(node.scroll_y(), Some(0.0));
        assert_eq!(node.scroll_y_min(), Some(0.0));
        assert_eq!(node.scroll_y_max(), Some((100.0 * 20.0) - 56.0));
        assert!(node.supports_action(Action::ScrollDown));
        assert!(node.supports_action(Action::SetScrollOffset));

        assert!(ui.handle_accesskit_action(&scroll_offset_action(listbox, 180.0)));
        compute_layout(&mut ui);

        assert_eq!(list_scroll_offset_y(&ui, listbox), Some(180.0));
        assert!(realized_list_item(&ui, listbox, 9).is_some());
        assert!(realized_list_item(&ui, listbox, 0).is_none());
    }

    #[test]
    fn listbox_accessibility_scroll_actions_use_virtual_list_units() {
        let mut ui = Ui::new();
        let root = ui.root();
        let listbox = ui.append(root, scrolling_test_list_box());

        compute_layout(&mut ui);

        assert!(ui.handle_accesskit_action(&scroll_action(
            listbox,
            Action::ScrollDown,
            accesskit::ScrollUnit::Page,
        )));
        assert_eq!(list_scroll_offset_y(&ui, listbox), Some(56.0));

        assert!(ui.handle_accesskit_action(&scroll_action(
            listbox,
            Action::ScrollUp,
            accesskit::ScrollUnit::Item,
        )));
        assert_eq!(list_scroll_offset_y(&ui, listbox), Some(36.0));
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
        compute_layout(&mut ui);

        assert_eq!(list_scroll_offset_y(&ui, listbox), Some(0.0));
        assert!(set_list_scroll_offset_y(&mut ui, listbox, 200.0));
        compute_layout(&mut ui);

        assert_eq!(list_scroll_offset_y(&ui, listbox), Some(200.0));
        assert!(realized_list_item(&ui, listbox, 10).is_some());
        assert!(realized_list_item(&ui, listbox, 0).is_none());
    }

    #[test]
    fn external_scrollbar_track_click_scrolls_virtualized_listbox() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(8.0).logical_size(180.0, 80.0));
        let listbox = ui.append(root, scrolling_test_list_box());
        let bar = ui.append(
            root,
            ScrollBar::vertical(listbox)
                .length(56.0)
                .thickness(8.0)
                .thumb_extent(12.0),
        );

        compute_layout(&mut ui);
        let bar_bounds = ui.effective_bounds(bar);
        let target = Point::new(bar_bounds.center().x, bar_bounds.y1 - 1.0);

        assert!(
            ui.dispatch_pointer_event(&pointer_down(target, 1))
                .changed()
        );
        assert!(ui.dispatch_pointer_event(&pointer_up(target, 2)).handled);
        compute_layout(&mut ui);

        assert!(list_scroll_offset_y(&ui, listbox).is_some_and(|offset| offset > 0.0));
        assert!(realized_list_item(&ui, listbox, 0).is_none());
    }

    #[test]
    fn external_scrollbar_thumb_drag_scrolls_virtualized_listbox() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(8.0).logical_size(180.0, 80.0));
        let listbox = ui.append(root, scrolling_test_list_box());
        let bar = ui.append(
            root,
            ScrollBar::vertical(listbox)
                .length(56.0)
                .thickness(8.0)
                .thumb_extent(12.0),
        );
        let thumb = ui
            .slot(bar, SLOT_SCROLL_BAR_THUMB)
            .expect("scrollbar template should create a thumb slot");

        compute_layout(&mut ui);
        let start = ui.effective_bounds(thumb).center();
        assert!(ui.dispatch_pointer_event(&pointer_down(start, 1)).changed());
        assert!(
            ui.dispatch_pointer_event(&pointer_move(Point::new(start.x, start.y + 30.0), 2, true))
                .changed()
        );
        compute_layout(&mut ui);

        assert!(list_scroll_offset_y(&ui, listbox).is_some_and(|offset| offset > 0.0));
        assert!(realized_list_item(&ui, listbox, 0).is_none());
    }

    #[test]
    fn listbox_scroll_area_bar_scrolls_virtualized_listbox() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 90.0));
        let frame = ui.append(
            root,
            scrolling_test_list_box()
                .with_vertical_scrollbar()
                .thumb_extent(12.0),
        );
        let listbox =
            list_box_scroll_area_list(&ui, frame).expect("scroll frame should expose listbox");
        let bar = list_box_scroll_area_scrollbar(&ui, frame)
            .expect("scroll frame should expose scrollbar");

        compute_layout(&mut ui);
        let bar_bounds = ui.effective_bounds(bar);
        let target = Point::new(bar_bounds.center().x, bar_bounds.y1 - 1.0);

        assert!(
            ui.dispatch_pointer_event(&pointer_down(target, 1))
                .changed()
        );
        assert!(ui.dispatch_pointer_event(&pointer_up(target, 2)).handled);
        compute_layout(&mut ui);

        assert!(list_scroll_offset_y(&ui, frame).is_some_and(|offset| offset > 0.0));
        assert_eq!(
            list_scroll_offset_y(&ui, frame),
            list_scroll_offset_y(&ui, listbox)
        );
        assert!(realized_list_item(&ui, frame, 0).is_none());
    }

    #[test]
    fn listbox_scroll_area_children_follow_arranged_height() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(0.0).logical_size(180.0, 120.0));
        let frame = ui.append(
            root,
            scrolling_test_list_box()
                .with_vertical_scrollbar()
                .thumb_extent(12.0)
                .layout(Layout::new().logical_size(158.0, 56.0).fill_height()),
        );
        let listbox =
            list_box_scroll_area_list(&ui, frame).expect("scroll frame should expose listbox");
        let bar = list_box_scroll_area_scrollbar(&ui, frame)
            .expect("scroll frame should expose scrollbar");

        compute_layout(&mut ui);
        let frame_bounds = ui.effective_bounds(frame);
        let list_bounds = ui.effective_bounds(listbox);
        let bar_bounds = ui.effective_bounds(bar);

        assert_eq!(frame_bounds.height(), 120.0);
        assert_eq!(list_bounds.height(), 120.0);
        assert_eq!(bar_bounds.height(), 120.0);
        assert_eq!(list_bounds.width(), 140.0);
        assert_eq!(bar_bounds.width(), 10.0);
        assert!(realized_list_item(&ui, listbox, 0).is_some());

        let target = Point::new(bar_bounds.center().x, bar_bounds.y1 - 1.0);
        assert!(
            ui.dispatch_pointer_event(&pointer_down(target, 1))
                .changed()
        );
        assert!(ui.dispatch_pointer_event(&pointer_up(target, 2)).handled);
        compute_layout(&mut ui);

        assert!(list_scroll_offset_y(&ui, frame).is_some_and(|offset| offset > 0.0));
        assert!(realized_list_item(&ui, frame, 0).is_none());
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
        compute_layout(&mut ui);
        ui.set_focus(Some(listbox));

        let end = KeyboardEvent::key_down(Key::Named(NamedKey::End), Code::End);
        assert!(ui.dispatch_key(&end).handled);
        compute_layout(&mut ui);

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
        compute_layout(&mut ui);
        let item = realized_list_item(&ui, listbox, 0).unwrap();

        assert!(select_list_index(&mut ui, listbox, 0));
        let background = ui.resolve_value_for(ui.get(item).unwrap(), ui.properties().background);
        assert_eq!(
            background,
            Some(Brush::Solid(Color::from_rgba8(0x3a, 0x69, 0xd6, 0xff)))
        );
    }

    #[test]
    fn realized_list_item_label_is_inset_and_centered() {
        let mut ui = Ui::new();
        ui.set_theme(crate::theme::default_theme());
        ui.set_cascade(crate::theme::default_cascade(ui.properties()));
        let root = ui.root();
        let listbox = ui.append(root, test_list_box());

        compute_layout(&mut ui);

        let item = realized_list_item(&ui, listbox, 0).unwrap();
        let content = ui.part(item, PART_LIST_ITEM_CONTENT).unwrap();
        let label = ui.slot(item, SLOT_LIST_ITEM_CONTENT).unwrap();
        let item_bounds = ui.effective_bounds(item);
        let content_bounds = ui.effective_bounds(content);
        let label_bounds = ui.effective_bounds(label);

        assert_eq!(content_bounds, item_bounds);
        assert!(
            label_bounds.x0 >= item_bounds.x0 + 8.0,
            "label should be inset from item edge; item={item_bounds:?}, label={label_bounds:?}"
        );
        assert!(
            (label_bounds.center().y - item_bounds.center().y).abs() <= 1.0,
            "label should be vertically centered; item={item_bounds:?}, label={label_bounds:?}"
        );
    }

    fn test_list_box() -> ListBox {
        ListBox::new()
            .items(TEST_ITEMS)
            .size(180.0, 128.0)
            .item_extent(24.0)
            .overscan(0.0)
    }

    fn scrolling_test_list_box() -> ListBox {
        ListBox::new()
            .items((0..100).map(|index| format!("Item {index}")))
            .size(140.0, 56.0)
            .item_extent(20.0)
            .overscan(0.0)
    }

    fn compute_layout(ui: &mut Ui) {
        ui.compute_layout(Size::new(400.0, 300.0));
    }

    fn accesskit_node_for(update: &TreeUpdate, element: ElementId) -> &Node {
        update
            .nodes
            .iter()
            .find(|(id, _)| *id == accesskit_node_id(element))
            .map(|(_, node)| node)
            .expect("node should exist")
    }

    fn scroll_action(
        element: ElementId,
        action: Action,
        unit: accesskit::ScrollUnit,
    ) -> ActionRequest {
        ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: accesskit_node_id(element),
            data: Some(ActionData::ScrollUnit(unit)),
        }
    }

    fn scroll_offset_action(element: ElementId, offset_y: f64) -> ActionRequest {
        ActionRequest {
            action: Action::SetScrollOffset,
            target_tree: TreeId::ROOT,
            target_node: accesskit_node_id(element),
            data: Some(ActionData::SetScrollOffset(accesskit::Point::new(
                0.0, offset_y,
            ))),
        }
    }
}
