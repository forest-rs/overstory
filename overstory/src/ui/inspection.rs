// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::inspection::{
    CompositionChangeStats, CompositionInspectionSnapshot, CompositionLayerInspectionRow,
    ElementInspectionRemoval, ElementInspectionRow, ElementPropertyInspectionField,
    ElementPropertyInspectionGroup, ElementPropertyInspectionOrigin, ElementPropertyInspectionRow,
    ElementPropertyInspectionValue, ElementStateInspection, ElementTreeInspectionPatch,
    LayerInspectionId, LayoutInspectionSnapshot, LiveInspectionRecording, OverlayAnchorInspection,
    OverlayFocusScopeInspectionRow, OverlayGraceInspectionRow, OverlayInspectionRow,
    OverlayInspectionSnapshot, SurfaceInspectionId, TemplateContractInspection,
    TimelineAnimationInspectionRow, TimelineInspectionSnapshot, UiInspectionChanges,
    UiInspectionSnapshot, UiStorageInspectionSnapshot,
};
use crate::overlay::OverlayAnchor;
use crate::paragraph_direction::paragraph_base_direction;
use crate::pseudos::{PSEUDO_FOCUS_VISIBLE, PSEUDO_HOVER, PSEUDO_PRESSED};
use peniko::Brush;
use understory_presentation::{BaseDirection, BorderStyle, CornerShape};
use understory_presentation_properties::CornerRadius;
use understory_property::{DependencyObject, Property};
use understory_style::ResolvedSource;

/// Maps style provenance to the coarse origin shown in inspection rows.
fn property_inspection_origin(source: &ResolvedSource) -> ElementPropertyInspectionOrigin {
    match source {
        ResolvedSource::Animation => ElementPropertyInspectionOrigin::Animation,
        ResolvedSource::LocalOverride { .. } => ElementPropertyInspectionOrigin::Local,
        ResolvedSource::CascadeRule { .. } | ResolvedSource::CascadeDirect { .. } => {
            ElementPropertyInspectionOrigin::Cascade
        }
        ResolvedSource::ThemeResource { .. } => ElementPropertyInspectionOrigin::Theme,
        ResolvedSource::Inherited { .. } => ElementPropertyInspectionOrigin::Inherited,
        ResolvedSource::Default | ResolvedSource::DefaultExpression { .. } => {
            ElementPropertyInspectionOrigin::Default
        }
    }
}

use super::{Ui, collect_preorder};

impl Ui {
    /// Returns an owned snapshot of state useful to inspectors and devtools.
    ///
    /// The snapshot is intentionally detached from the live UI: callers can
    /// render or transmit it without borrowing the target tree.
    #[must_use]
    pub fn inspection_snapshot(&self) -> UiInspectionSnapshot {
        UiInspectionSnapshot {
            revision: self
                .inspection_changes
                .projected_revision(self.timeline.inspection_revision()),
            root: self.root(),
            elements: self.element_inspection_rows(),
            layout: self.layout_inspection_snapshot(),
            composition: self.composition_inspection_snapshot(),
            overlays: self.overlay_inspection_snapshot(),
            storage: self.storage_inspection_snapshot(),
            timeline: self.timeline_inspection_snapshot(),
            style_vocabulary: self.style_vocabulary_snapshot(),
            frame: self.frame_stats(),
        }
    }

    fn layout_inspection_snapshot(&self) -> LayoutInspectionSnapshot {
        LayoutInspectionSnapshot {
            outcome: self.frame_stats.layout_outcome,
            causes: self.layout_stabilization_causes.clone(),
        }
    }

    fn overlay_inspection_snapshot(&self) -> OverlayInspectionSnapshot {
        let frame = self.overlays.frame();
        let entries = self
            .overlays
            .entries()
            .map(|(entry, host)| {
                let derived = frame.overlays.iter().find(|derived| derived.id == entry.id);
                OverlayInspectionRow {
                    element: host.element,
                    content: host.content,
                    target: host.target,
                    anchor: match host.anchor {
                        OverlayAnchor::TargetBounds => OverlayAnchorInspection::TargetBounds,
                        OverlayAnchor::Point(point) => OverlayAnchorInspection::Point(point),
                    },
                    parent: entry
                        .parent
                        .and_then(|parent| self.overlays.host_for(parent))
                        .map(|parent| parent.element),
                    behavior: entry.behavior,
                    layer: entry.layer,
                    rect: derived
                        .map_or_else(|| self.effective_bounds(host.element), |entry| entry.rect),
                    z: derived.map_or(0, |entry| entry.z),
                    restore_focus: host.restore_focus,
                }
            })
            .collect();
        let focus_scopes = frame
            .focus_scopes
            .iter()
            .filter_map(|scope| {
                let host = self.overlays.host_for(scope.overlay)?;
                Some(OverlayFocusScopeInspectionRow {
                    overlay: host.element,
                    content: host.content,
                    rect: scope.rect,
                    contain: scope.contain,
                    restore_on_close: scope.restore_on_close,
                })
            })
            .collect();
        let grace_regions = frame
            .grace_regions
            .iter()
            .filter_map(|region| {
                let parent = self.overlays.host_for(region.parent)?;
                let child = self.overlays.host_for(region.child)?;
                Some(OverlayGraceInspectionRow {
                    parent: parent.element,
                    child: child.element,
                    shape: region.shape,
                })
            })
            .collect();
        OverlayInspectionSnapshot {
            entries,
            focus_scopes,
            grace_regions,
        }
    }

    /// Drains inspectable state changes recorded since the previous drain.
    ///
    /// This is the live-update counterpart to [`Self::inspection_snapshot`].
    /// Hosts and devtools should use snapshots for initial attachment or
    /// resync, then consume drained changes for steady-state updates.
    ///
    /// Returns `None` when `recording` is not an active handle from this UI.
    /// An active recording with no pending work returns an empty batch.
    pub fn drain_inspection_changes(
        &mut self,
        recording: LiveInspectionRecording,
    ) -> Option<UiInspectionChanges> {
        self.synchronize_inspection_sources();
        let root = self.root();
        let frame = self.frame_stats();
        let pending = self.inspection_changes.drain(recording)?;
        let has_changes = !pending.is_empty();
        let upserted = pending
            .upserted
            .into_iter()
            .filter_map(|id| self.element_inspection_row(id))
            .collect();
        let updated = pending
            .updated
            .into_iter()
            .filter_map(|id| self.element_inspection_row(id))
            .collect();
        Some(UiInspectionChanges {
            baseline_revision: pending.baseline_revision,
            revision: pending.revision,
            tree: ElementTreeInspectionPatch {
                root: Some(root),
                removed: pending.removed,
                upserted,
                updated,
            },
            layout: has_changes.then(|| self.layout_inspection_snapshot()),
            overlays: has_changes.then(|| self.overlay_inspection_snapshot()),
            composition: has_changes.then(|| self.composition_inspection_snapshot()),
            storage: has_changes.then(|| self.storage_inspection_snapshot()),
            timeline: has_changes.then(|| self.timeline_inspection_snapshot()),
            style_vocabulary: has_changes.then(|| self.style_vocabulary_snapshot()),
            frame: has_changes.then_some(frame),
        })
    }

    /// Starts recording live inspection changes.
    ///
    /// Snapshot capture does not require live recording. Call this when a live
    /// inspector attaches and intends to drain incremental changes through
    /// [`Self::drain_inspection_changes`]. Each call returns an independent
    /// handle, so draining one inspector never consumes another inspector's
    /// changes.
    ///
    /// # API transition
    ///
    /// Live recording is handle-based. Keep the returned handle and pass it to
    /// `drain_inspection_changes`, `clear_inspection_changes`, and
    /// `end_live_inspection_recording`.
    pub fn begin_live_inspection_recording(&mut self) -> LiveInspectionRecording {
        self.synchronize_inspection_sources();
        self.inspection_changes.begin_recording()
    }

    /// Stops one live inspection recording attachment.
    ///
    /// When the final attachment ends, pending changes are discarded so a later
    /// inspector starts from a fresh baseline capture instead of replaying
    /// stale closed-inspector changes. Returns whether `recording` was active.
    pub fn end_live_inspection_recording(&mut self, recording: LiveInspectionRecording) -> bool {
        self.inspection_changes.end_recording(recording)
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
    /// after the mirror already has the baseline state. Returns whether
    /// `recording` was active.
    pub fn clear_inspection_changes(&mut self, recording: LiveInspectionRecording) -> bool {
        self.synchronize_inspection_sources();
        self.inspection_changes.clear(recording)
    }

    fn synchronize_inspection_sources(&mut self) {
        let timeline_revision = self.timeline.inspection_revision();
        self.inspection_changes
            .observe_timeline_revision(timeline_revision);
    }

    fn element_inspection_rows(&self) -> Vec<ElementInspectionRow> {
        collect_preorder(self, self.root())
            .into_iter()
            .filter_map(|id| self.element_inspection_row(id))
            .collect()
    }

    pub(in crate::ui) fn record_element_tree_upsert(&mut self, id: crate::ElementId) {
        if self.get(id).is_some() {
            self.inspection_changes.record_tree_upsert(id);
        }
    }

    pub(in crate::ui) fn record_element_tree_update(&mut self, id: crate::ElementId) {
        if self.get(id).is_some() {
            self.inspection_changes.record_tree_update(id);
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
            template_contract: self.template_contract_inspection(element),
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
                has_context_menu: self.has_context_menu(id),
            },
            scroll: self.scroll_state_snapshot(id),
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
            properties: self.element_property_inspection_rows(element),
        })
    }

    fn template_contract_inspection(
        &self,
        element: &crate::Element,
    ) -> Option<TemplateContractInspection> {
        let report = self.template_expansion_report(element.id())?;
        Some(TemplateContractInspection::from_expansion_report(&report))
    }

    fn element_property_inspection_rows(
        &self,
        element: &crate::Element,
    ) -> Vec<ElementPropertyInspectionRow> {
        let mut rows = Vec::new();
        let properties = self.properties;

        self.push_surface_property_rows(&mut rows, element);

        self.push_property_row(
            &mut rows,
            element,
            "Content",
            ElementPropertyInspectionGroup::Text,
            properties.content,
            text_value,
        );
        self.push_property_row(
            &mut rows,
            element,
            "Text",
            ElementPropertyInspectionGroup::Text,
            properties.text,
            text_value,
        );
        self.push_property_row(
            &mut rows,
            element,
            "Foreground",
            ElementPropertyInspectionGroup::Text,
            properties.foreground,
            brush_value,
        );
        self.push_property_row(
            &mut rows,
            element,
            "FontSize",
            ElementPropertyInspectionGroup::Text,
            properties.font_size,
            number_value,
        );
        self.push_debug_property_row(
            &mut rows,
            element,
            "FontFamily",
            ElementPropertyInspectionGroup::Text,
            properties.font_family,
        );
        self.push_debug_property_row(
            &mut rows,
            element,
            "FontWidth",
            ElementPropertyInspectionGroup::Text,
            properties.font_width,
        );
        self.push_debug_property_row(
            &mut rows,
            element,
            "FontWeight",
            ElementPropertyInspectionGroup::Text,
            properties.font_weight,
        );
        self.push_debug_property_row(
            &mut rows,
            element,
            "FontStyle",
            ElementPropertyInspectionGroup::Text,
            properties.font_style,
        );
        self.push_property_row(
            &mut rows,
            element,
            "FontVariations",
            ElementPropertyInspectionGroup::Text,
            properties.font_variations,
            |value| count_value(value.len()),
        );
        self.push_property_row(
            &mut rows,
            element,
            "FontFeatures",
            ElementPropertyInspectionGroup::Text,
            properties.font_features,
            |value| count_value(value.len()),
        );
        self.push_debug_property_row(
            &mut rows,
            element,
            "TextLanguage",
            ElementPropertyInspectionGroup::Text,
            properties.text_language,
        );
        self.push_paragraph_direction_row(&mut rows, element);
        self.push_debug_property_row(
            &mut rows,
            element,
            "LineHeight",
            ElementPropertyInspectionGroup::Text,
            properties.line_height,
        );
        self.push_property_row(
            &mut rows,
            element,
            "LetterSpacing",
            ElementPropertyInspectionGroup::Text,
            properties.letter_spacing,
            |value| number_value(f64::from(value)),
        );
        self.push_property_row(
            &mut rows,
            element,
            "WordSpacing",
            ElementPropertyInspectionGroup::Text,
            properties.word_spacing,
            |value| number_value(f64::from(value)),
        );
        self.push_debug_property_row(
            &mut rows,
            element,
            "WordBreak",
            ElementPropertyInspectionGroup::Text,
            properties.word_break,
        );
        self.push_debug_property_row(
            &mut rows,
            element,
            "OverflowWrap",
            ElementPropertyInspectionGroup::Text,
            properties.overflow_wrap,
        );
        self.push_debug_property_row(
            &mut rows,
            element,
            "TextWrapMode",
            ElementPropertyInspectionGroup::Text,
            properties.text_wrap_mode,
        );
        self.push_debug_property_row(
            &mut rows,
            element,
            "TextAlign",
            ElementPropertyInspectionGroup::Text,
            properties.text_align,
        );

        self.push_property_row(
            &mut rows,
            element,
            "LayoutDirection",
            ElementPropertyInspectionGroup::Layout,
            properties.layout_direction,
            |value| ElementPropertyInspectionValue::Token(String::from(value.inspection_summary())),
        );
        self.push_property_row(
            &mut rows,
            element,
            "Layout",
            ElementPropertyInspectionGroup::Layout,
            properties.layout,
            |value| layout_record(value.inspection_summary(), value.inspection_fields()),
        );
        self.push_property_row(
            &mut rows,
            element,
            "LayoutContainer",
            ElementPropertyInspectionGroup::Layout,
            properties.layout_container,
            |value| layout_record(value.inspection_summary(), value.inspection_fields()),
        );
        self.push_property_row(
            &mut rows,
            element,
            "LayoutSize",
            ElementPropertyInspectionGroup::Layout,
            properties.layout_size,
            |value| layout_record(value.inspection_summary(), value.inspection_fields()),
        );
        self.push_property_row(
            &mut rows,
            element,
            "LayoutSpacing",
            ElementPropertyInspectionGroup::Layout,
            properties.layout_spacing,
            |value| layout_record(value.inspection_summary(), value.inspection_fields()),
        );
        self.push_property_row(
            &mut rows,
            element,
            "LayoutAlignment",
            ElementPropertyInspectionGroup::Layout,
            properties.layout_alignment,
            |value| layout_record(value.inspection_summary(), value.inspection_fields()),
        );
        self.push_property_row(
            &mut rows,
            element,
            "LayoutOverflow",
            ElementPropertyInspectionGroup::Layout,
            properties.layout_overflow,
            |value| layout_record(value.inspection_summary(), value.inspection_fields()),
        );
        self.push_property_row(
            &mut rows,
            element,
            "LayoutPlacement",
            ElementPropertyInspectionGroup::Layout,
            properties.layout_placement,
            |value| layout_record(value.inspection_summary(), value.inspection_fields()),
        );

        self.push_property_row(
            &mut rows,
            element,
            "Opacity",
            ElementPropertyInspectionGroup::Motion,
            properties.opacity,
            |value| number_value(f64::from(value)),
        );
        self.push_debug_property_row(
            &mut rows,
            element,
            "VisualTransform",
            ElementPropertyInspectionGroup::Motion,
            properties.visual_transform,
        );

        self.push_property_row(
            &mut rows,
            element,
            "IsChecked",
            ElementPropertyInspectionGroup::Control,
            properties.is_checked,
            bool_value,
        );
        self.push_property_row(
            &mut rows,
            element,
            "IsExpanded",
            ElementPropertyInspectionGroup::Control,
            properties.is_expanded,
            bool_value,
        );
        self.push_property_row(
            &mut rows,
            element,
            "IsEnabled",
            ElementPropertyInspectionGroup::Control,
            properties.is_enabled,
            bool_value,
        );
        self.push_property_row(
            &mut rows,
            element,
            "IsReadOnly",
            ElementPropertyInspectionGroup::Control,
            properties.is_read_only,
            bool_value,
        );
        self.push_property_row(
            &mut rows,
            element,
            "SelectedIndex",
            ElementPropertyInspectionGroup::Control,
            properties.selected_index,
            optional_index_value,
        );

        self.push_property_row(
            &mut rows,
            element,
            "Value",
            ElementPropertyInspectionGroup::Range,
            properties.value,
            number_value,
        );
        self.push_property_row(
            &mut rows,
            element,
            "Minimum",
            ElementPropertyInspectionGroup::Range,
            properties.minimum,
            number_value,
        );
        self.push_property_row(
            &mut rows,
            element,
            "Maximum",
            ElementPropertyInspectionGroup::Range,
            properties.maximum,
            number_value,
        );
        self.push_property_row(
            &mut rows,
            element,
            "SmallChange",
            ElementPropertyInspectionGroup::Range,
            properties.small_change,
            number_value,
        );
        self.push_property_row(
            &mut rows,
            element,
            "LargeChange",
            ElementPropertyInspectionGroup::Range,
            properties.large_change,
            number_value,
        );

        rows
    }

    fn push_debug_property_row<T>(
        &self,
        rows: &mut Vec<ElementPropertyInspectionRow>,
        element: &crate::Element,
        name: &'static str,
        group: ElementPropertyInspectionGroup,
        property: Property<T>,
    ) where
        T: Clone + core::fmt::Debug + PartialEq + 'static,
    {
        self.push_property_row(rows, element, name, group, property, |value| {
            ElementPropertyInspectionValue::Token(format!("{value:?}"))
        });
    }

    fn push_paragraph_direction_row(
        &self,
        rows: &mut Vec<ElementPropertyInspectionRow>,
        element: &crate::Element,
    ) {
        let property = self.properties.paragraph_direction;
        let resolved = self.explain_value_for(element, property);
        let base_direction = paragraph_base_direction(element, self);
        if base_direction == BaseDirection::Ltr
            && !self.should_include_property_row(element, property, &resolved.value)
        {
            return;
        }
        let policy = resolved.value.inspection_summary();
        let base = base_direction_summary(base_direction);
        rows.push(ElementPropertyInspectionRow {
            name: "ParagraphDirection",
            group: ElementPropertyInspectionGroup::Text,
            value: layout_record(
                format!("{policy} → {base}"),
                vec![
                    ("policy", String::from(policy)),
                    ("resolved", String::from(base)),
                ],
            ),
            origin: property_inspection_origin(&resolved.source),
        });
    }

    fn push_surface_property_rows(
        &self,
        rows: &mut Vec<ElementPropertyInspectionRow>,
        element: &crate::Element,
    ) {
        let surface = self.properties.surface;
        let appearance = ElementPropertyInspectionGroup::Appearance;
        let box_model = ElementPropertyInspectionGroup::Box;

        self.push_property_row(
            rows,
            element,
            "Background",
            appearance,
            surface.background,
            option_brush_value,
        );

        for (name, property) in [
            ("Border top brush", surface.border_brushes.top),
            ("Border right brush", surface.border_brushes.right),
            ("Border bottom brush", surface.border_brushes.bottom),
            ("Border left brush", surface.border_brushes.left),
        ] {
            self.push_property_row(rows, element, name, box_model, property, option_brush_value);
        }

        for (name, property) in [
            ("Border top style", surface.border_styles.top),
            ("Border right style", surface.border_styles.right),
            ("Border bottom style", surface.border_styles.bottom),
            ("Border left style", surface.border_styles.left),
        ] {
            self.push_property_row(rows, element, name, box_model, property, border_style_value);
        }

        for (name, property) in [
            ("Border top width", surface.border_widths.top),
            ("Border right width", surface.border_widths.right),
            ("Border bottom width", surface.border_widths.bottom),
            ("Border left width", surface.border_widths.left),
        ] {
            self.push_property_row(rows, element, name, box_model, property, number_value);
        }

        for (name, property) in [
            ("Padding top", surface.padding_widths.top),
            ("Padding right", surface.padding_widths.right),
            ("Padding bottom", surface.padding_widths.bottom),
            ("Padding left", surface.padding_widths.left),
        ] {
            self.push_property_row(rows, element, name, box_model, property, number_value);
        }

        for (name, property) in [
            ("Top-left corner radius", surface.corner_radii.top_left),
            ("Top-right corner radius", surface.corner_radii.top_right),
            (
                "Bottom-right corner radius",
                surface.corner_radii.bottom_right,
            ),
            (
                "Bottom-left corner radius",
                surface.corner_radii.bottom_left,
            ),
        ] {
            self.push_property_row(
                rows,
                element,
                name,
                box_model,
                property,
                corner_radius_value,
            );
        }

        for (name, property) in [
            ("Top-left corner shape", surface.corner_shapes.top_left),
            ("Top-right corner shape", surface.corner_shapes.top_right),
            (
                "Bottom-right corner shape",
                surface.corner_shapes.bottom_right,
            ),
            (
                "Bottom-left corner shape",
                surface.corner_shapes.bottom_left,
            ),
        ] {
            self.push_property_row(rows, element, name, box_model, property, corner_shape_value);
        }
    }

    fn push_property_row<T>(
        &self,
        rows: &mut Vec<ElementPropertyInspectionRow>,
        element: &crate::Element,
        name: &'static str,
        group: ElementPropertyInspectionGroup,
        property: Property<T>,
        value: impl FnOnce(T) -> ElementPropertyInspectionValue,
    ) where
        T: Clone + PartialEq + 'static,
    {
        let resolved = self.explain_value_for(element, property);
        if !self.should_include_property_row(element, property, &resolved.value) {
            return;
        }
        rows.push(ElementPropertyInspectionRow {
            name,
            group,
            value: value(resolved.value),
            origin: property_inspection_origin(&resolved.source),
        });
    }

    fn should_include_property_row<T>(
        &self,
        element: &crate::Element,
        property: Property<T>,
        value: &T,
    ) -> bool
    where
        T: Clone + PartialEq + 'static,
    {
        let store = element.property_store();
        if store.winning_local_source_for_id(property.id()).is_some()
            || store.animation_entries_has(property.id())
        {
            return true;
        }
        self.registry
            .get_metadata(property)
            .is_some_and(|metadata| metadata.default_value() != value)
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
            active_transitions: self.timeline.active_transition_count(),
            active_effects: self.timeline.active_effect_count(),
            pending_transitions: self.timeline.pending_transition_count(),
            pending_cues: self.timeline.pending_cue_count(),
            animations: self
                .timeline
                .animation_rows(self.now())
                .into_iter()
                .map(|row| TimelineAnimationInspectionRow {
                    id: row.id(),
                    element: row.key().element(),
                    target: row.key().target(),
                    state: row.state(),
                    current_time: row.current_time(),
                    timing_sample: row.timing_sample(),
                    playback_rate: row.playback_rate(),
                    composite: row.composite(),
                    value_kind: row.value_kind(),
                    timeline: row.timeline(),
                })
                .collect(),
        }
    }
}

fn option_brush_value(value: Option<Brush>) -> ElementPropertyInspectionValue {
    value.map_or(ElementPropertyInspectionValue::Empty, brush_value)
}

fn brush_value(value: Brush) -> ElementPropertyInspectionValue {
    match value {
        Brush::Solid(color) => ElementPropertyInspectionValue::Color(color),
        Brush::Gradient(_) => ElementPropertyInspectionValue::Token(String::from("gradient")),
        Brush::Image(_) => ElementPropertyInspectionValue::Token(String::from("image")),
    }
}

fn text_value(value: String) -> ElementPropertyInspectionValue {
    if value.is_empty() {
        ElementPropertyInspectionValue::Empty
    } else {
        ElementPropertyInspectionValue::Text(value)
    }
}

fn bool_value(value: bool) -> ElementPropertyInspectionValue {
    ElementPropertyInspectionValue::Bool(value)
}

fn number_value(value: f64) -> ElementPropertyInspectionValue {
    ElementPropertyInspectionValue::Number(value)
}

fn corner_radius_value(value: CornerRadius) -> ElementPropertyInspectionValue {
    if value.horizontal == value.vertical {
        number_value(value.horizontal)
    } else {
        let mut fields = Vec::with_capacity(2);
        fields.push(ElementPropertyInspectionField {
            name: "horizontal",
            value: number_value(value.horizontal),
        });
        fields.push(ElementPropertyInspectionField {
            name: "vertical",
            value: number_value(value.vertical),
        });
        ElementPropertyInspectionValue::Record {
            summary: format!("{} x {}", value.horizontal, value.vertical),
            fields,
        }
    }
}

fn corner_shape_value(value: CornerShape) -> ElementPropertyInspectionValue {
    ElementPropertyInspectionValue::Token(format!("{value:?}"))
}

fn border_style_value(value: BorderStyle) -> ElementPropertyInspectionValue {
    ElementPropertyInspectionValue::Token(format!("{value:?}"))
}

const fn base_direction_summary(direction: BaseDirection) -> &'static str {
    match direction {
        BaseDirection::Auto => "auto",
        BaseDirection::Ltr => "ltr",
        BaseDirection::Rtl => "rtl",
    }
}

fn count_value(value: usize) -> ElementPropertyInspectionValue {
    if value == 0 {
        ElementPropertyInspectionValue::Empty
    } else {
        ElementPropertyInspectionValue::Count(value)
    }
}

fn layout_record(
    summary: String,
    fields: Vec<(&'static str, String)>,
) -> ElementPropertyInspectionValue {
    ElementPropertyInspectionValue::Record {
        summary,
        fields: fields
            .into_iter()
            .map(|(name, value)| ElementPropertyInspectionField {
                name,
                value: ElementPropertyInspectionValue::Token(value),
            })
            .collect(),
    }
}

fn optional_index_value(value: Option<usize>) -> ElementPropertyInspectionValue {
    value.map_or(ElementPropertyInspectionValue::Empty, |value| {
        ElementPropertyInspectionValue::Text(format!("{value}"))
    })
}
