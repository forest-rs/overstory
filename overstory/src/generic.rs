// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared surface-presentation helper.
//!
//! Most widgets' visual contribution is a single resolved surface, sized by
//! the element's bounds. The [`present_surface`] helper resolves Overstory's
//! canonical surface-property bundle through the cascade and writes the result
//! as a `SurfacePrimitive` on the element's presentation node. The default
//! `Widget::present` implementation delegates here.
//!
//! Roadmap:
//! - Keep the default surface path boring and shared: most box-like widgets
//!   should not need custom presentation code.
//! - Expand surface primitives through `understory_presentation` when common
//!   visual needs emerge, such as shadows, layered backgrounds, outlines, or
//!   focus rings.
//! - Keep theme/state styling resolved before this helper writes presentation
//!   so surface painting does not learn about selectors.

use understory_box_tree::NodeId;
use understory_presentation::PresentationStore;
use understory_presentation_properties::StyleMatch;
use understory_style::ResolveCx;

use crate::behavior::PresentContext;
use crate::element::{Element, ElementId};

/// Reads surface longhands through the cascade and writes the result as a
/// `SurfacePrimitive` on `node`. Widgets whose visual is "a box that fills my
/// bounds" call this from their `Widget::present` (or rely on the trait's
/// default impl).
pub fn present_surface(
    element: &Element,
    ctx: &PresentContext<'_>,
    store: &mut PresentationStore<NodeId, ElementId>,
    node: NodeId,
) {
    let Some(surface) = store.surface_mut(node) else {
        return;
    };
    *surface = ctx.properties.surface.resolve_surface(
        &ResolveCx::new(ctx.registry, ctx.theme, ctx.lookup),
        element,
        Some(StyleMatch::new(
            ctx.cascade,
            ctx.lookup.match_state(element.id()),
        )),
    );
}
