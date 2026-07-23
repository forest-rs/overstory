// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tooltip attachment and active overlay lifecycle.

use crate::controls::tooltip::{Tooltip, append_overlay_tooltip};
use crate::element::ElementId;
use crate::overlay::{OverlayAnchor, OverlayHost};

use super::Ui;

impl Ui {
    /// Attaches `tooltip` to `target`.
    ///
    /// The tooltip appears when `target` or one of its descendants is hovered
    /// or focused. Passing a stale target is a no-op.
    pub fn set_tooltip(&mut self, target: ElementId, tooltip: Tooltip) {
        if self.get(target).is_none() {
            return;
        }
        let replace_active = self
            .overlays
            .active_tooltip()
            .is_some_and(|active| active.target == target);
        self.overlays.set_tooltip(target, tooltip);
        if replace_active {
            self.hide_active_tooltip();
            self.show_tooltip_for(target);
        } else {
            self.refresh_tooltip_from_focus();
        }
    }

    /// Removes any tooltip attached to `target`.
    pub fn clear_tooltip(&mut self, target: ElementId) {
        let active = self.overlays.remove_tooltip(target);
        self.close_overlay_hosts(active);
        self.refresh_tooltip_from_focus();
    }

    pub(in crate::ui) fn refresh_tooltip_from_focus(&mut self) -> bool {
        let requested = self
            .hovered()
            .and_then(|hovered| self.tooltip_target_for(hovered))
            .or_else(|| {
                self.focused()
                    .and_then(|focused| self.tooltip_target_for(focused))
            });
        self.sync_tooltip(requested)
    }

    pub(in crate::ui) fn refresh_tooltip_from_pointer(&mut self) -> bool {
        let requested = self
            .hovered()
            .and_then(|hovered| self.tooltip_target_for(hovered));
        self.sync_tooltip(requested)
    }

    fn sync_tooltip(&mut self, requested: Option<ElementId>) -> bool {
        let active_overlay = self.overlays.topmost_non_tooltip();
        let requested = requested.filter(|target| {
            active_overlay.is_none_or(|active| self.is_descendant_or_self(*target, active.element))
        });
        match (requested, self.overlays.active_tooltip()) {
            (Some(target), Some(active)) if active.target == target => false,
            (Some(target), _) => {
                let changed = self.hide_active_tooltip();
                self.show_tooltip_for(target) || changed
            }
            (None, Some(_)) => self.hide_active_tooltip(),
            (None, None) => false,
        }
    }

    fn tooltip_target_for(&self, start: ElementId) -> Option<ElementId> {
        let mut next = Some(start);
        while let Some(id) = next {
            if self.overlays.has_tooltip(id) {
                return Some(id);
            }
            next = self.get(id).and_then(|element| element.parent());
        }
        None
    }

    fn show_tooltip_for(&mut self, target: ElementId) -> bool {
        if !self.is_overlay_anchor_eligible(target) {
            return false;
        }
        let Some(tooltip) = self.overlays.tooltip_for(target).cloned() else {
            return false;
        };
        let placement = tooltip.placement_value();
        let offset = tooltip.offset_value();
        let tooltip_element = append_overlay_tooltip(self, &tooltip, kurbo::Point::ZERO);
        self.sync_overlay_direction(target, tooltip_element);
        self.overlays.open_tooltip(OverlayHost {
            behavior: understory_overlay::OverlayBehavior::Tooltip,
            target,
            element: tooltip_element,
            content: tooltip_element,
            anchor: OverlayAnchor::TargetBounds,
            placement,
            offset,
            restore_focus: None,
        });
        self.refresh_overlay_frame();
        true
    }

    fn hide_active_tooltip(&mut self) -> bool {
        let active = self.overlays.take_tooltip();
        self.close_overlay_hosts(active)
    }
}
