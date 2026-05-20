// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The root [`Ui`] context: owns the visual tree, property registry, behavior
//! registry, invalidation tracker, and per-element scene cache.
//!
//! In Phase 1b only paint flows through here; later phases add style cascade,
//! layout (taffy), templates, and input dispatch.

use alloc::boxed::Box;
use alloc::vec::Vec;

use hashbrown::HashMap;
use imaging::record::Scene;
use invalidation::{ChannelSet, InvalidationTracker};
use kurbo::Affine;
use understory_property::{DependencyObjectExt, Property, PropertyRegistry};

use crate::behavior::{BehaviorRegistry, PaintContext};
use crate::channels::{COMPOSE, PAINT};
use crate::element::{ELEMENT_KIND_GENERIC, Element, ElementId, ElementKind};
use crate::generic::GENERIC_BEHAVIOR;
use crate::properties::BuiltInProperties;

/// Root context for an Overstory UI instance.
///
/// Owns the visual tree, the slot allocator, the property and behavior
/// registries, the invalidation tracker, and a per-element [`Scene`] cache.
///
/// Phase 1b wires only paint through this struct; style cascade, taffy
/// layout, templates, and input dispatch come in later phases.
pub struct Ui {
    slots: Vec<Slot>,
    /// Free list of slot indices available for reuse. LIFO for locality.
    free: Vec<u32>,
    root: ElementId,
    registry: PropertyRegistry,
    properties: BuiltInProperties,
    behaviors: BehaviorRegistry,
    invalidation: InvalidationTracker<ElementId>,
    scene_cache: HashMap<ElementId, Scene>,
}

impl core::fmt::Debug for Ui {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Ui")
            .field("elements", &(self.slots.len() - self.free.len()))
            .field("free_slots", &self.free.len())
            .field("paint_dirty", &self.invalidation.has_invalidated(PAINT))
            .finish_non_exhaustive()
    }
}

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
    /// Creates a new [`Ui`] with an empty root element of kind
    /// [`ELEMENT_KIND_GENERIC`], the built-in properties registered, and the
    /// generic behavior installed for `ELEMENT_KIND_GENERIC`.
    #[must_use]
    pub fn new() -> Self {
        let mut registry = PropertyRegistry::new();
        let properties = BuiltInProperties::register(&mut registry);
        let mut behaviors = BehaviorRegistry::new();
        behaviors.register(ELEMENT_KIND_GENERIC, &GENERIC_BEHAVIOR);

        let mut me = Self {
            slots: Vec::new(),
            free: Vec::new(),
            root: ElementId {
                slot: 0,
                generation: 0,
            },
            registry,
            properties,
            behaviors,
            invalidation: InvalidationTracker::new(),
            scene_cache: HashMap::new(),
        };
        let root = me.alloc_element(ELEMENT_KIND_GENERIC, None);
        me.root = root;
        me
    }

    /// Returns the id of the tree root.
    #[must_use]
    #[inline]
    pub fn root(&self) -> ElementId {
        self.root
    }

    /// Returns the property registry. Application code can register its own
    /// properties on top of the built-ins.
    #[must_use]
    #[inline]
    pub fn registry(&self) -> &PropertyRegistry {
        &self.registry
    }

    /// Mutably borrows the property registry.
    #[inline]
    pub fn registry_mut(&mut self) -> &mut PropertyRegistry {
        &mut self.registry
    }

    /// Returns the built-in property handles.
    #[must_use]
    #[inline]
    pub fn properties(&self) -> &BuiltInProperties {
        &self.properties
    }

    /// Returns the behavior registry. Application code can register its own
    /// behaviors for new element kinds.
    #[must_use]
    #[inline]
    pub fn behaviors(&self) -> &BehaviorRegistry {
        &self.behaviors
    }

    /// Mutably borrows the behavior registry.
    #[inline]
    pub fn behaviors_mut(&mut self) -> &mut BehaviorRegistry {
        &mut self.behaviors
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
        match self.slots.get(id.slot() as usize)? {
            Slot::Live {
                generation,
                element,
            } if *generation == id.generation() => Some(element.as_ref()),
            _ => None,
        }
    }

    /// Returns a mutable reference to the element with the given id, or `None`
    /// if the id is stale.
    #[must_use]
    pub fn get_mut(&mut self, id: ElementId) -> Option<&mut Element> {
        match self.slots.get_mut(id.slot() as usize)? {
            Slot::Live {
                generation,
                element,
            } if *generation == id.generation() => Some(element.as_mut()),
            _ => None,
        }
    }

    /// Inserts a new child of the given `kind` under `parent` and returns its id.
    ///
    /// The new element is marked PAINT-dirty so the next [`Self::paint`] will
    /// record it.
    ///
    /// # Panics
    ///
    /// Panics if `parent` is a stale id.
    pub fn insert_child(&mut self, parent: ElementId, kind: ElementKind) -> ElementId {
        assert!(
            self.get(parent).is_some(),
            "insert_child: parent id is stale"
        );
        let child = self.alloc_element(kind, Some(parent));
        // SAFETY of unwrap: we asserted `parent` is live above and `alloc_element`
        // only mutates the child slot.
        self.get_mut(parent).unwrap().children_mut().push(child);
        self.invalidation.mark(child, PAINT);
        self.invalidation.mark(parent, COMPOSE);
        child
    }

    /// Removes `id` and all of its descendants from the tree.
    ///
    /// Each removed slot's generation is incremented so subsequent reuse
    /// invalidates outstanding handles. The parent (if any) is marked
    /// `COMPOSE`-dirty so the next frame re-assembles without the removed
    /// subtree.
    ///
    /// # Panics
    ///
    /// Panics if `id` is the root (the root cannot be removed) or if `id` is
    /// stale.
    pub fn remove(&mut self, id: ElementId) {
        assert!(id != self.root, "remove: cannot remove the tree root");
        assert!(self.get(id).is_some(), "remove: id is stale");

        // Detach from parent's children list and dirty the parent for compose.
        let parent = self.get(id).and_then(Element::parent);
        if let Some(parent) = parent {
            if let Some(parent_node) = self.get_mut(parent) {
                parent_node.children_mut().retain(|c| *c != id);
            }
            self.invalidation.mark(parent, COMPOSE);
        }
        self.remove_subtree(id);
    }

    /// Writes a property value with notification.
    ///
    /// Calls [`DependencyObjectExt::set_local_notifying`] and unions the
    /// returned channel set into the invalidation tracker for `id`. This is
    /// the blessed write path; direct mutation through [`Self::get_mut`] is
    /// available but apps must mark dirty themselves.
    pub fn set_local<T: Clone + PartialEq + 'static>(
        &mut self,
        id: ElementId,
        property: Property<T>,
        value: T,
    ) {
        let channels = {
            let registry = &self.registry;
            let Some(element) = element_mut(&mut self.slots, id) else {
                return;
            };
            element.set_local_notifying(property, value, registry)
        };
        self.mark_channels(id, channels);
    }

    /// Marks every channel in `set` dirty for `id` in the tracker.
    pub fn mark_channels(&mut self, id: ElementId, set: ChannelSet) {
        for channel in set.iter() {
            self.invalidation.mark(id, channel);
        }
    }

    /// Returns the invalidation tracker for diagnostics or custom drainers.
    #[must_use]
    #[inline]
    pub fn invalidation(&self) -> &InvalidationTracker<ElementId> {
        &self.invalidation
    }

    /// Paints the dirty elements into per-element scene caches, then
    /// assembles every element's cached scene into `output` in document
    /// (depth-first preorder) order.
    pub fn paint(&mut self, output: &mut Scene) {
        // 1. Re-record any element marked PAINT-dirty.
        let dirty: Vec<ElementId> = self.invalidation.drain_sorted(PAINT).collect();
        for id in dirty {
            self.repaint_element(id);
        }
        // 2. Walk the tree depth-first and append each cached scene into output.
        output.clear();
        self.assemble_into(self.root, output);
        // 3. Clear the COMPOSE channel; the assembly above is the response.
        self.invalidation.clear(COMPOSE);
    }

    fn repaint_element(&mut self, id: ElementId) {
        let Some(element) = self.get(id) else {
            // Stale; drop any cached scene.
            self.scene_cache.remove(&id);
            return;
        };
        let Some(behavior) = self.behaviors.get(element.kind()) else {
            // No behavior registered for this kind; clear any stale cache.
            self.scene_cache.remove(&id);
            return;
        };
        let ctx = PaintContext {
            registry: &self.registry,
            properties: &self.properties,
        };
        let mut scene = Scene::new();
        behavior.paint(element, &ctx, &mut scene);
        if scene.is_empty() {
            self.scene_cache.remove(&id);
        } else {
            self.scene_cache.insert(id, scene);
        }
    }

    fn assemble_into(&self, id: ElementId, output: &mut Scene) {
        let Some(element) = self.get(id) else {
            return;
        };
        if let Some(cached) = self.scene_cache.get(&id) {
            output.append_transformed(cached, Affine::IDENTITY);
        }
        for &child in element.children() {
            self.assemble_into(child, output);
        }
    }

    fn remove_subtree(&mut self, id: ElementId) {
        let children: Vec<ElementId> = self
            .get(id)
            .map(|e| e.children().to_vec())
            .unwrap_or_default();
        for child in children {
            self.remove_subtree(child);
        }
        if let Some(Slot::Live { generation, .. }) = self.slots.get(id.slot() as usize) {
            let next_generation = generation.wrapping_add(1);
            self.slots[id.slot() as usize] = Slot::Vacant {
                generation: next_generation,
            };
            self.free.push(id.slot());
        }
        self.scene_cache.remove(&id);
        self.invalidation.remove_key(id);
    }

    fn alloc_element(&mut self, kind: ElementKind, parent: Option<ElementId>) -> ElementId {
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
                element: Box::new(Element::new(id, kind, parent)),
            };
            id
        } else {
            let slot_index = u32::try_from(self.slots.len()).expect("Ui slot index overflowed u32");
            let id = ElementId::new(slot_index, 0);
            self.slots.push(Slot::Live {
                generation: 0,
                element: Box::new(Element::new(id, kind, parent)),
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

/// Direct slot accessor used by `set_local` so we can re-borrow the registry
/// for the notifying call without conflicting with the slot vec borrow. Not
/// exposed publicly — callers go through [`Ui::get_mut`].
fn element_mut(slots: &mut [Slot], id: ElementId) -> Option<&mut Element> {
    match slots.get_mut(id.slot() as usize)? {
        Slot::Live {
            generation,
            element,
        } if *generation == id.generation() => Some(element.as_mut()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Rect;
    use peniko::{Brush, Color};

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
    fn insert_child_appends_to_parent_and_marks_dirty() {
        let mut ui = Ui::new();
        let root = ui.root();
        let a = ui.insert_child(root, ELEMENT_KIND_GENERIC);
        let b = ui.insert_child(root, ELEMENT_KIND_GENERIC);

        assert_eq!(ui.len(), 3);
        let root_children = ui.get(root).unwrap().children();
        assert_eq!(root_children, &[a, b]);
        assert_eq!(ui.get(a).unwrap().parent(), Some(root));
        assert_eq!(ui.get(b).unwrap().parent(), Some(root));

        // Both children should be PAINT-dirty for their first frame.
        assert!(ui.invalidation().is_invalidated(a, PAINT));
        assert!(ui.invalidation().is_invalidated(b, PAINT));
        // The parent gets a COMPOSE dirty.
        assert!(ui.invalidation().is_invalidated(root, COMPOSE));
    }

    #[test]
    fn remove_drops_subtree_and_invalidates_handles() {
        let mut ui = Ui::new();
        let root = ui.root();
        let a = ui.insert_child(root, ELEMENT_KIND_GENERIC);
        let grandchild = ui.insert_child(a, ELEMENT_KIND_GENERIC);
        let b = ui.insert_child(root, ELEMENT_KIND_GENERIC);

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
        let a = ui.insert_child(root, ELEMENT_KIND_GENERIC);
        ui.remove(a);

        let c = ui.insert_child(root, ELEMENT_KIND_GENERIC);
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
        let a = ui.insert_child(root, ELEMENT_KIND_GENERIC);
        ui.remove(a);
        let _ = ui.insert_child(a, ELEMENT_KIND_GENERIC);
    }

    #[test]
    fn set_local_marks_paint_dirty() {
        let mut ui = Ui::new();
        let root = ui.root();
        let bounds = ui.properties().bounds;
        let background = ui.properties().background;

        // Drain the initial dirty marks from insert.
        let _ = ui
            .invalidation
            .drain_sorted(PAINT)
            .collect::<Vec<ElementId>>();

        ui.set_local(root, bounds, Rect::new(0.0, 0.0, 100.0, 50.0));
        assert!(ui.invalidation().is_invalidated(root, PAINT));

        // Subsequent identical write should be a no-op (effective unchanged).
        ui.invalidation.clear(PAINT);
        ui.set_local(root, bounds, Rect::new(0.0, 0.0, 100.0, 50.0));
        assert!(!ui.invalidation().is_invalidated(root, PAINT));

        // Background write dirties.
        ui.set_local(
            root,
            background,
            Some(Brush::Solid(Color::from_rgba8(0, 0, 0, 0xff))),
        );
        assert!(ui.invalidation().is_invalidated(root, PAINT));
    }

    #[test]
    fn paint_drains_dirty_and_records_into_cache() {
        let mut ui = Ui::new();
        let root = ui.root();
        let bounds = ui.properties().bounds;
        let background = ui.properties().background;

        ui.set_local(root, bounds, Rect::new(0.0, 0.0, 10.0, 10.0));
        ui.set_local(
            root,
            background,
            Some(Brush::Solid(Color::from_rgba8(0xff, 0, 0, 0xff))),
        );

        let mut output = Scene::new();
        ui.paint(&mut output);

        // After paint, the PAINT channel is drained.
        assert!(!ui.invalidation().has_invalidated(PAINT));
        // The output scene should contain at least one command (the fill).
        assert!(!output.is_empty());
    }

    #[test]
    fn paint_skips_zero_area_elements() {
        let mut ui = Ui::new();
        let root = ui.root();
        let background = ui.properties().background;

        // No bounds set → defaults to Rect::ZERO → behavior bails early.
        ui.set_local(
            root,
            background,
            Some(Brush::Solid(Color::from_rgba8(0xff, 0, 0, 0xff))),
        );

        let mut output = Scene::new();
        ui.paint(&mut output);
        assert!(output.is_empty(), "zero-area element should record nothing");
    }
}
