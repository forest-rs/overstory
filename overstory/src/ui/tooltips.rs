// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tooltip attachment and active overlay lifecycle.

use crate::controls::tooltip::{Tooltip, append_overlay_tooltip};
use crate::element::ElementId;

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
            .tooltips
            .active()
            .is_some_and(|active| active.target == target);
        self.tooltips.set(target, tooltip);
        if replace_active {
            self.hide_active_tooltip();
            self.show_tooltip_for(target);
        } else {
            self.refresh_tooltip_from_focus();
        }
    }

    /// Removes any tooltip attached to `target`.
    pub fn clear_tooltip(&mut self, target: ElementId) {
        if let Some(active) = self.tooltips.remove_target(target)
            && self.get(active).is_some()
        {
            self.remove(active);
        }
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
        match (requested, self.tooltips.active()) {
            (Some(target), Some(active)) if active.target == target => false,
            (Some(target), _) => {
                let changed = self.hide_active_tooltip();
                self.show_tooltip_for(target) || changed
            }
            (None, Some(_)) => self.hide_active_tooltip(),
            (None, None) => false,
        }
    }

    pub(in crate::ui) fn remove_tooltip_state_for(&mut self, id: ElementId) {
        let Some(active) = self.tooltips.remove_target(id) else {
            return;
        };
        if active != id && self.get(active).is_some() {
            self.remove(active);
        }
    }

    fn tooltip_target_for(&self, start: ElementId) -> Option<ElementId> {
        let mut next = Some(start);
        while let Some(id) = next {
            if self.tooltips.has_tooltip(id) {
                return Some(id);
            }
            next = self.get(id).and_then(|element| element.parent());
        }
        None
    }

    fn show_tooltip_for(&mut self, target: ElementId) -> bool {
        let Some(tooltip) = self.tooltips.tooltip_for(target).cloned() else {
            return false;
        };
        let bounds = self.effective_bounds(target);
        let tooltip_element = append_overlay_tooltip(self, &tooltip, bounds);
        self.tooltips.set_active(target, tooltip_element);
        true
    }

    fn hide_active_tooltip(&mut self) -> bool {
        let Some(active) = self.tooltips.take_active() else {
            return false;
        };
        if self.get(active.element).is_some() {
            self.remove(active.element);
        }
        true
    }
}
