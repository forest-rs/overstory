// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Explicit anchored popup lifecycle.

use kurbo::Size;

use crate::controls::popup::{Popup, append_overlay_popup};
use crate::element::ElementId;
use crate::overlay::{ActiveOverlay, OverlayKind};

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
        let origin = placement.resolve(bounds, Size::ZERO, self.last_viewport, offset);
        let popup_element = append_overlay_popup(self, &popup, origin);
        build(self, popup_element);
        self.overlays.open(ActiveOverlay {
            kind: OverlayKind::Popup,
            target,
            element: popup_element,
            placement,
            offset,
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
        if self.get(active.element).is_some() {
            self.remove(active.element);
        }
        true
    }
}
