// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Runtime accumulator for live inspection changes.
//!
//! This runtime owns only recording identities, revisions, and the short-lived
//! change batches between drains. It does not own snapshot capture, devtools
//! protocol encoding, rendering, or host transport.

use alloc::vec::Vec;

use hashbrown::HashSet;

use crate::element::ElementId;
use crate::inspection::{ElementInspectionRemoval, InspectionRevision, LiveInspectionRecording};

#[derive(Debug, Default)]
pub(crate) struct InspectionChangeRuntime {
    revision: InspectionRevision,
    timeline_revision: u64,
    next_recording: u64,
    recordings: Vec<InspectionRecordingRuntime>,
}

impl InspectionChangeRuntime {
    pub(crate) fn begin_recording(&mut self) -> LiveInspectionRecording {
        let id = self.next_recording;
        self.next_recording = self
            .next_recording
            .checked_add(1)
            .expect("live inspection recording ids exhausted");
        let recording = LiveInspectionRecording::new(id);
        self.recordings.push(InspectionRecordingRuntime {
            recording,
            baseline_revision: self.revision,
            changes: PendingInspectionState::default(),
        });
        recording
    }

    pub(crate) fn end_recording(&mut self, recording: LiveInspectionRecording) -> bool {
        let Some(index) = self.recording_index(recording) else {
            return false;
        };
        self.recordings.remove(index);
        true
    }

    pub(crate) fn is_recording(&self) -> bool {
        !self.recordings.is_empty()
    }

    pub(crate) fn projected_revision(&self, timeline_revision: u64) -> InspectionRevision {
        if !self.recordings.is_empty() && self.timeline_revision != timeline_revision {
            self.revision.next()
        } else {
            self.revision
        }
    }

    pub(crate) fn observe_timeline_revision(&mut self, timeline_revision: u64) {
        if self.timeline_revision == timeline_revision {
            return;
        }
        self.timeline_revision = timeline_revision;
        self.record_change();
    }

    pub(crate) fn record_tree_upsert(&mut self, id: ElementId) {
        if !self.advance_revision() {
            return;
        }
        for recording in &mut self.recordings {
            recording.changes.tree.record_upsert(id);
        }
    }

    pub(crate) fn record_tree_update(&mut self, id: ElementId) {
        if !self.advance_revision() {
            return;
        }
        for recording in &mut self.recordings {
            recording.changes.tree.record_update(id);
        }
    }

    pub(crate) fn record_tree_removal(&mut self, removal: ElementInspectionRemoval) {
        if !self.advance_revision() {
            return;
        }
        for recording in &mut self.recordings {
            recording.changes.tree.record_removal(removal);
        }
    }

    pub(crate) fn record_change(&mut self) {
        if !self.advance_revision() {
            return;
        }
        for recording in &mut self.recordings {
            recording.changes.dirty = true;
        }
    }

    pub(crate) fn drain(
        &mut self,
        recording: LiveInspectionRecording,
    ) -> Option<PendingInspectionChanges> {
        let revision = self.revision;
        let index = self.recording_index(recording)?;
        let recording = &mut self.recordings[index];
        let changes = core::mem::take(&mut recording.changes);
        let pending = PendingInspectionChanges {
            baseline_revision: recording.baseline_revision,
            revision,
            removed: changes.tree.removed,
            upserted: changes.tree.upserted,
            updated: changes.tree.updated,
            dirty: changes.dirty,
        };
        recording.baseline_revision = revision;
        Some(pending)
    }

    pub(crate) fn clear(&mut self, recording: LiveInspectionRecording) -> bool {
        let revision = self.revision;
        let Some(index) = self.recording_index(recording) else {
            return false;
        };
        let recording = &mut self.recordings[index];
        recording.changes.clear();
        recording.baseline_revision = revision;
        true
    }

    fn advance_revision(&mut self) -> bool {
        if self.recordings.is_empty() {
            return false;
        }
        self.revision = self.revision.next();
        true
    }

    fn recording_index(&self, recording: LiveInspectionRecording) -> Option<usize> {
        self.recordings
            .iter()
            .position(|candidate| candidate.recording == recording)
    }
}

#[derive(Debug)]
struct InspectionRecordingRuntime {
    recording: LiveInspectionRecording,
    baseline_revision: InspectionRevision,
    changes: PendingInspectionState,
}

#[derive(Debug, Default)]
struct PendingInspectionState {
    tree: ElementTreeChangeRuntime,
    dirty: bool,
}

impl PendingInspectionState {
    fn clear(&mut self) {
        self.tree.clear();
        self.dirty = false;
    }
}

#[derive(Debug, Default)]
pub(crate) struct PendingInspectionChanges {
    pub(crate) baseline_revision: InspectionRevision,
    pub(crate) revision: InspectionRevision,
    pub(crate) removed: Vec<ElementInspectionRemoval>,
    pub(crate) upserted: Vec<ElementId>,
    pub(crate) updated: Vec<ElementId>,
    pub(crate) dirty: bool,
}

impl PendingInspectionChanges {
    pub(crate) fn is_empty(&self) -> bool {
        self.removed.is_empty()
            && self.upserted.is_empty()
            && self.updated.is_empty()
            && !self.dirty
    }
}

#[derive(Debug, Default)]
struct ElementTreeChangeRuntime {
    removed: Vec<ElementInspectionRemoval>,
    upserted: Vec<ElementId>,
    upserted_set: HashSet<ElementId>,
    updated: Vec<ElementId>,
    updated_set: HashSet<ElementId>,
}

impl ElementTreeChangeRuntime {
    fn record_upsert(&mut self, id: ElementId) {
        if self.removed.iter().any(|removal| removal.id == id) {
            return;
        }
        if self.updated_set.remove(&id) {
            self.updated.retain(|updated| *updated != id);
        }
        if self.upserted_set.insert(id) {
            self.upserted.push(id);
        }
    }

    fn record_update(&mut self, id: ElementId) {
        if self.removed.iter().any(|removal| removal.id == id) || self.upserted_set.contains(&id) {
            return;
        }
        if self.updated_set.insert(id) {
            self.updated.push(id);
        }
    }

    fn record_removal(&mut self, removal: ElementInspectionRemoval) {
        if self.upserted_set.remove(&removal.id) {
            self.upserted.retain(|id| *id != removal.id);
        }
        if self.updated_set.remove(&removal.id) {
            self.updated.retain(|id| *id != removal.id);
        }
        if !self
            .removed
            .iter()
            .any(|existing| existing.id == removal.id)
        {
            self.removed.push(removal);
        }
    }

    fn clear(&mut self) {
        self.removed.clear();
        self.upserted.clear();
        self.upserted_set.clear();
        self.updated.clear();
        self.updated_set.clear();
    }
}
