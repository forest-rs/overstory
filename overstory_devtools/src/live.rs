// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Same-process devtools controller.
//!
//! This module owns the connection between target snapshot capture, the
//! inspection session, and a client-side mirror. It explicitly does not own the
//! target `Ui`, host event loop, renderer, or transport.

use overstory::{ElementId, OutlineNodeKey};
use spoor_session::SessionError;

use crate::{
    DevtoolsEvent, DevtoolsSession, DevtoolsSessionHandle, DevtoolsSnapshot, DevtoolsTarget,
    InspectionClient, InspectionMirror, InspectionSubscription,
};

/// Summary of one same-process devtools refresh.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevtoolsRefresh {
    /// Session event produced by the refresh.
    pub event: DevtoolsEvent,
    /// Number of subscribed stream items applied to the mirror.
    pub stream_item_count: usize,
    /// Selection after stale ids were normalized against the new snapshot.
    pub selected: Option<ElementId>,
}

impl DevtoolsRefresh {
    /// Returns element-tree patch counts when this refresh streamed a tree patch.
    #[must_use]
    pub const fn tree_patch_counts(&self) -> Option<DevtoolsTreePatchCounts> {
        match &self.event {
            DevtoolsEvent::ElementTreePatched { tree_patch, .. } => Some(DevtoolsTreePatchCounts {
                removed: tree_patch.removed.len(),
                upserted: tree_patch.upserted.len(),
            }),
            DevtoolsEvent::SelectionChanged { .. } | DevtoolsEvent::SnapshotReplaced { .. } => None,
        }
    }
}

/// Element-tree operation counts streamed during one refresh.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DevtoolsTreePatchCounts {
    /// Elements removed from the client-side mirror.
    pub removed: usize,
    /// Elements added or updated in the client-side mirror.
    pub upserted: usize,
}

/// Same-process controller for an Overstory devtools session.
///
/// The controller is the intended bridge for host integrations that can
/// periodically capture a target snapshot and want the inspector to keep a
/// retained client-side mirror. Current refreshes compute keyed element-tree
/// patches from those captures; target-side mutation hooks can produce the
/// same patch payloads later without a full recapture step.
#[derive(Clone, Debug)]
pub struct LiveDevtools {
    session: DevtoolsSessionHandle,
    client: InspectionClient,
    mirror: InspectionMirror,
}

impl LiveDevtools {
    /// Captures `target`, starts a session, subscribes to all inspection
    /// updates, and attaches an initial mirror.
    pub fn capture(
        target: &impl DevtoolsTarget,
        selected: Option<ElementId>,
    ) -> Result<Self, SessionError> {
        let session = DevtoolsSessionHandle::new(
            DevtoolsSession::capture(target).with_selected_element(selected),
        );
        let client = InspectionClient::new(session.clone());
        client.subscribe(InspectionSubscription::all())?;
        let mirror = InspectionMirror::attach(&client)?;
        Ok(Self {
            session,
            client,
            mirror,
        })
    }

    /// Returns the same-process session handle.
    #[must_use]
    pub const fn session(&self) -> &DevtoolsSessionHandle {
        &self.session
    }

    /// Returns the request client for this controller.
    #[must_use]
    pub const fn client(&self) -> &InspectionClient {
        &self.client
    }

    /// Returns the retained client-side mirror.
    #[must_use]
    pub const fn mirror(&self) -> &InspectionMirror {
        &self.mirror
    }

    /// Returns a mutable retained client-side mirror.
    #[must_use]
    pub const fn mirror_mut(&mut self) -> &mut InspectionMirror {
        &mut self.mirror
    }

    /// Replaces the session snapshot with a fresh capture from `target`.
    ///
    /// The refresh is applied through the session stream vocabulary so the
    /// mirror can react to local target refreshes and future remote deliveries.
    /// This same-process path still captures the target to compute the patch,
    /// but subscribed mirrors receive a keyed tree update rather than a full
    /// snapshot.
    pub fn refresh_from(
        &mut self,
        target: &impl DevtoolsTarget,
    ) -> Result<DevtoolsRefresh, SessionError> {
        let snapshot = DevtoolsSnapshot::capture(target);
        let event = self
            .session
            .with_mut(|session| session.replace_snapshot(snapshot));
        let stream_items = self.client.stream_items_for_event(event.clone());
        let stream_item_count = stream_items.len();
        for item in &stream_items {
            self.mirror.apply_stream_item(&self.client, item)?;
        }
        let selected = self.session.with(DevtoolsSession::selected_element);
        Ok(DevtoolsRefresh {
            event,
            stream_item_count,
            selected,
        })
    }

    /// Selects `element` through the inspection protocol.
    pub fn select_element(
        &mut self,
        element: Option<ElementId>,
    ) -> Result<Option<ElementId>, SessionError> {
        let selected = self.mirror.select_element(&self.client, element)?;
        self.mirror.apply_pending_stream_items(&self.client)?;
        Ok(selected)
    }

    /// Selects the element represented by `key` through the inspection protocol.
    pub fn select_outline_key(
        &mut self,
        key: Option<OutlineNodeKey>,
    ) -> Result<Option<ElementId>, SessionError> {
        let selected = self.mirror.select_outline_key(&self.client, key)?;
        self.mirror.apply_pending_stream_items(&self.client)?;
        Ok(selected)
    }
}

#[cfg(test)]
mod tests {
    use overstory::{Panel, Ui};

    use super::*;

    #[test]
    fn live_devtools_selects_through_protocol() {
        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.append(root, Panel::new());
        let mut devtools = LiveDevtools::capture(&ui, None).expect("devtools should attach");

        let selected = devtools
            .select_element(Some(child))
            .expect("selection should update");

        assert_eq!(selected, Some(child));
        assert_eq!(devtools.mirror().selected_element(), Some(child));
        assert_eq!(
            devtools.session().with(DevtoolsSession::selected_element),
            Some(child)
        );
    }

    #[test]
    fn live_devtools_refresh_updates_mirror_tree() {
        let mut ui = Ui::new();
        let root = ui.root();
        let mut devtools = LiveDevtools::capture(&ui, None).expect("devtools should attach");
        assert_eq!(devtools.mirror().tree().nodes.len(), 1);

        let child = ui.append(root, Panel::new());
        let refresh = devtools
            .refresh_from(&ui)
            .expect("refresh should update mirror");

        let DevtoolsEvent::ElementTreePatched {
            selected: None,
            tree_patch,
        } = refresh.event
        else {
            panic!("refresh should emit an element tree patch");
        };
        assert!(
            tree_patch
                .upserted
                .iter()
                .any(|node| node.row.element == root)
        );
        assert!(
            tree_patch
                .upserted
                .iter()
                .any(|node| node.row.element == child)
        );
        assert_eq!(refresh.stream_item_count, 1);
        assert_eq!(devtools.mirror().tree().nodes.len(), 2);
        assert!(devtools.mirror().outline_key_for_element(child).is_some());
    }

    #[test]
    fn live_devtools_refresh_clears_stale_selection() {
        let mut first = Ui::new();
        let first_root = first.root();
        let selected = first.append(first_root, Panel::new());
        let mut devtools =
            LiveDevtools::capture(&first, Some(selected)).expect("devtools should attach");

        let second = Ui::new();
        let refresh = devtools
            .refresh_from(&second)
            .expect("refresh should update mirror");

        assert_eq!(refresh.selected, None);
        assert_eq!(devtools.mirror().selected_element(), None);
        assert_eq!(
            devtools.session().with(DevtoolsSession::selected_element),
            None
        );
    }
}
