// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Element identity and the per-element node.
//!
//! Each element carries identity, family pointers, a typed
//! dependency property store, and an optional [`Widget`] — the
//! per-element behavior + state. Anonymous containers (the root,
//! layout panels) have `widget = None`; control elements (`Button`,
//! `ToggleSwitch`, `Spinner`, `Slider`, `Text`, …) have a boxed
//! widget.

use alloc::boxed::Box;
use core::fmt;

use invalidation::DenseKey;
use smallvec::SmallVec;
use understory_property::{DependencyObject, PropertyStore};
use understory_style::{ClassId, PartTag, PseudoClassId};

use crate::widget::Widget;

/// Stable, generational handle to an [`Element`].
///
/// `ElementId`s are issued by [`crate::Ui`]. The handle survives the
/// element being removed and a slot being reused, because the
/// `generation` increments on slot reuse — a stale handle compares
/// unequal to the new occupant.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ElementId {
    pub(crate) slot: u32,
    pub(crate) generation: u32,
}

impl ElementId {
    /// Constructs an id with explicit slot and generation. Used by
    /// [`crate::Ui`] when issuing new ids; consumers should not
    /// normally build their own.
    #[must_use]
    #[inline]
    pub(crate) const fn new(slot: u32, generation: u32) -> Self {
        Self { slot, generation }
    }

    /// Returns the slot index this id was issued for. Slot indices
    /// are reused after an element is removed; pair with
    /// [`Self::generation`] to detect stale handles.
    #[must_use]
    #[inline]
    pub const fn slot(self) -> u32 {
        self.slot
    }

    /// Returns the generation counter for this id's slot at issuance
    /// time.
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
    #[inline]
    fn index(self) -> usize {
        self.slot as usize
    }
}

/// Inline capacity for an element's children list.
const CHILDREN_INLINE: usize = 4;

/// One node in Overstory's element tree.
pub struct Element {
    id: ElementId,
    parent: Option<ElementId>,
    /// The control whose template instantiated this element, if any.
    templated_parent: Option<ElementId>,
    /// Owner-local template part identity for selector matching.
    part_tag: Option<PartTag>,
    children: SmallVec<[ElementId; CHILDREN_INLINE]>,
    store: PropertyStore<ElementId>,
    /// Class identifiers this element participates in. Kept
    /// sorted+deduped so `understory_style`'s `SelectorInputs` can
    /// borrow the slice directly.
    classes: SmallVec<[ClassId; 2]>,
    /// Pseudoclass identifiers currently active on this element.
    pseudos: SmallVec<[PseudoClassId; 2]>,
    /// Per-element behavior + state. `None` for anonymous containers.
    widget: Option<Box<dyn Widget>>,
}

impl fmt::Debug for Element {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Element")
            .field("id", &self.id)
            .field("parent", &self.parent)
            .field("templated_parent", &self.templated_parent)
            .field("part_tag", &self.part_tag)
            .field("children_len", &self.children.len())
            .field("widget", &self.widget)
            .field("classes", &self.classes.as_slice())
            .field("pseudos", &self.pseudos.as_slice())
            .finish_non_exhaustive()
    }
}

impl Element {
    pub(crate) fn new(
        id: ElementId,
        parent: Option<ElementId>,
        templated_parent: Option<ElementId>,
        part_tag: Option<PartTag>,
        widget: Option<Box<dyn Widget>>,
    ) -> Self {
        Self {
            id,
            parent,
            templated_parent,
            part_tag,
            children: SmallVec::new(),
            store: PropertyStore::new(id),
            classes: SmallVec::new(),
            pseudos: SmallVec::new(),
            widget,
        }
    }

    /// Returns this element's active class identifiers in ascending
    /// order.
    #[must_use]
    #[inline]
    pub fn classes(&self) -> &[ClassId] {
        &self.classes
    }

    /// Returns this element's active pseudoclass identifiers in
    /// ascending order.
    #[must_use]
    #[inline]
    pub fn pseudos(&self) -> &[PseudoClassId] {
        &self.pseudos
    }

    /// Adds `class` to this element if not already present. Returns
    /// `true` if the set changed.
    pub fn add_class(&mut self, class: ClassId) -> bool {
        match self.classes.binary_search(&class) {
            Ok(_) => false,
            Err(idx) => {
                self.classes.insert(idx, class);
                true
            }
        }
    }

    /// Removes `class` from this element. Returns `true` if it was
    /// present.
    pub fn remove_class(&mut self, class: ClassId) -> bool {
        match self.classes.binary_search(&class) {
            Ok(idx) => {
                self.classes.remove(idx);
                true
            }
            Err(_) => false,
        }
    }

    /// Adds `pseudo` to this element if not already present.
    /// Returns `true` if the set changed.
    pub fn add_pseudo(&mut self, pseudo: PseudoClassId) -> bool {
        match self.pseudos.binary_search(&pseudo) {
            Ok(_) => false,
            Err(idx) => {
                self.pseudos.insert(idx, pseudo);
                true
            }
        }
    }

    /// Removes `pseudo` from this element. Returns `true` if it was
    /// present.
    pub fn remove_pseudo(&mut self, pseudo: PseudoClassId) -> bool {
        match self.pseudos.binary_search(&pseudo) {
            Ok(idx) => {
                self.pseudos.remove(idx);
                true
            }
            Err(_) => false,
        }
    }

    /// Returns this element's stable id.
    #[must_use]
    #[inline]
    pub fn id(&self) -> ElementId {
        self.id
    }

    /// Returns the parent element's id, or `None` for the tree
    /// root.
    #[must_use]
    #[inline]
    pub fn parent(&self) -> Option<ElementId> {
        self.parent
    }

    /// Returns the templated parent — the control whose template
    /// instantiated this element — if any.
    #[must_use]
    #[inline]
    pub fn templated_parent(&self) -> Option<ElementId> {
        self.templated_parent
    }

    /// Returns this element's owner-local template part identity, if any.
    #[must_use]
    #[inline]
    pub fn part_tag(&self) -> Option<PartTag> {
        self.part_tag
    }

    /// Returns this element's children in document order.
    #[must_use]
    #[inline]
    pub fn children(&self) -> &[ElementId] {
        &self.children
    }

    /// Returns this element's widget, if any. Anonymous containers
    /// (the root, plain panels) return `None`.
    #[must_use]
    #[inline]
    pub fn widget(&self) -> Option<&dyn Widget> {
        self.widget.as_deref()
    }

    /// Returns mutable access to this element's widget, if any.
    /// Used by widget-internal state mutation (e.g. animation
    /// timers updating a phase counter).
    #[must_use]
    #[inline]
    pub fn widget_mut(&mut self) -> Option<&mut dyn Widget> {
        self.widget.as_deref_mut()
    }

    pub(crate) fn take_widget(&mut self) -> Option<Box<dyn Widget>> {
        self.widget.take()
    }

    pub(crate) fn restore_widget(&mut self, widget: Box<dyn Widget>) {
        debug_assert!(self.widget.is_none(), "restoring over an existing widget");
        if self.widget.is_none() {
            self.widget = Some(widget);
        }
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
