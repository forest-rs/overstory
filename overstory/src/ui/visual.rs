// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Visual-only presentation properties synchronized into composition layers.

use alloc::vec::Vec;

use kurbo::{Affine, Vec2};

use crate::channels::COMPOSE;
use crate::element::ElementId;
use crate::timeline_runtime::{TimelineTarget, VisualOutput};

use super::Ui;

impl Ui {
    /// Writes a visual-only presentation transform for `id`.
    ///
    /// The transform is applied by composition after layout has placed the
    /// element's layer. It does not change layout bounds, flow, or the element's
    /// target geometry.
    pub fn set_visual_transform(&mut self, id: ElementId, transform: Affine) {
        self.set_local(id, self.properties.visual_transform, transform);
        self.composition.set_visual_transform(id, transform);
    }

    /// Returns the cascade-resolved visual presentation transform for `id`.
    ///
    /// Missing or stale element ids resolve to [`Affine::IDENTITY`].
    #[must_use]
    pub fn effective_visual_transform(&self, id: ElementId) -> Affine {
        self.get(id)
            .map(|element| self.resolve_value_for(element, self.properties.visual_transform))
            .unwrap_or(Affine::IDENTITY)
    }

    pub(in crate::ui) fn sync_composition_values(&mut self) {
        let dirty: Vec<ElementId> = self.invalidation.drain_sorted(COMPOSE).collect();
        for id in dirty {
            if self.get(id).is_none() {
                continue;
            }
            let visual_transform = self.presented_visual_transform(id);
            let opacity = self.presented_layer_opacity(id);
            self.composition.set_visual_transform(id, visual_transform);
            self.composition.set_opacity(id, opacity);
        }
    }

    pub(in crate::ui) fn presented_visual_transform(&self, id: ElementId) -> Affine {
        let translation = self.presented_value(
            id,
            TimelineTarget::VisualOutput(VisualOutput::VisualTranslation),
            Vec2::ZERO,
        );
        Affine::translate((translation.x, translation.y)) * self.effective_visual_transform(id)
    }
}
