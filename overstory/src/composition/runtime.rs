// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Integration with `subduction_core`'s retained layer tree.
//!
//! This runtime maps Overstory elements onto compositor-neutral Subduction
//! layers and `SurfaceId`-keyed software scenes. It owns the UI-to-layer
//! mapping and software content registry; `subduction_core::layer::LayerStore`
//! owns layer topology, dirty evaluation, transforms, clips, and
//! backend-facing frame changes.

use alloc::vec::Vec;

use hashbrown::HashMap;
use imaging::record::{Clip, Geometry, Scene};
use kurbo::{Affine, Rect, RoundedRect, Size, Vec2};
use subduction_core::layer::{
    ClipShape, FrameChanges, HitPolicy, HitRegion, LayerId, LayerStore, SurfaceId, SurfaceIds,
};
use subduction_core::transform::Transform3d;

use crate::element::ElementId;

/// Borrowed scene content associated with a Subduction [`SurfaceId`].
///
/// Overstory owns these software scene recordings until a host replaces them
/// with compositor-backed or GPU-backed resources keyed by the same surface ID.
#[derive(Clone, Copy, Debug)]
pub struct CompositionSurface<'a> {
    surface: SurfaceId,
    scene: &'a Scene,
}

impl<'a> CompositionSurface<'a> {
    /// Creates a borrowed surface scene view.
    pub(crate) const fn new(surface: SurfaceId, scene: &'a Scene) -> Self {
        Self { surface, scene }
    }

    /// Returns the stable Subduction surface identity for this content.
    #[must_use]
    pub const fn surface(&self) -> SurfaceId {
        self.surface
    }

    /// Returns the retained imaging scene for this surface.
    #[must_use]
    pub const fn scene(&self) -> &'a Scene {
        self.scene
    }
}

/// Maintains the Subduction layer tree for one [`crate::Ui`].
#[derive(Debug)]
pub(crate) struct CompositionRuntime {
    layers: LayerStore,
    surface_ids: SurfaceIds,
    element_to_layer: HashMap<ElementId, LayerId>,
    layer_to_element: HashMap<LayerId, ElementId>,
    element_to_surface: HashMap<ElementId, SurfaceId>,
    surfaces: HashMap<SurfaceId, Scene>,
    updated_surfaces: Vec<SurfaceId>,
    changes: FrameChanges,
}

impl CompositionRuntime {
    /// Creates an empty composition runtime.
    pub(crate) fn new() -> Self {
        Self {
            layers: LayerStore::new(),
            surface_ids: SurfaceIds::new(),
            element_to_layer: HashMap::new(),
            layer_to_element: HashMap::new(),
            element_to_surface: HashMap::new(),
            surfaces: HashMap::new(),
            updated_surfaces: Vec::new(),
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
        if parent.is_none() {
            let surface = self.surface_ids.create();
            self.layers.set_content(layer, Some(surface));
            self.element_to_surface.insert(id, surface);
            self.surfaces.insert(surface, Scene::new());
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
        let removed_surfaces: Vec<SurfaceId> = self
            .element_to_surface
            .iter()
            .filter_map(|(element, surface)| {
                (!self.element_to_layer.contains_key(element)).then_some(*surface)
            })
            .collect();
        self.element_to_surface
            .retain(|element, _| self.element_to_layer.contains_key(element));
        for surface in removed_surfaces {
            let _ = self.surface_ids.release(surface);
            self.surfaces.remove(&surface);
            self.updated_surfaces
                .retain(|candidate| *candidate != surface);
        }
    }

    /// Returns the layer corresponding to `id`.
    pub(crate) fn layer_for(&self, id: ElementId) -> Option<LayerId> {
        self.element_to_layer.get(&id).copied()
    }

    /// Returns the software content surface corresponding to `id`.
    pub(crate) fn surface_for(&self, id: ElementId) -> Option<SurfaceId> {
        self.element_to_surface.get(&id).copied()
    }

    /// Returns a retained software scene for `surface`.
    #[cfg(test)]
    pub(crate) fn surface(&self, surface: SurfaceId) -> Option<&Scene> {
        self.surfaces.get(&surface)
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

    /// Returns every retained software surface scene.
    pub(crate) fn surfaces(&self) -> impl Iterator<Item = CompositionSurface<'_>> + '_ {
        self.surfaces
            .iter()
            .map(|(surface, scene)| CompositionSurface::new(*surface, scene))
    }

    /// Returns surface scenes rewritten by the most recent paint pass.
    pub(crate) fn surface_updates(&self) -> impl Iterator<Item = CompositionSurface<'_>> + '_ {
        self.updated_surfaces.iter().filter_map(|surface| {
            self.surfaces
                .get(surface)
                .map(|scene| CompositionSurface::new(*surface, scene))
        })
    }

    /// Clears the per-paint surface update list.
    pub(crate) fn clear_surface_updates(&mut self) {
        self.updated_surfaces.clear();
    }

    /// Takes a surface scene out for rebuilding.
    pub(crate) fn take_surface(&mut self, surface: SurfaceId) -> Scene {
        self.surfaces.remove(&surface).unwrap_or_default()
    }

    /// Restores a rebuilt surface scene and marks it as updated.
    pub(crate) fn restore_surface(&mut self, surface: SurfaceId, scene: Scene) {
        debug_assert!(
            self.surface_ids.is_alive(surface),
            "restored composition surface must still be live"
        );
        self.surfaces.insert(surface, scene);
        if !self.updated_surfaces.contains(&surface) {
            self.updated_surfaces.push(surface);
        }
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

    /// Composes all attached software surfaces through the current layer tree.
    pub(crate) fn render_into(&self, output: &mut Scene) {
        output.clear();
        for root in self.layers.roots() {
            self.render_layer(root, output);
        }
    }

    fn render_layer(&self, layer: LayerId, output: &mut Scene) {
        if self.layers.effective_hidden(layer) {
            return;
        }
        let Some(transform) = affine_from_transform(self.layers.world_transform(layer)) else {
            return;
        };

        let clip_pushed = self.layers.clip(layer).map(|clip| {
            output.push_clip(layer_clip(clip, transform));
        });

        if let Some(surface) = self.layers.content(layer)
            && let Some(scene) = self.surfaces.get(&surface)
        {
            output.append_transformed(scene, transform);
        }

        for child in self.layers.children(layer) {
            self.render_layer(child, output);
        }

        if clip_pushed.is_some() {
            output.pop_clip();
        }
    }
}

fn layer_clip(clip: ClipShape, transform: Affine) -> Clip {
    let shape = match clip {
        ClipShape::Rect(rect) => Geometry::Rect(rect),
        ClipShape::RoundedRect(rounded) => Geometry::RoundedRect(rounded),
    };
    Clip::Fill {
        transform,
        shape,
        fill_rule: peniko::Fill::NonZero,
    }
}

fn affine_from_transform(transform: Transform3d) -> Option<Affine> {
    let c = transform.to_cols_array_2d();
    let is_2d_affine = c[0][2] == 0.0
        && c[0][3] == 0.0
        && c[1][2] == 0.0
        && c[1][3] == 0.0
        && c[2] == [0.0, 0.0, 1.0, 0.0]
        && c[3][2] == 0.0
        && c[3][3] == 1.0;
    is_2d_affine.then(|| Affine::new([c[0][0], c[0][1], c[1][0], c[1][1], c[3][0], c[3][1]]))
}
