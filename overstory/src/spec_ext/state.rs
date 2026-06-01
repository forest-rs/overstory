// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Semantic state authoring extension traits.

use crate::spec_ext::ElementSpec;
use crate::widget::BuiltInPropertyKey;

/// Semantic state builder methods available to element specs.
///
/// Controls usually provide domain-specific inherent methods for these values.
/// This shared trait is useful for composed specs and generic authoring code.
pub trait ControlStateSpecExt: ElementSpec + Sized {
    /// Sets the initial checked state property.
    #[must_use]
    fn checked(mut self, checked: bool) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::IsChecked, checked);
        self
    }

    /// Sets the initial expanded state property.
    #[must_use]
    fn expanded(mut self, expanded: bool) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::IsExpanded, expanded);
        self
    }

    /// Sets whether an editable text-like control starts read-only.
    #[must_use]
    fn read_only(mut self, read_only: bool) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::IsReadOnly, read_only);
        self
    }

    /// Sets the initial selected index property.
    #[must_use]
    fn selected_index(mut self, index: impl Into<Option<usize>>) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::SelectedIndex, index.into());
        self
    }
}

impl<T> ControlStateSpecExt for T where T: ElementSpec {}
