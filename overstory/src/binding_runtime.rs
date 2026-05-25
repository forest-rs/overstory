// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained property-binding graph for one UI tree.
//!
//! Roadmap:
//! - Keep bindings type-safe and property-oriented; templates and controls
//!   should not need an expression language to synchronize simple state.
//! - Make template bindings, app bindings, and future two-way bindings share
//!   one graph and one diagnostic story.
//! - Add cycle, stale endpoint, and binding-cost diagnostics before bindings
//!   become hard to debug in larger examples.
//! - Keep binding evaluation inside Overstory while leaving the dependency
//!   property substrate in `understory_property_binding`.

use understory_property_binding::{
    BindingError, BindingId, BindingSet, EndpointKey, PropertyEndpoint,
};

use crate::channels::BINDING;
use crate::element::ElementId;

/// One-way property bindings and their internal invalidation tracker.
pub(crate) struct BindingRuntime {
    /// Binding graph for retained elements.
    ///
    /// Temporarily `None` while `Ui::flush_bindings` owns the set and evaluates
    /// updates against `&mut Ui`.
    bindings: Option<BindingSet<ElementId>>,
}

impl BindingRuntime {
    pub(crate) fn new() -> Self {
        Self {
            bindings: Some(BindingSet::new(BINDING)),
        }
    }

    pub(crate) fn bind<T>(
        &mut self,
        source: PropertyEndpoint<ElementId, T>,
        target: PropertyEndpoint<ElementId, T>,
    ) -> Result<BindingId, BindingError<ElementId>>
    where
        T: Clone + 'static,
    {
        let bindings = self
            .bindings
            .as_mut()
            .expect("bindings always Some outside flush_bindings");
        let id = bindings.bind(source, target)?;
        bindings.mark_binding_dirty(id);
        Ok(id)
    }

    /// Marks bindings that depend on `endpoint` dirty.
    ///
    /// No-ops while bindings are already being flushed; the active flush owns
    /// the binding set and records changes directly.
    pub(crate) fn mark_endpoint_changed(&mut self, endpoint: EndpointKey<ElementId>) {
        if let Some(bindings) = self.bindings.as_mut() {
            bindings.mark_endpoint_changed(endpoint);
        }
    }

    pub(crate) fn clear_owner(&mut self, owner: ElementId) {
        if let Some(bindings) = self.bindings.as_mut() {
            bindings.clear_owner(owner);
        }
    }

    /// Takes the binding graph for a flush pass.
    pub(crate) fn take(&mut self) -> BindingSet<ElementId> {
        self.bindings
            .take()
            .expect("bindings always Some outside flush_bindings")
    }

    /// Restores the binding graph after a flush pass.
    pub(crate) fn restore(&mut self, bindings: BindingSet<ElementId>) {
        self.bindings = Some(bindings);
    }
}
