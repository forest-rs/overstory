// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The root [`Ui`] context: owns the visual tree and hands out [`ElementId`]s.

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::element::{Element, ElementId};

/// Root context for an Overstory UI instance.
///
/// Owns the visual tree, the slot allocator, and (in later phases) the
/// invalidation cascade, behavior registry, style cascade, layout (taffy),
/// paint cache, input dispatcher, and accessibility mirror.
///
/// In Phase 1a only the visual tree exists; everything else is built up in
/// subsequent phases.
#[derive(Debug)]
pub struct Ui {
    slots: Vec<Slot>,
    /// Free list of slot indices available for reuse. LIFO for locality.
    free: Vec<u32>,
    root: ElementId,
}

#[derive(Debug)]
enum Slot {
    /// `Element` is heap-allocated so that `Vacant` slots stay tiny in the
    /// presence of element churn — important since the tree's slot vec is
    /// the toolkit's central hot data structure.
    Live {
        generation: u32,
        element: Box<Element>,
    },
    /// A previously-live slot that has been removed. `generation` is the
    /// generation of the *next* element to occupy this slot — already
    /// incremented past the removed element's generation so stale handles
    /// compare unequal.
    Vacant { generation: u32 },
}

impl Ui {
    /// Creates a new [`Ui`] with an empty root element.
    #[must_use]
    pub fn new() -> Self {
        let mut me = Self {
            slots: Vec::new(),
            free: Vec::new(),
            root: ElementId {
                slot: 0,
                generation: 0,
            },
        };
        let root = me.alloc_element(None);
        me.root = root;
        me
    }

    /// Returns the id of the tree root.
    #[must_use]
    #[inline]
    pub fn root(&self) -> ElementId {
        self.root
    }

    /// Returns the total number of live elements (including the root).
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len() - self.free.len()
    }

    /// Returns `true` if no elements are live.
    ///
    /// (`Ui::new` always inserts a root, so this is `false` for any [`Ui`]
    /// constructed via [`Self::new`].)
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a reference to the element with the given id, or `None` if the
    /// id is stale (the slot has been recycled or removed).
    #[must_use]
    pub fn get(&self, id: ElementId) -> Option<&Element> {
        match self.slots.get(id.slot as usize)? {
            Slot::Live {
                generation,
                element,
            } if *generation == id.generation => Some(element.as_ref()),
            _ => None,
        }
    }

    /// Returns a mutable reference to the element with the given id, or `None`
    /// if the id is stale.
    #[must_use]
    pub fn get_mut(&mut self, id: ElementId) -> Option<&mut Element> {
        match self.slots.get_mut(id.slot as usize)? {
            Slot::Live {
                generation,
                element,
            } if *generation == id.generation => Some(element.as_mut()),
            _ => None,
        }
    }

    /// Inserts a new child under `parent` and returns its id.
    ///
    /// # Panics
    ///
    /// Panics if `parent` is a stale id.
    pub fn insert_child(&mut self, parent: ElementId) -> ElementId {
        assert!(
            self.get(parent).is_some(),
            "insert_child: parent id is stale"
        );
        let child = self.alloc_element(Some(parent));
        // SAFETY of unwrap: we asserted `parent` is live above and `alloc_element`
        // only mutates the child slot.
        self.get_mut(parent).unwrap().children_mut().push(child);
        child
    }

    /// Removes `id` and all of its descendants from the tree.
    ///
    /// Each removed slot's generation is incremented so subsequent reuse
    /// invalidates outstanding handles.
    ///
    /// # Panics
    ///
    /// Panics if `id` is the root (the root cannot be removed) or if `id` is
    /// stale.
    pub fn remove(&mut self, id: ElementId) {
        assert!(id != self.root, "remove: cannot remove the tree root");
        assert!(self.get(id).is_some(), "remove: id is stale");

        // Detach from parent's children list.
        if let Some(parent) = self.get(id).and_then(Element::parent)
            && let Some(parent_node) = self.get_mut(parent)
        {
            parent_node.children_mut().retain(|c| *c != id);
        }
        self.remove_subtree(id);
    }

    fn remove_subtree(&mut self, id: ElementId) {
        let children: Vec<ElementId> = self
            .get(id)
            .map(|e| e.children().to_vec())
            .unwrap_or_default();
        for child in children {
            self.remove_subtree(child);
        }
        if let Some(Slot::Live { generation, .. }) = self.slots.get(id.slot as usize) {
            let next_generation = generation.wrapping_add(1);
            self.slots[id.slot as usize] = Slot::Vacant {
                generation: next_generation,
            };
            self.free.push(id.slot);
        }
    }

    fn alloc_element(&mut self, parent: Option<ElementId>) -> ElementId {
        if let Some(slot_index) = self.free.pop() {
            let slot = &mut self.slots[slot_index as usize];
            let generation = match slot {
                Slot::Vacant { generation } => *generation,
                Slot::Live { .. } => {
                    unreachable!("free list invariant: free slots are Vacant")
                }
            };
            let id = ElementId {
                slot: slot_index,
                generation,
            };
            *slot = Slot::Live {
                generation,
                element: Box::new(Element::new(id, parent)),
            };
            id
        } else {
            let slot_index = u32::try_from(self.slots.len()).expect("Ui slot index overflowed u32");
            let id = ElementId {
                slot: slot_index,
                generation: 0,
            };
            self.slots.push(Slot::Live {
                generation: 0,
                element: Box::new(Element::new(id, parent)),
            });
            id
        }
    }
}

impl Default for Ui {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_ui_has_root() {
        let ui = Ui::new();
        let root = ui.root();
        assert!(ui.get(root).is_some());
        assert_eq!(ui.len(), 1);
        assert_eq!(ui.get(root).unwrap().parent(), None);
        assert!(ui.get(root).unwrap().children().is_empty());
    }

    #[test]
    fn insert_child_appends_to_parent() {
        let mut ui = Ui::new();
        let root = ui.root();
        let a = ui.insert_child(root);
        let b = ui.insert_child(root);

        assert_eq!(ui.len(), 3);
        let root_children = ui.get(root).unwrap().children();
        assert_eq!(root_children, &[a, b]);
        assert_eq!(ui.get(a).unwrap().parent(), Some(root));
        assert_eq!(ui.get(b).unwrap().parent(), Some(root));
    }

    #[test]
    fn remove_drops_subtree_and_invalidates_handles() {
        let mut ui = Ui::new();
        let root = ui.root();
        let a = ui.insert_child(root);
        let grandchild = ui.insert_child(a);
        let b = ui.insert_child(root);

        ui.remove(a);

        // `a` and `grandchild` are gone; `b` survives.
        assert!(ui.get(a).is_none());
        assert!(ui.get(grandchild).is_none());
        assert!(ui.get(b).is_some());
        assert_eq!(ui.get(root).unwrap().children(), &[b]);
        assert_eq!(ui.len(), 2);
    }

    #[test]
    fn slot_reuse_invalidates_old_handle() {
        let mut ui = Ui::new();
        let root = ui.root();
        let a = ui.insert_child(root);
        ui.remove(a);

        let c = ui.insert_child(root);
        // The free list reused `a`'s slot, but with a fresh generation.
        assert_eq!(c.slot(), a.slot());
        assert_ne!(c.generation(), a.generation());
        assert!(ui.get(a).is_none());
        assert!(ui.get(c).is_some());
    }

    #[test]
    #[should_panic(expected = "cannot remove the tree root")]
    fn remove_root_panics() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.remove(root);
    }

    #[test]
    #[should_panic(expected = "parent id is stale")]
    fn insert_child_with_stale_parent_panics() {
        let mut ui = Ui::new();
        let root = ui.root();
        let a = ui.insert_child(root);
        ui.remove(a);
        let _ = ui.insert_child(a);
    }
}
