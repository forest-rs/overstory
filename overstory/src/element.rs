// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Element identity, kind tags, and the per-element node.

use core::fmt;

use invalidation::DenseKey;
use smallvec::SmallVec;
use understory_property::{DependencyObject, PropertyStore};

/// Open-ended type tag identifying an element's behavior in the
/// [`crate::BehaviorRegistry`].
///
/// `ElementKind` is a plain `u32` newtype rather than an enum so applications
/// can register their own kinds without modifying the toolkit. Reserve low
/// values (0..256) for Overstory built-ins; apps should pick values above.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ElementKind(u32);

impl ElementKind {
    /// Constructs a new kind tag.
    #[must_use]
    #[inline]
    pub const fn new(tag: u32) -> Self {
        Self(tag)
    }

    /// Returns the raw tag value.
    #[must_use]
    #[inline]
    pub const fn tag(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for ElementKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ElementKind({})", self.0)
    }
}

/// Generic element kind: a panel-like leaf that fills its bounds with the
/// `Background` brush (rounded by `CornerRadius`). The Phase 1b default.
pub const ELEMENT_KIND_GENERIC: ElementKind = ElementKind::new(0);

/// Stable, generational handle to an [`Element`].
///
/// `ElementId`s are issued by [`crate::Ui`]. The handle survives the element
/// being removed and a slot being reused, because the `generation` increments
/// on slot reuse — a stale handle compares unequal to the new occupant.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ElementId {
    pub(crate) slot: u32,
    pub(crate) generation: u32,
}

impl ElementId {
    /// Constructs an id with explicit slot and generation. Used by [`crate::Ui`]
    /// when issuing new ids; consumers should not normally build their own.
    #[must_use]
    #[inline]
    pub(crate) const fn new(slot: u32, generation: u32) -> Self {
        Self { slot, generation }
    }

    /// Returns the slot index this id was issued for.
    ///
    /// Slot indices are reused after an element is removed; pair with
    /// [`Self::generation`] to detect stale handles.
    #[must_use]
    #[inline]
    pub const fn slot(self) -> u32 {
        self.slot
    }

    /// Returns the generation counter for this id's slot at issuance time.
    ///
    /// Each time a slot is recycled the generation increments by one, so a
    /// stale `ElementId` whose slot has been re-used compares unequal to the
    /// new occupant's id.
    #[must_use]
    #[inline]
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

impl fmt::Debug for ElementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ElementId(slot={}, gen={})", self.slot, self.generation)
    }
}

impl DenseKey for ElementId {
    /// Dense storage index in [`invalidation`] sets is the slot index — the
    /// generation only disambiguates stale handles, it doesn't affect
    /// dirty-marking storage. Stale ids can't reach the tracker through the
    /// `Ui` API (every entry point validates the id first).
    #[inline]
    fn index(self) -> usize {
        self.slot as usize
    }
}

/// Inline capacity for an element's children list.
///
/// Most container elements have a handful of children; this keeps the common
/// case heap-free without inflating leaves too much.
const CHILDREN_INLINE: usize = 4;

/// One node in Overstory's visual tree.
///
/// An [`Element`] carries identity, family pointers, and a typed dependency
/// property store. Per-element behavior (measure / arrange / paint / event
/// handlers) is dispatched by `ElementKind` in later phases; the bare struct
/// here is the substrate.
#[derive(Debug)]
pub struct Element {
    id: ElementId,
    kind: ElementKind,
    parent: Option<ElementId>,
    children: SmallVec<[ElementId; CHILDREN_INLINE]>,
    store: PropertyStore<ElementId>,
}

impl Element {
    pub(crate) fn new(id: ElementId, kind: ElementKind, parent: Option<ElementId>) -> Self {
        Self {
            id,
            kind,
            parent,
            children: SmallVec::new(),
            store: PropertyStore::new(id),
        }
    }

    /// Returns this element's stable id.
    #[must_use]
    #[inline]
    pub fn id(&self) -> ElementId {
        self.id
    }

    /// Returns this element's [`ElementKind`], used by the
    /// [`crate::BehaviorRegistry`] to dispatch paint/event handlers.
    #[must_use]
    #[inline]
    pub fn kind(&self) -> ElementKind {
        self.kind
    }

    /// Returns the parent element's id, or `None` for the tree root.
    #[must_use]
    #[inline]
    pub fn parent(&self) -> Option<ElementId> {
        self.parent
    }

    /// Returns this element's children in document order.
    #[must_use]
    #[inline]
    pub fn children(&self) -> &[ElementId] {
        &self.children
    }

    pub(crate) fn children_mut(&mut self) -> &mut SmallVec<[ElementId; CHILDREN_INLINE]> {
        &mut self.children
    }
}

impl DependencyObject<ElementId> for Element {
    fn property_store(&self) -> &PropertyStore<ElementId> {
        &self.store
    }

    fn property_store_mut(&mut self) -> &mut PropertyStore<ElementId> {
        &mut self.store
    }

    fn key(&self) -> ElementId {
        self.id
    }

    fn parent_key(&self) -> Option<ElementId> {
        self.parent
    }
}
