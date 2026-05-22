// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use understory_property_binding::{
    BindingError, BindingId, BindingSet, EndpointKey, PropertyEndpoint,
};

use crate::channels::BINDING;
use crate::element::ElementId;

/// One-way property bindings and their internal invalidation tracker.
pub(crate) struct BindingRuntime {
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

    pub(crate) fn take(&mut self) -> BindingSet<ElementId> {
        self.bindings
            .take()
            .expect("bindings always Some outside flush_bindings")
    }

    pub(crate) fn restore(&mut self, bindings: BindingSet<ElementId>) {
        self.bindings = Some(bindings);
    }
}
