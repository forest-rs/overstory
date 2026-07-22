// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Same-process devtools session state.

use core::cell::RefCell;

#[cfg(feature = "spoor-session")]
use alloc::boxed::Box;
use alloc::rc::Rc;
#[cfg(any(feature = "spoor-session", feature = "spoor-snapshot-session"))]
use alloc::vec::Vec;
#[cfg(feature = "spoor-session")]
use overstory::inspection::UiInspectionChanges;
use overstory::{ElementId, OutlineNodeKey};
#[cfg(feature = "spoor-snapshot-session")]
use spoor_session::{OwnedSessionEnvelope, OwnedSessionPayload};
#[cfg(feature = "spoor-session")]
use spoor_session::{
    SessionError, SessionErrorCode, SessionResponse, SessionStreamId, SessionStreamItem,
};

use crate::{DevtoolsSnapshot, DevtoolsTarget, InspectionRevisionMismatch};
#[cfg(feature = "spoor-session")]
use crate::{
    InspectionDelta, InspectionDispatch, InspectionElementDetails, InspectionRequest,
    InspectionResponse, InspectionStreamItem, InspectionSubscription, InspectionSubscriptionKind,
    InspectionSubsystem, InspectionSummaries, InspectionUpdateReason, LiveInspectionRequest,
    LiveInspectionResponse, LiveInspectionStreamItem, selected_element_property_grid,
};

/// Same-process devtools session state.
///
/// This type owns Overstory-specific snapshot and selection state; it
/// explicitly does not own host event loops, rendering, network transport, or
/// Spoor's generic telemetry vocabulary.
#[derive(Clone, Debug)]
pub struct DevtoolsSession {
    snapshot: DevtoolsSnapshot,
    selected_element: Option<ElementId>,
    #[cfg(feature = "spoor-session")]
    next_inspection_stream_id: u64,
    #[cfg(feature = "spoor-session")]
    inspection_streams: Vec<InspectionStreamState>,
    #[cfg(feature = "spoor-snapshot-session")]
    spoor: DevtoolsSpoorSession,
}

impl DevtoolsSession {
    /// Creates a session for `snapshot` with no explicit selected element.
    #[must_use]
    pub fn new(snapshot: DevtoolsSnapshot) -> Self {
        Self {
            snapshot,
            selected_element: None,
            #[cfg(feature = "spoor-session")]
            next_inspection_stream_id: 1,
            #[cfg(feature = "spoor-session")]
            inspection_streams: Vec::new(),
            #[cfg(feature = "spoor-snapshot-session")]
            spoor: DevtoolsSpoorSession::default(),
        }
    }

    /// Captures a snapshot from `target` and starts a session around it.
    #[must_use]
    pub fn capture(target: &impl DevtoolsTarget) -> Self {
        Self::new(DevtoolsSnapshot::capture(target))
    }

    /// Returns a copy of this session with `selected` as the selected element.
    #[must_use]
    pub fn with_selected_element(mut self, selected: Option<ElementId>) -> Self {
        self.set_selected_element(selected);
        self
    }

    /// Returns the currently attached inspection snapshot.
    #[must_use]
    pub const fn snapshot(&self) -> &DevtoolsSnapshot {
        &self.snapshot
    }

    /// Returns the currently selected element, if one is selected.
    #[must_use]
    pub const fn selected_element(&self) -> Option<ElementId> {
        self.selected_element
    }

    /// Returns the outline key for the currently selected element, if one is selected.
    #[must_use]
    pub fn selected_outline_key(&self) -> Option<OutlineNodeKey> {
        self.selected_element
            .and_then(|id| self.snapshot.outline_key_for_element(id))
    }

    /// Replaces the current selected element.
    ///
    /// Stale element ids are normalized to `None` because a session selection
    /// must be meaningful against its current snapshot.
    pub fn set_selected_element(&mut self, selected: Option<ElementId>) -> Option<DevtoolsEvent> {
        let selected = selected.filter(|id| self.snapshot.element(*id).is_some());
        if self.selected_element == selected {
            return None;
        }
        self.selected_element = selected;
        Some(DevtoolsEvent::SelectionChanged { selected })
    }

    /// Selects the element represented by `key`.
    pub fn select_outline_key(&mut self, key: Option<OutlineNodeKey>) -> Option<DevtoolsEvent> {
        let selected = key.and_then(|key| self.snapshot.element_for_outline_key(key));
        self.set_selected_element(selected)
    }

    /// Replaces the current inspection snapshot.
    ///
    /// If the selected element is not present in the new snapshot, selection is
    /// cleared.
    pub fn replace_snapshot(&mut self, snapshot: DevtoolsSnapshot) -> DevtoolsEvent {
        #[cfg(feature = "spoor-session")]
        {
            let previous_tree = self.snapshot.inspection_element_tree();
            let current_tree = snapshot.inspection_element_tree();
            let tree_patch = previous_tree.patch_to(&current_tree);
            self.snapshot = snapshot;
            self.normalize_selected_element();
            DevtoolsEvent::ElementTreePatched {
                selected: self.selected_element,
                tree_patch,
            }
        }
        #[cfg(not(feature = "spoor-session"))]
        {
            self.snapshot = snapshot;
            self.normalize_selected_element();
            DevtoolsEvent::SnapshotReplaced {
                selected: self.selected_element,
            }
        }
    }

    /// Applies target-authored inspection changes to the retained snapshot.
    ///
    /// This is the incremental live-update path. It keeps the session snapshot
    /// current without forcing a full target recapture, then emits the same
    /// keyed tree-patch event that snapshot replacement uses for mirrors and
    /// remote transports.
    #[cfg(feature = "spoor-session")]
    pub fn apply_inspection_changes(
        &mut self,
        changes: UiInspectionChanges,
    ) -> Result<Option<DevtoolsEvent>, InspectionRevisionMismatch> {
        if changes.is_empty() {
            return Ok(None);
        }

        let tree_topology_changed =
            !changes.tree.removed.is_empty() || !changes.tree.upserted.is_empty();
        let overlays_changed = changes
            .overlays
            .as_ref()
            .is_some_and(|overlays| overlays != &self.snapshot.inspection().overlays);
        let previous_tree = tree_topology_changed.then(|| self.snapshot.inspection_element_tree());
        self.snapshot.apply_inspection_changes(changes)?;
        self.normalize_selected_element();
        let Some(previous_tree) = previous_tree else {
            return Ok(Some(if overlays_changed {
                DevtoolsEvent::OverlaysChanged {
                    selected: self.selected_element,
                }
            } else {
                DevtoolsEvent::TargetChanged {
                    selected: self.selected_element,
                }
            }));
        };

        let current_tree = self.snapshot.inspection_element_tree();
        let tree_patch = previous_tree.patch_to(&current_tree);
        if tree_patch.removed.is_empty() && tree_patch.upserted.is_empty() {
            return Ok(Some(DevtoolsEvent::TargetChanged {
                selected: self.selected_element,
            }));
        }

        Ok(Some(DevtoolsEvent::ElementTreePatched {
            selected: self.selected_element,
            tree_patch,
        }))
    }

    /// Handles one typed live inspection request.
    ///
    /// This is the same-process implementation of the Overstory inspection
    /// protocol. A network transport can carry the same payloads through
    /// Spoor's live-session envelopes after choosing an encoding.
    #[cfg(feature = "spoor-session")]
    #[must_use]
    pub fn handle_inspection_request(
        &mut self,
        request: LiveInspectionRequest,
    ) -> LiveInspectionResponse {
        let request_id = request.id;
        match self.handle_inspection_payload(request.payload) {
            Ok(handled) => SessionResponse::ok(request_id, handled.response),
            Err(error) => SessionResponse::err(request_id, error),
        }
    }

    /// Dispatches one typed live inspection request.
    ///
    /// This returns both the response and any stream items emitted by the
    /// request. Use this path when a same-process client owns subscribed
    /// mirrors and wants selection or target changes to flow through the same
    /// stream vocabulary as remote transports.
    #[cfg(feature = "spoor-session")]
    #[must_use]
    pub fn dispatch_inspection_request(
        &mut self,
        request: LiveInspectionRequest,
    ) -> InspectionDispatch {
        let request_id = request.id;
        match self.handle_inspection_payload(request.payload) {
            Ok(handled) => {
                let stream_items = handled.event.map_or_else(Vec::new, |event| {
                    self.inspection_stream_items_for_event(event)
                });
                InspectionDispatch::new(
                    SessionResponse::ok(request_id, handled.response),
                    stream_items,
                )
            }
            Err(error) => {
                InspectionDispatch::new(SessionResponse::err(request_id, error), Vec::new())
            }
        }
    }

    #[cfg(feature = "spoor-session")]
    fn handle_inspection_payload(
        &mut self,
        payload: InspectionRequest,
    ) -> Result<HandledInspectionPayload, SessionError> {
        match payload {
            InspectionRequest::CaptureSnapshot => Ok(HandledInspectionPayload::response(
                InspectionResponse::Snapshot(Box::new(self.snapshot.clone())),
            )),
            InspectionRequest::ElementTree => Ok(HandledInspectionPayload::response(
                InspectionResponse::ElementTree(self.snapshot.inspection_element_tree()),
            )),
            InspectionRequest::ElementDetails { element } => {
                let row = self
                    .snapshot
                    .element(element)
                    .cloned()
                    .ok_or_else(|| element_not_found(element))?;
                Ok(HandledInspectionPayload::response(
                    InspectionResponse::ElementDetails(Box::new(InspectionElementDetails {
                        row: row.clone(),
                        properties: selected_element_property_grid(&self.snapshot, &row),
                    })),
                ))
            }
            InspectionRequest::Selection => Ok(HandledInspectionPayload::response(
                InspectionResponse::Selection {
                    selected: self.selected_element,
                },
            )),
            InspectionRequest::Summaries => Ok(HandledInspectionPayload::response(
                InspectionResponse::Summaries(Box::new(InspectionSummaries::from_snapshot(
                    &self.snapshot,
                ))),
            )),
            InspectionRequest::SetSelection { selected } => {
                let event = self.set_selected_element(selected);
                Ok(HandledInspectionPayload::new(
                    InspectionResponse::Selection {
                        selected: self.selected_element,
                    },
                    event,
                ))
            }
            InspectionRequest::Subscribe(subscription) => {
                let stream_id = self.allocate_inspection_stream(subscription);
                Ok(HandledInspectionPayload::response(
                    InspectionResponse::Subscribed {
                        stream_id,
                        subscription,
                    },
                ))
            }
        }
    }

    #[cfg(feature = "spoor-session")]
    fn allocate_inspection_stream(
        &mut self,
        subscription: InspectionSubscription,
    ) -> SessionStreamId {
        let stream_id = SessionStreamId::new(self.next_inspection_stream_id);
        self.next_inspection_stream_id = self.next_inspection_stream_id.saturating_add(1);
        self.inspection_streams.push(InspectionStreamState {
            id: stream_id,
            subscription,
            next_sequence: 0,
        });
        stream_id
    }

    /// Builds stream items for `event` across all matching inspection streams.
    #[cfg(feature = "spoor-session")]
    #[must_use]
    pub fn inspection_stream_items_for_event(
        &mut self,
        event: DevtoolsEvent,
    ) -> Vec<LiveInspectionStreamItem> {
        let stream_item = inspection_stream_item_for_event(event);
        let mut items = Vec::new();
        for stream in &mut self.inspection_streams {
            if !stream.matches(stream_item.subsystem()) {
                continue;
            }
            let item = SessionStreamItem::new(stream.id, stream.next_sequence, stream_item.clone());
            stream.next_sequence = stream.next_sequence.saturating_add(1);
            items.push(item);
        }
        items
    }

    /// Replaces the attached Spoor session envelopes.
    #[cfg(feature = "spoor-snapshot-session")]
    pub fn replace_spoor_envelopes(&mut self, envelopes: Vec<OwnedSessionEnvelope>) {
        self.spoor = DevtoolsSpoorSession::new(envelopes);
    }

    /// Returns the attached Spoor session envelope summary.
    #[cfg(feature = "spoor-snapshot-session")]
    #[must_use]
    pub const fn spoor_summary(&self) -> DevtoolsSpoorSummary {
        self.spoor.summary
    }

    /// Returns the attached Spoor session envelopes.
    #[cfg(feature = "spoor-snapshot-session")]
    #[must_use]
    pub fn spoor_envelopes(&self) -> &[OwnedSessionEnvelope] {
        &self.spoor.envelopes
    }
}

impl DevtoolsSession {
    fn normalize_selected_element(&mut self) {
        if self
            .selected_element
            .is_some_and(|id| self.snapshot.element(id).is_none())
        {
            self.selected_element = None;
        }
    }
}

#[cfg(feature = "spoor-session")]
#[derive(Clone, Debug)]
struct HandledInspectionPayload {
    response: InspectionResponse,
    event: Option<DevtoolsEvent>,
}

#[cfg(feature = "spoor-session")]
impl HandledInspectionPayload {
    fn response(response: InspectionResponse) -> Self {
        Self::new(response, None)
    }

    const fn new(response: InspectionResponse, event: Option<DevtoolsEvent>) -> Self {
        Self { response, event }
    }
}

#[cfg(feature = "spoor-session")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct InspectionStreamState {
    id: SessionStreamId,
    subscription: InspectionSubscription,
    next_sequence: u64,
}

#[cfg(feature = "spoor-session")]
impl InspectionStreamState {
    fn matches(self, subsystem: InspectionSubsystem) -> bool {
        match self.subscription.kind {
            InspectionSubscriptionKind::All => true,
            InspectionSubscriptionKind::Subsystem(candidate) => candidate == subsystem,
        }
    }
}

#[cfg(feature = "spoor-session")]
fn element_not_found(element: ElementId) -> SessionError {
    SessionError::new(
        SessionErrorCode::NotFound,
        alloc::format!("element {element:?} is not present in the current snapshot"),
    )
}

#[cfg(feature = "spoor-session")]
fn inspection_stream_item_for_event(event: DevtoolsEvent) -> InspectionStreamItem {
    match event {
        DevtoolsEvent::SelectionChanged { .. } => {
            InspectionStreamItem::Delta(InspectionDelta::new(
                InspectionSubsystem::Elements,
                InspectionUpdateReason::SelectionChanged,
            ))
        }
        DevtoolsEvent::SnapshotReplaced { .. } => {
            InspectionStreamItem::Delta(InspectionDelta::new(
                InspectionSubsystem::Elements,
                InspectionUpdateReason::SnapshotReplaced,
            ))
        }
        DevtoolsEvent::TargetChanged { .. } => InspectionStreamItem::Delta(InspectionDelta::new(
            InspectionSubsystem::Elements,
            InspectionUpdateReason::TargetChanged,
        )),
        DevtoolsEvent::OverlaysChanged { .. } => InspectionStreamItem::Delta(InspectionDelta::new(
            InspectionSubsystem::Overlays,
            InspectionUpdateReason::TargetChanged,
        )),
        DevtoolsEvent::ElementTreePatched { tree_patch, .. } => {
            InspectionStreamItem::ElementTreePatch(tree_patch)
        }
    }
}

/// Shared handle for a same-process devtools session.
///
/// The handle is intentionally small: it lets inspector UI callbacks update
/// session state without making the target `Ui` borrowable through the devtools
/// view.
#[derive(Clone, Debug)]
pub struct DevtoolsSessionHandle {
    inner: Rc<RefCell<DevtoolsSession>>,
}

impl DevtoolsSessionHandle {
    /// Wraps `session` in a shareable handle.
    #[must_use]
    pub fn new(session: DevtoolsSession) -> Self {
        Self {
            inner: Rc::new(RefCell::new(session)),
        }
    }

    /// Reads the session through a short-lived callback.
    pub fn with<R>(&self, f: impl FnOnce(&DevtoolsSession) -> R) -> R {
        f(&self.inner.borrow())
    }

    /// Mutates the session through a short-lived callback.
    pub fn with_mut<R>(&self, f: impl FnOnce(&mut DevtoolsSession) -> R) -> R {
        f(&mut self.inner.borrow_mut())
    }
}

/// Event produced by a [`DevtoolsSession`] mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DevtoolsEvent {
    /// The selected element changed.
    SelectionChanged {
        /// Newly selected element, or `None` when selection was cleared.
        selected: Option<ElementId>,
    },
    /// The current snapshot changed.
    SnapshotReplaced {
        /// Selection after stale ids were normalized.
        selected: Option<ElementId>,
    },
    /// Target details changed without changing the projected element tree.
    #[cfg(feature = "spoor-session")]
    TargetChanged {
        /// Selection after stale ids were normalized.
        selected: Option<ElementId>,
    },
    /// Overlay state changed without changing the projected element tree.
    #[cfg(feature = "spoor-session")]
    OverlaysChanged {
        /// Selection after stale ids were normalized.
        selected: Option<ElementId>,
    },
    /// The projected retained element tree changed.
    #[cfg(feature = "spoor-session")]
    ElementTreePatched {
        /// Selection after stale ids were normalized.
        selected: Option<ElementId>,
        /// Keyed element tree patch for subscribed mirrors.
        tree_patch: crate::InspectionElementTreePatch,
    },
}

/// Summary of attached Spoor session envelopes.
#[cfg(feature = "spoor-snapshot-session")]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DevtoolsSpoorSummary {
    /// Number of envelopes attached to the session.
    pub envelope_count: usize,
    /// Latest observed Spoor session sequence.
    pub session_sequence: Option<u64>,
    /// Number of context rows in the attached envelope batch.
    pub context_count: usize,
    /// Number of counter rows in the attached envelope batch.
    pub counter_count: usize,
    /// Number of event rows in the attached envelope batch.
    pub event_count: usize,
    /// Context rows dropped by the producer.
    pub dropped_contexts: u64,
    /// Counter rows dropped by the producer.
    pub dropped_counters: u64,
    /// Event rows dropped by the producer.
    pub dropped_events: u64,
}

#[cfg(feature = "spoor-snapshot-session")]
#[derive(Clone, Debug, Default, PartialEq)]
struct DevtoolsSpoorSession {
    envelopes: Vec<OwnedSessionEnvelope>,
    summary: DevtoolsSpoorSummary,
}

#[cfg(feature = "spoor-snapshot-session")]
impl DevtoolsSpoorSession {
    fn new(envelopes: Vec<OwnedSessionEnvelope>) -> Self {
        let summary = summarize_spoor_envelopes(&envelopes);
        Self { envelopes, summary }
    }
}

#[cfg(feature = "spoor-snapshot-session")]
fn summarize_spoor_envelopes(envelopes: &[OwnedSessionEnvelope]) -> DevtoolsSpoorSummary {
    let mut summary = DevtoolsSpoorSummary {
        envelope_count: envelopes.len(),
        ..DevtoolsSpoorSummary::default()
    };
    for envelope in envelopes {
        summary.session_sequence = Some(envelope.header.session_sequence);
        match &envelope.payload {
            OwnedSessionPayload::SnapshotStart(snapshot) => {
                summary.context_count =
                    summary.context_count.saturating_add(snapshot.context_count);
                summary.counter_count =
                    summary.counter_count.saturating_add(snapshot.counter_count);
                summary.event_count = summary.event_count.saturating_add(snapshot.event_count);
            }
            OwnedSessionPayload::Contexts(_)
            | OwnedSessionPayload::Counters(_)
            | OwnedSessionPayload::Events(_) => {}
            OwnedSessionPayload::SnapshotEnd(dropped) => {
                summary.dropped_contexts =
                    summary.dropped_contexts.saturating_add(dropped.contexts);
                summary.dropped_counters =
                    summary.dropped_counters.saturating_add(dropped.counters);
                summary.dropped_events = summary.dropped_events.saturating_add(dropped.events);
            }
        }
    }
    summary
}
