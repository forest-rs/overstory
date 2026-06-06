// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Presentation collection and lowering into an imaging scene.

use alloc::vec::Vec;
use core::cell::RefCell;

use imaging::record::Scene;
use kurbo::{Affine, Rect};
use peniko::{Brush, Fill};
use subduction_core::layer::SurfaceId;
use understory_box_tree::NodeId;
use understory_presentation::{
    BorderSide, BorderSideGeometry, BoxContour, BoxDecorationGeometry, PathPaintOrder,
    PathPrimitive, PresentationStore, Primitive, Side, SurfacePrimitive, TextPrimitive,
};

use crate::behavior::PresentContext;
use crate::channels::{COMPOSE, GEOMETRY, PAINT};
use crate::element::ElementId;
use crate::lookup::UiResolveLookup;
use crate::text::TextEngine;

use super::Ui;

fn plain_text_wraps(plain: &understory_presentation::PlainTextPrimitive) -> bool {
    plain.style.text_wrap_mode != understory_presentation::TextStyle::default().text_wrap_mode
}

fn lower_path<S: imaging::PaintSink + ?Sized>(
    path: &PathPrimitive,
    painter: &mut imaging::Painter<'_, S>,
) {
    match path.paint_order {
        PathPaintOrder::FillThenStroke => {
            lower_path_fill(path, painter);
            lower_path_stroke(path, painter);
        }
        PathPaintOrder::StrokeThenFill => {
            lower_path_stroke(path, painter);
            lower_path_fill(path, painter);
        }
    }
}

fn lower_path_fill<S: imaging::PaintSink + ?Sized>(
    path: &PathPrimitive,
    painter: &mut imaging::Painter<'_, S>,
) {
    if let Some(fill) = path.fill.as_ref() {
        painter
            .fill(&path.path, &fill.brush)
            .transform(path.transform)
            .fill_rule(fill.rule)
            .draw();
    }
}

fn lower_path_stroke<S: imaging::PaintSink + ?Sized>(
    path: &PathPrimitive,
    painter: &mut imaging::Painter<'_, S>,
) {
    if let Some(stroke) = path.stroke.as_ref() {
        painter
            .stroke(&path.path, &stroke.stroke, &stroke.brush)
            .transform(path.transform)
            .draw();
    }
}

fn fill_contour<S: imaging::PaintSink + ?Sized>(
    contour: BoxContour,
    brush: &Brush,
    painter: &mut imaging::Painter<'_, S>,
) {
    if let Some(rounded) = contour.rounded_rect() {
        painter.fill(rounded, brush).draw();
    } else {
        let path = contour.to_path();
        painter.fill(&path, brush).draw();
    }
}

fn lower_surface_border<S: imaging::PaintSink + ?Sized>(
    surface: &SurfacePrimitive,
    geometry: BoxDecorationGeometry,
    painter: &mut imaging::Painter<'_, S>,
) {
    if let Some(side) = surface.border.uniform_visible_side() {
        let Some(brush) = side.brush.as_ref() else {
            return;
        };
        let path = geometry.to_border_ring_path();
        painter.fill(&path, brush).fill_rule(Fill::EvenOdd).draw();
        return;
    }

    for side in Side::ALL {
        let border_side = match side {
            Side::Top => &surface.border.top,
            Side::Right => &surface.border.right,
            Side::Bottom => &surface.border.bottom,
            Side::Left => &surface.border.left,
        };
        lower_border_side(geometry.border_side_region(side), border_side, painter);
    }
}

fn lower_border_side<S: imaging::PaintSink + ?Sized>(
    geometry: BorderSideGeometry,
    side: &BorderSide,
    painter: &mut imaging::Painter<'_, S>,
) {
    if geometry.is_empty() {
        return;
    }
    let Some(brush) = side.brush.as_ref() else {
        return;
    };

    let mut path = kurbo::BezPath::new();
    geometry.write_path(&mut path);
    painter.fill(&path, brush).draw();
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

    let mut painter = Painter::new(output);
    for primitive in primitives {
        match primitive {
            Primitive::Surface(surface) => {
                let geometry = surface.decoration_geometry(bounds);
                for layer in &surface.backgrounds {
                    fill_contour(geometry.border_edge, &layer.brush, &mut painter);
                }
                lower_surface_border(surface, geometry, &mut painter);
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
                        &plain.style,
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
            Primitive::Path(path) => {
                lower_path(path, &mut painter);
            }
            // Image / future primitives land when controls need them.
            _ => {}
        }
    }
}

impl Ui {
    pub(crate) fn with_plain_editor<R>(
        &self,
        editor: &mut parley::PlainEditor<Brush>,
        f: impl FnOnce(&mut parley::PlainEditorDriver<'_, Brush>) -> R,
    ) -> R {
        self.text.borrow_mut().with_plain_editor(editor, f)
    }

    /// Runs the present pass, rebuilds composition surface contents, and
    /// composites the layer tree into `output`.
    ///
    /// Steps:
    ///
    /// 1. Flush any pending STYLE work (it may dirty PAINT).
    /// 2. Drain GEOMETRY work so shape-only invalidations do not stay
    ///    live after the downstream paint work has refreshed.
    /// 3. For every PAINT-dirty element, call its behavior's `present`
    ///    so it resolves properties through the cascade and writes
    ///    primitives into [`PresentationStore`].
    /// 4. Drain the presentation store's dirty set (callers can hook
    ///    in here later for damage-driven re-lowering).
    /// 5. Rebuild each PAINT-dirty element's composition surface in
    ///    layer-local coordinates, lowering only the element's own
    ///    presentation primitives.
    /// 6. Traverse the Subduction layer tree, appending attached scene
    ///    surfaces into `output` through layer transforms and clips.
    /// 7. Clear COMPOSE.
    pub fn paint(&mut self, output: &mut Scene) {
        self.refresh_presentation_surfaces();
        self.composition.render_into(output);
    }

    /// Runs the present pass and rebuilds owned composition surface contents
    /// without composing the layer tree into an output scene.
    ///
    /// Hosts use this before synchronizing [`Self::composition_surface_updates`]
    /// into a backend-owned surface cache, then call
    /// [`Self::render_composition_with`] to produce the frame output exactly
    /// once. This clears `COMPOSE`, matching [`Self::paint`], because the layer
    /// state and owned surfaces are ready for immediate composition.
    pub fn refresh_presentation_surfaces(&mut self) {
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
        let geometry_dirty = self.invalidation.drain_sorted(GEOMETRY).count();
        self.frame_stats.geometry_elements += geometry_dirty;
        let dirty: Vec<ElementId> = self.invalidation.drain_sorted(PAINT).collect();
        self.frame_stats.painted_elements += dirty.len();
        for &id in &dirty {
            self.present_element(id);
        }
        // We don't yet drive incremental re-lowering off this, but
        // draining keeps the dirty set bounded.
        let dirty_presentation: Vec<NodeId> = self.presentation.drain_dirty();
        self.frame_stats.presentation_dirty_nodes += dirty_presentation.len();

        self.composition.clear_surface_updates();
        let mut rebuilt_surfaces = 0_usize;
        for id in dirty {
            let Some(surface) = self.composition.owned_scene_surface_for(id) else {
                continue;
            };
            let mut scene = self.composition.take_surface(surface);
            scene.clear();
            self.lower_element_surface(id, &mut scene);
            self.composition.restore_surface(surface, scene);
            rebuilt_surfaces += 1;
        }
        self.frame_stats.rebuilt_surfaces += rebuilt_surfaces;
        self.invalidation.clear(COMPOSE);
    }

    /// Composes the current layer tree into `output` using caller-owned
    /// surface scenes.
    ///
    /// Hosts use this after [`Self::paint`] or
    /// [`Self::refresh_presentation_surfaces`] has refreshed
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
        };
        if let Some(widget) = element.widget() {
            widget.present(element, &ctx, presentation, node);
        } else {
            // Anonymous container: still render its surface from the
            // cascade so panels with `Surface.Background` rules paint.
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
