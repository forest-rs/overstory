// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Explicit anchored popup lifecycle.

use alloc::vec::Vec;

use kurbo::Point;
use ui_events::keyboard::{Key, KeyState, KeyboardEvent, NamedKey};
use understory_focus::Navigation;
use understory_overlay::{FocusPolicy, OverlayBehavior, OverlayEvent, OverlayKey};

use crate::controls::popup::{Popup, append_overlay_popup};
use crate::element::ElementId;
use crate::overlay::{OverlayAnchor, OverlayHost};

use super::Ui;

impl Ui {
    /// Opens an anchored popup under the overlay root.
    ///
    /// `build` receives the popup root element and should append the popup's
    /// visible content below it. The returned element id is the popup root and
    /// can be passed to [`Self::close_popup`].
    pub fn open_popup<F>(&mut self, target: ElementId, popup: Popup, build: F) -> Option<ElementId>
    where
        F: FnOnce(&mut Self, ElementId),
    {
        self.open_popup_with_behavior(target, popup, OverlayBehavior::Popover, build)
    }

    pub(crate) fn open_popup_with_behavior<F>(
        &mut self,
        target: ElementId,
        popup: Popup,
        behavior: OverlayBehavior,
        build: F,
    ) -> Option<ElementId>
    where
        F: FnOnce(&mut Self, ElementId),
    {
        self.open_semantic_popup_with_behavior(target, None, popup, behavior, |ui, popup| {
            build(ui, popup);
            popup
        })
    }

    pub(crate) fn open_semantic_popup_with_behavior<F>(
        &mut self,
        target: ElementId,
        parent: Option<ElementId>,
        popup: Popup,
        behavior: OverlayBehavior,
        build: F,
    ) -> Option<ElementId>
    where
        F: FnOnce(&mut Self, ElementId) -> ElementId,
    {
        self.open_semantic_popup_with_anchor(
            target,
            parent,
            OverlayAnchor::TargetBounds,
            popup,
            behavior,
            build,
        )
    }

    pub(crate) fn open_semantic_popup_with_anchor<F>(
        &mut self,
        target: ElementId,
        parent: Option<ElementId>,
        anchor: OverlayAnchor,
        popup: Popup,
        behavior: OverlayBehavior,
        build: F,
    ) -> Option<ElementId>
    where
        F: FnOnce(&mut Self, ElementId) -> ElementId,
    {
        if !self.is_overlay_anchor_eligible(target)
            || parent.is_some_and(|parent| self.overlays.active_for_element(parent).is_none())
        {
            return None;
        }
        let placement = popup.placement_value();
        let offset = popup.offset_value();
        let restore_focus = if popup.restore_focus_on_close_value() {
            self.focused()
        } else {
            None
        };
        let popup_element = append_overlay_popup(self, &popup, Point::ZERO);
        self.sync_overlay_direction(target, popup_element);
        let content = build(self, popup_element);
        if self.get(content).is_none() || !self.is_descendant_or_self(content, popup_element) {
            if self.get(popup_element).is_some() {
                self.remove(popup_element);
            }
            return None;
        }
        self.overlays.open_popup(
            OverlayHost {
                behavior,
                target,
                element: popup_element,
                content,
                anchor,
                placement,
                offset,
                restore_focus,
            },
            parent,
            popup.dismiss_on_outside_pointer_value(),
            popup.dismiss_on_escape_value(),
            popup_focus_policy(behavior, restore_focus.is_some()),
        );
        if behavior == OverlayBehavior::Submenu {
            self.add_pseudo(target, crate::pseudos::PSEUDO_EXPANDED);
        }
        self.refresh_overlay_frame();
        // A popup supersedes tooltip requests outside its retained subtree.
        let _ = self.refresh_tooltip_from_focus();
        Some(popup_element)
    }

    /// Closes a popup previously returned by [`Self::open_popup`].
    ///
    /// Returns `true` when `popup` was an active popup root.
    pub fn close_popup(&mut self, popup: ElementId) -> bool {
        let active = self.overlays.close_popup(popup);
        self.close_overlay_hosts(active)
    }

    pub(crate) fn close_overlay_descendants(&mut self, popup: ElementId) -> bool {
        let active = self.overlays.close_descendants(popup);
        self.close_overlay_hosts(active)
    }

    pub(crate) fn close_overlay_tree(&mut self, popup: ElementId) -> bool {
        let active = self.overlays.close_tree(popup);
        self.close_overlay_hosts(active)
    }

    pub(in crate::ui) fn dismiss_popup_for_escape(&mut self, event: &KeyboardEvent) -> bool {
        if event.state != KeyState::Down || !matches!(event.key, Key::Named(NamedKey::Escape)) {
            return false;
        }
        self.apply_overlay_event(OverlayEvent::KeyDown {
            key: OverlayKey::Escape,
        })
    }

    pub(in crate::ui) fn dismiss_popups_for_pointer_down(&mut self, point: Point) -> bool {
        self.apply_overlay_event(OverlayEvent::PointerDown { point })
    }

    pub(in crate::ui) fn close_overlay_host(&mut self, active: OverlayHost) {
        self.release_overlay_host_state(active);
        if self.get(active.element).is_some() {
            self.remove(active.element);
        }
    }

    pub(in crate::ui) fn release_overlay_host_state(&mut self, active: OverlayHost) {
        let focused_inside = self
            .focused()
            .is_some_and(|focused| self.is_descendant_or_self(focused, active.element));
        if focused_inside {
            let restored = active
                .restore_focus
                .filter(|target| self.is_enabled(*target) && self.is_effectively_visible(*target))
                .is_some_and(|target| {
                    self.set_focus(Some(target)) || self.focused() == Some(target)
                });
            if !restored {
                let repaired = active.restore_focus.is_some_and(|target| {
                    let Some(scope) = self.overlays.topmost_containing_focus_scope() else {
                        return false;
                    };
                    self.is_descendant_or_self(target, scope.element)
                        && self
                            .focus_scope_from(scope.element, Some(target), Navigation::Next)
                            .is_some()
                });
                if !repaired {
                    let _ = self.set_focus(None);
                }
            }
        }
        if active.behavior == OverlayBehavior::Submenu {
            self.remove_pseudo(active.target, crate::pseudos::PSEUDO_EXPANDED);
        }
    }

    pub(in crate::ui) fn close_overlay_hosts(&mut self, active: Vec<OverlayHost>) -> bool {
        let changed = !active.is_empty();
        if self.overlays.frame_needs_refresh() {
            self.refresh_overlay_frame();
        }
        for host in active.into_iter().rev() {
            self.close_overlay_host(host);
        }
        changed
    }
}

fn popup_focus_policy(behavior: OverlayBehavior, restore: bool) -> FocusPolicy {
    // A Select keeps real focus on its owner and projects the active option to
    // its listbox. It therefore has no contained focus scope even though
    // keyboard-driven menu-family overlays generally do.
    let contain = behavior != OverlayBehavior::ComboboxPopup
        && matches!(
            behavior.focus_policy(),
            FocusPolicy::Contain | FocusPolicy::ContainAndRestore
        );
    match (contain, restore) {
        (false, false) => FocusPolicy::None,
        (false, true) => FocusPolicy::RestoreOnClose,
        (true, false) => FocusPolicy::Contain,
        (true, true) => FocusPolicy::ContainAndRestore,
    }
}
