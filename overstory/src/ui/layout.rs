// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Layout computation, bounds writes, and box-tree geometry queries.

use alloc::vec::Vec;

use kurbo::{Point, Rect, RoundedRect, Size, Vec2};
use understory_box_tree::NodeId;

use crate::animation_runtime::PresentationProperty;
use crate::behavior::{KnownDimensions, MeasureConstraints};
use crate::channels::LAYOUT;
use crate::element::ElementId;
use crate::layouts::Layout;

use super::Ui;

fn available_to_f64(s: taffy::AvailableSpace) -> f64 {
    match s {
        taffy::AvailableSpace::Definite(v) => f64::from(v),
        taffy::AvailableSpace::MinContent | taffy::AvailableSpace::MaxContent => f64::INFINITY,
    }
}

impl Ui {
    /// Sets an element's layout contribution.
    ///
    /// This is the app-facing shortcut for writing
    /// [`BuiltInProperties::layout`](crate::BuiltInProperties::layout);
    /// custom controls may still use [`Self::set_local`] when they are
    /// working generically with dependency properties.
    pub fn set_layout(&mut self, id: ElementId, layout: impl Into<Layout>) {
        let layout_prop = self.properties.layout;
        self.set_local(id, layout_prop, layout.into());
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
        let hit = self.geometry.hit_test(point)?;
        self.node_to_element(hit.node)
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
        self.effective_value_for(element, self.properties.bounds)
    }

    /// Resolves the effective `Layout` for `id` through the cascade.
    #[must_use]
    pub fn effective_layout(&self, id: ElementId) -> Layout {
        let Some(element) = self.get(id) else {
            return Layout::default();
        };
        self.effective_value_for(element, self.properties.layout)
    }

    /// Computes layout for the tree against the given viewport size and
    /// writes each element's `Bounds` (via the cascade-resolved `Layout`).
    ///
    /// Drains pending property bindings first so binding-driven writes
    /// participate in this frame's layout. Then flushes the LAYOUT
    /// channel: any element whose `Layout` has changed gets its engine style
    /// pushed before layout is recomputed. `Bounds` writes use the standard
    /// `set_local_notifying` path so PAINT cascades naturally for elements
    /// whose box changed.
    pub fn compute_layout(&mut self, viewport: Size) {
        self.last_viewport = viewport;
        self.compute_layout_until_stable(viewport);
        if self.reposition_overlays(viewport) {
            self.compute_layout_until_stable(viewport);
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
        // Bindings first — they may dirty LAYOUT/PAINT for their targets.
        self.flush_bindings();
        // Push any pending Layout changes into the layout engine. We collect
        // first to separate the immutable cascade walk from the mutable engine
        // update.
        let dirty: Vec<ElementId> = self.invalidation.drain_sorted(LAYOUT).collect();
        let updates: Vec<(taffy::NodeId, taffy::Style)> = dirty
            .into_iter()
            .filter_map(|id| {
                let taffy_node = self.layout.node_for(id)?;
                let style = self.effective_layout(id).to_taffy();
                Some((taffy_node, style))
            })
            .collect();
        for (node, style) in updates {
            self.layout.set_style(node, style);
        }

        let Some(root_taffy) = self.layout.node_for(self.root()) else {
            return false;
        };
        let available = taffy::Size {
            #[expect(clippy::cast_possible_truncation, reason = "viewport size fits f32")]
            width: taffy::AvailableSpace::Definite(viewport.width as f32),
            #[expect(clippy::cast_possible_truncation, reason = "viewport size fits f32")]
            height: taffy::AvailableSpace::Definite(viewport.height as f32),
        };

        // Run taffy with a measure callback that dispatches to the elements'
        // widgets. Split-borrow the disjoint Ui fields so the closure can
        // read everything it needs while the taffy tree is &mut.
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
                    let match_state = style.match_state(id);
                    let measure_ctx = crate::behavior::MeasureContext {
                        registry,
                        properties,
                        theme: style.theme(),
                        cascade: style.cascade(),
                        match_state,
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
                        #[expect(
                            clippy::cast_possible_truncation,
                            reason = "measured size fits f32"
                        )]
                        Some(size) => taffy::Size {
                            width: size.width as f32,
                            height: size.height as f32,
                        },
                        None => taffy::Size::ZERO,
                    }
                },
            );
        }

        // Walk the tree and write absolute bounds for every element.
        self.write_bounds_recursive(self.root(), Point::ZERO, Vec2::ZERO);
        self.run_post_layout_widgets()
    }

    fn run_post_layout_widgets(&mut self) -> bool {
        let mut ids = Vec::new();
        self.collect_subtree_ids(self.root(), &mut ids);
        let mut changed = false;
        for id in ids {
            if self.get(id).is_none() {
                continue;
            }
            let mut widget_changed = false;
            self.with_widget_mut(id, |widget, ui| {
                widget_changed = widget.post_layout(id, ui);
            });
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
        // Composition can use a sampled presentation value while layout and hit
        // testing keep the resolved target geometry.
        let composition_offset = self.presented_composition_offset(id, local_offset);
        self.composition.set_geometry(
            id,
            composition_offset,
            Vec2::new(rect.x0, rect.y0),
            rect.size(),
            local_clip,
        );
        let opacity = self.presented_scalar(id, PresentationProperty::LayerOpacity, 1.0);
        self.composition.set_opacity(id, opacity);

        let children: Vec<ElementId> = self
            .get(id)
            .map(|el| el.children().to_vec())
            .unwrap_or_default();
        let child_origin = origin + child_offset;
        for child in children {
            self.write_bounds_recursive(child, child_origin, child_offset);
        }

        if let Some(element) = self.get(id)
            && let Some(widget) = element.widget()
        {
            let ctx = crate::behavior::LayoutContext { ui: self };
            widget.layout_updated(element, &ctx);
        }
    }

    fn presented_composition_offset(&self, id: ElementId, local_offset: Vec2) -> Vec2 {
        self.presented_offset(id, PresentationProperty::LayerOffset, local_offset)
    }
}
