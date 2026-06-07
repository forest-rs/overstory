// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Scroll target registration.

use crate::channels::{ARRANGE, LAYOUT};
use crate::element::ElementId;
use crate::scroll_runtime::{ScrollState, ScrollTarget};

use super::Ui;

impl Ui {
    /// Registers `target` as a scrollable viewport backed by `state`.
    ///
    /// Viewport-like widgets call this when appended so scrollbars,
    /// accessibility, and generic scroll helpers can all address the same
    /// scroll offset by element id. Registering a new state for the same target
    /// replaces the previous one.
    pub fn register_scroll_target(&mut self, target: ElementId, state: ScrollState) {
        self.scrolling
            .register_target(target, ScrollTarget::new(state, ARRANGE.into()));
    }

    pub(crate) fn register_virtual_scroll_target(&mut self, target: ElementId, state: ScrollState) {
        self.scrolling
            .register_target(target, ScrollTarget::new(state, LAYOUT.into()));
    }

    pub(crate) fn scroll_target(&self, target: ElementId) -> Option<ScrollTarget> {
        self.scrolling.target(target)
    }
}
