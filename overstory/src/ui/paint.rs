// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Presentation collection and lowering into an imaging scene.

use alloc::vec::Vec;
use core::cell::RefCell;

use imaging::record::Scene;
use kurbo::{Affine, Rect};
use subduction_core::layer::SurfaceId;
use understory_box_tree::NodeId;
use understory_presentation::{PresentationStore, Primitive, TextPrimitive};

use crate::behavior::PresentContext;
use crate::channels::{COMPOSE, PAINT};
use crate::element::ElementId;
use crate::lookup::UiResolveLookup;
use crate::text::TextEngine;

use super::Ui;

fn plain_text_wraps(plain: &understory_presentation::PlainTextPrimitive) -> bool {
    plain.style.text_wrap_mode != understory_presentation::TextStyle::default().text_wrap_mode
}

/// Lowers a node's resolved presentation primitives into `output`,
/// positioned at `bounds`. Stand-in for a future `imaging`-side
/// renderer that consumes `understory_presentation` directly.
fn lower_presentation(
    primitives: &[Primitive<u64>],
    bounds: Rect,
    output: &mut Scene,
    text: &RefCell<TextEngine>,
) {
    use imaging::Painter;
    use kurbo::RoundedRect;

    let mut painter = Painter::new(output);
    for primitive in primitives {
        match primitive {
            Primitive::Surface(surface) => {
                let corner_radius = surface.corner_radii.top_left;
                for layer in &surface.backgrounds {
                    if corner_radius > 0.0 {
                        let rounded = RoundedRect::from_rect(bounds, corner_radius);
                        painter.fill(rounded, &layer.brush).draw();
                    } else {
                        painter.fill(bounds, &layer.brush).draw();
                    }
                }
                // Border. We only lower a uniform border for now —
                // the same brush + width on all four sides. The top
                // side carries the source of truth and the other
                // sides are ignored. Stroked centered on the surface
                // edge, which leaks half the width outside `bounds`;
                // a more polished pass would inset by `width / 2`.
                let side = &surface.border.top;
                if let Some(brush) = side.brush.as_ref()
                    && side.width > 0.0
                {
                    let stroke = kurbo::Stroke::new(side.width);
                    if corner_radius > 0.0 {
                        let rounded = RoundedRect::from_rect(bounds, corner_radius);
                        painter.stroke(rounded, &stroke, brush).draw();
                    } else {
                        painter.stroke(bounds, &stroke, brush).draw();
                    }
                }
                // Shadow lowering — TODO when controls need it.
            }
            Primitive::Text(boxed) => {
                if let TextPrimitive::Plain(plain) = boxed.as_ref() {
                    if plain.content.is_empty() {
                        continue;
                    }
                    let Some(fg) = plain.foreground.as_ref() else {
                        continue;
                    };
                    let layout = text.borrow_mut().shape(
                        plain.content.as_str(),
                        plain.style.font_size,
                        if plain_text_wraps(plain) {
                            Some(bounds.width())
                        } else {
                            None
                        },
                    );
                    let origin = bounds.origin();
                    crate::text::render_text(
                        &mut painter,
                        Affine::translate((origin.x, origin.y)),
                        &layout,
                        fg,
                    );
                }
            }
            // Image / future primitives land when controls need them.
            _ => {}
        }
    }
}

impl Ui {
    pub(crate) fn with_plain_editor<R>(
        &self,
        editor: &mut parley::PlainEditor<peniko::Brush>,
        f: impl FnOnce(&mut parley::PlainEditorDriver<'_, peniko::Brush>) -> R,
    ) -> R {
        self.text.borrow_mut().with_plain_editor(editor, f)
    }

    /// Runs the present pass, rebuilds composition surface contents, and
    /// composites the layer tree into `output`.
    ///
    /// Steps:
    ///
    /// 1. Flush any pending STYLE work (it may dirty PAINT).
    /// 2. For every PAINT-dirty element, call its behavior's `present`
    ///    so it resolves properties through the cascade and writes
    ///    primitives into [`PresentationStore`].
    /// 3. Drain the presentation store's dirty set (callers can hook
    ///    in here later for damage-driven re-lowering).
    /// 4. Rebuild each PAINT-dirty element's composition surface in
    ///    layer-local coordinates, lowering only the element's own
    ///    presentation primitives.
    /// 5. Traverse the Subduction layer tree, appending attached scene
    ///    surfaces into `output` through layer transforms and clips.
    /// 6. Clear COMPOSE.
    pub fn paint(&mut self, output: &mut Scene) {
        self.flush_bindings();
        self.flush_style();
        // Make sure the box tree's world-space data and spatial index
        // reflect any structural / bounds changes since the last
        // commit. `compute_layout` already commits at the end, so
        // when paint follows layout this is a cheap no-op; tests
        // that paint without layout (or callers exploring partial
        // pipelines) get a correct commit on demand.
        self.geometry.commit();
        self.flush_composition();
        let dirty: Vec<ElementId> = self.invalidation.drain_sorted(PAINT).collect();
        for &id in &dirty {
            self.present_element(id);
        }
        // We don't yet drive incremental re-lowering off this, but
        // draining keeps the dirty set bounded.
        let _: Vec<NodeId> = self.presentation.drain_dirty();

        self.composition.clear_surface_updates();
        for id in dirty {
            let Some(surface) = self.composition.owned_scene_surface_for(id) else {
                continue;
            };
            let mut scene = self.composition.take_surface(surface);
            scene.clear();
            self.lower_element_surface(id, &mut scene);
            self.composition.restore_surface(surface, scene);
        }
        self.composition.render_into(output);
        self.invalidation.clear(COMPOSE);
    }

    /// Composes the current layer tree into `output` using caller-owned
    /// surface scenes.
    ///
    /// Hosts use this after [`Self::paint`] has refreshed
    /// [`Self::composition_surface_updates`]. The lookup is called for each
    /// `SurfaceId` attached to a visible composition layer; returning `None`
    /// skips that layer's content while preserving descendants.
    pub fn render_composition_with<'a>(
        &self,
        output: &mut Scene,
        surface_scene: impl FnMut(SurfaceId) -> Option<&'a Scene>,
    ) {
        self.composition.render_into_with(output, surface_scene);
    }

    /// Runs `behavior.present` for one element, updating its
    /// presentation node in the store. If the element or its behavior
    /// is gone we drop silently; the presentation entry is removed in
    /// `remove_subtree` separately.
    fn present_element(&mut self, id: ElementId) {
        // Take the presentation store out first so we can later hold
        // both a `&Ui` borrow (for `UiResolveLookup`) and a
        // `&mut PresentationStore` simultaneously. Always put it back
        // before returning.
        let mut presentation = self.presentation.take_presentation();
        self.present_element_inner(id, &mut presentation);
        self.presentation.restore_presentation(presentation);
    }

    fn present_element_inner(
        &self,
        id: ElementId,
        presentation: &mut PresentationStore<NodeId, ElementId>,
    ) {
        let Some(element) = self.get(id) else {
            return;
        };
        let Some(node) = self.box_for(id) else {
            return;
        };
        let match_state = self.style.match_state(id);
        let ctx = PresentContext {
            registry: &self.registry,
            properties: &self.properties,
            theme: self.style.theme(),
            cascade: self.style.cascade(),
            match_state,
            lookup: UiResolveLookup::new(self),
            animations: &self.animations,
            now: self.now(),
        };
        if let Some(widget) = element.widget() {
            widget.present(element, &ctx, presentation, node);
        } else {
            // Anonymous container: still render its surface from the
            // cascade so panels with `Background` rules paint.
            crate::generic::present_surface(element, &ctx, presentation, node);
        }
    }

    fn lower_element_surface(&self, id: ElementId, output: &mut Scene) {
        let Some(node) = self.box_for(id) else {
            return;
        };
        let Some(size) = self.composition.bounds_for(id) else {
            return;
        };
        let bounds = Rect::from_origin_size((0.0, 0.0), size);
        if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
            return;
        }
        let Some(presentation) = self.presentation.presentation(node) else {
            return;
        };
        lower_presentation(presentation.primitives(), bounds, output, &self.text);
    }
}
