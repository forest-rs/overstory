// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![no_std]

//! Snapshot adapters and model helpers for Overstory devtools.
//!
//! ```text
//! Ui -> UiInspectionSnapshot -> DevtoolsSnapshot -> DevtoolsSession -> inspector UI / transport
//! ```
//!
//! This crate owns the inspection adapter layer. It explicitly does not own
//! live UI mutation, rendering, host integration, or time-series telemetry.

extern crate alloc;

use core::cell::Cell;

use alloc::format;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
#[cfg(test)]
use alloc::vec;
use alloc::vec::Vec;

use overstory::inspection::{
    CompositionInspectionSnapshot, CompositionLayerInspectionRow, ElementInspectionRow,
    ElementPropertyInspectionGroup, ElementPropertyInspectionValue, ElementTreeInspectionPatch,
    LayerInspectionId, SurfaceInspectionId, TemplateContractInspection, TemplateSlotInspectionRow,
    UiInspectionChanges, UiInspectionSnapshot, UiStorageInspectionSnapshot,
};
use overstory::{
    ElementId, ElementSpecExt, Layout, OutlineNodeKey, OutlineView, OutlineViewNode, Panel,
    ScrollArea, SurfaceSpecExt, TemplateDiagnostic, TemplateItemKey, Text, Ui,
    scroll_area_viewport, theme,
};
#[cfg(feature = "spoor-session")]
use overstory::{expanded_outline_keys, replace_outline_nodes};

#[cfg(feature = "spoor-session")]
mod client;
#[cfg(feature = "spoor-session")]
mod live;
mod property_grid;
#[cfg(feature = "spoor-session")]
mod protocol;
mod session;

#[cfg(feature = "spoor-session")]
pub use client::{InspectionClient, InspectionMirror};
#[cfg(feature = "spoor-session")]
pub use live::{DevtoolsRefresh, DevtoolsRefreshChanges, DevtoolsTreePatchCounts, LiveDevtools};
pub use property_grid::{
    PropertyGrid, PropertyGridColor, PropertyGridElementLink, PropertyGridOrigin, PropertyGridRow,
    PropertyGridSection, PropertyGridToken, PropertyGridValue,
};
#[cfg(feature = "spoor-session")]
pub use protocol::{
    INSPECTION_DOMAIN, InspectionDelta, InspectionDispatch, InspectionElementDetails,
    InspectionElementNode, InspectionElementNodeUpdate, InspectionElementTree,
    InspectionElementTreePatch, InspectionRequest, InspectionResponse, InspectionStreamItem,
    InspectionSubscription, InspectionSubscriptionKind, InspectionSubsystem, InspectionSummaries,
    InspectionUpdateReason, LiveInspectionRequest, LiveInspectionResponse,
    LiveInspectionStreamItem, LiveInspectionStreamStart, inspection_method,
    inspection_stream_start,
};
#[cfg(feature = "spoor-snapshot-session")]
pub use session::DevtoolsSpoorSummary;
pub use session::{DevtoolsEvent, DevtoolsSession, DevtoolsSessionHandle};

use property_grid::{DetailActions, append_property_grid_sections};

/// A source that can produce an owned Overstory inspection snapshot.
///
/// Implement this for host-side targets that want to expose a snapshot without
/// exposing their live UI internals.
pub trait DevtoolsTarget {
    /// Captures the target's current inspectable state.
    fn capture_inspection_snapshot(&self) -> UiInspectionSnapshot;
}

/// A live target that can report inspection changes since a baseline capture.
///
/// A live devtools session should capture a snapshot once, clear changes
/// created before that baseline, then drain changes for steady-state updates.
pub trait LiveDevtoolsTarget: DevtoolsTarget {
    /// Starts target-side live inspection recording.
    fn begin_live_inspection_recording(&mut self);

    /// Stops one target-side live inspection recording attachment.
    fn end_live_inspection_recording(&mut self);

    /// Drains inspectable state changes recorded since the previous drain.
    fn drain_inspection_changes(&mut self) -> UiInspectionChanges;

    /// Clears pending inspection changes without returning them.
    fn clear_inspection_changes(&mut self);
}

impl DevtoolsTarget for Ui {
    fn capture_inspection_snapshot(&self) -> UiInspectionSnapshot {
        Self::inspection_snapshot(self)
    }
}

impl LiveDevtoolsTarget for Ui {
    fn begin_live_inspection_recording(&mut self) {
        Self::begin_live_inspection_recording(self);
    }

    fn end_live_inspection_recording(&mut self) {
        Self::end_live_inspection_recording(self);
    }

    fn drain_inspection_changes(&mut self) -> UiInspectionChanges {
        Self::drain_inspection_changes(self)
    }

    fn clear_inspection_changes(&mut self) {
        Self::clear_inspection_changes(self);
    }
}

/// Filtered target view over a live [`Ui`].
///
/// Use this for in-process devtools: the target can be the running app root
/// while the devtools pane or popup is excluded from the captured snapshot.
#[derive(Clone, Copy, Debug)]
pub struct UiInspectionTarget<'a> {
    ui: &'a Ui,
    root: ElementId,
    excluded_roots: &'a [ElementId],
}

impl<'a> UiInspectionTarget<'a> {
    /// Creates a target for the whole `ui`.
    #[must_use]
    pub fn new(ui: &'a Ui) -> Self {
        Self {
            ui,
            root: ui.root(),
            excluded_roots: &[],
        }
    }

    /// Returns a copy that captures only the subtree rooted at `root`.
    #[must_use]
    pub const fn rooted_at(mut self, root: ElementId) -> Self {
        self.root = root;
        self
    }

    /// Returns a copy that omits each subtree in `excluded_roots`.
    #[must_use]
    pub const fn excluding(mut self, excluded_roots: &'a [ElementId]) -> Self {
        self.excluded_roots = excluded_roots;
        self
    }
}

impl DevtoolsTarget for UiInspectionTarget<'_> {
    fn capture_inspection_snapshot(&self) -> UiInspectionSnapshot {
        filter_inspection_snapshot(
            self.ui.inspection_snapshot(),
            self.root,
            self.excluded_roots,
        )
    }
}

/// Filtered mutable target view over a live [`Ui`].
///
/// This is the live-update counterpart to [`UiInspectionTarget`]. It is used by
/// same-process devtools to drain target-side changes while excluding the
/// inspector's own retained subtree.
#[derive(Debug)]
pub struct UiLiveInspectionTarget<'a> {
    ui: &'a mut Ui,
    root: ElementId,
    excluded_roots: &'a [ElementId],
}

impl<'a> UiLiveInspectionTarget<'a> {
    /// Creates a live target for the whole `ui`.
    #[must_use]
    pub fn new(ui: &'a mut Ui) -> Self {
        let root = ui.root();
        Self {
            ui,
            root,
            excluded_roots: &[],
        }
    }

    /// Returns a target that captures and drains only the subtree rooted at `root`.
    #[must_use]
    pub const fn rooted_at(mut self, root: ElementId) -> Self {
        self.root = root;
        self
    }

    /// Returns a target that omits each subtree in `excluded_roots`.
    #[must_use]
    pub const fn excluding(mut self, excluded_roots: &'a [ElementId]) -> Self {
        self.excluded_roots = excluded_roots;
        self
    }

    fn element_is_included(&self, element: ElementId) -> bool {
        element_is_descendant_or_self(self.ui, self.root, element)
            && !self
                .excluded_roots
                .iter()
                .any(|excluded| element_is_descendant_or_self(self.ui, *excluded, element))
    }
}

impl DevtoolsTarget for UiLiveInspectionTarget<'_> {
    fn capture_inspection_snapshot(&self) -> UiInspectionSnapshot {
        filter_inspection_snapshot(
            self.ui.inspection_snapshot(),
            self.root,
            self.excluded_roots,
        )
    }
}

impl LiveDevtoolsTarget for UiLiveInspectionTarget<'_> {
    fn begin_live_inspection_recording(&mut self) {
        self.ui.begin_live_inspection_recording();
    }

    fn end_live_inspection_recording(&mut self) {
        self.ui.end_live_inspection_recording();
    }

    fn drain_inspection_changes(&mut self) -> UiInspectionChanges {
        let changes = self.ui.drain_inspection_changes();
        filter_inspection_changes(changes, self)
    }

    fn clear_inspection_changes(&mut self) {
        self.ui.clear_inspection_changes();
    }
}

fn filter_inspection_changes(
    mut changes: UiInspectionChanges,
    target: &UiLiveInspectionTarget<'_>,
) -> UiInspectionChanges {
    let mut removed_decisions = Vec::new();
    let mut removed = Vec::new();
    for removal in changes.tree.removed {
        let included = if contains_element(target.excluded_roots, removal.id) {
            false
        } else if let Some(parent) = removal.parent {
            removed_decisions
                .iter()
                .find(|(id, _)| *id == parent)
                .map_or_else(
                    || {
                        target
                            .ui
                            .get(parent)
                            .is_none_or(|_| target.element_is_included(parent))
                    },
                    |(_, included)| *included,
                )
        } else {
            removal.id == target.root
        };
        removed_decisions.push((removal.id, included));
        if included {
            removed.push(removal);
        }
    }

    let upserted = changes
        .tree
        .upserted
        .into_iter()
        .filter(|row| target.element_is_included(row.id))
        .map(|row| filter_inspection_row(row, target))
        .collect();
    let updated = changes
        .tree
        .updated
        .into_iter()
        .filter(|row| target.element_is_included(row.id))
        .map(|row| filter_inspection_row(row, target))
        .collect();

    changes.tree.root = Some(target.root);
    changes.tree.removed = removed;
    changes.tree.upserted = upserted;
    changes.tree.updated = updated;
    changes
}

fn filter_inspection_row(
    mut row: ElementInspectionRow,
    target: &UiLiveInspectionTarget<'_>,
) -> ElementInspectionRow {
    if row.id == target.root {
        row.parent = None;
    } else {
        row.parent = row
            .parent
            .filter(|parent| target.element_is_included(*parent));
    }
    row.children
        .retain(|child| target.element_is_included(*child));
    sanitize_template_links(&mut row, |element| target.element_is_included(element));
    row
}

/// Owned devtools snapshot plus lightweight lookup tables.
#[derive(Clone, Debug)]
pub struct DevtoolsSnapshot {
    inspection: UiInspectionSnapshot,
    element_index: Vec<(ElementId, usize)>,
    layer_index: Vec<(LayerInspectionId, usize)>,
    surface_index: Vec<(SurfaceInspectionId, usize)>,
}

impl DevtoolsSnapshot {
    /// Captures a snapshot from `target`.
    #[must_use]
    pub fn capture(target: &impl DevtoolsTarget) -> Self {
        Self::new(target.capture_inspection_snapshot())
    }

    /// Wraps an existing inspection snapshot with lookup helpers.
    #[must_use]
    pub fn new(inspection: UiInspectionSnapshot) -> Self {
        let mut snapshot = Self {
            inspection,
            element_index: Vec::new(),
            layer_index: Vec::new(),
            surface_index: Vec::new(),
        };
        snapshot.rebuild_indexes();
        snapshot
    }

    /// Returns the wrapped inspection snapshot.
    #[must_use]
    pub const fn inspection(&self) -> &UiInspectionSnapshot {
        &self.inspection
    }

    /// Applies target-side inspection changes to the retained snapshot.
    ///
    /// This is the session-side half of live devtools. Full captures remain
    /// the resync path; this method keeps the retained baseline current between
    /// captures when the target can provide explicit change records.
    pub fn apply_inspection_changes(&mut self, changes: UiInspectionChanges) {
        if let Some(frame) = changes.frame {
            self.inspection.frame = frame;
        }
        self.apply_element_tree_patch(changes.tree);
    }

    fn apply_element_tree_patch(&mut self, patch: ElementTreeInspectionPatch) {
        if let Some(root) = patch.root {
            self.inspection.root = root;
        }

        let mut removed_ids = Vec::new();
        for removal in &patch.removed {
            self.push_snapshot_subtree(removal.id, &mut removed_ids);
        }
        removed_ids.sort();
        removed_ids.dedup();
        self.inspection
            .elements
            .retain(|row| removed_ids.binary_search(&row.id).is_err());

        for row in patch.upserted.into_iter().chain(patch.updated) {
            if let Some(existing) = self
                .inspection
                .elements
                .iter_mut()
                .find(|existing| existing.id == row.id)
            {
                *existing = row;
            } else {
                self.inspection.elements.push(row);
            }
        }

        let live_elements: Vec<_> = self.inspection.elements.iter().map(|row| row.id).collect();
        for row in &mut self.inspection.elements {
            if row.id == self.inspection.root {
                row.parent = None;
            } else {
                row.parent = row
                    .parent
                    .filter(|id| contains_element(&live_elements, *id));
            }
            row.children
                .retain(|child| contains_element(&live_elements, *child));
            sanitize_template_links(row, |element| contains_element(&live_elements, element));
        }

        self.inspection.elements = ordered_element_rows(
            self.inspection.root,
            core::mem::take(&mut self.inspection.elements),
        );
        self.inspection.storage.live_elements = self.inspection.elements.len();
        self.inspection.storage.free_element_slots = 0;
        self.inspection.storage.allocated_element_slots = self.inspection.elements.len();
        self.rebuild_indexes();
    }

    /// Returns the element row for `id`.
    #[must_use]
    pub fn element(&self, id: ElementId) -> Option<&ElementInspectionRow> {
        let index = self.element_offset(id)?;
        self.inspection.elements.get(index)
    }

    /// Returns the outline key that represents `id` in the element tree.
    #[must_use]
    pub fn outline_key_for_element(&self, id: ElementId) -> Option<OutlineNodeKey> {
        self.element_offset(id).map(OutlineNodeKey::from)
    }

    /// Returns the element represented by `key` in the element outline.
    #[must_use]
    pub fn element_for_outline_key(&self, key: OutlineNodeKey) -> Option<ElementId> {
        self.inspection.elements.get(key.index()).map(|row| row.id)
    }

    /// Returns the composition-layer row for `id`.
    #[must_use]
    pub fn layer(&self, id: LayerInspectionId) -> Option<&CompositionLayerInspectionRow> {
        let index = self
            .layer_index
            .binary_search_by_key(&id, |(candidate, _)| *candidate)
            .ok()
            .map(|index| self.layer_index[index].1)?;
        self.inspection.composition.layers.get(index)
    }

    /// Returns whether `id` is a live Overstory-owned surface in the snapshot.
    #[must_use]
    pub fn has_surface(&self, id: SurfaceInspectionId) -> bool {
        self.surface_index
            .binary_search_by_key(&id, |(candidate, _)| *candidate)
            .is_ok()
    }

    /// Projects the retained element tree into outline rows.
    #[must_use]
    pub fn element_outline(&self) -> Vec<ElementOutlineRow> {
        let mut rows = Vec::new();
        self.push_element_outline(self.inspection.root, 0, &mut rows);
        rows
    }

    /// Projects the retained element tree into string-backed outline nodes.
    ///
    /// The returned nodes use element snapshot indices as
    /// [`OutlineNodeKey`] values, so an outline selection can be mapped back
    /// to `inspection.elements[key.index()].id`.
    #[must_use]
    pub fn element_outline_nodes(&self) -> Vec<OutlineViewNode> {
        self.inspection
            .elements
            .iter()
            .map(|row| {
                let mut node = OutlineViewNode::new(element_label(self, row));
                if let Some(first_child) = row
                    .children
                    .first()
                    .and_then(|child| self.element_offset(*child))
                {
                    node = node.with_first_child(Some(first_child));
                }
                if let Some(next_sibling) = self.next_sibling_offset(row) {
                    node = node.with_next_sibling(Some(next_sibling));
                }
                node
            })
            .collect()
    }

    /// Builds an [`OutlineView`] for the retained element tree.
    ///
    /// Rows with children start expanded so the first devtools view shows the
    /// whole retained tree without requiring interaction.
    #[must_use]
    pub fn element_outline_view(&self) -> OutlineView {
        let mut view = OutlineView::new().nodes(self.element_outline_nodes(), Some(0));
        for (index, row) in self.inspection.elements.iter().enumerate() {
            if !row.children.is_empty() {
                view = view.expanded(OutlineNodeKey::from(index));
            }
        }
        view
    }

    fn push_element_outline(
        &self,
        element: ElementId,
        depth: usize,
        rows: &mut Vec<ElementOutlineRow>,
    ) {
        let Some(row) = self.element(element) else {
            return;
        };
        rows.push(ElementOutlineRow {
            element,
            parent: row.parent,
            depth,
            child_count: row.children.len(),
            has_widget_identity: row.has_widget_identity,
            layer: row.layer,
            surface: row.surface,
        });
        for child in &row.children {
            self.push_element_outline(*child, depth + 1, rows);
        }
    }

    pub(crate) fn element_offset(&self, id: ElementId) -> Option<usize> {
        self.element_index
            .binary_search_by_key(&id, |(candidate, _)| *candidate)
            .ok()
            .map(|index| self.element_index[index].1)
    }

    pub(crate) fn next_sibling_offset(&self, row: &ElementInspectionRow) -> Option<usize> {
        let siblings = row
            .parent
            .and_then(|parent| self.element(parent))
            .map(|parent| parent.children.as_slice())?;
        let sibling = siblings
            .iter()
            .position(|candidate| *candidate == row.id)
            .and_then(|index| siblings.get(index + 1))?;
        self.element_offset(*sibling)
    }

    fn push_snapshot_subtree(&self, element: ElementId, removed: &mut Vec<ElementId>) {
        if contains_element(removed, element) {
            return;
        }
        removed.push(element);
        let Some(row) = self.element(element) else {
            return;
        };
        for child in &row.children {
            self.push_snapshot_subtree(*child, removed);
        }
    }

    fn rebuild_indexes(&mut self) {
        self.element_index = self
            .inspection
            .elements
            .iter()
            .enumerate()
            .map(|(index, row)| (row.id, index))
            .collect();
        self.element_index.sort_by_key(|(id, _)| *id);

        self.layer_index = self
            .inspection
            .composition
            .layers
            .iter()
            .enumerate()
            .map(|(index, row)| (row.id, index))
            .collect();
        self.layer_index.sort_by_key(|(id, _)| *id);

        self.surface_index = self
            .inspection
            .composition
            .surfaces
            .iter()
            .copied()
            .enumerate()
            .map(|(index, surface)| (surface, index))
            .collect();
        self.surface_index.sort_by_key(|(id, _)| *id);
    }
}

fn ordered_element_rows(
    root: ElementId,
    rows: Vec<ElementInspectionRow>,
) -> Vec<ElementInspectionRow> {
    let mut ordered = Vec::new();
    push_ordered_row(root, &rows, &mut ordered);
    for row in rows {
        if !ordered.iter().any(|ordered| ordered.id == row.id) {
            ordered.push(row);
        }
    }
    ordered
}

fn push_ordered_row(
    element: ElementId,
    rows: &[ElementInspectionRow],
    ordered: &mut Vec<ElementInspectionRow>,
) {
    if ordered.iter().any(|row| row.id == element) {
        return;
    }
    let Some(row) = rows.iter().find(|row| row.id == element) else {
        return;
    };
    ordered.push(row.clone());
    for child in &row.children {
        push_ordered_row(*child, rows, ordered);
    }
}

fn sanitize_template_links(
    row: &mut ElementInspectionRow,
    is_included: impl Fn(ElementId) -> bool,
) {
    if row
        .templated_parent
        .is_some_and(|owner| !is_included(owner))
    {
        row.templated_parent = None;
        row.template_slot = None;
        row.template_key = None;
    }
    if let Some(contract) = &mut row.template_contract {
        sanitize_template_slot_links(&mut contract.required_slots, &is_included);
        sanitize_template_slot_links(&mut contract.optional_slots, &is_included);
        sanitize_template_slot_links(&mut contract.recommended_slots, &is_included);
        contract
            .diagnostics
            .retain(|diagnostic| template_diagnostic_is_included(diagnostic, &is_included));
    }
}

fn template_diagnostic_is_included(
    diagnostic: &TemplateDiagnostic,
    is_included: &impl Fn(ElementId) -> bool,
) -> bool {
    match *diagnostic {
        TemplateDiagnostic::DuplicateSlot {
            first, duplicate, ..
        }
        | TemplateDiagnostic::DuplicatePart {
            first, duplicate, ..
        } => is_included(first) && is_included(duplicate),
    }
}

fn sanitize_template_slot_links(
    slots: &mut [TemplateSlotInspectionRow],
    is_included: &impl Fn(ElementId) -> bool,
) {
    for slot in slots {
        if slot.element.is_some_and(|element| !is_included(element)) {
            slot.element = None;
        }
    }
}

fn filter_inspection_snapshot(
    mut snapshot: UiInspectionSnapshot,
    root: ElementId,
    excluded_roots: &[ElementId],
) -> UiInspectionSnapshot {
    let mut included = Vec::new();
    push_included_elements(root, excluded_roots, &snapshot.elements, &mut included);

    snapshot.elements = snapshot
        .elements
        .into_iter()
        .filter(|row| contains_element(&included, row.id))
        .map(|mut row| {
            if row.id == root {
                row.parent = None;
            } else {
                row.parent = row.parent.filter(|id| contains_element(&included, *id));
            }
            row.children
                .retain(|child| contains_element(&included, *child));
            sanitize_template_links(&mut row, |element| contains_element(&included, element));
            row
        })
        .collect();
    snapshot.root = root;
    snapshot.composition = filtered_composition_snapshot(snapshot.composition, &included);
    snapshot.storage = filtered_storage_snapshot(snapshot.storage, &snapshot);
    snapshot
}

fn element_is_descendant_or_self(ui: &Ui, root: ElementId, element: ElementId) -> bool {
    let mut current = Some(element);
    while let Some(candidate) = current {
        if candidate == root {
            return true;
        }
        current = ui.get(candidate).and_then(overstory::Element::parent);
    }
    false
}

fn push_included_elements(
    element: ElementId,
    excluded_roots: &[ElementId],
    rows: &[ElementInspectionRow],
    included: &mut Vec<ElementId>,
) {
    if contains_element(excluded_roots, element) || contains_element(included, element) {
        return;
    }
    let Some(row) = rows.iter().find(|row| row.id == element) else {
        return;
    };
    included.push(element);
    for child in &row.children {
        push_included_elements(*child, excluded_roots, rows, included);
    }
}

fn filtered_composition_snapshot(
    composition: CompositionInspectionSnapshot,
    included_elements: &[ElementId],
) -> CompositionInspectionSnapshot {
    let mut layers: Vec<_> = composition
        .layers
        .into_iter()
        .filter(|row| {
            row.element
                .is_some_and(|element| contains_element(included_elements, element))
        })
        .collect();
    let included_layers: Vec<_> = layers.iter().map(|row| row.id).collect();
    for row in &mut layers {
        row.parent = row
            .parent
            .filter(|id| contains_layer(&included_layers, *id));
        row.children
            .retain(|child| contains_layer(&included_layers, *child));
    }
    let roots = layers
        .iter()
        .filter(|row| row.parent.is_none())
        .map(|row| row.id)
        .collect();
    let surfaces: Vec<_> = layers.iter().filter_map(|row| row.surface).collect();
    let updated_surfaces = composition
        .updated_surfaces
        .into_iter()
        .filter(|surface| surfaces.contains(surface))
        .collect();

    CompositionInspectionSnapshot {
        roots,
        layers,
        surfaces,
        updated_surfaces,
        changes: composition.changes,
    }
}

fn filtered_storage_snapshot(
    mut storage: UiStorageInspectionSnapshot,
    snapshot: &UiInspectionSnapshot,
) -> UiStorageInspectionSnapshot {
    storage.live_elements = snapshot.elements.len();
    storage.free_element_slots = 0;
    storage.allocated_element_slots = snapshot.elements.len();
    storage.composition_layers = snapshot.composition.layers.len();
    storage.composition_surfaces = snapshot.composition.surfaces.len();
    storage
}

fn contains_element(elements: &[ElementId], element: ElementId) -> bool {
    elements.contains(&element)
}

fn contains_layer(layers: &[LayerInspectionId], layer: LayerInspectionId) -> bool {
    layers.contains(&layer)
}

/// One projected retained-element outline row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ElementOutlineRow {
    /// Element represented by this row.
    pub element: ElementId,
    /// Parent element, or `None` for the root.
    pub parent: Option<ElementId>,
    /// Tree depth from the root.
    pub depth: usize,
    /// Number of retained children.
    pub child_count: usize,
    /// Whether the element has widget selector identity.
    pub has_widget_identity: bool,
    /// Composition layer corresponding to this element, if one exists.
    pub layer: Option<LayerInspectionId>,
    /// Surface attached to the element's layer, if one exists.
    pub surface: Option<SurfaceInspectionId>,
}

/// Element ids created by [`append_inspection_view`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DevtoolsViewElements {
    /// Root panel for the devtools view.
    pub root: ElementId,
    /// Outline control showing the retained element tree.
    pub outline: ElementId,
    /// Details panel for the selected element.
    pub details: ElementId,
}

const MAX_RELATIONSHIP_CHILDREN: usize = 8;
const INSPECTOR_OUTLINE_WIDTH: f32 = 220.0;
const INSPECTOR_DETAILS_WIDTH: f32 = 300.0;
const INSPECTOR_VIEW_HEIGHT: f32 = 420.0;
const INSPECTOR_PADDING: f32 = 8.0;
const INSPECTOR_GAP: f32 = 12.0;

/// Appends a simple inspector view for `snapshot`.
///
/// This is a bootstrap UI for same-process devtools: it renders the retained
/// element tree in an [`OutlineView`] and shows selected element and
/// composition details beside it. Selecting an outline row rebuilds the details
/// panel from the same snapshot.
pub fn append_inspection_view(
    ui: &mut Ui,
    parent: ElementId,
    snapshot: &DevtoolsSnapshot,
    selected: Option<ElementId>,
) -> DevtoolsViewElements {
    append_session_view(
        ui,
        parent,
        DevtoolsSessionHandle::new(
            DevtoolsSession::new(snapshot.clone()).with_selected_element(selected),
        ),
    )
}

/// Appends an inspector view backed by `session`.
///
/// Selection changes update both the rendered details panel and the supplied
/// session handle.
#[cfg(feature = "spoor-session")]
pub fn append_session_view(
    ui: &mut Ui,
    parent: ElementId,
    session: DevtoolsSessionHandle,
) -> DevtoolsViewElements {
    let root = ui.append(
        parent,
        Panel::row(INSPECTOR_GAP)
            .padding_all(f64::from(INSPECTOR_PADDING))
            .layout(inspector_root_layout()),
    );
    if let Some(foreground) = ui.theme().get::<overstory::Brush>(theme::ON_SURFACE_FG) {
        ui.set_local(root, ui.properties().foreground, foreground.clone());
    }
    append_session_view_contents(ui, root, session)
}

/// Rebuilds an existing inspector view from `session`.
///
/// Use this after a target refresh when the rendered inspector should show
/// a changed element tree or details surface. The root element is preserved so
/// outer layout and styling remain stable; only the outline and details
/// children are recreated.
#[cfg(feature = "spoor-session")]
pub fn refresh_session_view(
    ui: &mut Ui,
    elements: DevtoolsViewElements,
    session: DevtoolsSessionHandle,
) -> DevtoolsViewElements {
    clear_children(ui, elements.root);
    append_session_view_contents(ui, elements.root, session)
}

/// Refreshes an existing inspector view from a patched `mirror`.
///
/// The view state is preserved by mapping currently expanded rows through
/// `previous_tree` element ids into the mirror's current tree keys, then
/// replacing the outline control's model in place.
#[cfg(feature = "spoor-session")]
pub fn refresh_session_view_from_mirror(
    ui: &mut Ui,
    elements: DevtoolsViewElements,
    previous_tree: &InspectionElementTree,
    mirror: &InspectionMirror,
    session: DevtoolsSessionHandle,
) -> DevtoolsViewElements {
    let current_tree = mirror.tree();
    let expanded_elements = expanded_outline_keys(ui, elements.outline)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|key| previous_tree.element_for_outline_key(key))
        .collect::<Vec<_>>();
    let expanded_keys = expanded_elements
        .into_iter()
        .filter_map(|element| current_tree.outline_key_for_element(element))
        .collect::<Vec<_>>();
    let selected_key = session
        .with(DevtoolsSession::selected_element)
        .and_then(|element| current_tree.outline_key_for_element(element));

    if !replace_outline_nodes(
        ui,
        elements.outline,
        current_tree.outline_nodes(),
        current_tree.root_key(),
        expanded_keys,
        selected_key,
    ) {
        return refresh_session_view(ui, elements, session);
    }

    refresh_session_details_from_mirror(ui, elements, mirror, session)
}

/// Refreshes only the selected details pane from a patched `mirror`.
///
/// Live row changes such as hover state, selected-element properties, or
/// bounds updates should not recreate the outline control. Use this when the
/// projected tree is unchanged but the currently inspected row changed.
#[cfg(feature = "spoor-session")]
pub fn refresh_session_details_from_mirror(
    ui: &mut Ui,
    elements: DevtoolsViewElements,
    mirror: &InspectionMirror,
    session: DevtoolsSessionHandle,
) -> DevtoolsViewElements {
    clear_children(ui, elements.details);
    let client = InspectionClient::new(session.clone());
    let actions = DetailActions::from_client(client, elements.outline);
    append_inspection_element_details(
        ui,
        elements.details,
        mirror.selected_details(),
        Some(&actions),
    );
    elements
}

#[cfg(feature = "spoor-session")]
fn append_session_view_contents(
    ui: &mut Ui,
    root: ElementId,
    session: DevtoolsSessionHandle,
) -> DevtoolsViewElements {
    let details_slot: Rc<Cell<Option<ElementId>>> = Rc::new(Cell::new(None));
    let details_for_selection = Rc::clone(&details_slot);
    let client = InspectionClient::new(session.clone());
    let _ = client.subscribe(InspectionSubscription::all());
    let mirror = InspectionMirror::attach(&client).unwrap_or_else(|_| {
        session.with(|session| {
            InspectionMirror::from_snapshot(session.snapshot(), session.selected_element())
        })
    });
    let client_for_selection = client.clone();
    let outline_view = mirror
        .outline_view()
        .size(INSPECTOR_OUTLINE_WIDTH, INSPECTOR_VIEW_HEIGHT)
        .layout(inspector_pane_layout(INSPECTOR_OUTLINE_WIDTH));
    let outline = ui.append(
        root,
        outline_view.on_selection_changed(move |ui, event| {
            let Some(details) = details_for_selection.get() else {
                return;
            };
            clear_children(ui, details);
            let tree = match client_for_selection.element_tree() {
                Ok(tree) => tree,
                Err(error) => {
                    append_session_error(ui, details, "Tree request failed", &error);
                    return;
                }
            };
            let selected = event
                .selected_key
                .and_then(|key| tree.element_for_outline_key(key));
            let selected = match client_for_selection.set_selection(selected) {
                Ok(selected) => selected,
                Err(error) => {
                    append_session_error(ui, details, "Selection request failed", &error);
                    return;
                }
            };
            let detail_element = selected.or_else(|| tree.root_element());
            let detail_result = detail_element
                .map(|element| client_for_selection.element_details(element))
                .transpose();
            let detail = match detail_result {
                Ok(detail) => detail,
                Err(error) => {
                    append_session_error(ui, details, "Detail request failed", &error);
                    return;
                }
            };
            let actions = DetailActions::from_client(client_for_selection.clone(), event.outline);
            append_inspection_element_details(ui, details, detail.as_ref(), Some(&actions));
        }),
    );
    let details_frame = ui.append(
        root,
        ScrollArea::new()
            .size(INSPECTOR_DETAILS_WIDTH, INSPECTOR_VIEW_HEIGHT)
            .layout(inspector_pane_layout(INSPECTOR_DETAILS_WIDTH))
            .gap(6.0)
            .scrollbar_thickness(8.0)
            .thumb_extent(36.0),
    );
    let details_viewport = scroll_area_viewport(ui, details_frame)
        .expect("ScrollArea should expose its viewport immediately after append");
    details_slot.set(Some(details_viewport));

    let actions = DetailActions::from_client(client.clone(), outline);
    append_inspection_element_details(
        ui,
        details_viewport,
        mirror.selected_details(),
        Some(&actions),
    );

    DevtoolsViewElements {
        root,
        outline,
        details: details_viewport,
    }
}

/// Appends an inspector view backed by `session`.
///
/// Selection changes update both the rendered details panel and the supplied
/// session handle.
#[cfg(not(feature = "spoor-session"))]
pub fn append_session_view(
    ui: &mut Ui,
    parent: ElementId,
    session: DevtoolsSessionHandle,
) -> DevtoolsViewElements {
    let root = ui.append(
        parent,
        Panel::row(INSPECTOR_GAP)
            .padding_all(f64::from(INSPECTOR_PADDING))
            .layout(inspector_root_layout()),
    );
    if let Some(foreground) = ui.theme().get::<overstory::Brush>(theme::ON_SURFACE_FG) {
        ui.set_local(root, ui.properties().foreground, foreground.clone());
    }
    append_session_view_contents(ui, root, session)
}

/// Rebuilds an existing inspector view from `session`.
///
/// Use this after a target snapshot refresh when the rendered inspector should
/// show a changed element tree or details surface. The root element is
/// preserved so outer layout and styling remain stable; only the outline and
/// details children are recreated.
#[cfg(not(feature = "spoor-session"))]
pub fn refresh_session_view(
    ui: &mut Ui,
    elements: DevtoolsViewElements,
    session: DevtoolsSessionHandle,
) -> DevtoolsViewElements {
    clear_children(ui, elements.root);
    append_session_view_contents(ui, elements.root, session)
}

#[cfg(not(feature = "spoor-session"))]
fn append_session_view_contents(
    ui: &mut Ui,
    root: ElementId,
    session: DevtoolsSessionHandle,
) -> DevtoolsViewElements {
    let details_slot: Rc<Cell<Option<ElementId>>> = Rc::new(Cell::new(None));
    let details_for_selection = Rc::clone(&details_slot);
    let session_for_selection = session.clone();
    let mut outline_view = session.with(|session| {
        session
            .snapshot()
            .element_outline_view()
            .size(INSPECTOR_OUTLINE_WIDTH, INSPECTOR_VIEW_HEIGHT)
            .layout(inspector_pane_layout(INSPECTOR_OUTLINE_WIDTH))
    });
    if let Some(key) = session.with(DevtoolsSession::selected_outline_key) {
        outline_view = outline_view.selected_key(key);
    }
    let outline = ui.append(
        root,
        outline_view.on_selection_changed(move |ui, event| {
            let Some(details) = details_for_selection.get() else {
                return;
            };
            clear_children(ui, details);
            let selected = session_for_selection.with_mut(|session| {
                let _ = session.select_outline_key(event.selected_key);
                session.selected_element()
            });
            let actions = DetailActions::from_session(session_for_selection.clone(), event.outline);
            session_for_selection.with(|session| {
                append_selected_element_details(
                    ui,
                    details,
                    session.snapshot(),
                    selected,
                    Some(&actions),
                );
            });
        }),
    );
    let details_frame = ui.append(
        root,
        ScrollArea::new()
            .size(INSPECTOR_DETAILS_WIDTH, INSPECTOR_VIEW_HEIGHT)
            .layout(inspector_pane_layout(INSPECTOR_DETAILS_WIDTH))
            .gap(6.0)
            .scrollbar_thickness(8.0)
            .thumb_extent(36.0),
    );
    let details_viewport = scroll_area_viewport(ui, details_frame)
        .expect("ScrollArea should expose its viewport immediately after append");
    details_slot.set(Some(details_viewport));

    let actions = DetailActions::from_session(session.clone(), outline);
    session.with(|session| {
        append_selected_element_details(
            ui,
            details_viewport,
            session.snapshot(),
            session.selected_element(),
            Some(&actions),
        );
    });

    DevtoolsViewElements {
        root,
        outline,
        details: details_viewport,
    }
}

fn inspector_root_layout() -> Layout {
    Layout::row(INSPECTOR_GAP)
        .fill_height()
        .flex_grow(1.0)
        .flex_shrink(1.0)
}

fn inspector_pane_layout(width: f32) -> Layout {
    Layout::new()
        .logical_size(width, INSPECTOR_VIEW_HEIGHT)
        .fill_height()
        .flex_shrink(0.0)
}

fn clear_children(ui: &mut Ui, parent: ElementId) {
    let children = ui
        .get(parent)
        .map(|element| element.children().to_vec())
        .unwrap_or_default();
    for child in children {
        ui.remove(child);
    }
}

fn element_label(snapshot: &DevtoolsSnapshot, row: &ElementInspectionRow) -> String {
    let prefix = element_display_label(snapshot, row);
    format!("{prefix} {}:{}", row.id.slot(), row.id.generation())
}

fn element_display_label(snapshot: &DevtoolsSnapshot, row: &ElementInspectionRow) -> String {
    if let Some(label) = template_part_display_label(snapshot, row) {
        return label.into();
    }
    element_type_label(snapshot, row)
}

fn element_type_label(snapshot: &DevtoolsSnapshot, row: &ElementInspectionRow) -> String {
    let Some(tag) = row.type_tag else {
        return String::from("Element");
    };
    snapshot
        .inspection
        .style_vocabulary
        .type_name(tag)
        .map_or_else(
            || {
                if row.has_widget_identity {
                    String::from("Widget")
                } else {
                    String::from("Element")
                }
            },
            String::from,
        )
}

#[cfg(not(feature = "spoor-session"))]
fn append_selected_element_details(
    ui: &mut Ui,
    parent: ElementId,
    snapshot: &DevtoolsSnapshot,
    selected: Option<ElementId>,
    actions: Option<&DetailActions>,
) {
    let selected = selected.unwrap_or(snapshot.inspection.root);
    let Some(row) = snapshot.element(selected) else {
        ui.append(parent, Text::new("No selected element").font_size(13.0));
        return;
    };

    let grid = selected_element_property_grid(snapshot, row);
    append_property_grid_sections(ui, parent, &grid, actions);
}

#[cfg(feature = "spoor-session")]
fn append_inspection_element_details(
    ui: &mut Ui,
    parent: ElementId,
    details: Option<&InspectionElementDetails>,
    actions: Option<&DetailActions>,
) {
    let Some(details) = details else {
        ui.append(parent, Text::new("No selected element").font_size(13.0));
        return;
    };
    append_property_grid_sections(ui, parent, &details.properties, actions);
}

#[cfg(feature = "spoor-session")]
fn append_session_error(
    ui: &mut Ui,
    parent: ElementId,
    label: &str,
    error: &spoor_session::SessionError,
) {
    ui.append(
        parent,
        Text::new(format!("{label}: {}", error.message)).font_size(13.0),
    );
}

fn selected_element_property_grid(
    snapshot: &DevtoolsSnapshot,
    row: &ElementInspectionRow,
) -> PropertyGrid {
    let identity = PropertyGridSection::new("Element")
        .row(PropertyGridRow::new(
            "Element id",
            format!("{}:{}", row.id.slot(), row.id.generation()),
        ))
        .row(PropertyGridRow::new(
            "Type",
            element_type_label(snapshot, row),
        ))
        .row(PropertyGridRow::new("Role", element_role(snapshot, row)))
        .row(PropertyGridRow::new(
            "Widget behavior",
            row.has_widget_identity,
        ))
        .row(type_tag_row(snapshot, row));

    let mut children = PropertyGridRow::new("Children", row.children.len());
    for (index, child) in row
        .children
        .iter()
        .take(MAX_RELATIONSHIP_CHILDREN)
        .copied()
        .enumerate()
    {
        children.push_child(element_relation_row(
            snapshot,
            &child_relation_label(snapshot, row, index, child),
            Some(child),
        ));
    }
    if row.children.len() > MAX_RELATIONSHIP_CHILDREN {
        children.push_child(PropertyGridRow::new(
            "More children",
            PropertyGridValue::count(row.children.len() - MAX_RELATIONSHIP_CHILDREN),
        ));
    }
    let tree = PropertyGridSection::new("Hierarchy")
        .row(element_relation_row(snapshot, "Parent", row.parent))
        .row(children)
        .row(element_relation_row(
            snapshot,
            "Template owner",
            row.templated_parent,
        ));

    let mut template = PropertyGridSection::new("Template")
        .row(PropertyGridRow::new("Role", template_role(snapshot, row)))
        .row(PropertyGridRow::new(
            "Owned part count",
            template_part_count(snapshot, row.id),
        ))
        .row(element_relation_row(
            snapshot,
            "Owner",
            row.templated_parent,
        ))
        .row(PropertyGridRow::new(
            "Style part",
            part_tag_value(snapshot, row),
        ))
        .row(PropertyGridRow::new(
            "Slot",
            row.template_slot
                .map_or_else(PropertyGridValue::empty, PropertyGridValue::token),
        ))
        .row(PropertyGridRow::new("Key", optional_u64(row.template_key)));
    if row.templated_parent.is_some() {
        template.push_row(element_relation_row(
            snapshot,
            "Routed physical source",
            Some(row.id),
        ));
        template.push_row(element_relation_row(
            snapshot,
            "Routed semantic target",
            row.templated_parent,
        ));
    }
    if let Some(contract) = &row.template_contract {
        template.push_row(template_contract_status_row(contract));
        template.push_row(template_contract_slots_row(
            snapshot,
            row,
            "Required slots",
            &contract.required_slots,
            "missing",
        ));
        if !contract.optional_slots.is_empty() {
            template.push_row(template_contract_slots_row(
                snapshot,
                row,
                "Optional slots",
                &contract.optional_slots,
                "not present",
            ));
        }
        if !contract.recommended_slots.is_empty() {
            template.push_row(template_contract_slots_row(
                snapshot,
                row,
                "Recommended slots",
                &contract.recommended_slots,
                "missing",
            ));
        }
        if !contract.diagnostics.is_empty() {
            template.push_row(template_contract_diagnostics_row(
                snapshot,
                row,
                &contract.diagnostics,
            ));
        }
    }

    let style = PropertyGridSection::new("Style")
        .row(class_list_row(snapshot, row))
        .row(pseudo_list_row(snapshot, row));

    let state = PropertyGridSection::new("State")
        .row(PropertyGridRow::new("Enabled", row.state.enabled))
        .row(PropertyGridRow::new("Focusable", row.state.focusable))
        .row(PropertyGridRow::new("Focused", row.state.focused))
        .row(PropertyGridRow::new(
            "Focus visible",
            row.state.focus_visible,
        ))
        .row(PropertyGridRow::new("Hovered", row.state.hovered))
        .row(PropertyGridRow::new("Pressed", row.state.pressed))
        .row(PropertyGridRow::new(
            "Pointer capture",
            row.state.pointer_captured,
        ));

    let layout = PropertyGridSection::new("Geometry")
        .row(PropertyGridRow::new("Bounds", rect_summary(row.bounds)));

    let mut layer = PropertyGridRow::new("Layer", optional_layer_id(row.layer));
    if let Some(layer_row) = row.layer.and_then(|id| snapshot.layer(id)) {
        layer.push_child(PropertyGridRow::new(
            "Bounds",
            size_summary(layer_row.bounds),
        ));
        layer.push_child(
            PropertyGridRow::new(
                "Opacity",
                format!(
                    "{:.3} / {:.3}",
                    layer_row.local_opacity, layer_row.effective_opacity
                ),
            )
            .child(PropertyGridRow::new(
                "Local",
                format!("{:.3}", layer_row.local_opacity),
            ))
            .child(PropertyGridRow::new(
                "Effective",
                format!("{:.3}", layer_row.effective_opacity),
            )),
        );
        layer.push_child(
            PropertyGridRow::new(
                "Hidden",
                format!("{} / {}", layer_row.hidden, layer_row.effectively_hidden),
            )
            .child(PropertyGridRow::new("Local", layer_row.hidden))
            .child(PropertyGridRow::new(
                "Effective",
                layer_row.effectively_hidden,
            )),
        );
        layer.push_child(PropertyGridRow::new("Clip", layer_row.has_clip));
    }
    let composition = PropertyGridSection::new("Composition")
        .row(layer)
        .row(PropertyGridRow::new(
            "Surface id",
            optional_surface_id(row.surface),
        ))
        .row(PropertyGridRow::new(
            "Layer policy",
            format!("{:?}", row.composition_policy),
        ));

    let mut grid = PropertyGrid::new()
        .section(identity)
        .section(tree)
        .section(template)
        .section(style);
    for section in property_grid_sections(row) {
        grid.push_section(section);
    }
    grid.section(state).section(layout).section(composition)
}

fn element_relation_row(
    snapshot: &DevtoolsSnapshot,
    label: &str,
    target: Option<ElementId>,
) -> PropertyGridRow {
    match target {
        Some(element) => PropertyGridRow::new(
            label,
            PropertyGridValue::element_link(element, element_relation_summary(snapshot, element)),
        ),
        None => PropertyGridRow::new(label, PropertyGridValue::empty()),
    }
}

fn element_relation_summary(snapshot: &DevtoolsSnapshot, element: ElementId) -> String {
    snapshot.element(element).map_or_else(
        || element_id_summary(element),
        |row| {
            format!(
                "{} {}",
                element_display_label(snapshot, row),
                element_id_summary(element)
            )
        },
    )
}

fn template_contract_status_row(contract: &TemplateContractInspection) -> PropertyGridRow {
    let status = if contract.has_missing_required_slots() {
        PropertyGridValue::token("missing required")
    } else if !contract.diagnostics.is_empty() {
        PropertyGridValue::token("diagnostics")
    } else if contract.has_missing_recommended_slots() {
        PropertyGridValue::token("degraded")
    } else {
        PropertyGridValue::token("satisfied")
    };
    PropertyGridRow::new("Contract", status)
}

fn template_contract_diagnostics_row(
    snapshot: &DevtoolsSnapshot,
    owner: &ElementInspectionRow,
    diagnostics: &[TemplateDiagnostic],
) -> PropertyGridRow {
    let mut row = PropertyGridRow::new("Diagnostics", PropertyGridValue::count(diagnostics.len()));
    for diagnostic in diagnostics {
        row.push_child(template_diagnostic_row(snapshot, owner, diagnostic));
    }
    row
}

fn template_diagnostic_row(
    snapshot: &DevtoolsSnapshot,
    owner: &ElementInspectionRow,
    diagnostic: &TemplateDiagnostic,
) -> PropertyGridRow {
    match *diagnostic {
        TemplateDiagnostic::DuplicateSlot {
            slot,
            key,
            first,
            duplicate,
        } => {
            let mut row =
                PropertyGridRow::new("Duplicate slot", PropertyGridValue::token(slot.name()));
            row.push_child(PropertyGridRow::new("Key", optional_template_key(key)));
            if let Some(part_tag) = slot.part_tag() {
                row.push_child(PropertyGridRow::new(
                    "Style part",
                    owner_part_tag_value(owner.type_tag.and_then(|owner_type_tag| {
                        snapshot
                            .inspection
                            .style_vocabulary
                            .part_name(owner_type_tag, part_tag)
                    })),
                ));
            }
            push_duplicate_elements(snapshot, &mut row, first, duplicate);
            row
        }
        TemplateDiagnostic::DuplicatePart {
            part_tag,
            key,
            first,
            duplicate,
        } => {
            let mut row = PropertyGridRow::new(
                "Duplicate style part",
                owner_part_tag_value(owner.type_tag.and_then(|owner_type_tag| {
                    snapshot
                        .inspection
                        .style_vocabulary
                        .part_name(owner_type_tag, part_tag)
                })),
            );
            row.push_child(PropertyGridRow::new("Key", optional_template_key(key)));
            push_duplicate_elements(snapshot, &mut row, first, duplicate);
            row
        }
    }
}

fn push_duplicate_elements(
    snapshot: &DevtoolsSnapshot,
    row: &mut PropertyGridRow,
    first: ElementId,
    duplicate: ElementId,
) {
    row.push_child(element_relation_row(snapshot, "First", Some(first)));
    row.push_child(element_relation_row(snapshot, "Duplicate", Some(duplicate)));
}

fn template_contract_slots_row(
    snapshot: &DevtoolsSnapshot,
    owner: &ElementInspectionRow,
    label: &str,
    slots: &[TemplateSlotInspectionRow],
    absent_label: &'static str,
) -> PropertyGridRow {
    let mut row = PropertyGridRow::new(label, PropertyGridValue::count(slots.len()));
    for slot in slots {
        row.push_child(template_contract_slot_row(
            snapshot,
            owner,
            slot,
            absent_label,
        ));
    }
    row
}

fn template_contract_slot_row(
    snapshot: &DevtoolsSnapshot,
    owner: &ElementInspectionRow,
    slot: &TemplateSlotInspectionRow,
    absent_label: &'static str,
) -> PropertyGridRow {
    let value = slot.element.map_or_else(
        || PropertyGridValue::token(absent_label),
        |element| {
            PropertyGridValue::element_link(element, element_relation_summary(snapshot, element))
        },
    );
    let mut row = PropertyGridRow::new(slot.name, value);
    if slot.part_tag.is_some() {
        row.push_child(PropertyGridRow::new(
            "Style part",
            contract_slot_part_tag_value(snapshot, owner, slot),
        ));
    }
    row
}

fn child_relation_label(
    snapshot: &DevtoolsSnapshot,
    parent: &ElementInspectionRow,
    index: usize,
    child: ElementId,
) -> String {
    let is_template_part = snapshot
        .element(child)
        .is_some_and(|row| row.templated_parent == Some(parent.id));
    if is_template_part {
        format!("Template part {index}")
    } else {
        format!("Child {index}")
    }
}

fn element_role(snapshot: &DevtoolsSnapshot, row: &ElementInspectionRow) -> &'static str {
    if row.templated_parent.is_some() {
        template_child_role(row)
    } else if template_part_count(snapshot, row.id) > 0 {
        "templated control"
    } else if row.has_widget_identity {
        "widget"
    } else {
        "element"
    }
}

fn template_role(snapshot: &DevtoolsSnapshot, row: &ElementInspectionRow) -> &'static str {
    if row.templated_parent.is_some() {
        template_child_role(row)
    } else if template_part_count(snapshot, row.id) > 0 {
        "owner"
    } else {
        "none"
    }
}

fn template_child_role(row: &ElementInspectionRow) -> &'static str {
    match (row.template_slot.is_some(), row.part_tag.is_some()) {
        (true, true) => "template slot part",
        (true, false) => "template slot",
        (false, true) => "template part",
        (false, false) => "template child",
    }
}

fn template_part_count(snapshot: &DevtoolsSnapshot, owner: ElementId) -> usize {
    snapshot
        .inspection()
        .elements
        .iter()
        .filter(|row| row.templated_parent == Some(owner))
        .count()
}

fn part_tag_value(snapshot: &DevtoolsSnapshot, row: &ElementInspectionRow) -> PropertyGridValue {
    if row.part_tag.is_none() {
        return PropertyGridValue::empty();
    }
    let label = template_part_display_label(snapshot, row).unwrap_or("Custom part");
    PropertyGridValue::token(label)
}

fn contract_slot_part_tag_value(
    snapshot: &DevtoolsSnapshot,
    owner: &ElementInspectionRow,
    slot: &TemplateSlotInspectionRow,
) -> PropertyGridValue {
    let Some(part_tag) = slot.part_tag else {
        return PropertyGridValue::empty();
    };
    owner_part_tag_value(owner.type_tag.and_then(|owner_type_tag| {
        snapshot
            .inspection
            .style_vocabulary
            .part_name(owner_type_tag, part_tag)
    }))
}

fn owner_part_tag_value(part_name: Option<&str>) -> PropertyGridValue {
    let label = part_name.unwrap_or("Custom part");
    PropertyGridValue::token(label)
}

fn template_part_display_label<'a>(
    snapshot: &'a DevtoolsSnapshot,
    row: &ElementInspectionRow,
) -> Option<&'a str> {
    let part_tag = row.part_tag?;
    let owner_type_tag = row
        .templated_parent
        .and_then(|owner| snapshot.element(owner))
        .and_then(|owner| owner.type_tag)?;
    snapshot
        .inspection
        .style_vocabulary
        .part_name(owner_type_tag, part_tag)
}

fn type_tag_row(snapshot: &DevtoolsSnapshot, row: &ElementInspectionRow) -> PropertyGridRow {
    if row.type_tag.is_none() {
        return PropertyGridRow::new("Type tag", PropertyGridValue::empty());
    }
    PropertyGridRow::new(
        "Type tag",
        PropertyGridValue::token(element_type_label(snapshot, row)),
    )
}

fn property_grid_sections(row: &ElementInspectionRow) -> Vec<PropertyGridSection> {
    let mut sections = Vec::new();
    push_property_group(
        &mut sections,
        row,
        ElementPropertyInspectionGroup::Appearance,
        "Appearance",
    );
    push_property_group(
        &mut sections,
        row,
        ElementPropertyInspectionGroup::Box,
        "Box model",
    );
    push_property_group(
        &mut sections,
        row,
        ElementPropertyInspectionGroup::Text,
        "Text",
    );
    push_property_group(
        &mut sections,
        row,
        ElementPropertyInspectionGroup::Layout,
        "Layout inputs",
    );
    push_property_group(
        &mut sections,
        row,
        ElementPropertyInspectionGroup::Motion,
        "Motion",
    );
    push_property_group(
        &mut sections,
        row,
        ElementPropertyInspectionGroup::Control,
        "Control inputs",
    );
    push_property_group(
        &mut sections,
        row,
        ElementPropertyInspectionGroup::Range,
        "Range",
    );
    sections
}

fn push_property_group(
    sections: &mut Vec<PropertyGridSection>,
    row: &ElementInspectionRow,
    group: ElementPropertyInspectionGroup,
    title: &str,
) {
    let mut section = PropertyGridSection::new(title);
    for property in row
        .properties
        .iter()
        .filter(|property| property.group == group)
    {
        section.push_row(inspection_property_row(property));
    }
    if !section.rows().is_empty() {
        sections.push(section);
    }
}

fn inspection_property_row(
    property: &overstory::inspection::ElementPropertyInspectionRow,
) -> PropertyGridRow {
    let mut row = PropertyGridRow::new(property.name, inspection_property_value(&property.value))
        .with_origin(property.origin.label());
    push_inspection_property_children(&mut row, &property.value);
    row
}

fn push_inspection_property_children(
    row: &mut PropertyGridRow,
    value: &ElementPropertyInspectionValue,
) {
    if let ElementPropertyInspectionValue::Record { fields, .. } = value {
        for field in fields {
            let mut child =
                PropertyGridRow::new(field.name, inspection_property_value(&field.value));
            push_inspection_property_children(&mut child, &field.value);
            row.push_child(child);
        }
    }
}

fn inspection_property_value(value: &ElementPropertyInspectionValue) -> PropertyGridValue {
    match value {
        ElementPropertyInspectionValue::Empty => PropertyGridValue::empty(),
        ElementPropertyInspectionValue::Text(value) => PropertyGridValue::text(value.clone()),
        ElementPropertyInspectionValue::Bool(value) => PropertyGridValue::Bool(*value),
        ElementPropertyInspectionValue::Number(value) => {
            PropertyGridValue::text(number_summary(*value))
        }
        ElementPropertyInspectionValue::Count(value) => PropertyGridValue::count(*value),
        ElementPropertyInspectionValue::Token(value) => PropertyGridValue::token(value.clone()),
        ElementPropertyInspectionValue::Record { summary, .. } => {
            PropertyGridValue::token(summary.clone())
        }
        ElementPropertyInspectionValue::Color(color) => PropertyGridValue::color(*color),
        ElementPropertyInspectionValue::Rect { x0, y0, x1, y1 } => {
            PropertyGridValue::text(rect_summary(overstory::Rect::new(*x0, *y0, *x1, *y1)))
        }
    }
}

fn number_summary(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.3}")
    }
}

fn class_list_row(snapshot: &DevtoolsSnapshot, row: &ElementInspectionRow) -> PropertyGridRow {
    let mut classes = list_summary_row("Classes", row.classes.len());
    for class in &row.classes {
        let label = snapshot
            .inspection
            .style_vocabulary
            .class_name(*class)
            .unwrap_or("Custom class");
        classes.push_child(PropertyGridRow::new(label, PropertyGridValue::token(label)));
    }
    classes
}

fn pseudo_list_row(snapshot: &DevtoolsSnapshot, row: &ElementInspectionRow) -> PropertyGridRow {
    let mut pseudos = list_summary_row("Pseudos", row.pseudos.len());
    for pseudo in &row.pseudos {
        let label = snapshot
            .inspection
            .style_vocabulary
            .pseudo_name(*pseudo)
            .unwrap_or("Custom pseudo");
        pseudos.push_child(PropertyGridRow::new(label, PropertyGridValue::token(label)));
    }
    pseudos
}

fn list_summary_row(label: &str, len: usize) -> PropertyGridRow {
    PropertyGridRow::new(
        label,
        if len == 0 {
            PropertyGridValue::empty()
        } else {
            PropertyGridValue::count(len)
        },
    )
}

fn element_id_summary(id: ElementId) -> String {
    format!("{}:{}", id.slot(), id.generation())
}

fn optional_u64(value: Option<u64>) -> PropertyGridValue {
    value.map_or_else(PropertyGridValue::empty, |value| {
        PropertyGridValue::text(format!("{value}"))
    })
}

fn optional_template_key(key: Option<TemplateItemKey>) -> PropertyGridValue {
    optional_u64(key.map(TemplateItemKey::value))
}

fn optional_layer_id(id: Option<LayerInspectionId>) -> PropertyGridValue {
    optional_display_token(id)
}

fn optional_surface_id(id: Option<SurfaceInspectionId>) -> PropertyGridValue {
    optional_display_token(id)
}

fn optional_display_token<T: ToString>(value: Option<T>) -> PropertyGridValue {
    value.map_or_else(PropertyGridValue::empty, |value| {
        PropertyGridValue::token(value.to_string())
    })
}

fn rect_summary(rect: overstory::Rect) -> String {
    format!(
        "{:.1},{:.1} {:.1}x{:.1}",
        rect.x0,
        rect.y0,
        rect.width(),
        rect.height()
    )
}

fn size_summary(size: overstory::Size) -> String {
    format!("{:.1}x{:.1}", size.width, size.height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use overstory::{ControlTemplateSpecExt, SurfaceSpecExt, TextSpecExt};
    use understory_property::DependencyObjectExt;

    #[test]
    fn snapshot_capture_indexes_elements_and_layers() {
        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.append(root, Panel::new());

        let snapshot = DevtoolsSnapshot::capture(&ui);
        assert_eq!(snapshot.inspection().root, root);
        assert_eq!(
            snapshot
                .element(child)
                .expect("child row should be indexed")
                .parent,
            Some(root)
        );

        let layer = snapshot
            .element(child)
            .expect("child row should be indexed")
            .layer
            .expect("child should have a composition layer");
        assert_eq!(
            snapshot
                .layer(layer)
                .expect("layer row should be indexed")
                .element,
            Some(child)
        );
    }

    #[test]
    fn element_outline_preserves_tree_depth() {
        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.append(root, Panel::new());
        let grandchild = ui.append(child, Panel::new());

        let snapshot = DevtoolsSnapshot::capture(&ui);
        let outline = snapshot.element_outline();

        assert_eq!(outline[0].element, root);
        assert_eq!(outline[0].depth, 0);
        assert_eq!(outline[1].element, child);
        assert_eq!(outline[1].depth, 1);
        assert_eq!(outline[2].element, grandchild);
        assert_eq!(outline[2].depth, 2);
    }

    #[test]
    fn element_outline_nodes_can_feed_outline_view() {
        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.append(root, Panel::new());

        let snapshot = DevtoolsSnapshot::capture(&ui);
        let nodes = snapshot.element_outline_nodes();

        assert_eq!(nodes.len(), 2);
        assert_eq!(
            nodes[0].label(),
            format!("Panel {}:{}", root.slot(), root.generation())
        );
        assert_eq!(
            nodes[1].label(),
            format!("Panel {}:{}", child.slot(), child.generation())
        );

        let view = snapshot.element_outline_view();
        assert!(!alloc::format!("{view:?}").is_empty());
    }

    #[test]
    fn element_outline_keys_map_back_to_elements() {
        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.append(root, Panel::new());

        let snapshot = DevtoolsSnapshot::capture(&ui);
        let key = snapshot
            .outline_key_for_element(child)
            .expect("child should have an outline key");

        assert_eq!(snapshot.element_for_outline_key(key), Some(child));
    }

    #[test]
    fn ui_inspection_target_can_capture_subtree() {
        let mut ui = Ui::new();
        let root = ui.root();
        let app = ui.append(root, Panel::new());
        let app_child = ui.append(app, overstory::Button::new("Deploy"));
        let devtools = ui.append(root, Panel::new());

        let target = UiInspectionTarget::new(&ui).rooted_at(app);
        let snapshot = DevtoolsSnapshot::capture(&target);

        assert_eq!(snapshot.inspection().root, app);
        assert!(snapshot.element(app).is_some());
        assert!(snapshot.element(app_child).is_some());
        assert!(snapshot.element(devtools).is_none());
        assert_eq!(
            snapshot.element(app).expect("app should exist").parent,
            None
        );
        assert_eq!(snapshot.inspection().storage.live_elements, 3);
    }

    #[test]
    fn ui_inspection_target_can_exclude_devtools_subtree() {
        let mut ui = Ui::new();
        let root = ui.root();
        let app = ui.append(root, Panel::new());
        let devtools = ui.append(root, Panel::new());
        let devtools_child = ui.append(devtools, overstory::Button::new("Refresh"));
        let excluded = [devtools];

        let target = UiInspectionTarget::new(&ui).excluding(&excluded);
        let snapshot = DevtoolsSnapshot::capture(&target);

        assert!(snapshot.element(root).is_some());
        assert!(snapshot.element(app).is_some());
        assert!(snapshot.element(devtools).is_none());
        assert!(snapshot.element(devtools_child).is_none());
        assert_eq!(
            snapshot.element(root).expect("root should exist").children,
            vec![app],
        );
    }

    #[test]
    fn ui_live_inspection_target_filters_drained_devtools_changes() {
        let mut ui = Ui::new();
        let root = ui.root();
        let app = ui.append(root, Panel::new());
        let devtools = ui.append(root, Panel::new());
        let excluded = [devtools];
        {
            let mut target = UiLiveInspectionTarget::new(&mut ui).excluding(&excluded);
            target.begin_live_inspection_recording();
            target.clear_inspection_changes();
        }

        let app_child = ui.append(app, Panel::new());
        let devtools_child = ui.append(devtools, Panel::new());
        let changes = {
            let mut target = UiLiveInspectionTarget::new(&mut ui).excluding(&excluded);
            target.drain_inspection_changes()
        };

        assert!(changes.tree.removed.is_empty());
        assert!(
            changes
                .tree
                .upserted
                .iter()
                .any(|row| row.id == app && row.children.contains(&app_child))
        );
        assert!(changes.tree.upserted.iter().any(|row| row.id == app_child));
        assert!(
            !changes
                .tree
                .upserted
                .iter()
                .any(|row| row.id == devtools || row.id == devtools_child)
        );

        ui.set_local(app_child, ui.properties().opacity, 0.5);
        ui.set_local(devtools_child, ui.properties().opacity, 0.5);
        let changes = {
            let mut target = UiLiveInspectionTarget::new(&mut ui).excluding(&excluded);
            target.drain_inspection_changes()
        };

        assert!(changes.tree.removed.is_empty());
        assert!(changes.tree.upserted.is_empty());
        assert!(changes.tree.updated.iter().any(|row| row.id == app_child));
        assert!(
            !changes
                .tree
                .updated
                .iter()
                .any(|row| row.id == devtools_child)
        );
        {
            let mut target = UiLiveInspectionTarget::new(&mut ui).excluding(&excluded);
            target.end_live_inspection_recording();
        }
    }

    #[test]
    fn devtools_session_selects_elements_by_outline_key() {
        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.append(root, Panel::new());
        let grandchild = ui.append(child, Panel::new());
        let snapshot = DevtoolsSnapshot::capture(&ui);
        let grandchild_key = snapshot
            .outline_key_for_element(grandchild)
            .expect("grandchild should have an outline key");

        let mut session = DevtoolsSession::new(snapshot).with_selected_element(Some(child));
        let event = session.select_outline_key(Some(grandchild_key));

        assert_eq!(
            event,
            Some(DevtoolsEvent::SelectionChanged {
                selected: Some(grandchild)
            })
        );
        assert_eq!(session.selected_element(), Some(grandchild));
        assert_eq!(session.selected_outline_key(), Some(grandchild_key));
    }

    #[test]
    fn devtools_session_replaces_snapshot_and_clears_stale_selection() {
        let mut first = Ui::new();
        let first_root = first.root();
        let selected = first.append(first_root, Panel::new());
        let mut session = DevtoolsSession::capture(&first).with_selected_element(Some(selected));

        let second = Ui::new();
        let event = session.replace_snapshot(DevtoolsSnapshot::capture(&second));

        #[cfg(feature = "spoor-session")]
        {
            let DevtoolsEvent::ElementTreePatched {
                selected: None,
                tree_patch,
            } = event
            else {
                panic!("snapshot replacement should emit an element tree patch");
            };
            assert_eq!(tree_patch.removed, vec![selected]);
        }
        #[cfg(not(feature = "spoor-session"))]
        assert_eq!(event, DevtoolsEvent::SnapshotReplaced { selected: None });
        assert_eq!(session.selected_element(), None);
    }

    #[cfg(feature = "spoor-session")]
    #[test]
    fn devtools_session_handles_live_element_detail_requests() {
        use spoor_session::{SessionRequestId, SessionResult};

        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.append(root, Panel::new());
        let mut session = DevtoolsSession::capture(&ui);

        let response = session.handle_inspection_request(LiveInspectionRequest::new(
            SessionRequestId::new(21),
            inspection_method("elementDetails"),
            InspectionRequest::ElementDetails { element: child },
        ));

        assert_eq!(response.request_id, SessionRequestId::new(21));
        let SessionResult::Ok(InspectionResponse::ElementDetails(details)) = response.result else {
            panic!("expected element details response");
        };
        assert_eq!(details.row.id, child);
        assert!(!details.properties.is_empty());
    }

    #[cfg(feature = "spoor-session")]
    #[test]
    fn devtools_session_streams_compact_deltas_for_matching_subscriptions() {
        use spoor_session::{SessionRequestId, SessionResult};

        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.append(root, Panel::new());
        let mut session = DevtoolsSession::capture(&ui);

        let response = session.handle_inspection_request(LiveInspectionRequest::new(
            SessionRequestId::new(22),
            inspection_method("subscribe"),
            InspectionRequest::Subscribe(InspectionSubscription::all()),
        ));
        let SessionResult::Ok(InspectionResponse::Subscribed {
            stream_id,
            subscription,
        }) = response.result
        else {
            panic!("expected subscription response");
        };
        assert_eq!(subscription, InspectionSubscription::all());

        let event = session
            .set_selected_element(Some(child))
            .expect("selection should change");
        let items = session.inspection_stream_items_for_event(event);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].stream_id, stream_id);
        assert_eq!(items[0].sequence, 0);
        assert_eq!(
            items[0].payload,
            InspectionStreamItem::Delta(InspectionDelta::new(
                InspectionSubsystem::Elements,
                InspectionUpdateReason::SelectionChanged,
            ))
        );
    }

    #[cfg(feature = "spoor-session")]
    #[test]
    fn devtools_session_streams_element_tree_patches_for_snapshot_replacement() {
        use spoor_session::{SessionRequestId, SessionResult};

        let mut ui = Ui::new();
        let root = ui.root();
        let mut session = DevtoolsSession::capture(&ui);

        let response = session.handle_inspection_request(LiveInspectionRequest::new(
            SessionRequestId::new(23),
            inspection_method("subscribe"),
            InspectionRequest::Subscribe(InspectionSubscription::all()),
        ));
        let SessionResult::Ok(InspectionResponse::Subscribed { stream_id, .. }) = response.result
        else {
            panic!("expected subscription response");
        };

        let child = ui.append(root, Panel::new());
        let event = session.replace_snapshot(DevtoolsSnapshot::capture(&ui));
        let items = session.inspection_stream_items_for_event(event);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].stream_id, stream_id);
        let InspectionStreamItem::ElementTreePatch(patch) = &items[0].payload else {
            panic!("snapshot replacement should stream an element tree patch");
        };
        assert!(patch.upserted.iter().any(|node| node.row.element == root));
        assert!(patch.upserted.iter().any(|node| node.row.element == child));
        assert!(patch.removed.is_empty());
    }

    #[cfg(feature = "spoor-session")]
    #[test]
    fn devtools_session_dispatch_emits_stream_items_for_request_events() {
        use spoor_session::{SessionRequestId, SessionResult};

        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.append(root, Panel::new());
        let mut session = DevtoolsSession::capture(&ui);

        let response = session.handle_inspection_request(LiveInspectionRequest::new(
            SessionRequestId::new(24),
            inspection_method("subscribe"),
            InspectionRequest::Subscribe(InspectionSubscription::all()),
        ));
        let SessionResult::Ok(InspectionResponse::Subscribed { stream_id, .. }) = response.result
        else {
            panic!("expected subscription response");
        };

        let dispatch = session.dispatch_inspection_request(LiveInspectionRequest::new(
            SessionRequestId::new(25),
            inspection_method("setSelection"),
            InspectionRequest::SetSelection {
                selected: Some(child),
            },
        ));

        assert_eq!(dispatch.response.request_id, SessionRequestId::new(25));
        assert_eq!(dispatch.stream_items.len(), 1);
        assert_eq!(dispatch.stream_items[0].stream_id, stream_id);
        assert_eq!(
            dispatch.stream_items[0].payload,
            InspectionStreamItem::Delta(InspectionDelta::new(
                InspectionSubsystem::Elements,
                InspectionUpdateReason::SelectionChanged,
            ))
        );
    }

    #[cfg(feature = "spoor-snapshot-session")]
    #[test]
    fn devtools_session_summarizes_spoor_envelopes() {
        use spoor_session::{
            DroppedSummary, EnvelopeHeader, OwnedSessionEnvelope, OwnedSessionPayload,
            OwnedSnapshotMetadata, OwnedSnapshotSummary, PROTOCOL_VERSION,
        };

        let mut session = DevtoolsSession::new(DevtoolsSnapshot::capture(&Ui::new()));
        session.replace_spoor_envelopes(vec![
            OwnedSessionEnvelope {
                header: EnvelopeHeader {
                    protocol_version: PROTOCOL_VERSION,
                    session_sequence: 7,
                    chunk_index: 0,
                },
                payload: OwnedSessionPayload::SnapshotStart(OwnedSnapshotSummary {
                    metadata: OwnedSnapshotMetadata {
                        process_name: String::from("overstory_demo"),
                        session_name: Some(String::from("devtools")),
                        session_sequence: 7,
                    },
                    capture_start_micros: 0,
                    captured_at_micros: 10,
                    context_count: 3,
                    counter_count: 2,
                    event_count: 1,
                }),
            },
            OwnedSessionEnvelope {
                header: EnvelopeHeader {
                    protocol_version: PROTOCOL_VERSION,
                    session_sequence: 7,
                    chunk_index: 1,
                },
                payload: OwnedSessionPayload::SnapshotEnd(DroppedSummary {
                    contexts: 1,
                    counters: 2,
                    events: 3,
                }),
            },
        ]);

        assert_eq!(
            session.spoor_summary(),
            DevtoolsSpoorSummary {
                envelope_count: 2,
                session_sequence: Some(7),
                context_count: 3,
                counter_count: 2,
                event_count: 1,
                dropped_contexts: 1,
                dropped_counters: 2,
                dropped_events: 3,
            }
        );
    }

    #[test]
    fn append_inspection_view_builds_outline_and_details() {
        let mut target = Ui::new();
        let target_root = target.root();
        let selected = target.append(target_root, Panel::new());
        let snapshot = DevtoolsSnapshot::capture(&target);

        let mut inspector = Ui::new();
        let root = inspector.root();
        let elements = append_inspection_view(&mut inspector, root, &snapshot, Some(selected));

        assert_eq!(
            inspector
                .get(elements.root)
                .and_then(overstory::Element::parent),
            Some(root)
        );
        assert_eq!(
            inspector
                .get(elements.outline)
                .and_then(overstory::Element::parent),
            Some(elements.root)
        );
        assert!(
            inspector
                .get(elements.details)
                .map(overstory::Element::children)
                .is_some_and(|children| children.len() >= 5),
            "details should render sectioned selected-element panels"
        );
    }

    #[test]
    fn selected_element_property_grid_preserves_structured_rows() {
        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.append(root, Panel::new());
        let grandchild = ui.append(child, Panel::new());
        let snapshot = DevtoolsSnapshot::capture(&ui);
        let row = snapshot
            .element(child)
            .expect("child should be inspectable");

        let grid = selected_element_property_grid(&snapshot, row);
        let tree = grid
            .sections()
            .iter()
            .find(|section| section.title() == "Hierarchy")
            .expect("grid should contain hierarchy section");
        let children = tree
            .rows()
            .iter()
            .find(|row| row.label() == "Children")
            .expect("hierarchy section should contain children row");

        assert_eq!(children.value(), &PropertyGridValue::count(1));
        assert_eq!(children.children().len(), 1);
        assert_eq!(children.children()[0].label(), "Child 0");
        assert_eq!(
            children.children()[0].value(),
            &PropertyGridValue::element_link(
                grandchild,
                format!("Panel {}", element_id_summary(grandchild))
            )
        );
    }

    #[test]
    fn selected_element_property_grid_labels_style_vocabulary_by_name_or_id() {
        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.append(root, Panel::new());
        let custom_class = ui.style_vocabulary_mut().class_id(".custom-row");
        ui.add_class(child, overstory::CLASS_SLIDER_MAJOR_TICK);
        ui.add_class(child, custom_class);
        ui.add_pseudo(child, overstory::pseudos::PSEUDO_HOVER);
        let snapshot = DevtoolsSnapshot::capture(&ui);
        let row = snapshot
            .element(child)
            .expect("child should be inspectable");

        let grid = selected_element_property_grid(&snapshot, row);
        let style = grid
            .sections()
            .iter()
            .find(|section| section.title() == "Style")
            .expect("grid should contain style section");
        let classes = style
            .rows()
            .iter()
            .find(|row| row.label() == "Classes")
            .expect("style section should contain classes row");
        let pseudos = style
            .rows()
            .iter()
            .find(|row| row.label() == "Pseudos")
            .expect("style section should contain pseudos row");

        assert_eq!(classes.children()[0].label(), ".slider-major-tick");
        assert_eq!(
            classes.children()[0].value(),
            &PropertyGridValue::token(".slider-major-tick")
        );
        assert!(classes.children()[0].children().is_empty());
        assert!(
            classes
                .children()
                .iter()
                .any(|row| row.label() == ".custom-row"
                    && row.value() == &PropertyGridValue::token(".custom-row"))
        );
        assert_eq!(pseudos.children()[0].label(), ":hover");
        assert_eq!(
            pseudos.children()[0].value(),
            &PropertyGridValue::token(":hover")
        );
    }

    #[test]
    fn selected_element_property_grid_groups_effective_properties() {
        let mut ui = Ui::new();
        let root = ui.root();
        let background = overstory::Color::from_rgba8(0x12, 0x34, 0x56, 0xff);
        let child = ui.append(
            root,
            Panel::new()
                .background(Some(background.into()))
                .padding_xy(12.0, 6.0)
                .font_size(18.0)
                .layout(Layout::row(8.0).logical_size(120.0, 64.0)),
        );
        let snapshot = DevtoolsSnapshot::capture(&ui);
        let row = snapshot
            .element(child)
            .expect("child should be inspectable");

        let grid = selected_element_property_grid(&snapshot, row);
        let surface = grid
            .sections()
            .iter()
            .find(|section| section.title() == "Appearance")
            .expect("grid should contain appearance property section");
        let background = surface
            .rows()
            .iter()
            .find(|row| row.label() == "Background")
            .expect("surface section should contain background row");
        assert_eq!(background.origin(), Some("local"));
        assert_eq!(
            background
                .origin_info()
                .expect("background row should carry origin metadata")
                .description(),
            "Value source: local"
        );
        assert_eq!(
            background.value(),
            &PropertyGridValue::color(overstory::Rgba8 {
                r: 0x12,
                g: 0x34,
                b: 0x56,
                a: 0xff,
            })
        );

        let box_model = grid
            .sections()
            .iter()
            .find(|section| section.title() == "Box model")
            .expect("grid should contain box model property section");
        assert!(
            box_model
                .rows()
                .iter()
                .any(|row| row.label() == "Padding left"
                    && row.value() == &PropertyGridValue::text("12"))
        );

        let text = grid
            .sections()
            .iter()
            .find(|section| section.title() == "Text")
            .expect("grid should contain text property section");
        assert!(
            text.rows()
                .iter()
                .any(|row| row.label() == "FontSize"
                    && row.value() == &PropertyGridValue::text("18"))
        );

        let layout = grid
            .sections()
            .iter()
            .find(|section| section.title() == "Layout inputs")
            .expect("grid should contain layout property section");
        let size = layout
            .rows()
            .iter()
            .find(|row| row.label() == "LayoutSize")
            .expect("layout section should contain layout size row");
        assert_eq!(
            size.value(),
            &PropertyGridValue::token("width 120px, height 64px")
        );
        assert!(
            size.children()
                .iter()
                .any(|row| row.label() == "Width"
                    && row.value() == &PropertyGridValue::token("120px"))
        );
        assert!(
            size.children()
                .iter()
                .any(|row| row.label() == "Height"
                    && row.value() == &PropertyGridValue::token("64px"))
        );
    }

    #[test]
    fn property_grid_renders_label_value_row_panels() {
        let mut target = Ui::new();
        let target_root = target.root();
        let selected = target.append(target_root, Panel::new());
        let snapshot = DevtoolsSnapshot::capture(&target);

        let mut inspector = Ui::new();
        let root = inspector.root();
        let elements = append_inspection_view(&mut inspector, root, &snapshot, Some(selected));
        let element_section = inspector
            .get(elements.details)
            .and_then(|details| details.children().first().copied())
            .expect("details should contain element section");
        let element_header = inspector
            .get(element_section)
            .and_then(|section| section.children().first().copied())
            .expect("element section should contain a header");
        let header_children = inspector
            .get(element_header)
            .map(overstory::Element::children)
            .expect("section header should exist");
        assert_eq!(
            header_children.len(),
            2,
            "section headers should show a title and row count"
        );
        assert_eq!(
            inspector
                .get(header_children[0])
                .and_then(|element| element.get_local_value(inspector.properties().text)),
            Some(&String::from("ELEMENT"))
        );
        assert_eq!(
            inspector
                .get(header_children[1])
                .and_then(|element| element.get_local_value(inspector.properties().text)),
            Some(&String::from("5"))
        );
        let element_row = inspector
            .get(element_section)
            .and_then(|section| section.children().get(1).copied())
            .expect("element section should contain an element id row after its header");

        let row_children = inspector
            .get(element_row)
            .map(overstory::Element::children)
            .expect("property row should exist");
        assert_eq!(row_children.len(), 2);
        assert_eq!(
            inspector
                .get(row_children[1])
                .map(overstory::Element::children)
                .map(|children| children.len()),
            Some(1),
            "property values should render inside a dedicated value cell"
        );
    }

    #[test]
    fn property_grid_renders_semantic_value_presenters() {
        let grid = PropertyGrid::new().section(
            PropertyGridSection::new("Style")
                .row(PropertyGridRow::new(
                    "Background",
                    PropertyGridValue::color_rgba8(0x2f, 0x62, 0xd6, 0xff),
                ))
                .row(PropertyGridRow::new(
                    "Pseudo",
                    PropertyGridValue::token(":hover"),
                )),
        );
        let mut inspector = Ui::new();
        let root = inspector.root();

        append_property_grid_sections(&mut inspector, root, &grid, None);

        let section = inspector.get(root).unwrap().children()[0];
        let color_row = inspector.get(section).unwrap().children()[1];
        let color_cell = inspector.get(color_row).unwrap().children()[1];
        let color_children = inspector.get(color_cell).unwrap().children();
        assert_eq!(
            color_children.len(),
            2,
            "color values should render as swatch plus text"
        );
        assert!(
            inspector
                .get(color_children[0])
                .and_then(|element| {
                    element.get_local_value(inspector.properties().surface.background)
                })
                .is_some_and(Option::is_some),
            "color swatch should carry a local background brush"
        );

        let token_row = inspector.get(section).unwrap().children()[2];
        let token_cell = inspector.get(token_row).unwrap().children()[1];
        let token_chip = inspector.get(token_cell).unwrap().children()[0];
        let token_text = inspector.get(token_chip).unwrap().children()[0];
        assert_eq!(
            inspector
                .get(token_text)
                .and_then(|element| element.get_local_value(inspector.properties().text)),
            Some(&String::from(":hover")),
            "token values should render as token chips"
        );
    }

    #[test]
    fn append_inspection_view_places_details_in_scroll_area() {
        let mut target = Ui::new();
        let target_root = target.root();
        let selected = target.append(target_root, Panel::new());
        let snapshot = DevtoolsSnapshot::capture(&target);

        let mut inspector = Ui::new();
        let root = inspector.root();
        let elements = append_inspection_view(&mut inspector, root, &snapshot, Some(selected));
        let details_frame = inspector
            .get(elements.root)
            .and_then(|root| root.children().get(1).copied())
            .expect("inspector root should contain a details scroll area");
        let viewport = scroll_area_viewport(&inspector, details_frame)
            .expect("details should be placed inside a scroll area viewport");

        assert_eq!(elements.details, viewport);
    }

    #[test]
    fn append_inspection_view_panes_fill_available_height() {
        let mut target = Ui::new();
        let target_root = target.root();
        let selected = target.append(target_root, overstory::Button::new("Deploy"));
        let snapshot = DevtoolsSnapshot::capture(&target);

        let mut inspector = Ui::new();
        let root = inspector.root();
        inspector.set_layout(root, Layout::column(0.0).logical_size(640.0, 600.0));
        let elements = append_inspection_view(&mut inspector, root, &snapshot, Some(selected));
        let details_frame = inspector
            .get(elements.root)
            .and_then(|root| root.children().get(1).copied())
            .expect("inspector root should contain a details scroll area");

        inspector.compute_layout(overstory::Size::new(640.0, 600.0));

        let expected_pane_height = 600.0 - f64::from(INSPECTOR_PADDING * 2.0);
        assert_eq!(inspector.effective_bounds(elements.root).height(), 600.0);
        assert_eq!(
            inspector.effective_bounds(elements.outline).height(),
            expected_pane_height
        );
        assert_eq!(
            inspector.effective_bounds(details_frame).height(),
            expected_pane_height
        );
        assert_eq!(
            inspector.effective_bounds(elements.details).height(),
            expected_pane_height
        );
        assert!(
            inspector.effective_bounds(elements.details).height()
                > f64::from(INSPECTOR_VIEW_HEIGHT),
            "details should fill the available inspector height, not stay fixed at the bootstrap height"
        );
    }

    #[test]
    fn details_scroll_area_accepts_scroll_offsets() {
        let mut target = Ui::new();
        let target_root = target.root();
        let selected = target.append(target_root, overstory::Button::new("Deploy"));
        let snapshot = DevtoolsSnapshot::capture(&target);

        let mut inspector = Ui::new();
        let root = inspector.root();
        inspector.set_layout(root, Layout::new().logical_size(520.0, 520.0));
        let elements = append_inspection_view(&mut inspector, root, &snapshot, Some(selected));
        let details_frame = inspector
            .get(elements.root)
            .and_then(|root| root.children().get(1).copied())
            .expect("inspector root should contain a details scroll area");
        let viewport = scroll_area_viewport(&inspector, details_frame)
            .expect("details should be placed inside a scroll area viewport");

        inspector.compute_layout(overstory::Size::new(520.0, 520.0));

        assert!(overstory::set_scroll_offset_y(
            &mut inspector,
            viewport,
            48.0
        ));
        assert!(
            overstory::scroll_offset_y(&inspector, viewport).is_some_and(|offset| offset > 0.0),
            "details scroll area should expose non-zero overflow"
        );
    }

    #[test]
    fn selected_element_property_grid_identifies_template_parts() {
        let mut ui = Ui::new();
        let root = ui.root();
        let button = ui.append(root, overstory::Button::new("Run"));
        let label = ui
            .slot(button, overstory::SLOT_BUTTON_CONTENT)
            .expect("button template should expose a content part");
        let snapshot = DevtoolsSnapshot::capture(&ui);
        let button_row = snapshot
            .element(button)
            .expect("button should be inspectable");

        let grid = selected_element_property_grid(&snapshot, button_row);
        let identity = grid
            .sections()
            .iter()
            .find(|section| section.title() == "Element")
            .expect("grid should contain element section");
        let role = identity
            .rows()
            .iter()
            .find(|row| row.label() == "Role")
            .expect("element section should expose element role");
        let type_tag = identity
            .rows()
            .iter()
            .find(|row| row.label() == "Type tag")
            .expect("element section should expose type tag");
        let hierarchy = grid
            .sections()
            .iter()
            .find(|section| section.title() == "Hierarchy")
            .expect("grid should contain hierarchy section");
        let children = hierarchy
            .rows()
            .iter()
            .find(|row| row.label() == "Children")
            .expect("hierarchy section should contain children row");
        let template = grid
            .sections()
            .iter()
            .find(|section| section.title() == "Template")
            .expect("grid should contain template section");
        let owned_parts = template
            .rows()
            .iter()
            .find(|row| row.label() == "Owned part count")
            .expect("template section should expose owned part count");
        let contract = template
            .rows()
            .iter()
            .find(|row| row.label() == "Contract")
            .expect("template section should expose template contract status");
        let required_slots = template
            .rows()
            .iter()
            .find(|row| row.label() == "Required slots")
            .expect("template section should expose required slots");

        assert_eq!(role.value(), &PropertyGridValue::text("templated control"));
        assert_eq!(type_tag.value(), &PropertyGridValue::token("Button"));
        assert!(
            type_tag.children().is_empty(),
            "main details should not expose raw type tag ids"
        );
        assert_eq!(owned_parts.value(), &PropertyGridValue::count(1));
        assert_eq!(contract.value(), &PropertyGridValue::token("satisfied"));
        assert_eq!(required_slots.value(), &PropertyGridValue::count(1));
        assert_eq!(required_slots.children()[0].label(), "content");
        assert_eq!(
            required_slots.children()[0].value(),
            &PropertyGridValue::element_link(
                label,
                format!("Button.Label {}", element_id_summary(label))
            )
        );
        assert_eq!(
            required_slots.children()[0].children()[0].value(),
            &PropertyGridValue::token("Button.Label")
        );
        assert_eq!(children.children()[0].label(), "Template part 0");
        assert_eq!(
            children.children()[0].value(),
            &PropertyGridValue::element_link(
                label,
                format!("Button.Label {}", element_id_summary(label))
            )
        );

        let label_row = snapshot
            .element(label)
            .expect("button content should be inspectable");
        let label_grid = selected_element_property_grid(&snapshot, label_row);
        let label_identity = label_grid
            .sections()
            .iter()
            .find(|section| section.title() == "Element")
            .expect("template part grid should contain element section");
        let label_role = label_identity
            .rows()
            .iter()
            .find(|row| row.label() == "Role")
            .expect("element section should expose role");
        let label_type_tag = label_identity
            .rows()
            .iter()
            .find(|row| row.label() == "Type tag")
            .expect("element section should expose type tag");
        let label_template = label_grid
            .sections()
            .iter()
            .find(|section| section.title() == "Template")
            .expect("template part grid should contain template section");
        let template_role = label_template
            .rows()
            .iter()
            .find(|row| row.label() == "Role")
            .expect("template section should expose template role");
        let physical_source = label_template
            .rows()
            .iter()
            .find(|row| row.label() == "Routed physical source")
            .expect("template part should expose routed physical source");
        let semantic_target = label_template
            .rows()
            .iter()
            .find(|row| row.label() == "Routed semantic target")
            .expect("template part should expose routed semantic target");
        let part_tag = label_template
            .rows()
            .iter()
            .find(|row| row.label() == "Style part")
            .expect("template part should expose style part");
        assert_eq!(
            label_role.value(),
            &PropertyGridValue::text("template slot part")
        );
        assert_eq!(label_type_tag.value(), &PropertyGridValue::token("Text"));
        assert_eq!(
            template_role.value(),
            &PropertyGridValue::text("template slot part")
        );
        assert_eq!(part_tag.value(), &PropertyGridValue::token("Button.Label"));
        assert_eq!(
            physical_source.value(),
            &PropertyGridValue::element_link(
                label,
                format!("Button.Label {}", element_id_summary(label))
            )
        );
        assert_eq!(
            semantic_target.value(),
            &PropertyGridValue::element_link(
                button,
                format!("Button {}", element_id_summary(button))
            )
        );
    }

    #[test]
    fn selected_element_property_grid_reports_template_contract_diagnostics() {
        use overstory::{
            Button, SLOT_BUTTON_CONTENT, Template, TemplateChild, TemplateContext, TextWidget,
        };

        struct DuplicateButtonTemplate;
        impl Template for DuplicateButtonTemplate {
            fn instantiate(&self, cx: &mut TemplateContext<'_>) {
                let _first = cx.insert(TemplateChild::slot(
                    SLOT_BUTTON_CONTENT,
                    alloc::boxed::Box::new(TextWidget),
                ));
                let _duplicate = cx.insert(TemplateChild::slot(
                    SLOT_BUTTON_CONTENT,
                    alloc::boxed::Box::new(TextWidget),
                ));
            }
        }
        static DUPLICATE_TEMPLATE: DuplicateButtonTemplate = DuplicateButtonTemplate;

        let mut ui = Ui::new();
        let root = ui.root();
        let button = ui.append(root, Button::new("Duplicate").template(&DUPLICATE_TEMPLATE));
        let snapshot = DevtoolsSnapshot::capture(&ui);
        let row = snapshot
            .element(button)
            .expect("button should be inspectable");

        let grid = selected_element_property_grid(&snapshot, row);
        let template = grid
            .sections()
            .iter()
            .find(|section| section.title() == "Template")
            .expect("grid should contain template section");
        let contract = template
            .rows()
            .iter()
            .find(|row| row.label() == "Contract")
            .expect("template section should expose template contract status");
        let diagnostics = template
            .rows()
            .iter()
            .find(|row| row.label() == "Diagnostics")
            .expect("template section should expose diagnostics");

        assert_eq!(contract.value(), &PropertyGridValue::token("diagnostics"));
        assert_eq!(diagnostics.value(), &PropertyGridValue::count(2));
        assert!(diagnostics.children().iter().any(|row| {
            row.label() == "Duplicate slot" && row.value() == &PropertyGridValue::token("content")
        }));
        assert!(diagnostics.children().iter().any(|row| {
            row.label() == "Duplicate style part"
                && row.value() == &PropertyGridValue::token("Button.Label")
        }));
    }

    #[test]
    fn element_outline_labels_surface_template_parts_by_part_name() {
        let mut ui = Ui::new();
        let root = ui.root();
        let slider = ui.append(root, overstory::Slider::new());
        let track = ui
            .slot(slider, overstory::SLOT_SLIDER_TRACK)
            .expect("slider template should expose a track part");
        let snapshot = DevtoolsSnapshot::capture(&ui);

        let track_key = snapshot
            .outline_key_for_element(track)
            .expect("track should have an outline key");
        let track_node = snapshot
            .element_outline_nodes()
            .get(track_key.index())
            .expect("track outline node should exist")
            .clone();

        assert_eq!(
            track_node.label(),
            format!("Slider.Track {}", element_id_summary(track))
        );

        let slider_row = snapshot
            .element(slider)
            .expect("slider should be inspectable");
        let grid = selected_element_property_grid(&snapshot, slider_row);
        let template = grid
            .sections()
            .iter()
            .find(|section| section.title() == "Template")
            .expect("grid should contain template section");
        let recommended_slots = template
            .rows()
            .iter()
            .find(|row| row.label() == "Recommended slots")
            .expect("slider should expose recommended template slots");
        let optional_slots = template
            .rows()
            .iter()
            .find(|row| row.label() == "Optional slots")
            .expect("slider should expose optional template slots");
        let track_slot = recommended_slots
            .children()
            .iter()
            .find(|row| row.label() == "slider-track")
            .expect("slider track slot should be listed");
        assert_eq!(
            track_slot.value(),
            &PropertyGridValue::element_link(
                track,
                format!("Slider.Track {}", element_id_summary(track))
            )
        );
        assert_eq!(optional_slots.value(), &PropertyGridValue::count(3));
        assert!(
            optional_slots
                .children()
                .iter()
                .any(|row| row.label() == "slider-axis")
        );

        let track_row = snapshot
            .element(track)
            .expect("track should be inspectable");
        let track_grid = selected_element_property_grid(&snapshot, track_row);
        let track_identity = track_grid
            .sections()
            .iter()
            .find(|section| section.title() == "Element")
            .expect("track details should contain element section");
        let track_role = track_identity
            .rows()
            .iter()
            .find(|row| row.label() == "Role")
            .expect("track element details should expose role");
        let track_type_tag = track_identity
            .rows()
            .iter()
            .find(|row| row.label() == "Type tag")
            .expect("track element details should expose type tag");
        let track_template = track_grid
            .sections()
            .iter()
            .find(|section| section.title() == "Template")
            .expect("track details should contain template section");
        let track_style_part = track_template
            .rows()
            .iter()
            .find(|row| row.label() == "Style part")
            .expect("track template details should expose style part");
        let track_slot_name = track_template
            .rows()
            .iter()
            .find(|row| row.label() == "Slot")
            .expect("track template details should expose slot name");
        assert_eq!(
            track_role.value(),
            &PropertyGridValue::text("template slot part")
        );
        assert_eq!(track_type_tag.value(), &PropertyGridValue::empty());
        assert_eq!(
            track_style_part.value(),
            &PropertyGridValue::token("Slider.Track")
        );
        assert_eq!(
            track_slot_name.value(),
            &PropertyGridValue::token("slider-track")
        );
    }

    #[test]
    fn selected_element_property_grid_reports_degraded_optional_template_slots() {
        struct MinimalSliderTemplate;
        impl overstory::Template for MinimalSliderTemplate {
            fn instantiate(&self, cx: &mut overstory::TemplateContext<'_>) {
                let _thumb = cx.insert(overstory::TemplateChild::surface_slot(
                    overstory::SLOT_SLIDER_THUMB,
                ));
            }
        }

        static MINIMAL_SLIDER_TEMPLATE: MinimalSliderTemplate = MinimalSliderTemplate;

        let mut ui = Ui::new();
        let root = ui.root();
        let slider = ui.append(
            root,
            overstory::Slider::new().template(&MINIMAL_SLIDER_TEMPLATE),
        );
        let thumb = ui
            .slot(slider, overstory::SLOT_SLIDER_THUMB)
            .expect("minimal template should expose slider thumb");
        let snapshot = DevtoolsSnapshot::capture(&ui);
        let slider_row = snapshot
            .element(slider)
            .expect("slider should be inspectable");
        let grid = selected_element_property_grid(&snapshot, slider_row);
        let template = grid
            .sections()
            .iter()
            .find(|section| section.title() == "Template")
            .expect("grid should contain template section");
        let contract = template
            .rows()
            .iter()
            .find(|row| row.label() == "Contract")
            .expect("template section should expose template contract status");
        let required_slots = template
            .rows()
            .iter()
            .find(|row| row.label() == "Required slots")
            .expect("slider should expose required slots");
        let optional_slots = template
            .rows()
            .iter()
            .find(|row| row.label() == "Optional slots")
            .expect("slider should expose optional slots");
        let recommended_slots = template
            .rows()
            .iter()
            .find(|row| row.label() == "Recommended slots")
            .expect("slider should expose recommended slots");

        assert_eq!(contract.value(), &PropertyGridValue::token("degraded"));
        assert_eq!(required_slots.value(), &PropertyGridValue::count(1));
        assert_eq!(
            required_slots.children()[0].value(),
            &PropertyGridValue::element_link(
                thumb,
                format!("Slider.Thumb {}", element_id_summary(thumb))
            )
        );
        assert_eq!(optional_slots.value(), &PropertyGridValue::count(3));
        assert!(
            optional_slots
                .children()
                .iter()
                .all(|row| row.value() == &PropertyGridValue::token("not present"))
        );
        assert_eq!(recommended_slots.value(), &PropertyGridValue::count(1));
        assert_eq!(
            recommended_slots.children()[0].value(),
            &PropertyGridValue::token("missing")
        );
    }

    #[test]
    fn append_inspection_view_updates_details_when_outline_selection_changes() {
        let mut target = Ui::new();
        let target_root = target.root();
        let child = target.append(target_root, Panel::new());
        let grandchild = target.append(child, Panel::new());
        let snapshot = DevtoolsSnapshot::capture(&target);

        let mut inspector = Ui::new();
        let root = inspector.root();
        let elements = append_inspection_view(&mut inspector, root, &snapshot, Some(child));
        let child_key = snapshot
            .outline_key_for_element(child)
            .expect("child should have an outline key");
        let grandchild_key = snapshot
            .outline_key_for_element(grandchild)
            .expect("grandchild should have an outline key");
        let first_detail_before = inspector
            .get(elements.details)
            .and_then(|element| element.children().first().copied())
            .expect("details should contain selected element text");

        assert_eq!(
            overstory::selected_outline_key(&inspector, elements.outline),
            Some(child_key)
        );
        assert!(overstory::select_outline_key(
            &mut inspector,
            elements.outline,
            grandchild_key
        ));

        let detail_children = inspector
            .get(elements.details)
            .map(overstory::Element::children)
            .expect("details panel should still exist");
        assert!(detail_children.len() >= 5);
        assert_ne!(detail_children[0], first_detail_before);
        assert_eq!(
            overstory::selected_outline_key(&inspector, elements.outline),
            Some(grandchild_key)
        );
    }

    #[test]
    fn append_session_view_updates_session_selection() {
        let mut target = Ui::new();
        let target_root = target.root();
        let child = target.append(target_root, Panel::new());
        let grandchild = target.append(child, Panel::new());
        let snapshot = DevtoolsSnapshot::capture(&target);
        let grandchild_key = snapshot
            .outline_key_for_element(grandchild)
            .expect("grandchild should have an outline key");
        let session = DevtoolsSessionHandle::new(
            DevtoolsSession::new(snapshot).with_selected_element(Some(child)),
        );

        let mut inspector = Ui::new();
        let root = inspector.root();
        let elements = append_session_view(&mut inspector, root, session.clone());

        assert!(overstory::select_outline_key(
            &mut inspector,
            elements.outline,
            grandchild_key
        ));
        assert_eq!(
            session.with(DevtoolsSession::selected_element),
            Some(grandchild)
        );
    }

    #[test]
    fn refresh_session_view_rebuilds_under_same_root() {
        let mut target = Ui::new();
        let target_root = target.root();
        let child = target.append(target_root, Panel::new());
        let session = DevtoolsSessionHandle::new(
            DevtoolsSession::capture(&target).with_selected_element(Some(child)),
        );

        let mut inspector = Ui::new();
        let root = inspector.root();
        let elements = append_session_view(&mut inspector, root, session.clone());
        let old_outline = elements.outline;
        let old_details = elements.details;

        target.append(target_root, Panel::new());
        session.with_mut(|session| {
            let _ = session.replace_snapshot(DevtoolsSnapshot::capture(&target));
        });
        let refreshed = refresh_session_view(&mut inspector, elements, session.clone());

        assert_eq!(refreshed.root, elements.root);
        assert!(inspector.get(refreshed.outline).is_some());
        assert!(inspector.get(refreshed.details).is_some());
        assert!(
            inspector.get(old_outline).is_none(),
            "refresh should remove the old outline subtree"
        );
        assert!(
            inspector.get(old_details).is_none(),
            "refresh should remove the old details subtree"
        );
    }

    #[cfg(feature = "spoor-session")]
    #[test]
    fn live_devtools_refresh_preserves_rendered_session_view_state() {
        let mut target = Ui::new();
        let target_root = target.root();
        let group = target.append(target_root, Panel::new());
        for _ in 0..30 {
            target.append(group, Panel::new());
        }
        for _ in 0..30 {
            target.append(target_root, Panel::new());
        }
        let mut devtools =
            LiveDevtools::capture(&target, Some(group)).expect("devtools should attach");
        let session = devtools.session().clone();

        let mut inspector = Ui::new();
        let root = inspector.root();
        let elements = append_session_view(&mut inspector, root, session.clone());
        let old_outline = elements.outline;
        let old_details = elements.details;
        let previous_tree = devtools.mirror().tree().clone();
        let group_key = previous_tree
            .outline_key_for_element(group)
            .expect("group should have an outline key");
        assert!(overstory::set_outline_key_expanded(
            &mut inspector,
            elements.outline,
            group_key,
            false
        ));
        assert!(overstory::set_outline_scroll_offset_y(
            &mut inspector,
            elements.outline,
            120.0
        ));

        target.append(target_root, Panel::new());
        let refresh = devtools
            .refresh_from(&target)
            .expect("target refresh should update live session");
        let current_tree = devtools.mirror().tree().clone();
        let refreshed = refresh_session_view_from_mirror(
            &mut inspector,
            elements,
            &previous_tree,
            devtools.mirror(),
            session,
        );

        assert_eq!(refresh.stream_item_count, 1);
        assert_eq!(refreshed.outline, old_outline);
        assert_eq!(refreshed.details, old_details);
        assert!(inspector.get(refreshed.outline).is_some());
        assert!(inspector.get(refreshed.details).is_some());
        let refreshed_group_key = current_tree
            .outline_key_for_element(group)
            .expect("group should keep an outline key");
        assert_eq!(
            overstory::outline_key_expanded(&inspector, refreshed.outline, refreshed_group_key),
            Some(false)
        );
        assert_eq!(
            overstory::outline_scroll_offset_y(&inspector, refreshed.outline),
            Some(120.0)
        );
        assert_eq!(
            overstory::selected_outline_key(&inspector, refreshed.outline),
            devtools.mirror().outline_key_for_element(group)
        );
    }

    #[test]
    fn selected_element_relationship_rows_select_elements() {
        let mut target = Ui::new();
        let target_root = target.root();
        let child = target.append(target_root, Panel::new());
        let snapshot = DevtoolsSnapshot::capture(&target);
        let root_key = snapshot
            .outline_key_for_element(target_root)
            .expect("root should have an outline key");
        let session = DevtoolsSessionHandle::new(
            DevtoolsSession::new(snapshot).with_selected_element(Some(child)),
        );

        let mut inspector = Ui::new();
        let root = inspector.root();
        let elements = append_session_view(&mut inspector, root, session.clone());
        let tree_section = inspector
            .get(elements.details)
            .and_then(|details| details.children().get(1).copied())
            .expect("details should contain a tree section");
        let parent_row = inspector
            .get(tree_section)
            .and_then(|tree| tree.children().get(1).copied())
            .expect("tree section should contain a parent relationship row");

        inspector.synthesize_click(parent_row, overstory::Rect::ZERO.center());

        assert_eq!(
            session.with(DevtoolsSession::selected_element),
            Some(target_root)
        );
        assert_eq!(
            overstory::selected_outline_key(&inspector, elements.outline),
            Some(root_key)
        );
    }
}
