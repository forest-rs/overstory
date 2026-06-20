// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use understory_focus::Navigation;

use crate::element::ElementId;
use crate::event_runtime::{FocusChangeReason, RoutedEventKind};

use super::super::Ui;

impl Ui {
    /// Moves focus per `direction` (e.g. [`Navigation::Next`] for
    /// Tab, [`Navigation::Prev`] for Shift+Tab). Uses
    /// [`understory_focus::DefaultPolicy`] with `WrapMode::Scope` so
    /// focus cycles within the tree. Returns the new focused element.
    pub fn focus_navigate(&mut self, direction: Navigation) -> Option<ElementId> {
        self.geometry.commit();
        let root_node = self.box_for(self.root())?;
        // Resolve the current focus's node before we borrow `tree`
        // and `focus_buf` to build the FocusSpace.
        let current_node = self.focus.focused().and_then(|id| self.box_for(id));
        let new_node =
            self.focus
                .navigate(self.geometry.tree(), root_node, current_node, direction);
        let new_element = new_node.and_then(|n| self.node_to_element(n));
        self.set_keyboard_focus(new_element);
        new_element
    }
    pub(in crate::ui) fn dispatch_focus_change(
        &mut self,
        previous: Option<ElementId>,
        focused: Option<ElementId>,
        focus_visible: bool,
        reason: FocusChangeReason,
    ) {
        if let Some(previous) = previous
            && self.get(previous).is_some()
        {
            self.dispatch_focus_event(
                RoutedEventKind::FocusLost,
                previous,
                Some(previous),
                focused,
                focus_visible,
                reason,
            );
        }
        if let Some(focused_id) = focused
            && self.get(focused_id).is_some()
        {
            self.dispatch_focus_event(
                RoutedEventKind::FocusGained,
                focused_id,
                previous,
                Some(focused_id),
                focus_visible,
                reason,
            );
        }
    }
}
