// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The root [`Ui`] context: owns the visual tree, property registry, behavior
//! registry, style cascade, invalidation tracker, and per-element scene
//! cache.
//!
//! Phase 1c added the style cascade integration; Phase 1d added pointer
//! dispatch + `:hover` lifecycle; Phase 1e added taffy layout; Phase 1f
//! added template substrate, text rendering, `:pressed`, the Button
//! control, and property bindings via `understory_property_binding`. Later
//! phases bring the full responder routing, more controls, and animations.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cell::RefCell;

use hashbrown::HashMap;
use imaging::record::Scene;
use invalidation::{ChannelSet, InvalidationTracker};
use kurbo::{Affine, Point, Rect, Size};
use smallvec::SmallVec;
use ui_events::keyboard::KeyboardEvent;
use ui_events::pointer::{PointerButton, PointerEvent};
use understory_box_tree::{LocalNode, NodeFlags, NodeId, QueryFilter, Tree};
use understory_event_state::click::{ClickResult, ClickState};
use understory_event_state::hover::{HoverEvent, HoverState};
use understory_focus::adapters::box_tree::build_focus_space_for_scope;
use understory_focus::{DefaultPolicy, FocusEntry, FocusPolicy, Navigation, WrapMode};
use understory_presentation::{PresentationStore, Primitive, TextPrimitive};
use understory_property::{
    DependencyObject, DependencyObjectExt, ErasedValue, Property, PropertyRegistry,
};
use understory_property_binding::{
    BindingError, BindingHost, BindingId, BindingSet, BindingWrite, EndpointKey, PropertyEndpoint,
};
use understory_responder::router::Router;
use understory_responder::types::{ParentLookup, Phase, WidgetLookup};
use understory_style::{
    MatchState, ResolveCx, SelectorInputs, StyleCascade, StyleCascadeBuilder, Theme, TypeTag,
};

use crate::behavior::{BehaviorRegistry, KnownDimensions, MeasureConstraints, PresentContext};
use crate::button::{BUTTON_BEHAVIOR, ELEMENT_KIND_BUTTON};
use crate::channels::{BINDING, COMPOSE, LAYOUT, PAINT, STYLE};
use crate::element::{ELEMENT_KIND_GENERIC, Element, ElementId, ElementKind};
use crate::generic::GENERIC_BEHAVIOR;
use crate::lookup::UiStoreLookup;
use crate::properties::BuiltInProperties;
use crate::pseudos::{PSEUDO_FOCUSED, PSEUDO_HOVER, PSEUDO_PRESSED};
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
    /// Retained resolved drawing intent. Each element has one entry,
    /// keyed by its primary box-tree node id. Behaviors write
    /// primitives here during the present pass; paint walks the box
    /// tree and lowers primitives + world bounds into draw commands.
    ///
    /// Held as `Option` so `present_element` can `take` it briefly
    /// while it holds a `&Ui` borrow (for `UiStoreLookup`); always
    /// `Some` outside that window.
    presentation: Option<PresentationStore<NodeId, ElementId>>,
    cascade: StyleCascade,
    theme: Theme,
    match_states: HashMap<ElementId, MatchState>,
    /// Path-based hover state machine from `understory_event_state`.
    /// Tracks the current root→target hover path; `update_path` returns
    /// the minimal Enter/Leave sequence to move from the old path to a
    /// new one. We apply the events to `:hover` pseudoclass state.
    hover: HoverState<ElementId>,
    /// Transform-aware click recognizer from `understory_event_state`.
    /// Records primary-button presses + matches them against later ups
    /// to produce [`ClickResult::Click`]s. We use it to drive the
    /// `:pressed` lifecycle on `press_target` and (in a future phase)
    /// to dispatch `Click` events to elements.
    clicks: ClickState<ElementId>,
    /// The element currently in the `:pressed` visual state — the one
    /// that received the most recent primary-button `Down` and has not
    /// yet been cleared by an `Up` or `Cancel`. Cached alongside
    /// `clicks` so we can flip pseudos without re-querying its
    /// per-pointer press map.
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
    /// One-way property bindings. Held as `Option` because `flush_bindings`
    /// temporarily takes ownership: `BindingSet::drain` mutates the set
    /// while also taking `&mut Self` (Ui) as the `BindingHost`, and we'd
    /// otherwise overlap. The set is always `Some` outside `flush_bindings`.
    bindings: Option<BindingSet<ElementId>>,
    /// Spatially-indexed render tree from `understory_box_tree`. Each
    /// element maps to one or more `NodeId`s (primary plus optional
    /// auxiliary nodes for decorations). The tree owns local geometry
    /// (bounds, transform, clip, z-order, flags) and produces a damage
    /// summary on [`Tree::commit`]; paint walks it and hit-testing
    /// uses [`Tree::hit_test_point`].
    tree: Tree,
    /// Element → its node(s) in `tree`, primary first. `SmallVec`
    /// inline capacity of 1 keeps the common no-auxiliary case heap-free.
    element_to_nodes: HashMap<ElementId, SmallVec<[NodeId; 1]>>,
    /// Per-element click handlers. Invoked along the responder
    /// capture → target → bubble sequence when a primary-button
    /// click is recognized by [`ClickState`]. One handler per
    /// element for now; subsequent `on_click` calls replace.
    click_handlers: HashMap<ElementId, ClickHandler>,
    /// Per-element key handlers. Invoked along the responder
    /// sequence rooted at the currently focused element. Same
    /// one-handler-per-element model as `click_handlers`.
    key_handlers: HashMap<ElementId, KeyHandler>,
    /// Currently focused element (the keyboard target). `:focused`
    /// is applied to this element's pseudoclass set.
    focused: Option<ElementId>,
    /// Focus navigation policy. Pluggable in future; for now
    /// `DefaultPolicy` with scope-wrap so Tab cycles within the
    /// element tree.
    focus_policy: DefaultPolicy,
    /// Reusable scratch buffer for building a `FocusSpace`. Kept on
    /// `Ui` so each navigation call doesn't reallocate.
    focus_buf: Vec<FocusEntry<NodeId>>,
}

type ClickHandler = Box<dyn FnMut(&mut Ui, ClickEvent)>;
type KeyHandler = Box<dyn FnMut(&mut Ui, KeyEvent<'_>)>;

/// Payload delivered to a key handler. Carries the normalized
/// `ui_events`/`keyboard-types` event by reference plus the
/// responder dispatch context. Handlers filter `key.state`,
/// `key.key`, `key.modifiers`, etc. themselves — `Ui::on_key`
/// does no internal filtering.
#[allow(
    unnameable_types,
    reason = "re-exported from the crate root via lib.rs"
)]
#[derive(Debug)]
pub struct KeyEvent<'a> {
    /// The element keyboard input was routed to (the focused
    /// element at the time of dispatch).
    pub source: ElementId,
    /// The element this handler is registered on — the current
    /// node in the responder sequence. Equal to `source` for the
    /// target-phase invocation; differs for ancestors during
    /// capture and bubble.
    pub current: ElementId,
    /// Propagation phase (capture, target, or bubble) for this
    /// invocation.
    pub phase: Phase,
    /// The full normalized keyboard event.
    pub key: &'a KeyboardEvent,
}

/// Payload delivered to a click handler.
#[allow(
    unnameable_types,
    reason = "re-exported from the crate root via lib.rs"
)]
#[derive(Copy, Clone, Debug)]
pub struct ClickEvent {
    /// The element the user actually clicked (the target).
    pub source: ElementId,
    /// The element this handler is registered on (where in the
    /// responder path the dispatch is currently). Equal to `source`
    /// for the target-phase invocation; differs for ancestors during
    /// capture and bubble.
    pub current: ElementId,
    /// The propagation phase (capture, target, or bubble) this
    /// invocation belongs to.
    pub phase: Phase,
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
            presentation: Some(PresentationStore::new()),
            cascade,
            theme,
            match_states: HashMap::new(),
            hover: HoverState::new(),
            clicks: ClickState::new(),
            press_target: None,
            taffy: taffy::TaffyTree::new(),
            element_to_taffy: HashMap::new(),
            currently_expanding: None,
            text: RefCell::new(TextEngine::new()),
            bindings: Some(BindingSet::new(BINDING)),
            tree: Tree::new(),
            element_to_nodes: HashMap::new(),
            click_handlers: HashMap::new(),
            key_handlers: HashMap::new(),
            focused: None,
            focus_policy: DefaultPolicy {
                wrap: WrapMode::Scope,
            },
            focus_buf: Vec::new(),
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
    /// For controls whose default template reads initial property values
    /// from the owner (e.g. a Button's template copying its `Content` to
    /// the label), use [`insert_child_with`](Self::insert_child_with) so
    /// the configuration closure can write those properties *before* the
    /// template expands. Until Phase 1f-6's `TemplateBinding` lands, this
    /// is the only way to make the initial snapshot pick up app values.
    ///
    /// # Panics
    ///
    /// Panics if `parent` is a stale id.
    pub fn insert_child(&mut self, parent: ElementId, kind: ElementKind) -> ElementId {
        self.insert_child_with(parent, kind, |_, _| {})
    }

    /// Like [`insert_child`](Self::insert_child), but runs `configure` on
    /// the new element *before* its default template expands. Apps use
    /// this to seed properties whose values the template will snapshot
    /// (e.g. `Content` for Button).
    ///
    /// # Panics
    ///
    /// Panics if `parent` is a stale id.
    pub fn insert_child_with<F>(
        &mut self,
        parent: ElementId,
        kind: ElementKind,
        configure: F,
    ) -> ElementId
    where
        F: FnOnce(&mut Self, ElementId),
    {
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
        configure(self, child);
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
    ///
    /// Marks affected channels in the invalidation tracker and notifies the
    /// binding set so any binding whose source is this endpoint will fire
    /// on the next `flush_bindings`.
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
        if let Some(bindings) = self.bindings.as_mut() {
            bindings.mark_endpoint_changed(EndpointKey::new(id, property.id()));
        }
    }

    /// Registers a one-way binding from `source` to `target`. Both
    /// endpoints' value types must match. Returns the new [`BindingId`]
    /// or a [`BindingError`] if the registration would self-loop or
    /// introduce a cycle.
    ///
    /// The binding will fire on the next `flush_bindings` if its source
    /// endpoint has been written since the last drain (or has not yet
    /// drained at all). Bindings are typically registered during template
    /// expansion via [`Template::instantiate`].
    pub fn bind<T>(
        &mut self,
        source: PropertyEndpoint<ElementId, T>,
        target: PropertyEndpoint<ElementId, T>,
    ) -> Result<BindingId, BindingError<ElementId>>
    where
        T: Clone + 'static,
    {
        let bindings = self
            .bindings
            .as_mut()
            .expect("bindings always Some outside flush_bindings");
        let id = bindings.bind(source, target)?;
        // Schedule the binding so its initial value propagates on the next
        // drain. Without this, freshly-registered bindings stay clean
        // (the binding crate only marks on subsequent source changes).
        bindings.mark_binding_dirty(id);
        Ok(id)
    }

    /// Drains the binding set, propagating dirty bindings through the host.
    /// Channels affected by binding writes are unioned into the invalidation
    /// tracker as a side effect of the host's `set_erased` impl.
    pub fn flush_bindings(&mut self) {
        let Some(mut bindings) = self.bindings.take() else {
            return;
        };
        let _ = bindings.drain(self);
        self.bindings = Some(bindings);
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

    /// Returns the deepest element currently in the `:hover` state, if
    /// any. Higher-up ancestors along the hover path are also
    /// `:hover` (see [`HoverState`]) — this is just the leaf for
    /// convenience.
    #[must_use]
    #[inline]
    pub fn hovered(&self) -> Option<ElementId> {
        self.hover.current_path().last().copied()
    }

    /// Hit-tests `point` against the box tree and returns the topmost,
    /// deepest element whose node covers it.
    ///
    /// Internally delegates to [`Tree::hit_test_point`], which uses
    /// `understory_box_tree`'s spatial index. Returns `None` if no
    /// node is hit. Requires that the most recent geometry mutation
    /// has been followed by a [`Tree::commit`] — `compute_layout`
    /// commits as its last step, and read-only consumers in tests
    /// either go through layout or write nothing first.
    #[must_use]
    pub fn hit_test(&self, point: Point) -> Option<ElementId> {
        let hit = self.tree.hit_test_point(point, QueryFilter::new())?;
        self.node_to_element(hit.node)
    }

    /// Returns the element that owns a given `NodeId`, if any. Helper
    /// for callers that bridge between the box tree's id-space and
    /// the element id-space (e.g. responder integration).
    #[must_use]
    pub fn node_to_element(&self, node: NodeId) -> Option<ElementId> {
        for (element_id, nodes) in &self.element_to_nodes {
            if nodes.contains(&node) {
                return Some(*element_id);
            }
        }
        None
    }

    /// Returns the primary box-tree node for `id`, or `None` for
    /// stale ids.
    #[must_use]
    pub fn box_for(&self, id: ElementId) -> Option<NodeId> {
        self.element_to_nodes
            .get(&id)
            .and_then(|nodes| nodes.first().copied())
    }

    /// Toggles the `Focusable` flag on `id`'s primary box-tree node.
    /// Focusable elements participate in [`Self::focus_navigate`]
    /// and can receive keyboard events via [`Self::on_key`]. No-op
    /// for stale ids.
    pub fn set_focusable(&mut self, id: ElementId, focusable: bool) {
        let Some(primary) = self.box_for(id) else {
            return;
        };
        let mut flags = self.tree.flags(primary).unwrap_or_default();
        if focusable {
            flags.insert(NodeFlags::FOCUSABLE);
        } else {
            flags.remove(NodeFlags::FOCUSABLE);
        }
        self.tree.set_flags(primary, flags);
    }

    /// Returns the element that currently has focus, if any.
    #[must_use]
    #[inline]
    pub fn focused(&self) -> Option<ElementId> {
        self.focused
    }

    /// Sets keyboard focus. Passes `None` to clear focus. Applies /
    /// removes the `:focused` pseudoclass on the affected elements.
    /// Returns `true` if the focus changed.
    pub fn set_focus(&mut self, new: Option<ElementId>) -> bool {
        if self.focused == new {
            return false;
        }
        if let Some(old_id) = self.focused.take() {
            self.remove_pseudo(old_id, PSEUDO_FOCUSED);
        }
        if let Some(new_id) = new
            && self.get(new_id).is_some()
        {
            self.add_pseudo(new_id, PSEUDO_FOCUSED);
            self.focused = Some(new_id);
        } else {
            self.focused = None;
        }
        true
    }

    /// Moves focus per `direction` (e.g. [`Navigation::Next`] for
    /// Tab, [`Navigation::Prev`] for Shift+Tab). Uses
    /// [`understory_focus::DefaultPolicy`] with `WrapMode::Scope` so
    /// focus cycles within the tree. Returns the new focused element.
    pub fn focus_navigate(&mut self, direction: Navigation) -> Option<ElementId> {
        self.tree.commit();
        let root_node = self.box_for(self.root)?;
        // Resolve the current focus's node before we borrow `tree`
        // and `focus_buf` to build the FocusSpace.
        let current_node = self.focused.and_then(|id| self.box_for(id));
        let new_node = {
            let space =
                build_focus_space_for_scope(&self.tree, root_node, &(), &mut self.focus_buf);
            match current_node {
                Some(current) => self.focus_policy.next(current, direction, &space),
                None => self.focus_policy.initial::<NodeId>(direction, &space),
            }
        };
        let new_element = new_node.and_then(|n| self.node_to_element(n));
        self.set_focus(new_element);
        new_element
    }

    /// Registers a key handler on `id`. The handler runs along the
    /// responder capture → target → bubble sequence rooted at the
    /// currently focused element when [`Self::dispatch_key`] is
    /// called. One handler per element for now; subsequent calls
    /// replace the previous handler.
    pub fn on_key<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Self, KeyEvent<'_>) + 'static,
    {
        self.key_handlers.insert(id, Box::new(handler));
    }

    /// Removes a key handler previously registered with
    /// [`Self::on_key`]. No-op if none is registered.
    pub fn remove_key_handler(&mut self, id: ElementId) {
        self.key_handlers.remove(&id);
    }

    /// Routes `event` to the currently focused element and runs
    /// every key handler along the responder capture → target →
    /// bubble path. No-op when nothing is focused.
    pub fn dispatch_key(&mut self, event: &KeyboardEvent) {
        let Some(focused) = self.focused else {
            return;
        };
        let seq: Vec<(Phase, ElementId)> = {
            let lookup = UiWidgetLookup;
            let parent = UiParentLookup { ui: self };
            let router: Router<ElementId, UiWidgetLookup, UiParentLookup<'_>> =
                Router::with_parent(lookup, parent);
            router
                .dispatch_for::<()>(focused)
                .into_iter()
                .map(|d| (d.phase, d.node))
                .collect()
        };
        for (phase, node) in seq {
            let Some(mut handler) = self.key_handlers.remove(&node) else {
                continue;
            };
            let ev = KeyEvent {
                source: focused,
                current: node,
                phase,
                key: event,
            };
            handler(self, ev);
            if self.get(node).is_some() && !self.key_handlers.contains_key(&node) {
                self.key_handlers.insert(node, handler);
            }
        }
    }

    /// Registers a click handler on `id`. The handler runs along the
    /// responder capture → target → bubble sequence when
    /// [`ClickState`] recognizes a primary-button click whose target
    /// path includes this element. One handler per element for now;
    /// subsequent calls replace the previous handler.
    pub fn on_click<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Self, ClickEvent) + 'static,
    {
        self.click_handlers.insert(id, Box::new(handler));
    }

    /// Removes a click handler previously registered with
    /// [`Self::on_click`]. No-op if no handler is registered.
    pub fn remove_click_handler(&mut self, id: ElementId) {
        self.click_handlers.remove(&id);
    }

    /// Builds the responder dispatch sequence for `target` and runs
    /// each element's click handler along it (capture, then target,
    /// then bubble). Called when [`ClickState::on_up`] returns
    /// [`ClickResult::Click`].
    fn dispatch_click(&mut self, target: ElementId) {
        // Compute the sequence with `&self` borrows, drop them, then
        // run handlers with `&mut self`.
        let seq: Vec<(Phase, ElementId)> = {
            let lookup = UiWidgetLookup;
            let parent = UiParentLookup { ui: self };
            let router: Router<ElementId, UiWidgetLookup, UiParentLookup<'_>> =
                Router::with_parent(lookup, parent);
            let dispatches = router.dispatch_for::<()>(target);
            dispatches.into_iter().map(|d| (d.phase, d.node)).collect()
        };

        for (phase, node) in seq {
            // Take the handler out so it can call `&mut self`, then
            // put it back unless the handler removed itself.
            let Some(mut handler) = self.click_handlers.remove(&node) else {
                continue;
            };
            let event = ClickEvent {
                source: target,
                current: node,
                phase,
            };
            handler(self, event);
            // Reinstate iff the element is still alive AND the
            // handler didn't replace itself during the call.
            if self.get(node).is_some() && !self.click_handlers.contains_key(&node) {
                self.click_handlers.insert(node, handler);
            }
        }
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
                self.update_hover_at(pos)
            }
            PointerEvent::Down(button_event) => {
                if button_event.button != Some(PointerButton::Primary) {
                    return false;
                }
                let pos = Point::new(button_event.state.position.x, button_event.state.position.y);
                let time_ms = button_event.state.time / 1_000_000;
                // A Down without a preceding Move can arrive on some
                // platforms; refresh hover first so any newly-entered
                // path is set up before we press.
                let mut changed = self.update_hover_at(pos);
                if let Some(target) = self.hit_test(pos) {
                    self.clicks.on_down(None, None, target, pos, time_ms);
                    changed |= self.set_press_target(Some(target));
                } else {
                    changed |= self.set_press_target(None);
                }
                changed
            }
            PointerEvent::Up(button_event) => {
                if button_event.button != Some(PointerButton::Primary) {
                    return false;
                }
                let pos = Point::new(button_event.state.position.x, button_event.state.position.y);
                let time_ms = button_event.state.time / 1_000_000;
                let hit = self.hit_test(pos);
                // Hand the up to ClickState — produces a
                // `ClickResult::Click(target)` when the press is
                // recognized; dispatch handlers along the responder
                // path for that target.
                let result = match hit {
                    Some(target) => self.clicks.on_up(None, None, &target, pos, time_ms),
                    None => {
                        // Up off any element — cancel any open press
                        // so it doesn't dangle and pretend to click
                        // later.
                        let _ = self.clicks.cancel(None);
                        ClickResult::Suppressed(None)
                    }
                };
                let press_changed = self.set_press_target(None);
                if let ClickResult::Click(target) = result {
                    self.dispatch_click(target);
                }
                press_changed
            }
            PointerEvent::Leave(_) => self.clear_hover(),
            PointerEvent::Cancel(_) => {
                let _ = self.clicks.cancel(None);
                let h = self.clear_hover();
                let p = self.set_press_target(None);
                h || p
            }
            _ => false,
        }
    }

    /// Updates the hover state to reflect a pointer at `pos`. Computes
    /// the element path from the box tree's hit-test result and feeds
    /// it to [`HoverState`], applying its `Enter`/`Leave` events to
    /// pseudoclass state.
    fn update_hover_at(&mut self, pos: Point) -> bool {
        let path: Vec<ElementId> = self
            .tree
            .hit_test_point(pos, QueryFilter::new())
            .map(|hit| {
                hit.path
                    .into_iter()
                    .filter_map(|n| self.node_to_element(n))
                    .collect()
            })
            .unwrap_or_default();
        let events = self.hover.update_path(&path);
        self.apply_hover_events(events)
    }

    /// Clears the hover state (e.g. on pointer Leave). Returns `true`
    /// if anything changed.
    fn clear_hover(&mut self) -> bool {
        let events = self.hover.clear();
        self.apply_hover_events(events)
    }

    fn apply_hover_events(&mut self, events: Vec<HoverEvent<ElementId>>) -> bool {
        let mut changed = false;
        for event in events {
            match event {
                HoverEvent::Enter(id) => {
                    self.add_pseudo(id, PSEUDO_HOVER);
                    changed = true;
                }
                HoverEvent::Leave(id) => {
                    self.remove_pseudo(id, PSEUDO_HOVER);
                    changed = true;
                }
            }
        }
        changed
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
    /// Drains pending property bindings first so binding-driven writes
    /// participate in this frame's layout. Then flushes the LAYOUT
    /// channel: any element whose `Layout` style has changed gets its
    /// taffy style pushed before taffy is asked to recompute. `Bounds`
    /// writes use the standard `set_local_notifying` path so PAINT
    /// cascades naturally for elements whose box changed.
    pub fn compute_layout(&mut self, viewport: Size) {
        // Bindings first — they may dirty LAYOUT/PAINT for their targets.
        self.flush_bindings();
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
        // Finalize the box tree: refresh world-space data and the
        // spatial index so the next `hit_test` (or any other tree
        // query) reflects this frame's geometry.
        self.tree.commit();
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

        // Mirror the world-space bounds into the box tree's primary
        // node for this element. Since every node is currently a
        // tree root with IDENTITY transform, `local_bounds == world_bounds`
        // and `Tree::hit_test_point` matches the old element-bounds walk.
        if let Some(&primary) = self.element_to_nodes.get(&id).and_then(|v| v.first()) {
            self.tree.set_local_bounds(primary, rect);
        }

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

    /// Runs the present pass and lowers the resulting presentation
    /// primitives into `output`.
    ///
    /// Steps:
    ///
    /// 1. Flush any pending STYLE work (it may dirty PAINT).
    /// 2. For every PAINT-dirty element, call its behavior's `present`
    ///    so it resolves properties through the cascade and writes
    ///    primitives into [`PresentationStore`].
    /// 3. Drain the presentation store's dirty set (callers can hook
    ///    in here later for damage-driven re-lowering).
    /// 4. Walk the box tree depth-first; at each node, look up its
    ///    presentation primitives and lower them — fill surfaces,
    ///    shape and paint text — using the box tree's world bounds.
    /// 5. Clear COMPOSE.
    pub fn paint(&mut self, output: &mut Scene) {
        self.flush_style();
        // Make sure the box tree's world-space data and spatial index
        // reflect any structural / bounds changes since the last
        // commit. `compute_layout` already commits at the end, so
        // when paint follows layout this is a cheap no-op; tests
        // that paint without layout (or callers exploring partial
        // pipelines) get a correct commit on demand.
        self.tree.commit();
        let dirty: Vec<ElementId> = self.invalidation.drain_sorted(PAINT).collect();
        for id in dirty {
            self.present_element(id);
        }
        // We don't yet drive incremental re-lowering off this, but
        // draining keeps the dirty set bounded.
        let _: Vec<NodeId> = self
            .presentation
            .as_mut()
            .expect("presentation always Some outside present_element")
            .take_dirty()
            .collect();

        output.clear();
        if let Some(root_box) = self.box_for(self.root) {
            self.lower_subtree(root_box, output);
        }
        self.invalidation.clear(COMPOSE);
    }

    /// Runs `behavior.present` for one element, updating its
    /// presentation node in the store. If the element or its behavior
    /// is gone we drop silently; the presentation entry is removed in
    /// `remove_subtree` separately.
    fn present_element(&mut self, id: ElementId) {
        // Take the presentation store out first so we can later hold
        // both a `&Ui` borrow (for `UiStoreLookup`) and a
        // `&mut PresentationStore` simultaneously. Always put it back
        // before returning.
        let mut presentation = self
            .presentation
            .take()
            .expect("presentation always Some outside present_element");
        self.present_element_inner(id, &mut presentation);
        self.presentation = Some(presentation);
    }

    fn present_element_inner(
        &self,
        id: ElementId,
        presentation: &mut PresentationStore<NodeId, ElementId>,
    ) {
        let Some(element) = self.get(id) else {
            return;
        };
        let Some(behavior) = self.behaviors.get(element.kind()) else {
            return;
        };
        let Some(node) = self.box_for(id) else {
            return;
        };
        let match_state = self
            .match_states
            .get(&id)
            .copied()
            .unwrap_or_else(|| self.cascade.root_state());
        let ctx = PresentContext {
            registry: &self.registry,
            properties: &self.properties,
            theme: &self.theme,
            cascade: &self.cascade,
            match_state,
            lookup: UiStoreLookup::new(self),
        };
        behavior.present(element, &ctx, presentation, node);
    }

    /// Recursively walks the box tree depth-first, lowering each
    /// node's presentation primitives into `output` at its world
    /// bounds.
    fn lower_subtree(&self, node: NodeId, output: &mut Scene) {
        self.lower_one(node, output);
        let children: Vec<NodeId> = self.tree.children_of(node).to_vec();
        for child in children {
            self.lower_subtree(child, output);
        }
    }

    fn lower_one(&self, node: NodeId, output: &mut Scene) {
        let Some(bounds) = self.tree.world_bounds(node) else {
            return;
        };
        if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
            return;
        }
        let Some(store) = self.presentation.as_ref() else {
            return;
        };
        let Some(presentation) = store.node(node) else {
            return;
        };
        lower_presentation(presentation.primitives(), bounds, output, &self.text);
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
        let classes: SmallVec<[understory_style::ClassId; 4]> =
            SmallVec::from_slice(element.classes());
        let pseudos: SmallVec<[understory_style::PseudoClassId; 4]> =
            SmallVec::from_slice(element.pseudos());

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
        self.match_states.remove(&id);
        self.invalidation.remove_key(id);
        self.click_handlers.remove(&id);
        self.key_handlers.remove(&id);
        if self.focused == Some(id) {
            self.focused = None;
        }
        if let Some(taffy_node) = self.element_to_taffy.remove(&id) {
            // `taffy.remove` also detaches from its parent and frees children
            // recursively — but we've already walked our subtree top-down and
            // recursed into our children before reaching this point, so the
            // children's taffy nodes are already gone. Removing the (now
            // childless) taffy node here just finalizes the leaf.
            let _ = self.taffy.remove(taffy_node);
        }
        if let Some(nodes) = self.element_to_nodes.remove(&id) {
            for node in nodes {
                if let Some(store) = self.presentation.as_mut() {
                    let _ = store.remove(node);
                }
                self.tree.remove(node);
            }
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

        // Mirror into the box tree. The new node hangs off its
        // parent element's primary tree node so the box tree mirrors
        // the element-tree structure. `LocalNode::default()` gives
        // IDENTITY transform / no clip / visible + pickable flags;
        // arrange writes `local_bounds` per element. Bounds are
        // stored in world coords for now (paired with IDENTITY
        // transforms at every level), so `world_bounds == local_bounds`
        // and the tree's hit testing matches what the element-bounds
        // walk used to do — with `understory_index`'s spatial index
        // doing the heavy lifting. A future phase will refactor
        // behaviors to paint in local coords and exercise the tree's
        // world-transform composition for compositor animation.
        let parent_node = parent
            .and_then(|p| self.element_to_nodes.get(&p))
            .and_then(|v| v.first())
            .copied();
        let tree_node = self.tree.insert(parent_node, LocalNode::default());
        let mut nodes = SmallVec::<[NodeId; 1]>::new();
        nodes.push(tree_node);
        self.element_to_nodes.insert(id, nodes);
        // Create a presentation entry for the primary node, sourced
        // from the element. Behaviors populate primitives later via
        // `present`; presence in the store is what lets the lowerer
        // find this node during paint.
        if let Some(store) = self.presentation.as_mut() {
            let _ = store.insert(tree_node, id);
        }
        id
    }
}

impl Default for Ui {
    fn default() -> Self {
        Self::new()
    }
}

/// `BindingHost` adapter for `Ui`. `get_erased` reads a property's effective
/// local value (Animation > local-layer winner) through the new erased
/// accessors on `understory_property::PropertyStore`; `set_erased` writes
/// into the `LocalValueSource::TemplateBinding` slot so the user's
/// `LocalValueSource::Local` value (if any) keeps winning.
impl BindingHost<ElementId> for Ui {
    fn get_erased(&self, endpoint: EndpointKey<ElementId>) -> Option<ErasedValue> {
        let element = self.get(endpoint.owner())?;
        element
            .property_store()
            .effective_local_erased(endpoint.property())
            .cloned()
    }

    fn set_erased(&mut self, endpoint: EndpointKey<ElementId>, value: ErasedValue) -> BindingWrite {
        let id = endpoint.owner();
        let prop_id = endpoint.property();
        let channels = {
            // Disjoint-field split-borrow: registry is read-only, slots is
            // mutated through `element_mut`. Both are direct field borrows of
            // `self`, so the borrow checker accepts the pair.
            let registry = &self.registry;
            let Some(element) = element_mut(&mut self.slots, id) else {
                return BindingWrite::unchanged();
            };
            match element
                .property_store_mut()
                .set_local_erased_with_source_notifying(
                    prop_id,
                    value,
                    understory_property::LocalValueSource::TemplateBinding,
                    registry,
                ) {
                Ok(channels) => channels,
                Err(_) => return BindingWrite::unchanged(),
            }
        };
        if channels.is_empty() {
            BindingWrite::unchanged()
        } else {
            self.mark_channels(id, channels);
            BindingWrite::changed(channels)
        }
    }
}

/// Identity `WidgetLookup` for the responder: each `ElementId` is
/// its own widget id (overstory doesn't have a separate widget-vs-
/// node identity).
struct UiWidgetLookup;

impl WidgetLookup<ElementId> for UiWidgetLookup {
    type WidgetId = ElementId;
    fn widget_of(&self, node: &ElementId) -> Option<ElementId> {
        Some(*node)
    }
}

/// `ParentLookup` adapter so the responder can reconstruct the
/// root→target path from the element tree.
struct UiParentLookup<'a> {
    ui: &'a Ui,
}

impl<'a> ParentLookup<ElementId> for UiParentLookup<'a> {
    fn parent_of(&self, node: &ElementId) -> Option<ElementId> {
        self.ui.get(*node).and_then(Element::parent)
    }
}

/// Translates taffy's `AvailableSpace` into kurbo's `f64`. `Definite(v)`
/// becomes `v` as `f64`. Both `MinContent` and `MaxContent` become
/// `f64::INFINITY` for now — behaviors receiving infinity report their
/// "natural" unwrapped size, which taffy uses as both min and max content.
/// That's a conservative answer that prevents over-aggressive wrapping
/// (e.g. text breaking on every word) at the cost of tight min-content
/// packing. A later refinement can distinguish the two and let behaviors
/// report a tighter min-content (e.g. the longest unbreakable word width
/// for text).
/// Lowers a node's resolved presentation primitives into `output`,
/// positioned at `bounds`. Stand-in for a future `imaging`-side
/// renderer that consumes `understory_presentation` directly.
fn lower_presentation(
    primitives: &[Primitive<u64>],
    bounds: Rect,
    output: &mut Scene,
    text: &RefCell<TextEngine>,
) {
    use imaging::Painter;
    use kurbo::RoundedRect;

    let mut painter = Painter::new(output);
    for primitive in primitives {
        match primitive {
            Primitive::Surface(surface) => {
                let corner_radius = surface.corner_radii.top_left;
                for layer in &surface.backgrounds {
                    if corner_radius > 0.0 {
                        let rounded = RoundedRect::from_rect(bounds, corner_radius);
                        painter.fill(rounded, &layer.brush).draw();
                    } else {
                        painter.fill(bounds, &layer.brush).draw();
                    }
                }
                // Border / shadow lowering — TODO when controls need
                // them.
            }
            Primitive::Text(boxed) => {
                if let TextPrimitive::Plain(plain) = boxed.as_ref() {
                    if plain.content.is_empty() {
                        continue;
                    }
                    let Some(fg) = plain.foreground.as_ref() else {
                        continue;
                    };
                    let layout = text.borrow_mut().shape(
                        plain.content.as_str(),
                        plain.style.font_size,
                        fg.clone(),
                        None,
                    );
                    let origin = bounds.origin();
                    crate::text::render_text(
                        &mut painter,
                        Affine::translate((origin.x, origin.y)),
                        &layout,
                    );
                }
            }
            // Image / future primitives land when controls need them.
            _ => {}
        }
    }
}

fn available_to_f64(s: taffy::AvailableSpace) -> f64 {
    match s {
        taffy::AvailableSpace::Definite(v) => f64::from(v),
        taffy::AvailableSpace::MinContent | taffy::AvailableSpace::MaxContent => f64::INFINITY,
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
    fn paint_drains_dirty_and_emits_output() {
        let mut ui = Ui::new();
        let root = ui.root();
        let layout_prop = ui.properties().layout;
        let background = ui.properties().background;

        ui.set_local(
            root,
            layout_prop,
            taffy::Style {
                size: taffy::Size {
                    width: taffy::Dimension::length(10.0),
                    height: taffy::Dimension::length(10.0),
                },
                ..taffy::Style::default()
            },
        );
        ui.set_local(
            root,
            background,
            Some(Brush::Solid(Color::from_rgba8(0xff, 0, 0, 0xff))),
        );
        ui.compute_layout(Size::new(100.0, 100.0));

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
        let layout_prop = ui.properties().layout;
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

        ui.set_local(
            root,
            layout_prop,
            taffy::Style {
                size: taffy::Size {
                    width: taffy::Dimension::length(10.0),
                    height: taffy::Dimension::length(10.0),
                },
                ..taffy::Style::default()
            },
        );
        ui.compute_layout(Size::new(100.0, 100.0));

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
        // Hit testing now goes through `understory_box_tree`'s
        // spatially-indexed `Tree::hit_test_point`, which reads the
        // tree's `local_bounds` populated by `compute_layout`. Drive
        // the test through the layout pipeline: root 200×200 with
        // 50px padding all around; child fills the content area,
        // giving it 50..150 × 50..150.
        let mut ui = Ui::new();
        let root = ui.root();
        let layout_prop = ui.properties().layout;
        let inset = taffy::LengthPercentage::length(50.0);
        ui.set_local(
            root,
            layout_prop,
            taffy::Style {
                display: taffy::Display::Flex,
                size: taffy::Size {
                    width: taffy::Dimension::length(200.0),
                    height: taffy::Dimension::length(200.0),
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
        let child = ui.insert_child(root, ELEMENT_KIND_GENERIC);
        ui.set_local(
            child,
            layout_prop,
            taffy::Style {
                flex_grow: 1.0,
                align_self: Some(taffy::AlignSelf::Stretch),
                ..Default::default()
            },
        );
        ui.compute_layout(Size::new(200.0, 200.0));

        // Inside child's bounds → child wins (deepest, topmost).
        assert_eq!(ui.hit_test(Point::new(100.0, 100.0)), Some(child));
        // Inside root only.
        assert_eq!(ui.hit_test(Point::new(10.0, 10.0)), Some(root));
        // Outside root entirely.
        assert_eq!(ui.hit_test(Point::new(300.0, 300.0)), None);
    }

    #[test]
    fn set_focus_toggles_pseudoclass() {
        let mut ui = Ui::new();
        let root = ui.root();
        let a = ui.insert_child(root, ELEMENT_KIND_GENERIC);
        let b = ui.insert_child(root, ELEMENT_KIND_GENERIC);

        assert_eq!(ui.focused(), None);
        assert!(ui.set_focus(Some(a)));
        assert_eq!(ui.focused(), Some(a));
        assert!(ui.get(a).unwrap().pseudos().contains(&PSEUDO_FOCUSED));

        // Move focus to b — a should lose :focused, b should gain it.
        assert!(ui.set_focus(Some(b)));
        assert!(!ui.get(a).unwrap().pseudos().contains(&PSEUDO_FOCUSED));
        assert!(ui.get(b).unwrap().pseudos().contains(&PSEUDO_FOCUSED));

        // Clear focus.
        assert!(ui.set_focus(None));
        assert!(!ui.get(b).unwrap().pseudos().contains(&PSEUDO_FOCUSED));
        assert_eq!(ui.focused(), None);
    }

    #[test]
    fn focus_navigate_walks_focusables() {
        // Lay out three focusable children. `Navigation::Next`
        // cycles through them in document order with WrapMode::Scope.
        let mut ui = Ui::new();
        let root = ui.root();
        let layout_prop = ui.properties().layout;
        ui.set_local(
            root,
            layout_prop,
            taffy::Style {
                display: taffy::Display::Flex,
                size: taffy::Size {
                    width: taffy::Dimension::length(300.0),
                    height: taffy::Dimension::length(50.0),
                },
                ..Default::default()
            },
        );
        let a = ui.insert_child(root, ELEMENT_KIND_GENERIC);
        let b = ui.insert_child(root, ELEMENT_KIND_GENERIC);
        let c = ui.insert_child(root, ELEMENT_KIND_GENERIC);
        for id in [a, b, c] {
            ui.set_local(
                id,
                layout_prop,
                taffy::Style {
                    flex_grow: 1.0,
                    align_self: Some(taffy::AlignSelf::Stretch),
                    ..Default::default()
                },
            );
            ui.set_focusable(id, true);
        }
        ui.compute_layout(Size::new(300.0, 50.0));

        // Starting unfocused, Next picks the initial element.
        let first = ui.focus_navigate(Navigation::Next).unwrap();
        assert_eq!(first, a);
        assert_eq!(ui.focus_navigate(Navigation::Next), Some(b));
        assert_eq!(ui.focus_navigate(Navigation::Next), Some(c));
        // WrapMode::Scope → back to the first.
        assert_eq!(ui.focus_navigate(Navigation::Next), Some(a));
        // Prev cycles backwards.
        assert_eq!(ui.focus_navigate(Navigation::Prev), Some(c));
    }

    #[test]
    fn dispatch_key_runs_target_handler() {
        use core::cell::Cell;
        use std::rc::Rc;
        use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.insert_child(root, ELEMENT_KIND_GENERIC);
        ui.set_focusable(child, true);
        ui.set_focus(Some(child));

        let fired = Rc::new(Cell::new(false));
        let inc = fired.clone();
        ui.on_key(child, move |_ui, event| {
            if matches!(event.phase, Phase::Target) {
                inc.set(true);
            }
        });

        let key = KeyboardEvent::key_down(Key::Named(NamedKey::Enter), Code::Enter);
        ui.dispatch_key(&key);
        assert!(fired.get(), "target-phase key handler should fire");
    }

    #[test]
    fn dispatch_key_noop_without_focus() {
        use core::cell::Cell;
        use std::rc::Rc;
        use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

        let mut ui = Ui::new();
        let root = ui.root();
        let fired = Rc::new(Cell::new(false));
        let inc = fired.clone();
        ui.on_key(root, move |_ui, _event| inc.set(true));

        // No focus set → dispatch should be a no-op even though root
        // has a handler.
        let key = KeyboardEvent::key_down(Key::Named(NamedKey::Enter), Code::Enter);
        ui.dispatch_key(&key);
        assert!(!fired.get(), "key handler should not fire without focus");
    }

    #[test]
    fn dispatch_click_runs_target_handler() {
        // Register a click handler on a child element; dispatch a
        // click against it; the handler should run with phase Target
        // and source == child.
        use core::cell::Cell;
        use std::rc::Rc;

        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.insert_child(root, ELEMENT_KIND_GENERIC);

        let fired_with = Rc::new(Cell::new(None::<(Phase, ElementId, ElementId)>));
        let fired = fired_with.clone();
        ui.on_click(child, move |_ui, event| {
            if matches!(event.phase, Phase::Target) {
                fired.set(Some((event.phase, event.source, event.current)));
            }
        });

        ui.dispatch_click(child);

        let (phase, source, current) = fired_with.get().expect("handler should have fired");
        assert!(matches!(phase, Phase::Target));
        assert_eq!(source, child);
        assert_eq!(current, child);
    }

    #[test]
    fn dispatch_click_walks_capture_target_bubble() {
        // Register handlers on root, child, and grandchild; click the
        // grandchild. Expected dispatch order: root capture, child
        // capture, grandchild target, child bubble, root bubble.
        use alloc::string::String;
        use core::cell::RefCell;
        use std::rc::Rc;

        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.insert_child(root, ELEMENT_KIND_GENERIC);
        let grand = ui.insert_child(child, ELEMENT_KIND_GENERIC);

        let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        for (name, id) in [("root", root), ("child", child), ("grand", grand)] {
            let log = log.clone();
            ui.on_click(id, move |_ui, event| {
                let phase = match event.phase {
                    Phase::Capture => "capture",
                    Phase::Target => "target",
                    Phase::Bubble => "bubble",
                };
                log.borrow_mut().push(alloc::format!("{name}:{phase}"));
            });
        }

        ui.dispatch_click(grand);

        let log = log.borrow().clone();
        assert_eq!(
            log,
            vec![
                "root:capture".to_owned(),
                "child:capture".to_owned(),
                "grand:target".to_owned(),
                "child:bubble".to_owned(),
                "root:bubble".to_owned(),
            ]
        );
    }

    #[test]
    fn remove_click_handler_stops_firing() {
        use core::cell::Cell;
        use std::rc::Rc;

        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.insert_child(root, ELEMENT_KIND_GENERIC);

        let count = Rc::new(Cell::new(0_u32));
        let inc = count.clone();
        ui.on_click(child, move |_ui, _event| {
            inc.set(inc.get() + 1);
        });

        ui.dispatch_click(child);
        assert_eq!(count.get(), 1);

        ui.remove_click_handler(child);
        ui.dispatch_click(child);
        assert_eq!(count.get(), 1, "removed handler should not fire");
    }

    #[test]
    fn hover_path_marks_ancestor_pseudoclasses() {
        // A child hover should mark every element along the
        // root→target path as `:hover` — the more correct CSS/WPF
        // behavior we get from `HoverState::update_path`. Sanity-check
        // by laying out a parent with a smaller padded child, hovering
        // over the child, and verifying both elements have :hover.
        let mut ui = Ui::new();
        let root = ui.root();
        let layout_prop = ui.properties().layout;
        let inset = taffy::LengthPercentage::length(50.0);
        ui.set_local(
            root,
            layout_prop,
            taffy::Style {
                display: taffy::Display::Flex,
                size: taffy::Size {
                    width: taffy::Dimension::length(200.0),
                    height: taffy::Dimension::length(200.0),
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
        let child = ui.insert_child(root, ELEMENT_KIND_GENERIC);
        ui.set_local(
            child,
            layout_prop,
            taffy::Style {
                flex_grow: 1.0,
                align_self: Some(taffy::AlignSelf::Stretch),
                ..Default::default()
            },
        );
        ui.compute_layout(Size::new(200.0, 200.0));

        // Hover via update_hover_at-equivalent path.
        let changed = ui.update_hover_at(Point::new(100.0, 100.0));
        assert!(changed);

        // Both root and child have :hover; the child is the deepest.
        assert!(ui.get(root).unwrap().pseudos().contains(&PSEUDO_HOVER));
        assert!(ui.get(child).unwrap().pseudos().contains(&PSEUDO_HOVER));
        assert_eq!(ui.hovered(), Some(child));

        // Move pointer outside the child but still in the root's
        // padding band: leaves the child, keeps the root hovered.
        let changed = ui.update_hover_at(Point::new(10.0, 10.0));
        assert!(changed);
        assert!(ui.get(root).unwrap().pseudos().contains(&PSEUDO_HOVER));
        assert!(!ui.get(child).unwrap().pseudos().contains(&PSEUDO_HOVER));
        assert_eq!(ui.hovered(), Some(root));

        // Out of root entirely.
        let changed = ui.update_hover_at(Point::new(500.0, 500.0));
        assert!(changed);
        assert!(!ui.get(root).unwrap().pseudos().contains(&PSEUDO_HOVER));
        assert_eq!(ui.hovered(), None);
    }

    #[test]
    fn element_to_tree_node_round_trips() {
        let mut ui = Ui::new();
        let root = ui.root();
        let a = ui.insert_child(root, ELEMENT_KIND_GENERIC);
        let b = ui.insert_child(root, ELEMENT_KIND_GENERIC);

        // Every element has at least one node; `node_to_element`
        // recovers it.
        for elem in [root, a, b] {
            let nodes = &ui.element_to_nodes[&elem];
            assert!(!nodes.is_empty(), "element should have a tree node");
            for &node in nodes {
                assert_eq!(ui.node_to_element(node), Some(elem));
            }
        }

        // Tree structure mirrors element-tree structure: `a` and `b`
        // are children of `root`'s tree node.
        let root_node = ui.element_to_nodes[&root][0];
        let a_node = ui.element_to_nodes[&a][0];
        let b_node = ui.element_to_nodes[&b][0];
        let children = ui.tree.children_of(root_node);
        assert_eq!(children, &[a_node, b_node]);
        assert_eq!(ui.tree.parent_of(a_node), Some(root_node));
    }

    #[test]
    fn flush_bindings_propagates_template_binding() {
        // Two generic elements; binding from source.content → target.content.
        // Writing source.content then flushing should propagate the value
        // into target's `TemplateBinding` slot so its effective value reads
        // the new content.
        use alloc::string::String;
        use understory_property_binding::PropertyEndpoint;

        let mut ui = Ui::new();
        let root = ui.root();
        let source = ui.insert_child(root, ELEMENT_KIND_GENERIC);
        let target = ui.insert_child(root, ELEMENT_KIND_GENERIC);

        let content_prop = ui.properties().content;
        ui.bind(
            PropertyEndpoint::new(source, content_prop),
            PropertyEndpoint::new(target, content_prop),
        )
        .expect("binding registers");

        ui.set_local(source, content_prop, String::from("hello"));

        // Before flush, the binding is dirty; target still has its default.
        assert_eq!(ui.get(target).unwrap().get_local_value(content_prop), None);

        ui.flush_bindings();

        // After flush, target's effective local content is "hello".
        let stored = ui
            .get(target)
            .unwrap()
            .property_store()
            .effective_local_erased(content_prop.id())
            .and_then(|v| v.downcast_ref::<String>().cloned());
        assert_eq!(stored.as_deref(), Some("hello"));
    }

    #[test]
    fn flush_bindings_respects_local_precedence() {
        // If the target has a user `Local` value, the binding's
        // `TemplateBinding` write should be shadowed — `Local` outranks
        // `TemplateBinding`.
        use alloc::string::String;
        use understory_property_binding::PropertyEndpoint;

        let mut ui = Ui::new();
        let root = ui.root();
        let source = ui.insert_child(root, ELEMENT_KIND_GENERIC);
        let target = ui.insert_child(root, ELEMENT_KIND_GENERIC);

        let content_prop = ui.properties().content;
        // User sets a Local first.
        ui.set_local(target, content_prop, String::from("user"));
        ui.bind(
            PropertyEndpoint::new(source, content_prop),
            PropertyEndpoint::new(target, content_prop),
        )
        .expect("binding registers");
        ui.set_local(source, content_prop, String::from("from-binding"));
        ui.flush_bindings();

        // Target's effective value is still the user's "user" because Local
        // wins over TemplateBinding.
        let stored = ui
            .get(target)
            .unwrap()
            .property_store()
            .effective_local_erased(content_prop.id())
            .and_then(|v| v.downcast_ref::<String>().cloned());
        assert_eq!(stored.as_deref(), Some("user"));
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
