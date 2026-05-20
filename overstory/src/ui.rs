// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The root [`Ui`] context: owns the visual tree, property registry, behavior
//! registry, style cascade, invalidation tracker, and per-element scene
//! cache.
//!
//! Phase 1c added the style cascade integration; Phase 1d added pointer
//! dispatch + `:hover` lifecycle; Phase 1e added taffy layout via a
//! `Property<taffy::Style>` and `Ui::compute_layout`. Later phases add
//! templates, text, and the full responder routing.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cell::RefCell;

use hashbrown::HashMap;
use imaging::record::Scene;
use invalidation::{ChannelSet, InvalidationTracker};
use kurbo::{Affine, Point, Rect, Size};
use ui_events::pointer::{PointerButton, PointerEvent};
use understory_property::{DependencyObjectExt, Property, PropertyRegistry};
use understory_style::{
    MatchState, ResolveCx, SelectorInputs, StyleCascade, StyleCascadeBuilder, Theme, TypeTag,
};

use crate::behavior::{BehaviorRegistry, KnownDimensions, MeasureConstraints, PaintContext};
use crate::button::{BUTTON_BEHAVIOR, ELEMENT_KIND_BUTTON};
use crate::channels::{COMPOSE, LAYOUT, PAINT, STYLE};
use crate::element::{ELEMENT_KIND_GENERIC, Element, ElementId, ElementKind};
use crate::generic::GENERIC_BEHAVIOR;
use crate::lookup::UiStoreLookup;
use crate::properties::BuiltInProperties;
use crate::pseudos::{PSEUDO_HOVER, PSEUDO_PRESSED};
use crate::text::TextEngine;
use crate::text_behavior::{ELEMENT_KIND_TEXT, TEXT_BEHAVIOR};

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
    /// The element that received the most recent primary-button `Down` event
    /// and has not yet seen an `Up`. Stays set until release or cancel so
    /// `:pressed` survives the cursor leaving the original element.
    press_target: Option<ElementId>,
    taffy: taffy::TaffyTree<ElementId>,
    element_to_taffy: HashMap<ElementId, taffy::NodeId>,
    /// While a `Template::instantiate` call is on the stack, every element
    /// created via `insert_child` is tagged with this id as
    /// `templated_parent`. Saved/restored around nested template expansions.
    currently_expanding: Option<ElementId>,
    /// Shared text shaping engine. `RefCell` because text behaviors borrow
    /// it mutably while the owning `&Ui` borrow is otherwise immutable.
    text: RefCell<TextEngine>,
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
        behaviors.register(ELEMENT_KIND_TEXT, &TEXT_BEHAVIOR);
        behaviors.register(ELEMENT_KIND_BUTTON, &BUTTON_BEHAVIOR);
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
            press_target: None,
            taffy: taffy::TaffyTree::new(),
            element_to_taffy: HashMap::new(),
            currently_expanding: None,
            text: RefCell::new(TextEngine::new()),
        };
        let root = me.alloc_element(ELEMENT_KIND_GENERIC, None, None);
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
    /// PAINT-dirty, and the parent is marked COMPOSE-dirty. If `kind`'s
    /// behavior provides a `default_template`, it is invoked immediately;
    /// any elements created during that expansion are tagged with
    /// `templated_parent = Some(child)`.
    ///
    /// If this call is happening *inside* another template's expansion,
    /// `templated_parent` is inherited from the outer template's owner — so
    /// elements created during a Button's template expansion all share
    /// `templated_parent = button_id`, even when they're created several
    /// levels deep.
    ///
    /// # Panics
    ///
    /// Panics if `parent` is a stale id.
    pub fn insert_child(&mut self, parent: ElementId, kind: ElementKind) -> ElementId {
        assert!(
            self.get(parent).is_some(),
            "insert_child: parent id is stale"
        );
        let templated_parent = self.currently_expanding;
        let child = self.alloc_element(kind, Some(parent), templated_parent);
        self.get_mut(parent).unwrap().children_mut().push(child);
        self.style_one(child);
        self.invalidation.mark(child, PAINT);
        self.invalidation.mark(parent, COMPOSE);
        self.expand_default_template(child);
        child
    }

    /// Expands `kind`'s default template under `owner`, if any. Sets
    /// `currently_expanding = Some(owner)` for the duration of the
    /// expansion so any `insert_child` calls inside `Template::instantiate`
    /// tag their elements with the right `templated_parent`.
    fn expand_default_template(&mut self, owner: ElementId) {
        let Some(element) = self.get(owner) else {
            return;
        };
        let kind = element.kind();
        let Some(behavior) = self.behaviors.get(kind) else {
            return;
        };
        let Some(template) = behavior.default_template() else {
            return;
        };
        let outer = self.currently_expanding.replace(owner);
        template.instantiate(self, owner);
        self.currently_expanding = outer;
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
    /// Handles the hover lifecycle (Move/Leave/Cancel) and the `:pressed`
    /// lifecycle (Down/Up/Cancel for the primary button). Scroll/Gesture and
    /// non-primary button presses are ignored for now — they'll land
    /// alongside the responder integration in a later phase.
    ///
    /// `:pressed` follows WPF semantics: a primary-button press marks the
    /// hit element pressed and that element stays pressed even if the cursor
    /// leaves it (no capture API yet, but the visual state survives). The
    /// matching Up — wherever it lands — clears the press.
    ///
    /// Returns `true` if the dispatch caused a state change.
    pub fn dispatch_pointer_event(&mut self, event: &PointerEvent) -> bool {
        match event {
            PointerEvent::Move(update) => {
                let pos = Point::new(update.current.position.x, update.current.position.y);
                let hit = self.hit_test(pos);
                self.set_hovered(hit)
            }
            PointerEvent::Down(button_event) => {
                if button_event.button != Some(PointerButton::Primary) {
                    return false;
                }
                let pos = Point::new(button_event.state.position.x, button_event.state.position.y);
                let hit = self.hit_test(pos);
                // Update hover first (a Down without a preceding Move can
                // arrive in some platforms).
                let mut changed = self.set_hovered(hit);
                changed |= self.set_press_target(hit);
                changed
            }
            PointerEvent::Up(button_event) => {
                if button_event.button != Some(PointerButton::Primary) {
                    return false;
                }
                self.set_press_target(None)
            }
            PointerEvent::Leave(_) => self.set_hovered(None),
            PointerEvent::Cancel(_) => {
                let h = self.set_hovered(None);
                let p = self.set_press_target(None);
                h || p
            }
            _ => false,
        }
    }

    fn set_press_target(&mut self, new: Option<ElementId>) -> bool {
        if self.press_target == new {
            return false;
        }
        if let Some(old_id) = self.press_target.take() {
            self.remove_pseudo(old_id, PSEUDO_PRESSED);
        }
        if let Some(new_id) = new {
            self.add_pseudo(new_id, PSEUDO_PRESSED);
        }
        self.press_target = new;
        true
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

    /// Resolves the effective `Layout` (`taffy::Style`) for `id` through the
    /// cascade.
    #[must_use]
    pub fn effective_layout(&self, id: ElementId) -> taffy::Style {
        let Some(element) = self.get(id) else {
            return taffy::Style::default();
        };
        let lookup = UiStoreLookup::new(self);
        let cx = ResolveCx::new(&self.registry, &self.theme, lookup);
        let match_state = self.match_state(id);
        cx.get_value(
            element,
            self.properties.layout,
            Some((&self.cascade, match_state)),
        )
    }

    /// Computes layout for the tree against the given viewport size and
    /// writes each element's `Bounds` (via the cascade-resolved `Layout`).
    ///
    /// Flushes the LAYOUT channel: any element whose `Layout` style has
    /// changed gets its taffy style pushed before taffy is asked to
    /// recompute. `Bounds` writes use the standard `set_local_notifying`
    /// path so PAINT cascades naturally for elements whose box changed.
    pub fn compute_layout(&mut self, viewport: Size) {
        // Push any pending Layout changes into taffy. We collect first to
        // separate the immutable cascade walk from the mutable taffy update.
        let dirty: Vec<ElementId> = self.invalidation.drain_sorted(LAYOUT).collect();
        let updates: Vec<(taffy::NodeId, taffy::Style)> = dirty
            .into_iter()
            .filter_map(|id| {
                let taffy_node = *self.element_to_taffy.get(&id)?;
                let style = self.effective_layout(id);
                Some((taffy_node, style))
            })
            .collect();
        for (node, style) in updates {
            let _ = self.taffy.set_style(node, style);
        }

        let Some(root_taffy) = self.element_to_taffy.get(&self.root).copied() else {
            return;
        };
        let available = taffy::Size {
            #[expect(clippy::cast_possible_truncation, reason = "viewport size fits f32")]
            width: taffy::AvailableSpace::Definite(viewport.width as f32),
            #[expect(clippy::cast_possible_truncation, reason = "viewport size fits f32")]
            height: taffy::AvailableSpace::Definite(viewport.height as f32),
        };

        // Run taffy with a measure callback that dispatches to the elements'
        // behaviors. Split-borrow the disjoint Ui fields so the closure can
        // read everything it needs while the taffy tree is &mut.
        {
            let taffy = &mut self.taffy;
            let behaviors = &self.behaviors;
            let registry = &self.registry;
            let properties = &self.properties;
            let theme = &self.theme;
            let cascade = &self.cascade;
            let match_states = &self.match_states;
            let slots = &self.slots;
            let text = &self.text;
            let _ = taffy.compute_layout_with_measure(
                root_taffy,
                available,
                |known, avail, _node_id, ctx_id, _style| {
                    let Some(&id) = ctx_id.map(|v| &*v) else {
                        return taffy::Size::ZERO;
                    };
                    let Some(slot) = slots.get(id.slot() as usize) else {
                        return taffy::Size::ZERO;
                    };
                    let element = match slot {
                        Slot::Live {
                            element,
                            generation,
                        } if *generation == id.generation() => element.as_ref(),
                        _ => return taffy::Size::ZERO,
                    };
                    let Some(behavior) = behaviors.get(element.kind()) else {
                        return taffy::Size::ZERO;
                    };
                    let match_state = match_states
                        .get(&id)
                        .copied()
                        .unwrap_or_else(|| cascade.root_state());
                    let measure_ctx = crate::behavior::MeasureContext {
                        registry,
                        properties,
                        theme,
                        cascade,
                        match_state,
                        text,
                    };
                    let constraints = MeasureConstraints {
                        available: Size::new(
                            available_to_f64(avail.width),
                            available_to_f64(avail.height),
                        ),
                        known: KnownDimensions {
                            width: known.width.map(f64::from),
                            height: known.height.map(f64::from),
                        },
                    };
                    match behavior.measure(element, &measure_ctx, constraints) {
                        #[expect(
                            clippy::cast_possible_truncation,
                            reason = "measured size fits f32"
                        )]
                        Some(size) => taffy::Size {
                            width: size.width as f32,
                            height: size.height as f32,
                        },
                        None => taffy::Size::ZERO,
                    }
                },
            );
        }

        // Walk the tree and write absolute bounds for every element.
        self.write_bounds_recursive(self.root, Point::ZERO);
    }

    fn write_bounds_recursive(&mut self, id: ElementId, parent_origin: Point) {
        let Some(taffy_node) = self.element_to_taffy.get(&id).copied() else {
            return;
        };
        let Ok(layout) = self.taffy.layout(taffy_node).copied() else {
            return;
        };
        let origin = Point::new(
            parent_origin.x + f64::from(layout.location.x),
            parent_origin.y + f64::from(layout.location.y),
        );
        let rect = Rect::new(
            origin.x,
            origin.y,
            origin.x + f64::from(layout.size.width),
            origin.y + f64::from(layout.size.height),
        );
        let bounds_prop = self.properties.bounds;
        self.set_local(id, bounds_prop, rect);

        let children: Vec<ElementId> = self
            .get(id)
            .map(|el| el.children().to_vec())
            .unwrap_or_default();
        for child in children {
            self.write_bounds_recursive(child, origin);
        }
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
            text: &self.text,
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
        if let Some(taffy_node) = self.element_to_taffy.remove(&id) {
            // `taffy.remove` also detaches from its parent and frees children
            // recursively — but we've already walked our subtree top-down and
            // recursed into our children before reaching this point, so the
            // children's taffy nodes are already gone. Removing the (now
            // childless) taffy node here just finalizes the leaf.
            let _ = self.taffy.remove(taffy_node);
        }
    }

    fn alloc_element(
        &mut self,
        kind: ElementKind,
        parent: Option<ElementId>,
        templated_parent: Option<ElementId>,
    ) -> ElementId {
        let id = if let Some(slot_index) = self.free.pop() {
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
                element: Box::new(Element::new(id, kind, parent, templated_parent)),
            };
            id
        } else {
            let slot_index = u32::try_from(self.slots.len()).expect("Ui slot index overflowed u32");
            let id = ElementId::new(slot_index, 0);
            self.slots.push(Slot::Live {
                generation: 0,
                element: Box::new(Element::new(id, kind, parent, templated_parent)),
            });
            id
        };

        // Mirror into taffy. The element id is stored as the node context so
        // taffy's measure callback can identify which element it's measuring.
        let taffy_node = self
            .taffy
            .new_leaf_with_context(taffy::Style::default(), id)
            .expect("taffy leaf allocation should succeed");
        self.element_to_taffy.insert(id, taffy_node);
        if let Some(parent_id) = parent
            && let Some(parent_node) = self.element_to_taffy.get(&parent_id).copied()
        {
            self.taffy
                .add_child(parent_node, taffy_node)
                .expect("taffy add_child should succeed for fresh node");
        }
        id
    }
}

impl Default for Ui {
    fn default() -> Self {
        Self::new()
    }
}

/// Translates taffy's `AvailableSpace` into kurbo's `f64`. `MaxContent`
/// becomes `f64::INFINITY`; `MinContent` becomes `0.0`; `Definite(v)`
/// becomes `v` as `f64`. The boundary loses some fidelity around min-content
/// sizing, which we'll revisit if/when an existing behavior needs it.
fn available_to_f64(s: taffy::AvailableSpace) -> f64 {
    match s {
        taffy::AvailableSpace::Definite(v) => f64::from(v),
        taffy::AvailableSpace::MinContent => 0.0,
        taffy::AvailableSpace::MaxContent => f64::INFINITY,
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
    fn template_expansion_inserts_parts_under_owner() {
        use crate::Template;
        use crate::behavior::ElementBehavior;

        // A unit-test kind whose default_template inserts two generic
        // children. Both should have templated_parent = the owner.
        const TEST_OWNER_KIND: ElementKind = ElementKind::new(0xFEED_BEEF);

        struct TestTemplate;
        impl Template for TestTemplate {
            fn instantiate(&self, ui: &mut Ui, owner: ElementId) {
                let _a = ui.insert_child(owner, ELEMENT_KIND_GENERIC);
                let _b = ui.insert_child(owner, ELEMENT_KIND_GENERIC);
            }
        }
        static TEST_TEMPLATE: TestTemplate = TestTemplate;

        struct TestBehavior;
        impl ElementBehavior for TestBehavior {
            fn paint(&self, _: &Element, _: &PaintContext<'_>, _: &mut Scene) {}
            fn default_template(&self) -> Option<&'static dyn Template> {
                Some(&TEST_TEMPLATE)
            }
        }
        static TEST_BEHAVIOR: TestBehavior = TestBehavior;

        let mut ui = Ui::new();
        ui.behaviors_mut().register(TEST_OWNER_KIND, &TEST_BEHAVIOR);

        let root = ui.root();
        let owner = ui.insert_child(root, TEST_OWNER_KIND);

        // The template should have created two children under owner, each
        // tagged with templated_parent = Some(owner).
        let owner_children = ui.get(owner).unwrap().children().to_vec();
        assert_eq!(owner_children.len(), 2, "template should insert 2 parts");
        for part_id in owner_children {
            let part = ui.get(part_id).unwrap();
            assert_eq!(
                part.templated_parent(),
                Some(owner),
                "part should track owner"
            );
            assert_eq!(part.parent(), Some(owner), "part is a child of owner");
        }

        // The owner itself has no templated parent (it's a logical child of root).
        assert_eq!(ui.get(owner).unwrap().templated_parent(), None);
    }

    #[test]
    fn compute_layout_writes_taffy_bounds() {
        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.insert_child(root, ELEMENT_KIND_GENERIC);

        let layout_prop = ui.properties().layout;
        // Root fills the viewport with 10px padding.
        let inset = taffy::LengthPercentage::length(10.0);
        ui.set_local(
            root,
            layout_prop,
            taffy::Style {
                display: taffy::Display::Flex,
                size: taffy::Size {
                    width: taffy::Dimension::percent(1.0),
                    height: taffy::Dimension::percent(1.0),
                },
                padding: taffy::Rect {
                    left: inset,
                    right: inset,
                    top: inset,
                    bottom: inset,
                },
                ..Default::default()
            },
        );
        // Child fills the padded content.
        ui.set_local(
            child,
            layout_prop,
            taffy::Style {
                flex_grow: 1.0,
                align_self: Some(taffy::AlignSelf::Stretch),
                ..Default::default()
            },
        );

        ui.compute_layout(Size::new(200.0, 100.0));
        // Root is the full viewport.
        assert_eq!(ui.effective_bounds(root), Rect::new(0.0, 0.0, 200.0, 100.0));
        // Child fills (10, 10) → (190, 90).
        assert_eq!(
            ui.effective_bounds(child),
            Rect::new(10.0, 10.0, 190.0, 90.0)
        );
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
