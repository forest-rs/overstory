// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Same-process devtools controller.
//!
//! This module owns the connection between target snapshot capture, the
//! inspection session, and a client-side mirror. It explicitly does not own the
//! target `Ui`, host event loop, renderer, or transport.

use alloc::vec::Vec;

use overstory::inspection::LiveInspectionRecording;
use overstory::{ElementId, OutlineNodeKey};
use spoor_session::{SessionError, SessionErrorCode};

use crate::{
    DevtoolsEvent, DevtoolsSession, DevtoolsSessionHandle, DevtoolsSnapshot, DevtoolsTarget,
    InspectionClient, InspectionMirror, InspectionRevisionMismatch, InspectionSubscription,
    LiveDevtoolsTarget,
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
    /// Source-side changes that produced this refresh.
    pub changes: DevtoolsRefreshChanges,
    /// Revision mismatch that caused this refresh to recapture the target.
    pub resynced_from: Option<InspectionRevisionMismatch>,
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
                | DevtoolsEvent::TargetChanged { .. }
                | DevtoolsEvent::OverlaysChanged { .. },
            )
            | None => None,
        }
    }
}

/// Source-side change categories observed during one devtools refresh.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DevtoolsRefreshChanges {
    /// The projected outline tree changed.
    pub tree: bool,
    /// The details for the currently inspected element changed.
    pub details: bool,
    /// Overlay lifecycle or derived overlay geometry changed.
    pub overlays: bool,
    /// Frame timing or renderer summary counters changed.
    pub frame: bool,
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
/// The controller is the intended bridge for host integrations that want an
/// inspector to keep a retained client-side mirror. Snapshot refreshes remain
/// available as a fallback and resynchronization path; live refreshes drain
/// target-authored changes without recapturing the complete element tree.
///
/// `LiveDevtools` is no longer cloneable because one controller owns one target
/// recording handle. Share it through an owner such as `Rc<RefCell<_>>` when
/// several same-process views need the same controller.
#[derive(Debug)]
pub struct LiveDevtools {
    session: DevtoolsSessionHandle,
    client: InspectionClient,
    mirror: InspectionMirror,
    recording: Option<LiveInspectionRecording>,
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
            recording: None,
        })
    }

    /// Starts recording, captures `target` as the baseline, and opens a live session.
    pub fn capture_from_live(
        target: &mut impl LiveDevtoolsTarget,
        selected: Option<ElementId>,
    ) -> Result<Self, SessionError> {
        let recording = target.begin_live_inspection_recording();
        let mut devtools = match Self::capture(target, selected) {
            Ok(devtools) => devtools,
            Err(error) => {
                target.end_live_inspection_recording(recording);
                return Err(error);
            }
        };
        if !target.clear_inspection_changes(recording) {
            target.end_live_inspection_recording(recording);
            return Err(inactive_recording_error());
        }
        devtools.recording = Some(recording);
        Ok(devtools)
    }

    /// Stops target-side recording for this live session.
    ///
    /// The controller does not own the target `Ui`, so hosts call this when
    /// their inspector surface is closed. Returns whether this controller held
    /// a recording that was still active on `target`.
    pub fn detach_from_live(&mut self, target: &mut impl LiveDevtoolsTarget) -> bool {
        self.recording
            .take()
            .is_some_and(|recording| target.end_live_inspection_recording(recording))
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
            changes: DevtoolsRefreshChanges {
                tree: true,
                details: true,
                overlays: true,
                frame: true,
            },
            resynced_from: None,
        })
    }

    /// Drains target-authored changes and applies them to the live mirror.
    pub fn refresh_from_live(
        &mut self,
        target: &mut impl LiveDevtoolsTarget,
    ) -> Result<DevtoolsRefresh, SessionError> {
        let recording = self.recording.ok_or_else(|| {
            SessionError::new(
                SessionErrorCode::InvalidRequest,
                "live devtools controller is not attached to a recording",
            )
        })?;
        let changes = target
            .drain_inspection_changes(recording)
            .ok_or_else(inactive_recording_error)?;
        let (frame_changed, overlays_changed) = self.session.with(|session| {
            let inspection = session.snapshot().inspection();
            (
                changes
                    .frame
                    .as_ref()
                    .is_some_and(|frame| frame != &inspection.frame),
                changes
                    .overlays
                    .as_ref()
                    .is_some_and(|overlays| overlays != &inspection.overlays),
            )
        });
        let inspected_element = self
            .session
            .with(DevtoolsSession::selected_element)
            .or_else(|| self.mirror.tree().root_element());
        let details_changed = inspected_element.is_some_and(|element| {
            changes
                .tree
                .removed
                .iter()
                .any(|removal| removal.id == element)
                || changes.tree.upserted.iter().any(|row| row.id == element)
                || changes.tree.updated.iter().any(|row| row.id == element)
        });
        let event = match self
            .session
            .with_mut(|session| session.apply_inspection_changes(changes))
        {
            Ok(event) => event,
            Err(mismatch) => {
                let mut refresh = self.refresh_from(target)?;
                if !target.clear_inspection_changes(recording) {
                    return Err(inactive_recording_error());
                }
                refresh.resynced_from = Some(mismatch);
                return Ok(refresh);
            }
        };
        let tree_changed = matches!(event, Some(DevtoolsEvent::ElementTreePatched { .. }));
        let refresh_changes = DevtoolsRefreshChanges {
            tree: tree_changed,
            details: details_changed,
            overlays: overlays_changed,
            frame: frame_changed,
        };
        let mut stream_items = event
            .clone()
            .map_or_else(Vec::new, |event| self.client.stream_items_for_event(event));
        if overlays_changed && !matches!(event, Some(DevtoolsEvent::OverlaysChanged { .. })) {
            let selected = self.session.with(DevtoolsSession::selected_element);
            stream_items.extend(
                self.client
                    .stream_items_for_event(DevtoolsEvent::OverlaysChanged { selected }),
            );
        }
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
            changes: refresh_changes,
            resynced_from: None,
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

fn inactive_recording_error() -> SessionError {
    SessionError::new(
        SessionErrorCode::Failed,
        "live inspection recording is no longer active on the target",
    )
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use core::time::Duration;

    use overstory::inspection::{UiInspectionChanges, UiInspectionSnapshot};
    use overstory::pseudos::PSEUDO_HOVER;
    use overstory::{
        BindingSpecExt, Button, EffectClip, EffectKeyframe, Layout, Panel, Popup, Size, Text, Ui,
    };

    use super::*;

    #[derive(Debug)]
    struct CorruptingTarget {
        ui: Ui,
        corrupt_next_batch: bool,
    }

    impl DevtoolsTarget for CorruptingTarget {
        fn capture_inspection_snapshot(&self) -> UiInspectionSnapshot {
            self.ui.inspection_snapshot()
        }
    }

    impl LiveDevtoolsTarget for CorruptingTarget {
        fn begin_live_inspection_recording(&mut self) -> LiveInspectionRecording {
            self.ui.begin_live_inspection_recording()
        }

        fn end_live_inspection_recording(&mut self, recording: LiveInspectionRecording) -> bool {
            self.ui.end_live_inspection_recording(recording)
        }

        fn drain_inspection_changes(
            &mut self,
            recording: LiveInspectionRecording,
        ) -> Option<UiInspectionChanges> {
            let mut changes = self.ui.drain_inspection_changes(recording)?;
            if self.corrupt_next_batch && !changes.is_empty() {
                changes.baseline_revision = changes.revision;
                self.corrupt_next_batch = false;
            }
            Some(changes)
        }

        fn clear_inspection_changes(&mut self, recording: LiveInspectionRecording) -> bool {
            self.ui.clear_inspection_changes(recording)
        }
    }

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
        assert_eq!(
            devtools
                .session()
                .with(|session| session.snapshot().inspection().clone()),
            ui.inspection_snapshot()
        );
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
        assert_eq!(
            devtools
                .session()
                .with(|session| session.snapshot().inspection().clone()),
            ui.inspection_snapshot()
        );

        devtools.detach_from_live(&mut ui);
        assert!(!ui.is_live_inspection_recording());
    }

    #[test]
    fn live_devtools_attachments_receive_changes_independently() {
        let mut ui = Ui::new();
        let root = ui.root();
        let mut first =
            LiveDevtools::capture_from_live(&mut ui, None).expect("first devtools should attach");
        let mut second =
            LiveDevtools::capture_from_live(&mut ui, None).expect("second devtools should attach");

        let child = ui.append(root, Panel::new());
        first
            .refresh_from_live(&mut ui)
            .expect("first devtools should receive the insertion");
        second
            .refresh_from_live(&mut ui)
            .expect("second devtools should receive the same insertion");

        assert!(first.mirror().outline_key_for_element(child).is_some());
        assert!(second.mirror().outline_key_for_element(child).is_some());

        assert!(first.detach_from_live(&mut ui));
        assert!(ui.is_live_inspection_recording());
        ui.add_pseudo(child, PSEUDO_HOVER);
        second
            .refresh_from_live(&mut ui)
            .expect("the remaining devtools should continue receiving changes");
        assert!(second.session().with(|session| {
            session
                .snapshot()
                .element(child)
                .is_some_and(|row| row.state.hovered)
        }));
        assert!(second.detach_from_live(&mut ui));
        assert!(!ui.is_live_inspection_recording());
    }

    #[test]
    fn live_devtools_resyncs_instead_of_applying_a_revision_gap() {
        let mut target = CorruptingTarget {
            ui: Ui::new(),
            corrupt_next_batch: false,
        };
        let root = target.ui.root();
        let mut devtools =
            LiveDevtools::capture_from_live(&mut target, None).expect("devtools should attach");

        let child = target.ui.append(root, Panel::new());
        target.ui.set_frame_timings(overstory::UiFrameTimings {
            total: Duration::from_millis(12),
            ..overstory::UiFrameTimings::default()
        });
        target.corrupt_next_batch = true;
        let refresh = devtools
            .refresh_from_live(&mut target)
            .expect("a revision gap should trigger a full resync");

        assert!(refresh.resynced_from.is_some());
        assert!(devtools.mirror().outline_key_for_element(child).is_some());
        assert_eq!(
            devtools.mirror().summaries().frame.timings.total,
            Duration::from_millis(12)
        );
        assert_eq!(
            devtools
                .session()
                .with(|session| session.snapshot().inspection().clone()),
            target.ui.inspection_snapshot()
        );
    }

    #[test]
    fn live_devtools_converges_across_all_inspection_domains() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::new().logical_size(320.0, 180.0));
        let panel = ui.append(root, Panel::new());
        let count = ui.local_state(0_u32);
        let bound_label = ui.append(
            panel,
            Text::new("").bind_text(count.map(|count| alloc::format!("Count: {count}"))),
        );
        ui.flush_bindings();
        ui.compute_layout(Size::new(320.0, 180.0));
        let mut devtools =
            LiveDevtools::capture_from_live(&mut ui, Some(panel)).expect("devtools should attach");

        let opacity = ui.properties().opacity;
        let animation = ui
            .animate_property(
                panel,
                opacity,
                EffectClip::from_keyframes([
                    EffectKeyframe::new(0, 1.0_f32),
                    EffectKeyframe::new(100, 0.25_f32),
                ]),
            )
            .expect("the active effect should start");
        let custom_class = ui.class_id(".live-inspection");
        ui.add_class(panel, custom_class);
        assert!(count.set(&mut ui, 42));
        ui.flush_bindings();
        ui.set_theme(overstory::theme::light_theme());
        let cascade = overstory::theme::default_cascade(ui.properties());
        ui.set_cascade(cascade);
        ui.schedule_timeline_cue(50, 7);
        let child = ui.append(panel, Text::new("Revision-consistent"));
        ui.compute_layout(Size::new(320.0, 180.0));

        let refresh = devtools
            .refresh_from_live(&mut ui)
            .expect("all live domains should refresh together");
        let retained = devtools
            .session()
            .with(|session| session.snapshot().inspection().clone());

        assert!(refresh.resynced_from.is_none());
        assert!(retained.elements.iter().any(|row| row.id == child));
        assert!(retained.elements.iter().any(|row| {
            row.id == bound_label
                && row.properties.iter().any(|property| {
                    property.name == "Text"
                        && property.value
                            == overstory::inspection::ElementPropertyInspectionValue::Text(
                                alloc::string::String::from("Count: 42"),
                            )
                })
        }));
        assert!(retained.timeline.active_effects > 0);
        assert!(
            retained
                .timeline
                .animations
                .iter()
                .any(|row| row.id == animation)
        );
        assert_eq!(
            retained
                .style_vocabulary
                .class_id_by_name(".live-inspection"),
            Some(custom_class)
        );
        assert_eq!(retained, ui.inspection_snapshot());

        ui.set_now(200);
        let completed = devtools
            .refresh_from_live(&mut ui)
            .expect("animation completion should update every live domain");
        let retained = devtools
            .session()
            .with(|session| session.snapshot().inspection().clone());

        assert!(completed.resynced_from.is_none());
        assert_eq!(retained.timeline.active_effects, 0);
        assert_eq!(retained, ui.inspection_snapshot());
    }

    #[test]
    fn live_devtools_refresh_keeps_overlay_summaries_current() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::new().logical_size(320.0, 180.0));
        let button = ui.append(root, Button::new("Open"));
        ui.compute_layout(Size::new(320.0, 180.0));
        let mut devtools =
            LiveDevtools::capture_from_live(&mut ui, None).expect("devtools should attach");

        let popup = ui
            .open_popup(button, Popup::new(), |ui, popup| {
                ui.append(popup, Text::new("Inspectable overlay"));
            })
            .expect("popup should open");
        ui.compute_layout(Size::new(320.0, 180.0));
        let opened = devtools
            .refresh_from_live(&mut ui)
            .expect("opening should refresh live overlay state");

        assert!(opened.changes.overlays);
        assert_eq!(
            devtools
                .mirror()
                .summaries()
                .overlays
                .entries
                .iter()
                .map(|entry| entry.element)
                .collect::<Vec<_>>(),
            vec![popup]
        );
        assert_eq!(
            devtools
                .session()
                .with(|session| session.snapshot().inspection().clone()),
            ui.inspection_snapshot()
        );

        assert!(ui.close_popup(popup));
        let closed = devtools
            .refresh_from_live(&mut ui)
            .expect("closing should refresh live overlay state");
        assert!(closed.changes.overlays);
        assert!(devtools.mirror().summaries().overlays.entries.is_empty());
        assert_eq!(
            devtools
                .session()
                .with(|session| session.snapshot().inspection().clone()),
            ui.inspection_snapshot()
        );
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
        assert_eq!(
            refresh.changes,
            DevtoolsRefreshChanges {
                tree: false,
                details: false,
                overlays: false,
                frame: true,
            }
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
            refresh.changes,
            DevtoolsRefreshChanges {
                tree: false,
                details: true,
                overlays: false,
                frame: true,
            }
        );
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
    fn live_devtools_refresh_ignores_unselected_row_detail_changes() {
        let mut ui = Ui::new();
        let root = ui.root();
        let selected = ui.append(root, Button::new("Selected"));
        let other = ui.append(root, Button::new("Other"));
        let mut devtools = LiveDevtools::capture_from_live(&mut ui, Some(selected))
            .expect("devtools should attach");

        ui.add_pseudo(other, PSEUDO_HOVER);
        let refresh = devtools
            .refresh_from_live(&mut ui)
            .expect("live state refresh should update mirror");

        assert_eq!(
            refresh.event,
            Some(DevtoolsEvent::TargetChanged {
                selected: Some(selected),
            })
        );
        assert_eq!(
            refresh.changes,
            DevtoolsRefreshChanges {
                tree: false,
                details: false,
                overlays: false,
                frame: true,
            }
        );
        assert!(
            devtools
                .mirror()
                .selected_details()
                .is_some_and(|details| details.row.id == selected && !details.row.state.hovered)
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
