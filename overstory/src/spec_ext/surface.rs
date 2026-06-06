// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Surface and composition authoring extension traits.

use kurbo::Affine;
use peniko::Brush;
use understory_presentation::{CornerShape, CornerShapes, Corners, Edges};
use understory_presentation_properties::CornerRadius;

use crate::motion::TransitionRules;
use crate::spec_ext::ElementSpec;
use crate::widget::BuiltInPropertyKey;

/// Surface and composition builder methods available to element specs.
///
/// This trait covers properties that affect the visual surface of an element
/// rather than its identity, layout tree position, or semantic state.
pub trait SurfaceSpecExt: ElementSpec + Sized {
    /// Sets the initial background brush.
    ///
    /// Pass `None` to explicitly author no background as a local value.
    #[must_use]
    fn background(mut self, brush: impl Into<Option<Brush>>) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::SurfaceBackground, brush.into());
        self
    }

    /// Sets the initial foreground brush.
    #[must_use]
    fn foreground(mut self, brush: impl Into<Brush>) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::Foreground, brush.into());
        self
    }

    /// Sets the initial border brush.
    ///
    /// Pass `None` to explicitly author no border brush as a local value.
    #[must_use]
    fn border_brush(mut self, brush: impl Into<Option<Brush>>) -> Self {
        let brush = brush.into();
        let element = self.element_config_mut();
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderTopBrush, brush.clone());
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderRightBrush, brush.clone());
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderBottomBrush, brush.clone());
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderLeftBrush, brush);
        self
    }

    /// Sets initial physical border brushes in top/right/bottom/left order.
    ///
    /// Pass `None` for any side that should not draw.
    #[must_use]
    fn border_brushes(
        mut self,
        top: impl Into<Option<Brush>>,
        right: impl Into<Option<Brush>>,
        bottom: impl Into<Option<Brush>>,
        left: impl Into<Option<Brush>>,
    ) -> Self {
        let element = self.element_config_mut();
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderTopBrush, top.into());
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderRightBrush, right.into());
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderBottomBrush, bottom.into());
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderLeftBrush, left.into());
        self
    }

    /// Sets the initial border width in logical pixels.
    #[must_use]
    fn border_width(mut self, width: f64) -> Self {
        self = self.border_widths(Edges::all(width));
        self
    }

    /// Sets initial physical border widths in top/right/bottom/left order.
    #[must_use]
    fn border_widths(mut self, widths: Edges<f64>) -> Self {
        let element = self.element_config_mut();
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderTopWidth, widths.top);
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderRightWidth, widths.right);
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderBottomWidth, widths.bottom);
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderLeftWidth, widths.left);
        self
    }

    /// Sets the initial corner radius in logical pixels.
    #[must_use]
    fn corner_radius(mut self, radius: f64) -> Self {
        self = self.corner_radii(Corners::all(CornerRadius::circular(radius)));
        self
    }

    /// Sets initial property-level corner radii in top-left, top-right,
    /// bottom-right, bottom-left order.
    #[must_use]
    fn corner_radii(mut self, radii: Corners<CornerRadius>) -> Self {
        let element = self.element_config_mut();
        element.set_built_in_local(
            BuiltInPropertyKey::SurfaceCornerTopLeftRadius,
            radii.top_left,
        );
        element.set_built_in_local(
            BuiltInPropertyKey::SurfaceCornerTopRightRadius,
            radii.top_right,
        );
        element.set_built_in_local(
            BuiltInPropertyKey::SurfaceCornerBottomRightRadius,
            radii.bottom_right,
        );
        element.set_built_in_local(
            BuiltInPropertyKey::SurfaceCornerBottomLeftRadius,
            radii.bottom_left,
        );
        self
    }

    /// Sets one initial corner shape for every corner.
    #[must_use]
    fn corner_shape(mut self, shape: CornerShape) -> Self {
        self = self.corner_shapes(CornerShapes::all(shape));
        self
    }

    /// Sets initial corner shapes in top-left, top-right, bottom-right,
    /// bottom-left order.
    #[must_use]
    fn corner_shapes(mut self, shapes: CornerShapes) -> Self {
        let element = self.element_config_mut();
        element.set_built_in_local(
            BuiltInPropertyKey::SurfaceCornerTopLeftShape,
            shapes.top_left,
        );
        element.set_built_in_local(
            BuiltInPropertyKey::SurfaceCornerTopRightShape,
            shapes.top_right,
        );
        element.set_built_in_local(
            BuiltInPropertyKey::SurfaceCornerBottomRightShape,
            shapes.bottom_right,
        );
        element.set_built_in_local(
            BuiltInPropertyKey::SurfaceCornerBottomLeftShape,
            shapes.bottom_left,
        );
        self
    }

    /// Sets uniform initial box padding.
    ///
    /// Box padding is the canonical content inset for both layout and surface
    /// decoration geometry.
    #[must_use]
    fn padding_all(self, padding: f64) -> Self {
        self.padding_widths(Edges::all(padding))
    }

    /// Sets horizontal and vertical initial box padding.
    ///
    /// `horizontal` applies to left/right; `vertical` applies to top/bottom.
    #[must_use]
    fn padding_xy(self, horizontal: f64, vertical: f64) -> Self {
        self.padding_widths(Edges::vertical_horizontal(vertical, horizontal))
    }

    /// Sets initial physical box padding widths in top/right/bottom/left
    /// order.
    ///
    /// The same values feed Taffy content insets and
    /// `SurfacePrimitive::padding_widths`, so content placement and decoration
    /// contours agree.
    #[must_use]
    fn padding_widths(mut self, padding: Edges<f64>) -> Self {
        let element = self.element_config_mut();
        element.set_built_in_local(BuiltInPropertyKey::SurfacePaddingTop, padding.top);
        element.set_built_in_local(BuiltInPropertyKey::SurfacePaddingRight, padding.right);
        element.set_built_in_local(BuiltInPropertyKey::SurfacePaddingBottom, padding.bottom);
        element.set_built_in_local(BuiltInPropertyKey::SurfacePaddingLeft, padding.left);
        self
    }

    /// Sets the initial layer opacity.
    #[must_use]
    fn opacity(mut self, opacity: f32) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::Opacity, opacity);
        self
    }

    /// Sets the initial visual transform applied after layout.
    #[must_use]
    fn visual_transform(mut self, transform: Affine) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::VisualTransform, transform);
        self
    }

    /// Sets initial style-authored transition rules.
    #[must_use]
    fn transitions(mut self, transitions: TransitionRules) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::Transitions, transitions);
        self
    }
}

impl<T> SurfaceSpecExt for T where T: ElementSpec {}
