// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `ParentLookup` implementation that walks an Overstory [`crate::Ui`] tree.

use understory_property::{DependencyObject, ParentLookup, PropertyStore};

use crate::element::ElementId;
use crate::ui::Ui;

/// Adapts a [`Ui`] reference to the
/// [`understory_property::ParentLookup`] trait so the property and style
/// crates can walk parents during inheritance and cascade resolution.
#[derive(Copy, Clone, Debug)]
pub struct UiStoreLookup<'a> {
    ui: &'a Ui,
}

impl<'a> UiStoreLookup<'a> {
    /// Wraps `ui` for use by [`understory_style::ResolveCx`] and similar.
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
