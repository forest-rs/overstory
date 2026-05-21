// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared surface-presentation helper.
//!
//! Most widgets' visual contribution is a single rounded surface
//! with optional border, sized by the element's bounds. The
//! [`present_surface`] helper resolves `Background` /
//! `BorderBrush` / `BorderWidth` / `CornerRadius` through the
//! cascade and writes the result as a `SurfacePrimitive` on the
//! element's presentation node. The default `Widget::present`
//! implementation delegates here.

use peniko::Brush;
use understory_box_tree::NodeId;
use understory_presentation::{BackgroundLayer, Border, PresentationStore, RoundedRectRadii};

use crate::behavior::PresentContext;
use crate::element::{Element, ElementId};

/// Reads `Background` / `CornerRadius` / `BorderBrush` /
/// `BorderWidth` through the cascade and writes the result as a
/// `SurfacePrimitive` on `node`. Widgets whose visual is "a
/// rounded box that fills my bounds" call this from their
/// `Widget::present` (or rely on the trait's default impl).
pub fn present_surface(
    element: &Element,
    ctx: &PresentContext<'_>,
    store: &mut PresentationStore<NodeId, ElementId>,
    node: NodeId,
) {
    let background: Option<Brush> = ctx.get_value(element, ctx.properties.background);
    let corner_radius: f64 = ctx.get_value(element, ctx.properties.corner_radius);
    let border_brush: Option<Brush> = ctx.get_value(element, ctx.properties.border_brush);
    let border_width: f64 = ctx.get_value(element, ctx.properties.border_width);

    let Some(surface) = store.surface_mut(node) else {
        return;
    };
    surface.backgrounds.clear();
    if let Some(brush) = background {
        surface.backgrounds.push(BackgroundLayer::new(brush));
    }
    surface.corner_radii = RoundedRectRadii::from_single_radius(corner_radius);
    surface.border = match border_brush {
        Some(brush) if border_width > 0.0 => Border::uniform(brush, border_width),
        _ => Border::default(),
    };
}
