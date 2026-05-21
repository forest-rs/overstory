// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Behavior implementation for [`crate::ELEMENT_KIND_GENERIC`].
//!
//! Resolves `Background` and `CornerRadius` through the cascade and writes
//! the result as a `SurfacePrimitive` on the element's presentation node.
//! The lowerer in [`crate::Ui::paint`] turns the primitive plus the box
//! tree's world bounds into an actual fill draw.

use peniko::Brush;
use understory_box_tree::NodeId;
use understory_presentation::{BackgroundLayer, PresentationStore, RoundedRectRadii};

use crate::behavior::{ElementBehavior, PresentContext};
use crate::element::{Element, ElementId};

/// The generic kind's behavior. Stateless; instantiated as a `'static`.
#[derive(Debug, Default)]
pub struct GenericBehavior;

/// `'static` instance registered in [`crate::Ui::new`].
pub(crate) static GENERIC_BEHAVIOR: GenericBehavior = GenericBehavior;

impl ElementBehavior for GenericBehavior {
    fn present(
        &self,
        element: &Element,
        ctx: &PresentContext<'_>,
        store: &mut PresentationStore<NodeId, ElementId>,
        node: NodeId,
    ) {
        present_surface(element, ctx, store, node);
    }
}

/// Shared surface-presenter used by the generic kind and any other
/// behavior whose "surface" intent is the same — fill the box's bounds
/// with `Background`, rounded by `CornerRadius`. Re-resolves on every
/// call (no caching beyond what the presentation store already does).
pub(crate) fn present_surface(
    element: &Element,
    ctx: &PresentContext<'_>,
    store: &mut PresentationStore<NodeId, ElementId>,
    node: NodeId,
) {
    let cx = ctx.resolve_cx();
    let style = Some((ctx.cascade, ctx.match_state));
    let background: Option<Brush> = cx.get_value(element, ctx.properties.background, style);
    let corner_radius: f64 = cx.get_value(element, ctx.properties.corner_radius, style);

    let Some(surface) = store.surface_mut(node) else {
        return;
    };
    surface.backgrounds.clear();
    if let Some(brush) = background {
        surface.backgrounds.push(BackgroundLayer::new(brush));
    }
    surface.corner_radii = RoundedRectRadii::from_single_radius(corner_radius);
}
