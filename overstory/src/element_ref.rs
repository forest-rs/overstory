// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Typed references to retained elements.
//!
//! `ElementRef` is an authoring-time handle for code that needs to talk about
//! an element after it is appended without threading raw [`ElementId`] values
//! through local variables. It deliberately does not create a second identity
//! system: resolving the ref still checks the retained tree's generational
//! [`ElementId`].

use alloc::rc::Rc;
use core::cell::Cell;
use core::fmt;
use core::marker::PhantomData;

use understory_property::Property;

use crate::element::{Element, ElementId};
use crate::ui::Ui;
use crate::widget::{AppendSpec, ElementConfig, ElementSpec};

/// Typed handle to an element appended into a [`Ui`].
///
/// The type parameter is a marker for app code and diagnostics; Overstory does
/// not currently runtime-check that the resolved element was created from that
/// exact builder type. Use [`ElementRefSpecExt::reference`] on a spec to attach
/// the ref during insertion.
pub struct ElementRef<T = Element> {
    id: Rc<Cell<Option<ElementId>>>,
    marker: PhantomData<fn() -> T>,
}

impl<T> ElementRef<T> {
    /// Creates an unattached element reference.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: Rc::new(Cell::new(None)),
            marker: PhantomData,
        }
    }

    /// Returns the last element id assigned to this ref.
    ///
    /// The returned id may be stale if the element has been removed. Use
    /// [`Self::resolve`] when interacting with a [`Ui`].
    #[must_use]
    pub fn current_id(&self) -> Option<ElementId> {
        self.id.get()
    }

    /// Resolves this ref against `ui`, returning `None` for stale or unattached
    /// refs.
    #[must_use]
    pub fn resolve(&self, ui: &Ui) -> Option<ElementId> {
        let id = self.current_id()?;
        ui.get(id).map(|_| id)
    }

    /// Returns whether this ref currently resolves to a live element in `ui`.
    #[must_use]
    pub fn is_live(&self, ui: &Ui) -> bool {
        self.resolve(ui).is_some()
    }

    /// Clears the ref without mutating the retained tree.
    pub fn clear(&self) {
        self.id.set(None);
    }

    /// Writes a local property value to the referenced element if it is live.
    pub fn set_local<V>(&self, ui: &mut Ui, property: Property<V>, value: V)
    where
        V: Clone + PartialEq + 'static,
    {
        let Some(id) = self.resolve(ui) else {
            return;
        };
        ui.set_local(id, property, value);
    }

    /// Clears a local property value on the referenced element if it is live.
    ///
    /// Returns `true` if a local value was removed.
    pub fn clear_local<V>(&self, ui: &mut Ui, property: Property<V>) -> bool
    where
        V: Clone + PartialEq + 'static,
    {
        let Some(id) = self.resolve(ui) else {
            return false;
        };
        ui.clear_local(id, property)
    }

    pub(crate) fn assign(&self, id: ElementId) {
        self.id.set(Some(id));
    }
}

impl<T> Clone for ElementRef<T> {
    fn clone(&self) -> Self {
        Self {
            id: Rc::clone(&self.id),
            marker: PhantomData,
        }
    }
}

impl<T> Default for ElementRef<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> fmt::Debug for ElementRef<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ElementRef")
            .field("current_id", &self.current_id())
            .finish()
    }
}

/// Append wrapper that records the inserted element into an [`ElementRef`].
pub struct ReferencedSpec<S, T> {
    spec: S,
    element_ref: ElementRef<T>,
}

impl<S, T> fmt::Debug for ReferencedSpec<S, T>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReferencedSpec")
            .field("spec", &self.spec)
            .field("element_ref", &self.element_ref)
            .finish()
    }
}

impl<S, T> ReferencedSpec<S, T> {
    /// Creates a wrapper around `spec` that assigns `element_ref` on append.
    #[must_use]
    pub fn new(spec: S, element_ref: ElementRef<T>) -> Self {
        Self { spec, element_ref }
    }
}

impl<S, T> AppendSpec for ReferencedSpec<S, T>
where
    S: AppendSpec,
{
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = self.spec.append_to(ui, parent);
        self.element_ref.assign(id);
        id
    }
}

impl<S, T> ElementSpec for ReferencedSpec<S, T>
where
    S: ElementSpec,
{
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        self.spec.element_config_mut()
    }
}

/// Extension trait for assigning a typed element ref during append.
pub trait ElementRefSpecExt: AppendSpec + Sized {
    /// Records the inserted element into `element_ref`.
    #[must_use]
    fn reference<T>(self, element_ref: ElementRef<T>) -> ReferencedSpec<Self, T> {
        ReferencedSpec::new(self, element_ref)
    }
}

impl<S> ElementRefSpecExt for S where S: AppendSpec {}
