// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! App-facing property binding sources and append-spec binding helpers.
//!
//! Overstory uses `understory_property_binding` as the generic endpoint graph.
//! This module owns the UI-specific endpoint owners: retained elements,
//! local app state, and explicitly refreshed external sources.

use alloc::boxed::Box;
use core::marker::PhantomData;

use understory_property::{ErasedValue, Property, PropertyId};
use understory_property_binding::PropertyEndpoint;

use crate::element::ElementId;
use crate::properties::BuiltInProperties;
use crate::ui::Ui;
use crate::widget::{AppendSpec, ElementConfig, ElementSpec};

const SOURCE_VALUE_PROPERTY: PropertyId = PropertyId::new(u16::MAX);

type BindingMap<Source, Target> = Box<dyn Fn(&Source) -> Target>;

/// Owner key for endpoints in Overstory's retained binding graph.
///
/// The binding graph is still property-oriented, but sources are not limited
/// to elements. A local state value or an external source can participate in
/// the same dependency ordering and drain pass as template bindings.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum BindingOwner {
    /// A retained UI element.
    Element(ElementId),
    /// A UI-owned local state slot.
    LocalState(LocalStateId),
    /// A caller-owned value read through an explicit external source adapter.
    ExternalSource(ExternalSourceId),
}

impl BindingOwner {
    /// Returns an owner key for an element endpoint.
    #[must_use]
    pub const fn element(id: ElementId) -> Self {
        Self::Element(id)
    }

    /// Returns an owner key for a local state endpoint.
    #[must_use]
    pub const fn local_state(id: LocalStateId) -> Self {
        Self::LocalState(id)
    }

    /// Returns an owner key for an external source endpoint.
    #[must_use]
    pub const fn external_source(id: ExternalSourceId) -> Self {
        Self::ExternalSource(id)
    }
}

impl From<ElementId> for BindingOwner {
    fn from(value: ElementId) -> Self {
        Self::Element(value)
    }
}

/// Stable id for a local state slot owned by one [`Ui`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct LocalStateId(usize);

impl LocalStateId {
    /// Returns the slot index inside the owning UI.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Stable id for an external binding source registered with one [`Ui`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ExternalSourceId(usize);

impl ExternalSourceId {
    /// Returns the slot index inside the owning UI.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Handle to a local state value owned by one [`Ui`] instance.
///
/// "Local" means local to the retained UI tree that created the handle, not
/// local to a widget, control, or builder closure. Local state is useful for
/// app or demo values that should feed retained dependency properties without
/// rerunning a view builder. It is deliberately just a binding source and
/// optional binding target, not a component state model.
pub struct LocalState<T> {
    id: LocalStateId,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Copy for LocalState<T> {}

impl<T> Clone for LocalState<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> core::fmt::Debug for LocalState<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LocalState").field("id", &self.id).finish()
    }
}

impl<T> LocalState<T> {
    pub(crate) const fn new(id: LocalStateId) -> Self {
        Self {
            id,
            _marker: PhantomData,
        }
    }

    /// Returns this state's stable id.
    #[must_use]
    pub const fn id(self) -> LocalStateId {
        self.id
    }

    /// Returns this state's typed binding endpoint.
    #[must_use]
    pub fn endpoint(self) -> PropertyEndpoint<BindingOwner, T> {
        PropertyEndpoint::new(BindingOwner::local_state(self.id), state_value_property())
    }
}

impl<T> LocalState<T>
where
    T: Clone + 'static,
{
    /// Reads the current state value from `ui`.
    #[must_use]
    pub fn get(self, ui: &Ui) -> Option<T> {
        ui.local_state_value(self)
    }

    /// Creates an identity binding from this state value.
    #[must_use]
    pub fn binding(self) -> ValueBinding<T> {
        ValueBinding::from_endpoint(self.endpoint())
    }

    /// Creates a mapped binding from this state value.
    #[must_use]
    pub fn map<U, F>(self, map: F) -> ValueBinding<T, U>
    where
        U: 'static,
        F: Fn(&T) -> U + 'static,
    {
        self.binding().map(map)
    }
}

impl<T> LocalState<T>
where
    T: Clone + PartialEq + 'static,
{
    /// Writes a new value into this state slot.
    ///
    /// Returns `true` when the stored value changed and dependent bindings were
    /// marked dirty.
    pub fn set(self, ui: &mut Ui, value: T) -> bool {
        ui.set_local_state(self, value)
    }

    /// Mutates the stored value in place.
    ///
    /// Returns `true` when the mutation changed the observable value and
    /// dependent bindings were marked dirty.
    pub fn update(self, ui: &mut Ui, f: impl FnOnce(&mut T)) -> bool {
        let Some(mut value) = self.get(ui) else {
            return false;
        };
        let before = value.clone();
        f(&mut value);
        if value == before {
            return false;
        }
        self.set(ui, value)
    }
}

/// Handle to a caller-owned value that can feed the binding graph.
///
/// External sources are pull-based: Overstory reads the value during binding
/// drain after the caller explicitly marks the source changed.
pub struct ExternalSource<T> {
    id: ExternalSourceId,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Copy for ExternalSource<T> {}

impl<T> Clone for ExternalSource<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> core::fmt::Debug for ExternalSource<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ExternalSource")
            .field("id", &self.id)
            .finish()
    }
}

impl<T> ExternalSource<T> {
    pub(crate) const fn new(id: ExternalSourceId) -> Self {
        Self {
            id,
            _marker: PhantomData,
        }
    }

    /// Returns this source's stable id.
    #[must_use]
    pub const fn id(self) -> ExternalSourceId {
        self.id
    }

    /// Returns this source's typed binding endpoint.
    #[must_use]
    pub fn endpoint(self) -> PropertyEndpoint<BindingOwner, T> {
        PropertyEndpoint::new(
            BindingOwner::external_source(self.id),
            state_value_property(),
        )
    }
}

impl<T> ExternalSource<T>
where
    T: Clone + 'static,
{
    /// Creates an identity binding from this source.
    #[must_use]
    pub fn binding(self) -> ValueBinding<T> {
        ValueBinding::from_endpoint(self.endpoint())
    }

    /// Creates a mapped binding from this source.
    #[must_use]
    pub fn map<U, F>(self, map: F) -> ValueBinding<T, U>
    where
        U: 'static,
        F: Fn(&T) -> U + 'static,
    {
        self.binding().map(map)
    }

    /// Marks this source dirty so dependent bindings read it on the next drain.
    pub fn notify_changed(self, ui: &mut Ui) -> bool {
        ui.notify_external_source_changed(self)
    }
}

/// A typed value expression that can feed a dependency property binding.
///
/// The expression is one-way: when the source endpoint changes, the mapping
/// function produces the next target property value.
pub struct ValueBinding<Source, Target = Source> {
    source: PropertyEndpoint<BindingOwner, Source>,
    map: BindingMap<Source, Target>,
}

impl<Source, Target> core::fmt::Debug for ValueBinding<Source, Target> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ValueBinding")
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

impl<T> ValueBinding<T>
where
    T: Clone + 'static,
{
    /// Creates an identity binding from a typed source endpoint.
    #[must_use]
    pub fn from_endpoint(source: PropertyEndpoint<BindingOwner, T>) -> Self {
        Self {
            source,
            map: Box::new(T::clone),
        }
    }
}

impl<Source, Target> ValueBinding<Source, Target>
where
    Source: Clone + 'static,
    Target: 'static,
{
    /// Returns the source endpoint that this expression reads.
    #[must_use]
    pub const fn source(&self) -> PropertyEndpoint<BindingOwner, Source> {
        self.source
    }

    /// Maps the expression's target value into another type.
    #[must_use]
    pub fn map<Next, F>(self, map: F) -> ValueBinding<Source, Next>
    where
        Next: 'static,
        F: Fn(&Target) -> Next + 'static,
    {
        let prior = self.map;
        ValueBinding {
            source: self.source,
            map: Box::new(move |source| {
                let target = prior(source);
                map(&target)
            }),
        }
    }

    fn into_parts(
        self,
    ) -> (
        PropertyEndpoint<BindingOwner, Source>,
        BindingMap<Source, Target>,
    ) {
        (self.source, self.map)
    }
}

/// Extension methods for binding append specs to app state.
pub trait BindingSpecExt: AppendSpec + Sized {
    /// Binds `binding` into `property` on the element produced by this spec.
    #[must_use]
    fn bind_property<Source, Target>(
        self,
        property: Property<Target>,
        binding: ValueBinding<Source, Target>,
    ) -> BoundSpec<Self, Source, Target>
    where
        Source: Clone + 'static,
        Target: Clone + 'static,
    {
        BoundSpec::new(self, BindingTarget::Property(property), binding)
    }

    /// Binds `binding` into the built-in text/content property.
    #[must_use]
    fn bind_content<Source>(
        self,
        binding: ValueBinding<Source, alloc::string::String>,
    ) -> BoundSpec<Self, Source, alloc::string::String>
    where
        Source: Clone + 'static,
    {
        BoundSpec::new(self, BindingTarget::BuiltIn(content_property), binding)
    }
}

impl<T> BindingSpecExt for T where T: AppendSpec {}

/// Append spec wrapper that registers one property binding after insertion.
pub struct BoundSpec<Spec, Source, Target> {
    spec: Spec,
    target: BindingTarget<Target>,
    binding: ValueBinding<Source, Target>,
}

impl<Spec, Source, Target> core::fmt::Debug for BoundSpec<Spec, Source, Target>
where
    Spec: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BoundSpec")
            .field("spec", &self.spec)
            .field("target", &self.target)
            .field("binding", &self.binding)
            .finish()
    }
}

impl<Spec, Source, Target> BoundSpec<Spec, Source, Target> {
    fn new(
        spec: Spec,
        target: BindingTarget<Target>,
        binding: ValueBinding<Source, Target>,
    ) -> Self {
        Self {
            spec,
            target,
            binding,
        }
    }
}

impl<Spec, Source, Target> AppendSpec for BoundSpec<Spec, Source, Target>
where
    Spec: AppendSpec,
    Source: Clone + 'static,
    Target: Clone + 'static,
{
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = self.spec.append_to(ui, parent);
        let target_property = self.target.resolve(ui.properties());
        let target = PropertyEndpoint::new(BindingOwner::element(id), target_property);
        let (source, map) = self.binding.into_parts();
        ui.bind_map(source, target, move |value| map(value))
            .expect("append-spec binding should be valid");
        id
    }
}

impl<Spec, Source, Target> ElementSpec for BoundSpec<Spec, Source, Target>
where
    Spec: ElementSpec,
    Source: Clone + 'static,
    Target: Clone + 'static,
{
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        self.spec.element_config_mut()
    }
}

enum BindingTarget<T> {
    Property(Property<T>),
    BuiltIn(fn(&BuiltInProperties) -> Property<T>),
}

impl<T> core::fmt::Debug for BindingTarget<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Property(property) => f.debug_tuple("Property").field(property).finish(),
            Self::BuiltIn(_) => f.write_str("BuiltIn(..)"),
        }
    }
}

impl<T> BindingTarget<T> {
    fn resolve(self, properties: &BuiltInProperties) -> Property<T> {
        match self {
            Self::Property(property) => property,
            Self::BuiltIn(resolve) => resolve(properties),
        }
    }
}

pub(crate) struct BindingSourceRuntime {
    local_states: alloc::vec::Vec<LocalStateEntry>,
    external_sources: alloc::vec::Vec<ExternalSourceEntry>,
}

impl BindingSourceRuntime {
    pub(crate) fn new() -> Self {
        Self {
            local_states: alloc::vec::Vec::new(),
            external_sources: alloc::vec::Vec::new(),
        }
    }

    pub(crate) fn push_local<T>(&mut self, value: T) -> LocalState<T>
    where
        T: Clone + 'static,
    {
        let id = LocalStateId(self.local_states.len());
        self.local_states.push(LocalStateEntry {
            value: ErasedValue::new(value),
        });
        LocalState::new(id)
    }

    pub(crate) fn push_external<T, F>(&mut self, read: F) -> ExternalSource<T>
    where
        T: Clone + 'static,
        F: Fn() -> T + 'static,
    {
        let id = ExternalSourceId(self.external_sources.len());
        self.external_sources.push(ExternalSourceEntry {
            read: Box::new(move || ErasedValue::new(read())),
        });
        ExternalSource::new(id)
    }

    pub(crate) fn local_value<T>(&self, state: LocalState<T>) -> Option<T>
    where
        T: Clone + 'static,
    {
        self.local_states
            .get(state.id.index())?
            .value
            .downcast_ref::<T>()
            .cloned()
    }

    pub(crate) fn set_local_erased(&mut self, id: LocalStateId, value: ErasedValue) -> bool {
        let Some(entry) = self.local_states.get_mut(id.index()) else {
            return false;
        };
        entry.value = value;
        true
    }

    pub(crate) fn set_local<T>(&mut self, state: LocalState<T>, value: T) -> bool
    where
        T: Clone + PartialEq + 'static,
    {
        let Some(entry) = self.local_states.get_mut(state.id.index()) else {
            return false;
        };
        if entry.value.downcast_ref::<T>() == Some(&value) {
            return false;
        }
        entry.value = ErasedValue::new(value);
        true
    }

    pub(crate) fn get_erased(
        &self,
        owner: BindingOwner,
        property: PropertyId,
    ) -> Option<ErasedValue> {
        if property != SOURCE_VALUE_PROPERTY {
            return None;
        }
        match owner {
            BindingOwner::Element(_) => None,
            BindingOwner::LocalState(id) => Some(self.local_states.get(id.index())?.value.clone()),
            BindingOwner::ExternalSource(id) => {
                let entry = self.external_sources.get(id.index())?;
                Some((entry.read)())
            }
        }
    }
}

struct LocalStateEntry {
    value: ErasedValue,
}

struct ExternalSourceEntry {
    read: Box<dyn Fn() -> ErasedValue>,
}

const fn state_value_property<T>() -> Property<T> {
    Property::from_id(SOURCE_VALUE_PROPERTY)
}

fn content_property(properties: &BuiltInProperties) -> Property<alloc::string::String> {
    properties.content
}
