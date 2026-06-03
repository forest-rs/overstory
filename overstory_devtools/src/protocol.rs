// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Overstory inspection payloads carried by Spoor live-session messages.
//!
//! Spoor owns request ids, stream ids, and generic response/stream envelopes.
//! This module owns the Overstory-specific payloads that talk about element
//! trees, property details, selection, and inspection deltas.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::cmp::Ordering;

use overstory::UiFrameStats;
use overstory::inspection::{
    CompositionInspectionSnapshot, ElementInspectionRow, TimelineInspectionSnapshot,
    UiStorageInspectionSnapshot,
};
use overstory::{ElementId, OutlineNodeKey, OutlineViewNode};
use spoor_session::{
    SessionMethod, SessionRequest, SessionRequestId, SessionResponse, SessionStreamId,
    SessionStreamItem, SessionStreamStart,
};

use crate::{DevtoolsSnapshot, ElementOutlineRow, PropertyGrid};

/// Domain used for Overstory inspection live-session methods.
pub const INSPECTION_DOMAIN: &str = "overstory.inspection";

/// Builds a Spoor live-session method in the Overstory inspection domain.
#[must_use]
pub fn inspection_method(name: impl Into<String>) -> SessionMethod {
    SessionMethod::new(INSPECTION_DOMAIN, name)
}

/// Builds the Spoor stream-start row for an accepted inspection subscription.
#[must_use]
pub fn inspection_stream_start(
    request_id: Option<SessionRequestId>,
    stream_id: SessionStreamId,
    subscription: InspectionSubscription,
) -> LiveInspectionStreamStart {
    SessionStreamStart::new(
        request_id,
        stream_id,
        inspection_method("updates"),
        InspectionResponse::Subscribed {
            stream_id,
            subscription,
        },
    )
}

/// Spoor live-session request carrying an Overstory inspection payload.
pub type LiveInspectionRequest = SessionRequest<InspectionRequest>;

/// Spoor live-session response carrying an Overstory inspection payload.
pub type LiveInspectionResponse = SessionResponse<InspectionResponse>;

/// Spoor live-session stream-start row carrying an Overstory inspection payload.
pub type LiveInspectionStreamStart = SessionStreamStart<InspectionResponse>;

/// Spoor live-session stream item carrying an Overstory inspection payload.
pub type LiveInspectionStreamItem = SessionStreamItem<InspectionStreamItem>;

/// Same-process result of dispatching one live inspection request.
///
/// The response completes the request. Stream items carry any deltas emitted by
/// the request, such as selection changes that should refresh subscribed
/// mirrors.
#[derive(Clone, Debug, PartialEq)]
pub struct InspectionDispatch {
    /// Response to the dispatched request.
    pub response: LiveInspectionResponse,
    /// Stream items emitted while handling the request.
    pub stream_items: Vec<LiveInspectionStreamItem>,
}

impl InspectionDispatch {
    /// Creates a dispatch result.
    #[must_use]
    pub fn new(
        response: LiveInspectionResponse,
        stream_items: Vec<LiveInspectionStreamItem>,
    ) -> Self {
        Self {
            response,
            stream_items,
        }
    }
}

/// Request payloads understood by the Overstory inspector domain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InspectionRequest {
    /// Capture a full owned snapshot.
    ///
    /// This is useful for initial attachment and offline dumps. Incremental
    /// inspector refresh should prefer compact change signals plus lazy detail
    /// requests.
    CaptureSnapshot,
    /// Return the retained element tree as a compact projected outline.
    ElementTree,
    /// Return details for one element.
    ElementDetails {
        /// Element to inspect.
        element: ElementId,
    },
    /// Return the currently selected element.
    Selection,
    /// Return compact subsystem summaries.
    Summaries,
    /// Set the currently selected element.
    SetSelection {
        /// Element to select, or `None` to clear selection.
        selected: Option<ElementId>,
    },
    /// Subscribe to a live inspection stream.
    Subscribe(InspectionSubscription),
}

/// Response payloads produced by the Overstory inspector domain.
#[derive(Clone, Debug, PartialEq)]
pub enum InspectionResponse {
    /// Full owned snapshot response.
    Snapshot(Box<DevtoolsSnapshot>),
    /// Compact retained element outline response.
    ElementTree(InspectionElementTree),
    /// Details for one selected element.
    ElementDetails(Box<InspectionElementDetails>),
    /// Current selection response.
    Selection {
        /// Selected element, if any.
        selected: Option<ElementId>,
    },
    /// Compact subsystem summary response.
    Summaries(Box<InspectionSummaries>),
    /// A stream was accepted by the inspector domain.
    Subscribed {
        /// Stream opened for this subscription.
        stream_id: SessionStreamId,
        /// Accepted subscription.
        subscription: InspectionSubscription,
    },
}

/// Compact subsystem summaries for diagnostics panels.
#[derive(Clone, Debug, PartialEq)]
pub struct InspectionSummaries {
    /// Composition layer, surface, and change counters.
    pub composition: CompositionInspectionSnapshot,
    /// Storage and cache size counters.
    pub storage: UiStorageInspectionSnapshot,
    /// Timeline runtime counters.
    pub timeline: TimelineInspectionSnapshot,
    /// Frame counters captured with the source snapshot.
    pub frame: UiFrameStats,
}

impl InspectionSummaries {
    /// Builds summaries from `snapshot`.
    #[must_use]
    pub fn from_snapshot(snapshot: &DevtoolsSnapshot) -> Self {
        let inspection = snapshot.inspection();
        Self {
            composition: inspection.composition.clone(),
            storage: inspection.storage,
            timeline: inspection.timeline,
            frame: inspection.frame,
        }
    }
}

/// Projected retained-element tree returned by the live inspection protocol.
///
/// Initial tree responses use the projected node order as [`OutlineNodeKey`]
/// values. Live updates should preserve state by mapping keys back to
/// [`ElementId`] before applying an [`InspectionElementTreePatch`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectionElementTree {
    /// Root node index, if the snapshot contains one.
    pub root: Option<usize>,
    /// Projected nodes in outline-key order.
    pub nodes: Vec<InspectionElementNode>,
}

impl InspectionElementTree {
    /// Creates an empty projected element tree.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            root: None,
            nodes: Vec::new(),
        }
    }

    /// Returns whether this tree contains no nodes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Returns the element id represented by the root node.
    #[must_use]
    pub fn root_element(&self) -> Option<ElementId> {
        self.root
            .and_then(|index| self.nodes.get(index))
            .map(|node| node.row.element)
    }

    /// Returns the outline key for `element`.
    #[must_use]
    pub fn outline_key_for_element(&self, element: ElementId) -> Option<OutlineNodeKey> {
        self.nodes
            .iter()
            .position(|node| node.row.element == element)
            .map(OutlineNodeKey::from)
    }

    /// Returns the element represented by `key`.
    #[must_use]
    pub fn element_for_outline_key(&self, key: OutlineNodeKey) -> Option<ElementId> {
        self.nodes.get(key.index()).map(|node| node.row.element)
    }

    /// Returns the root outline key, if this tree has a root node.
    #[must_use]
    pub fn root_key(&self) -> Option<OutlineNodeKey> {
        self.root.map(OutlineNodeKey::from)
    }

    /// Projects this tree into nodes accepted by Overstory's outline control.
    #[must_use]
    pub fn outline_nodes(&self) -> Vec<OutlineViewNode> {
        self.nodes
            .iter()
            .map(|node| {
                OutlineViewNode::new(node.label.clone())
                    .with_first_child(node.first_child)
                    .with_next_sibling(node.next_sibling)
            })
            .collect()
    }

    /// Builds a keyed patch that turns this tree into `next`.
    #[must_use]
    pub fn patch_to(&self, next: &Self) -> InspectionElementTreePatch {
        let previous_index = sorted_node_index(self);
        let next_index = sorted_node_index(next);
        let mut removed = Vec::new();
        let mut upserted = Vec::new();
        let mut previous = 0;
        let mut current = 0;

        while previous < previous_index.len() || current < next_index.len() {
            match (previous_index.get(previous), next_index.get(current)) {
                (Some((previous_id, previous_node)), Some((current_id, current_node))) => {
                    match previous_id.cmp(current_id) {
                        Ordering::Less => {
                            removed.push(*previous_id);
                            previous += 1;
                        }
                        Ordering::Greater => {
                            upserted.push(next.node_update_at(*current_node));
                            current += 1;
                        }
                        Ordering::Equal => {
                            let previous_update = self.node_update_at(*previous_node);
                            let current_update = next.node_update_at(*current_node);
                            if previous_update != current_update {
                                upserted.push(current_update);
                            }
                            previous += 1;
                            current += 1;
                        }
                    }
                }
                (Some((previous_id, _)), None) => {
                    removed.push(*previous_id);
                    previous += 1;
                }
                (None, Some((_, current_node))) => {
                    upserted.push(next.node_update_at(*current_node));
                    current += 1;
                }
                (None, None) => break,
            }
        }

        InspectionElementTreePatch {
            root: next.root_element(),
            removed,
            upserted,
        }
    }

    /// Applies a keyed tree patch in place.
    pub fn apply_patch(&mut self, patch: &InspectionElementTreePatch) {
        let mut updates = self.node_updates();
        let mut removed = patch.removed.clone();
        removed.sort();
        updates.retain(|node| removed.binary_search(&node.row.element).is_err());
        for update in &patch.upserted {
            if let Some(existing) = updates
                .iter_mut()
                .find(|node| node.row.element == update.row.element)
            {
                *existing = update.clone();
            } else {
                updates.push(update.clone());
            }
        }
        *self = Self::from_node_updates(patch.root, updates);
    }

    fn node_updates(&self) -> Vec<InspectionElementNodeUpdate> {
        (0..self.nodes.len())
            .map(|index| self.node_update_at(index))
            .collect()
    }

    fn node_update_at(&self, index: usize) -> InspectionElementNodeUpdate {
        let node = &self.nodes[index];
        InspectionElementNodeUpdate {
            row: node.row,
            label: node.label.clone(),
            first_child: node.first_child.and_then(|child| self.node_element(child)),
            next_sibling: node
                .next_sibling
                .and_then(|sibling| self.node_element(sibling)),
        }
    }

    fn node_element(&self, index: usize) -> Option<ElementId> {
        self.nodes.get(index).map(|node| node.row.element)
    }

    fn from_node_updates(
        root: Option<ElementId>,
        updates: Vec<InspectionElementNodeUpdate>,
    ) -> Self {
        let index = sorted_update_index(&updates);
        let nodes = updates
            .into_iter()
            .map(|update| InspectionElementNode {
                row: update.row,
                label: update.label,
                first_child: update
                    .first_child
                    .and_then(|element| update_index_for(&index, element)),
                next_sibling: update
                    .next_sibling
                    .and_then(|element| update_index_for(&index, element)),
            })
            .collect();

        Self {
            root: root.and_then(|element| update_index_for(&index, element)),
            nodes,
        }
    }
}

/// One projected retained-element node in an [`InspectionElementTree`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectionElementNode {
    /// Compact inspection row for the element.
    pub row: ElementOutlineRow,
    /// Display label for the element.
    pub label: String,
    /// First child node index.
    pub first_child: Option<usize>,
    /// Next sibling node index.
    pub next_sibling: Option<usize>,
}

/// Keyed update for one retained-element tree node.
///
/// Links use [`ElementId`] rather than outline indices so the update can
/// survive insertions and removals that shift a rendered outline's row keys.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectionElementNodeUpdate {
    /// Compact inspection row for the element.
    pub row: ElementOutlineRow,
    /// Display label for the element.
    pub label: String,
    /// First child element id.
    pub first_child: Option<ElementId>,
    /// Next sibling element id.
    pub next_sibling: Option<ElementId>,
}

/// Incremental retained-element tree update.
///
/// The patch carries enough keyed structure to update a client-side
/// [`InspectionElementTree`] without transferring a full [`DevtoolsSnapshot`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectionElementTreePatch {
    /// Root element after the patch.
    pub root: Option<ElementId>,
    /// Elements removed from the projected tree.
    pub removed: Vec<ElementId>,
    /// Added or changed node states.
    pub upserted: Vec<InspectionElementNodeUpdate>,
}

impl DevtoolsSnapshot {
    /// Projects this snapshot into the live inspection element tree payload.
    #[must_use]
    pub fn inspection_element_tree(&self) -> InspectionElementTree {
        let nodes = self
            .inspection()
            .elements
            .iter()
            .map(|row| InspectionElementNode {
                row: ElementOutlineRow {
                    element: row.id,
                    parent: row.parent,
                    depth: element_depth(self, row),
                    child_count: row.children.len(),
                    has_widget_identity: row.has_widget_identity,
                    layer: row.layer,
                    surface: row.surface,
                },
                label: crate::element_label(self, row),
                first_child: row
                    .children
                    .first()
                    .and_then(|child| self.element_offset(*child)),
                next_sibling: self.next_sibling_offset(row),
            })
            .collect();

        InspectionElementTree {
            root: self.element_offset(self.inspection().root),
            nodes,
        }
    }
}

fn element_depth(snapshot: &DevtoolsSnapshot, row: &ElementInspectionRow) -> usize {
    let mut depth = 0;
    let mut parent = row.parent;
    while let Some(parent_id) = parent {
        depth += 1;
        parent = snapshot.element(parent_id).and_then(|row| row.parent);
    }
    depth
}

/// Element-specific details returned by a lazy inspection query.
#[derive(Clone, Debug, PartialEq)]
pub struct InspectionElementDetails {
    /// Snapshot row for the inspected element.
    pub row: ElementInspectionRow,
    /// Structured property grid for the inspected element.
    pub properties: PropertyGrid,
}

/// Live inspection stream item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InspectionStreamItem {
    /// The target changed since the last item.
    Delta(InspectionDelta),
    /// Retained element tree changed.
    ElementTreePatch(InspectionElementTreePatch),
}

impl InspectionStreamItem {
    pub(crate) const fn subsystem(&self) -> InspectionSubsystem {
        match self {
            Self::Delta(delta) => delta.subsystem,
            Self::ElementTreePatch(_) => InspectionSubsystem::Elements,
        }
    }
}

/// Subscription request for an Overstory inspection stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InspectionSubscription {
    /// Kind of inspection updates requested.
    pub kind: InspectionSubscriptionKind,
}

fn sorted_node_index(tree: &InspectionElementTree) -> Vec<(ElementId, usize)> {
    let mut index = tree
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.row.element, index))
        .collect::<Vec<_>>();
    index.sort_by_key(|(element, _)| *element);
    index
}

fn sorted_update_index(updates: &[InspectionElementNodeUpdate]) -> Vec<(ElementId, usize)> {
    let mut index = updates
        .iter()
        .enumerate()
        .map(|(index, update)| (update.row.element, index))
        .collect::<Vec<_>>();
    index.sort_by_key(|(element, _)| *element);
    index
}

fn update_index_for(index: &[(ElementId, usize)], element: ElementId) -> Option<usize> {
    index
        .binary_search_by_key(&element, |(candidate, _)| *candidate)
        .ok()
        .map(|offset| index[offset].1)
}

impl InspectionSubscription {
    /// Subscribes to all inspection domains.
    #[must_use]
    pub const fn all() -> Self {
        Self {
            kind: InspectionSubscriptionKind::All,
        }
    }

    /// Subscribes to one inspection subsystem.
    #[must_use]
    pub const fn subsystem(subsystem: InspectionSubsystem) -> Self {
        Self {
            kind: InspectionSubscriptionKind::Subsystem(subsystem),
        }
    }
}

/// Scope of an inspection stream subscription.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectionSubscriptionKind {
    /// All inspection changes.
    All,
    /// Changes for one subsystem.
    Subsystem(InspectionSubsystem),
}

/// Compact inspection change signal.
///
/// This deliberately avoids carrying a full snapshot. Consumers should use the
/// signal to invalidate retained mirror state and issue lazy queries for the
/// tree, summaries, or details they actually display.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InspectionDelta {
    /// Subsystem that changed.
    pub subsystem: InspectionSubsystem,
    /// Reason the stream item was emitted.
    pub reason: InspectionUpdateReason,
}

impl InspectionDelta {
    /// Creates a compact inspection delta.
    #[must_use]
    pub const fn new(subsystem: InspectionSubsystem, reason: InspectionUpdateReason) -> Self {
        Self { subsystem, reason }
    }
}

/// Inspectable subsystem that can produce updates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectionSubsystem {
    /// Retained element tree topology or element identity.
    Elements,
    /// Layout bounds or layout-derived geometry.
    Layout,
    /// Style state, classes, pseudos, or resolved style outputs.
    Style,
    /// Composition layers, surfaces, clips, opacity, or transforms.
    Composition,
    /// Property binding graph or binding results.
    Bindings,
    /// Timeline tracks, clips, or sampled presentation values.
    Timeline,
    /// Frame counters and latency summaries.
    Frame,
}

/// Reason an inspection update was emitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectionUpdateReason {
    /// Initial stream state.
    Initial,
    /// A full snapshot was replaced.
    SnapshotReplaced,
    /// User or tool selection changed.
    SelectionChanged,
    /// Target state changed and lazy details may be stale.
    TargetChanged,
}

#[cfg(test)]
mod tests {
    use super::{
        INSPECTION_DOMAIN, InspectionDelta, InspectionDispatch, InspectionRequest,
        InspectionResponse, InspectionSubscription, InspectionSubsystem, InspectionUpdateReason,
        LiveInspectionRequest, inspection_method, inspection_stream_start,
    };
    use crate::DevtoolsSnapshot;
    use overstory::{Panel, Ui};
    use spoor_session::{SessionRequestId, SessionStreamId};

    #[test]
    fn inspection_method_uses_overstory_domain() {
        let method = inspection_method("elementDetails");

        assert_eq!(method.domain, INSPECTION_DOMAIN);
        assert_eq!(method.name, "elementDetails");
    }

    #[test]
    fn live_inspection_request_preserves_spoor_request_id() {
        let request = LiveInspectionRequest::new(
            SessionRequestId::new(12),
            inspection_method("snapshot"),
            InspectionRequest::CaptureSnapshot,
        );

        assert_eq!(request.id, SessionRequestId::new(12));
        assert_eq!(request.payload, InspectionRequest::CaptureSnapshot);
    }

    #[test]
    fn inspection_delta_is_compact() {
        let delta = InspectionDelta::new(
            InspectionSubsystem::Layout,
            InspectionUpdateReason::TargetChanged,
        );

        assert_eq!(delta.subsystem, InspectionSubsystem::Layout);
        assert_eq!(delta.reason, InspectionUpdateReason::TargetChanged);
    }

    #[test]
    fn inspection_dispatch_carries_response_and_stream_items() {
        let response = spoor_session::SessionResponse::ok(
            SessionRequestId::new(2),
            InspectionResponse::Selection { selected: None },
        );
        let item = spoor_session::SessionStreamItem::new(
            SessionStreamId::new(3),
            0,
            super::InspectionStreamItem::Delta(InspectionDelta::new(
                InspectionSubsystem::Elements,
                InspectionUpdateReason::SelectionChanged,
            )),
        );

        let dispatch = InspectionDispatch::new(response.clone(), alloc::vec![item.clone()]);

        assert_eq!(dispatch.response, response);
        assert_eq!(dispatch.stream_items, alloc::vec![item]);
    }

    #[test]
    fn projected_element_tree_carries_outline_links_and_labels() {
        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.append(root, Panel::new());
        let snapshot = DevtoolsSnapshot::capture(&ui);
        let tree = snapshot.inspection_element_tree();

        assert_eq!(tree.root_element(), Some(root));
        assert_eq!(tree.nodes.len(), 2);
        assert_eq!(tree.nodes[0].row.child_count, 1);
        assert_eq!(tree.nodes[0].first_child, Some(1));
        assert_eq!(tree.nodes[1].row.element, child);
        assert!(tree.nodes[1].label.starts_with("Panel "));
        assert_eq!(
            tree.element_for_outline_key(
                tree.outline_key_for_element(child)
                    .expect("child should have an outline key"),
            ),
            Some(child),
        );
    }

    #[test]
    fn element_tree_patch_updates_keyed_projection_without_snapshot() {
        let mut ui = Ui::new();
        let root = ui.root();
        let removed = ui.append(root, Panel::new());
        let kept = ui.append(root, Panel::new());
        let mut previous = DevtoolsSnapshot::capture(&ui).inspection_element_tree();

        ui.remove(removed);
        let added = ui.append(root, Panel::new());
        let next = DevtoolsSnapshot::capture(&ui).inspection_element_tree();
        let patch = previous.patch_to(&next);

        assert_eq!(patch.root, Some(root));
        assert_eq!(patch.removed, alloc::vec![removed]);
        assert!(patch.upserted.iter().any(|node| node.row.element == root));
        assert!(patch.upserted.iter().any(|node| node.row.element == added));

        previous.apply_patch(&patch);

        assert_eq!(previous, next);
        assert!(previous.outline_key_for_element(removed).is_none());
        assert!(previous.outline_key_for_element(kept).is_some());
        assert!(previous.outline_key_for_element(added).is_some());
    }

    #[test]
    fn inspection_stream_start_uses_spoor_stream_correlation() {
        let request_id = SessionRequestId::new(4);
        let stream_id = SessionStreamId::new(8);
        let subscription = InspectionSubscription::all();

        let start = inspection_stream_start(Some(request_id), stream_id, subscription);

        assert_eq!(start.request_id, Some(request_id));
        assert_eq!(start.stream_id, stream_id);
        assert_eq!(start.method.domain, INSPECTION_DOMAIN);
        assert_eq!(start.method.name, "updates");
        assert_eq!(
            start.payload,
            InspectionResponse::Subscribed {
                stream_id,
                subscription,
            }
        );
    }
}
