// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Lookup adapters that let Understory crates walk an Overstory [`crate::Ui`] tree.
//!
//! Roadmap:
//! - Keep parent-walking adapters thin so property inheritance and style-aware
//!   resolution remain Understory responsibilities.
//! - Add lookup variants only for real integration boundaries, such as
//!   accessibility, diagnostics, or future style queries.
//! - Avoid reimplementing cascade precedence in Overstory; if a resolver needs
//!   ancestor style state, pass it through an adapter like `UiResolveLookup`.

use understory_property::{DependencyObject, ParentLookup, PropertyStore};
use understory_style::{MatchState, ResolveParent, ResolveParentLookup};

use crate::element::{Element, ElementId};
use crate::element_store::ElementStore;
use crate::style_runtime::StyleRuntime;
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

    /// Returns a live element by id.
    #[must_use]
    #[inline]
    pub(crate) fn element(&self, id: ElementId) -> Option<&'a Element> {
        self.ui.get(id)
    }

    /// Returns the style match state for `id`.
    #[must_use]
    #[inline]
    pub(crate) fn match_state(&self, id: ElementId) -> MatchState {
        self.ui.match_state(id)
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

/// Style-aware parent lookup over split-borrowed UI fields.
///
/// Layout measurement cannot borrow the whole [`Ui`], but it can borrow the
/// element arena and style runtime separately. This adapter keeps measure-time
/// property resolution on the same [`understory_style::ResolveCx`] path as
/// presentation.
#[derive(Copy, Clone)]
pub(crate) struct ElementResolveLookup<'a> {
    elements: &'a ElementStore,
    style: &'a StyleRuntime,
}

impl<'a> ElementResolveLookup<'a> {
    /// Wraps split-borrowed element and style storage.
    #[must_use]
    #[inline]
    pub(crate) fn new(elements: &'a ElementStore, style: &'a StyleRuntime) -> Self {
        Self { elements, style }
    }

    /// Returns a live element by id.
    #[must_use]
    #[inline]
    pub(crate) fn element(&self, id: ElementId) -> Option<&'a Element> {
        self.elements.get(id)
    }

    /// Returns the style match state for `id`.
    #[must_use]
    #[inline]
    pub(crate) fn match_state(&self, id: ElementId) -> MatchState {
        self.style.match_state(id)
    }
}

impl<'a> ResolveParentLookup<'a, ElementId> for ElementResolveLookup<'a> {
    #[inline]
    fn lookup_resolve_parent(&self, key: ElementId) -> Option<ResolveParent<'a, ElementId>> {
        self.elements.get(key).map(|el| {
            ResolveParent::with_match_state(
                el.property_store(),
                el.parent(),
                self.style.match_state(key),
            )
        })
    }
}
