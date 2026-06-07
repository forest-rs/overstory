// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Core element authoring extension traits.

use understory_property::Property;

use crate::layouts::Layout;
use crate::style_class::StyleClass;
use crate::ui::ControlState;
use crate::widget::{AppendSpec, ElementConfig};

/// Append specs that expose the shared element authoring surface.
///
/// Implement this for builders that want the common extension methods from
/// [`ElementSpecExt`]. The trait is deliberately about construction-time
/// element settings, not widget behavior.
pub trait ElementSpec: AppendSpec {
    /// Returns mutable access to the spec's shared element config.
    fn element_config_mut(&mut self) -> &mut ElementConfig;
}

/// Core builder methods available to element specs.
///
/// Import this trait, or use `overstory::prelude::*`, to get a uniform
/// `.class(...)`, `.layout(...)`, `.enabled(...)`, `.disabled()`,
/// `.focusable(...)`, and low-level `.local(...)` surface on specs that
/// implement [`ElementSpec`].
pub trait ElementSpecExt: ElementSpec + Sized {
    /// Writes an initial local dependency-property value when the element is
    /// appended.
    ///
    /// Use this for custom properties or for built-ins that do not yet have a
    /// dedicated helper. Dedicated helpers such as
    /// [`SurfaceSpecExt::background`](super::SurfaceSpecExt::background) and
    /// [`TextSpecExt::font_features`](super::TextSpecExt::font_features) keep
    /// common call-sites independent from a particular [`Ui`](crate::Ui)'s
    /// registered property handles.
    #[must_use]
    fn local<T>(mut self, property: Property<T>, value: T) -> Self
    where
        T: Clone + 'static,
    {
        self.element_config_mut().set_local(property, value);
        self
    }

    /// Adds a style class to the resulting element.
    ///
    /// Pass an existing `ClassId` when one has already been resolved, or pass a
    /// class name such as `.primary` to have the owning `Ui` vocabulary resolve
    /// it when the spec is appended.
    #[must_use]
    fn class(mut self, class: impl Into<StyleClass>) -> Self {
        self.element_config_mut().add_class(class);
        self
    }

    /// Adds several style classes to the resulting element.
    #[must_use]
    fn classes<C>(mut self, classes: impl IntoIterator<Item = C>) -> Self
    where
        C: Into<StyleClass>,
    {
        for class in classes {
            self.element_config_mut().add_class(class);
        }
        self
    }

    /// Sets layout longhand contributions on the resulting element.
    ///
    /// Only the longhands explicitly carried by `layout` are written; this is
    /// the same partial-update behavior as [`Ui::set_layout`](crate::Ui::set_layout).
    #[must_use]
    fn layout(mut self, layout: impl Into<Layout>) -> Self {
        self.element_config_mut().set_layout(layout);
        self
    }

    /// Sets the resulting element's shared interactive control state.
    #[must_use]
    fn control_state(mut self, state: ControlState) -> Self {
        self.element_config_mut().set_control_state(state);
        self
    }

    /// Sets whether the resulting element starts enabled.
    #[must_use]
    fn enabled(mut self, enabled: bool) -> Self {
        self.element_config_mut()
            .update_control_state(|state| state.enabled(enabled));
        self
    }

    /// Convenience for [`Self::enabled`] with `false`.
    #[must_use]
    fn disabled(self) -> Self {
        self.enabled(false)
    }

    /// Sets whether the resulting element participates in focus traversal.
    #[must_use]
    fn focusable(mut self, focusable: bool) -> Self {
        self.element_config_mut()
            .update_control_state(|state| state.focusable(focusable));
        self
    }
}

impl<T> ElementSpecExt for T where T: ElementSpec {}
