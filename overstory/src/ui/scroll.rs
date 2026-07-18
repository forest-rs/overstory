// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained scroll-target registration and command application.

use kurbo::Vec2;

use crate::channels::{ARRANGE, LAYOUT};
use crate::element::ElementId;
use crate::scroll_runtime::{
    ScrollCommandResult, ScrollInputSource, ScrollState, ScrollStateSnapshot, ScrollTarget,
};

use super::Ui;

impl Ui {
    /// Registers `target` as a scrollable viewport backed by `state`.
    ///
    /// Viewport-like widgets call this when appended so scrollbars,
    /// accessibility, and generic scroll helpers can all address the same
    /// scroll offset by element id.
    ///
    /// A target may be registered only once during its lifetime. Code that
    /// previously replaced a registration should retain and mutate the
    /// originally registered `ScrollState`; replacing it would split the
    /// widget and registry across different state.
    ///
    /// # Panics
    ///
    /// Panics if `target` is stale or already has a registered scroll state.
    pub fn register_scroll_target(&mut self, target: ElementId, state: ScrollState) {
        assert!(
            self.get(target).is_some(),
            "cannot register stale scroll target {target:?}"
        );
        self.scrolling
            .register_target(target, ScrollTarget::new(state, ARRANGE.into()));
    }

    pub(crate) fn register_virtual_scroll_target(&mut self, target: ElementId, state: ScrollState) {
        assert!(
            self.get(target).is_some(),
            "cannot register stale virtual scroll target {target:?}"
        );
        self.scrolling
            .register_target(target, ScrollTarget::new(state, LAYOUT.into()));
    }

    pub(crate) fn scroll_target(&self, target: ElementId) -> Option<ScrollTarget> {
        self.scrolling.target(target)
    }

    pub(crate) fn apply_scroll_command(
        &mut self,
        target: ElementId,
        offset: Vec2,
        source: ScrollInputSource,
    ) -> Option<ScrollCommandResult> {
        let target_state = self.scroll_target(target)?;
        let change = target_state.state().apply_offset(offset, source);
        if change.changed() {
            self.mark_channels(target, target_state.offset_channels());
        } else {
            self.record_element_tree_update(target);
        }
        Some(change)
    }

    /// Returns current metrics and command diagnostics for a scrollable target.
    #[must_use]
    pub fn scroll_state_snapshot(&self, target: ElementId) -> Option<ScrollStateSnapshot> {
        self.scroll_target(target)
            .map(|target| target.state().snapshot())
    }
}
