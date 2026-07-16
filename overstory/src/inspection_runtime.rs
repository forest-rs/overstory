// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Runtime accumulator for live inspection changes.
//!
//! This runtime owns only the short-lived change batch between drains. It does
//! not own snapshot capture, devtools protocol encoding, rendering, or host
//! transport.

use alloc::vec::Vec;

use hashbrown::HashSet;

use crate::diagnostics::UiFrameStats;
use crate::element::ElementId;
use crate::inspection::ElementInspectionRemoval;

#[derive(Debug, Default)]
pub(crate) struct InspectionChangeRuntime {
    recording_depth: u32,
    tree: ElementTreeChangeRuntime,
    overlays_dirty: bool,
    frame_dirty: bool,
}

impl InspectionChangeRuntime {
    pub(crate) fn begin_recording(&mut self) {
        self.recording_depth = self.recording_depth.saturating_add(1);
    }

    pub(crate) fn end_recording(&mut self) {
        self.recording_depth = self.recording_depth.saturating_sub(1);
        if !self.is_recording() {
            self.clear();
        }
    }

    pub(crate) const fn is_recording(&self) -> bool {
        self.recording_depth > 0
    }

    pub(crate) fn record_tree_upsert(&mut self, id: ElementId) {
        if !self.is_recording() {
            return;
        }
        self.tree.record_upsert(id);
    }

    pub(crate) fn record_tree_update(&mut self, id: ElementId) {
        if !self.is_recording() {
            return;
        }
        self.tree.record_update(id);
    }

    pub(crate) fn record_tree_removal(&mut self, removal: ElementInspectionRemoval) {
        if !self.is_recording() {
            return;
        }
        self.tree.record_removal(removal);
    }

    pub(crate) fn record_frame(&mut self) {
        if !self.is_recording() {
            return;
        }
        self.frame_dirty = true;
    }

    pub(crate) fn record_overlays(&mut self) {
        if self.is_recording() {
            self.overlays_dirty = true;
        }
    }

    pub(crate) fn drain(&mut self, frame: UiFrameStats) -> PendingInspectionChanges {
        let changes = PendingInspectionChanges {
            removed: core::mem::take(&mut self.tree.removed),
            upserted: self.tree.drain_upserts(),
            updated: self.tree.drain_updates(),
            overlays_dirty: self.overlays_dirty,
            frame: self.frame_dirty.then_some(frame),
        };
        self.overlays_dirty = false;
        self.frame_dirty = false;
        changes
    }

    pub(crate) fn clear(&mut self) {
        self.tree.clear();
        self.overlays_dirty = false;
        self.frame_dirty = false;
    }
}

#[derive(Debug, Default)]
pub(crate) struct PendingInspectionChanges {
    pub(crate) removed: Vec<ElementInspectionRemoval>,
    pub(crate) upserted: Vec<ElementId>,
    pub(crate) updated: Vec<ElementId>,
    pub(crate) overlays_dirty: bool,
    pub(crate) frame: Option<UiFrameStats>,
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

    fn drain_upserts(&mut self) -> Vec<ElementId> {
        self.upserted_set.clear();
        core::mem::take(&mut self.upserted)
    }

    fn drain_updates(&mut self) -> Vec<ElementId> {
        self.updated_set.clear();
        core::mem::take(&mut self.updated)
    }

    fn clear(&mut self) {
        self.removed.clear();
        self.upserted.clear();
        self.upserted_set.clear();
        self.updated.clear();
        self.updated_set.clear();
    }
}
