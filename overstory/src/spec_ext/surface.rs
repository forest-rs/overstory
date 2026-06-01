// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Surface and composition authoring extension traits.

use kurbo::Affine;
use peniko::Brush;

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
            .set_built_in_local(BuiltInPropertyKey::Background, brush.into());
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
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::BorderBrush, brush.into());
        self
    }

    /// Sets the initial border width in logical pixels.
    #[must_use]
    fn border_width(mut self, width: f64) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::BorderWidth, width);
        self
    }

    /// Sets the initial corner radius in logical pixels.
    #[must_use]
    fn corner_radius(mut self, radius: f64) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::CornerRadius, radius);
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
