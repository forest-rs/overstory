// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Integration with `subduction_core`'s retained layer tree.
//!
//! This runtime maps Overstory elements onto compositor-neutral Subduction
//! layers. It owns only the mapping and UI-to-layer synchronization policy;
//! `subduction_core::layer::LayerStore` owns layer topology, dirty evaluation,
//! transforms, clips, and backend-facing frame changes.

use hashbrown::HashMap;
use kurbo::{Affine, Rect, RoundedRect, Size, Vec2};
use subduction_core::layer::{ClipShape, FrameChanges, HitPolicy, HitRegion, LayerId, LayerStore};
use subduction_core::transform::Transform3d;

use crate::element::ElementId;

/// Maintains the Subduction layer tree for one [`crate::Ui`].
#[derive(Debug)]
pub(crate) struct CompositionRuntime {
    layers: LayerStore,
    element_to_layer: HashMap<ElementId, LayerId>,
    layer_to_element: HashMap<LayerId, ElementId>,
    changes: FrameChanges,
}

impl CompositionRuntime {
    /// Creates an empty composition runtime.
    pub(crate) fn new() -> Self {
        Self {
            layers: LayerStore::new(),
            element_to_layer: HashMap::new(),
            layer_to_element: HashMap::new(),
            changes: FrameChanges::default(),
        }
    }

    /// Inserts a layer for `id`, optionally parented under `parent`.
    pub(crate) fn insert_element(&mut self, id: ElementId, parent: Option<ElementId>) {
        let layer = self.layers.create_layer();
        self.layers.set_hit_policy(layer, HitPolicy::Region);
        if let Some(parent_layer) = parent.and_then(|parent| self.element_to_layer.get(&parent)) {
            self.layers.add_child(*parent_layer, layer);
        }
        self.element_to_layer.insert(id, layer);
        self.layer_to_element.insert(layer, id);
    }

    /// Removes the layer for `id` and any remaining descendant layers.
    pub(crate) fn remove_element(&mut self, id: ElementId) {
        let Some(layer) = self.element_to_layer.remove(&id) else {
            return;
        };
        self.layers.destroy_subtree(layer);
        self.element_to_layer
            .retain(|_, candidate| self.layers.is_alive(*candidate));
        self.layer_to_element
            .retain(|candidate, _| self.layers.is_alive(*candidate));
    }

    /// Returns the layer corresponding to `id`.
    pub(crate) fn layer_for(&self, id: ElementId) -> Option<LayerId> {
        self.element_to_layer.get(&id).copied()
    }

    /// Returns the element corresponding to `layer`.
    #[cfg(test)]
    pub(crate) fn element_for_layer(&self, layer: LayerId) -> Option<ElementId> {
        self.layer_to_element.get(&layer).copied()
    }

    /// Returns the current Subduction layer store.
    pub(crate) fn layers(&self) -> &LayerStore {
        &self.layers
    }

    /// Updates compositor-local geometry for an element layer.
    pub(crate) fn set_geometry(
        &mut self,
        id: ElementId,
        local_offset: Vec2,
        size: Size,
        clip: Option<RoundedRect>,
    ) {
        let Some(layer) = self.layer_for(id) else {
            return;
        };
        self.layers.set_transform(
            layer,
            Transform3d::from(Affine::translate((local_offset.x, local_offset.y))),
        );
        self.layers.set_bounds(layer, size);
        self.layers.set_hit_region(
            layer,
            Some(HitRegion::Rect(Rect::from_origin_size((0.0, 0.0), size))),
        );
        self.layers.set_clip(
            layer,
            clip.map(|clip| {
                let local = RoundedRect::from_rect(
                    Rect::from_origin_size((0.0, 0.0), clip.rect().size()),
                    clip.radii(),
                );
                ClipShape::RoundedRect(local)
            }),
        );
    }

    /// Evaluates layer dirty state and stores the latest frame changes.
    pub(crate) fn evaluate(&mut self) {
        self.layers.evaluate_into(&mut self.changes);
    }

    /// Returns the most recent frame changes produced by [`Self::evaluate`].
    pub(crate) fn last_changes(&self) -> &FrameChanges {
        &self.changes
    }
}
