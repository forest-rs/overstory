// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Plain data snapshots for Overstory devtools and inspectors.
//!
//! Inspection snapshots describe the current retained UI state. They are
//! intentionally separate from Spoor telemetry: Spoor records what happened
//! over time, while these rows describe what exists now.

use alloc::vec::Vec;

use kurbo::{Rect, Size};
use understory_style::{ClassId, PartTag, PseudoClassId, TypeTag};

use crate::composition::{CompositionPolicy, SurfaceId};
use crate::diagnostics::UiFrameStats;
use crate::element::ElementId;

/// Owned snapshot of the inspectable state of one [`crate::Ui`].
#[derive(Clone, Debug, PartialEq)]
pub struct UiInspectionSnapshot {
    /// Root element id for the retained element tree.
    pub root: ElementId,
    /// Elements in root-first document order.
    pub elements: Vec<ElementInspectionRow>,
    /// Composition layer tree and surface summary.
    pub composition: CompositionInspectionSnapshot,
    /// Storage and cache size counters.
    pub storage: UiStorageInspectionSnapshot,
    /// Timeline runtime counters.
    pub timeline: TimelineInspectionSnapshot,
    /// Frame counters captured at the same instant as the snapshot.
    pub frame: UiFrameStats,
}

/// One retained element row in a [`UiInspectionSnapshot`].
#[derive(Clone, Debug, PartialEq)]
pub struct ElementInspectionRow {
    /// Stable generational element id.
    pub id: ElementId,
    /// Parent element, or `None` for the root.
    pub parent: Option<ElementId>,
    /// Child elements in document order.
    pub children: Vec<ElementId>,
    /// Control whose template created this element, when this is a template
    /// part.
    pub templated_parent: Option<ElementId>,
    /// Widget selector identity for this element, if any.
    pub type_tag: Option<TypeTag>,
    /// Owner-local template part identity for this element, if any.
    pub part_tag: Option<PartTag>,
    /// Semantic template slot name for this element, if any.
    pub template_slot: Option<&'static str>,
    /// Repeated template item key for this element, if any.
    pub template_key: Option<u64>,
    /// Sorted class identifiers active on the element.
    pub classes: Vec<ClassId>,
    /// Sorted pseudoclass identifiers active on the element.
    pub pseudos: Vec<PseudoClassId>,
    /// Interaction state captured for this element.
    pub state: ElementStateInspection,
    /// Whether this element was inserted with widget identity.
    pub has_widget_identity: bool,
    /// Current layout bounds in logical pixels.
    pub bounds: Rect,
    /// Composition layer corresponding to this element, if one exists.
    pub layer: Option<LayerInspectionId>,
    /// Surface attached to this element's composition layer, if one exists.
    pub surface: Option<SurfaceInspectionId>,
    /// Composition policy applied to this element's layer, if one exists.
    pub composition_policy: Option<CompositionPolicy>,
}

/// Interaction state for one retained element in an inspection snapshot.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ElementStateInspection {
    /// Whether the element is enabled after style/property resolution.
    pub enabled: bool,
    /// Whether the element currently participates in focus traversal.
    pub focusable: bool,
    /// Whether this element currently has focus.
    pub focused: bool,
    /// Whether focus should be rendered as keyboard-visible.
    pub focus_visible: bool,
    /// Whether the element currently matches `:hover`.
    pub hovered: bool,
    /// Whether the element currently matches `:pressed`.
    pub pressed: bool,
    /// Whether this element currently owns pointer capture.
    pub pointer_captured: bool,
}

/// Composition-layer and surface rows for one UI snapshot.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompositionInspectionSnapshot {
    /// Root composition layers in back-to-front order.
    pub roots: Vec<LayerInspectionId>,
    /// Layer rows in root-first composition order.
    pub layers: Vec<CompositionLayerInspectionRow>,
    /// Live Overstory-owned surfaces.
    pub surfaces: Vec<SurfaceInspectionId>,
    /// Overstory-owned surfaces rewritten during the most recent paint pass.
    pub updated_surfaces: Vec<SurfaceInspectionId>,
    /// Counts from the most recent Subduction frame-change evaluation.
    pub changes: CompositionChangeStats,
}

/// One composition-layer row in a [`CompositionInspectionSnapshot`].
#[derive(Clone, Debug, PartialEq)]
pub struct CompositionLayerInspectionRow {
    /// Stable generational layer id.
    pub id: LayerInspectionId,
    /// Parent layer, or `None` for a composition root.
    pub parent: Option<LayerInspectionId>,
    /// Child layers in back-to-front order.
    pub children: Vec<LayerInspectionId>,
    /// Element represented by this layer, when the layer is element-backed.
    pub element: Option<ElementId>,
    /// Composition policy for the represented element, when available.
    pub policy: Option<CompositionPolicy>,
    /// Surface content attached to the layer, if any.
    pub surface: Option<SurfaceInspectionId>,
    /// Layer bounds in local logical pixels.
    pub bounds: Size,
    /// Local opacity set on the layer.
    pub local_opacity: f32,
    /// Effective opacity after ancestor opacity propagation.
    pub effective_opacity: f32,
    /// Whether the layer's own hidden flag is set.
    pub hidden: bool,
    /// Whether the layer is hidden after ancestor propagation.
    pub effectively_hidden: bool,
    /// Whether the layer has a clip shape.
    pub has_clip: bool,
}

/// Stable layer identity copied into inspection snapshots.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LayerInspectionId {
    /// Raw Subduction layer slot index.
    pub index: u32,
    /// Generation counter for the layer slot.
    pub generation: u32,
}

impl LayerInspectionId {
    pub(crate) const fn from_layer(id: subduction_core::layer::LayerId) -> Self {
        Self {
            index: id.index(),
            generation: id.generation(),
        }
    }
}

/// Stable surface identity copied into inspection snapshots.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SurfaceInspectionId {
    /// Raw Subduction surface slot index.
    pub index: u32,
    /// Generation counter for the surface slot.
    pub generation: u32,
}

impl SurfaceInspectionId {
    pub(crate) const fn from_surface(id: SurfaceId) -> Self {
        Self {
            index: id.index(),
            generation: id.generation(),
        }
    }
}

/// Counts from a Subduction composition-frame change set.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CompositionChangeStats {
    /// Layers whose world transform was recomputed.
    pub transforms: usize,
    /// Layers whose effective opacity was recomputed.
    pub opacities: usize,
    /// Layers whose clip changed.
    pub clips: usize,
    /// Layers whose surface content attachment changed.
    pub content: usize,
    /// Layers whose bounds changed.
    pub bounds: usize,
    /// Layers that became hidden.
    pub hidden: usize,
    /// Layers that became visible.
    pub unhidden: usize,
    /// Layers added since the previous evaluation.
    pub added: usize,
    /// Layers removed since the previous evaluation.
    pub removed: usize,
    /// Whether composition topology changed.
    pub topology_changed: bool,
}

impl CompositionChangeStats {
    pub(crate) fn from_changes(changes: &subduction_core::layer::FrameChanges) -> Self {
        Self {
            transforms: changes.transforms.len(),
            opacities: changes.opacities.len(),
            clips: changes.clips.len(),
            content: changes.content.len(),
            bounds: changes.bounds.len(),
            hidden: changes.hidden.len(),
            unhidden: changes.unhidden.len(),
            added: changes.added.len(),
            removed: changes.removed.len(),
            topology_changed: changes.topology_changed,
        }
    }
}

/// Storage-size counters useful for memory-oriented devtools views.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UiStorageInspectionSnapshot {
    /// Live retained elements.
    pub live_elements: usize,
    /// Vacant element arena slots waiting for reuse.
    pub free_element_slots: usize,
    /// Allocated element arena slots, including vacant slots.
    pub allocated_element_slots: usize,
    /// Live composition layers.
    pub composition_layers: usize,
    /// Live Overstory-owned composition surfaces.
    pub composition_surfaces: usize,
}

/// Timeline runtime counters for timeline-oriented devtools views.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TimelineInspectionSnapshot {
    /// Targets currently retained by the timeline runtime.
    pub targets: usize,
    /// Active tracks currently sampling presentation values.
    pub active_tracks: usize,
    /// Transition specs waiting for a future target change.
    pub pending_transitions: usize,
    /// Scheduled cues that have not yet fired.
    pub pending_cues: usize,
}
