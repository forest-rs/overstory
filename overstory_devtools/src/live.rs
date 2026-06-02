// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Same-process devtools controller.
//!
//! This module owns the connection between target snapshot capture, the
//! inspection session, and a client-side mirror. It explicitly does not own the
//! target `Ui`, host event loop, renderer, or transport.

use alloc::vec::Vec;

use overstory::{ElementId, OutlineNodeKey};
use spoor_session::SessionError;

use crate::{
    DevtoolsEvent, DevtoolsSession, DevtoolsSessionHandle, DevtoolsSnapshot, DevtoolsTarget,
    InspectionClient, InspectionMirror, InspectionSubscription, LiveDevtoolsTarget,
};

/// Summary of one same-process devtools refresh.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevtoolsRefresh {
    /// Session event produced by the refresh.
    pub event: Option<DevtoolsEvent>,
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
            Some(DevtoolsEvent::ElementTreePatched { tree_patch, .. }) => {
                Some(DevtoolsTreePatchCounts {
                    removed: tree_patch.removed.len(),
                    upserted: tree_patch.upserted.len(),
                })
            }
            Some(
                DevtoolsEvent::SelectionChanged { .. }
                | DevtoolsEvent::SnapshotReplaced { .. }
                | DevtoolsEvent::TargetChanged { .. },
            )
            | None => None,
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

    /// Captures `target`, clears pre-baseline changes, and starts a live session.
    pub fn capture_from_live(
        target: &mut impl LiveDevtoolsTarget,
        selected: Option<ElementId>,
    ) -> Result<Self, SessionError> {
        let devtools = Self::capture(target, selected)?;
        target.begin_live_inspection_recording();
        target.clear_inspection_changes();
        Ok(devtools)
    }

    /// Stops target-side recording for this live session.
    ///
    /// The controller does not own the target `Ui`, so hosts call this when
    /// their inspector surface is closed.
    pub fn detach_from_live(&mut self, target: &mut impl LiveDevtoolsTarget) {
        target.end_live_inspection_recording();
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
        self.sync_mirror_selection()?;
        let selected = self.session.with(DevtoolsSession::selected_element);
        Ok(DevtoolsRefresh {
            event: Some(event),
            stream_item_count,
            selected,
        })
    }

    /// Drains target-authored changes and applies them to the live mirror.
    pub fn refresh_from_live(
        &mut self,
        target: &mut impl LiveDevtoolsTarget,
    ) -> Result<DevtoolsRefresh, SessionError> {
        let changes = target.drain_inspection_changes();
        let event = self
            .session
            .with_mut(|session| session.apply_inspection_changes(changes));
        let stream_items = event
            .clone()
            .map_or_else(Vec::new, |event| self.client.stream_items_for_event(event));
        let stream_item_count = stream_items.len();
        for item in &stream_items {
            self.mirror.apply_stream_item(&self.client, item)?;
        }
        self.sync_mirror_selection()?;
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

    fn sync_mirror_selection(&mut self) -> Result<(), SessionError> {
        let selected = self.session.with(DevtoolsSession::selected_element);
        if self.mirror.selected_element() != selected {
            self.mirror.refresh_selection(&self.client)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use overstory::pseudos::PSEUDO_HOVER;
    use overstory::{Button, Panel, Ui};

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
    fn live_devtools_refresh_syncs_selection_changed_by_another_client() {
        let mut ui = Ui::new();
        let root = ui.root();
        let button = ui.append(root, Button::new("Inspect app"));
        let child = ui.append(root, Panel::new());
        let mut devtools =
            LiveDevtools::capture_from_live(&mut ui, Some(button)).expect("devtools should attach");
        let rendered_view_client = InspectionClient::new(devtools.session().clone());

        let selected = rendered_view_client
            .set_selection(Some(child))
            .expect("rendered devtools view should select the child");
        assert_eq!(selected, Some(child));
        assert_eq!(
            devtools.mirror().selected_element(),
            Some(button),
            "the live controller mirror has not observed the other client's stream item yet"
        );

        let refresh = devtools
            .refresh_from_live(&mut ui)
            .expect("live refresh should sync controller state");

        assert_eq!(refresh.selected, Some(child));
        assert_eq!(devtools.mirror().selected_element(), Some(child));
        assert_eq!(
            devtools
                .mirror()
                .selected_details()
                .expect("selected details should be refreshed")
                .row
                .id,
            child
        );
        devtools.detach_from_live(&mut ui);
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

        let Some(DevtoolsEvent::ElementTreePatched {
            selected: None,
            tree_patch,
        }) = refresh.event
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
    fn live_devtools_refresh_from_live_drains_tree_changes() {
        let mut ui = Ui::new();
        let root = ui.root();
        let mut devtools =
            LiveDevtools::capture_from_live(&mut ui, None).expect("devtools should attach");
        assert!(ui.is_live_inspection_recording());

        let first = devtools
            .refresh_from_live(&mut ui)
            .expect("unchanged target should refresh");
        assert_eq!(first.event, None);
        assert_eq!(first.stream_item_count, 0);

        let child = ui.append(root, Panel::new());
        let second = devtools
            .refresh_from_live(&mut ui)
            .expect("live refresh should update mirror");

        assert_eq!(second.stream_item_count, 1);
        assert!(
            second
                .tree_patch_counts()
                .is_some_and(|counts| { counts.removed == 0 && counts.upserted >= 1 })
        );
        assert_eq!(devtools.mirror().tree().nodes.len(), 2);
        assert!(devtools.mirror().outline_key_for_element(child).is_some());

        ui.remove(child);
        let third = devtools
            .refresh_from_live(&mut ui)
            .expect("live removal should update mirror");

        assert_eq!(third.stream_item_count, 1);
        assert_eq!(
            third.tree_patch_counts(),
            Some(DevtoolsTreePatchCounts {
                removed: 1,
                upserted: 1,
            })
        );
        assert_eq!(devtools.mirror().tree().nodes.len(), 1);
        assert!(devtools.mirror().outline_key_for_element(child).is_none());

        devtools.detach_from_live(&mut ui);
        assert!(!ui.is_live_inspection_recording());
    }

    #[test]
    fn live_devtools_refresh_from_live_streams_frame_summary_updates() {
        let mut ui = Ui::new();
        let mut devtools =
            LiveDevtools::capture_from_live(&mut ui, None).expect("devtools should attach");

        ui.set_frame_timings(overstory::UiFrameTimings {
            surface_refresh: Duration::from_millis(4),
            total: Duration::from_millis(9),
            ..overstory::UiFrameTimings::default()
        });
        let refresh = devtools
            .refresh_from_live(&mut ui)
            .expect("frame summary should stream");

        assert_eq!(
            refresh.event,
            Some(DevtoolsEvent::TargetChanged { selected: None })
        );
        assert_eq!(refresh.stream_item_count, 1);
        assert_eq!(
            devtools.mirror().summaries().frame.timings.surface_refresh,
            Duration::from_millis(4)
        );
        assert_eq!(
            devtools.mirror().summaries().frame.timings.total,
            Duration::from_millis(9)
        );

        ui.set_renderer_timings(overstory::UiRendererTimings {
            scene_to_texture: Duration::from_millis(2),
            surface_acquire: Duration::from_millis(1),
            blit: Duration::from_millis(3),
            present: Duration::from_millis(1),
            total: Duration::from_millis(7),
        });
        let refresh = devtools
            .refresh_from_live(&mut ui)
            .expect("renderer summary should stream");

        assert_eq!(
            refresh.event,
            Some(DevtoolsEvent::TargetChanged { selected: None })
        );
        assert_eq!(
            devtools.mirror().summaries().frame.renderer_timings.total,
            Duration::from_millis(7)
        );
        devtools.detach_from_live(&mut ui);
    }

    #[test]
    fn live_devtools_refresh_from_live_updates_selected_details() {
        let mut ui = Ui::new();
        let root = ui.root();
        let button = ui.append(root, Button::new("Inspect"));
        let mut devtools =
            LiveDevtools::capture_from_live(&mut ui, Some(button)).expect("devtools should attach");

        ui.add_pseudo(button, PSEUDO_HOVER);
        let refresh = devtools
            .refresh_from_live(&mut ui)
            .expect("live state refresh should update mirror");

        assert_eq!(refresh.stream_item_count, 1);
        assert_eq!(
            refresh.event,
            Some(DevtoolsEvent::TargetChanged {
                selected: Some(button),
            })
        );
        assert!(
            devtools
                .mirror()
                .selected_details()
                .is_some_and(|details| details.row.state.hovered)
        );
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
