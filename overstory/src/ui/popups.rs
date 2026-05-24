// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Explicit anchored popup lifecycle.

use kurbo::Size;
use ui_events::keyboard::{Key, KeyState, KeyboardEvent, NamedKey};

use crate::controls::popup::{Popup, append_overlay_popup};
use crate::element::ElementId;
use crate::overlay::{ActiveOverlay, OverlayDismissal, OverlayKind};

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
        self.get(target)?;
        let bounds = self.effective_bounds(target);
        let placement = popup.placement_value();
        let offset = popup.offset_value();
        let restore_focus = if popup.restore_focus_on_close_value() {
            self.focused()
        } else {
            None
        };
        let origin = placement.resolve(bounds, Size::ZERO, self.last_viewport, offset);
        let popup_element = append_overlay_popup(self, &popup, origin);
        build(self, popup_element);
        self.overlays.open(ActiveOverlay {
            kind: OverlayKind::Popup,
            target,
            element: popup_element,
            placement,
            offset,
            dismissal: OverlayDismissal {
                outside_pointer: popup.dismiss_on_outside_pointer_value(),
                escape: popup.dismiss_on_escape_value(),
                restore_focus,
            },
        });
        Some(popup_element)
    }

    /// Closes a popup previously returned by [`Self::open_popup`].
    ///
    /// Returns `true` when `popup` was an active popup root.
    pub fn close_popup(&mut self, popup: ElementId) -> bool {
        let Some(active) = self.overlays.take_element_kind(popup, OverlayKind::Popup) else {
            return false;
        };
        self.close_popup_entry(active);
        true
    }

    pub(in crate::ui) fn dismiss_popup_for_escape(&mut self, event: &KeyboardEvent) -> bool {
        if event.state != KeyState::Down || !matches!(event.key, Key::Named(NamedKey::Escape)) {
            return false;
        }
        let Some(active) = self.overlays.active_popup() else {
            return false;
        };
        if !active.dismissal.escape {
            return false;
        }
        let Some(active) = self
            .overlays
            .take_element_kind(active.element, OverlayKind::Popup)
        else {
            return false;
        };
        self.close_popup_entry(active);
        true
    }

    pub(in crate::ui) fn dismiss_popups_for_pointer_down(
        &mut self,
        hit: Option<ElementId>,
    ) -> bool {
        let active = self
            .overlays
            .active_popups_top_to_bottom()
            .collect::<Vec<_>>();
        let mut to_close = Vec::new();
        for entry in active {
            if let Some(hit) = hit
                && (self.is_descendant_or_self(hit, entry.element)
                    || self.is_descendant_or_self(hit, entry.target))
            {
                break;
            }
            if !entry.dismissal.outside_pointer {
                break;
            }
            to_close.push(entry.element);
        }

        let mut changed = false;
        for popup in to_close {
            changed |= self.close_popup(popup);
        }
        changed
    }

    fn close_popup_entry(&mut self, active: ActiveOverlay) {
        let should_restore_focus = match (active.dismissal.restore_focus, self.focused()) {
            (Some(restore), Some(focused)) => {
                self.get(restore).is_some() && self.is_descendant_or_self(focused, active.element)
            }
            _ => false,
        };
        if self.get(active.element).is_some() {
            self.remove(active.element);
        }
        if should_restore_focus {
            let _ = self.set_focus(active.dismissal.restore_focus);
        }
    }
}
