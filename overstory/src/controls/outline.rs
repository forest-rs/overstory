// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Hierarchical outline controls.
//!
//! Roadmap:
//! - Keep hierarchy projection in `understory_outline` so Overstory controls
//!   only own rendering, input, accessibility, and styling policy.
//! - Keep viewport math in `understory_virtual_list`; an outline virtualizes
//!   the visible row projection, not the source tree.
//! - Grow toward a `TreeView` / hierarchical `ListView` once item templates can
//!   expose disclosure parts, row depth, selection, and per-node commands as
//!   app-provided data instead of string-only rows.
//! - Share scroll target registration, scrollbars, keyboard navigation, and
//!   accessibility metadata with the other virtualized controls.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::ops::Range;

use kurbo::{Size, Vec2};
use understory_outline::{Outline, OutlineModel, OutlineNode, VisibleRow};
use understory_property::DependencyObjectExt;
use understory_style::{PartTag, TypeTag};
use understory_virtual_list::{FixedExtentModel, ScrollAlign, VirtualList};

use crate::Phase;
use crate::behavior::WidgetEventContext;
use crate::channels::LAYOUT;
use crate::controls::scroll::{
    ScrollBar, normalize_scroll_offset_y, state_scroll_offset_y, wheel_scroll_delta,
};
use crate::controls::support::{action_scroll_unit, f32_from_logical};
use crate::controls::text::TextWidget;
use crate::element::{Element, ElementId};
use crate::event_runtime::{ClickEvent, KeyEvent, ScrollEvent};
use crate::layouts::{Layout, LayoutDimension, LayoutPlacement, LayoutSize};
use crate::pseudos::PSEUDO_SELECTED;
use crate::scroll_runtime::ScrollState;
use crate::template::{
    TemplateBinding, TemplateBuiltInProperty, TemplateNode, TemplateSlot, TemplateSpec,
    TemplateWidgetFactory,
};
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, SurfacePartWidget, Widget};

/// Selector identity for `OutlineView`.
pub const TYPE_OUTLINE_VIEW: TypeTag = TypeTag(21);
/// Selector identity for realized `OutlineView` item containers.
pub const TYPE_OUTLINE_ITEM: TypeTag = TypeTag(22);
/// Owner-local part identity for the styled outline row surface.
pub const PART_OUTLINE_ITEM_CONTENT: PartTag = PartTag(1);
/// Owner-local part identity for the disclosure arrow.
pub const PART_OUTLINE_ITEM_DISCLOSURE: PartTag = PartTag(2);
/// Owner-local part identity for the visible row label.
pub const PART_OUTLINE_ITEM_LABEL: PartTag = PartTag(3);
/// Semantic slot for the disclosure arrow.
pub const SLOT_OUTLINE_ITEM_DISCLOSURE: TemplateSlot =
    TemplateSlot::new("disclosure").with_part_tag(PART_OUTLINE_ITEM_DISCLOSURE);
/// Semantic slot for the visible row label.
pub const SLOT_OUTLINE_ITEM_LABEL: TemplateSlot =
    TemplateSlot::new("label").with_part_tag(PART_OUTLINE_ITEM_LABEL);

const DEFAULT_OUTLINE_WIDTH: f32 = 220.0;
const DEFAULT_OUTLINE_HEIGHT: f32 = 160.0;
const DEFAULT_OUTLINE_ITEM_EXTENT: f64 = 30.0;
const DEFAULT_OUTLINE_OVERSCAN: f64 = 60.0;
const DEFAULT_OUTLINE_SCROLLBAR_GAP: f32 = 8.0;
const DEFAULT_OUTLINE_SCROLLBAR_THICKNESS: f32 = 10.0;
const DEFAULT_OUTLINE_SCROLLBAR_THUMB_EXTENT: f32 = 30.0;
const OUTLINE_INDENT_PER_DEPTH: f32 = 18.0;
const OUTLINE_DISCLOSURE_WIDTH: f32 = 26.0;

type OutlineVirtualizer = VirtualList<FixedExtentModel<f64>>;

/// A string-backed node for [`OutlineView`].
///
/// The node links use stable indices into the node vector supplied to
/// [`OutlineView::nodes`]. This is the same compact shape used by
/// `understory_outline::SliceOutline`.
pub type OutlineViewNode = OutlineNode<String>;

/// Builder + spec for a virtualized outline view.
///
/// `OutlineView` owns its expansion state and realizes only the currently
/// visible rows. Node identity is the node's stable `usize` key in the supplied
/// node vector, so selection and expansion do not drift when siblings expand or
/// collapse.
#[derive(Clone, Debug)]
pub struct OutlineView {
    nodes: Vec<OutlineViewNode>,
    first_root: Option<usize>,
    expanded: Vec<usize>,
    size: Option<(f32, f32)>,
    item_extent: f64,
    overscan: f64,
    selected_key: Option<usize>,
    scroll_offset_y: f64,
    state: ControlState,
}

impl OutlineView {
    /// Starts a new empty outline view.
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            first_root: None,
            expanded: Vec::new(),
            size: None,
            item_extent: DEFAULT_OUTLINE_ITEM_EXTENT,
            overscan: DEFAULT_OUTLINE_OVERSCAN,
            selected_key: None,
            scroll_offset_y: 0.0,
            state: ControlState::interactive(),
        }
    }

    /// Replaces the outline nodes and first root key.
    ///
    /// Node keys are indices into `nodes`. Each node's `first_child` and
    /// `next_sibling` fields must point at indices in the same vector.
    #[must_use]
    pub fn nodes(mut self, nodes: Vec<OutlineViewNode>, first_root: Option<usize>) -> Self {
        self.nodes = nodes;
        self.first_root = first_root;
        self
    }

    /// Marks a node key as initially expanded.
    #[must_use]
    pub fn expanded(mut self, key: usize) -> Self {
        if !self.expanded.contains(&key) {
            self.expanded.push(key);
        }
        self
    }

    /// Sets the outline's fixed logical-pixel size.
    #[must_use]
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = Some((width, height));
        self
    }

    /// Sets the uniform logical-pixel height of each realized row.
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

    /// Sets the initially selected node key.
    #[must_use]
    pub fn selected_key(mut self, key: usize) -> Self {
        self.selected_key = Some(key);
        self
    }

    /// Sets the initial vertical scroll offset in logical pixels.
    #[must_use]
    pub fn scroll_offset_y(mut self, offset: f64) -> Self {
        self.scroll_offset_y = normalize_scroll_offset_y(offset);
        self
    }

    /// Sets whether the outline participates in focus traversal.
    #[must_use]
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.state = self.state.focusable(focusable);
        self
    }

    /// Sets whether the outline starts enabled.
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

    /// Wraps this outline in a reserved vertical scrollbar frame.
    #[must_use]
    pub fn with_vertical_scrollbar(self) -> OutlineViewScrollArea {
        OutlineViewScrollArea::new(self)
    }
}

impl Default for OutlineView {
    fn default() -> Self {
        Self::new()
    }
}

impl AppendSpec for OutlineView {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let scroll_state = ScrollState::with_offset_y(self.scroll_offset_y);
        let mut virtual_list = VirtualList::new(
            FixedExtentModel::new(0, self.item_extent),
            0.0,
            self.overscan,
        );
        virtual_list.set_scroll_offset(self.scroll_offset_y);

        let mut outline = Outline::new(OwnedOutlineModel {
            nodes: self.nodes,
            first_root: self.first_root,
        });
        let _ = outline.replace_expanded(self.expanded);
        let selected_key = self
            .selected_key
            .filter(|key| outline.model().contains_key(key));

        let id = ui.append_widget(
            parent,
            Box::new(OutlineViewWidget {
                outline,
                scroll_state: scroll_state.clone(),
                virtual_list,
                item_extent: self.item_extent,
                selected_key,
                realized: Vec::new(),
                last_viewport: Size::ZERO,
            }),
        );

        let (width, height) = self
            .size
            .unwrap_or((DEFAULT_OUTLINE_WIDTH, DEFAULT_OUTLINE_HEIGHT));
        ui.register_scroll_target(id, scroll_state);
        ui.set_layout(id, Layout::new().relative().logical_size(width, height));
        ui.set_control_state(id, self.state);
        id
    }
}

/// Builder + spec for an [`OutlineView`] with a reserved vertical scrollbar.
#[derive(Clone, Debug)]
pub struct OutlineViewScrollArea {
    outline: OutlineView,
    gap: f32,
    scrollbar_thickness: f32,
    thumb_extent: f32,
}

impl OutlineViewScrollArea {
    /// Starts a scrollbar frame around `outline`.
    #[must_use]
    pub fn new(outline: OutlineView) -> Self {
        Self {
            outline,
            gap: DEFAULT_OUTLINE_SCROLLBAR_GAP,
            scrollbar_thickness: DEFAULT_OUTLINE_SCROLLBAR_THICKNESS,
            thumb_extent: DEFAULT_OUTLINE_SCROLLBAR_THUMB_EXTENT,
        }
    }

    /// Sets the gap between the outline viewport and the reserved scrollbar.
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

impl AppendSpec for OutlineViewScrollArea {
    fn append_to(mut self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let (width, height) = self
            .outline
            .size
            .unwrap_or((DEFAULT_OUTLINE_WIDTH, DEFAULT_OUTLINE_HEIGHT));
        self.outline.size = Some((width, height));
        let enabled = self.outline.state.enabled;
        let frame_width = width + self.gap + self.scrollbar_thickness;

        let frame = ui.append_widget(parent, Box::new(OutlineViewScrollAreaWidget::default()));
        ui.set_layout(
            frame,
            Layout::row(self.gap).logical_size(frame_width, height),
        );
        ui.set_control_state(frame, ControlState::interactive().focusable(false));

        let outline = ui.append(frame, self.outline);
        let scrollbar = ui.append(
            frame,
            ScrollBar::vertical(outline)
                .length(height)
                .thickness(self.scrollbar_thickness)
                .thumb_extent(self.thumb_extent)
                .enabled(enabled),
        );
        set_outline_view_scroll_area_parts(ui, frame, outline, scrollbar);
        frame
    }
}

/// Per-element widget for an [`OutlineView`] scrollbar frame.
#[derive(Debug, Default)]
pub(crate) struct OutlineViewScrollAreaWidget {
    outline: core::cell::Cell<Option<ElementId>>,
    scrollbar: core::cell::Cell<Option<ElementId>>,
}

impl Widget for OutlineViewScrollAreaWidget {
    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::GenericContainer)
    }
}

fn set_outline_view_scroll_area_parts(
    ui: &Ui,
    frame: ElementId,
    outline: ElementId,
    scrollbar: ElementId,
) {
    if let Some(widget) = outline_view_scroll_area_widget(ui, frame) {
        widget.outline.set(Some(outline));
        widget.scrollbar.set(Some(scrollbar));
    }
}

fn outline_view_scroll_area_widget(
    ui: &Ui,
    frame: ElementId,
) -> Option<&OutlineViewScrollAreaWidget> {
    ui.get(frame)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<OutlineViewScrollAreaWidget>())
}

/// Returns the inner outline target for an [`OutlineViewScrollArea`].
#[must_use]
pub fn outline_view_scroll_area_outline(ui: &Ui, frame: ElementId) -> Option<ElementId> {
    outline_view_scroll_area_widget(ui, frame).and_then(|widget| widget.outline.get())
}

/// Returns the reserved scrollbar for an [`OutlineViewScrollArea`].
#[must_use]
pub fn outline_view_scroll_area_scrollbar(ui: &Ui, frame: ElementId) -> Option<ElementId> {
    outline_view_scroll_area_widget(ui, frame).and_then(|widget| widget.scrollbar.get())
}

#[derive(Clone, Debug)]
struct OwnedOutlineModel {
    nodes: Vec<OutlineViewNode>,
    first_root: Option<usize>,
}

impl OutlineModel for OwnedOutlineModel {
    type Key = usize;
    type Item = String;

    fn first_root_key(&self) -> Option<Self::Key> {
        self.first_root.filter(|key| self.contains_key(key))
    }

    fn contains_key(&self, key: &Self::Key) -> bool {
        *key < self.nodes.len()
    }

    fn next_sibling_key(&self, key: &Self::Key) -> Option<Self::Key> {
        self.nodes.get(*key).and_then(|node| node.next_sibling)
    }

    fn first_child_key(&self, key: &Self::Key) -> Option<Self::Key> {
        self.nodes.get(*key).and_then(|node| node.first_child)
    }

    fn item(&self, key: &Self::Key) -> Option<Self::Item> {
        self.nodes.get(*key).map(|node| node.item.clone())
    }
}

/// Per-element widget for `OutlineView`.
pub(crate) struct OutlineViewWidget {
    outline: Outline<OwnedOutlineModel>,
    scroll_state: ScrollState,
    virtual_list: OutlineVirtualizer,
    item_extent: f64,
    selected_key: Option<usize>,
    realized: Vec<RealizedOutlineItem>,
    last_viewport: Size,
}

impl core::fmt::Debug for OutlineViewWidget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OutlineViewWidget")
            .field("visible_len", &self.visible_len())
            .field("item_extent", &self.item_extent)
            .field("selected_key", &self.selected_key)
            .field("realized", &self.realized)
            .finish_non_exhaustive()
    }
}

impl Widget for OutlineViewWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_OUTLINE_VIEW)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::Tree)
    }

    fn configure_accessibility(
        &self,
        element: &Element,
        ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        node.set_size_of_set(self.visible_len());
        let offset = self.scroll_state.offset();
        let max = self.scroll_state.max_offset();
        node.set_scroll_y(offset.y);
        node.set_scroll_y_min(0.0);
        node.set_scroll_y_max(max.y);
        if ctx.is_enabled(element.id()) && max.y > f64::EPSILON {
            node.add_action(accesskit::Action::ScrollUp);
            node.add_action(accesskit::Action::ScrollDown);
            node.add_action(accesskit::Action::SetScrollOffset);
        }
        if let Some(selected_key) = self.selected_key
            && let Some(item) = self.realized.iter().find(|item| item.key == selected_key)
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

    fn post_layout(&mut self, outline: ElementId, ui: &mut Ui) -> bool {
        self.sync_virtual_items(outline, ui)
    }

    fn handle_scroll(&mut self, ctx: &mut WidgetEventContext<'_>, event: ScrollEvent<'_>) {
        let outline = ctx.id();
        let Some(delta) = wheel_scroll_delta(ctx, outline, event) else {
            return;
        };
        if delta.y != 0.0 {
            self.scroll_by(outline, ctx, delta.y);
            event.mark_handled();
        }
    }

    fn handle_key(&mut self, ctx: &mut WidgetEventContext<'_>, event: KeyEvent<'_>) {
        use ui_events::keyboard::{Key, KeyState, NamedKey};

        let outline = ctx.id();
        if !matches!(event.phase, Phase::Target) {
            return;
        }
        if event.key.state != KeyState::Down
            || !ctx.is_enabled(outline)
            || self.visible_len_mut() == 0
        {
            return;
        }

        let handled = match &event.key.key {
            Key::Named(NamedKey::ArrowDown) => self.move_selection_by(outline, ctx, 1),
            Key::Named(NamedKey::ArrowUp) => self.move_selection_by(outline, ctx, -1),
            Key::Named(NamedKey::PageDown) => {
                let count = self.visible_item_count();
                self.move_selection_by(outline, ctx, count.try_into().unwrap_or(isize::MAX))
            }
            Key::Named(NamedKey::PageUp) => {
                let count = self.visible_item_count();
                let delta = count
                    .try_into()
                    .map_or(isize::MIN + 1, |value: isize| -value);
                self.move_selection_by(outline, ctx, delta)
            }
            Key::Named(NamedKey::Home) => self.select_visible_index(outline, ctx, 0),
            Key::Named(NamedKey::End) => {
                let last = self.visible_len_mut() - 1;
                self.select_visible_index(outline, ctx, last)
            }
            Key::Named(NamedKey::ArrowRight) => self.expand_or_advance(outline, ctx),
            Key::Named(NamedKey::ArrowLeft) => self.collapse_selected(outline, ctx),
            Key::Named(NamedKey::Enter) => self.toggle_selected(outline, ctx),
            Key::Character(s) if s == " " => self.toggle_selected(outline, ctx),
            _ => false,
        };

        if handled {
            event.mark_handled();
        }
    }

    fn handle_accessibility_action(
        &mut self,
        ctx: &mut WidgetEventContext<'_>,
        request: &accesskit::ActionRequest,
    ) -> bool {
        let outline = ctx.id();
        if !ctx.is_enabled(outline) {
            return false;
        }
        match request.action {
            accesskit::Action::ScrollUp => {
                self.scroll_by_accessibility_action(outline, ctx, -1.0, request)
            }
            accesskit::Action::ScrollDown => {
                self.scroll_by_accessibility_action(outline, ctx, 1.0, request)
            }
            accesskit::Action::SetScrollOffset => {
                let Some(accesskit::ActionData::SetScrollOffset(point)) = request.data.as_ref()
                else {
                    return false;
                };
                self.set_scroll_offset_y(outline, ctx, point.y);
                true
            }
            _ => false,
        }
    }
}

impl OutlineViewWidget {
    fn sync_virtual_items(&mut self, outline: ElementId, ui: &mut Ui) -> bool {
        let sync = self.sync_virtual_metrics(outline, ui);
        let mut changed = sync.changed;
        changed |= self.reconcile_realized_items(outline, ui, sync.range);
        changed |= self.layout_realized_items(ui, sync.viewport);
        self.sync_realized_selected_pseudos(ui);
        changed
    }

    fn sync_virtual_metrics(&mut self, outline: ElementId, ui: &mut Ui) -> OutlineVirtualSync {
        let bounds = ui.effective_bounds(outline);
        let viewport = Size::new(bounds.width().max(0.0), bounds.height().max(0.0));
        let viewport_changed = !sizes_nearly_equal(viewport, self.last_viewport);
        self.last_viewport = viewport;

        let visible_len = self.visible_len_mut();
        self.virtual_list.set_len(visible_len);
        self.virtual_list.set_viewport_extent(viewport.height);
        self.virtual_list
            .set_scroll_offset(self.scroll_state.offset_y());
        self.virtual_list.clamp_scroll_to_content();
        let offset_changed = self
            .scroll_state
            .set_offset_y_raw(self.virtual_list.scroll_offset());
        let content_height = self.virtual_list.total_extent();
        self.scroll_state
            .set_metrics(viewport, Size::new(viewport.width, content_height));

        OutlineVirtualSync {
            viewport,
            range: self.virtual_list.materialized_range(),
            changed: viewport_changed || offset_changed,
        }
    }

    fn reconcile_realized_items(
        &mut self,
        outline: ElementId,
        ui: &mut Ui,
        range: Range<usize>,
    ) -> bool {
        let rows = self.outline.visible_rows().to_vec();
        let mut changed = false;
        let mut kept = Vec::with_capacity(self.realized.len());
        for item in self.realized.drain(..) {
            let keep = range.contains(&item.index)
                && rows.get(item.index).is_some_and(|row| row.key == item.key)
                && ui.get(item.element).is_some();
            if keep {
                if let Some(row) = rows.get(item.index)
                    && let Some(label) = self.outline.item(&row.key)
                {
                    update_realized_outline_item(
                        ui,
                        item.element,
                        &OutlineRowSnapshot::new(item.index, row, label),
                    );
                }
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
            let Some(row) = rows.get(index) else {
                continue;
            };
            let Some(label) = self.outline.item(&row.key) else {
                continue;
            };
            let snapshot = OutlineRowSnapshot::new(index, row, label);
            let element = append_realized_outline_item(ui, outline, &snapshot);
            self.realized.push(RealizedOutlineItem {
                index,
                key: snapshot.key,
                element,
            });
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
            let size = LayoutSize::logical(width, height);
            let placement = LayoutPlacement::absolute_at(0.0, top);
            let size_changed = ui
                .get(item.element)
                .and_then(|element| element.get_local_value(ui.properties().layout_size))
                != Some(&size);
            let placement_changed = ui
                .get(item.element)
                .and_then(|element| element.get_local_value(ui.properties().layout_placement))
                != Some(&placement);
            if size_changed || placement_changed {
                let props = *ui.properties();
                ui.set_local(item.element, props.layout_size, size);
                ui.set_local(item.element, props.layout_placement, placement);
                changed = true;
            }
        }
        changed
    }

    fn scroll_by(&mut self, outline: ElementId, ui: &mut Ui, delta_y: f64) {
        let next = self.virtual_list.scroll_offset() + delta_y;
        self.set_scroll_offset_y(outline, ui, next);
    }

    fn scroll_by_accessibility_action(
        &mut self,
        outline: ElementId,
        ui: &mut Ui,
        direction: f64,
        request: &accesskit::ActionRequest,
    ) -> bool {
        let Some(unit) = action_scroll_unit(request) else {
            return false;
        };
        let amount = match unit {
            accesskit::ScrollUnit::Item => self.item_extent,
            accesskit::ScrollUnit::Page => {
                self.virtual_list.viewport_extent().max(self.item_extent)
            }
        };
        self.scroll_by(outline, ui, amount * direction);
        true
    }

    fn set_scroll_offset_y(&mut self, outline: ElementId, ui: &mut Ui, offset: f64) {
        self.virtual_list
            .set_scroll_offset(normalize_scroll_offset_y(offset));
        self.virtual_list.clamp_scroll_to_content();
        let _ = self
            .scroll_state
            .set_offset_y_raw(self.virtual_list.scroll_offset());
        ui.mark_channels(outline, LAYOUT.into());
        let _ = self.sync_virtual_items(outline, ui);
    }

    fn visible_len(&self) -> usize {
        self.outline.clone().visible_len()
    }

    fn visible_len_mut(&mut self) -> usize {
        self.outline.visible_len()
    }

    fn selected_visible_index(&mut self) -> Option<usize> {
        self.selected_key
            .and_then(|key| self.outline.index_of_key(&key))
    }

    fn select_visible_index(&mut self, outline: ElementId, ui: &mut Ui, index: usize) -> bool {
        let Some(row) = self.outline.visible_row(index).cloned() else {
            return false;
        };
        self.selected_key = Some(row.key);
        self.scroll_index_into_view(outline, ui, index);
        self.sync_realized_selected_pseudos(ui);
        true
    }

    fn select_key(&mut self, outline: ElementId, ui: &mut Ui, key: usize) -> bool {
        if self.outline.model().contains_key(&key) {
            self.selected_key = Some(key);
            if let Some(index) = self.outline.index_of_key(&key) {
                self.scroll_index_into_view(outline, ui, index);
            }
            self.sync_realized_selected_pseudos(ui);
            return true;
        }
        false
    }

    fn move_selection_by(&mut self, outline: ElementId, ui: &mut Ui, delta: isize) -> bool {
        let len = self.visible_len_mut();
        if len == 0 {
            return false;
        }
        let current = self.selected_visible_index().unwrap_or(0);
        let next = current.saturating_add_signed(delta).min(len - 1);
        self.select_visible_index(outline, ui, next)
    }

    fn scroll_index_into_view(&mut self, outline: ElementId, ui: &mut Ui, index: usize) {
        self.virtual_list
            .scroll_to_index(index, ScrollAlign::Nearest);
        let _ = self
            .scroll_state
            .set_offset_y_raw(self.virtual_list.scroll_offset());
        ui.mark_channels(outline, LAYOUT.into());
        let _ = self.sync_virtual_items(outline, ui);
    }

    fn toggle_selected(&mut self, outline: ElementId, ui: &mut Ui) -> bool {
        let Some(key) = self.selected_key else {
            return false;
        };
        self.toggle_key_expanded(outline, ui, key)
    }

    fn expand_or_advance(&mut self, outline: ElementId, ui: &mut Ui) -> bool {
        let Some(index) = self.selected_visible_index() else {
            return self.select_visible_index(outline, ui, 0);
        };
        let Some(row) = self.outline.visible_row(index).cloned() else {
            return false;
        };
        if row.has_children && !row.is_expanded {
            return self.set_key_expanded(outline, ui, row.key, true);
        }
        let last = self.visible_len_mut() - 1;
        self.select_visible_index(outline, ui, (index + 1).min(last))
    }

    fn collapse_selected(&mut self, outline: ElementId, ui: &mut Ui) -> bool {
        let Some(index) = self.selected_visible_index() else {
            return false;
        };
        let Some(row) = self.outline.visible_row(index).cloned() else {
            return false;
        };
        if row.has_children && row.is_expanded {
            return self.set_key_expanded(outline, ui, row.key, false);
        }
        false
    }

    fn toggle_key_expanded(&mut self, outline: ElementId, ui: &mut Ui, key: usize) -> bool {
        let expanded = self.outline.is_expanded(&key);
        self.set_key_expanded(outline, ui, key, !expanded)
    }

    fn set_key_expanded(
        &mut self,
        outline: ElementId,
        ui: &mut Ui,
        key: usize,
        expanded: bool,
    ) -> bool {
        if self.outline.model().first_child_key(&key).is_none() {
            return false;
        }
        if !self.outline.set_expanded(key, expanded) {
            return false;
        }

        if self
            .selected_key
            .is_none_or(|selected| self.outline.index_of_key(&selected).is_none())
        {
            self.selected_key = Some(key);
        }
        let visible_len = self.visible_len_mut();
        self.virtual_list.set_len(visible_len);
        self.virtual_list.clamp_scroll_to_content();
        let _ = self
            .scroll_state
            .set_offset_y_raw(self.virtual_list.scroll_offset());
        ui.mark_channels(outline, LAYOUT.into());
        let _ = self.sync_virtual_items(outline, ui);
        true
    }

    fn sync_realized_selected_pseudos(&self, ui: &mut Ui) {
        for item in &self.realized {
            if Some(item.key) == self.selected_key {
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

/// Per-element widget for a realized `OutlineView` item.
#[derive(Debug)]
pub(crate) struct OutlineItemWidget {
    key: usize,
    index: usize,
    depth: usize,
    has_children: bool,
    is_expanded: bool,
}

impl Widget for OutlineItemWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_OUTLINE_ITEM)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::TreeItem)
    }

    fn configure_accessibility(
        &self,
        element: &Element,
        ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        ctx.add_click_action_if_enabled(element, node);
        ctx.apply_selected_state(element, node);
        node.set_level(self.depth + 1);
        node.set_position_in_set(self.index + 1);
        if self.has_children {
            node.set_expanded(self.is_expanded);
            if self.is_expanded {
                node.add_action(accesskit::Action::Collapse);
            } else {
                node.add_action(accesskit::Action::Expand);
            }
        }
    }

    fn default_template(&self) -> Option<&'static dyn crate::Template> {
        Some(&OUTLINE_ITEM_TEMPLATE)
    }

    fn handle_click(&mut self, ctx: &mut WidgetEventContext<'_>, event: ClickEvent<'_>) {
        let item = ctx.id();
        if !matches!(event.phase, Phase::Target | Phase::Bubble) || !ctx.is_enabled(item) {
            return;
        }
        let Some(outline) = outline_parent(ctx, item) else {
            return;
        };
        let _ = select_outline_key(ctx, outline, self.key);
        if self.has_children {
            let _ = toggle_outline_key(ctx, outline, self.key);
        }
        let _ = ctx.set_pointer_focus(Some(outline));
        event.mark_handled();
    }

    fn handle_accessibility_action(
        &mut self,
        ctx: &mut WidgetEventContext<'_>,
        request: &accesskit::ActionRequest,
    ) -> bool {
        let Some(outline) = outline_parent(ctx, ctx.id()) else {
            return false;
        };
        match request.action {
            accesskit::Action::Expand => set_outline_key_expanded(ctx, outline, self.key, true),
            accesskit::Action::Collapse => set_outline_key_expanded(ctx, outline, self.key, false),
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RealizedOutlineItem {
    index: usize,
    key: usize,
    element: ElementId,
}

#[derive(Clone, Debug)]
struct OutlineVirtualSync {
    viewport: Size,
    range: Range<usize>,
    changed: bool,
}

struct OutlineRowSnapshot {
    index: usize,
    key: usize,
    depth: usize,
    has_children: bool,
    is_expanded: bool,
    label: String,
}

impl OutlineRowSnapshot {
    fn new(index: usize, row: &VisibleRow<usize>, label: String) -> Self {
        Self {
            index,
            key: row.key,
            depth: row.depth,
            has_children: row.has_children,
            is_expanded: row.is_expanded,
            label,
        }
    }
}

type OutlineItemTemplate = TemplateSpec;

static OUTLINE_ITEM_TEMPLATE: OutlineItemTemplate = TemplateSpec::new(OUTLINE_ITEM_TEMPLATE_NODES);

const OUTLINE_ITEM_LABEL_BINDINGS: &[TemplateBinding] = &[
    TemplateBinding::owner_to_part(
        TemplateBuiltInProperty::Content,
        TemplateBuiltInProperty::Content,
    ),
    TemplateBinding::owner_to_part(
        TemplateBuiltInProperty::FontSize,
        TemplateBuiltInProperty::FontSize,
    ),
];

const OUTLINE_ITEM_CONTENT_CHILDREN: &[TemplateNode] = &[
    TemplateNode::widget(TemplateWidgetFactory::new(outline_item_disclosure_widget))
        .with_part(PART_OUTLINE_ITEM_DISCLOSURE)
        .with_slot(SLOT_OUTLINE_ITEM_DISCLOSURE),
    TemplateNode::widget(TemplateWidgetFactory::new(outline_item_label_widget))
        .with_part(PART_OUTLINE_ITEM_LABEL)
        .with_slot(SLOT_OUTLINE_ITEM_LABEL)
        .with_bindings(OUTLINE_ITEM_LABEL_BINDINGS),
];

const OUTLINE_ITEM_TEMPLATE_NODES: &[TemplateNode] =
    &[
        TemplateNode::widget(TemplateWidgetFactory::new(outline_item_content_widget))
            .with_part(PART_OUTLINE_ITEM_CONTENT)
            .with_children(OUTLINE_ITEM_CONTENT_CHILDREN),
    ];

fn outline_item_content_widget() -> Box<dyn Widget> {
    Box::new(SurfacePartWidget)
}

fn outline_item_disclosure_widget() -> Box<dyn Widget> {
    Box::new(TextWidget)
}

fn outline_item_label_widget() -> Box<dyn Widget> {
    Box::new(TextWidget)
}

fn append_realized_outline_item(
    ui: &mut Ui,
    outline: ElementId,
    item: &OutlineRowSnapshot,
) -> ElementId {
    let id = ui.append_widget(
        outline,
        Box::new(OutlineItemWidget {
            key: item.key,
            index: item.index,
            depth: item.depth,
            has_children: item.has_children,
            is_expanded: item.is_expanded,
        }),
    );
    update_realized_outline_item(ui, id, item);
    ui.set_control_state(id, ControlState::interactive().focusable(false));
    id
}

fn update_realized_outline_item(ui: &mut Ui, id: ElementId, item: &OutlineRowSnapshot) {
    ui.with_widget_mut(id, |widget, _ui| {
        if let Some(widget) = widget.downcast_mut::<OutlineItemWidget>() {
            widget.key = item.key;
            widget.index = item.index;
            widget.depth = item.depth;
            widget.has_children = item.has_children;
            widget.is_expanded = item.is_expanded;
        }
    });
    ui.set_local(id, ui.properties().content, item.label.clone());
    if let Some(disclosure) = ui.slot(id, SLOT_OUTLINE_ITEM_DISCLOSURE) {
        ui.set_local(
            disclosure,
            ui.properties().content,
            disclosure_text(item.depth, item.has_children, item.is_expanded),
        );
        ui.set_local(
            disclosure,
            ui.properties().layout_size,
            LayoutSize::new()
                .width(LayoutDimension::Logical(disclosure_width(item.depth)))
                .height(LayoutDimension::Percent(1.0)),
        );
    }
}

/// Selects `key` in `outline`.
///
/// Returns `false` if `outline` is stale, disabled, not an `OutlineView`, or
/// if `key` is outside the model.
pub fn select_outline_key(ui: &mut Ui, outline: ElementId, key: usize) -> bool {
    with_outline_view_mut(ui, outline, |widget, ui| {
        widget.select_key(outline, ui, key)
    })
    .unwrap_or(false)
}

/// Returns the currently selected node key in `outline`, if any.
#[must_use]
pub fn selected_outline_key(ui: &Ui, outline: ElementId) -> Option<usize> {
    outline_view_widget(ui, outline).and_then(|widget| widget.selected_key)
}

/// Returns whether `key` is currently expanded in `outline`.
#[must_use]
pub fn outline_key_expanded(ui: &Ui, outline: ElementId, key: usize) -> Option<bool> {
    outline_view_widget(ui, outline).map(|widget| widget.outline.is_expanded(&key))
}

/// Sets whether `key` is expanded in `outline`.
///
/// Returns `false` if `outline` is stale, disabled, not an `OutlineView`, if
/// `key` does not have children, or if the state was already correct.
pub fn set_outline_key_expanded(
    ui: &mut Ui,
    outline: ElementId,
    key: usize,
    expanded: bool,
) -> bool {
    with_outline_view_mut(ui, outline, |widget, ui| {
        widget.set_key_expanded(outline, ui, key, expanded)
    })
    .unwrap_or(false)
}

/// Toggles expansion for `key` in `outline`.
pub fn toggle_outline_key(ui: &mut Ui, outline: ElementId, key: usize) -> bool {
    with_outline_view_mut(ui, outline, |widget, ui| {
        widget.toggle_key_expanded(outline, ui, key)
    })
    .unwrap_or(false)
}

/// Returns the outline's current vertical scroll offset in logical pixels.
#[must_use]
pub fn outline_scroll_offset_y(ui: &Ui, outline: ElementId) -> Option<f64> {
    outline_view_widget(ui, outline).map(|widget| state_scroll_offset_y(&widget.scroll_state))
}

/// Sets the outline's vertical scroll offset in logical pixels.
///
/// Returns `false` if `outline` is stale or does not own outline state.
pub fn set_outline_scroll_offset_y(ui: &mut Ui, outline: ElementId, offset: f64) -> bool {
    with_outline_view_mut(ui, outline, |widget, ui| {
        widget.set_scroll_offset_y(outline, ui, offset);
        true
    })
    .unwrap_or(false)
}

/// Returns the node key represented by a realized outline item.
#[must_use]
pub fn realized_outline_item_key(ui: &Ui, item: ElementId) -> Option<usize> {
    ui.get(item)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<OutlineItemWidget>())
        .map(|widget| widget.key)
}

#[cfg(test)]
pub(crate) fn realized_outline_item(ui: &Ui, outline: ElementId, key: usize) -> Option<ElementId> {
    outline_view_widget(ui, outline)?
        .realized
        .iter()
        .find(|item| item.key == key)
        .map(|item| item.element)
        .filter(|item| ui.get(*item).is_some())
}

fn outline_parent(ui: &Ui, item: ElementId) -> Option<ElementId> {
    let parent = ui.get(item)?.parent()?;
    direct_outline_view_widget(ui, parent)
        .is_some()
        .then_some(parent)
}

fn outline_view_widget(ui: &Ui, outline: ElementId) -> Option<&OutlineViewWidget> {
    direct_outline_view_widget(ui, outline_view_target(ui, outline)?)
}

fn direct_outline_view_widget(ui: &Ui, outline: ElementId) -> Option<&OutlineViewWidget> {
    ui.get(outline)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<OutlineViewWidget>())
}

fn outline_view_target(ui: &Ui, id: ElementId) -> Option<ElementId> {
    if direct_outline_view_widget(ui, id).is_some() {
        return Some(id);
    }
    outline_view_scroll_area_outline(ui, id)
}

fn with_outline_view_mut<R>(
    ui: &mut Ui,
    outline: ElementId,
    f: impl FnOnce(&mut OutlineViewWidget, &mut Ui) -> R,
) -> Option<R> {
    let outline = outline_view_target(ui, outline)?;
    let mut output = None;
    ui.with_widget_mut(outline, |widget, ui| {
        if let Some(outline_widget) = widget.downcast_mut::<OutlineViewWidget>() {
            output = Some(f(outline_widget, ui));
        }
    });
    output
}

fn disclosure_text(depth: usize, has_children: bool, is_expanded: bool) -> String {
    let mut text = String::new();
    for _ in 0..depth {
        text.push_str("  ");
    }
    text.push_str(match (has_children, is_expanded) {
        (true, true) => "▾",
        (true, false) => "▸",
        (false, _) => " ",
    });
    text
}

fn disclosure_width(depth: usize) -> f32 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "outline depth is UI-local and intentionally maps to logical pixels"
    )]
    {
        OUTLINE_DISCLOSURE_WIDTH + ((depth as f32) * OUTLINE_INDENT_PER_DEPTH)
    }
}

fn normalize_item_extent(extent: f64) -> f64 {
    if extent.is_finite() {
        extent.max(1.0)
    } else {
        DEFAULT_OUTLINE_ITEM_EXTENT
    }
}

fn sizes_nearly_equal(a: Size, b: Size) -> bool {
    (a.width - b.width).abs() <= f64::EPSILON && (a.height - b.height).abs() <= f64::EPSILON
}

#[cfg(test)]
mod tests {
    use alloc::string::String;
    use alloc::vec;

    use accesskit::{Node, NodeId, Role, TreeUpdate};
    use kurbo::Point;
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

    use super::*;
    use crate::controls::scroll::{SLOT_SCROLL_BAR_THUMB, ScrollBar};
    use crate::pseudos::PSEUDO_SELECTED;
    use crate::test_support::{pointer_down, pointer_move, pointer_up};

    fn node(label: &str) -> OutlineViewNode {
        OutlineViewNode::new(String::from(label))
    }

    fn test_outline() -> OutlineView {
        OutlineView::new()
            .nodes(
                vec![
                    node("Root").with_first_child(Some(1)),
                    node("Child").with_next_sibling(Some(2)),
                    node("Sibling"),
                ],
                Some(0),
            )
            .size(180.0, 90.0)
            .item_extent(24.0)
            .overscan(0.0)
    }

    #[test]
    fn clicking_parent_toggles_expansion_and_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let outline = ui.append(root, test_outline());
        realize(&mut ui);

        let parent = realized_outline_item(&ui, outline, 0).unwrap();
        assert!(realized_outline_item(&ui, outline, 1).is_none());

        ui.synthesize_click(parent, Point::ZERO);
        realize(&mut ui);

        assert_eq!(selected_outline_key(&ui, outline), Some(0));
        assert_eq!(outline_key_expanded(&ui, outline, 0), Some(true));
        assert!(realized_outline_item(&ui, outline, 1).is_some());
        assert!(ui.get(parent).unwrap().pseudos().contains(&PSEUDO_SELECTED));

        let parent = realized_outline_item(&ui, outline, 0).unwrap();
        ui.synthesize_click(parent, Point::ZERO);
        realize(&mut ui);

        assert_eq!(outline_key_expanded(&ui, outline, 0), Some(false));
        assert!(realized_outline_item(&ui, outline, 1).is_none());
    }

    #[test]
    fn keyboard_expands_collapses_and_moves_outline_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let outline = ui.append(root, test_outline().selected_key(0));
        realize(&mut ui);
        ui.set_focus(Some(outline));

        let right = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
        assert!(ui.dispatch_key(&right).handled);
        realize(&mut ui);
        assert_eq!(outline_key_expanded(&ui, outline, 0), Some(true));
        assert!(realized_outline_item(&ui, outline, 1).is_some());

        assert!(ui.dispatch_key(&right).handled);
        realize(&mut ui);
        assert_eq!(selected_outline_key(&ui, outline), Some(1));

        let up = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowUp), Code::ArrowUp);
        assert!(ui.dispatch_key(&up).handled);
        assert_eq!(selected_outline_key(&ui, outline), Some(0));

        let left = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowLeft), Code::ArrowLeft);
        assert!(ui.dispatch_key(&left).handled);
        realize(&mut ui);
        assert_eq!(outline_key_expanded(&ui, outline, 0), Some(false));
        assert!(realized_outline_item(&ui, outline, 1).is_none());
    }

    #[test]
    fn outline_scroll_area_bar_scrolls_virtualized_outline() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 90.0));
        let frame = ui.append(
            root,
            OutlineView::new()
                .nodes(scrolling_nodes(100), Some(0))
                .expanded(0)
                .size(140.0, 56.0)
                .item_extent(20.0)
                .overscan(0.0)
                .with_vertical_scrollbar()
                .thumb_extent(12.0),
        );
        let outline = outline_view_scroll_area_outline(&ui, frame)
            .expect("scroll frame should expose outline");
        let bar = outline_view_scroll_area_scrollbar(&ui, frame)
            .expect("scroll frame should expose scrollbar");

        realize(&mut ui);
        assert!(realized_outline_item(&ui, outline, 1).is_some());
        let bar_bounds = ui.effective_bounds(bar);
        let target = Point::new(bar_bounds.center().x, bar_bounds.y1 - 1.0);

        assert!(
            ui.dispatch_pointer_event(&pointer_down(target, 1))
                .changed()
        );
        assert!(ui.dispatch_pointer_event(&pointer_up(target, 2)).handled);
        realize(&mut ui);

        assert!(outline_scroll_offset_y(&ui, frame).is_some_and(|offset| offset > 0.0));
        assert!(realized_outline_item(&ui, frame, 1).is_none());
    }

    #[test]
    fn external_scrollbar_thumb_drag_scrolls_outline() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(8.0).logical_size(180.0, 80.0));
        let outline = ui.append(
            root,
            OutlineView::new()
                .nodes(scrolling_nodes(100), Some(0))
                .expanded(0)
                .size(140.0, 56.0)
                .item_extent(20.0)
                .overscan(0.0),
        );
        let bar = ui.append(
            root,
            ScrollBar::vertical(outline)
                .length(56.0)
                .thickness(8.0)
                .thumb_extent(12.0),
        );
        let thumb = ui
            .slot(bar, SLOT_SCROLL_BAR_THUMB)
            .expect("scrollbar template should create a thumb slot");

        realize(&mut ui);
        let start = ui.effective_bounds(thumb).center();
        assert!(ui.dispatch_pointer_event(&pointer_down(start, 1)).changed());
        assert!(
            ui.dispatch_pointer_event(&pointer_move(Point::new(start.x, start.y + 30.0), 2, true))
                .changed()
        );
        realize(&mut ui);

        assert!(outline_scroll_offset_y(&ui, outline).is_some_and(|offset| offset > 0.0));
        assert!(realized_outline_item(&ui, outline, 1).is_none());
    }

    #[test]
    fn outline_accessibility_exposes_tree_items_and_expansion() {
        let mut ui = Ui::new();
        let root = ui.root();
        let outline = ui.append(root, test_outline().expanded(0).selected_key(1));
        realize(&mut ui);
        let child = realized_outline_item(&ui, outline, 1).unwrap();
        let update = ui.accesskit_tree_update();

        let outline_node = accesskit_node_for(&update, outline);
        assert_eq!(outline_node.role(), Role::Tree);
        assert_eq!(outline_node.size_of_set(), Some(3));

        let child_node = accesskit_node_for(&update, child);
        assert_eq!(child_node.role(), Role::TreeItem);
        assert_eq!(child_node.level(), Some(2));
        assert_eq!(child_node.is_selected(), Some(true));
    }

    fn scrolling_nodes(len: usize) -> Vec<OutlineViewNode> {
        let mut nodes = Vec::with_capacity(len + 1);
        nodes.push(node("Root").with_first_child(Some(1)));
        for index in 1..=len {
            let next = (index < len).then_some(index + 1);
            nodes.push(OutlineViewNode::new(format!("Item {index}")).with_next_sibling(next));
        }
        nodes
    }

    fn realize(ui: &mut Ui) {
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

    fn accesskit_node_id(element: ElementId) -> NodeId {
        NodeId((u64::from(element.generation()) << 32) | u64::from(element.slot()))
    }
}
