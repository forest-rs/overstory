// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Attached context-menu invocation and root overlay lifetime.

use kurbo::Point;
use understory_overlay::OverlayBehavior;

use crate::controls::menu::{
    ContextMenu, ContextMenuRequest, ContextMenuSource, Menu, open_context_menu_surface,
};
use crate::element::ElementId;

use super::Ui;

impl Ui {
    /// Attaches `context_menu` to `target`.
    ///
    /// Secondary pointer presses originating in `target`'s retained subtree,
    /// keyboard context-menu requests from focused descendants, and AccessKit
    /// `ShowContextMenu` actions can invoke the attachment. Passing a stale
    /// target is a no-op.
    pub fn set_context_menu(&mut self, target: ElementId, context_menu: ContextMenu) {
        if self.get(target).is_none() {
            return;
        }
        self.context_menus.insert(target, context_menu);
        self.record_element_tree_update(target);
    }

    /// Removes the context-menu attachment from `target` and closes its active
    /// menu tree, if any.
    pub fn clear_context_menu(&mut self, target: ElementId) {
        if self.context_menus.remove(target).is_some() {
            let _ = self.close_context_menu(target);
            self.record_element_tree_update(target);
        }
    }

    /// Returns whether `target` has a context-menu attachment.
    #[must_use]
    pub fn has_context_menu(&self, target: ElementId) -> bool {
        self.context_menus.get(target).is_some()
    }

    /// Opens `menu` at a scene-space point with lifetime tied to `target`.
    ///
    /// This is the direct retained-core path for custom invocation policies.
    /// Ordinary secondary-click, keyboard, and accessibility invocation should
    /// use [`Self::set_context_menu`] or [`crate::ContextMenuSpecExt`].
    pub fn open_context_menu_at(
        &mut self,
        target: ElementId,
        position: Point,
        menu: Menu,
    ) -> Option<ElementId> {
        if !position.is_finite() || !self.is_overlay_anchor_eligible(target) {
            return None;
        }
        self.close_active_context_menu();
        open_context_menu_surface(self, target, position, menu)
    }

    /// Closes the active context-menu tree owned by `target`.
    pub fn close_context_menu(&mut self, target: ElementId) -> bool {
        let Some(popup) = self.active_overlay_for(target, OverlayBehavior::ContextMenu) else {
            return false;
        };
        self.close_overlay_tree(popup)
    }

    pub(in crate::ui) fn request_pointer_context_menu(
        &mut self,
        invoked_on: ElementId,
        position: Point,
    ) -> bool {
        self.request_attached_context_menu(invoked_on, position, ContextMenuSource::Pointer)
    }

    pub(in crate::ui) fn request_keyboard_context_menu(&mut self, invoked_on: ElementId) -> bool {
        let bounds = self.effective_bounds(invoked_on);
        self.request_attached_context_menu(
            invoked_on,
            Point::new(bounds.x0, bounds.y1),
            ContextMenuSource::Keyboard,
        )
    }

    pub(crate) fn request_accessible_context_menu(&mut self, owner: ElementId) -> bool {
        let bounds = self.effective_bounds(owner);
        self.invoke_context_menu(
            owner,
            owner,
            Point::new(bounds.x0, bounds.y1),
            ContextMenuSource::Accessibility,
        )
    }

    fn request_attached_context_menu(
        &mut self,
        invoked_on: ElementId,
        position: Point,
        source: ContextMenuSource,
    ) -> bool {
        let mut current = Some(invoked_on);
        while let Some(owner) = current {
            let parent = self.get(owner).and_then(crate::Element::parent);
            if self.context_menus.get(owner).is_some()
                && self.invoke_context_menu(owner, invoked_on, position, source)
            {
                return true;
            }
            current = parent;
        }
        false
    }

    fn invoke_context_menu(
        &mut self,
        owner: ElementId,
        invoked_on: ElementId,
        position: Point,
        source: ContextMenuSource,
    ) -> bool {
        if !position.is_finite() || !self.is_overlay_anchor_eligible(owner) {
            return false;
        }
        let Some(context_menu) = self.context_menus.get(owner).cloned() else {
            return false;
        };
        let request = ContextMenuRequest {
            owner,
            invoked_on,
            position,
            source,
        };
        let Some(menu) = context_menu.build(self, request) else {
            return false;
        };
        if self.context_menus.get(owner).is_none() || !self.is_overlay_anchor_eligible(owner) {
            return false;
        }
        self.open_context_menu_at(owner, position, menu).is_some()
    }

    fn close_active_context_menu(&mut self) -> bool {
        let Some(active) = self
            .overlays
            .active_for_behavior(OverlayBehavior::ContextMenu)
        else {
            return false;
        };
        self.close_overlay_tree(active.element)
    }
}
