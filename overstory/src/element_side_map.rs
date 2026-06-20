// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Dense side storage keyed by live element ids.
//!
//! Roadmap:
//! - Keep side data close to the runtime that owns it while sharing one stale-id
//!   safe dense map implementation.
//! - Add capacity and memory diagnostics for large trees, virtualized controls,
//!   and benchmark scenarios.
//! - Consider arena-owned side storage only if repeated runtime maps become a
//!   measured cost or make lifetime invariants harder to see.

use alloc::vec::Vec;

use crate::element::ElementId;

/// Dense side storage keyed by live [`ElementId`] values.
///
/// `ElementStore` owns element lifetime. This map mirrors that identity space
/// for runtimes that need one optional value per element without paying for a
/// hash table keyed by `ElementId`.
#[derive(Clone, Debug)]
pub(crate) struct ElementSideMap<T> {
    slots: Vec<Option<ElementSideSlot<T>>>,
}

#[derive(Clone, Debug)]
struct ElementSideSlot<T> {
    generation: u32,
    value: T,
}

impl<T> Default for ElementSideMap<T> {
    fn default() -> Self {
        Self { slots: Vec::new() }
    }
}

impl<T> ElementSideMap<T> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn clear(&mut self) {
        self.slots.clear();
    }

    pub(crate) fn insert(&mut self, id: ElementId, value: T) -> Option<T> {
        let index = id.slot() as usize;
        if self.slots.len() <= index {
            self.slots.resize_with(index + 1, || None);
        }
        let old = self.slots[index].replace(ElementSideSlot {
            generation: id.generation(),
            value,
        });
        old.and_then(|old| (old.generation == id.generation()).then_some(old.value))
    }

    pub(crate) fn get(&self, id: ElementId) -> Option<&T> {
        let slot = self.slots.get(id.slot() as usize)?.as_ref()?;
        (slot.generation == id.generation()).then_some(&slot.value)
    }

    pub(crate) fn remove(&mut self, id: ElementId) -> Option<T> {
        let slot = self.slots.get_mut(id.slot() as usize)?;
        slot.take_if(|entry| entry.generation == id.generation())
            .map(|entry| entry.value)
    }

    pub(crate) fn values_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.slots
            .iter_mut()
            .filter_map(|slot| slot.as_mut().map(|slot| &mut slot.value))
    }

    pub(crate) fn retain(&mut self, mut keep: impl FnMut(ElementId, &mut T) -> bool) {
        for (slot, entry) in self.slots.iter_mut().enumerate() {
            let Some(occupied) = entry.as_mut() else {
                continue;
            };
            let id = ElementId::new(
                u32::try_from(slot).expect("ElementSideMap slot overflowed u32"),
                occupied.generation,
            );
            if !keep(id, &mut occupied.value) {
                *entry = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ElementSideMap;
    use crate::element::ElementId;

    #[test]
    fn stale_generation_does_not_read_or_remove_new_value() {
        let old = ElementId::new(2, 0);
        let new = ElementId::new(2, 1);
        let mut map = ElementSideMap::new();

        map.insert(new, "current");

        assert_eq!(map.get(old), None);
        assert_eq!(map.remove(old), None);
        assert_eq!(map.get(new), Some(&"current"));
    }

    #[test]
    fn retain_receives_reconstructed_element_ids() {
        let a = ElementId::new(0, 3);
        let b = ElementId::new(4, 9);
        let mut map = ElementSideMap::new();

        map.insert(a, 10);
        map.insert(b, 20);
        map.retain(|id, value| {
            *value += 1;
            id == b
        });

        assert_eq!(map.get(a), None);
        assert_eq!(map.get(b), Some(&21));
    }
}
