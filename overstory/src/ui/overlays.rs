// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Overlay placement updates after layout has measured overlay content.

use kurbo::{Point, Size};

use crate::element::ElementId;

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
            let origin = entry.placement.resolve(
                target_bounds,
                overlay_bounds.size(),
                viewport,
                entry.offset,
            );
            if same_origin(overlay_bounds.origin(), origin) {
                continue;
            }
            #[expect(
                clippy::cast_possible_truncation,
                reason = "overlay coordinates fit the layout engine"
            )]
            let left = origin.x as f32;
            #[expect(
                clippy::cast_possible_truncation,
                reason = "overlay coordinates fit the layout engine"
            )]
            let top = origin.y as f32;
            let layout = self.effective_layout(entry.element).absolute_at(left, top);
            self.set_layout(entry.element, layout);
            changed = true;
        }
        changed
    }
}

fn same_origin(a: Point, b: Point) -> bool {
    const EPSILON: f64 = 0.001;
    (a.x - b.x).abs() <= EPSILON && (a.y - b.y).abs() <= EPSILON
}
