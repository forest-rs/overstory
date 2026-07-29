// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Surface and composition authoring extension traits.

use kurbo::Affine;
use peniko::Brush;
use understory_presentation::{BorderStyle, CornerShape, CornerShapes, Corners, Edges};
use understory_presentation_properties::CornerRadius;

use crate::motion::{EffectRules, Transitions};
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
    /// If no border style has already been authored, this also defaults the
    /// border style to [`BorderStyle::Solid`] for a brush or
    /// [`BorderStyle::None`] for no brush.
    #[must_use]
    fn border_brush(mut self, brush: impl Into<Option<Brush>>) -> Self {
        let brush = brush.into();
        let style = if brush.is_some() {
            BorderStyle::Solid
        } else {
            BorderStyle::None
        };
        let element = self.element_config_mut();
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderTopBrush, brush.clone());
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderRightBrush, brush.clone());
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderBottomBrush, brush.clone());
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderLeftBrush, brush);
        element.set_built_in_local_if_absent(BuiltInPropertyKey::SurfaceBorderTopStyle, style);
        element.set_built_in_local_if_absent(BuiltInPropertyKey::SurfaceBorderRightStyle, style);
        element.set_built_in_local_if_absent(BuiltInPropertyKey::SurfaceBorderBottomStyle, style);
        element.set_built_in_local_if_absent(BuiltInPropertyKey::SurfaceBorderLeftStyle, style);
        self
    }

    /// Sets initial physical border brushes in top/right/bottom/left order.
    ///
    /// Pass `None` for any side that should not draw.
    /// Sides without an authored border style default to
    /// [`BorderStyle::Solid`] when a brush is present, or
    /// [`BorderStyle::None`] when it is absent.
    #[must_use]
    fn border_brushes(
        mut self,
        top: impl Into<Option<Brush>>,
        right: impl Into<Option<Brush>>,
        bottom: impl Into<Option<Brush>>,
        left: impl Into<Option<Brush>>,
    ) -> Self {
        let top = top.into();
        let right = right.into();
        let bottom = bottom.into();
        let left = left.into();
        let element = self.element_config_mut();
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderTopBrush, top.clone());
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderRightBrush, right.clone());
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderBottomBrush, bottom.clone());
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderLeftBrush, left.clone());
        element.set_built_in_local_if_absent(
            BuiltInPropertyKey::SurfaceBorderTopStyle,
            border_style_for_brush(&top),
        );
        element.set_built_in_local_if_absent(
            BuiltInPropertyKey::SurfaceBorderRightStyle,
            border_style_for_brush(&right),
        );
        element.set_built_in_local_if_absent(
            BuiltInPropertyKey::SurfaceBorderBottomStyle,
            border_style_for_brush(&bottom),
        );
        element.set_built_in_local_if_absent(
            BuiltInPropertyKey::SurfaceBorderLeftStyle,
            border_style_for_brush(&left),
        );
        self
    }

    /// Sets the initial border style for every side.
    #[must_use]
    fn border_style(mut self, style: BorderStyle) -> Self {
        self = self.border_styles(Edges::all(style));
        self
    }

    /// Sets initial physical border styles in top/right/bottom/left order.
    #[must_use]
    fn border_styles(mut self, styles: Edges<BorderStyle>) -> Self {
        let element = self.element_config_mut();
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderTopStyle, styles.top);
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderRightStyle, styles.right);
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderBottomStyle, styles.bottom);
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderLeftStyle, styles.left);
        self
    }

    /// Sets the initial border width in logical pixels.
    ///
    /// If no border style has already been authored, this also defaults each
    /// side to [`BorderStyle::Solid`] so border width remains layout-affecting.
    #[must_use]
    fn border_width(mut self, width: f64) -> Self {
        self = self.border_widths(Edges::all(width));
        self
    }

    /// Sets initial physical border widths in top/right/bottom/left order.
    ///
    /// If no border style has already been authored, this also defaults each
    /// side to [`BorderStyle::Solid`] so border width remains layout-affecting.
    #[must_use]
    fn border_widths(mut self, widths: Edges<f64>) -> Self {
        let element = self.element_config_mut();
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderTopWidth, widths.top);
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderRightWidth, widths.right);
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderBottomWidth, widths.bottom);
        element.set_built_in_local(BuiltInPropertyKey::SurfaceBorderLeftWidth, widths.left);
        element.set_built_in_local_if_absent(
            BuiltInPropertyKey::SurfaceBorderTopStyle,
            BorderStyle::Solid,
        );
        element.set_built_in_local_if_absent(
            BuiltInPropertyKey::SurfaceBorderRightStyle,
            BorderStyle::Solid,
        );
        element.set_built_in_local_if_absent(
            BuiltInPropertyKey::SurfaceBorderBottomStyle,
            BorderStyle::Solid,
        );
        element.set_built_in_local_if_absent(
            BuiltInPropertyKey::SurfaceBorderLeftStyle,
            BorderStyle::Solid,
        );
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
    fn transitions(mut self, transitions: Transitions) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::Transitions, transitions);
        self
    }

    /// Sets initial style-authored effect rules.
    #[must_use]
    fn effects(mut self, effects: EffectRules) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::Effects, effects);
        self
    }
}

impl<T> SurfaceSpecExt for T where T: ElementSpec {}

fn border_style_for_brush(brush: &Option<Brush>) -> BorderStyle {
    if brush.is_some() {
        BorderStyle::Solid
    } else {
        BorderStyle::None
    }
}
