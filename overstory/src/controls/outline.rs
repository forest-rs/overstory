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
use understory_outline::{Outline, OutlineModel, VisibleRow};
use understory_selection::Selection;
use understory_style::{PartTag, TypeTag};

use crate::behavior::{ElementRealizeContext, ElementRealizeResult, WidgetEventContext};
use crate::controls::scroll::{
    normalize_scroll_offset_y, state_scroll_offset_y, wheel_scroll_delta,
};
use crate::controls::support::{
    CallbackSlot, EventHandler, FixedVirtualizer, ReservedVerticalScrollArea,
    append_reserved_vertical_scroll_area, clamp_fixed_virtual_scroll_target,
    configure_vertical_scroll_accessibility, fixed_virtual_accessibility_scroll_delta,
    fixed_virtualizer, fixed_visible_item_count, is_descendant_or_self, layout_fixed_virtual_rows,
    reserved_scroll_frame_widget, scroll_fixed_virtual_index_into_view,
    set_fixed_virtual_scroll_offset_y, sync_fixed_virtual_rows,
};
use crate::element::{Element, ElementId};
use crate::event_runtime::{ClickEvent, KeyEvent, ScrollEvent};
use crate::layouts::{Layout, LayoutDimension, LayoutSize};
use crate::pseudos::PSEUDO_SELECTED;
use crate::scroll_runtime::ScrollState;
use crate::template::{
    TemplateContract, TemplateNode, TemplateSlot, TemplateSpec, TemplateSpecRecipe,
};
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, ElementConfig, ElementSpec, Widget};

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

const DEFAULT_OUTLINE_WIDTH: f64 = 220.0;
const DEFAULT_OUTLINE_HEIGHT: f64 = 160.0;
const DEFAULT_OUTLINE_ITEM_EXTENT: f64 = 30.0;
const DEFAULT_OUTLINE_OVERSCAN: f64 = 60.0;
const DEFAULT_OUTLINE_SCROLLBAR_GAP: f64 = 8.0;
const DEFAULT_OUTLINE_SCROLLBAR_THICKNESS: f64 = 10.0;
const DEFAULT_OUTLINE_SCROLLBAR_THUMB_EXTENT: f64 = 30.0;
const OUTLINE_INDENT_PER_DEPTH: f64 = 18.0;
const OUTLINE_DISCLOSURE_WIDTH: f64 = 26.0;

type OutlineSelectionChangedHandler = EventHandler<OutlineSelectionChangedEvent>;
type OutlineSelectionChangedSlot = CallbackSlot<OutlineSelectionChangedEvent>;
type OutlineExpansionChangedHandler = EventHandler<OutlineExpansionChangedEvent>;
type OutlineExpansionChangedSlot = CallbackSlot<OutlineExpansionChangedEvent>;

/// Stable key for a dense [`OutlineView`] node.
///
/// Keys are indices into the node vector supplied to [`OutlineView::nodes`].
/// The newtype keeps public APIs explicit about key semantics instead of
/// accepting arbitrary row indices.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OutlineNodeKey(usize);

impl OutlineNodeKey {
    /// Creates a key from a dense node index.
    #[must_use]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns this key's dense node index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

impl From<usize> for OutlineNodeKey {
    fn from(index: usize) -> Self {
        Self::new(index)
    }
}

/// A string-backed node for [`OutlineView`].
///
/// The node links use stable [`OutlineNodeKey`] values into the node vector
/// supplied to [`OutlineView::nodes`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutlineViewNode {
    label: String,
    first_child: Option<OutlineNodeKey>,
    next_sibling: Option<OutlineNodeKey>,
}

impl OutlineViewNode {
    /// Creates a leaf node with no children or siblings.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            first_child: None,
            next_sibling: None,
        }
    }

    /// Returns a copy of this node with `first_child` set.
    #[must_use]
    pub fn with_first_child(mut self, first_child: Option<impl Into<OutlineNodeKey>>) -> Self {
        self.first_child = first_child.map(Into::into);
        self
    }

    /// Returns a copy of this node with `next_sibling` set.
    #[must_use]
    pub fn with_next_sibling(mut self, next_sibling: Option<impl Into<OutlineNodeKey>>) -> Self {
        self.next_sibling = next_sibling.map(Into::into);
        self
    }

    /// Returns the node's visible label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Event delivered when an [`OutlineView`] selection changes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutlineSelectionChangedEvent {
    /// The outline whose selection changed.
    pub outline: ElementId,
    /// The selected node key after the change, if any.
    pub selected_key: Option<OutlineNodeKey>,
}

/// Event delivered when an [`OutlineView`] node expands or collapses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutlineExpansionChangedEvent {
    /// The outline whose expansion state changed.
    pub outline: ElementId,
    /// The node key whose expansion state changed.
    pub key: OutlineNodeKey,
    /// Whether the node is expanded after the change.
    pub expanded: bool,
}

/// Semantic row metadata for a realized [`OutlineView`] item.
///
/// The item element is only a realized container; this value describes the
/// stable outline node and its current visible-row projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutlineItemInfo {
    /// The current visible row index.
    pub index: usize,
    /// The stable node key represented by this row.
    pub key: OutlineNodeKey,
    /// The row depth below the outline root. Root rows have depth `0`.
    pub depth: usize,
    /// One-based position among this node's siblings.
    pub position_in_set: usize,
    /// Number of siblings in this node's sibling set.
    pub size_of_set: usize,
    /// Whether this node can expand.
    pub has_children: bool,
    /// Whether this node is currently expanded.
    pub is_expanded: bool,
}

/// Builder + spec for a virtualized outline view.
///
/// `OutlineView` owns its expansion state and realizes only the currently
/// visible rows. Node identity is the node's stable [`OutlineNodeKey`], so
/// selection and expansion do not drift when siblings expand or collapse.
pub struct OutlineView {
    nodes: Vec<OutlineViewNode>,
    first_root: Option<OutlineNodeKey>,
    expanded: Vec<OutlineNodeKey>,
    size: Option<(f64, f64)>,
    item_extent: f64,
    overscan: f64,
    selected_key: Option<OutlineNodeKey>,
    scroll_offset_y: f64,
    element: ElementConfig,
    on_selection_changed: Option<OutlineSelectionChangedHandler>,
    on_expansion_changed: Option<OutlineExpansionChangedHandler>,
}

impl core::fmt::Debug for OutlineView {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OutlineView")
            .field("nodes_len", &self.nodes.len())
            .field("first_root", &self.first_root)
            .field("expanded", &self.expanded)
            .field("size", &self.size)
            .field("item_extent", &self.item_extent)
            .field("overscan", &self.overscan)
            .field("selected_key", &self.selected_key)
            .field("scroll_offset_y", &self.scroll_offset_y)
            .field("control_state", &self.control_state())
            .finish_non_exhaustive()
    }
}

impl OutlineView {
    /// Starts a new empty outline view.
    #[must_use]
    pub fn new() -> Self {
        let mut element = ElementConfig::default();
        element.set_control_state(ControlState::interactive());
        Self {
            nodes: Vec::new(),
            first_root: None,
            expanded: Vec::new(),
            size: None,
            item_extent: DEFAULT_OUTLINE_ITEM_EXTENT,
            overscan: DEFAULT_OUTLINE_OVERSCAN,
            selected_key: None,
            scroll_offset_y: 0.0,
            element,
            on_selection_changed: None,
            on_expansion_changed: None,
        }
    }

    /// Replaces the outline nodes and first root key.
    ///
    /// Node keys are indices into `nodes`. Each node's `first_child` and
    /// `next_sibling` fields must point at indices in the same vector.
    #[must_use]
    pub fn nodes(
        mut self,
        nodes: Vec<OutlineViewNode>,
        first_root: Option<impl Into<OutlineNodeKey>>,
    ) -> Self {
        self.nodes = nodes;
        self.first_root = first_root.map(Into::into);
        self
    }

    /// Marks a node key as initially expanded.
    #[must_use]
    pub fn expanded(mut self, key: impl Into<OutlineNodeKey>) -> Self {
        let key = key.into();
        if !self.expanded.contains(&key) {
            self.expanded.push(key);
        }
        self
    }

    /// Sets the outline's fixed logical-pixel size.
    #[must_use]
    pub fn size(mut self, width: f64, height: f64) -> Self {
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
    pub fn selected_key(mut self, key: impl Into<OutlineNodeKey>) -> Self {
        self.selected_key = Some(key.into());
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
        self.element
            .update_control_state(|state| state.focusable(focusable));
        self
    }

    /// Sets whether the outline starts enabled.
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

    /// Registers a handler fired after the selected node key changes.
    #[must_use]
    pub fn on_selection_changed<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, OutlineSelectionChangedEvent) + 'static,
    {
        self.on_selection_changed = Some(Box::new(handler));
        self
    }

    /// Registers a handler fired after a node expands or collapses.
    #[must_use]
    pub fn on_expansion_changed<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, OutlineExpansionChangedEvent) + 'static,
    {
        self.on_expansion_changed = Some(Box::new(handler));
        self
    }

    /// Wraps this outline in a reserved vertical scrollbar frame.
    #[must_use]
    pub fn with_vertical_scrollbar(self) -> OutlineViewScrollArea {
        OutlineViewScrollArea::new(self)
    }

    fn control_state(&self) -> ControlState {
        self.element
            .control_state()
            .unwrap_or_else(ControlState::interactive)
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
        let virtual_list =
            fixed_virtualizer(0, self.item_extent, self.scroll_offset_y, self.overscan);

        let mut outline = Outline::new(OwnedOutlineModel {
            nodes: self.nodes,
            first_root: self.first_root,
        });
        let _ = outline.replace_expanded(self.expanded);
        let mut selection = Selection::new();
        if let Some(selected_key) = self
            .selected_key
            .filter(|key| outline.model().contains_key(key))
        {
            let _ = selection.select_only(selected_key);
        }

        let id = ui.append_widget(
            parent,
            Box::new(OutlineViewWidget {
                outline,
                selection,
                selection_changed: OutlineSelectionChangedSlot::new(self.on_selection_changed),
                expansion_changed: OutlineExpansionChangedSlot::new(self.on_expansion_changed),
                scroll_state: scroll_state.clone(),
                virtual_list,
                item_extent: self.item_extent,
                realized: Vec::new(),
                last_viewport: Size::ZERO,
                visible_len: 0,
            }),
        );

        let (width, height) = self
            .size
            .unwrap_or((DEFAULT_OUTLINE_WIDTH, DEFAULT_OUTLINE_HEIGHT));
        ui.register_virtual_scroll_target(id, scroll_state);
        ui.set_layout(id, Layout::new().relative().logical_size(width, height));
        self.element.apply_to(ui, id);
        id
    }
}

impl ElementSpec for OutlineView {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

/// Builder + spec for an [`OutlineView`] with a reserved vertical scrollbar.
pub struct OutlineViewScrollArea {
    outline: OutlineView,
    gap: f64,
    scrollbar_thickness: f64,
    thumb_extent: f64,
    element: ElementConfig,
}

impl core::fmt::Debug for OutlineViewScrollArea {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OutlineViewScrollArea")
            .field("outline", &self.outline)
            .field("gap", &self.gap)
            .field("scrollbar_thickness", &self.scrollbar_thickness)
            .field("thumb_extent", &self.thumb_extent)
            .field("element", &self.element)
            .finish()
    }
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
            element: ElementConfig::default(),
        }
    }

    /// Sets the gap between the outline viewport and the reserved scrollbar.
    #[must_use]
    pub fn gap(mut self, gap: f64) -> Self {
        self.gap = gap.max(0.0);
        self
    }

    /// Sets the reserved scrollbar thickness.
    #[must_use]
    pub fn scrollbar_thickness(mut self, thickness: f64) -> Self {
        self.scrollbar_thickness = thickness.max(1.0);
        self
    }

    /// Sets the fixed scrollbar thumb extent.
    #[must_use]
    pub fn thumb_extent(mut self, extent: f64) -> Self {
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
        let enabled = self.outline.control_state().enabled;

        let (frame, _, _) = append_reserved_vertical_scroll_area(
            ui,
            parent,
            self.outline,
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

impl ElementSpec for OutlineViewScrollArea {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

/// Returns the inner outline target for an [`OutlineViewScrollArea`].
#[must_use]
pub fn outline_view_scroll_area_outline(ui: &Ui, frame: ElementId) -> Option<ElementId> {
    reserved_scroll_frame_widget(ui, frame).and_then(|widget| widget.target())
}

/// Returns the reserved scrollbar for an [`OutlineViewScrollArea`].
#[must_use]
pub fn outline_view_scroll_area_scrollbar(ui: &Ui, frame: ElementId) -> Option<ElementId> {
    reserved_scroll_frame_widget(ui, frame).and_then(|widget| widget.scrollbar())
}

#[derive(Clone, Debug)]
struct OwnedOutlineModel {
    nodes: Vec<OutlineViewNode>,
    first_root: Option<OutlineNodeKey>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SiblingSet {
    position: usize,
    size: usize,
}

impl OwnedOutlineModel {
    fn root_set_len(&self) -> usize {
        self.sibling_keys(self.first_root).len()
    }

    fn sibling_set(&self, key: OutlineNodeKey) -> Option<SiblingSet> {
        self.sibling_set_from(self.first_root, key)
    }

    fn sibling_set_from(
        &self,
        first_key: Option<OutlineNodeKey>,
        target: OutlineNodeKey,
    ) -> Option<SiblingSet> {
        let siblings = self.sibling_keys(first_key);
        if let Some(index) = siblings.iter().position(|key| *key == target) {
            return Some(SiblingSet {
                position: index + 1,
                size: siblings.len(),
            });
        }

        siblings
            .into_iter()
            .find_map(|key| self.sibling_set_from(self.first_child_key(&key), target))
    }

    fn sibling_keys(&self, first_key: Option<OutlineNodeKey>) -> Vec<OutlineNodeKey> {
        let mut keys = Vec::new();
        let mut key = first_key.filter(|key| self.contains_key(key));
        while let Some(current) = key {
            keys.push(current);
            key = self
                .next_sibling_key(&current)
                .filter(|key| self.contains_key(key));
        }
        keys
    }
}

impl OutlineModel for OwnedOutlineModel {
    type Key = OutlineNodeKey;
    type Item = String;

    fn first_root_key(&self) -> Option<Self::Key> {
        self.first_root.filter(|key| self.contains_key(key))
    }

    fn contains_key(&self, key: &Self::Key) -> bool {
        key.index() < self.nodes.len()
    }

    fn next_sibling_key(&self, key: &Self::Key) -> Option<Self::Key> {
        self.nodes
            .get(key.index())
            .and_then(|node| node.next_sibling)
    }

    fn first_child_key(&self, key: &Self::Key) -> Option<Self::Key> {
        self.nodes
            .get(key.index())
            .and_then(|node| node.first_child)
    }

    fn item(&self, key: &Self::Key) -> Option<Self::Item> {
        self.nodes.get(key.index()).map(|node| node.label.clone())
    }
}

/// Per-element widget for `OutlineView`.
pub(crate) struct OutlineViewWidget {
    outline: Outline<OwnedOutlineModel>,
    selection: Selection<OutlineNodeKey>,
    selection_changed: OutlineSelectionChangedSlot,
    expansion_changed: OutlineExpansionChangedSlot,
    scroll_state: ScrollState,
    virtual_list: FixedVirtualizer,
    item_extent: f64,
    realized: Vec<RealizedOutlineItem>,
    last_viewport: Size,
    visible_len: usize,
}

impl core::fmt::Debug for OutlineViewWidget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OutlineViewWidget")
            .field("visible_len", &self.visible_len())
            .field("item_extent", &self.item_extent)
            .field("selected_key", &self.selected_key())
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
        node.set_size_of_set(self.outline.model().root_set_len());
        configure_vertical_scroll_accessibility(element, ctx, node, &self.scroll_state);
        if let Some(selected_key) = self.selected_key()
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

    fn realize_elements(&mut self, ctx: &mut ElementRealizeContext<'_>) -> ElementRealizeResult {
        let outline = ctx.id();
        ElementRealizeResult::from_changed(self.sync_virtual_items(outline, ctx.ui_mut()))
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
        if !event.is_target() {
            return;
        }
        if event.key.state != KeyState::Down
            || !ctx.is_enabled(outline)
            || self.update_visible_len() == 0
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
                let last = self.update_visible_len() - 1;
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
    fn replace_model(
        &mut self,
        outline: ElementId,
        ui: &mut Ui,
        nodes: Vec<OutlineViewNode>,
        first_root: Option<OutlineNodeKey>,
        expanded: impl IntoIterator<Item = OutlineNodeKey>,
        selected_key: Option<OutlineNodeKey>,
    ) {
        {
            let model = self.outline.model_mut();
            model.nodes = nodes;
            model.first_root = first_root;
        }
        let expanded = expanded
            .into_iter()
            .filter(|key| {
                self.outline.model().contains_key(key)
                    && self.outline.model().first_child_key(key).is_some()
            })
            .collect::<Vec<_>>();
        let _ = self.outline.replace_expanded(expanded);
        if let Some(key) = selected_key.filter(|key| self.outline.model().contains_key(key)) {
            let _ = self.selection.select_only(key);
        } else {
            let _ = self.selection.clear();
        }

        let visible_len = self.update_visible_len();
        self.virtual_list.set_len(visible_len);
        clamp_fixed_virtual_scroll_target(outline, ui, &mut self.virtual_list, &self.scroll_state);
        let _ = self.sync_virtual_items(outline, ui);
    }

    fn sync_virtual_items(&mut self, outline: ElementId, ui: &mut Ui) -> bool {
        let sync = self.sync_virtual_metrics(outline, ui);
        let mut changed = sync.changed;
        changed |= self.reconcile_realized_items(outline, ui, sync.range);
        changed |= self.layout_realized_items(ui, sync.viewport);
        self.sync_realized_selected_pseudos(ui);
        changed
    }

    fn sync_virtual_metrics(
        &mut self,
        outline: ElementId,
        ui: &mut Ui,
    ) -> crate::controls::support::FixedVirtualRowsSync {
        let visible_len = self.update_visible_len();
        sync_fixed_virtual_rows(
            outline,
            ui,
            &mut self.virtual_list,
            &self.scroll_state,
            visible_len,
            &mut self.last_viewport,
        )
    }

    fn reconcile_realized_items(
        &mut self,
        outline: ElementId,
        ui: &mut Ui,
        range: Range<usize>,
    ) -> bool {
        let rows = self.outline.visible_rows().to_vec();
        let model = self.outline.model().clone();
        let visible_len = self.visible_len();
        let mut changed = false;
        let mut kept = Vec::with_capacity(self.realized.len());
        for item in self.realized.drain(..) {
            let keep = range.contains(&item.index)
                && rows.get(item.index).is_some_and(|row| row.key == item.key)
                && ui.get(item.element).is_some();
            if keep {
                if let Some(row) = rows.get(item.index)
                    && let Some(label) = model.item(&row.key)
                {
                    let snapshot =
                        outline_row_snapshot(&model, visible_len, item.index, row, label);
                    update_realized_outline_item(ui, item.element, &snapshot);
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
            let Some(label) = model.item(&row.key) else {
                continue;
            };
            let snapshot = outline_row_snapshot(&model, visible_len, index, row, label);
            let element = append_realized_outline_item(ui, outline, &snapshot);
            self.realized.push(RealizedOutlineItem {
                index,
                key: snapshot.info.key,
                element,
            });
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
        let Some(delta) = fixed_virtual_accessibility_scroll_delta(
            request,
            direction,
            self.item_extent,
            self.virtual_list.viewport_extent(),
        ) else {
            return false;
        };
        self.scroll_by(outline, ui, delta);
        true
    }

    fn set_scroll_offset_y(&mut self, outline: ElementId, ui: &mut Ui, offset: f64) {
        set_fixed_virtual_scroll_offset_y(
            outline,
            ui,
            &mut self.virtual_list,
            &self.scroll_state,
            offset,
        );
        let _ = self.sync_virtual_items(outline, ui);
    }

    fn visible_len(&self) -> usize {
        self.visible_len
    }

    fn update_visible_len(&mut self) -> usize {
        self.visible_len = self.outline.visible_len();
        self.visible_len
    }

    fn selected_key(&self) -> Option<OutlineNodeKey> {
        self.selection.primary().copied()
    }

    fn selected_visible_index(&mut self) -> Option<usize> {
        self.selected_key()
            .and_then(|key| self.outline.index_of_key(&key))
    }

    fn select_visible_index(&mut self, outline: ElementId, ui: &mut Ui, index: usize) -> bool {
        let Some(row) = self.outline.visible_row(index).cloned() else {
            return false;
        };
        let changed = self.select_key(outline, ui, row.key);
        self.scroll_index_into_view(outline, ui, index);
        changed
    }

    fn select_key(&mut self, outline: ElementId, ui: &mut Ui, key: OutlineNodeKey) -> bool {
        if self.outline.model().contains_key(&key) {
            let changed =
                self.mutate_selection(ui, outline, |selection| selection.select_only(key));
            if let Some(index) = self.outline.index_of_key(&key) {
                self.scroll_index_into_view(outline, ui, index);
            } else {
                self.sync_realized_selected_pseudos(ui);
            }
            return changed;
        }
        false
    }

    fn move_selection_by(&mut self, outline: ElementId, ui: &mut Ui, delta: isize) -> bool {
        let len = self.update_visible_len();
        if len == 0 {
            return false;
        }
        let current = self.selected_visible_index().unwrap_or(0);
        let next = current.saturating_add_signed(delta).min(len - 1);
        self.select_visible_index(outline, ui, next)
    }

    fn scroll_index_into_view(&mut self, outline: ElementId, ui: &mut Ui, index: usize) {
        let visible_len = self.visible_len();
        if scroll_fixed_virtual_index_into_view(
            outline,
            ui,
            &mut self.virtual_list,
            &self.scroll_state,
            index,
            visible_len,
        ) {
            let _ = self.sync_virtual_items(outline, ui);
        }
    }

    fn toggle_selected(&mut self, outline: ElementId, ui: &mut Ui) -> bool {
        let Some(key) = self.selected_key() else {
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
        let last = self.update_visible_len() - 1;
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

    fn toggle_key_expanded(
        &mut self,
        outline: ElementId,
        ui: &mut Ui,
        key: OutlineNodeKey,
    ) -> bool {
        let expanded = self.outline.is_expanded(&key);
        self.set_key_expanded(outline, ui, key, !expanded)
    }

    fn set_key_expanded(
        &mut self,
        outline: ElementId,
        ui: &mut Ui,
        key: OutlineNodeKey,
        expanded: bool,
    ) -> bool {
        if self.outline.model().first_child_key(&key).is_none() {
            return false;
        }
        if !self.outline.set_expanded(key, expanded) {
            return false;
        }

        if self
            .selected_key()
            .is_none_or(|selected| self.outline.index_of_key(&selected).is_none())
        {
            let _ = self.mutate_selection(ui, outline, |selection| selection.select_only(key));
        }
        let visible_len = self.update_visible_len();
        self.virtual_list.set_len(visible_len);
        clamp_fixed_virtual_scroll_target(outline, ui, &mut self.virtual_list, &self.scroll_state);
        let _ = self.sync_virtual_items(outline, ui);
        self.expansion_changed.dispatch(
            ui,
            OutlineExpansionChangedEvent {
                outline,
                key,
                expanded,
            },
        );
        true
    }

    fn mutate_selection(
        &mut self,
        ui: &mut Ui,
        outline: ElementId,
        mutate: impl FnOnce(&mut Selection<OutlineNodeKey>) -> bool,
    ) -> bool {
        let changed = mutate(&mut self.selection);
        self.sync_realized_selected_pseudos(ui);
        if changed {
            self.selection_changed.dispatch(
                ui,
                OutlineSelectionChangedEvent {
                    outline,
                    selected_key: self.selected_key(),
                },
            );
        }
        changed
    }

    fn sync_realized_selected_pseudos(&self, ui: &mut Ui) {
        for item in &self.realized {
            if self.selection.contains(&item.key) {
                ui.add_pseudo(item.element, PSEUDO_SELECTED);
            } else {
                ui.remove_pseudo(item.element, PSEUDO_SELECTED);
            }
        }
    }

    fn visible_item_count(&self) -> usize {
        fixed_visible_item_count(self.item_extent, self.virtual_list.viewport_extent())
    }
}

/// Per-element widget for a realized `OutlineView` item.
#[derive(Debug)]
pub(crate) struct OutlineItemWidget {
    info: OutlineItemInfo,
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
        node.set_level(self.info.depth + 1);
        node.set_position_in_set(self.info.position_in_set);
        node.set_size_of_set(self.info.size_of_set);
        if self.info.has_children {
            node.set_expanded(self.info.is_expanded);
            if self.info.is_expanded {
                node.add_action(accesskit::Action::Collapse);
            } else {
                node.add_action(accesskit::Action::Expand);
            }
        }
    }

    fn template_contract(&self) -> TemplateContract {
        TemplateContract::new()
            .with_required_slot(SLOT_OUTLINE_ITEM_LABEL)
            .with_recommended_slot(SLOT_OUTLINE_ITEM_DISCLOSURE)
    }

    fn handle_click(&mut self, ctx: &mut WidgetEventContext<'_>, event: ClickEvent<'_>) {
        let item = ctx.id();
        if !event.is_semantic_target() || !ctx.is_enabled(item) {
            return;
        }
        let Some(outline) = outline_parent(ctx, item) else {
            return;
        };
        let _ = select_outline_key(ctx, outline, self.info.key);
        if self.info.has_children && click_source_is_disclosure(ctx, item, event.source) {
            let _ = toggle_outline_key(ctx, outline, self.info.key);
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
            accesskit::Action::Expand => {
                set_outline_key_expanded(ctx, outline, self.info.key, true)
            }
            accesskit::Action::Collapse => {
                set_outline_key_expanded(ctx, outline, self.info.key, false)
            }
            _ => false,
        }
    }
}

fn click_source_is_disclosure(
    ctx: &WidgetEventContext<'_>,
    item: ElementId,
    source: ElementId,
) -> bool {
    let Some(disclosure) = ctx.slot(item, SLOT_OUTLINE_ITEM_DISCLOSURE) else {
        return false;
    };
    is_descendant_or_self(ctx, source, disclosure)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RealizedOutlineItem {
    index: usize,
    key: OutlineNodeKey,
    element: ElementId,
}

struct OutlineRowSnapshot {
    info: OutlineItemInfo,
    label: String,
}

fn outline_row_snapshot(
    model: &OwnedOutlineModel,
    visible_len: usize,
    index: usize,
    row: &VisibleRow<OutlineNodeKey>,
    label: String,
) -> OutlineRowSnapshot {
    let sibling_set = model.sibling_set(row.key).unwrap_or(SiblingSet {
        position: index + 1,
        size: visible_len,
    });
    OutlineRowSnapshot::new(index, row, label, sibling_set)
}

impl OutlineRowSnapshot {
    fn new(
        index: usize,
        row: &VisibleRow<OutlineNodeKey>,
        label: String,
        sibling_set: SiblingSet,
    ) -> Self {
        Self {
            info: OutlineItemInfo {
                index,
                key: row.key,
                depth: row.depth,
                position_in_set: sibling_set.position,
                size_of_set: sibling_set.size,
                has_children: row.has_children,
                is_expanded: row.is_expanded,
            },
            label,
        }
    }
}

static OUTLINE_ITEM_TEMPLATE: TemplateSpecRecipe =
    TemplateSpecRecipe::new(outline_item_template_spec);

pub(crate) fn default_outline_item_template() -> crate::ControlTemplate {
    crate::ControlTemplate::from_static(&OUTLINE_ITEM_TEMPLATE)
}

fn outline_item_template_spec() -> TemplateSpec {
    TemplateSpec::from_nodes([TemplateNode::surface_part(PART_OUTLINE_ITEM_CONTENT)
        .with_child(TemplateNode::text_slot(SLOT_OUTLINE_ITEM_DISCLOSURE))
        .with_child(TemplateNode::content_presenter(SLOT_OUTLINE_ITEM_LABEL))])
}

fn append_realized_outline_item(
    ui: &mut Ui,
    outline: ElementId,
    item: &OutlineRowSnapshot,
) -> ElementId {
    let id = ui.append_widget(outline, Box::new(OutlineItemWidget { info: item.info }));
    update_realized_outline_item(ui, id, item);
    ui.set_control_state(id, ControlState::interactive().focusable(false));
    id
}

fn update_realized_outline_item(ui: &mut Ui, id: ElementId, item: &OutlineRowSnapshot) {
    ui.with_widget_mut(id, |widget, _ui| {
        if let Some(widget) = widget.downcast_mut::<OutlineItemWidget>() {
            widget.info = item.info;
        }
    });
    ui.set_local(id, ui.properties().content, item.label.clone());
    if let Some(disclosure) = ui.slot(id, SLOT_OUTLINE_ITEM_DISCLOSURE) {
        ui.set_local(
            disclosure,
            ui.properties().content,
            disclosure_text(
                item.info.depth,
                item.info.has_children,
                item.info.is_expanded,
            ),
        );
        ui.set_local(
            disclosure,
            ui.properties().layout_size,
            LayoutSize::new()
                .width(LayoutDimension::Logical(disclosure_width(item.info.depth)))
                .height(LayoutDimension::Percent(1.0)),
        );
    }
}

/// Selects `key` in `outline`.
///
/// Returns `false` if `outline` is stale, disabled, not an `OutlineView`, or
/// if `key` is outside the model.
pub fn select_outline_key(ui: &mut Ui, outline: ElementId, key: impl Into<OutlineNodeKey>) -> bool {
    let key = key.into();
    with_outline_view_mut(ui, outline, |widget, ui| {
        widget.select_key(outline, ui, key)
    })
    .unwrap_or(false)
}

/// Returns the currently selected node key in `outline`, if any.
#[must_use]
pub fn selected_outline_key(ui: &Ui, outline: ElementId) -> Option<OutlineNodeKey> {
    outline_view_widget(ui, outline).and_then(OutlineViewWidget::selected_key)
}

/// Returns whether `key` is currently expanded in `outline`.
#[must_use]
pub fn outline_key_expanded(
    ui: &Ui,
    outline: ElementId,
    key: impl Into<OutlineNodeKey>,
) -> Option<bool> {
    let key = key.into();
    outline_view_widget(ui, outline).map(|widget| widget.outline.is_expanded(&key))
}

/// Returns the currently expanded node keys in `outline`.
#[must_use]
pub fn expanded_outline_keys(ui: &Ui, outline: ElementId) -> Option<Vec<OutlineNodeKey>> {
    outline_view_widget(ui, outline)
        .map(|widget| widget.outline.expansion().iter().copied().collect())
}

/// Sets whether `key` is expanded in `outline`.
///
/// Returns `false` if `outline` is stale, disabled, not an `OutlineView`, if
/// `key` does not have children, or if the state was already correct.
pub fn set_outline_key_expanded(
    ui: &mut Ui,
    outline: ElementId,
    key: impl Into<OutlineNodeKey>,
    expanded: bool,
) -> bool {
    let key = key.into();
    with_outline_view_mut(ui, outline, |widget, ui| {
        widget.set_key_expanded(outline, ui, key, expanded)
    })
    .unwrap_or(false)
}

/// Replaces `outline`'s node model without replacing the outline element.
///
/// Expansion, selection, and scroll policy remain caller-owned. Supply
/// `expanded` and `selected_key` after mapping any previously retained state
/// into the new model's [`OutlineNodeKey`] values.
///
/// Returns `false` if `outline` is stale or does not own outline state.
pub fn replace_outline_nodes(
    ui: &mut Ui,
    outline: ElementId,
    nodes: Vec<OutlineViewNode>,
    first_root: Option<OutlineNodeKey>,
    expanded: impl IntoIterator<Item = OutlineNodeKey>,
    selected_key: Option<OutlineNodeKey>,
) -> bool {
    with_outline_view_mut(ui, outline, |widget, ui| {
        widget.replace_model(outline, ui, nodes, first_root, expanded, selected_key);
        true
    })
    .unwrap_or(false)
}

/// Toggles expansion for `key` in `outline`.
pub fn toggle_outline_key(ui: &mut Ui, outline: ElementId, key: impl Into<OutlineNodeKey>) -> bool {
    let key = key.into();
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
pub fn realized_outline_item_key(ui: &Ui, item: ElementId) -> Option<OutlineNodeKey> {
    realized_outline_item_info(ui, item).map(|info| info.key)
}

/// Returns semantic row metadata for a realized outline item.
#[must_use]
pub fn realized_outline_item_info(ui: &Ui, item: ElementId) -> Option<OutlineItemInfo> {
    ui.get(item)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<OutlineItemWidget>())
        .map(|widget| widget.info)
}

#[cfg(test)]
pub(crate) fn realized_outline_item(
    ui: &Ui,
    outline: ElementId,
    key: impl Into<OutlineNodeKey>,
) -> Option<ElementId> {
    let key = key.into();
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

fn disclosure_width(depth: usize) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "outline depth is UI-local and intentionally maps to logical pixels"
    )]
    {
        OUTLINE_DISCLOSURE_WIDTH + ((depth as f64) * OUTLINE_INDENT_PER_DEPTH)
    }
}

fn normalize_item_extent(extent: f64) -> f64 {
    if extent.is_finite() {
        extent.max(1.0)
    } else {
        DEFAULT_OUTLINE_ITEM_EXTENT
    }
}

#[cfg(test)]
mod tests {
    use alloc::rc::Rc;
    use alloc::string::String;
    use alloc::vec;
    use core::cell::RefCell;

    use accesskit::{Node, Role, TreeUpdate};
    use kurbo::Point;
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};
    use understory_property::DependencyObjectExt;

    use super::*;
    use crate::ElementSpecExt;
    use crate::accessibility::accesskit_node_id;
    use crate::controls::scroll::{SLOT_SCROLL_BAR_THUMB, ScrollBar};
    use crate::pseudos::PSEUDO_SELECTED;
    use crate::test_support::{pointer_down, pointer_move, pointer_up};

    fn node(label: &str) -> OutlineViewNode {
        OutlineViewNode::new(String::from(label))
    }

    fn key(index: usize) -> OutlineNodeKey {
        OutlineNodeKey::new(index)
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
    fn clicking_disclosure_toggles_expansion_and_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let outline = ui.append(root, test_outline());
        compute_layout(&mut ui);

        let parent = realized_outline_item(&ui, outline, 0).unwrap();
        let disclosure = ui
            .slot(parent, SLOT_OUTLINE_ITEM_DISCLOSURE)
            .expect("parent should expose disclosure slot");
        assert!(realized_outline_item(&ui, outline, 1).is_none());

        ui.synthesize_click(disclosure, Point::ZERO);
        compute_layout(&mut ui);

        assert_eq!(selected_outline_key(&ui, outline), Some(key(0)));
        assert_eq!(outline_key_expanded(&ui, outline, 0), Some(true));
        assert!(realized_outline_item(&ui, outline, 1).is_some());
        assert!(ui.get(parent).unwrap().pseudos().contains(&PSEUDO_SELECTED));

        let parent = realized_outline_item(&ui, outline, 0).unwrap();
        let disclosure = ui
            .slot(parent, SLOT_OUTLINE_ITEM_DISCLOSURE)
            .expect("parent should expose disclosure slot");
        ui.synthesize_click(disclosure, Point::ZERO);
        compute_layout(&mut ui);

        assert_eq!(outline_key_expanded(&ui, outline, 0), Some(false));
        assert!(realized_outline_item(&ui, outline, 1).is_none());
    }

    #[test]
    fn clicking_outline_label_selects_without_toggling_expansion() {
        let mut ui = Ui::new();
        let root = ui.root();
        let outline = ui.append(root, test_outline());
        compute_layout(&mut ui);

        let parent = realized_outline_item(&ui, outline, 0).unwrap();
        let label = ui
            .slot(parent, SLOT_OUTLINE_ITEM_LABEL)
            .expect("parent should expose label slot");

        ui.synthesize_click(label, Point::ZERO);
        compute_layout(&mut ui);

        assert_eq!(selected_outline_key(&ui, outline), Some(key(0)));
        assert_eq!(outline_key_expanded(&ui, outline, 0), Some(false));
        assert!(realized_outline_item(&ui, outline, 1).is_none());
    }

    #[test]
    fn clicking_outline_row_background_selects_without_toggling_expansion() {
        let mut ui = Ui::new();
        let root = ui.root();
        let outline = ui.append(root, test_outline());
        compute_layout(&mut ui);

        let parent = realized_outline_item(&ui, outline, 0).unwrap();

        ui.synthesize_click(parent, Point::ZERO);
        compute_layout(&mut ui);

        assert_eq!(selected_outline_key(&ui, outline), Some(key(0)));
        assert_eq!(outline_key_expanded(&ui, outline, 0), Some(false));
        assert!(realized_outline_item(&ui, outline, 1).is_none());
    }

    #[test]
    fn keyboard_expands_collapses_and_moves_outline_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let outline = ui.append(root, test_outline().selected_key(0));
        compute_layout(&mut ui);
        ui.set_focus(Some(outline));

        let right = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
        assert!(ui.dispatch_key(&right).handled);
        compute_layout(&mut ui);
        assert_eq!(outline_key_expanded(&ui, outline, 0), Some(true));
        assert!(realized_outline_item(&ui, outline, 1).is_some());

        assert!(ui.dispatch_key(&right).handled);
        compute_layout(&mut ui);
        assert_eq!(selected_outline_key(&ui, outline), Some(key(1)));

        let up = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowUp), Code::ArrowUp);
        assert!(ui.dispatch_key(&up).handled);
        assert_eq!(selected_outline_key(&ui, outline), Some(key(0)));

        let left = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowLeft), Code::ArrowLeft);
        assert!(ui.dispatch_key(&left).handled);
        compute_layout(&mut ui);
        assert_eq!(outline_key_expanded(&ui, outline, 0), Some(false));
        assert!(realized_outline_item(&ui, outline, 1).is_none());
    }

    #[test]
    fn outline_selection_and_expansion_handlers_report_stable_keys() {
        let selected = Rc::new(RefCell::new(Vec::new()));
        let expanded = Rc::new(RefCell::new(Vec::new()));
        let selected_for_handler = Rc::clone(&selected);
        let expanded_for_handler = Rc::clone(&expanded);

        let mut ui = Ui::new();
        let root = ui.root();
        let outline = ui.append(
            root,
            test_outline()
                .on_selection_changed(move |_ui, event| {
                    selected_for_handler
                        .borrow_mut()
                        .push((event.outline, event.selected_key));
                })
                .on_expansion_changed(move |_ui, event| {
                    expanded_for_handler.borrow_mut().push((
                        event.outline,
                        event.key,
                        event.expanded,
                    ));
                }),
        );
        compute_layout(&mut ui);

        assert!(set_outline_key_expanded(&mut ui, outline, 0, true));
        assert!(select_outline_key(&mut ui, outline, 2));

        assert_eq!(
            &*selected.borrow(),
            &[(outline, Some(key(0))), (outline, Some(key(2)))]
        );
        assert_eq!(&*expanded.borrow(), &[(outline, key(0), true)]);
    }

    #[test]
    fn realized_outline_item_info_reports_row_projection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let outline = ui.append(root, test_outline().expanded(0).selected_key(1));
        compute_layout(&mut ui);

        let child = realized_outline_item(&ui, outline, 1).unwrap();

        assert_eq!(
            realized_outline_item_info(&ui, child),
            Some(OutlineItemInfo {
                index: 1,
                key: key(1),
                depth: 1,
                position_in_set: 1,
                size_of_set: 2,
                has_children: false,
                is_expanded: false,
            })
        );
        assert_eq!(realized_outline_item_key(&ui, child), Some(key(1)));
    }

    #[test]
    fn replace_outline_nodes_preserves_outline_state_and_realized_items() {
        let mut ui = Ui::new();
        let root = ui.root();
        let outline = ui.append(
            root,
            OutlineView::new()
                .nodes(scrolling_nodes(100), Some(0))
                .expanded(0)
                .selected_key(20)
                .size(140.0, 56.0)
                .item_extent(20.0)
                .overscan(0.0),
        );
        compute_layout(&mut ui);
        assert!(set_outline_scroll_offset_y(&mut ui, outline, 380.0));
        let old_outline = outline;

        let mut replacement = scrolling_nodes(120);
        replacement[20] = node("Updated item 20").with_next_sibling(Some(21));
        assert!(replace_outline_nodes(
            &mut ui,
            outline,
            replacement,
            Some(key(0)),
            [key(0)],
            Some(key(20)),
        ));
        compute_layout(&mut ui);

        assert!(ui.get(old_outline).is_some());
        assert_eq!(expanded_outline_keys(&ui, outline), Some(vec![key(0)]));
        assert_eq!(selected_outline_key(&ui, outline), Some(key(20)));
        assert_eq!(outline_scroll_offset_y(&ui, outline), Some(380.0));
        let realized = realized_outline_item(&ui, outline, 20)
            .expect("updated selected row should still be realized");
        assert_eq!(
            ui.get(realized)
                .and_then(|element| element.get_local_value(ui.properties().content))
                .map(String::as_str),
            Some("Updated item 20"),
        );
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

        compute_layout(&mut ui);
        assert!(realized_outline_item(&ui, outline, 1).is_some());
        let bar_bounds = ui.effective_bounds(bar);
        let target = Point::new(bar_bounds.center().x, bar_bounds.y1 - 1.0);

        assert!(
            ui.dispatch_pointer_event(&pointer_down(target, 1))
                .changed()
        );
        assert!(ui.dispatch_pointer_event(&pointer_up(target, 2)).handled);
        compute_layout(&mut ui);

        assert!(outline_scroll_offset_y(&ui, frame).is_some_and(|offset| offset > 0.0));
        assert!(realized_outline_item(&ui, frame, 1).is_none());
    }

    #[test]
    fn outline_scroll_area_children_follow_arranged_height() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(0.0).logical_size(180.0, 120.0));
        let frame = ui.append(
            root,
            OutlineView::new()
                .nodes(scrolling_nodes(100), Some(0))
                .expanded(0)
                .size(140.0, 56.0)
                .item_extent(20.0)
                .overscan(0.0)
                .with_vertical_scrollbar()
                .thumb_extent(12.0)
                .layout(Layout::new().logical_size(158.0, 56.0).fill_height()),
        );
        let outline = outline_view_scroll_area_outline(&ui, frame)
            .expect("scroll frame should expose outline");
        let bar = outline_view_scroll_area_scrollbar(&ui, frame)
            .expect("scroll frame should expose scrollbar");

        compute_layout(&mut ui);
        let frame_bounds = ui.effective_bounds(frame);
        let outline_bounds = ui.effective_bounds(outline);
        let bar_bounds = ui.effective_bounds(bar);

        assert_eq!(frame_bounds.height(), 120.0);
        assert_eq!(outline_bounds.height(), 120.0);
        assert_eq!(bar_bounds.height(), 120.0);
        assert_eq!(outline_bounds.width(), 140.0);
        assert_eq!(bar_bounds.width(), 10.0);
        assert!(realized_outline_item(&ui, outline, 1).is_some());

        let target = Point::new(bar_bounds.center().x, bar_bounds.y1 - 1.0);
        assert!(
            ui.dispatch_pointer_event(&pointer_down(target, 1))
                .changed()
        );
        assert!(ui.dispatch_pointer_event(&pointer_up(target, 2)).handled);
        compute_layout(&mut ui);

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

        compute_layout(&mut ui);
        let start = ui.effective_bounds(thumb).center();
        assert!(ui.dispatch_pointer_event(&pointer_down(start, 1)).changed());
        assert!(
            ui.dispatch_pointer_event(&pointer_move(Point::new(start.x, start.y + 30.0), 2, true))
                .changed()
        );
        compute_layout(&mut ui);

        assert!(outline_scroll_offset_y(&ui, outline).is_some_and(|offset| offset > 0.0));
        assert!(realized_outline_item(&ui, outline, 1).is_none());
    }

    #[test]
    fn outline_accessibility_exposes_tree_items_and_expansion() {
        let mut ui = Ui::new();
        let root = ui.root();
        let outline = ui.append(root, test_outline().expanded(0).selected_key(1));
        compute_layout(&mut ui);
        let child = realized_outline_item(&ui, outline, 1).unwrap();
        let update = ui.accesskit_tree_update();

        let outline_node = accesskit_node_for(&update, outline);
        assert_eq!(outline_node.role(), Role::Tree);
        assert_eq!(outline_node.size_of_set(), Some(1));

        let child_node = accesskit_node_for(&update, child);
        assert_eq!(child_node.role(), Role::TreeItem);
        assert_eq!(child_node.level(), Some(2));
        assert_eq!(child_node.position_in_set(), Some(1));
        assert_eq!(child_node.size_of_set(), Some(2));
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
}
