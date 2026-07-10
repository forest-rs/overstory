// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Overlay placement updates after layout has measured overlay content.

use alloc::vec::Vec;

use kurbo::{Point, Rect, Size};
use understory_overlay::{
    OverlayEvent, OverlayFrameInput, OverlayGeometry, build_overlay_frame, resolve_event,
};

use crate::element::ElementId;
use crate::layouts::LayoutPlacement;
use crate::overlay::resolve_placement;

use super::Ui;

impl Ui {
    pub(in crate::ui) fn remove_overlay_state_for(&mut self, id: ElementId) {
        for element in self.overlays.remove_element(id) {
            if element != id && self.get(element).is_some() {
                self.remove(element);
            }
        }
    }

    pub(in crate::ui) fn reposition_overlays(&mut self, viewport: Size) -> bool {
        let active = self.overlays.active_entries().collect::<Vec<_>>();
        let mut changed = false;
        for entry in active {
            if self.get(entry.target).is_none() || self.get(entry.element).is_none() {
                continue;
            }
            let target_bounds = self.effective_bounds(entry.target);
            let overlay_bounds = self.effective_bounds(entry.element);
            let origin = resolve_placement(
                entry.placement,
                target_bounds,
                overlay_bounds.size(),
                viewport,
                entry.offset,
            );
            if same_origin(overlay_bounds.origin(), origin) {
                continue;
            }
            self.set_layout_placement(
                entry.element,
                LayoutPlacement::absolute_at(origin.x, origin.y),
            );
            changed = true;
        }
        changed
    }

    pub(in crate::ui) fn apply_overlay_event(&mut self, event: OverlayEvent) -> bool {
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
        let frame = build_overlay_frame(self.overlays.stack(), input);
        let operations = resolve_event(self.overlays.stack(), &frame, event).ops;
        let closed = operations
            .into_iter()
            .flat_map(|operation| self.overlays.apply_operation(operation))
            .collect::<Vec<_>>();
        let changed = !closed.is_empty();
        for host in closed.into_iter().rev() {
            self.close_overlay_host(host);
        }
        changed
    }
}

fn same_origin(a: Point, b: Point) -> bool {
    const EPSILON: f64 = 0.001;
    (a.x - b.x).abs() <= EPSILON && (a.y - b.y).abs() <= EPSILON
}
