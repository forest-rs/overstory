// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Property writes and dependency-property binding integration.

use invalidation::ChannelSet;
use understory_property::{
    DependencyObject, DependencyObjectExt, ErasedValue, LocalValueSource, Property, PropertyId,
};
use understory_property_binding::{
    BindingDrainError, BindingError, BindingHost, BindingId, BindingReport, BindingWrite,
    EndpointKey, PropertyEndpoint,
};

use crate::binding::{BindingOwner, ExternalSource, LocalState};
use crate::element::ElementId;

use super::Ui;
use super::property_changes::{PropertyChange, StateProjectionPolicy, TemplateRefreshPolicy};

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
        let Some(_) = self.write_typed_local(id, property, value) else {
            return;
        };
        self.bindings
            .mark_endpoint_changed(EndpointKey::new(BindingOwner::element(id), property.id()));
    }

    /// Clears the ordinary local value for `property` on `id`.
    ///
    /// This removes only the app-authored [`LocalValueSource::Local`] slot.
    /// Values from template bindings, template defaults, style, theme
    /// resources, inheritance, or the property's registered default may become
    /// visible afterward. Clearing flows through the same invalidation,
    /// semantic-state projection, template refresh, and binding endpoint
    /// notification path as [`Self::set_local`].
    ///
    /// Returns `true` if a local value was removed.
    pub fn clear_local<T: Clone + PartialEq + 'static>(
        &mut self,
        id: ElementId,
        property: Property<T>,
    ) -> bool {
        let Some(removed) = self.clear_typed_local(id, property) else {
            return false;
        };
        if removed {
            self.bindings
                .mark_endpoint_changed(EndpointKey::new(BindingOwner::element(id), property.id()));
        }
        removed
    }

    /// Writes a typed local value and applies normal element write consequences.
    ///
    /// Typed writes keep the property store's typed equality check, so unchanged values can
    /// avoid broader invalidation. The shared finish step below still owns all semantic side
    /// effects of a real write.
    fn write_typed_local<T: Clone + PartialEq + 'static>(
        &mut self,
        id: ElementId,
        property: Property<T>,
        value: T,
    ) -> Option<ChannelSet> {
        // Keep the typed store path for precise no-op suppression; the erased
        // store path conservatively returns full metadata channels.
        let channels = {
            let registry = &self.registry;
            let element = self.elements.get_mut(id)?;
            element.set_local_notifying(property, value, registry)
        };
        Some(self.apply_property_change(PropertyChange {
            element: id,
            property: property.id(),
            channels,
            template_refresh: TemplateRefreshPolicy::RefreshRuntimeTemplate,
            state_projection: StateProjectionPolicy::ProjectSemanticState,
        }))
    }

    /// Clears a typed local value and applies normal element clear consequences.
    ///
    /// Returns `None` for stale element ids, `Some(false)` when the element was
    /// live but did not have a `Local` slot for `property`, and `Some(true)`
    /// when a local value was removed.
    fn clear_typed_local<T: Clone + PartialEq + 'static>(
        &mut self,
        id: ElementId,
        property: Property<T>,
    ) -> Option<bool> {
        let channels = {
            let element = self.elements.get_mut(id)?;
            if !element
                .property_store()
                .has_local_at_source(property, LocalValueSource::Local)
            {
                return Some(false);
            }
            element.clear_local_notifying(property, &self.registry)
        };
        self.apply_property_change(PropertyChange {
            element: id,
            property: property.id(),
            channels,
            template_refresh: TemplateRefreshPolicy::RefreshRuntimeTemplate,
            state_projection: StateProjectionPolicy::ProjectSemanticState,
        });
        Some(true)
    }

    /// Writes an erased local value and applies normal element write consequences.
    ///
    /// Erased writes come from spec configuration and bindings. They share invalidation,
    /// state projection, inherited-property, and template-dependency handling with typed
    /// writes, but their caller decides whether `ControlTemplate` should be re-expanded now.
    fn write_local_erased(
        &mut self,
        id: ElementId,
        property: PropertyId,
        value: ErasedValue,
        source: LocalValueSource,
        template_refresh: TemplateRefreshPolicy,
    ) -> Option<ChannelSet> {
        // Binding and config writes arrive erased, but they share all
        // post-write consequences with typed writes below.
        let channels = {
            let registry = &self.registry;
            let element = self.elements.get_mut(id)?;
            element
                .property_store_mut()
                .set_local_erased_with_source_notifying(property, value, source, registry)
                .ok()?
        };
        Some(self.apply_property_change(PropertyChange {
            element: id,
            property,
            channels,
            template_refresh,
            state_projection: StateProjectionPolicy::ProjectSemanticState,
        }))
    }

    /// Applies an erased local value during initial element construction.
    ///
    /// Initial writes notify binding sources and update semantic projections, but defer
    /// `ControlTemplate` expansion to the append/template construction lifecycle.
    pub(crate) fn set_initial_local_erased(
        &mut self,
        id: ElementId,
        property: PropertyId,
        value: ErasedValue,
    ) {
        let Some(_) = self.write_local_erased(
            id,
            property,
            value,
            LocalValueSource::Local,
            TemplateRefreshPolicy::DeferInitialExpansion,
        ) else {
            return;
        };
        self.bindings
            .mark_endpoint_changed(EndpointKey::new(BindingOwner::element(id), property));
    }

    /// Creates a local state value owned by this `Ui`.
    ///
    /// "Local" means local to this retained UI tree. Setting the value marks
    /// dependent bindings dirty, and those bindings write retained element
    /// properties during the normal frame pipeline.
    pub fn local_state<T>(&mut self, initial: T) -> LocalState<T>
    where
        T: Clone + PartialEq + 'static,
    {
        self.binding_sources.push_local(initial)
    }

    /// Reads a local state value by handle.
    #[must_use]
    pub fn local_state_value<T>(&self, state: LocalState<T>) -> Option<T>
    where
        T: Clone + 'static,
    {
        self.binding_sources.local_value(state)
    }

    /// Writes a local state value by handle.
    ///
    /// Returns `true` when the stored value changed and dependent bindings were
    /// marked dirty.
    pub fn set_local_state<T>(&mut self, state: LocalState<T>, value: T) -> bool
    where
        T: Clone + PartialEq + 'static,
    {
        if !self.binding_sources.set_local(state, value) {
            return false;
        }
        self.bindings.mark_endpoint_changed(state.endpoint().key());
        true
    }

    /// Registers an external value source for bindings.
    ///
    /// The source is pull-based. Call [`ExternalSource::notify_changed`] after
    /// the caller-owned value may have changed so the next binding drain reads
    /// the latest value.
    pub fn external_source<T, F>(&mut self, read: F) -> ExternalSource<T>
    where
        T: Clone + 'static,
        F: Fn() -> T + 'static,
    {
        self.binding_sources.push_external(read)
    }

    /// Marks an external source dirty.
    pub fn notify_external_source_changed<T>(&mut self, source: ExternalSource<T>) -> bool
    where
        T: Clone + 'static,
    {
        self.bindings.mark_endpoint_changed(source.endpoint().key())
    }

    /// Registers a template-precedence one-way binding from `source` to
    /// `target`. Both endpoints' value types must match. Returns the new
    /// [`BindingId`] or a [`BindingError`] if the registration would self-loop
    /// or introduce a cycle.
    ///
    /// Target writes use [`LocalValueSource::TemplateBinding`]. This is the
    /// low-level path used by control templates. App-facing append-spec
    /// bindings use the local source so authored bindings can replace initial
    /// constructor values.
    ///
    /// The binding will fire on the next `flush_bindings` if its source
    /// endpoint has been written since the last drain (or has not yet
    /// drained at all). Bindings are typically registered during template
    /// expansion via [`Template::instantiate`](crate::Template::instantiate).
    pub fn bind<T>(
        &mut self,
        source: PropertyEndpoint<BindingOwner, T>,
        target: PropertyEndpoint<BindingOwner, T>,
    ) -> Result<BindingId, BindingError<BindingOwner>>
    where
        T: Clone + 'static,
    {
        self.bindings.bind(source, target)
    }

    /// Registers a mapped template-precedence one-way binding from `source` to
    /// `target`.
    ///
    /// Target writes use [`LocalValueSource::TemplateBinding`]. App-facing
    /// append-spec bindings use the local source so authored bindings can
    /// replace initial constructor values.
    pub fn bind_map<S, T, F>(
        &mut self,
        source: PropertyEndpoint<BindingOwner, S>,
        target: PropertyEndpoint<BindingOwner, T>,
        map: F,
    ) -> Result<BindingId, BindingError<BindingOwner>>
    where
        S: Clone + 'static,
        T: Clone + 'static,
        F: Fn(&S) -> T + 'static,
    {
        self.bindings.bind_map(source, target, map)
    }

    pub(crate) fn bind_map_with_target_source<S, T, F>(
        &mut self,
        source: PropertyEndpoint<BindingOwner, S>,
        target: PropertyEndpoint<BindingOwner, T>,
        map: F,
        write_source: LocalValueSource,
    ) -> Result<BindingId, BindingError<BindingOwner>>
    where
        S: Clone + 'static,
        T: Clone + 'static,
        F: Fn(&S) -> T + 'static,
    {
        self.bindings
            .bind_map_with_target_source(source, target, map, write_source)
    }

    /// Drains the binding set, propagating dirty bindings through the host.
    /// Channels affected by binding writes are unioned into the invalidation
    /// tracker as a side effect of the host's `set_erased` impl.
    pub fn try_flush_bindings(&mut self) -> Result<BindingReport, BindingDrainError<BindingOwner>> {
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

impl BindingHost<BindingOwner, LocalValueSource> for Ui {
    fn get_erased(&self, endpoint: EndpointKey<BindingOwner>) -> Option<ErasedValue> {
        match endpoint.owner() {
            BindingOwner::Element(id) => {
                let element = self.get(id)?;
                element
                    .property_store()
                    .effective_local_erased(endpoint.property())
                    .cloned()
            }
            owner @ (BindingOwner::LocalState(_) | BindingOwner::ExternalSource(_)) => {
                self.binding_sources.get_erased(owner, endpoint.property())
            }
        }
    }

    fn set_erased(
        &mut self,
        endpoint: EndpointKey<BindingOwner>,
        value: ErasedValue,
        write_source: LocalValueSource,
    ) -> BindingWrite {
        match endpoint.owner() {
            BindingOwner::Element(id) => {
                let prop_id = endpoint.property();
                let Some(channels) = self.write_local_erased(
                    id,
                    prop_id,
                    value,
                    write_source,
                    TemplateRefreshPolicy::RefreshRuntimeTemplate,
                ) else {
                    return BindingWrite::unchanged();
                };
                if channels.is_empty() {
                    BindingWrite::unchanged()
                } else {
                    BindingWrite::changed(channels)
                }
            }
            BindingOwner::LocalState(id) => {
                if self.binding_sources.set_local_erased(id, value) {
                    BindingWrite::changed(ChannelSet::default())
                } else {
                    BindingWrite::unchanged()
                }
            }
            BindingOwner::ExternalSource(_) => BindingWrite::unchanged(),
        }
    }
}
