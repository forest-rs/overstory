// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Style-builder helpers for Overstory's canonical surface properties.

use peniko::Brush;
use understory_presentation::{CornerShape, CornerShapes, Corners, Edges};
use understory_presentation_properties::CornerRadius;
use understory_style::{ResourceKey, StyleBuilder};

use crate::properties::BuiltInProperties;

/// Surface property helpers for [`StyleBuilder`].
///
/// Overstory stores surface decoration as longhand properties through
/// `understory_presentation_properties`. These helpers keep common uniform
/// styles readable while still writing the canonical per-side and per-corner
/// property handles.
pub trait SurfaceStyleExt: Sized {
    /// Sets the first background layer brush.
    #[must_use]
    fn background(self, properties: &BuiltInProperties, brush: impl Into<Option<Brush>>) -> Self;

    /// Sets the first background layer from a theme resource.
    #[must_use]
    fn background_resource(self, properties: &BuiltInProperties, key: ResourceKey) -> Self;

    /// Sets one border brush for all four sides.
    #[must_use]
    fn border_brush(self, properties: &BuiltInProperties, brush: impl Into<Option<Brush>>) -> Self;

    /// Sets one border brush resource for all four sides.
    #[must_use]
    fn border_brush_resource(self, properties: &BuiltInProperties, key: ResourceKey) -> Self;

    /// Sets one border width for all four sides.
    #[must_use]
    fn border_width(self, properties: &BuiltInProperties, width: f64) -> Self;

    /// Sets physical border widths in top/right/bottom/left order.
    #[must_use]
    fn border_widths(self, properties: &BuiltInProperties, widths: Edges<f64>) -> Self;

    /// Sets one circular corner radius for every corner.
    #[must_use]
    fn corner_radius(self, properties: &BuiltInProperties, radius: f64) -> Self;

    /// Sets property-level corner radii in top-left, top-right,
    /// bottom-right, bottom-left order.
    #[must_use]
    fn corner_radii(self, properties: &BuiltInProperties, radii: Corners<CornerRadius>) -> Self;

    /// Sets one corner shape for every corner.
    #[must_use]
    fn corner_shape(self, properties: &BuiltInProperties, shape: CornerShape) -> Self;

    /// Sets physical corner shapes in top-left, top-right, bottom-right,
    /// bottom-left order.
    #[must_use]
    fn corner_shapes(self, properties: &BuiltInProperties, shapes: CornerShapes) -> Self;

    /// Sets uniform box padding.
    ///
    /// Box padding is the canonical content inset for both layout and surface
    /// decoration geometry.
    #[must_use]
    fn padding_all(self, properties: &BuiltInProperties, padding: f64) -> Self;

    /// Sets horizontal and vertical box padding.
    ///
    /// `horizontal` applies to left/right; `vertical` applies to top/bottom.
    #[must_use]
    fn padding_xy(self, properties: &BuiltInProperties, horizontal: f64, vertical: f64) -> Self;

    /// Sets physical box padding widths in top/right/bottom/left order.
    ///
    /// The same values feed Taffy content insets and
    /// `SurfacePrimitive::padding_widths`, so content placement and decoration
    /// contours agree.
    #[must_use]
    fn padding_widths(self, properties: &BuiltInProperties, padding: Edges<f64>) -> Self;
}

impl SurfaceStyleExt for StyleBuilder {
    fn background(self, properties: &BuiltInProperties, brush: impl Into<Option<Brush>>) -> Self {
        self.set(properties.surface.background, brush.into())
    }

    fn background_resource(self, properties: &BuiltInProperties, key: ResourceKey) -> Self {
        self.set_resource(properties.surface.background, key)
    }

    fn border_brush(self, properties: &BuiltInProperties, brush: impl Into<Option<Brush>>) -> Self {
        let brush = brush.into();
        self.set(properties.surface.border_brushes.top, brush.clone())
            .set(properties.surface.border_brushes.right, brush.clone())
            .set(properties.surface.border_brushes.bottom, brush.clone())
            .set(properties.surface.border_brushes.left, brush)
    }

    fn border_brush_resource(self, properties: &BuiltInProperties, key: ResourceKey) -> Self {
        self.set_resource(properties.surface.border_brushes.top, key)
            .set_resource(properties.surface.border_brushes.right, key)
            .set_resource(properties.surface.border_brushes.bottom, key)
            .set_resource(properties.surface.border_brushes.left, key)
    }

    fn border_width(self, properties: &BuiltInProperties, width: f64) -> Self {
        self.border_widths(properties, Edges::all(width))
    }

    fn border_widths(self, properties: &BuiltInProperties, widths: Edges<f64>) -> Self {
        self.set(properties.surface.border_widths.top, widths.top)
            .set(properties.surface.border_widths.right, widths.right)
            .set(properties.surface.border_widths.bottom, widths.bottom)
            .set(properties.surface.border_widths.left, widths.left)
    }

    fn corner_radius(self, properties: &BuiltInProperties, radius: f64) -> Self {
        self.corner_radii(properties, Corners::all(CornerRadius::circular(radius)))
    }

    fn corner_radii(self, properties: &BuiltInProperties, radii: Corners<CornerRadius>) -> Self {
        self.set(properties.surface.corner_radii.top_left, radii.top_left)
            .set(properties.surface.corner_radii.top_right, radii.top_right)
            .set(
                properties.surface.corner_radii.bottom_right,
                radii.bottom_right,
            )
            .set(
                properties.surface.corner_radii.bottom_left,
                radii.bottom_left,
            )
    }

    fn corner_shape(self, properties: &BuiltInProperties, shape: CornerShape) -> Self {
        self.corner_shapes(properties, CornerShapes::all(shape))
    }

    fn corner_shapes(self, properties: &BuiltInProperties, shapes: CornerShapes) -> Self {
        self.set(properties.surface.corner_shapes.top_left, shapes.top_left)
            .set(properties.surface.corner_shapes.top_right, shapes.top_right)
            .set(
                properties.surface.corner_shapes.bottom_right,
                shapes.bottom_right,
            )
            .set(
                properties.surface.corner_shapes.bottom_left,
                shapes.bottom_left,
            )
    }

    fn padding_all(self, properties: &BuiltInProperties, padding: f64) -> Self {
        self.padding_widths(properties, Edges::all(padding))
    }

    fn padding_xy(self, properties: &BuiltInProperties, horizontal: f64, vertical: f64) -> Self {
        self.padding_widths(properties, Edges::vertical_horizontal(vertical, horizontal))
    }

    fn padding_widths(self, properties: &BuiltInProperties, padding: Edges<f64>) -> Self {
        self.set(properties.surface.padding_widths.top, padding.top)
            .set(properties.surface.padding_widths.right, padding.right)
            .set(properties.surface.padding_widths.bottom, padding.bottom)
            .set(properties.surface.padding_widths.left, padding.left)
    }
}
