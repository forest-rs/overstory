// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Overlay placement updates after layout has measured overlay content.

use alloc::vec::Vec;

use kurbo::{Point, Rect, Size};
use understory_overlay::{
    FocusTarget, OverlayBehavior, OverlayEvent, OverlayFrameInput, OverlayGeometry,
};

use crate::element::ElementId;
use crate::overlay::{anchor_id, element_token};

use super::Ui;

impl Ui {
    pub(in crate::ui) fn remove_overlay_state_for(&mut self, id: ElementId) {
        let removed = self.overlays.remove_element(id);
        if self.overlays.frame_needs_refresh() {
            self.refresh_overlay_frame();
        }
        for element in removed {
            if element != id && self.get(element).is_some() {
                self.remove(element);
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
        let geometries = self
            .overlays
            .active_entries()
            .filter_map(|entry| {
                if self.get(entry.target).is_none() || self.get(entry.element).is_none() {
                    return None;
                }
                Some(
                    OverlayGeometry::new(entry.overlay_id(), self.effective_bounds(entry.element))
                        .with_anchor_rect(self.effective_bounds(entry.target)),
                )
            })
            .collect::<Vec<_>>();
        let input = OverlayFrameInput::new(
            Rect::from_origin_size(Point::ZERO, self.last_viewport),
            &geometries,
        );
        self.overlays.rebuild_frame(input);
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
