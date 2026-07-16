// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Overlay placement updates after layout has measured overlay content.

use alloc::vec::Vec;

use kurbo::{Point, Rect, Size};
use understory_overlay::{
    FocusTarget, OverlayBehavior, OverlayEvent, OverlayFrameInput, OverlayGeometry,
};

use crate::element::ElementId;
use crate::overlay::{OverlayHost, anchor_id, element_token};

use super::Ui;

impl Ui {
    pub(in crate::ui) fn remove_overlay_state_for(&mut self, id: ElementId) {
        let removed = self.overlays.remove_element(id);
        if self.overlays.frame_needs_refresh() {
            self.refresh_overlay_frame();
        }
        for host in removed.into_iter().rev() {
            if host.element == id {
                self.release_overlay_host_state(host);
            } else {
                self.close_overlay_host(host);
            }
        }
    }

    pub(in crate::ui) fn reposition_overlays(&mut self, viewport: Size) {
        let active = self.overlays.active_entries().collect::<Vec<_>>();
        for entry in active {
            if self.get(entry.target).is_none() || self.get(entry.element).is_none() {
                continue;
            }
            let target_bounds = self.effective_bounds(entry.target);
            let overlay_bounds = self.effective_bounds(entry.element);
            let rect = self.overlays.resolve_anchor_rect(
                entry.overlay_id(),
                target_bounds,
                overlay_bounds.size(),
                viewport,
            );
            let origin = rect.origin();
            if same_origin(overlay_bounds.origin(), origin) {
                continue;
            }
            self.arrange_overlay_at(entry.element, origin);
        }
    }

    pub(in crate::ui) fn refresh_overlay_frame(&mut self) {
        let active = self.overlays.active_entries().collect::<Vec<_>>();
        self.sync_overlay_hit_order(&active);
        let geometries = active
            .iter()
            .copied()
            .filter_map(|entry| {
                if self.get(entry.target).is_none() || self.get(entry.element).is_none() {
                    return None;
                }
                Some(
                    OverlayGeometry::new(entry.overlay_id(), self.effective_bounds(entry.element))
                        .with_anchor_rect(
                            entry
                                .anchor
                                .reference_rect(self.effective_bounds(entry.target)),
                        ),
                )
            })
            .collect::<Vec<_>>();
        let input = OverlayFrameInput::new(
            Rect::from_origin_size(Point::ZERO, self.last_viewport),
            &geometries,
        );
        self.overlays.rebuild_frame(input);
        self.inspection_changes.record_overlays();
    }

    fn sync_overlay_hit_order(&mut self, active: &[OverlayHost]) {
        // Box-tree hit testing compares z-index before tree depth. Apply the
        // overlay stack's order to each whole retained overlay subtree so deep
        // application content cannot receive input through a painted overlay.
        for (index, host) in active.iter().enumerate() {
            let z_index = i32::try_from(index.saturating_add(1)).unwrap_or(i32::MAX);
            let mut pending = Vec::from([host.element]);
            while let Some(element) = pending.pop() {
                self.geometry.set_z_index(element, z_index);
                if let Some(element) = self.get(element) {
                    pending.extend(element.children().iter().copied());
                }
            }
        }
    }

    pub(in crate::ui) fn update_overlay_pointer(&mut self, point: Point) {
        self.overlays.set_pointer(point);
    }

    pub(crate) fn begin_submenu_pointer_grace(&mut self, target: ElementId) {
        if self.overlays.begin_submenu_grace(target) {
            self.refresh_overlay_frame();
        }
    }

    pub(in crate::ui) fn apply_overlay_event(&mut self, event: OverlayEvent) -> bool {
        let operations = self.overlays.resolve_event(event).ops;
        let closed = operations
            .into_iter()
            .flat_map(|operation| self.overlays.apply_operation(operation))
            .collect::<Vec<_>>();
        self.close_overlay_hosts(closed)
    }

    pub(in crate::ui) fn apply_overlay_focus_change(&mut self, focused: Option<ElementId>) -> bool {
        let target = self.overlay_focus_target(focused);
        self.apply_overlay_event(OverlayEvent::FocusChanged { target })
    }

    pub(in crate::ui) fn dismiss_overlay_anchor(&mut self, target: ElementId) -> bool {
        self.apply_overlay_event(OverlayEvent::AnchorBlur {
            anchor: anchor_id(target),
        })
    }

    pub(in crate::ui) fn dismiss_ineligible_overlay_anchors(&mut self) -> bool {
        let targets = self
            .overlays
            .active_entries()
            .map(|host| host.target)
            .filter(|target| !self.is_overlay_anchor_eligible(*target))
            .collect::<Vec<_>>();
        let mut changed = false;
        for target in targets {
            changed |= self.dismiss_overlay_anchor(target);
        }
        changed
    }

    pub(in crate::ui) fn is_overlay_anchor_eligible(&self, target: ElementId) -> bool {
        self.get(target).is_some() && self.is_enabled(target) && self.is_effectively_visible(target)
    }

    pub(crate) fn active_overlay_for(
        &self,
        target: ElementId,
        behavior: OverlayBehavior,
    ) -> Option<ElementId> {
        self.overlays
            .active_for_target(target, behavior)
            .map(|host| host.element)
    }

    pub(crate) fn active_overlay_content_for(
        &self,
        target: ElementId,
        behavior: OverlayBehavior,
    ) -> Option<ElementId> {
        self.overlays
            .active_for_target(target, behavior)
            .map(|host| host.content)
    }

    pub(crate) fn containing_overlay_for(&self, descendant: ElementId) -> Option<ElementId> {
        self.overlays
            .active_entries()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .find(|host| self.is_descendant_or_self(descendant, host.element))
            .map(|host| host.element)
    }

    pub(crate) fn overlay_tree_anchor(&self, popup: ElementId) -> Option<ElementId> {
        self.overlays.root_of(popup).map(|host| host.target)
    }

    pub(crate) fn overlay_ancestor_targets(&self, descendant: ElementId) -> Vec<ElementId> {
        self.containing_overlay_for(descendant)
            .map_or_else(Vec::new, |popup| self.overlays.targets_to_root(popup))
    }

    pub(crate) fn active_descendant_for_overlay_content(
        &self,
        content: ElementId,
    ) -> Option<ElementId> {
        let focused = self.focused()?;
        if self.is_descendant_or_self(focused, content) {
            return Some(focused);
        }
        let owner = self.overlays.active_for_content(content)?;
        let focused_popup = self.containing_overlay_for(focused)?;
        self.overlays
            .child_anchor_towards(owner.element, focused_popup)
    }

    pub(crate) fn pointer_keeps_submenu_open(&self, target: ElementId) -> bool {
        self.overlays.pointer_keeps_submenu_open(target)
    }

    fn overlay_focus_target(&self, focused: Option<ElementId>) -> FocusTarget {
        let Some(focused) = focused else {
            return FocusTarget::Other(0);
        };
        let active = self.overlays.active_entries().collect::<Vec<_>>();
        for host in active.iter().rev() {
            if self.is_descendant_or_self(focused, host.target) {
                return FocusTarget::Anchor(anchor_id(host.target));
            }
        }
        for host in active.iter().rev() {
            if self.is_descendant_or_self(focused, host.element) {
                return FocusTarget::Overlay(host.overlay_id());
            }
        }
        FocusTarget::Other(element_token(focused))
    }
}

fn same_origin(a: Point, b: Point) -> bool {
    const EPSILON: f64 = 0.001;
    (a.x - b.x).abs() <= EPSILON && (a.y - b.y).abs() <= EPSILON
}
