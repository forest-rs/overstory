// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Lookup adapters that let Understory crates walk an Overstory [`crate::Ui`] tree.

use understory_property::{DependencyObject, ParentLookup, PropertyStore};
use understory_style::{ResolveParent, ResolveParentLookup};

use crate::element::ElementId;
use crate::ui::Ui;

/// Adapts a [`Ui`] reference to the
/// [`understory_property::ParentLookup`] trait so property-only helpers can
/// walk parents during inheritance.
#[derive(Copy, Clone, Debug)]
pub struct UiStoreLookup<'a> {
    ui: &'a Ui,
}

impl<'a> UiStoreLookup<'a> {
    /// Wraps `ui` for property-only parent walking.
    #[must_use]
    #[inline]
    pub fn new(ui: &'a Ui) -> Self {
        Self { ui }
    }
}

impl<'a> ParentLookup<'a, ElementId> for UiStoreLookup<'a> {
    #[inline]
    fn lookup(&self, key: ElementId) -> Option<(&'a PropertyStore<ElementId>, Option<ElementId>)> {
        self.ui
            .get(key)
            .map(|el| (el.property_store(), el.parent()))
    }
}

/// Style-aware parent lookup for [`understory_style::ResolveCx`].
///
/// This extends the plain property-store parent walk with each ancestor's
/// current style match state, allowing inherited properties to see ancestor
/// style values without Overstory reimplementing cascade precedence.
#[derive(Copy, Clone, Debug)]
pub struct UiResolveLookup<'a> {
    ui: &'a Ui,
}

impl<'a> UiResolveLookup<'a> {
    /// Wraps `ui` for use by [`understory_style::ResolveCx`].
    #[must_use]
    #[inline]
    pub fn new(ui: &'a Ui) -> Self {
        Self { ui }
    }
}

impl<'a> ResolveParentLookup<'a, ElementId> for UiResolveLookup<'a> {
    #[inline]
    fn lookup_resolve_parent(&self, key: ElementId) -> Option<ResolveParent<'a, ElementId>> {
        self.ui.get(key).map(|el| {
            ResolveParent::with_match_state(
                el.property_store(),
                el.parent(),
                self.ui.match_state(key),
            )
        })
    }
}
