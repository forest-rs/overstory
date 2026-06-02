// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Runtime accumulator for live inspection changes.
//!
//! This runtime owns only the short-lived change batch between drains. It does
//! not own snapshot capture, devtools protocol encoding, rendering, or host
//! transport.

use alloc::vec::Vec;

use crate::element::ElementId;
use crate::inspection::{
    ElementInspectionRemoval, ElementInspectionRow, ElementTreeInspectionPatch, UiInspectionChanges,
};

#[derive(Debug, Default)]
pub(crate) struct InspectionChangeRuntime {
    tree: ElementTreeChangeRuntime,
}

impl InspectionChangeRuntime {
    pub(crate) fn record_tree_upsert(&mut self, row: ElementInspectionRow) {
        self.tree.record_upsert(row);
    }

    pub(crate) fn record_tree_removal(&mut self, removal: ElementInspectionRemoval) {
        self.tree.record_removal(removal);
    }

    pub(crate) fn drain(&mut self, root: ElementId) -> UiInspectionChanges {
        UiInspectionChanges {
            tree: self.tree.drain(root),
        }
    }

    pub(crate) fn clear(&mut self) {
        self.tree.clear();
    }
}

#[derive(Debug, Default)]
struct ElementTreeChangeRuntime {
    removed: Vec<ElementInspectionRemoval>,
    upserted: Vec<ElementInspectionRow>,
}

impl ElementTreeChangeRuntime {
    fn record_upsert(&mut self, row: ElementInspectionRow) {
        if self.removed.iter().any(|removal| removal.id == row.id) {
            return;
        }
        if let Some(existing) = self
            .upserted
            .iter_mut()
            .find(|existing| existing.id == row.id)
        {
            *existing = row;
        } else {
            self.upserted.push(row);
        }
    }

    fn record_removal(&mut self, removal: ElementInspectionRemoval) {
        self.upserted.retain(|row| row.id != removal.id);
        if !self
            .removed
            .iter()
            .any(|existing| existing.id == removal.id)
        {
            self.removed.push(removal);
        }
    }

    fn drain(&mut self, root: ElementId) -> ElementTreeInspectionPatch {
        ElementTreeInspectionPatch {
            root: Some(root),
            removed: core::mem::take(&mut self.removed),
            upserted: core::mem::take(&mut self.upserted),
        }
    }

    fn clear(&mut self) {
        self.removed.clear();
        self.upserted.clear();
    }
}
