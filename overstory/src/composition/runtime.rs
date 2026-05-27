// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Integration with `subduction_core`'s retained layer tree.
//!
//! This runtime maps Overstory elements onto compositor-neutral Subduction
//! layers and `SurfaceId`-keyed scene recordings. It owns the UI-to-layer
//! mapping and scene content registry; `subduction_core::layer::LayerStore`
//! owns layer topology, dirty evaluation, transforms, clips, and
//! backend-facing frame changes.

use alloc::vec::Vec;

use hashbrown::HashMap;
use imaging::Composite;
use imaging::record::{Clip, Geometry, Group, Scene};
use kurbo::{Affine, Rect, RoundedRect, Size, Vec2};
use peniko::BlendMode;
use subduction_core::layer::{
    ClipShape, FrameChanges, HitPolicy, HitRegion, LayerId, LayerStore, SurfaceId, SurfaceIds,
};
use subduction_core::transform::Transform3d;

use crate::element::ElementId;
use crate::element_side_map::ElementSideMap;

/// How an Overstory element contributes content to its composition layer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CompositionPolicy {
    /// Record the element's presentation primitives into an Overstory-owned
    /// [`imaging::record::Scene`] keyed by the layer's `SurfaceId`.
    #[default]
    InlineScene,
    /// Record the element into an Overstory-owned scene parented under the overlay
    /// composition root instead of its element parent's layer.
    OverlayScene,
    /// Keep a layer for geometry, clipping, ordering, and descendants, but do
    /// not attach content for the element itself.
    Container,
    /// Attach a `SurfaceId` for host-owned or native content. Overstory owns
    /// the identity but does not maintain a scene recording for it.
    External,
}

impl CompositionPolicy {
    fn attaches_surface(self) -> bool {
        matches!(
            self,
            Self::InlineScene | Self::OverlayScene | Self::External
        )
    }

    fn records_owned_scene(self) -> bool {
        matches!(self, Self::InlineScene | Self::OverlayScene)
    }

    fn uses_overlay_root(self) -> bool {
        matches!(self, Self::OverlayScene)
    }
}

/// Borrowed scene content associated with a Subduction [`SurfaceId`].
///
/// Overstory owns these scene recordings until a host replaces them
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
    /// Retained compositor-neutral layer tree.
    layers: LayerStore,
    /// Allocator for stable surface identities attached to layers.
    surface_ids: SurfaceIds,
    /// Root layer corresponding to the UI root element.
    main_root: Option<LayerId>,
    /// Dedicated layer parent for popup/tooltip/overlay surfaces.
    overlay_root: Option<LayerId>,
    /// Per-element composition metadata.
    elements: ElementSideMap<ElementComposition>,
    /// Reverse lookup for diagnostics and tests.
    layer_to_element: HashMap<LayerId, ElementId>,
    /// Overstory-owned scene recordings keyed by surface identity.
    surfaces: HashMap<SurfaceId, Scene>,
    /// Surface ids whose scene was rewritten by the most recent paint.
    updated_surfaces: Vec<SurfaceId>,
    /// Accumulated layer-tree changes ready for the host frame bridge.
    changes: FrameChanges,
}

#[derive(Clone, Copy, Debug)]
struct ElementComposition {
    layer: LayerId,
    policy: CompositionPolicy,
    surface: Option<SurfaceId>,
}

impl CompositionRuntime {
    /// Creates an empty composition runtime.
    pub(crate) fn new() -> Self {
        Self {
            layers: LayerStore::new(),
            surface_ids: SurfaceIds::new(),
            main_root: None,
            overlay_root: None,
            elements: ElementSideMap::new(),
            layer_to_element: HashMap::new(),
            surfaces: HashMap::new(),
            updated_surfaces: Vec::new(),
            changes: FrameChanges::default(),
        }
    }

    /// Inserts a layer for `id`, optionally parented under `parent`, and
    /// attaches content according to `policy`.
    pub(crate) fn insert_element(
        &mut self,
        id: ElementId,
        parent: Option<ElementId>,
        policy: CompositionPolicy,
    ) {
        let layer = self.layers.create_layer();
        self.layers.set_hit_policy(layer, HitPolicy::Region);
        if parent.is_none() {
            self.main_root = Some(layer);
            self.ensure_overlay_root();
        }

        let composition_parent = if policy.uses_overlay_root() {
            self.ensure_overlay_root()
        } else {
            parent.and_then(|parent| self.elements.get(parent).map(|entry| entry.layer))
        };
        if let Some(parent_layer) = composition_parent {
            self.layers.add_child(parent_layer, layer);
        }
        let surface = self.attach_content_for(layer, policy);
        self.elements.insert(
            id,
            ElementComposition {
                layer,
                policy,
                surface,
            },
        );
        self.layer_to_element.insert(layer, id);
    }

    /// Removes the layer for `id` and any remaining descendant layers.
    pub(crate) fn remove_element(&mut self, id: ElementId) {
        let Some(entry) = self.elements.remove(id) else {
            return;
        };
        let layer = entry.layer;
        if self.main_root == Some(layer) {
            self.main_root = None;
        }
        self.layers.destroy_subtree(layer);
        let mut removed_surfaces = Vec::new();
        if let Some(surface) = entry.surface {
            removed_surfaces.push(surface);
        }
        self.elements.retain(|_, candidate| {
            let alive = self.layers.is_alive(candidate.layer);
            if !alive && let Some(surface) = candidate.surface {
                removed_surfaces.push(surface);
            }
            alive
        });
        self.layer_to_element
            .retain(|candidate, _| self.layers.is_alive(*candidate));
        if self
            .overlay_root
            .is_some_and(|layer| !self.layers.is_alive(layer))
        {
            self.overlay_root = None;
        }
        for surface in removed_surfaces {
            let _ = self.surface_ids.release(surface);
            self.surfaces.remove(&surface);
            self.updated_surfaces
                .retain(|candidate| *candidate != surface);
        }
    }

    /// Returns the layer corresponding to `id`.
    pub(crate) fn layer_for(&self, id: ElementId) -> Option<LayerId> {
        self.elements.get(id).map(|entry| entry.layer)
    }

    /// Returns the content surface corresponding to `id`.
    pub(crate) fn surface_for(&self, id: ElementId) -> Option<SurfaceId> {
        self.elements.get(id).and_then(|entry| entry.surface)
    }

    /// Returns the Overstory-owned scene surface corresponding to `id`.
    pub(crate) fn owned_scene_surface_for(&self, id: ElementId) -> Option<SurfaceId> {
        let entry = self.elements.get(id)?;
        entry
            .policy
            .records_owned_scene()
            .then(|| self.surface_for(id))?
    }

    /// Returns the compositor-local bounds size for `id`.
    pub(crate) fn bounds_for(&self, id: ElementId) -> Option<Size> {
        self.layer_for(id).map(|layer| self.layers.bounds(layer))
    }

    /// Returns the overlay root layer.
    #[cfg(test)]
    pub(crate) fn overlay_root(&self) -> Option<LayerId> {
        self.overlay_root
    }

    /// Returns a retained scene for `surface`.
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

    /// Returns every retained surface scene.
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

    fn attach_content_for(
        &mut self,
        layer: LayerId,
        policy: CompositionPolicy,
    ) -> Option<SurfaceId> {
        if !policy.attaches_surface() {
            self.layers.set_content(layer, None);
            return None;
        }

        let surface = self.surface_ids.create();
        self.layers.set_content(layer, Some(surface));
        if policy.records_owned_scene() {
            self.surfaces.insert(surface, Scene::new());
        }
        Some(surface)
    }

    fn ensure_overlay_root(&mut self) -> Option<LayerId> {
        if let Some(layer) = self.overlay_root
            && self.layers.is_alive(layer)
        {
            return Some(layer);
        }

        let layer = self.layers.create_layer();
        self.layers.set_hit_policy(layer, HitPolicy::Disabled);
        self.overlay_root = Some(layer);
        Some(layer)
    }

    /// Updates compositor-local geometry for an element layer.
    pub(crate) fn set_geometry(
        &mut self,
        id: ElementId,
        local_offset: Vec2,
        world_offset: Vec2,
        size: Size,
        clip: Option<RoundedRect>,
    ) {
        let Some(entry) = self.elements.get(id).copied() else {
            return;
        };
        let layer = entry.layer;
        let policy = entry.policy;
        let layer_offset = if policy.uses_overlay_root() {
            world_offset
        } else {
            local_offset
        };
        self.layers.set_transform(
            layer,
            Transform3d::from(Affine::translate((layer_offset.x, layer_offset.y))),
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
        if Some(layer) == self.main_root
            && let Some(overlay_root) = self.overlay_root
        {
            self.layers
                .set_transform(overlay_root, Transform3d::IDENTITY);
            self.layers.set_bounds(overlay_root, size);
            self.layers.set_hit_region(
                overlay_root,
                Some(HitRegion::Rect(Rect::from_origin_size((0.0, 0.0), size))),
            );
            self.layers.set_clip(overlay_root, None);
        }
    }

    /// Updates only the compositor-local offset for an element layer.
    pub(crate) fn set_layer_offset(&mut self, id: ElementId, local_offset: Vec2) {
        let Some(entry) = self.elements.get(id).copied() else {
            return;
        };
        self.layers.set_transform(
            entry.layer,
            Transform3d::from(Affine::translate((local_offset.x, local_offset.y))),
        );
    }

    /// Updates compositor-local opacity for an element layer.
    pub(crate) fn set_opacity(&mut self, id: ElementId, opacity: f32) {
        let Some(entry) = self.elements.get(id).copied() else {
            return;
        };
        self.layers
            .set_opacity(entry.layer, opacity.clamp(0.0, 1.0));
    }

    /// Evaluates layer dirty state and stores the latest frame changes.
    pub(crate) fn evaluate(&mut self) {
        self.layers.evaluate_into(&mut self.changes);
    }

    /// Returns the most recent frame changes produced by [`Self::evaluate`].
    pub(crate) fn last_changes(&self) -> &FrameChanges {
        &self.changes
    }

    /// Composes all attached surfaces through the current layer tree.
    pub(crate) fn render_into(&self, output: &mut Scene) {
        self.render_into_with(output, |surface| self.surfaces.get(&surface));
    }

    /// Composes attached surfaces through the current layer tree using a
    /// caller-supplied surface scene lookup.
    pub(crate) fn render_into_with<'a>(
        &self,
        output: &mut Scene,
        mut scene_for: impl FnMut(SurfaceId) -> Option<&'a Scene>,
    ) {
        output.clear();
        for root in self.layers.roots() {
            self.render_layer(root, output, &mut scene_for);
        }
    }

    fn render_layer<'a>(
        &self,
        layer: LayerId,
        output: &mut Scene,
        scene_for: &mut impl FnMut(SurfaceId) -> Option<&'a Scene>,
    ) {
        if self.layers.effective_hidden(layer) {
            return;
        }
        let opacity = self.layers.effective_opacity(layer);
        if opacity <= 0.0 {
            return;
        }
        let Some(transform) = affine_from_transform(self.layers.world_transform(layer)) else {
            return;
        };

        let clip_pushed = self.layers.clip(layer).map(|clip| {
            output.push_clip(layer_clip(clip, transform));
        });

        if let Some(surface) = self.layers.content(layer)
            && let Some(scene) = scene_for(surface)
        {
            let group_pushed = (opacity < 1.0).then(|| {
                output.push_group(Group {
                    composite: Composite::new(BlendMode::default(), opacity),
                    ..Group::default()
                });
            });
            output.append_transformed(scene, transform);
            if group_pushed.is_some() {
                output.pop_group();
            }
        }

        for child in self.layers.children(layer) {
            self.render_layer(child, output, scene_for);
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
