// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Deferred application callbacks for settled semantic control changes.
//!
//! Widget hooks may temporarily remove their widget from the retained element
//! while toolkit behavior runs. This runtime keeps application callbacks out
//! of that extraction window and gives nested semantic changes one FIFO drain.

use alloc::boxed::Box;
use alloc::collections::VecDeque;

use crate::element::ElementId;
use crate::ui::Ui;

pub(crate) type SemanticCallback = Box<dyn FnOnce(&mut Ui) + 'static>;

pub(crate) struct PendingSemanticCallback {
    pub(crate) source: ElementId,
    pub(crate) callback: SemanticCallback,
}

/// Maximum number of callbacks one outer semantic operation may dispatch.
///
/// Reaching this budget indicates a callback cycle rather than ordinary nested
/// control behavior. Panicking is intentional: silently retaining callbacks
/// would make a synchronous retained operation appear to have completed.
pub(crate) const SEMANTIC_CALLBACK_BUDGET: usize = 1_024;

#[derive(Default)]
pub(crate) struct SemanticCallbackRuntime {
    queue: VecDeque<PendingSemanticCallback>,
    boundary_depth: usize,
    draining: bool,
}

impl SemanticCallbackRuntime {
    pub(crate) fn begin_boundary(&mut self) {
        self.boundary_depth = self
            .boundary_depth
            .checked_add(1)
            .expect("semantic callback boundary depth overflowed");
    }

    pub(crate) fn end_boundary(&mut self) {
        self.boundary_depth = self
            .boundary_depth
            .checked_sub(1)
            .expect("semantic callback boundary ended without a matching begin");
    }

    pub(crate) fn enqueue(&mut self, source: ElementId, callback: SemanticCallback) {
        self.queue
            .push_back(PendingSemanticCallback { source, callback });
    }

    pub(crate) fn pop_front(&mut self) -> Option<PendingSemanticCallback> {
        self.queue.pop_front()
    }

    pub(crate) fn clear(&mut self) {
        self.queue.clear();
    }

    pub(crate) fn is_ready_to_drain(&self) -> bool {
        self.boundary_depth == 0 && !self.draining && !self.queue.is_empty()
    }

    pub(crate) fn set_draining(&mut self, draining: bool) {
        self.draining = draining;
    }

    pub(crate) fn pending_len(&self) -> usize {
        self.queue.len()
    }
}
