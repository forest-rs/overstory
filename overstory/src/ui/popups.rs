// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Explicit anchored popup lifecycle.

use alloc::vec::Vec;

use kurbo::{Point, Size};
use ui_events::keyboard::{Key, KeyState, KeyboardEvent, NamedKey};
use understory_overlay::{OverlayEvent, OverlayKey};

use crate::controls::popup::{Popup, append_overlay_popup};
use crate::element::ElementId;
use crate::overlay::{OverlayHost, resolve_placement};

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
        let origin = resolve_placement(placement, bounds, Size::ZERO, self.last_viewport, offset);
        let popup_element = append_overlay_popup(self, &popup, origin);
        build(self, popup_element);
        self.overlays.open_popup(
            OverlayHost {
                target,
                element: popup_element,
                placement,
                offset,
                restore_focus,
            },
            popup.dismiss_on_outside_pointer_value(),
            popup.dismiss_on_escape_value(),
        );
        Some(popup_element)
    }

    /// Closes a popup previously returned by [`Self::open_popup`].
    ///
    /// Returns `true` when `popup` was an active popup root.
    pub fn close_popup(&mut self, popup: ElementId) -> bool {
        let active = self.overlays.close_popup(popup);
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
        let should_restore_focus = match (active.restore_focus, self.focused()) {
            (Some(restore), Some(focused)) => {
                self.get(restore).is_some() && self.is_descendant_or_self(focused, active.element)
            }
            _ => false,
        };
        if self.get(active.element).is_some() {
            self.remove(active.element);
        }
        if should_restore_focus {
            let _ = self.set_focus(active.restore_focus);
        }
    }

    pub(in crate::ui) fn close_overlay_hosts(&mut self, active: Vec<OverlayHost>) -> bool {
        let changed = !active.is_empty();
        for host in active.into_iter().rev() {
            self.close_overlay_host(host);
        }
        changed
    }
}
