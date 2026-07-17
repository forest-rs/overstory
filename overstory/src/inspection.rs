// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Plain data snapshots for Overstory devtools and inspectors.
//!
//! Inspection snapshots describe the current retained UI state. They are
//! intentionally separate from Spoor telemetry: Spoor records what happened
//! over time, while these rows describe what exists now.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use kurbo::{Point, Rect, Size};
use peniko::Color;
use understory_animation::{AnimationPlayState, CompositeOperation, TimingSample};
use understory_overlay::{GraceShape, OverlayBehavior, OverlayLayer};
use understory_style::{ClassId, PartTag, PseudoClassId, StyleVocabulary, TypeTag};
use understory_timing::TimerDuration;

use crate::composition::{CompositionPolicy, SurfaceId};
use crate::diagnostics::{LayoutOutcome, LayoutStabilizationCause, UiFrameStats};
use crate::element::ElementId;
use crate::motion::{PresentationTarget, PresentationValueKind};
use crate::template::{TemplateDiagnostic, TemplateExpansionReport, TemplateSlotStatus};
use crate::timeline_runtime::{AnimationId, AnimationTimelineId};

/// Owned snapshot of the inspectable state of one [`crate::Ui`].
#[derive(Clone, Debug)]
pub struct UiInspectionSnapshot {
    /// Root element id for the retained element tree.
    pub root: ElementId,
    /// Elements in root-first document order.
    pub elements: Vec<ElementInspectionRow>,
    /// Layout stabilization outcome and the widgets that requested its final
    /// additional pass.
    pub layout: LayoutInspectionSnapshot,
    /// Composition layer tree and surface summary.
    pub composition: CompositionInspectionSnapshot,
    /// Active overlay lifecycle, geometry, focus scopes, and hover grace.
    pub overlays: OverlayInspectionSnapshot,
    /// Storage and cache size counters.
    pub storage: UiStorageInspectionSnapshot,
    /// Timeline runtime counters.
    pub timeline: TimelineInspectionSnapshot,
    /// Human-readable style vocabulary known to this UI.
    pub style_vocabulary: StyleVocabulary,
    /// Frame counters captured at the same instant as the snapshot.
    pub frame: UiFrameStats,
}

/// Inspectable facts from the most recent layout-stabilization attempt.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LayoutInspectionSnapshot {
    /// Whether the attempt stabilized or deferred remaining work.
    pub outcome: LayoutOutcome,
    /// Widgets that requested the final additional pass.
    ///
    /// This is empty after a stable attempt and contains only the last pass
    /// after budget exhaustion, keeping diagnostics bounded.
    pub causes: Vec<LayoutStabilizationCause>,
}

/// Inspectable projection of the retained overlay stack and its current frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OverlayInspectionSnapshot {
    /// Open overlays in deterministic stack order.
    pub entries: Vec<OverlayInspectionRow>,
    /// Containing or restoring focus scopes derived for the current frame.
    pub focus_scopes: Vec<OverlayFocusScopeInspectionRow>,
    /// Pointer-travel grace relationships derived for nested overlays.
    pub grace_regions: Vec<OverlayGraceInspectionRow>,
}

/// One active overlay and its Overstory-owned retained identities.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayInspectionRow {
    /// Retained popup host that owns placement and composition.
    pub element: ElementId,
    /// Semantic retained content represented by the host.
    pub content: ElementId,
    /// Retained element used as the overlay anchor.
    pub target: ElementId,
    /// Geometry source used to position the overlay.
    pub anchor: OverlayAnchorInspection,
    /// Parent popup host for a nested overlay.
    pub parent: Option<ElementId>,
    /// Semantic overlay behavior supplied to Understory.
    pub behavior: OverlayBehavior,
    /// Deterministic overlay layer supplied to Understory.
    pub layer: OverlayLayer,
    /// Current scene-space overlay rectangle.
    pub rect: Rect,
    /// Current deterministic z-order.
    pub z: i32,
    /// Focus target captured when the overlay opened.
    pub restore_focus: Option<ElementId>,
}

/// Inspectable geometry source for an active overlay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OverlayAnchorInspection {
    /// The overlay follows the current bounds of its semantic target.
    TargetBounds,
    /// The overlay remains at a captured scene-space point.
    Point(Point),
}

/// Inspectable focus-scope metadata derived from an active overlay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayFocusScopeInspectionRow {
    /// Popup host that owns the scope.
    pub overlay: ElementId,
    /// Semantic content contained by the scope.
    pub content: ElementId,
    /// Scene-space scope rectangle.
    pub rect: Rect,
    /// Whether traversal should remain inside the scope.
    pub contain: bool,
    /// Whether closing the scope should restore captured focus.
    pub restore_on_close: bool,
}

/// Inspectable pointer-travel relationship between nested overlays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayGraceInspectionRow {
    /// Parent popup host.
    pub parent: ElementId,
    /// Child popup host protected during pointer travel.
    pub child: ElementId,
    /// Current scene-space grace shape from Understory.
    pub shape: GraceShape,
}

/// Batched inspectable changes produced by normal [`crate::Ui`] mutation.
///
/// Snapshots are still the baseline and resync format. Change batches are the
/// steady-state feed for live devtools and inspectors that want to update a
/// retained mirror without recapturing the full tree every frame.
#[derive(Clone, Debug, Default)]
pub struct UiInspectionChanges {
    /// Retained-element tree changes.
    pub tree: ElementTreeInspectionPatch,
    /// Latest layout stabilization facts when a layout attempt ran.
    pub layout: Option<LayoutInspectionSnapshot>,
    /// Latest overlay state when lifecycle or derived geometry changed.
    pub overlays: Option<OverlayInspectionSnapshot>,
    /// Frame counters and host-supplied timings, when the most recent frame
    /// summary changed since the previous drain.
    pub frame: Option<UiFrameStats>,
}

impl UiInspectionChanges {
    /// Returns whether this batch carries no changes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
            && self.layout.is_none()
            && self.overlays.is_none()
            && self.frame.is_none()
    }
}

/// Incremental retained-element tree changes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ElementTreeInspectionPatch {
    /// Root element for the inspected tree after the patch.
    pub root: Option<ElementId>,
    /// Elements removed from the tree.
    pub removed: Vec<ElementInspectionRemoval>,
    /// Added or structurally changed element rows.
    ///
    /// These rows can change a retained inspector's projected tree because the
    /// element is new, moved, or has updated child links.
    pub upserted: Vec<ElementInspectionRow>,
    /// Existing element rows whose inspectable details changed.
    ///
    /// These rows update the retained mirror but do not imply a topology
    /// change. A devtools frontend may refresh a details pane without
    /// rebuilding an outline/tree control.
    pub updated: Vec<ElementInspectionRow>,
}

impl ElementTreeInspectionPatch {
    /// Returns whether this patch changes no element rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.removed.is_empty() && self.upserted.is_empty() && self.updated.is_empty()
    }
}

/// Removal of one retained element.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ElementInspectionRemoval {
    /// Removed element id.
    pub id: ElementId,
    /// Parent at removal time.
    pub parent: Option<ElementId>,
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
    /// Control-declared template contract status, when this element owns one.
    pub template_contract: Option<TemplateContractInspection>,
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
    /// Concise dependency-property values worth showing in an inspector.
    ///
    /// These rows summarize effective values for built-in properties that are
    /// authored, animated, or resolved to a non-default value. Detailed
    /// provenance is intentionally left for future drill-down APIs so the main
    /// property grid can stay readable.
    pub properties: Vec<ElementPropertyInspectionRow>,
}

/// Inspectable status for a control's declared template contract.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TemplateContractInspection {
    /// Required slots and whether the expanded template provided them.
    pub required_slots: Vec<TemplateSlotInspectionRow>,
    /// Optional capability slots and whether the expanded template provided
    /// them.
    pub optional_slots: Vec<TemplateSlotInspectionRow>,
    /// Recommended slots and whether the expanded template provided them.
    pub recommended_slots: Vec<TemplateSlotInspectionRow>,
    /// Non-fatal diagnostics reported while expanding the template instance.
    pub diagnostics: Vec<TemplateDiagnostic>,
}

impl TemplateContractInspection {
    pub(crate) fn from_expansion_report(report: &TemplateExpansionReport) -> Self {
        Self {
            required_slots: report
                .required_slots()
                .iter()
                .copied()
                .map(template_slot_inspection_row)
                .collect(),
            optional_slots: report
                .optional_slots()
                .iter()
                .copied()
                .map(template_slot_inspection_row)
                .collect(),
            recommended_slots: report
                .recommended_slots()
                .iter()
                .copied()
                .map(template_slot_inspection_row)
                .collect(),
            diagnostics: report.diagnostics().to_vec(),
        }
    }

    /// Returns whether any required slot is missing.
    #[must_use]
    pub fn has_missing_required_slots(&self) -> bool {
        self.required_slots
            .iter()
            .any(|slot| slot.element.is_none())
    }

    /// Returns whether any recommended slot is missing.
    ///
    /// Missing recommended slots do not make the contract invalid, but they are
    /// useful quality/degradation hints for custom templates.
    #[must_use]
    pub fn has_missing_recommended_slots(&self) -> bool {
        self.recommended_slots
            .iter()
            .any(|slot| slot.element.is_none())
    }

    /// Returns whether this contract has missing required slots or expansion
    /// diagnostics.
    #[must_use]
    pub fn has_issues(&self) -> bool {
        self.has_missing_required_slots() || !self.diagnostics.is_empty()
    }
}

fn template_slot_inspection_row(status: TemplateSlotStatus) -> TemplateSlotInspectionRow {
    let slot = status.slot();
    TemplateSlotInspectionRow {
        name: slot.name(),
        part_tag: slot.part_tag(),
        element: status.element(),
    }
}

/// Inspectable status for one semantic template slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TemplateSlotInspectionRow {
    /// Stable semantic slot name.
    pub name: &'static str,
    /// Owner-local part tag coupled to the slot, if any.
    pub part_tag: Option<PartTag>,
    /// Generated element that fills the slot, or `None` when missing.
    pub element: Option<ElementId>,
}

/// One inspectable dependency-property value for an element.
#[derive(Clone, Debug, PartialEq)]
pub struct ElementPropertyInspectionRow {
    /// User-facing property name.
    pub name: &'static str,
    /// Broad group used by devtools to organize the property grid.
    pub group: ElementPropertyInspectionGroup,
    /// Display value sampled for this snapshot.
    pub value: ElementPropertyInspectionValue,
    /// Resolution stage that supplied the value.
    pub origin: ElementPropertyInspectionOrigin,
}

/// One named field inside a structured inspectable property value.
#[derive(Clone, Debug, PartialEq)]
pub struct ElementPropertyInspectionField {
    /// User-facing field name.
    pub name: &'static str,
    /// Field value.
    pub value: ElementPropertyInspectionValue,
}

/// Broad group for an inspectable dependency property.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ElementPropertyInspectionGroup {
    /// Fill and other visual appearance properties.
    Appearance,
    /// Box model and surface decoration geometry properties.
    Box,
    /// Text content and typography properties.
    Text,
    /// Layout-intent properties.
    Layout,
    /// Time-varying or composition-oriented presentation properties.
    Motion,
    /// Control state and semantic behavior properties.
    Control,
    /// Numeric range/value properties.
    Range,
}

/// Origin of a concise property value in an inspection row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ElementPropertyInspectionOrigin {
    /// The element's animation slot supplied the value.
    Animation,
    /// A locally authored value supplied the value (a user/local write, a
    /// template binding, or a template default).
    Local,
    /// A style cascade rule or direct style supplied the value.
    Cascade,
    /// A theme resource fallback supplied the value.
    Theme,
    /// An ancestor supplied the value through inherited resolution.
    Inherited,
    /// The registry default supplied the value.
    Default,
}

impl ElementPropertyInspectionOrigin {
    /// Returns the short label devtools should show for this origin.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Animation => "animation",
            Self::Local => "local",
            Self::Cascade => "cascade",
            Self::Theme => "theme",
            Self::Inherited => "inherited",
            Self::Default => "default",
        }
    }
}

/// Semantic value for an inspectable dependency property.
#[derive(Clone, Debug, PartialEq)]
pub enum ElementPropertyInspectionValue {
    /// Missing, unset, or not-applicable value.
    Empty,
    /// Plain text value.
    Text(String),
    /// Boolean value.
    Bool(bool),
    /// Number value.
    Number(f64),
    /// Count of items in a collection-like value.
    Count(usize),
    /// Token-like value such as an enum variant or shorthand structure.
    Token(String),
    /// Structured value with a compact summary and expandable named fields.
    Record {
        /// Compact row value shown before expansion.
        summary: String,
        /// Structured fields shown as child rows.
        fields: Vec<ElementPropertyInspectionField>,
    },
    /// Canonical sRGB color value suitable for inspection and presentation.
    Color(Color),
    /// Logical rectangle value.
    Rect {
        /// Minimum x coordinate.
        x0: f64,
        /// Minimum y coordinate.
        y0: f64,
        /// Maximum x coordinate.
        x1: f64,
        /// Maximum y coordinate.
        y1: f64,
    },
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
    /// Whether this element owns an attached context-menu factory.
    pub has_context_menu: bool,
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

impl fmt::Display for LayerInspectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.index, self.generation)
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

impl fmt::Display for SurfaceInspectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.index, self.generation)
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

/// Timeline runtime state for timeline-oriented devtools views.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TimelineInspectionSnapshot {
    /// Targets currently retained by the timeline runtime.
    pub targets: usize,
    /// Active implicit transitions currently sampling presentation values.
    pub active_transitions: usize,
    /// Active explicit or style-triggered effects currently sampling presentation values.
    pub active_effects: usize,
    /// Transition specs waiting for a future target change.
    pub pending_transitions: usize,
    /// Scheduled cues that have not yet fired.
    pub pending_cues: usize,
    /// Retained animation instances in stable id order.
    pub animations: Vec<TimelineAnimationInspectionRow>,
}

/// One retained animation instance in a timeline inspection snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct TimelineAnimationInspectionRow {
    /// Stable runtime id for the animation instance.
    pub id: AnimationId,
    /// Element whose presentation target is animated.
    pub element: ElementId,
    /// Presentation target sampled by this animation.
    pub target: PresentationTarget,
    /// Current playback state.
    pub state: AnimationPlayState,
    /// Current local animation time, when playback has one.
    pub current_time: Option<TimerDuration>,
    /// Current timing sample, including phase, iteration, and progress.
    pub timing_sample: Option<TimingSample>,
    /// Playback rate applied to the animation's timeline time.
    pub playback_rate: f64,
    /// How this animation layer composites with the value beneath it.
    pub composite: CompositeOperation,
    /// Erased presentation value kind sampled by this animation, when the
    /// value is one of Overstory's built-in presentation sample kinds.
    ///
    /// Custom typed animation targets can be retained and inspectable without
    /// mapping to a legacy erased [`PresentationValueKind`], in which case this
    /// is `None`.
    pub value_kind: Option<PresentationValueKind>,
    /// Timeline source driving this animation.
    pub timeline: AnimationTimelineId,
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use super::*;

    #[test]
    fn inspection_ids_display_as_index_and_generation() {
        assert_eq!(
            LayerInspectionId {
                index: 12,
                generation: 3,
            }
            .to_string(),
            "12:3"
        );
        assert_eq!(
            SurfaceInspectionId {
                index: 4,
                generation: 9,
            }
            .to_string(),
            "4:9"
        );
    }
}
