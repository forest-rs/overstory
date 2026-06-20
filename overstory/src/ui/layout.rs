// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Layout computation, bounds writes, and box-tree geometry queries.

use alloc::vec::Vec;

use kurbo::{Point, Rect, RoundedRect, Size, Vec2};
use understory_box_tree::NodeId;
use understory_presentation::Edges;

use crate::behavior::{KnownDimensions, MeasureConstraints};
use crate::channels::{ARRANGE, LAYOUT};
use crate::element::{Element, ElementId};
use crate::layouts::{
    Layout, LayoutAlignment, LayoutContainer, LayoutEdges, LayoutOverflow, LayoutPlacement,
    LayoutSize, LayoutSpacing, taffy_scalar,
};
use crate::motion::{PresentationSlot, PresentationTarget};

use super::Ui;

fn available_to_f64(s: taffy::AvailableSpace) -> f64 {
    match s {
        taffy::AvailableSpace::Definite(v) => f64::from(v),
        taffy::AvailableSpace::MinContent | taffy::AvailableSpace::MaxContent => f64::INFINITY,
    }
}

fn finite_non_negative(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

impl Ui {
    /// Sets an element's layout contribution.
    ///
    /// This is the app-facing shortcut for writing layout longhands. Only
    /// contributions explicitly carried by `layout` are written, so a size
    /// update does not shadow styled padding, alignment, or placement.
    pub fn set_layout(&mut self, id: ElementId, layout: impl Into<Layout>) {
        let parts = layout.into().into_parts();
        if let Some(container) = parts.container {
            self.set_layout_container(id, container);
        }
        if let Some(size) = parts.size {
            self.set_layout_size(id, size);
        }
        if let Some(spacing) = parts.spacing {
            self.set_layout_spacing(id, spacing);
        }
        if let Some(alignment) = parts.alignment {
            self.set_layout_alignment(id, alignment);
        }
        if let Some(overflow) = parts.overflow {
            self.set_layout_overflow(id, overflow);
        }
        if let Some(placement) = parts.placement {
            self.set_layout_placement(id, placement);
        }
    }

    /// Sets an element's container-layout contribution.
    pub fn set_layout_container(&mut self, id: ElementId, container: impl Into<LayoutContainer>) {
        self.set_local(id, self.properties.layout_container, container.into());
    }

    /// Sets an element's size and flex-item contribution.
    pub fn set_layout_size(&mut self, id: ElementId, size: impl Into<LayoutSize>) {
        self.set_local(id, self.properties.layout_size, size.into());
    }

    /// Sets an element's margin and child-gap contribution.
    pub fn set_layout_spacing(&mut self, id: ElementId, spacing: impl Into<LayoutSpacing>) {
        self.set_local(id, self.properties.layout_spacing, spacing.into());
    }

    /// Sets the canonical box padding used by both layout and surface
    /// decoration geometry.
    ///
    /// The values are physical top/right/bottom/left logical pixels. Layout
    /// combines this padding with resolved border widths to derive the Taffy
    /// content inset; painting resolves the same properties into
    /// `SurfacePrimitive::padding_widths`.
    pub fn set_box_padding(&mut self, id: ElementId, padding: Edges<f64>) {
        let properties = self.properties.surface.padding_widths;
        self.set_local(id, properties.top, padding.top);
        self.set_local(id, properties.right, padding.right);
        self.set_local(id, properties.bottom, padding.bottom);
        self.set_local(id, properties.left, padding.left);
    }

    /// Sets an element's child and self-alignment contribution.
    pub fn set_layout_alignment(&mut self, id: ElementId, alignment: impl Into<LayoutAlignment>) {
        self.set_local(id, self.properties.layout_alignment, alignment.into());
    }

    /// Sets an element's overflow contribution.
    pub fn set_layout_overflow(&mut self, id: ElementId, overflow: impl Into<LayoutOverflow>) {
        self.set_local(id, self.properties.layout_overflow, overflow.into());
    }

    /// Sets an element's layout placement contribution.
    ///
    /// This controls flow-vs-absolute positioning independently from the
    /// element's sizing, padding, and child layout. If the element has a
    /// style-authored [`crate::motion::PresentationTarget::layer_offset`] rule,
    /// changing placement also requests a composition-sampled placement
    /// transition for the next layout pass.
    pub fn set_layout_placement(&mut self, id: ElementId, placement: impl Into<LayoutPlacement>) {
        self.request_styled_presentation_transition(
            id,
            PresentationTarget::Slot(PresentationSlot::LayerOffset),
        );
        self.set_layout_placement_raw(id, placement);
    }

    /// Sets an element's layout placement without requesting presentation
    /// motion, cancelling any active layer-offset transition for the element.
    pub(crate) fn set_layout_placement_without_motion(
        &mut self,
        id: ElementId,
        placement: impl Into<LayoutPlacement>,
    ) {
        self.clear_presentation_target(id, PresentationTarget::Slot(PresentationSlot::LayerOffset));
        self.mark_channels(id, ARRANGE.into());
        self.set_layout_placement_raw(id, placement);
    }

    fn set_layout_placement_raw(&mut self, id: ElementId, placement: impl Into<LayoutPlacement>) {
        let placement_prop = self.properties.layout_placement;
        self.set_local(id, placement_prop, placement.into());
    }

    /// Hit-tests `point` against the box tree and returns the topmost,
    /// deepest element whose node covers it.
    ///
    /// Internally delegates to `understory_box_tree`'s spatial index.
    /// Returns `None` if no node is hit. Requires that the most recent
    /// geometry mutation has been committed; `compute_layout` commits as its
    /// last step, and read-only consumers in tests either go through layout or
    /// write nothing first.
    #[must_use]
    pub fn hit_test(&self, point: Point) -> Option<ElementId> {
        self.responder().hit_test(point)
    }

    /// Returns the element that owns a given `NodeId`, if any. Helper
    /// for callers that bridge between the box tree's id-space and
    /// the element id-space (e.g. responder integration).
    #[must_use]
    pub fn node_to_element(&self, node: NodeId) -> Option<ElementId> {
        self.geometry.node_to_element(node)
    }

    /// Returns the primary box-tree node for `id`, or `None` for
    /// stale ids.
    #[must_use]
    pub fn box_for(&self, id: ElementId) -> Option<NodeId> {
        self.geometry.box_for(id)
    }

    /// Resolves the effective `Bounds` for `id` through the cascade.
    #[must_use]
    pub fn effective_bounds(&self, id: ElementId) -> Rect {
        let Some(element) = self.get(id) else {
            return Rect::ZERO;
        };
        self.resolve_value_for(element, self.properties.bounds)
    }

    /// Resolves the complete effective `Layout` for `id` through the cascade.
    #[must_use]
    pub fn effective_layout(&self, id: ElementId) -> Layout {
        let Some(element) = self.get(id) else {
            return Layout::default();
        };
        self.effective_layout_for_element(element)
    }

    fn effective_layout_for_element(&self, element: &Element) -> Layout {
        let mut layout = self.resolve_value_for(element, self.properties.layout);
        layout.merge_container(self.resolve_value_for(element, self.properties.layout_container));
        layout.merge_size(self.resolve_value_for(element, self.properties.layout_size));
        layout.merge_spacing(self.resolve_value_for(element, self.properties.layout_spacing));
        layout.merge_alignment(self.resolve_value_for(element, self.properties.layout_alignment));
        layout.merge_overflow(self.resolve_value_for(element, self.properties.layout_overflow));
        layout.merge_placement(self.resolve_value_for(element, self.properties.layout_placement));
        layout
    }

    fn effective_layout_for_taffy(&self, id: ElementId) -> Option<Layout> {
        let element = self.get(id)?;
        let mut layout = self.effective_layout_for_element(element);
        if let Some(inset) = self.effective_box_layout_inset(element) {
            layout.merge_spacing(LayoutSpacing::new().padding(inset));
        }
        Some(layout)
    }

    /// Resolves the effective `LayoutPlacement` for `id` through the cascade.
    #[must_use]
    pub fn effective_layout_placement(&self, id: ElementId) -> LayoutPlacement {
        self.effective_layout(id).placement()
    }

    /// Computes layout for the tree against the given viewport size and
    /// writes each element's `Bounds` (via the cascade-resolved `Layout`).
    ///
    /// Drains pending property bindings first so binding-driven writes
    /// participate in this frame's layout. Then flushes the LAYOUT
    /// channel: any element whose `Layout` has changed gets its engine style
    /// pushed before layout is recomputed. Arrange-only invalidations, such as
    /// scroll offsets, reuse the previous layout results and only rewrite
    /// world-space bounds, hit-test geometry, and composition geometry.
    /// `Bounds` writes use the standard `set_local_notifying` path so
    /// GEOMETRY/PAINT cascade naturally for elements whose box changed.
    pub fn compute_layout(&mut self, viewport: Size) {
        self.flush_bindings();
        if self.last_viewport != viewport {
            self.last_viewport = viewport;
            self.mark_channels(self.root(), LAYOUT.into());
        }
        let needs_layout = self.invalidation.has_invalidated(LAYOUT);
        self.frame_stats.layout_requested |= needs_layout;
        if needs_layout {
            self.compute_layout_until_stable(viewport);
            if self.reposition_overlays(viewport) {
                self.compute_layout_until_stable(viewport);
            }
        } else if self.invalidation.has_invalidated(ARRANGE) {
            self.write_arranged_bounds();
        }
        // Finalize the box tree: refresh world-space data and the
        // spatial index so the next `hit_test` (or any other tree
        // query) reflects this frame's geometry.
        self.geometry.commit();
        self.flush_composition();
    }

    fn compute_layout_until_stable(&mut self, viewport: Size) {
        for _ in 0..4 {
            if !self.compute_layout_pass(viewport) {
                return;
            }
        }
    }

    fn compute_layout_pass(&mut self, viewport: Size) -> bool {
        self.frame_stats.layout_passes += 1;
        // Bindings first — they may dirty LAYOUT/GEOMETRY/PAINT for their
        // targets.
        self.flush_bindings();
        // Push any pending Layout changes into the layout engine. We collect
        // first to separate the immutable cascade walk from the mutable engine
        // update.
        let dirty: Vec<ElementId> = self.invalidation.drain_sorted(LAYOUT).collect();
        let updates: Vec<(taffy::NodeId, taffy::Style)> = dirty
            .into_iter()
            .filter_map(|id| {
                let taffy_node = self.layout.node_for(id)?;
                let style = self.effective_layout_for_taffy(id)?.to_taffy();
                Some((taffy_node, style))
            })
            .collect();
        self.frame_stats.layout_style_updates += updates.len();
        for (node, style) in updates {
            self.layout.set_style(node, style);
        }

        let Some(root_taffy) = self.layout.node_for(self.root()) else {
            return false;
        };
        let available = taffy::Size {
            width: taffy::AvailableSpace::Definite(taffy_scalar(viewport.width)),
            height: taffy::AvailableSpace::Definite(taffy_scalar(viewport.height)),
        };

        // Run taffy with a measure callback that dispatches to the elements'
        // widgets. Split-borrow the disjoint Ui fields so the closure can
        // read everything it needs while the taffy tree is &mut.
        let mut measured_widgets = 0_usize;
        {
            let taffy = self.layout.taffy_mut();
            let registry = &self.registry;
            let properties = &self.properties;
            let style = &self.style;
            let elements = &self.elements;
            let text = &self.text;
            let _ = taffy.compute_layout_with_measure(
                root_taffy,
                available,
                |known, avail, _node_id, ctx_id, _style| {
                    let Some(&id) = ctx_id.map(|v| &*v) else {
                        return taffy::Size::ZERO;
                    };
                    let Some(element) = elements.get(id) else {
                        return taffy::Size::ZERO;
                    };
                    let Some(widget) = element.widget() else {
                        // Anonymous container: defer to taffy's
                        // default style-based sizing.
                        return taffy::Size::ZERO;
                    };
                    measured_widgets += 1;
                    let match_state = style.match_state(id);
                    let measure_ctx = crate::behavior::MeasureContext {
                        registry,
                        properties,
                        theme: style.theme(),
                        cascade: style.cascade(),
                        match_state,
                        lookup: crate::lookup::ElementResolveLookup::new(elements, style),
                        text,
                    };
                    let constraints = MeasureConstraints {
                        available: Size::new(
                            available_to_f64(avail.width),
                            available_to_f64(avail.height),
                        ),
                        known: KnownDimensions {
                            width: known.width.map(f64::from),
                            height: known.height.map(f64::from),
                        },
                    };
                    match widget.measure(element, &measure_ctx, constraints) {
                        Some(size) => taffy::Size {
                            width: taffy_scalar(size.width),
                            height: taffy_scalar(size.height),
                        },
                        None => taffy::Size::ZERO,
                    }
                },
            );
        }
        self.frame_stats.measured_widgets += measured_widgets;

        self.write_arranged_bounds();
        let arranged = self.run_arrange_widgets();
        arranged || self.run_element_realization()
    }

    fn run_arrange_widgets(&mut self) -> bool {
        let mut ids = Vec::new();
        self.collect_subtree_ids(self.root(), &mut ids);
        let mut changed = false;
        for id in ids {
            if self.get(id).is_none() {
                continue;
            }
            self.with_widget_mut(id, |widget, ui| {
                let mut ctx = crate::behavior::ArrangeContext::new(id, ui);
                changed |= widget.arrange(&mut ctx).layout_changed();
            });
        }
        changed
    }

    fn run_element_realization(&mut self) -> bool {
        let mut ids = Vec::new();
        self.collect_subtree_ids(self.root(), &mut ids);
        let mut changed = false;
        for id in ids {
            if self.get(id).is_none() {
                continue;
            }
            let mut widget_changed = false;
            self.with_widget_mut(id, |widget, ui| {
                let mut ctx = crate::behavior::ElementRealizeContext::new(id, ui);
                widget_changed = widget.realize_elements(&mut ctx).changed();
            });
            self.frame_stats.element_realize_visits += 1;
            self.frame_stats.element_realize_changes += usize::from(widget_changed);
            changed |= widget_changed;
        }
        changed
    }

    fn collect_subtree_ids(&self, id: ElementId, ids: &mut Vec<ElementId>) {
        let Some(element) = self.get(id) else {
            return;
        };
        ids.push(id);
        for child in element.children() {
            self.collect_subtree_ids(*child, ids);
        }
    }

    fn write_arranged_bounds(&mut self) {
        let dirty: Vec<ElementId> = self.invalidation.drain_sorted(ARRANGE).collect();
        if dirty.is_empty() {
            return;
        }
        // Walk the tree and write absolute logical bounds for every element.
        // Physical pixels remain a host/backend projection so layout, hit
        // testing, composition, and accessibility share one coordinate space.
        //
        // This currently rewrites from the root even for an arrange-only
        // subtree change. That keeps scrollbars, scrolled content, overlays,
        // hit testing, and composition in sync without re-entering Taffy; once
        // the pipeline has subtree arrangement roots, this is the obvious
        // place to narrow the walk.
        self.write_bounds_recursive(self.root(), Point::ZERO, Vec2::ZERO);
    }

    fn write_bounds_recursive(
        &mut self,
        id: ElementId,
        parent_origin: Point,
        parent_child_offset: Vec2,
    ) {
        let Some(taffy_node) = self.layout.node_for(id) else {
            return;
        };
        let Some(layout) = self.layout.layout(taffy_node) else {
            return;
        };
        let local_offset = parent_child_offset
            + Vec2::new(f64::from(layout.location.x), f64::from(layout.location.y));
        let origin = Point::new(
            parent_origin.x + f64::from(layout.location.x),
            parent_origin.y + f64::from(layout.location.y),
        );
        let rect = Rect::new(
            origin.x,
            origin.y,
            origin.x + f64::from(layout.size.width),
            origin.y + f64::from(layout.size.height),
        );
        let bounds_prop = self.properties.bounds;
        self.set_local(id, bounds_prop, rect);
        self.frame_stats.bounds_written += 1;
        let child_offset = self
            .get(id)
            .and_then(|element| {
                element
                    .widget()
                    .map(|widget| widget.child_layout_offset(element))
            })
            .unwrap_or(Vec2::ZERO);
        let clips_subtree = self
            .get(id)
            .and_then(|element| element.widget().map(|widget| widget.clips_subtree(element)))
            .unwrap_or(false);

        // Mirror the world-space bounds into the box tree's primary
        // node for this element. Since every node is currently a
        // tree root with IDENTITY transform, `local_bounds == world_bounds`
        // and hit testing matches the old element-bounds walk.
        if let Some(primary) = self.box_for(id) {
            self.geometry.set_local_bounds(primary, rect);
            let clip = clips_subtree.then(|| RoundedRect::from_rect(rect, 0.0));
            self.geometry.set_local_clip(primary, clip);
        }
        let local_clip = clips_subtree.then(|| {
            RoundedRect::from_rect(
                Rect::from_origin_size((0.0, 0.0), Size::new(rect.width(), rect.height())),
                0.0,
            )
        });
        // Composition can use a sampled timeline value while layout and hit
        // testing keep the resolved target geometry.
        self.sync_composition_offset_target(id, local_offset);
        let composition_offset = self.presented_composition_offset(id, local_offset);
        self.composition.set_geometry(
            id,
            composition_offset,
            Vec2::new(rect.x0, rect.y0),
            rect.size(),
            local_clip,
            self.presented_visual_transform(id),
        );
        let opacity = self.presented_layer_opacity(id);
        self.composition.set_opacity(id, opacity);

        let children: Vec<ElementId> = self
            .get(id)
            .map(|el| el.children().to_vec())
            .unwrap_or_default();
        let child_origin = origin + child_offset;
        for child in children {
            self.write_bounds_recursive(child, child_origin, child_offset);
        }
    }

    fn presented_composition_offset(&self, id: ElementId, local_offset: Vec2) -> Vec2 {
        self.presented_value(
            id,
            PresentationTarget::Slot(PresentationSlot::LayerOffset),
            local_offset,
        )
    }
}

impl Ui {
    fn effective_box_layout_inset(&self, element: &Element) -> Option<LayoutEdges> {
        let surface = self.properties.surface;
        let border = Edges::new(
            finite_non_negative(self.resolve_value_for(element, surface.border_widths.top)),
            finite_non_negative(self.resolve_value_for(element, surface.border_widths.right)),
            finite_non_negative(self.resolve_value_for(element, surface.border_widths.bottom)),
            finite_non_negative(self.resolve_value_for(element, surface.border_widths.left)),
        );
        let padding = Edges::new(
            finite_non_negative(self.resolve_value_for(element, surface.padding_widths.top)),
            finite_non_negative(self.resolve_value_for(element, surface.padding_widths.right)),
            finite_non_negative(self.resolve_value_for(element, surface.padding_widths.bottom)),
            finite_non_negative(self.resolve_value_for(element, surface.padding_widths.left)),
        );

        let inset = LayoutEdges::from_surface_edges(Edges::new(
            border.top + padding.top,
            border.right + padding.right,
            border.bottom + padding.bottom,
            border.left + padding.left,
        ));
        (!inset.is_zero()).then_some(inset)
    }
}
