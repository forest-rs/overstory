// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::boxed::Box;
use alloc::vec::Vec;

use understory_style::PartTag;

use crate::element::{Element, ElementId};
use crate::widget::Widget;

/// Retained element arena plus parent/child links for one UI tree.
pub(crate) struct ElementStore {
    slots: Vec<Slot>,
    free: Vec<u32>,
    root: ElementId,
}

impl ElementStore {
    pub(crate) fn new(root_widget: Box<dyn Widget>) -> Self {
        let mut store = Self {
            slots: Vec::new(),
            free: Vec::new(),
            root: ElementId::new(0, 0),
        };
        let root = store.alloc_element(None, None, None, Some(root_widget));
        store.root = root;
        store
    }

    #[inline]
    pub(crate) fn root(&self) -> ElementId {
        self.root
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.slots.len() - self.free.len()
    }

    #[inline]
    pub(crate) fn free_len(&self) -> usize {
        self.free.len()
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn get(&self, id: ElementId) -> Option<&Element> {
        match self.slots.get(id.slot() as usize)? {
            Slot::Live {
                generation,
                element,
            } if *generation == id.generation() => Some(element.as_ref()),
            _ => None,
        }
    }

    pub(crate) fn get_mut(&mut self, id: ElementId) -> Option<&mut Element> {
        match self.slots.get_mut(id.slot() as usize)? {
            Slot::Live {
                generation,
                element,
            } if *generation == id.generation() => Some(element.as_mut()),
            _ => None,
        }
    }

    pub(crate) fn insert_child(
        &mut self,
        parent: ElementId,
        templated_parent: Option<ElementId>,
        part_tag: Option<PartTag>,
        widget: Option<Box<dyn Widget>>,
    ) -> ElementId {
        assert!(
            self.get(parent).is_some(),
            "insert_child: parent id is stale"
        );
        let child = self.alloc_element(Some(parent), templated_parent, part_tag, widget);
        self.get_mut(parent)
            .expect("parent was just checked")
            .children_mut()
            .push(child);
        child
    }

    pub(crate) fn detach_from_parent(&mut self, id: ElementId) -> Option<ElementId> {
        let parent = self.get(id).and_then(Element::parent)?;
        self.get_mut(parent)?
            .children_mut()
            .retain(|child| *child != id);
        Some(parent)
    }

    pub(crate) fn remove_slot(&mut self, id: ElementId) -> bool {
        if let Some(Slot::Live { generation, .. }) = self.slots.get(id.slot() as usize) {
            let next_generation = generation.wrapping_add(1);
            self.slots[id.slot() as usize] = Slot::Vacant {
                generation: next_generation,
            };
            self.free.push(id.slot());
            true
        } else {
            false
        }
    }

    fn alloc_element(
        &mut self,
        parent: Option<ElementId>,
        templated_parent: Option<ElementId>,
        part_tag: Option<PartTag>,
        widget: Option<Box<dyn Widget>>,
    ) -> ElementId {
        if let Some(slot_index) = self.free.pop() {
            let slot = &mut self.slots[slot_index as usize];
            let generation = match slot {
                Slot::Vacant { generation } => *generation,
                Slot::Live { .. } => {
                    unreachable!("free list invariant: free slots are Vacant")
                }
            };
            let id = ElementId::new(slot_index, generation);
            *slot = Slot::Live {
                generation,
                element: Box::new(Element::new(id, parent, templated_parent, part_tag, widget)),
            };
            id
        } else {
            let slot_index = u32::try_from(self.slots.len()).expect("Ui slot index overflowed u32");
            let id = ElementId::new(slot_index, 0);
            self.slots.push(Slot::Live {
                generation: 0,
                element: Box::new(Element::new(id, parent, templated_parent, part_tag, widget)),
            });
            id
        }
    }
}

enum Slot {
    /// `Element` is heap-allocated so that `Vacant` slots stay tiny in the
    /// presence of element churn.
    Live {
        generation: u32,
        element: Box<Element>,
    },
    /// A previously-live slot that has been removed. `generation` is the
    /// generation of the next element to occupy this slot.
    Vacant { generation: u32 },
}
