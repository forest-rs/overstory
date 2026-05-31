// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Property writes and dependency-property binding integration.

use understory_property::{
    DependencyObject, DependencyObjectExt, ErasedValue, LocalValueSource, Property, PropertyId,
};
use understory_property_binding::{
    BindingDrainError, BindingError, BindingHost, BindingId, BindingReport, BindingWrite,
    EndpointKey, PropertyEndpoint,
};

use crate::element::ElementId;

use super::Ui;

impl Ui {
    /// Writes a property value with notification.
    ///
    /// Marks affected channels in the invalidation tracker and notifies the
    /// binding set so any binding whose source is this endpoint will fire
    /// on the next `flush_bindings`.
    pub fn set_local<T: Clone + PartialEq + 'static>(
        &mut self,
        id: ElementId,
        property: Property<T>,
        value: T,
    ) {
        let channels = {
            let registry = &self.registry;
            let Some(element) = self.elements.get_mut(id) else {
                return;
            };
            element.set_local_notifying(property, value, registry)
        };
        self.mark_channels(id, channels);
        self.bindings
            .mark_endpoint_changed(EndpointKey::new(id, property.id()));
    }

    pub(crate) fn set_local_erased(
        &mut self,
        id: ElementId,
        property: PropertyId,
        value: ErasedValue,
    ) {
        let channels = {
            let registry = &self.registry;
            let Some(element) = self.elements.get_mut(id) else {
                return;
            };
            match element
                .property_store_mut()
                .set_local_erased_with_source_notifying(
                    property,
                    value,
                    LocalValueSource::Local,
                    registry,
                ) {
                Ok(channels) => channels,
                Err(_) => return,
            }
        };
        self.mark_channels(id, channels);
        self.bindings
            .mark_endpoint_changed(EndpointKey::new(id, property));
    }

    /// Registers a one-way binding from `source` to `target`. Both
    /// endpoints' value types must match. Returns the new [`BindingId`]
    /// or a [`BindingError`] if the registration would self-loop or
    /// introduce a cycle.
    ///
    /// The binding will fire on the next `flush_bindings` if its source
    /// endpoint has been written since the last drain (or has not yet
    /// drained at all). Bindings are typically registered during template
    /// expansion via [`Template::instantiate`](crate::Template::instantiate).
    pub fn bind<T>(
        &mut self,
        source: PropertyEndpoint<ElementId, T>,
        target: PropertyEndpoint<ElementId, T>,
    ) -> Result<BindingId, BindingError<ElementId>>
    where
        T: Clone + 'static,
    {
        self.bindings.bind(source, target)
    }

    /// Drains the binding set, propagating dirty bindings through the host.
    /// Channels affected by binding writes are unioned into the invalidation
    /// tracker as a side effect of the host's `set_erased` impl.
    pub fn try_flush_bindings(&mut self) -> Result<BindingReport, BindingDrainError<ElementId>> {
        let mut bindings = self.bindings.take();
        let result = bindings.drain(self);
        self.bindings.restore(bindings);
        result
    }

    /// Drains dirty bindings and panics if a real binding evaluation error is
    /// found. Use [`Self::try_flush_bindings`] when binding graph or host type
    /// errors should be handled by the caller.
    pub fn flush_bindings(&mut self) -> BindingReport {
        self.try_flush_bindings()
            .expect("binding drain failed; use try_flush_bindings to inspect the error")
    }
}

impl BindingHost<ElementId> for Ui {
    fn get_erased(&self, endpoint: EndpointKey<ElementId>) -> Option<ErasedValue> {
        let element = self.get(endpoint.owner())?;
        element
            .property_store()
            .effective_local_erased(endpoint.property())
            .cloned()
    }

    fn set_erased(&mut self, endpoint: EndpointKey<ElementId>, value: ErasedValue) -> BindingWrite {
        let id = endpoint.owner();
        let prop_id = endpoint.property();
        let channels = {
            // Disjoint-field split-borrow: registry is read-only while the
            // element store mutates only the target element.
            let registry = &self.registry;
            let Some(element) = self.elements.get_mut(id) else {
                return BindingWrite::unchanged();
            };
            match element
                .property_store_mut()
                .set_local_erased_with_source_notifying(
                    prop_id,
                    value,
                    LocalValueSource::TemplateBinding,
                    registry,
                ) {
                Ok(channels) => channels,
                Err(_) => return BindingWrite::unchanged(),
            }
        };
        if channels.is_empty() {
            BindingWrite::unchanged()
        } else {
            self.mark_channels(id, channels);
            BindingWrite::changed(channels)
        }
    }
}
