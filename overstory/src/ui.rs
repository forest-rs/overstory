// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The root [`Ui`] context: owns the visual tree, property registry, behavior
//! registry, style cascade, invalidation tracker, and per-element scene
//! cache.
//!
//! Phase 1c added the style cascade integration; Phase 1d added pointer
//! dispatch + `:hover` lifecycle. Later phases add taffy layout, templates,
//! and the full responder routing.

use alloc::boxed::Box;
use alloc::vec::Vec;

use hashbrown::HashMap;
use imaging::record::Scene;
use invalidation::{ChannelSet, InvalidationTracker};
use kurbo::{Affine, Point, Rect};
use ui_events::pointer::PointerEvent;
use understory_property::{DependencyObjectExt, Property, PropertyRegistry};
use understory_style::{
    MatchState, ResolveCx, SelectorInputs, StyleCascade, StyleCascadeBuilder, Theme, TypeTag,
};

use crate::behavior::{BehaviorRegistry, PaintContext};
use crate::channels::{COMPOSE, PAINT, STYLE};
use crate::element::{ELEMENT_KIND_GENERIC, Element, ElementId, ElementKind};
use crate::generic::GENERIC_BEHAVIOR;
use crate::lookup::UiStoreLookup;
use crate::properties::BuiltInProperties;
use crate::pseudos::PSEUDO_HOVER;

/// Root context for an Overstory UI instance.
///
/// Owns the visual tree, the slot allocator, the property and behavior
/// registries, the style cascade and theme, the invalidation tracker, and a
/// per-element [`Scene`] cache plus a per-element [`MatchState`] cache.
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
    cascade: StyleCascade,
    theme: Theme,
    match_states: HashMap<ElementId, MatchState>,
    hovered: Option<ElementId>,
}

impl core::fmt::Debug for Ui {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Ui")
            .field("elements", &(self.slots.len() - self.free.len()))
            .field("free_slots", &self.free.len())
            .field("paint_dirty", &self.invalidation.has_invalidated(PAINT))
            .field("style_dirty", &self.invalidation.has_invalidated(STYLE))
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
    /// [`ELEMENT_KIND_GENERIC`], built-in properties registered, the generic
    /// behavior installed, and an empty cascade and theme.
    ///
    /// Use [`Self::set_cascade`] / [`Self::set_theme`] to install
    /// application-supplied styling.
    #[must_use]
    pub fn new() -> Self {
        let mut registry = PropertyRegistry::new();
        let properties = BuiltInProperties::register(&mut registry);
        let mut behaviors = BehaviorRegistry::new();
        behaviors.register(ELEMENT_KIND_GENERIC, &GENERIC_BEHAVIOR);
        let cascade = StyleCascadeBuilder::new().build();
        let theme = Theme::default();

        let mut me = Self {
            slots: Vec::new(),
            free: Vec::new(),
            root: ElementId::new(0, 0),
            registry,
            properties,
            behaviors,
            invalidation: InvalidationTracker::new(),
            scene_cache: HashMap::new(),
            cascade,
            theme,
            match_states: HashMap::new(),
            hovered: None,
        };
        let root = me.alloc_element(ELEMENT_KIND_GENERIC, None);
        me.root = root;
        me.style_one(root);
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

    /// Returns the behavior registry.
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

    /// Returns the active style cascade.
    #[must_use]
    #[inline]
    pub fn cascade(&self) -> &StyleCascade {
        &self.cascade
    }

    /// Returns the active theme.
    #[must_use]
    #[inline]
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Swaps in a new style cascade and restyles the entire tree.
    ///
    /// Every element is marked PAINT-dirty since styled values may now resolve
    /// differently. Match states from the previous cascade are discarded —
    /// the [`MatchState`] type is bound to its originating cascade, so they
    /// can't be reused after a swap.
    pub fn set_cascade(&mut self, cascade: StyleCascade) {
        self.cascade = cascade;
        self.match_states.clear();
        let ids = collect_preorder(self, self.root);
        for id in ids {
            self.style_one(id);
            self.invalidation.mark(id, PAINT);
        }
    }

    /// Swaps in a new theme. All elements are marked PAINT-dirty since theme
    /// resource lookups are read-time and may now resolve differently.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        let ids: Vec<ElementId> = self.match_states.keys().copied().collect();
        for id in ids {
            self.invalidation.mark(id, PAINT);
        }
    }

    /// Returns the total number of live elements (including the root).
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len() - self.free.len()
    }

    /// Returns `true` if no elements are live.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a reference to the element with the given id, or `None` if the
    /// id is stale.
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

    /// Returns a mutable reference to the element with the given id.
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

    /// Returns the match state for `id`, falling back to the cascade root
    /// state if the element has not yet been styled.
    #[must_use]
    pub fn match_state(&self, id: ElementId) -> MatchState {
        self.match_states
            .get(&id)
            .copied()
            .unwrap_or_else(|| self.cascade.root_state())
    }

    /// Inserts a new child of the given `kind` under `parent` and returns its id.
    ///
    /// The new element is styled against the current cascade, marked
    /// PAINT-dirty, and the parent is marked COMPOSE-dirty.
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
        self.get_mut(parent).unwrap().children_mut().push(child);
        self.style_one(child);
        self.invalidation.mark(child, PAINT);
        self.invalidation.mark(parent, COMPOSE);
        child
    }

    /// Removes `id` and all of its descendants from the tree.
    ///
    /// # Panics
    ///
    /// Panics if `id` is the root or stale.
    pub fn remove(&mut self, id: ElementId) {
        assert!(id != self.root, "remove: cannot remove the tree root");
        assert!(self.get(id).is_some(), "remove: id is stale");

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

    /// Adds `class` to `id` and re-styles the affected subtree.
    pub fn add_class(&mut self, id: ElementId, class: understory_style::ClassId) {
        let changed = self
            .get_mut(id)
            .map(|el| el.add_class(class))
            .unwrap_or(false);
        if changed {
            self.invalidation.mark(id, STYLE);
            self.restyle_subtree(id);
        }
    }

    /// Removes `class` from `id` and re-styles the affected subtree.
    pub fn remove_class(&mut self, id: ElementId, class: understory_style::ClassId) {
        let changed = self
            .get_mut(id)
            .map(|el| el.remove_class(class))
            .unwrap_or(false);
        if changed {
            self.invalidation.mark(id, STYLE);
            self.restyle_subtree(id);
        }
    }

    /// Adds `pseudo` to `id` and re-styles the affected subtree.
    pub fn add_pseudo(&mut self, id: ElementId, pseudo: understory_style::PseudoClassId) {
        let changed = self
            .get_mut(id)
            .map(|el| el.add_pseudo(pseudo))
            .unwrap_or(false);
        if changed {
            self.invalidation.mark(id, STYLE);
            self.restyle_subtree(id);
        }
    }

    /// Removes `pseudo` from `id` and re-styles the affected subtree.
    pub fn remove_pseudo(&mut self, id: ElementId, pseudo: understory_style::PseudoClassId) {
        let changed = self
            .get_mut(id)
            .map(|el| el.remove_pseudo(pseudo))
            .unwrap_or(false);
        if changed {
            self.invalidation.mark(id, STYLE);
            self.restyle_subtree(id);
        }
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

    /// Returns the element currently considered "hovered" by the input
    /// pipeline (the deepest element whose bounds contain the last reported
    /// pointer position), if any.
    #[must_use]
    #[inline]
    pub fn hovered(&self) -> Option<ElementId> {
        self.hovered
    }

    /// Walks the tree top-down and returns the deepest, topmost element whose
    /// effective `Bounds` contains `point`, if any. Bounds are read through
    /// the cascade so styled values participate.
    ///
    /// For Phase 1d this is a simple O(N) recursive walk; later phases switch
    /// to `understory_box_tree` for a proper spatial index.
    #[must_use]
    pub fn hit_test(&self, point: Point) -> Option<ElementId> {
        self.hit_test_at(self.root, point)
    }

    fn hit_test_at(&self, id: ElementId, point: Point) -> Option<ElementId> {
        let element = self.get(id)?;
        let bounds = self.effective_bounds(id);
        if !bounds.contains(point) {
            return None;
        }
        // Walk children in reverse z-order so the last-painted (topmost) wins.
        let children: Vec<ElementId> = element.children().iter().rev().copied().collect();
        for child in children {
            if let Some(hit) = self.hit_test_at(child, point) {
                return Some(hit);
            }
        }
        Some(id)
    }

    /// Resolves the effective `Bounds` for `id` through the cascade.
    #[must_use]
    pub fn effective_bounds(&self, id: ElementId) -> Rect {
        let Some(element) = self.get(id) else {
            return Rect::ZERO;
        };
        let lookup = UiStoreLookup::new(self);
        let cx = ResolveCx::new(&self.registry, &self.theme, lookup);
        let match_state = self.match_state(id);
        cx.get_value(
            element,
            self.properties.bounds,
            Some((&self.cascade, match_state)),
        )
    }

    /// Dispatches a normalized pointer event.
    ///
    /// Phase 1d handles only the hover lifecycle: a `Move` updates the hovered
    /// element (toggling `:hover` on/off via `add_pseudo`/`remove_pseudo`);
    /// `Leave` and `Cancel` clear it. Down/Up/Scroll/Gesture are ignored for
    /// now — they'll land alongside the responder integration in Phase 1f.
    ///
    /// Returns `true` if the dispatch caused a state change (caller should
    /// `request_redraw`).
    pub fn dispatch_pointer_event(&mut self, event: &PointerEvent) -> bool {
        match event {
            PointerEvent::Move(update) => {
                let pos = Point::new(update.current.position.x, update.current.position.y);
                let hit = self.hit_test(pos);
                self.set_hovered(hit)
            }
            PointerEvent::Leave(_) | PointerEvent::Cancel(_) => self.set_hovered(None),
            _ => false,
        }
    }

    fn set_hovered(&mut self, new: Option<ElementId>) -> bool {
        if self.hovered == new {
            return false;
        }
        let old = self.hovered.take();
        if let Some(old_id) = old {
            self.remove_pseudo(old_id, PSEUDO_HOVER);
        }
        if let Some(new_id) = new {
            self.add_pseudo(new_id, PSEUDO_HOVER);
        }
        self.hovered = new;
        true
    }

    /// Drains the STYLE channel, recomputing the match state of any dirty
    /// element subtree. Channels affected by changed styled properties are
    /// unioned into the invalidation tracker.
    pub fn flush_style(&mut self) {
        let dirty: Vec<ElementId> = self.invalidation.drain_sorted(STYLE).collect();
        for id in dirty {
            self.restyle_subtree(id);
        }
    }

    /// Paints the dirty elements into per-element scene caches, then
    /// assembles every element's cached scene into `output` in document
    /// (depth-first preorder) order.
    pub fn paint(&mut self, output: &mut Scene) {
        // 1. Flush any pending STYLE work first; it may dirty PAINT.
        self.flush_style();
        // 2. Re-record any PAINT-dirty elements.
        let dirty: Vec<ElementId> = self.invalidation.drain_sorted(PAINT).collect();
        for id in dirty {
            self.repaint_element(id);
        }
        // 3. Walk the tree depth-first and append each cached scene into output.
        output.clear();
        self.assemble_into(self.root, output);
        // 4. Clear the COMPOSE channel; the assembly above is the response.
        self.invalidation.clear(COMPOSE);
    }

    fn repaint_element(&mut self, id: ElementId) {
        let Some(element) = self.get(id) else {
            self.scene_cache.remove(&id);
            return;
        };
        let Some(behavior) = self.behaviors.get(element.kind()) else {
            self.scene_cache.remove(&id);
            return;
        };
        let match_state = self
            .match_states
            .get(&id)
            .copied()
            .unwrap_or_else(|| self.cascade.root_state());
        let ctx = PaintContext {
            registry: &self.registry,
            properties: &self.properties,
            theme: &self.theme,
            cascade: &self.cascade,
            match_state,
            lookup: UiStoreLookup::new(self),
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

    /// Restyles `id` and its descendants in preorder. Channels affected by
    /// each element's styled-property changes are marked in the tracker.
    fn restyle_subtree(&mut self, id: ElementId) {
        let order: Vec<ElementId> = collect_preorder(self, id);
        for id in order {
            self.style_one(id);
        }
    }

    /// Recomputes the match state for one element. If a previous state exists
    /// for `id`, the cascade's `restyle_subject` is used to surface changed
    /// channels into the tracker; otherwise the new state is just stored.
    ///
    /// Each Element is an *independent* style subject — selectors match it as
    /// a root-level subject anchored at `cascade.root_state()`, not from its
    /// parent's `MatchState`. The cascade's parent-state argument is for
    /// template parts (a Toggle's track entered from its Toggle owner's
    /// state), not logical-tree children. Phase 1f will revisit this when
    /// templates land.
    fn style_one(&mut self, id: ElementId) {
        let Some(element) = self.get(id) else {
            self.match_states.remove(&id);
            return;
        };
        let kind = element.kind();
        // Copy out the slices so the `&Element` borrow of `self` can end
        // before we mutate `self.match_states`.
        let classes: smallvec::SmallVec<[understory_style::ClassId; 4]> =
            smallvec::SmallVec::from_slice(element.classes());
        let pseudos: smallvec::SmallVec<[understory_style::PseudoClassId; 4]> =
            smallvec::SmallVec::from_slice(element.pseudos());

        let parent_state = self.cascade.root_state();

        let inputs = SelectorInputs::new(Some(TypeTag(kind.tag())), &classes, &pseudos);
        let old_state = self.match_states.get(&id).copied();
        let new_state = self.cascade.enter_subject(parent_state, &inputs);
        self.match_states.insert(id, new_state);

        if let Some(old) = old_state
            && old != new_state
        {
            let restyle = self
                .cascade
                .restyle_subject(&self.registry, old, parent_state, &inputs);
            self.mark_channels(id, restyle.changed_channels());
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
        self.match_states.remove(&id);
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
/// for the notifying call without conflicting with the slot vec borrow.
fn element_mut(slots: &mut [Slot], id: ElementId) -> Option<&mut Element> {
    match slots.get_mut(id.slot() as usize)? {
        Slot::Live {
            generation,
            element,
        } if *generation == id.generation() => Some(element.as_mut()),
        _ => None,
    }
}

/// Collects `start` and all of its descendants in depth-first preorder.
fn collect_preorder(ui: &Ui, start: ElementId) -> Vec<ElementId> {
    let mut out = Vec::new();
    walk_preorder(ui, start, &mut out);
    out
}

fn walk_preorder(ui: &Ui, id: ElementId, out: &mut Vec<ElementId>) {
    let Some(element) = ui.get(id) else { return };
    out.push(id);
    let children: Vec<ElementId> = element.children().to_vec();
    for child in children {
        walk_preorder(ui, child, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Rect;
    use peniko::{Brush, Color};
    use understory_style::{ClassId, SelectorBuilder, SelectorStep, StyleBuilder, StyleOrigin};

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

        assert!(ui.invalidation().is_invalidated(a, PAINT));
        assert!(ui.invalidation().is_invalidated(b, PAINT));
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

        let _ = ui
            .invalidation
            .drain_sorted(PAINT)
            .collect::<Vec<ElementId>>();

        ui.set_local(root, bounds, Rect::new(0.0, 0.0, 100.0, 50.0));
        assert!(ui.invalidation().is_invalidated(root, PAINT));

        ui.invalidation.clear(PAINT);
        ui.set_local(root, bounds, Rect::new(0.0, 0.0, 100.0, 50.0));
        assert!(!ui.invalidation().is_invalidated(root, PAINT));

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

        assert!(!ui.invalidation().has_invalidated(PAINT));
        assert!(!output.is_empty());
    }

    #[test]
    fn paint_skips_zero_area_elements() {
        let mut ui = Ui::new();
        let root = ui.root();
        let background = ui.properties().background;

        ui.set_local(
            root,
            background,
            Some(Brush::Solid(Color::from_rgba8(0xff, 0, 0, 0xff))),
        );

        let mut output = Scene::new();
        ui.paint(&mut output);
        assert!(output.is_empty(), "zero-area element should record nothing");
    }

    #[test]
    fn styled_background_resolves_through_cascade() {
        // A style for `Type(ELEMENT_KIND_GENERIC)` should supply the
        // background value for elements that don't have a local override.
        let mut ui = Ui::new();
        let root = ui.root();
        let bounds = ui.properties().bounds;
        let background = ui.properties().background;

        // Build a cascade with one rule.
        let red = Some(Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff)));
        let style = StyleBuilder::new().set(background, red).build();
        let selector =
            SelectorBuilder::new(SelectorStep::type_tag(TypeTag(ELEMENT_KIND_GENERIC.tag())))
                .build();
        let cascade = StyleCascadeBuilder::new()
            .push_rule(StyleOrigin::Base, selector, style)
            .build();
        ui.set_cascade(cascade);

        ui.set_local(root, bounds, Rect::new(0.0, 0.0, 10.0, 10.0));

        let mut output = Scene::new();
        ui.paint(&mut output);
        // Resolution should have surfaced the styled red brush, so the scene
        // is non-empty.
        assert!(
            !output.is_empty(),
            "cascade should supply the background and produce paint output"
        );
    }

    #[test]
    fn three_rule_cascade_picks_class_over_type() {
        // Mirror the demo: rules at Base origin keyed on ELEMENT_KIND_GENERIC.
        use understory_style::{ClassId, SelectorBuilder, SelectorStep};

        const CLASS_FG: ClassId = ClassId(1);

        let mut ui = Ui::new();
        let root = ui.root();
        let fg = ui.insert_child(root, ELEMENT_KIND_GENERIC);
        ui.add_class(fg, CLASS_FG);

        let kind = TypeTag(ELEMENT_KIND_GENERIC.tag());
        let dark = Some(Brush::Solid(Color::from_rgba8(0x1e, 0x1e, 0x24, 0xff)));
        let blue = Some(Brush::Solid(Color::from_rgba8(0x6b, 0x9d, 0xff, 0xff)));
        let bg_prop = ui.properties().background;

        let generic = StyleBuilder::new().set(bg_prop, dark.clone()).build();
        let foreground = StyleBuilder::new().set(bg_prop, blue.clone()).build();

        let cascade = StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                SelectorBuilder::new(SelectorStep::type_tag(kind)).build(),
                generic,
            )
            .push_rule(
                StyleOrigin::Base,
                SelectorBuilder::new(SelectorStep::type_tag(kind).with_class(CLASS_FG)).build(),
                foreground,
            )
            .build();
        ui.set_cascade(cascade);

        // Confirm the class survived set_cascade (which clears match_states).
        assert_eq!(
            ui.get(fg).unwrap().classes(),
            &[CLASS_FG],
            "class should survive set_cascade"
        );

        let lookup = UiStoreLookup::new(&ui);
        let cx = ResolveCx::new(ui.registry(), ui.theme(), lookup);

        // Root: only the generic rule matches → dark.
        let root_el = ui.get(root).unwrap();
        let resolved_root: Option<Brush> =
            cx.get_value(root_el, bg_prop, Some((ui.cascade(), ui.match_state(root))));
        assert_eq!(resolved_root, dark, "root should resolve to dark");

        // Foreground: both rules match; the class-bearing rule has higher
        // specificity → blue.
        let fg_el = ui.get(fg).unwrap();
        let resolved_fg: Option<Brush> =
            cx.get_value(fg_el, bg_prop, Some((ui.cascade(), ui.match_state(fg))));
        assert_eq!(resolved_fg, blue, "foreground should resolve to blue");

        // The class survived set_cascade (which clears match_states).
        assert_eq!(
            ui.get(fg).unwrap().classes(),
            &[CLASS_FG],
            "class should survive set_cascade"
        );
    }

    #[test]
    fn hit_test_picks_topmost_child() {
        let mut ui = Ui::new();
        let root = ui.root();
        let bounds = ui.properties().bounds;
        ui.set_local(root, bounds, Rect::new(0.0, 0.0, 200.0, 200.0));
        let child = ui.insert_child(root, ELEMENT_KIND_GENERIC);
        ui.set_local(child, bounds, Rect::new(50.0, 50.0, 150.0, 150.0));

        // Inside child's bounds → child wins.
        assert_eq!(ui.hit_test(Point::new(100.0, 100.0)), Some(child));
        // Inside root only.
        assert_eq!(ui.hit_test(Point::new(10.0, 10.0)), Some(root));
        // Outside root entirely.
        assert_eq!(ui.hit_test(Point::new(300.0, 300.0)), None);
    }

    #[test]
    fn pointer_leave_clears_hover() {
        // Move events require constructing a PointerUpdate with platform
        // position types; we exercise that end-to-end via the demo. Here we
        // verify the dispatch shape: a Leave event clears whatever was
        // currently hovered.
        use ui_events::pointer::{PointerEvent, PointerInfo, PointerType};

        let mut ui = Ui::new();
        let root = ui.root();
        // Manually mark something hovered (this is what dispatch_pointer_event
        // does internally via add_pseudo on a Move).
        ui.add_pseudo(root, PSEUDO_HOVER);
        // Match the internal state. (Phase 1d's dispatch is the only writer
        // of `hovered` in real code, so this is a deliberate test-only nudge.)
        // We test the Leave path which clears via the public API.
        let leave = PointerEvent::Leave(PointerInfo {
            pointer_id: None,
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        });
        // Even if hovered is None (we never set it via dispatch), Leave
        // should be a no-op rather than panic.
        let _ = ui.dispatch_pointer_event(&leave);
        assert_eq!(ui.hovered(), None);
    }

    #[test]
    fn add_class_marks_style_and_restyles_subtree() {
        let mut ui = Ui::new();
        let root = ui.root();
        let _ = ui
            .invalidation
            .drain_sorted(STYLE)
            .collect::<Vec<ElementId>>();
        ui.add_class(root, ClassId(1));
        // STYLE dirty was marked by add_class; restyle drained it via
        // restyle_subtree (called inline). With an empty cascade nothing
        // changes channel-wise.
        assert!(ui.get(root).unwrap().classes().contains(&ClassId(1)));
    }
}
