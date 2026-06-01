// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Numeric range authoring extension traits.

use crate::spec_ext::ElementSpec;
use crate::widget::BuiltInPropertyKey;

/// Numeric range builder methods available to element specs.
///
/// Range controls such as sliders and progress bars usually expose
/// control-specific inherent methods as well. This trait supplies the shared
/// dependency-property language for generic authoring code.
pub trait RangeSpecExt: ElementSpec + Sized {
    /// Sets the initial numeric value property.
    #[must_use]
    fn value(mut self, value: f64) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::Value, value);
        self
    }

    /// Sets the initial minimum numeric value.
    #[must_use]
    fn minimum(mut self, minimum: f64) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::Minimum, minimum);
        self
    }

    /// Sets the initial maximum numeric value.
    #[must_use]
    fn maximum(mut self, maximum: f64) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::Maximum, maximum);
        self
    }

    /// Sets the initial numeric value range.
    #[must_use]
    fn range(mut self, minimum: f64, maximum: f64) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::Minimum, minimum);
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::Maximum, maximum);
        self
    }

    /// Sets the initial small change value for range controls.
    #[must_use]
    fn small_change(mut self, small_change: f64) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::SmallChange, small_change);
        self
    }

    /// Sets the initial large change value for range controls.
    #[must_use]
    fn large_change(mut self, large_change: f64) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::LargeChange, large_change);
        self
    }
}

impl<T> RangeSpecExt for T where T: ElementSpec {}
