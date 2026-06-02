// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use crate::inspection::{
    CompositionChangeStats, CompositionInspectionSnapshot, CompositionLayerInspectionRow,
    ElementInspectionRemoval, ElementInspectionRow, ElementStateInspection, LayerInspectionId,
    SurfaceInspectionId, TimelineInspectionSnapshot, UiInspectionChanges, UiInspectionSnapshot,
    UiStorageInspectionSnapshot,
};
use crate::pseudos::{PSEUDO_FOCUS_VISIBLE, PSEUDO_HOVER, PSEUDO_PRESSED};

use super::{Ui, collect_preorder};

impl Ui {
    /// Returns an owned snapshot of state useful to inspectors and devtools.
    ///
    /// The snapshot is intentionally detached from the live UI: callers can
    /// render or transmit it without borrowing the target tree.
    #[must_use]
    pub fn inspection_snapshot(&self) -> UiInspectionSnapshot {
        UiInspectionSnapshot {
            root: self.root(),
            elements: self.element_inspection_rows(),
            composition: self.composition_inspection_snapshot(),
            storage: self.storage_inspection_snapshot(),
            timeline: self.timeline_inspection_snapshot(),
            frame: self.frame_stats(),
        }
    }

    /// Drains inspectable state changes recorded since the previous drain.
    ///
    /// This is the live-update counterpart to [`Self::inspection_snapshot`].
    /// Hosts and devtools should use snapshots for initial attachment or
    /// resync, then consume drained changes for steady-state updates.
    pub fn drain_inspection_changes(&mut self) -> UiInspectionChanges {
        let root = self.root();
        let frame = self.frame_stats();
        self.inspection_changes.drain(root, frame)
    }

    /// Starts recording live inspection changes.
    ///
    /// Snapshot capture does not require live recording. Call this when a live
    /// inspector attaches and intends to drain incremental changes through
    /// [`Self::drain_inspection_changes`]. Calls are counted so multiple
    /// inspectors can attach independently.
    pub fn begin_live_inspection_recording(&mut self) {
        self.inspection_changes.begin_recording();
    }

    /// Stops one live inspection recording attachment.
    ///
    /// When the final attachment ends, pending changes are discarded so a later
    /// inspector starts from a fresh baseline capture instead of replaying
    /// stale closed-inspector changes.
    pub fn end_live_inspection_recording(&mut self) {
        self.inspection_changes.end_recording();
    }

    /// Returns whether live inspection changes are currently recorded.
    #[must_use]
    pub fn is_live_inspection_recording(&self) -> bool {
        self.inspection_changes.is_recording()
    }

    /// Clears pending inspection changes without returning them.
    ///
    /// Use this immediately after taking a baseline snapshot for an attached
    /// inspector; otherwise earlier construction changes would be replayed
    /// after the mirror already has the baseline state.
    pub fn clear_inspection_changes(&mut self) {
        self.inspection_changes.clear();
    }

    fn element_inspection_rows(&self) -> Vec<ElementInspectionRow> {
        collect_preorder(self, self.root())
            .into_iter()
            .filter_map(|id| self.element_inspection_row(id))
            .collect()
    }

    pub(in crate::ui) fn record_element_tree_upsert(&mut self, id: crate::ElementId) {
        if let Some(row) = self.element_inspection_row(id) {
            self.inspection_changes.record_tree_upsert(row);
        }
    }

    pub(in crate::ui) fn record_element_tree_removal(
        &mut self,
        id: crate::ElementId,
        parent: Option<crate::ElementId>,
    ) {
        self.inspection_changes
            .record_tree_removal(ElementInspectionRemoval { id, parent });
    }

    fn element_inspection_row(&self, id: crate::ElementId) -> Option<ElementInspectionRow> {
        let element = self.get(id)?;
        let (template_slot, template_key) =
            self.template_identity_for(id, element.templated_parent());
        Some(ElementInspectionRow {
            id,
            parent: element.parent(),
            children: element.children().to_vec(),
            templated_parent: element.templated_parent(),
            type_tag: element.type_tag(),
            part_tag: element.part_tag(),
            template_slot,
            template_key,
            classes: element.classes().to_vec(),
            pseudos: element.pseudos().to_vec(),
            state: ElementStateInspection {
                enabled: self.is_enabled(id),
                focusable: self.is_focusable(id),
                focused: self.focused() == Some(id),
                focus_visible: element.pseudos().contains(&PSEUDO_FOCUS_VISIBLE),
                hovered: element.pseudos().contains(&PSEUDO_HOVER),
                pressed: element.pseudos().contains(&PSEUDO_PRESSED),
                pointer_captured: self.pointer_capture() == Some(id),
            },
            has_widget_identity: element.has_widget_identity(),
            bounds: self.effective_bounds(id),
            layer: self
                .composition
                .layer_for(id)
                .map(LayerInspectionId::from_layer),
            surface: self
                .composition
                .surface_for(id)
                .map(SurfaceInspectionId::from_surface),
            composition_policy: self.composition.policy_for(id),
        })
    }

    fn template_identity_for(
        &self,
        element: crate::ElementId,
        owner: Option<crate::ElementId>,
    ) -> (Option<&'static str>, Option<u64>) {
        let Some(owner) = owner else {
            return (None, None);
        };
        let Some(part) = self.template_instance(owner).and_then(|instance| {
            instance
                .part_records()
                .iter()
                .find(|part| part.element() == element)
        }) else {
            return (None, None);
        };
        (
            part.slot().map(|slot| slot.name()),
            part.key().map(|key| key.value()),
        )
    }

    fn composition_inspection_snapshot(&self) -> CompositionInspectionSnapshot {
        let layers = self.composition.layers();
        let roots = layers.roots();
        let mut rows = Vec::with_capacity(layers.len());
        for root in roots.iter().copied() {
            self.push_composition_layer_rows(root, &mut rows);
        }

        CompositionInspectionSnapshot {
            roots: roots
                .into_iter()
                .map(LayerInspectionId::from_layer)
                .collect(),
            layers: rows,
            surfaces: self
                .composition
                .surfaces()
                .map(|surface| SurfaceInspectionId::from_surface(surface.surface()))
                .collect(),
            updated_surfaces: self
                .composition
                .surface_updates()
                .map(|surface| SurfaceInspectionId::from_surface(surface.surface()))
                .collect(),
            changes: CompositionChangeStats::from_changes(self.composition.last_changes()),
        }
    }

    fn push_composition_layer_rows(
        &self,
        layer: subduction_core::layer::LayerId,
        rows: &mut Vec<CompositionLayerInspectionRow>,
    ) {
        let layers = self.composition.layers();
        let element = self.composition.element_for_layer(layer);
        rows.push(CompositionLayerInspectionRow {
            id: LayerInspectionId::from_layer(layer),
            parent: layers.parent(layer).map(LayerInspectionId::from_layer),
            children: layers
                .children(layer)
                .map(LayerInspectionId::from_layer)
                .collect(),
            element,
            policy: element.and_then(|id| self.composition.policy_for(id)),
            surface: layers.content(layer).map(SurfaceInspectionId::from_surface),
            bounds: layers.bounds(layer),
            local_opacity: layers.local_opacity(layer),
            effective_opacity: layers.effective_opacity(layer),
            hidden: layers.flags(layer).hidden,
            effectively_hidden: layers.effective_hidden(layer),
            has_clip: layers.clip(layer).is_some(),
        });
        for child in layers.children(layer) {
            self.push_composition_layer_rows(child, rows);
        }
    }

    fn storage_inspection_snapshot(&self) -> UiStorageInspectionSnapshot {
        let live_elements = self.len();
        let free_element_slots = self.elements.free_len();
        UiStorageInspectionSnapshot {
            live_elements,
            free_element_slots,
            allocated_element_slots: live_elements + free_element_slots,
            composition_layers: self.composition.layers().len(),
            composition_surfaces: self.composition.surfaces().count(),
        }
    }

    fn timeline_inspection_snapshot(&self) -> TimelineInspectionSnapshot {
        TimelineInspectionSnapshot {
            targets: self.timeline.target_count(),
            active_tracks: self.timeline.active_track_count(),
            pending_transitions: self.timeline.pending_transition_count(),
            pending_cues: self.timeline.pending_cue_count(),
        }
    }
}
