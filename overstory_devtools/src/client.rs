// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Same-process client and retained mirror for the inspection protocol.
//!
//! This module owns the devtools-frontend view of the live inspection session;
//! it explicitly does not own target snapshots, target mutation, rendering, or
//! transport encoding.

use core::cell::Cell;

use alloc::format;
use alloc::rc::Rc;
use alloc::vec::Vec;

use overstory::{ElementId, OutlineNodeKey, OutlineView, OutlineViewNode};
use spoor_session::{
    SessionError, SessionErrorCode, SessionRequestId, SessionResult, SessionStreamId,
};

use crate::{
    DevtoolsEvent, DevtoolsSessionHandle, DevtoolsSnapshot, InspectionDelta,
    InspectionElementDetails, InspectionElementTree, InspectionRequest, InspectionResponse,
    InspectionStreamItem, InspectionSubscription, InspectionSummaries, InspectionUpdateReason,
    LiveInspectionRequest, LiveInspectionStreamItem, selected_element_property_grid,
};

/// Same-process client for the Overstory inspection protocol.
///
/// The client talks to a [`DevtoolsSessionHandle`] using Spoor live-session
/// request and response envelopes. A future remote client can keep this public
/// surface and swap only the transport.
#[derive(Clone, Debug)]
pub struct InspectionClient {
    session: DevtoolsSessionHandle,
    next_request_id: Rc<Cell<u64>>,
}

impl InspectionClient {
    /// Creates a client around `session`.
    #[must_use]
    pub fn new(session: DevtoolsSessionHandle) -> Self {
        Self {
            session,
            next_request_id: Rc::new(Cell::new(1)),
        }
    }

    /// Returns the underlying same-process session handle.
    #[must_use]
    pub const fn session(&self) -> &DevtoolsSessionHandle {
        &self.session
    }

    /// Requests the current compact retained element tree.
    pub fn element_tree(&self) -> Result<InspectionElementTree, SessionError> {
        match self.request("elementTree", InspectionRequest::ElementTree)? {
            InspectionResponse::ElementTree(tree) => Ok(tree),
            response => Err(unexpected_response("elementTree", &response)),
        }
    }

    /// Requests details for one element.
    pub fn element_details(
        &self,
        element: ElementId,
    ) -> Result<InspectionElementDetails, SessionError> {
        match self.request(
            "elementDetails",
            InspectionRequest::ElementDetails { element },
        )? {
            InspectionResponse::ElementDetails(details) => Ok(*details),
            response => Err(unexpected_response("elementDetails", &response)),
        }
    }

    /// Requests the current selected element.
    pub fn selection(&self) -> Result<Option<ElementId>, SessionError> {
        match self.request("selection", InspectionRequest::Selection)? {
            InspectionResponse::Selection { selected } => Ok(selected),
            response => Err(unexpected_response("selection", &response)),
        }
    }

    /// Requests compact subsystem summaries.
    pub fn summaries(&self) -> Result<InspectionSummaries, SessionError> {
        match self.request("summaries", InspectionRequest::Summaries)? {
            InspectionResponse::Summaries(summaries) => Ok(*summaries),
            response => Err(unexpected_response("summaries", &response)),
        }
    }

    /// Replaces the current selected element and returns the normalized result.
    pub fn set_selection(
        &self,
        selected: Option<ElementId>,
    ) -> Result<Option<ElementId>, SessionError> {
        match self.request("setSelection", InspectionRequest::SetSelection { selected })? {
            InspectionResponse::Selection { selected } => Ok(selected),
            response => Err(unexpected_response("setSelection", &response)),
        }
    }

    /// Subscribes to a live inspection stream and returns the accepted stream id.
    pub fn subscribe(
        &self,
        subscription: InspectionSubscription,
    ) -> Result<SessionStreamId, SessionError> {
        match self.request("subscribe", InspectionRequest::Subscribe(subscription))? {
            InspectionResponse::Subscribed { stream_id, .. } => Ok(stream_id),
            response => Err(unexpected_response("subscribe", &response)),
        }
    }

    /// Emits stream items for a same-process session event.
    ///
    /// Host integrations call this after applying a session mutation so
    /// attached mirrors can refresh only the state affected by the event.
    #[must_use]
    pub fn stream_items_for_event(&self, event: DevtoolsEvent) -> Vec<LiveInspectionStreamItem> {
        self.session
            .with_mut(|session| session.inspection_stream_items_for_event(event))
    }

    fn request(
        &self,
        method: &'static str,
        payload: InspectionRequest,
    ) -> Result<InspectionResponse, SessionError> {
        let request_id = self.allocate_request_id();
        let response = self.session.with_mut(|session| {
            session.handle_inspection_request(LiveInspectionRequest::new(
                request_id,
                crate::inspection_method(method),
                payload,
            ))
        });
        if response.request_id != request_id {
            return Err(SessionError::new(
                SessionErrorCode::Failed,
                format!(
                    "inspection response id {:?} did not match request id {:?}",
                    response.request_id, request_id
                ),
            ));
        }
        match response.result {
            SessionResult::Ok(payload) => Ok(payload),
            SessionResult::Err(error) => Err(error),
        }
    }

    fn allocate_request_id(&self) -> SessionRequestId {
        let raw = self.next_request_id.get();
        self.next_request_id.set(raw.saturating_add(1));
        SessionRequestId::new(raw)
    }
}

/// Client-side retained state for the inspection UI.
///
/// A mirror is deliberately smaller than [`DevtoolsSnapshot`]: it stores the
/// projected element tree, current selection, and lazy details for the element
/// currently shown by the inspector.
#[derive(Clone, Debug, PartialEq)]
pub struct InspectionMirror {
    tree: InspectionElementTree,
    selected: Option<ElementId>,
    selected_details: Option<InspectionElementDetails>,
    summaries: InspectionSummaries,
}

impl InspectionMirror {
    /// Attaches to `client` and fetches the initial tree, selection, and details.
    pub fn attach(client: &InspectionClient) -> Result<Self, SessionError> {
        let mut mirror = Self {
            tree: client.element_tree()?,
            selected: client.selection()?,
            selected_details: None,
            summaries: client.summaries()?,
        };
        mirror.refresh_selected_details(client)?;
        Ok(mirror)
    }

    pub(crate) fn from_snapshot(snapshot: &DevtoolsSnapshot, selected: Option<ElementId>) -> Self {
        let tree = snapshot.inspection_element_tree();
        let selected = selected.filter(|element| tree.outline_key_for_element(*element).is_some());
        let selected_details = selected
            .or_else(|| tree.root_element())
            .and_then(|element| snapshot.element(element))
            .cloned()
            .map(|row| {
                let properties = selected_element_property_grid(snapshot, &row);
                InspectionElementDetails { row, properties }
            });

        Self {
            tree,
            selected,
            selected_details,
            summaries: InspectionSummaries::from_snapshot(snapshot),
        }
    }

    /// Returns the mirrored projected element tree.
    #[must_use]
    pub const fn tree(&self) -> &InspectionElementTree {
        &self.tree
    }

    /// Returns the selected element, if the session has an explicit selection.
    #[must_use]
    pub const fn selected_element(&self) -> Option<ElementId> {
        self.selected
    }

    /// Returns the details currently shown by the mirror.
    #[must_use]
    pub const fn selected_details(&self) -> Option<&InspectionElementDetails> {
        self.selected_details.as_ref()
    }

    /// Returns compact subsystem summaries for the mirrored target.
    #[must_use]
    pub const fn summaries(&self) -> &InspectionSummaries {
        &self.summaries
    }

    /// Returns the outline key for `element`.
    #[must_use]
    pub fn outline_key_for_element(&self, element: ElementId) -> Option<OutlineNodeKey> {
        self.tree.outline_key_for_element(element)
    }

    /// Returns the element represented by `key`.
    #[must_use]
    pub fn element_for_outline_key(&self, key: OutlineNodeKey) -> Option<ElementId> {
        self.tree.element_for_outline_key(key)
    }

    /// Builds an [`OutlineView`] for the current mirrored tree and selection.
    #[must_use]
    pub fn outline_view(&self) -> OutlineView {
        let mut view = OutlineView::new().nodes(
            self.tree
                .nodes
                .iter()
                .map(|node| {
                    OutlineViewNode::new(node.label.clone())
                        .with_first_child(node.first_child)
                        .with_next_sibling(node.next_sibling)
                })
                .collect(),
            self.tree.root,
        );
        for (index, node) in self.tree.nodes.iter().enumerate() {
            if node.row.child_count > 0 {
                view = view.expanded(OutlineNodeKey::from(index));
            }
        }
        if let Some(key) = self
            .selected
            .and_then(|element| self.outline_key_for_element(element))
        {
            view = view.selected_key(key);
        }
        view
    }

    /// Refreshes the projected element tree from the session.
    pub fn refresh_tree(&mut self, client: &InspectionClient) -> Result<(), SessionError> {
        self.tree = client.element_tree()?;
        self.summaries = client.summaries()?;
        if self
            .selected
            .is_some_and(|element| self.tree.outline_key_for_element(element).is_none())
        {
            self.selected = None;
        }
        self.refresh_selected_details(client)
    }

    /// Refreshes selection and selected details from the session.
    pub fn refresh_selection(&mut self, client: &InspectionClient) -> Result<(), SessionError> {
        self.selected = client.selection()?;
        self.refresh_selected_details(client)
    }

    /// Refreshes compact subsystem summaries from the session.
    pub fn refresh_summaries(&mut self, client: &InspectionClient) -> Result<(), SessionError> {
        self.summaries = client.summaries()?;
        Ok(())
    }

    /// Selects the element represented by `key`.
    pub fn select_outline_key(
        &mut self,
        client: &InspectionClient,
        key: Option<OutlineNodeKey>,
    ) -> Result<Option<ElementId>, SessionError> {
        let selected = key.and_then(|key| self.element_for_outline_key(key));
        self.select_element(client, selected)
    }

    /// Selects `element` and refreshes lazy details for the displayed element.
    pub fn select_element(
        &mut self,
        client: &InspectionClient,
        selected: Option<ElementId>,
    ) -> Result<Option<ElementId>, SessionError> {
        self.selected = client.set_selection(selected)?;
        self.refresh_selected_details(client)?;
        Ok(self.selected)
    }

    /// Applies one stream item to the mirror.
    pub fn apply_stream_item(
        &mut self,
        client: &InspectionClient,
        item: &LiveInspectionStreamItem,
    ) -> Result<(), SessionError> {
        match &item.payload {
            InspectionStreamItem::Delta(delta) => self.apply_delta(client, *delta),
        }
    }

    fn apply_delta(
        &mut self,
        client: &InspectionClient,
        delta: InspectionDelta,
    ) -> Result<(), SessionError> {
        match delta.reason {
            InspectionUpdateReason::Initial | InspectionUpdateReason::SnapshotReplaced => {
                self.refresh_tree(client)
            }
            InspectionUpdateReason::SelectionChanged => self.refresh_selection(client),
            InspectionUpdateReason::TargetChanged => {
                self.refresh_summaries(client)?;
                self.refresh_selected_details(client)
            }
        }
    }

    fn refresh_selected_details(&mut self, client: &InspectionClient) -> Result<(), SessionError> {
        self.selected_details = self
            .selected
            .or_else(|| self.tree.root_element())
            .map(|element| client.element_details(element))
            .transpose()?;
        Ok(())
    }
}

fn unexpected_response(method: &'static str, response: &InspectionResponse) -> SessionError {
    SessionError::new(
        SessionErrorCode::Failed,
        format!("inspection method {method} returned unexpected response {response:?}"),
    )
}

#[cfg(test)]
mod tests {
    use overstory::{Panel, Ui};

    use super::*;
    use crate::{DevtoolsSession, InspectionSubsystem};

    #[test]
    fn inspection_client_fetches_tree_selection_and_details() {
        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.append(root, Panel::new());
        let session = DevtoolsSessionHandle::new(
            DevtoolsSession::capture(&ui).with_selected_element(Some(child)),
        );
        let client = InspectionClient::new(session);

        let tree = client.element_tree().expect("tree request should succeed");
        let selected = client
            .selection()
            .expect("selection request should succeed");
        let summaries = client
            .summaries()
            .expect("summaries request should succeed");
        let details = client
            .element_details(child)
            .expect("details request should succeed");

        assert_eq!(tree.root_element(), Some(root));
        assert_eq!(tree.nodes.len(), 2);
        assert_eq!(selected, Some(child));
        assert_eq!(summaries.storage.live_elements, 2);
        assert_eq!(details.row.id, child);
    }

    #[test]
    fn inspection_mirror_tracks_outline_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.append(root, Panel::new());
        let grandchild = ui.append(child, Panel::new());
        let session = DevtoolsSessionHandle::new(
            DevtoolsSession::capture(&ui).with_selected_element(Some(child)),
        );
        let client = InspectionClient::new(session.clone());
        let mut mirror = InspectionMirror::attach(&client).expect("mirror should attach");
        let grandchild_key = mirror
            .outline_key_for_element(grandchild)
            .expect("grandchild should have an outline key");

        mirror
            .select_outline_key(&client, Some(grandchild_key))
            .expect("selection should update");

        assert_eq!(mirror.selected_element(), Some(grandchild));
        assert_eq!(
            mirror
                .selected_details()
                .expect("selected details should be loaded")
                .row
                .id,
            grandchild
        );
        assert_eq!(
            session.with(DevtoolsSession::selected_element),
            Some(grandchild)
        );
    }

    #[test]
    fn inspection_mirror_applies_selection_stream_delta() {
        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.append(root, Panel::new());
        let session = DevtoolsSessionHandle::new(DevtoolsSession::capture(&ui));
        let client = InspectionClient::new(session.clone());
        client
            .subscribe(InspectionSubscription::subsystem(
                InspectionSubsystem::Elements,
            ))
            .expect("subscription should be accepted");
        let mut mirror = InspectionMirror::attach(&client).expect("mirror should attach");

        let event = session
            .with_mut(|session| session.set_selected_element(Some(child)))
            .expect("selection should change");
        let items = client.stream_items_for_event(event);
        for item in &items {
            mirror
                .apply_stream_item(&client, item)
                .expect("stream delta should apply");
        }

        assert_eq!(mirror.selected_element(), Some(child));
        assert_eq!(
            mirror
                .selected_details()
                .expect("selected details should be loaded")
                .row
                .id,
            child
        );
    }

    #[test]
    fn inspection_mirror_defaults_details_to_root_without_selection() {
        let ui = Ui::new();
        let root = ui.root();
        let session = DevtoolsSessionHandle::new(DevtoolsSession::capture(&ui));
        let client = InspectionClient::new(session);
        let mirror = InspectionMirror::attach(&client).expect("mirror should attach");

        assert_eq!(mirror.selected_element(), None);
        assert_eq!(
            mirror
                .selected_details()
                .expect("root details should be loaded")
                .row
                .id,
            root
        );
    }
}
