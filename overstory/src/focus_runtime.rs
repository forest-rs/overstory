// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Focus state and keyboard focus navigation.
//!
//! Roadmap:
//! - Keep focus ownership in `Ui`, not in demos or platform hosts.
//! - Grow from one global focused element toward focus scopes for popups,
//!   menus, dialogs, and nested controls.
//! - Keep navigation policy backed by `understory_focus`, with Overstory
//!   providing element/geometry adapters and control-state integration.
//! - Make focus-visible, restore-focus, disabled state, capture, and routed
//!   focus events part of one predictable system.
//! - Add accessibility focus and keyboard navigation diagnostics before complex
//!   controls depend on implicit behavior.

use alloc::vec::Vec;

use hashbrown::HashMap;
use understory_box_tree::{NodeId, Tree};
use understory_focus::adapters::box_tree::build_focus_space_for_scope;
use understory_focus::{DefaultPolicy, FocusEntry, FocusPolicy, Navigation, WrapMode};

use crate::element::ElementId;

/// Focus state, focusability preferences, and focus navigation policy.
pub(crate) struct FocusRuntime {
    /// Currently focused element, if any.
    focused: Option<ElementId>,
    /// Construction-time focusability preference for each element.
    ///
    /// Disabled controls may be temporarily removed from focus traversal while
    /// preserving this preference so it can be restored when re-enabled.
    focusable_preferences: HashMap<ElementId, bool>,
    /// Policy used by keyboard focus navigation.
    focus_policy: DefaultPolicy,
    /// Scratch buffer reused when building an `understory_focus` focus space.
    focus_buf: Vec<FocusEntry<NodeId>>,
}

impl FocusRuntime {
    pub(crate) fn new() -> Self {
        Self {
            focused: None,
            focusable_preferences: HashMap::new(),
            focus_policy: DefaultPolicy {
                wrap: WrapMode::Scope,
            },
            focus_buf: Vec::new(),
        }
    }

    #[inline]
    pub(crate) fn focused(&self) -> Option<ElementId> {
        self.focused
    }

    #[inline]
    pub(crate) fn set_focused(&mut self, focused: Option<ElementId>) {
        self.focused = focused;
    }

    #[inline]
    pub(crate) fn take_focused(&mut self) -> Option<ElementId> {
        self.focused.take()
    }

    pub(crate) fn set_focusable_preference(&mut self, id: ElementId, focusable: bool) {
        self.focusable_preferences.insert(id, focusable);
    }

    pub(crate) fn focusable_preference(&self, id: ElementId) -> bool {
        self.focusable_preferences
            .get(&id)
            .copied()
            .unwrap_or(false)
    }

    pub(crate) fn remove_target(&mut self, id: ElementId) {
        self.focusable_preferences.remove(&id);
        if self.focused == Some(id) {
            self.focused = None;
        }
    }

    pub(crate) fn navigate(
        &mut self,
        tree: &Tree,
        root_node: NodeId,
        current_node: Option<NodeId>,
        direction: Navigation,
    ) -> Option<NodeId> {
        let space = build_focus_space_for_scope(tree, root_node, &(), &mut self.focus_buf);
        match current_node {
            Some(current) => self.focus_policy.next(current, direction, &space),
            None => self.focus_policy.initial::<NodeId>(direction, &space),
        }
    }
}
