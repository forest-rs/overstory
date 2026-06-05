// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Control-template authoring extension traits.

use crate::template::Template;
use crate::widget::AppendSpec;

/// Append specs that can replace a control's visual template.
///
/// This is an opt-in authoring capability for lookless controls. The control
/// still owns semantic state, routed input, accessibility role, and property
/// synchronization; the replacement template owns only the generated visual
/// tree.
pub trait ControlTemplateSpec: AppendSpec {
    /// Returns the pending construction-time template override.
    fn template_override_mut(&mut self) -> &mut Option<&'static dyn Template>;
}

/// Builder method shared by specs that support visual-template replacement.
///
/// Import this trait, or use `overstory::prelude::*`, to call
/// `.template(&MY_TEMPLATE)` on controls that implement [`ControlTemplateSpec`].
pub trait ControlTemplateSpecExt: ControlTemplateSpec + Sized {
    /// Replaces the control's default visual template for the element created
    /// by this spec.
    #[must_use]
    fn template(mut self, template: &'static dyn Template) -> Self {
        *self.template_override_mut() = Some(template);
        self
    }
}

impl<T> ControlTemplateSpecExt for T where T: ControlTemplateSpec {}
