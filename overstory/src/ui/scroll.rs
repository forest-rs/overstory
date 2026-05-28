// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Internal scroll target registration.

use crate::element::ElementId;
use crate::scroll_runtime::ScrollState;

use super::Ui;

impl Ui {
    pub(crate) fn register_scroll_target(&mut self, target: ElementId, state: ScrollState) {
        self.scrolling.register_target(target, state);
    }

    pub(crate) fn scroll_target_state(&self, target: ElementId) -> Option<ScrollState> {
        self.scrolling.target_state(target)
    }
}
