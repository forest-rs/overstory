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
        let scope = self
            .overlays
            .topmost_containing_focus_scope()
            .map_or_else(|| self.root(), |host| host.element);
        let current = self
            .focus
            .focused()
            .filter(|focused| self.is_descendant_or_self(*focused, scope));
        let new_element = self.focus_in_scope(scope, current, direction);
        self.set_keyboard_focus(new_element);
        new_element
    }

    pub(crate) fn focus_scope_edge(
        &mut self,
        scope: ElementId,
        direction: Navigation,
    ) -> Option<ElementId> {
        self.focus_scope_from(scope, None, direction)
    }

    pub(crate) fn focus_scope_from(
        &mut self,
        scope: ElementId,
        current: Option<ElementId>,
        direction: Navigation,
    ) -> Option<ElementId> {
        let new_element = self.focus_in_scope(scope, current, direction);
        self.set_keyboard_focus(new_element);
        new_element
    }

    fn focus_in_scope(
        &mut self,
        scope: ElementId,
        current: Option<ElementId>,
        direction: Navigation,
    ) -> Option<ElementId> {
        self.geometry.commit();
        let root_node = self.box_for(scope)?;
        // Resolve the current focus's node before we borrow `tree`
        // and `focus_buf` to build the FocusSpace.
        let current_node = current.and_then(|id| self.box_for(id));
        let new_node =
            self.focus
                .navigate(self.geometry.tree(), root_node, current_node, direction);
        new_node.and_then(|node| self.node_to_element(node))
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
