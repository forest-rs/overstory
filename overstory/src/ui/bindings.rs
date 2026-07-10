// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Property writes and dependency-property binding integration.

use alloc::boxed::Box;

use invalidation::ChannelSet;
use understory_property::{
    DependencyObject, DependencyObjectExt, ErasedValue, LocalValueSource, Property, PropertyId,
};
use understory_property_binding::{
    BindingError, BindingHost, BindingId, BindingWrite, EndpointKey, PropertyEndpoint,
    TwoWayBindingId, TwoWayDrainError, TwoWayReport,
};

use crate::binding::{BindingOwner, ExternalSource, LocalState};
use crate::element::ElementId;

use super::Ui;
use super::property_changes::{PropertyChange, StateProjectionPolicy, TemplateRefreshPolicy};

impl Ui {
    pub(in crate::ui) fn binding_graph(
        &mut self,
    ) -> &mut understory_property_binding::BindingGraph<BindingOwner, LocalValueSource> {
        self.bindings
            .as_mut()
            .expect("binding graph is only absent while flushing")
    }

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
        self.binding_graph()
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
            self.binding_graph()
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
        let channels = self.apply_property_change(PropertyChange {
            element: id,
            property: property.id(),
            channels,
            template_refresh: TemplateRefreshPolicy::RefreshRuntimeTemplate,
            state_projection: StateProjectionPolicy::ProjectSemanticState,
        });
        self.recoerce_value_after_range_change(id, property.id());
        Some(channels)
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
        self.recoerce_value_after_range_change(id, property.id());
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
        let channels = self.apply_property_change(PropertyChange {
            element: id,
            property,
            channels,
            template_refresh,
            state_projection: StateProjectionPolicy::ProjectSemanticState,
        });
        self.recoerce_value_after_range_change(id, property);
        Some(channels)
    }

    /// Re-runs `Value` coercion when either range bound changes.
    ///
    /// `understory_property` deliberately leaves cross-property coercion
    /// dependencies to its host. Overstory has one shared numeric range
    /// contract, so the dependency is explicit here rather than repeated in
    /// every value-bearing control.
    fn recoerce_value_after_range_change(&mut self, id: ElementId, property: PropertyId) {
        if property != self.properties.minimum.id() && property != self.properties.maximum.id() {
            return;
        }

        let value = self.properties.value;
        let channels = {
            let registry = &self.registry;
            let Some(element) = self.elements.get_mut(id) else {
                return;
            };
            element.coerce_value_notifying(value, registry)
        };
        if channels.is_empty() {
            return;
        }

        self.apply_property_change(PropertyChange {
            element: id,
            property: value.id(),
            channels,
            template_refresh: TemplateRefreshPolicy::RefreshRuntimeTemplate,
            state_projection: StateProjectionPolicy::ProjectSemanticState,
        });
        self.binding_graph()
            .mark_endpoint_changed(EndpointKey::new(BindingOwner::element(id), value.id()));
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
        self.binding_graph()
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
        self.binding_graph()
            .mark_endpoint_changed(state.endpoint().key());
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
        self.binding_graph()
            .mark_endpoint_changed(source.endpoint().key())
    }

    /// Returns a typed binding endpoint for `property` on `id`.
    ///
    /// Use this when app code wants a dependency property on one retained
    /// element to drive a dependency property on another retained element.
    /// The returned endpoints can be passed to [`Self::bind`] or
    /// [`Self::bind_map`], for example to map a slider's numeric value into
    /// another element's property without rebuilding the view.
    #[must_use]
    pub const fn element_endpoint<T>(
        id: ElementId,
        property: Property<T>,
    ) -> PropertyEndpoint<BindingOwner, T> {
        PropertyEndpoint::new(BindingOwner::element(id), property)
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
        let binding =
            self.binding_graph()
                .bind(source, target, LocalValueSource::TemplateBinding)?;
        self.binding_graph().mark_binding_dirty(binding);
        Ok(binding)
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
        self.bind_map_with_target_source(source, target, map, LocalValueSource::TemplateBinding)
    }

    /// Registers an app-authored two-way identity binding.
    ///
    /// The initial and subsequent source values flow forward into `target`.
    /// Host or control writes to `target` flow back into `source` during the
    /// next binding drain. Binding-driven target writes are not echoed back.
    pub fn bind_two_way<T>(
        &mut self,
        source: PropertyEndpoint<BindingOwner, T>,
        target: PropertyEndpoint<BindingOwner, T>,
    ) -> Result<TwoWayBindingId, BindingError<BindingOwner>>
    where
        T: Clone + 'static,
    {
        let binding = self.binding_graph().bind_two_way(
            source,
            target,
            LocalValueSource::Local,
            LocalValueSource::Local,
        )?;
        let forward = self
            .binding_graph()
            .forward_binding(binding)
            .expect("new two-way binding has a forward edge");
        self.binding_graph().mark_binding_dirty(forward);
        Ok(binding)
    }

    /// Registers an app-authored mapped two-way binding.
    ///
    /// `forward` maps source values into the target type. `reverse` may reject
    /// a target value by returning `None`; rejections are reported by
    /// [`Self::flush_bindings`] without poisoning other graph work.
    pub fn bind_two_way_map<S, T, F, R>(
        &mut self,
        source: PropertyEndpoint<BindingOwner, S>,
        target: PropertyEndpoint<BindingOwner, T>,
        forward: F,
        reverse: R,
    ) -> Result<TwoWayBindingId, BindingError<BindingOwner>>
    where
        S: Clone + 'static,
        T: Clone + 'static,
        F: Fn(&S) -> T + 'static,
        R: Fn(&T) -> Option<S> + 'static,
    {
        let binding = self.binding_graph().try_bind_two_way_map(
            source,
            target,
            LocalValueSource::Local,
            LocalValueSource::Local,
            forward,
            reverse,
        )?;
        let forward = self
            .binding_graph()
            .forward_binding(binding)
            .expect("new two-way binding has a forward edge");
        self.binding_graph().mark_binding_dirty(forward);
        Ok(binding)
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
        let binding = self
            .binding_graph()
            .bind_map(source, target, write_source, map)?;
        self.binding_graph().mark_binding_dirty(binding);
        Ok(binding)
    }

    /// Drains the binding set, propagating dirty bindings through the host.
    /// Channels affected by binding writes are unioned into the invalidation
    /// tracker as a side effect of the host's `set_erased` impl.
    pub fn try_flush_bindings(
        &mut self,
    ) -> Result<TwoWayReport<BindingOwner>, Box<TwoWayDrainError<BindingOwner>>> {
        let mut bindings = self
            .bindings
            .take()
            .expect("binding graph is only absent while flushing");
        let result = bindings.drain(self);
        self.bindings = Some(bindings);
        for owner in core::mem::take(&mut self.binding_retemplates) {
            self.retemplate_if_needed(owner);
        }
        result.map_err(Box::new)
    }

    /// Drains dirty bindings and panics if a real binding evaluation error is
    /// found. Use [`Self::try_flush_bindings`] when binding graph or host type
    /// errors should be handled by the caller.
    pub fn flush_bindings(&mut self) -> TwoWayReport<BindingOwner> {
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
                let refresh = if prop_id == self.properties.control_template.id() {
                    TemplateRefreshPolicy::DoNotRefreshTemplate
                } else {
                    TemplateRefreshPolicy::RefreshRuntimeTemplate
                };
                let Some(channels) =
                    self.write_local_erased(id, prop_id, value, write_source, refresh)
                else {
                    return BindingWrite::unchanged();
                };
                if refresh == TemplateRefreshPolicy::DoNotRefreshTemplate {
                    self.binding_retemplates.push(id);
                }
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
