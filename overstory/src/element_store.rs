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
        let slot = self.slots.get(id.slot() as usize)?;
        if slot.generation == id.generation() {
            slot.element.as_ref()
        } else {
            None
        }
    }

    pub(crate) fn get_mut(&mut self, id: ElementId) -> Option<&mut Element> {
        let slot = self.slots.get_mut(id.slot() as usize)?;
        if slot.generation == id.generation() {
            slot.element.as_mut()
        } else {
            None
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
        let Some(slot) = self.slots.get_mut(id.slot() as usize) else {
            return false;
        };
        if slot.generation != id.generation() || slot.element.is_none() {
            return false;
        }
        slot.element = None;
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push(id.slot());
        true
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
            debug_assert!(slot.element.is_none(), "free slots are vacant");
            let id = ElementId::new(slot_index, slot.generation);
            slot.element = Some(Element::new(id, parent, templated_parent, part_tag, widget));
            id
        } else {
            let slot_index = u32::try_from(self.slots.len()).expect("Ui slot index overflowed u32");
            let id = ElementId::new(slot_index, 0);
            self.slots.push(Slot {
                generation: 0,
                element: Some(Element::new(id, parent, templated_parent, part_tag, widget)),
            });
            id
        }
    }
}

struct Slot {
    /// Generation for the current live element, or the next element to occupy
    /// this slot when vacant.
    generation: u32,
    element: Option<Element>,
}
