// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The root [`Ui`] context: public facade and coordinator for one retained
//! Overstory tree.
//!
//! Phase 1c added the style cascade integration; Phase 1d added pointer
//! dispatch + `:hover` lifecycle; Phase 1e added layout; Phase 1f
//! added template substrate, text rendering, `:pressed`, the Button
//! control, and property bindings via `understory_property_binding`. Later
//! phases bring the full responder routing, more controls, and animations.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cell::RefCell;

use cursor_icon::CursorIcon;
use imaging::record::{Clip, Geometry, Scene};
use invalidation::{ChannelSet, InvalidationTracker};
use kurbo::{Affine, Point, Rect, RoundedRect, Size, Vec2};
use ui_events::ScrollDelta;
use ui_events::keyboard::{KeyboardEvent, Modifiers};
use ui_events::pointer::{PointerButton, PointerEvent};
use understory_box_tree::{NodeFlags, NodeId};
use understory_event_state::click::ClickResult;
use understory_event_state::hover::HoverEvent;
use understory_focus::Navigation;
use understory_presentation::{PresentationStore, Primitive, TextPrimitive};
use understory_property::{
    DependencyObject, DependencyObjectExt, ErasedValue, Property, PropertyRegistry,
};
use understory_property_binding::{
    BindingDrainError, BindingError, BindingHost, BindingId, BindingReport, BindingWrite,
    EndpointKey, PropertyEndpoint,
};
use understory_responder::router::Router;
use understory_responder::types::{ParentLookup, Phase, WidgetLookup};
use understory_style::{MatchState, PartTag, ResolveCx, StyleCascade, Theme};
use understory_timing::{TimerDuration, TimerId, TimerInstant};

use crate::behavior::{KnownDimensions, MeasureConstraints, PresentContext};
use crate::binding_runtime::BindingRuntime;
use crate::channels::{COMPOSE, LAYOUT, PAINT, STYLE, TEMPLATE};
use crate::controls::panel::PanelWidget;
use crate::element::{Element, ElementId};
use crate::element_store::ElementStore;
use crate::event_runtime::{
    ClickEvent, DragEvent, EventRuntime, KeyDispatchResult, KeyEvent, RoutedDispatchResult,
    RoutedEventContext, RoutedEventKind, RoutedEventState, ScrollEvent,
};
use crate::focus_runtime::FocusRuntime;
use crate::layout_runtime::LayoutRuntime;
use crate::layouts::Layout;
use crate::lookup::UiResolveLookup;
use crate::presentation_runtime::PresentationRuntime;
use crate::properties::BuiltInProperties;
use crate::pseudos::{
    PSEUDO_DISABLED, PSEUDO_FOCUS_VISIBLE, PSEUDO_FOCUSED, PSEUDO_HOVER, PSEUDO_PRESSED,
};
use crate::style_runtime::StyleRuntime;
use crate::template::{TemplateContext, TemplateInstance, TemplateSlot};
use crate::template_runtime::TemplateRuntime;
use crate::text::TextEngine;
use crate::timer_runtime::{TimerEvent, TimerRuntime};
use crate::widget::Widget;

/// Public facade for an Overstory UI instance.
///
/// `Ui` keeps the app-facing API concentrated in one object while delegating
/// narrow responsibilities to internal runtimes such as [`ElementStore`] and
/// [`TemplateRuntime`].
pub struct Ui {
    elements: ElementStore,
    registry: PropertyRegistry,
    properties: BuiltInProperties,
    invalidation: InvalidationTracker<ElementId>,
    /// Box-tree geometry, hit testing, and retained presentation data.
    presentation: PresentationRuntime,
    style: StyleRuntime,
    layout: LayoutRuntime,
    /// Retained default-template expansion data for each templated owner.
    templates: TemplateRuntime,
    /// Shared text shaping engine. `RefCell` because text behaviors borrow
    /// it mutably while the owning `&Ui` borrow is otherwise immutable.
    text: RefCell<TextEngine>,
    bindings: BindingRuntime,
    events: EventRuntime,
    focus: FocusRuntime,
    timers: TimerRuntime,
}

/// Shared construction-time state for interactive controls.
///
/// `enabled` controls pointer/key activation and `:disabled` styling.
/// `focusable` is the control's desired participation in focus traversal;
/// disabled controls are temporarily removed from traversal but regain this
/// preference when re-enabled.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ControlState {
    /// Whether the control can be activated by user input.
    pub enabled: bool,
    /// Whether the control should participate in focus traversal while enabled.
    pub focusable: bool,
}

impl ControlState {
    /// Returns the default interactive state: enabled and focusable.
    #[must_use]
    #[inline]
    pub const fn interactive() -> Self {
        Self {
            enabled: true,
            focusable: true,
        }
    }

    /// Sets whether the control is enabled.
    #[must_use]
    #[inline]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Returns this state with `enabled = false`.
    #[must_use]
    #[inline]
    pub const fn disabled(self) -> Self {
        self.enabled(false)
    }

    /// Sets whether the control should participate in focus traversal.
    #[must_use]
    #[inline]
    pub const fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }
}

impl Default for ControlState {
    fn default() -> Self {
        Self::interactive()
    }
}

impl core::fmt::Debug for Ui {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Ui")
            .field("elements", &self.elements.len())
            .field("free_slots", &self.elements.free_len())
            .field("paint_dirty", &self.invalidation.has_invalidated(PAINT))
            .field("style_dirty", &self.invalidation.has_invalidated(STYLE))
            .finish_non_exhaustive()
    }
}

impl Ui {
    /// Creates a new [`Ui`] with an empty root `PanelWidget`, built-in
    /// properties registered, the generic behavior installed, and an empty
    /// cascade and theme.
    ///
    /// Use [`Self::set_cascade`] / [`Self::set_theme`] to install
    /// application-supplied styling.
    #[must_use]
    pub fn new() -> Self {
        let mut registry = PropertyRegistry::new();
        let properties = BuiltInProperties::register(&mut registry);
        // Channel-to-channel cascade in the invalidation tracker:
        // STYLE → TEMPLATE → LAYOUT → PAINT → COMPOSE. Once
        // registered, marking any upstream channel auto-marks every
        // downstream one for the same key, so call sites don't have
        // to enumerate every channel a change touches. Bindings live
        // outside this chain.
        let mut invalidation = InvalidationTracker::new();
        invalidation
            .add_cascade(STYLE, TEMPLATE)
            .expect("STYLE → TEMPLATE cascade should be acyclic");
        invalidation
            .add_cascade(TEMPLATE, LAYOUT)
            .expect("TEMPLATE → LAYOUT cascade should be acyclic");
        invalidation
            .add_cascade(LAYOUT, PAINT)
            .expect("LAYOUT → PAINT cascade should be acyclic");
        invalidation
            .add_cascade(PAINT, COMPOSE)
            .expect("PAINT → COMPOSE cascade should be acyclic");

        let elements = ElementStore::new(Box::new(PanelWidget));
        let root = elements.root();

        let mut me = Self {
            elements,
            registry,
            properties,
            invalidation,
            presentation: PresentationRuntime::new(),
            style: StyleRuntime::new(),
            layout: LayoutRuntime::new(),
            templates: TemplateRuntime::new(),
            text: RefCell::new(TextEngine::new()),
            bindings: BindingRuntime::new(),
            events: EventRuntime::new(),
            focus: FocusRuntime::new(),
            timers: TimerRuntime::new(),
        };
        me.mirror_allocated_element(root, None);
        me.style_one(root);
        me
    }

    /// Returns the id of the tree root.
    #[must_use]
    #[inline]
    pub fn root(&self) -> ElementId {
        self.elements.root()
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

    /// Returns the active style cascade.
    #[must_use]
    #[inline]
    pub fn cascade(&self) -> &StyleCascade {
        self.style.cascade()
    }

    /// Returns the active theme.
    #[must_use]
    #[inline]
    pub fn theme(&self) -> &Theme {
        self.style.theme()
    }

    /// Swaps in a new style cascade and restyles the entire tree.
    ///
    /// Every element is marked LAYOUT-dirty since styled values may
    /// now resolve differently; the channel cascade
    /// (LAYOUT → PAINT → COMPOSE) handles the rest. Match states
    /// from the previous cascade are discarded — the [`MatchState`]
    /// type is bound to its originating cascade, so they can't be
    /// reused after a swap. The LAYOUT mark catches cascades that
    /// contribute a `Layout` for an element whose previous state
    /// didn't have one — `style_one`'s diff path only triggers on
    /// state *transitions*, not first-time styling.
    pub fn set_cascade(&mut self, cascade: StyleCascade) {
        self.style.set_cascade(cascade);
        let ids = collect_preorder(self, self.root());
        for id in ids {
            self.style_one(id);
            self.invalidation.mark(id, LAYOUT);
        }
    }

    /// Swaps in a new theme. All elements are marked LAYOUT-dirty since theme
    /// resources can feed layout, text measurement, and paint values.
    pub fn set_theme(&mut self, theme: Theme) {
        self.style.set_theme(theme);
        let ids = collect_preorder(self, self.root());
        for id in ids {
            self.invalidation.mark(id, LAYOUT);
        }
    }

    /// Returns the total number of live elements (including the root).
    #[must_use]
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Returns `true` if no elements are live.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Returns a reference to the element with the given id, or `None` if the
    /// id is stale.
    #[must_use]
    pub fn get(&self, id: ElementId) -> Option<&Element> {
        self.elements.get(id)
    }

    /// Returns a mutable reference to the element with the given id.
    #[must_use]
    pub fn get_mut(&mut self, id: ElementId) -> Option<&mut Element> {
        self.elements.get_mut(id)
    }

    /// Returns the retained template expansion for `owner`, if it has one.
    #[must_use]
    pub fn template_instance(&self, owner: ElementId) -> Option<&TemplateInstance> {
        self.templates.get(owner)
    }

    /// Returns the first template part registered for `owner + tag`.
    #[must_use]
    pub fn part(&self, owner: ElementId, tag: PartTag) -> Option<ElementId> {
        self.templates.part(owner, tag)
    }

    /// Returns the first template part registered for `owner + slot`.
    #[must_use]
    pub fn slot(&self, owner: ElementId, slot: TemplateSlot) -> Option<ElementId> {
        self.templates.slot(owner, slot)
    }

    /// Returns all template parts registered for `owner + tag`.
    pub fn parts(&self, owner: ElementId, tag: PartTag) -> impl Iterator<Item = ElementId> + '_ {
        self.templates.parts(owner, tag)
    }

    /// Returns all template parts registered for `owner + slot`.
    pub fn slots(
        &self,
        owner: ElementId,
        slot: TemplateSlot,
    ) -> impl Iterator<Item = ElementId> + '_ {
        self.templates.slots(owner, slot)
    }

    /// Returns the match state for `id`, falling back to the cascade root
    /// state if the element has not yet been styled.
    #[must_use]
    pub fn match_state(&self, id: ElementId) -> MatchState {
        self.style.match_state(id)
    }

    /// Inserts a new control under `parent`. The spec's builder
    /// methods produce its configuration; `append_to` reads those,
    /// creates the element + widget, registers handlers, and
    /// returns the new element id.
    ///
    /// This is the primary way for application code to add controls
    /// to the tree. See [`crate::Button`], [`crate::ToggleSwitch`],
    /// [`crate::Slider`], [`crate::Spinner`], [`crate::Panel`],
    /// [`crate::Text`].
    pub fn append<S: crate::widget::AppendSpec>(
        &mut self,
        parent: ElementId,
        spec: S,
    ) -> ElementId {
        spec.append_to(self, parent)
    }

    /// Lower-level entry point used by [`AppendSpec`](crate::widget::AppendSpec)
    /// implementations.
    ///
    /// Allocates a new element under `parent` carrying the supplied
    /// widget (or `None` for anonymous containers), runs the
    /// widget's default template if it has one, and returns the new
    /// id. Most callers should use [`Self::append`] instead.
    pub fn append_widget(&mut self, parent: ElementId, widget: Box<dyn Widget>) -> ElementId {
        self.insert_child_inner(parent, None, None, Some(widget), true)
    }

    pub(crate) fn append_widget_deferred_template(
        &mut self,
        parent: ElementId,
        widget: Box<dyn Widget>,
    ) -> ElementId {
        self.insert_child_inner(parent, None, None, Some(widget), false)
    }

    /// Inserts an anonymous container child. Used by tests + the
    /// occasional template that needs a no-widget slot; app code
    /// should prefer [`append(parent, Panel::new())`](Self::append).
    #[cfg(test)]
    pub(crate) fn insert_child(&mut self, parent: ElementId) -> ElementId {
        self.insert_child_inner(parent, None, None, None, true)
    }

    pub(crate) fn insert_template_child(
        &mut self,
        owner: ElementId,
        parent: ElementId,
        part_tag: Option<PartTag>,
        widget: Option<Box<dyn Widget>>,
    ) -> ElementId {
        assert!(
            self.get(owner).is_some(),
            "insert_template_child: owner id is stale"
        );
        self.insert_child_inner(parent, Some(owner), part_tag, widget, true)
    }

    fn insert_child_inner(
        &mut self,
        parent: ElementId,
        templated_parent: Option<ElementId>,
        part_tag: Option<PartTag>,
        widget: Option<Box<dyn Widget>>,
        expand_template: bool,
    ) -> ElementId {
        assert!(
            self.get(parent).is_some(),
            "insert_child: parent id is stale"
        );
        let child = self
            .elements
            .insert_child(parent, templated_parent, part_tag, widget);
        self.mirror_allocated_element(child, Some(parent));
        self.style_one(child);
        // LAYOUT cascades to PAINT and COMPOSE via the tracker's
        // channel cascade. `style_one`'s diff path only marks
        // channels on state transitions — first-time styling skips
        // the diff, so without this explicit mark a cascade-supplied
        // `Layout` would sit in the cascade but never reach the layout engine.
        self.invalidation.mark(child, LAYOUT);
        self.invalidation.mark(parent, COMPOSE);
        if expand_template {
            self.expand_default_template(child);
        }
        child
    }

    /// Expands the widget's default template under `owner`, if any.
    ///
    /// Template-generated elements are inserted through [`TemplateContext`],
    /// which records named parts and applies the owner back-reference
    /// explicitly.
    pub(crate) fn expand_default_template(&mut self, owner: ElementId) {
        let Some(element) = self.get(owner) else {
            return;
        };
        let Some(template) = element.widget().and_then(|w| w.default_template()) else {
            return;
        };
        let mut cx = TemplateContext::new(self, owner);
        template.instantiate(&mut cx);
        let instance = cx.finish();
        self.templates.insert(owner, instance);
    }

    /// Removes `id` and all of its descendants from the tree.
    ///
    /// # Panics
    ///
    /// Panics if `id` is the root or stale.
    pub fn remove(&mut self, id: ElementId) {
        assert!(id != self.root(), "remove: cannot remove the tree root");
        assert!(self.get(id).is_some(), "remove: id is stale");

        if let Some(parent) = self.elements.detach_from_parent(id) {
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
            let Some(element) = self.elements.get_mut(id) else {
                return;
            };
            element.set_local_notifying(property, value, registry)
        };
        self.mark_channels(id, channels);
        self.bindings
            .mark_endpoint_changed(EndpointKey::new(id, property.id()));
    }

    /// Sets an element's layout contribution.
    ///
    /// This is the app-facing shortcut for writing
    /// [`BuiltInProperties::layout`]; custom controls may still use
    /// [`Self::set_local`] when they are working generically with
    /// dependency properties.
    pub fn set_layout(&mut self, id: ElementId, layout: impl Into<Layout>) {
        let layout_prop = self.properties.layout;
        self.set_local(id, layout_prop, layout.into());
    }

    pub(crate) fn with_plain_editor<R>(
        &self,
        editor: &mut parley::PlainEditor<peniko::Brush>,
        f: impl FnOnce(&mut parley::PlainEditorDriver<'_, peniko::Brush>) -> R,
    ) -> R {
        self.text.borrow_mut().with_plain_editor(editor, f)
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
        self.bindings.bind(source, target)
    }

    /// Drains the binding set, propagating dirty bindings through the host.
    /// Channels affected by binding writes are unioned into the invalidation
    /// tracker as a side effect of the host's `set_erased` impl.
    pub fn try_flush_bindings(&mut self) -> Result<BindingReport, BindingDrainError<ElementId>> {
        let mut bindings = self.bindings.take();
        let result = bindings.drain(self);
        self.bindings.restore(bindings);
        result
    }

    /// Drains dirty bindings and panics if a real binding evaluation error is
    /// found. Use [`Self::try_flush_bindings`] when binding graph or host type
    /// errors should be handled by the caller.
    pub fn flush_bindings(&mut self) -> BindingReport {
        self.try_flush_bindings()
            .expect("binding drain failed; use try_flush_bindings to inspect the error")
    }

    /// Adds `class` to `id` and re-styles the affected subtree.
    pub fn add_class(&mut self, id: ElementId, class: understory_style::ClassId) {
        let changed = self
            .get_mut(id)
            .map(|el| el.add_class(class))
            .unwrap_or(false);
        if changed {
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
    /// `:hover`; this is just the leaf for
    /// convenience.
    #[must_use]
    #[inline]
    pub fn hovered(&self) -> Option<ElementId> {
        self.events.hovered()
    }

    /// Returns the cursor icon preferred by the active press target
    /// or deepest hovered widget.
    ///
    /// Hosts can call this after pointer dispatch and apply the
    /// result to their platform window. `None` means the host should
    /// use its platform default.
    #[must_use]
    pub fn cursor_icon(&self) -> Option<CursorIcon> {
        self.events
            .press_target()
            .or_else(|| self.hovered())
            .and_then(|target| self.cursor_icon_for(target))
    }

    /// Hit-tests `point` against the box tree and returns the topmost,
    /// deepest element whose node covers it.
    ///
    /// Internally delegates to `understory_box_tree`'s spatial index.
    /// Returns `None` if no node is hit. Requires that the most recent
    /// geometry mutation has been committed; `compute_layout` commits as its
    /// last step, and read-only consumers in tests either go through layout or
    /// write nothing first.
    #[must_use]
    pub fn hit_test(&self, point: Point) -> Option<ElementId> {
        let hit = self.presentation.hit_test(point)?;
        self.node_to_element(hit.node)
    }

    /// Returns the element that owns a given `NodeId`, if any. Helper
    /// for callers that bridge between the box tree's id-space and
    /// the element id-space (e.g. responder integration).
    #[must_use]
    pub fn node_to_element(&self, node: NodeId) -> Option<ElementId> {
        self.presentation.node_to_element(node)
    }

    /// Returns the primary box-tree node for `id`, or `None` for
    /// stale ids.
    #[must_use]
    pub fn box_for(&self, id: ElementId) -> Option<NodeId> {
        self.presentation.box_for(id)
    }

    /// Applies shared interactive control state to `id`.
    ///
    /// This is the canonical setup path for built-in controls. It stores the
    /// desired focusability, writes `IsEnabled`, updates `:disabled`, and keeps
    /// disabled controls out of focus traversal.
    pub fn set_control_state(&mut self, id: ElementId, state: ControlState) {
        self.set_focusable(id, state.focusable);
        self.set_enabled(id, state.enabled);
    }

    /// Toggles the desired `Focusable` flag for `id`.
    ///
    /// Focusable, enabled elements participate in [`Self::focus_navigate`] and
    /// can receive keyboard events via [`Self::on_key`]. If an element is
    /// disabled, the preference is retained but its box-tree focus flag stays
    /// cleared until it is re-enabled. No-op for stale ids.
    pub fn set_focusable(&mut self, id: ElementId, focusable: bool) {
        if self.get(id).is_none() {
            return;
        }
        self.focus.set_focusable_preference(id, focusable);
        self.apply_focusable_preference(id);
        if !focusable && self.focus.focused() == Some(id) {
            self.set_focus(None);
        }
    }

    /// Returns whether `id` is currently eligible for focus traversal.
    #[must_use]
    pub fn is_focusable(&self, id: ElementId) -> bool {
        let Some(primary) = self.box_for(id) else {
            return false;
        };
        self.presentation
            .flags(primary)
            .map(|flags| flags.contains(NodeFlags::FOCUSABLE))
            .unwrap_or(false)
    }

    fn apply_focusable_preference(&mut self, id: ElementId) {
        let focusable = self.focus.focusable_preference(id) && self.is_enabled(id);
        self.set_focusable_flag(id, focusable);
    }

    fn set_focusable_flag(&mut self, id: ElementId, focusable: bool) {
        let Some(primary) = self.box_for(id) else {
            return;
        };
        let mut flags = self.presentation.flags(primary).unwrap_or_default();
        if focusable {
            flags.insert(NodeFlags::FOCUSABLE);
        } else {
            flags.remove(NodeFlags::FOCUSABLE);
        }
        self.presentation.set_flags(primary, flags);
    }

    /// Returns whether `id` is currently enabled.
    #[must_use]
    pub fn is_enabled(&self, id: ElementId) -> bool {
        self.get(id)
            .map(|element| self.effective_value_for(element, self.properties.is_enabled))
            .unwrap_or(false)
    }

    /// Enables or disables a control element.
    ///
    /// Disabling clears focus and the current press target for `id`, removes it
    /// from focus traversal, and applies `:disabled` for style matching.
    pub fn set_enabled(&mut self, id: ElementId, enabled: bool) {
        if self.get(id).is_none() {
            return;
        }
        self.set_local(id, self.properties.is_enabled, enabled);
        if enabled {
            self.remove_pseudo(id, PSEUDO_DISABLED);
            self.apply_focusable_preference(id);
        } else {
            self.add_pseudo(id, PSEUDO_DISABLED);
            self.set_focusable_flag(id, false);
            if self.focus.focused() == Some(id) {
                self.set_focus(None);
            }
            if self.events.press_target() == Some(id) {
                self.set_press_target(None);
            }
        }
    }

    /// Returns the element that currently has focus, if any.
    #[must_use]
    #[inline]
    pub fn focused(&self) -> Option<ElementId> {
        self.focus.focused()
    }

    /// Sets keyboard-visible focus. Passes `None` to clear focus. Disabled or
    /// stale target ids are treated as `None`. Applies / removes `:focused`
    /// and `:focus-visible` on the affected elements. Returns `true` if the
    /// focus state changed.
    pub fn set_focus(&mut self, new: Option<ElementId>) -> bool {
        self.set_focus_with_visibility(new, true)
    }

    pub(crate) fn set_pointer_focus(&mut self, new: Option<ElementId>) -> bool {
        self.set_focus_with_visibility(new, false)
    }

    fn set_focus_with_visibility(&mut self, new: Option<ElementId>, focus_visible: bool) -> bool {
        let accepted = new.filter(|id| self.is_enabled(*id));
        let mut changed = false;
        if self.focus.focused() == accepted {
            if let Some(id) = accepted {
                changed |= self.set_focus_visible_pseudo(id, focus_visible);
            }
            return changed;
        }
        if let Some(old_id) = self.focus.take_focused() {
            self.remove_pseudo(old_id, PSEUDO_FOCUSED);
            self.remove_pseudo(old_id, PSEUDO_FOCUS_VISIBLE);
        }
        if let Some(new_id) = accepted {
            self.add_pseudo(new_id, PSEUDO_FOCUSED);
            self.set_focus_visible_pseudo(new_id, focus_visible);
            self.focus.set_focused(Some(new_id));
            changed = true;
        } else {
            self.focus.set_focused(None);
            changed = true;
        }
        changed
    }

    fn set_focus_visible_pseudo(&mut self, id: ElementId, focus_visible: bool) -> bool {
        let had_visible = self
            .get(id)
            .map(|element| element.pseudos().contains(&PSEUDO_FOCUS_VISIBLE))
            .unwrap_or(false);
        if focus_visible {
            self.add_pseudo(id, PSEUDO_FOCUS_VISIBLE);
        } else {
            self.remove_pseudo(id, PSEUDO_FOCUS_VISIBLE);
        }
        had_visible != focus_visible
    }

    /// Moves focus per `direction` (e.g. [`Navigation::Next`] for
    /// Tab, [`Navigation::Prev`] for Shift+Tab). Uses
    /// [`understory_focus::DefaultPolicy`] with `WrapMode::Scope` so
    /// focus cycles within the tree. Returns the new focused element.
    pub fn focus_navigate(&mut self, direction: Navigation) -> Option<ElementId> {
        self.presentation.commit();
        let root_node = self.box_for(self.root())?;
        // Resolve the current focus's node before we borrow `tree`
        // and `focus_buf` to build the FocusSpace.
        let current_node = self.focus.focused().and_then(|id| self.box_for(id));
        let new_node =
            self.focus
                .navigate(self.presentation.tree(), root_node, current_node, direction);
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
        self.events.on_key(id, Box::new(handler));
    }

    /// Removes a key handler previously registered with
    /// [`Self::on_key`]. No-op if none is registered.
    pub fn remove_key_handler(&mut self, id: ElementId) {
        self.events.remove_key(id);
    }

    /// Runs `dispatch_current` across the responder route for `source`.
    ///
    /// Route construction belongs to [`understory_responder::Router`]; this
    /// helper owns only Overstory's execution semantics: temporarily remove a
    /// handler before invoking it, share routed event state across invocations,
    /// and stop the ordinary route once an event is handled or propagation is
    /// explicitly stopped.
    fn dispatch_routed(
        &mut self,
        kind: RoutedEventKind,
        source: ElementId,
        mut dispatch_current: impl FnMut(&mut Self, RoutedEventContext<'_>),
    ) -> RoutedDispatchResult {
        let state = RoutedEventState::new();
        for (phase, node) in self.responder_sequence(source) {
            let route = RoutedEventContext::new(kind, source, node, phase, &state);
            dispatch_current(self, route);
            if state.should_stop_route() {
                break;
            }
        }
        RoutedDispatchResult::from_state(&state)
    }

    fn responder_sequence(&self, source: ElementId) -> Vec<(Phase, ElementId)> {
        let lookup = UiWidgetLookup;
        let parent = UiParentLookup { ui: self };
        let router: Router<ElementId, UiWidgetLookup, UiParentLookup<'_>> =
            Router::with_parent(lookup, parent);
        router
            .dispatch_for::<()>(source)
            .into_iter()
            .map(|d| (d.phase, d.node))
            .collect()
    }

    /// Routes `event` through the framework keyboard policy and,
    /// when appropriate, to the currently focused element.
    ///
    /// Tab / Shift+Tab focus navigation is handled by `Ui` before
    /// ordinary key handlers run. Other keys route along the
    /// responder capture → target → bubble path rooted at the
    /// focused element. A handler can call [`KeyEvent::mark_handled`]
    /// to stop the remaining route.
    #[must_use]
    pub fn dispatch_key(&mut self, event: &KeyboardEvent) -> KeyDispatchResult {
        let previous_focus = self.focus.focused();
        if let Some(direction) = focus_navigation_for_key(event) {
            let focused = self.focus_navigate(direction);
            return KeyDispatchResult {
                handled: focused.is_some(),
                propagation_stopped: false,
                focus_changed: previous_focus != self.focus.focused(),
                focused: self.focus.focused(),
            };
        }

        let Some(focused) = self.focus.focused() else {
            return KeyDispatchResult::default();
        };
        let routed = self.dispatch_routed(RoutedEventKind::Key, focused, |ui, route| {
            let node = route.current;
            let Some(mut handler) = ui.events.take_key(node) else {
                return;
            };
            let ev = KeyEvent { route, key: event };
            ui.events.begin_key(node);
            handler(ui, ev);
            ui.events.end_key(node);
            let handler_removed = ui.events.key_removed(node);
            if ui.get(node).is_some() && !handler_removed && !ui.events.has_key(node) {
                ui.events.insert_key_if_absent(node, handler);
            }
        });

        KeyDispatchResult {
            handled: routed.handled,
            propagation_stopped: routed.propagation_stopped,
            focus_changed: previous_focus != self.focus.focused(),
            focused: self.focus.focused(),
        }
    }

    /// Returns the host-supplied monotonic instant currently cached
    /// on the `Ui`. Updated whenever an event is dispatched or
    /// timers are pumped. Templates and handlers can use this to
    /// schedule follow-on timers without threading `now` through
    /// every call.
    #[must_use]
    #[inline]
    pub fn now(&self) -> TimerInstant {
        self.timers.now()
    }

    /// Sets the cached monotonic "now" used by relative timer APIs.
    /// Hosts can call this explicitly, but it's also updated by
    /// [`Self::pump_timers`], [`Self::dispatch_pointer_event`] (via
    /// the pointer state's timestamp), and [`Self::dispatch_key`].
    pub fn set_now(&mut self, now: TimerInstant) {
        self.timers.set_now(now);
    }

    /// Schedules a one-shot timer targeted at `target`. Returns the
    /// queue's id so callers can cancel it. The deadline is computed
    /// relative to [`Self::now`].
    pub fn schedule_timer_once(&mut self, target: ElementId, delay: TimerDuration) -> TimerId {
        self.timers.schedule_once(target, delay)
    }

    /// Schedules a repeating timer targeted at `target` that
    /// coalesces missed intervals (the typical UI-animation policy
    /// from [`understory_timing::TimerRepeat::coalescing`]). The first fire is `delay`
    /// from now; subsequent fires are `delay` apart relative to the
    /// previous drain time.
    pub fn schedule_timer_repeating(&mut self, target: ElementId, delay: TimerDuration) -> TimerId {
        self.timers.schedule_repeating(target, delay)
    }

    /// Cancels a pending timer. Returns `true` if a pending timer
    /// was removed. Idempotent.
    pub fn cancel_timer(&mut self, timer: TimerId) -> bool {
        self.timers.cancel(timer)
    }

    /// Returns the next pending timer deadline, if any. Hosts use
    /// this to arm their wakeup mechanism (e.g.
    /// `winit::ControlFlow::WaitUntil`).
    #[must_use]
    pub fn next_timer_deadline(&self) -> Option<TimerInstant> {
        self.timers.next_deadline()
    }

    /// Registers a timer handler on `id`. Fires whenever any timer
    /// scheduled against `id` reaches its deadline and is drained by
    /// [`Self::pump_timers`]. One handler per element; subsequent
    /// calls replace.
    pub fn on_timer<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Self, TimerEvent) + 'static,
    {
        self.timers.on_handler(id, Box::new(handler));
    }

    /// Removes a timer handler previously registered with
    /// [`Self::on_timer`]. No-op if none is registered.
    pub fn remove_timer_handler(&mut self, id: ElementId) {
        self.timers.remove_handler(id);
    }

    /// Drains every timer due at or before `now`, dispatching each
    /// to its target's registered handler. Repeating timers re-arm
    /// only when the target is still alive and still has a timer
    /// handler after dispatch. Updates the cached [`Self::now`] to
    /// `now`.
    pub fn pump_timers(&mut self, now: TimerInstant) {
        self.timers.set_now(now);
        let mut rearm = Vec::new();
        while let Some(timer) = self.timers.pop_expired(now) {
            let target = *timer.target();
            let id = timer.id();
            let repeating = timer.repeat().is_repeating();
            let mut should_rearm = false;
            // Take the handler out so it can borrow `&mut Ui`; put
            // back unless the handler removed itself or the element
            // vanished.
            if let Some(mut handler) = self.timers.take_handler(target) {
                let event = TimerEvent {
                    target,
                    timer: id,
                    now,
                };
                let previous_dispatching = self.timers.begin_dispatch(target);
                handler(self, event);
                self.timers.end_dispatch(previous_dispatching);

                let target_alive = self.get(target).is_some();
                let handler_removed = self.timers.take_handler_removed(target);
                if target_alive {
                    if !handler_removed && !self.timers.has_handler(target) {
                        self.timers.insert_handler_if_absent(target, handler);
                    }
                    should_rearm = repeating && self.timers.has_handler(target);
                }
            }
            if should_rearm {
                rearm.push(timer);
            }
        }
        for timer in rearm {
            self.timers.rearm(timer);
        }
    }

    /// Registers a click handler on `id`. The handler runs along the
    /// responder capture → target → bubble sequence when
    /// the click recognizer reports a primary-button click whose target
    /// path includes this element. One handler per element for now;
    /// subsequent calls replace the previous handler.
    pub fn on_click<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Self, ClickEvent<'_>) + 'static,
    {
        self.events.on_click(id, Box::new(handler));
    }

    /// Removes a click handler previously registered with
    /// [`Self::on_click`]. No-op if no handler is registered.
    pub fn remove_click_handler(&mut self, id: ElementId) {
        self.events.remove_click(id);
    }

    /// Registers a drag handler on `id`. Fires on every
    /// primary-button `Move` while the press is active, along the
    /// responder capture → target → bubble path rooted at the
    /// press target. Useful for sliders, resizers, draggable
    /// items. One handler per element; subsequent calls replace.
    pub fn on_drag<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Self, DragEvent<'_>) + 'static,
    {
        self.events.on_drag(id, Box::new(handler));
    }

    /// Removes a drag handler previously registered with
    /// [`Self::on_drag`]. No-op if no handler is registered.
    pub fn remove_drag_handler(&mut self, id: ElementId) {
        self.events.remove_drag(id);
    }

    /// Registers a scroll handler on `id`. The handler runs along the
    /// responder capture → target → bubble sequence rooted at the element
    /// under the pointer when a scroll event arrives.
    pub fn on_scroll<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Self, ScrollEvent<'_>) + 'static,
    {
        self.events.on_scroll(id, Box::new(handler));
    }

    /// Removes a scroll handler previously registered with [`Self::on_scroll`].
    /// No-op if no handler is registered.
    pub fn remove_scroll_handler(&mut self, id: ElementId) {
        self.events.remove_scroll(id);
    }

    /// Builds the responder dispatch sequence for the current
    /// press target and runs each drag handler along it. Called
    /// from `dispatch_pointer_event` on every `Move` while a press
    /// is active.
    fn dispatch_drag(&mut self, position: Point) -> RoutedDispatchResult {
        let Some(source) = self.events.press_target() else {
            return RoutedDispatchResult::default();
        };
        let Some(press_position) = self.events.press_position() else {
            return RoutedDispatchResult::default();
        };
        let last = self.events.last_drag_position().unwrap_or(press_position);
        let delta = position - last;
        self.events.set_last_drag_position(position);

        self.dispatch_routed(RoutedEventKind::Drag, source, |ui, route| {
            let node = route.current;
            let Some(mut handler) = ui.events.take_drag(node) else {
                return;
            };
            let event = DragEvent {
                route,
                position,
                press_position,
                delta,
            };
            ui.events.begin_drag(node);
            handler(ui, event);
            ui.events.end_drag(node);
            let handler_removed = ui.events.drag_removed(node);
            if ui.get(node).is_some() && !handler_removed && !ui.events.has_drag(node) {
                ui.events.insert_drag_if_absent(node, handler);
            }
        })
    }

    /// Programmatically synthesize a click on `target` at
    /// `position`. Used by widgets that translate non-pointer input
    /// (e.g. Button's Space / Enter key activation) into the same
    /// click dispatch the pointer pipeline would produce.
    pub fn synthesize_click(&mut self, target: ElementId, position: Point) -> RoutedDispatchResult {
        self.dispatch_click(target, position)
    }

    /// Programmatically synthesize a click with explicit keyboard
    /// modifiers.
    pub fn synthesize_click_with_modifiers(
        &mut self,
        target: ElementId,
        position: Point,
        modifiers: Modifiers,
    ) -> RoutedDispatchResult {
        self.dispatch_click_with_modifiers(target, position, modifiers)
    }

    /// Builds the responder dispatch sequence for `target` and runs
    /// each element's click handler along it (capture, then target,
    /// then bubble). Called when the click recognizer returns
    /// [`ClickResult::Click`] or by [`Self::synthesize_click`].
    fn dispatch_click(&mut self, target: ElementId, position: Point) -> RoutedDispatchResult {
        self.dispatch_click_with_modifiers(target, position, Modifiers::default())
    }

    fn dispatch_click_with_modifiers(
        &mut self,
        target: ElementId,
        position: Point,
        modifiers: Modifiers,
    ) -> RoutedDispatchResult {
        if !self.is_enabled(self.press_visual_target(target)) {
            return RoutedDispatchResult::default();
        }

        self.dispatch_routed(RoutedEventKind::Click, target, |ui, route| {
            let node = route.current;
            // Take the handler out so it can call `&mut self`, then
            // put it back unless the handler removed itself.
            let Some(mut handler) = ui.events.take_click(node) else {
                return;
            };
            let event = ClickEvent {
                route,
                position,
                modifiers,
            };
            ui.events.begin_click(node);
            handler(ui, event);
            ui.events.end_click(node);
            let handler_removed = ui.events.click_removed(node);
            // Reinstate iff the element is still alive AND the
            // handler didn't remove or replace itself during the call.
            if ui.get(node).is_some() && !handler_removed && !ui.events.has_click(node) {
                ui.events.insert_click_if_absent(node, handler);
            }
        })
    }

    /// Programmatically synthesize a scroll routed at `target`.
    pub fn synthesize_scroll(
        &mut self,
        target: ElementId,
        position: Point,
        delta: ScrollDelta,
    ) -> RoutedDispatchResult {
        self.dispatch_scroll_with_modifiers(target, position, delta, Modifiers::default())
    }

    fn dispatch_scroll_with_modifiers(
        &mut self,
        target: ElementId,
        position: Point,
        delta: ScrollDelta,
        modifiers: Modifiers,
    ) -> RoutedDispatchResult {
        if !self.is_enabled(self.press_visual_target(target)) {
            return RoutedDispatchResult::default();
        }

        self.dispatch_routed(RoutedEventKind::Scroll, target, |ui, route| {
            let node = route.current;
            let Some(mut handler) = ui.events.take_scroll(node) else {
                return;
            };
            let event = ScrollEvent {
                route,
                position,
                delta,
                modifiers,
            };
            ui.events.begin_scroll(node);
            handler(ui, event);
            ui.events.end_scroll(node);
            let handler_removed = ui.events.scroll_removed(node);
            if ui.get(node).is_some() && !handler_removed && !ui.events.has_scroll(node) {
                ui.events.insert_scroll_if_absent(node, handler);
            }
        })
    }

    /// Resolves the effective `Bounds` for `id` through the cascade.
    #[must_use]
    pub fn effective_bounds(&self, id: ElementId) -> Rect {
        let Some(element) = self.get(id) else {
            return Rect::ZERO;
        };
        self.effective_value_for(element, self.properties.bounds)
    }

    pub(crate) fn effective_value_for<T: Clone + 'static>(
        &self,
        element: &Element,
        property: Property<T>,
    ) -> T {
        ResolveCx::new(
            &self.registry,
            self.style.theme(),
            UiResolveLookup::new(self),
        )
        .get_value(
            element,
            property,
            Some((self.style.cascade(), self.match_state(element.id()))),
        )
    }

    /// Dispatches a normalized pointer event.
    ///
    /// Handles hover, primary-button press/click/drag, and routed scroll.
    /// Gesture and non-primary button presses are ignored for now.
    ///
    /// `:pressed` follows WPF semantics: a primary-button press marks the
    /// hit element pressed and that element stays pressed even if the cursor
    /// leaves it (no capture API yet, but the visual state survives). The
    /// matching Up — wherever it lands — clears the press.
    ///
    /// Returns `true` if the dispatch caused a state change.
    pub fn dispatch_pointer_event(&mut self, event: &PointerEvent) -> bool {
        // Update the cached `now` from whichever PointerState the
        // event carries — pointer events all expose a `time` in
        // nanoseconds. Handlers and templates can use `Ui::now()`
        // without threading it through every call.
        if let Some(t) = pointer_event_time(event) {
            self.timers.set_now(t);
        }
        match event {
            PointerEvent::Move(update) => {
                let pos = update.current.logical_point();
                let mut changed = self.update_hover_at(pos);
                // If a primary press is active, fan the move out to
                // drag handlers along the press target's responder
                // path before returning. We can't tell whether any
                // handler actually changed state, but redrawing on a
                // hover change is sufficient for drag-induced
                // updates because the drag handler typically writes
                // properties that mark COMPOSE.
                if self.events.press_target().is_some() {
                    self.dispatch_drag(pos);
                    changed = true;
                }
                changed
            }
            PointerEvent::Down(button_event) => {
                if button_event.button != Some(PointerButton::Primary) {
                    return false;
                }
                let pos = button_event.state.logical_point();
                let time_ms = button_event.state.time / 1_000_000;
                // A Down without a preceding Move can arrive on some
                // platforms; refresh hover first so any newly-entered
                // path is set up before we press.
                let mut changed = self.update_hover_at(pos);
                if let Some(target) = self.hit_test(pos) {
                    let visual = self.press_visual_target(target);
                    if !self.is_enabled(visual) {
                        self.events.cancel_click();
                        changed |= self.set_press_target(None);
                        self.events.clear_press();
                        return changed;
                    }
                    if self.is_focusable(visual) {
                        changed |= self.set_pointer_focus(Some(visual));
                    }
                    self.events.click_down(target, pos, time_ms);
                    changed |= self.set_press_target(Some(visual));
                    self.events.start_press(pos);
                } else {
                    changed |= self.set_press_target(None);
                    self.events.clear_press();
                }
                changed
            }
            PointerEvent::Up(button_event) => {
                if button_event.button != Some(PointerButton::Primary) {
                    return false;
                }
                let pos = button_event.state.logical_point();
                let time_ms = button_event.state.time / 1_000_000;
                let hit = self.hit_test(pos);
                // Hand the up to the click recognizer — produces a
                // `ClickResult::Click(target)` when the press is
                // recognized; dispatch handlers along the responder
                // path for that target.
                let result = match hit {
                    Some(target) => self.events.click_up(&target, pos, time_ms),
                    None => {
                        // Up off any element — cancel any open press
                        // so it doesn't dangle and pretend to click
                        // later.
                        self.events.cancel_click();
                        ClickResult::Suppressed(None)
                    }
                };
                let press_changed = self.set_press_target(None);
                self.events.clear_press();
                if let ClickResult::Click(target) = result {
                    self.dispatch_click_with_modifiers(target, pos, button_event.state.modifiers);
                }
                press_changed
            }
            PointerEvent::Scroll(scroll_event) => {
                let pos = scroll_event.state.logical_point();
                let changed = self.update_hover_at(pos);
                let result = self
                    .hit_test(pos)
                    .map(|target| {
                        self.dispatch_scroll_with_modifiers(
                            target,
                            pos,
                            scroll_event.delta,
                            scroll_event.state.modifiers,
                        )
                    })
                    .unwrap_or_default();
                changed || result.handled || result.propagation_stopped
            }
            PointerEvent::Leave(_) => self.clear_hover(),
            PointerEvent::Cancel(_) => {
                self.events.cancel_click();
                let h = self.clear_hover();
                let p = self.set_press_target(None);
                self.events.clear_press();
                h || p
            }
            _ => false,
        }
    }

    /// Updates the hover state to reflect a pointer at `pos`. Computes
    /// the element path from the box tree's hit-test result and feeds
    /// it to the event runtime, applying its `Enter`/`Leave` events to
    /// pseudoclass state.
    fn update_hover_at(&mut self, pos: Point) -> bool {
        let path: Vec<ElementId> = self
            .presentation
            .hit_test(pos)
            .map(|hit| {
                hit.path
                    .into_iter()
                    .filter_map(|n| self.node_to_element(n))
                    .collect()
            })
            .unwrap_or_default();
        let events = self.events.update_hover_path(&path);
        self.apply_hover_events(events)
    }

    /// Clears the hover state (e.g. on pointer Leave). Returns `true`
    /// if anything changed.
    fn clear_hover(&mut self) -> bool {
        let events = self.events.clear_hover();
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

    /// Resolves a hit element to the element that should carry the
    /// `:pressed` pseudoclass.
    ///
    /// Hit-testing returns the deepest box under the pointer, which
    /// for a templated control (Button → label `Text`) is the
    /// template part rather than the control itself. Cascade rules
    /// keyed on the control's `TypeTag` (`TYPE_BUTTON:pressed`, …)
    /// would never match that part. To match WPF, climb the
    /// `templated_parent` chain to the first ancestor whose widget
    /// owns a default template — that ancestor is the templated
    /// control the part belongs to. Non-template hits return
    /// themselves.
    fn press_visual_target(&self, hit: ElementId) -> ElementId {
        let mut current = hit;
        loop {
            let Some(element) = self.get(current) else {
                return hit;
            };
            if element
                .widget()
                .and_then(|w| w.default_template())
                .is_some()
            {
                return current;
            }
            match element.templated_parent() {
                Some(parent) => current = parent,
                None => return current,
            }
        }
    }

    fn cursor_icon_for(&self, target: ElementId) -> Option<CursorIcon> {
        let mut current = Some(target);
        while let Some(id) = current {
            let element = self.get(id)?;
            if let Some(icon) = element
                .widget()
                .and_then(|widget| widget.cursor_icon(element))
            {
                return Some(icon);
            }
            current = element.templated_parent().or_else(|| element.parent());
        }
        None
    }

    fn set_press_target(&mut self, new: Option<ElementId>) -> bool {
        if self.events.press_target() == new {
            return false;
        }
        if let Some(old_id) = self.events.take_press_target() {
            self.remove_pseudo(old_id, PSEUDO_PRESSED);
        }
        if let Some(new_id) = new {
            self.add_pseudo(new_id, PSEUDO_PRESSED);
        }
        self.events.set_press_target(new);
        true
    }

    /// Resolves the effective `Layout` for `id` through the cascade.
    #[must_use]
    pub fn effective_layout(&self, id: ElementId) -> Layout {
        let Some(element) = self.get(id) else {
            return Layout::default();
        };
        self.effective_value_for(element, self.properties.layout)
    }

    /// Computes layout for the tree against the given viewport size and
    /// writes each element's `Bounds` (via the cascade-resolved `Layout`).
    ///
    /// Drains pending property bindings first so binding-driven writes
    /// participate in this frame's layout. Then flushes the LAYOUT
    /// channel: any element whose `Layout` has changed gets its engine style
    /// pushed before layout is recomputed. `Bounds` writes use the standard
    /// `set_local_notifying` path so PAINT cascades naturally for elements
    /// whose box changed.
    pub fn compute_layout(&mut self, viewport: Size) {
        // Bindings first — they may dirty LAYOUT/PAINT for their targets.
        self.flush_bindings();
        // Push any pending Layout changes into the layout engine. We collect
        // first to separate the immutable cascade walk from the mutable engine
        // update.
        let dirty: Vec<ElementId> = self.invalidation.drain_sorted(LAYOUT).collect();
        let updates: Vec<(taffy::NodeId, taffy::Style)> = dirty
            .into_iter()
            .filter_map(|id| {
                let taffy_node = self.layout.node_for(id)?;
                let style = self.effective_layout(id).to_taffy();
                Some((taffy_node, style))
            })
            .collect();
        for (node, style) in updates {
            self.layout.set_style(node, style);
        }

        let Some(root_taffy) = self.layout.node_for(self.root()) else {
            return;
        };
        let available = taffy::Size {
            #[expect(clippy::cast_possible_truncation, reason = "viewport size fits f32")]
            width: taffy::AvailableSpace::Definite(viewport.width as f32),
            #[expect(clippy::cast_possible_truncation, reason = "viewport size fits f32")]
            height: taffy::AvailableSpace::Definite(viewport.height as f32),
        };

        // Run taffy with a measure callback that dispatches to the elements'
        // widgets. Split-borrow the disjoint Ui fields so the closure can
        // read everything it needs while the taffy tree is &mut.
        {
            let taffy = self.layout.taffy_mut();
            let registry = &self.registry;
            let properties = &self.properties;
            let style = &self.style;
            let elements = &self.elements;
            let text = &self.text;
            let _ = taffy.compute_layout_with_measure(
                root_taffy,
                available,
                |known, avail, _node_id, ctx_id, _style| {
                    let Some(&id) = ctx_id.map(|v| &*v) else {
                        return taffy::Size::ZERO;
                    };
                    let Some(element) = elements.get(id) else {
                        return taffy::Size::ZERO;
                    };
                    let Some(widget) = element.widget() else {
                        // Anonymous container: defer to taffy's
                        // default style-based sizing.
                        return taffy::Size::ZERO;
                    };
                    let match_state = style.match_state(id);
                    let measure_ctx = crate::behavior::MeasureContext {
                        registry,
                        properties,
                        theme: style.theme(),
                        cascade: style.cascade(),
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
                    match widget.measure(element, &measure_ctx, constraints) {
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
        self.write_bounds_recursive(self.root(), Point::ZERO);
        // Finalize the box tree: refresh world-space data and the
        // spatial index so the next `hit_test` (or any other tree
        // query) reflects this frame's geometry.
        self.presentation.commit();
    }

    fn write_bounds_recursive(&mut self, id: ElementId, parent_origin: Point) {
        let Some(taffy_node) = self.layout.node_for(id) else {
            return;
        };
        let Some(layout) = self.layout.layout(taffy_node) else {
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
        let child_offset = self
            .get(id)
            .and_then(|element| {
                element
                    .widget()
                    .map(|widget| widget.child_layout_offset(element))
            })
            .unwrap_or(Vec2::ZERO);
        let clips_subtree = self
            .get(id)
            .and_then(|element| element.widget().map(|widget| widget.clips_subtree(element)))
            .unwrap_or(false);

        // Mirror the world-space bounds into the box tree's primary
        // node for this element. Since every node is currently a
        // tree root with IDENTITY transform, `local_bounds == world_bounds`
        // and hit testing matches the old element-bounds walk.
        if let Some(primary) = self.box_for(id) {
            self.presentation.set_local_bounds(primary, rect);
            let clip = clips_subtree.then(|| RoundedRect::from_rect(rect, 0.0));
            self.presentation.set_local_clip(primary, clip);
        }

        let children: Vec<ElementId> = self
            .get(id)
            .map(|el| el.children().to_vec())
            .unwrap_or_default();
        let child_origin = origin + child_offset;
        for child in children {
            self.write_bounds_recursive(child, child_origin);
        }

        if let Some(element) = self.get(id)
            && let Some(widget) = element.widget()
        {
            let ctx = crate::behavior::LayoutContext { ui: self };
            widget.layout_updated(element, &ctx);
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
        self.flush_bindings();
        self.flush_style();
        // Make sure the box tree's world-space data and spatial index
        // reflect any structural / bounds changes since the last
        // commit. `compute_layout` already commits at the end, so
        // when paint follows layout this is a cheap no-op; tests
        // that paint without layout (or callers exploring partial
        // pipelines) get a correct commit on demand.
        self.presentation.commit();
        let dirty: Vec<ElementId> = self.invalidation.drain_sorted(PAINT).collect();
        for id in dirty {
            self.present_element(id);
        }
        // We don't yet drive incremental re-lowering off this, but
        // draining keeps the dirty set bounded.
        let _: Vec<NodeId> = self.presentation.drain_dirty();

        output.clear();
        if let Some(root_box) = self.box_for(self.root()) {
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
        // both a `&Ui` borrow (for `UiResolveLookup`) and a
        // `&mut PresentationStore` simultaneously. Always put it back
        // before returning.
        let mut presentation = self.presentation.take_presentation();
        self.present_element_inner(id, &mut presentation);
        self.presentation.restore_presentation(presentation);
    }

    fn present_element_inner(
        &self,
        id: ElementId,
        presentation: &mut PresentationStore<NodeId, ElementId>,
    ) {
        let Some(element) = self.get(id) else {
            return;
        };
        let Some(node) = self.box_for(id) else {
            return;
        };
        let match_state = self.style.match_state(id);
        let ctx = PresentContext {
            registry: &self.registry,
            properties: &self.properties,
            theme: self.style.theme(),
            cascade: self.style.cascade(),
            match_state,
            lookup: UiResolveLookup::new(self),
        };
        if let Some(widget) = element.widget() {
            widget.present(element, &ctx, presentation, node);
        } else {
            // Anonymous container: still render its surface from the
            // cascade so panels with `Background` rules paint.
            crate::generic::present_surface(element, &ctx, presentation, node);
        }
    }

    /// Recursively walks the box tree depth-first, lowering each
    /// node's presentation primitives into `output` at its world
    /// bounds.
    fn lower_subtree(&self, node: NodeId, output: &mut Scene) {
        let clip = self.presentation.local_clip(node).flatten();
        let had_clip = clip.is_some();
        if let Some(clip) = clip {
            output.push_clip(Clip::Fill {
                transform: self
                    .presentation
                    .world_transform(node)
                    .unwrap_or(Affine::IDENTITY),
                shape: Geometry::RoundedRect(clip),
                fill_rule: peniko::Fill::NonZero,
            });
        }
        self.lower_one(node, output);
        let children: Vec<NodeId> = self.presentation.children_of(node).to_vec();
        for child in children {
            self.lower_subtree(child, output);
        }
        if had_clip {
            output.pop_clip();
        }
    }

    fn lower_one(&self, node: NodeId, output: &mut Scene) {
        let Some(bounds) = self.presentation.world_bounds(node) else {
            return;
        };
        if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
            return;
        }
        let Some(presentation) = self.presentation.presentation(node) else {
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
    /// Elements are entered from their parent match state. The style matcher
    /// starts selectors at every subject, so single-step rules still apply to
    /// nested controls while child/descendant selectors can carry owner state
    /// into template parts.
    fn style_one(&mut self, id: ElementId) {
        let Some(element) = self.elements.get(id) else {
            self.style.remove(id);
            return;
        };
        let channels = self.style.style_one(id, element, &self.registry);
        if let Some(channels) = channels {
            self.mark_channels(id, channels);
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
        let _ = self.elements.remove_slot(id);
        self.style.remove(id);
        self.invalidation.remove_key(id);
        self.bindings.clear_owner(id);
        self.templates.remove_element(id);
        self.events.remove_target(id);
        self.timers.remove_target(id);
        self.focus.remove_target(id);
        self.layout.remove_element(id);
        self.presentation.remove_element(id);
    }

    fn mirror_allocated_element(&mut self, id: ElementId, parent: Option<ElementId>) {
        self.layout.insert_element(id, parent);

        self.presentation.insert_element(id, parent);
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
            // Disjoint-field split-borrow: registry is read-only while the
            // element store mutates only the target element.
            let registry = &self.registry;
            let Some(element) = self.elements.get_mut(id) else {
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
fn text_max_advance_for_bounds(bounds: Rect) -> Option<f32> {
    crate::text::max_advance_for_logical_width(bounds.width())
}

fn plain_text_wraps(plain: &understory_presentation::PlainTextPrimitive) -> bool {
    plain.style.text_wrap_mode != understory_presentation::TextStyle::default().text_wrap_mode
}

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
                // Border. We only lower a uniform border for now —
                // the same brush + width on all four sides. The top
                // side carries the source of truth and the other
                // sides are ignored. Stroked centered on the surface
                // edge, which leaks half the width outside `bounds`;
                // a more polished pass would inset by `width / 2`.
                let side = &surface.border.top;
                if let Some(brush) = side.brush.as_ref()
                    && side.width > 0.0
                {
                    let stroke = kurbo::Stroke::new(side.width);
                    if corner_radius > 0.0 {
                        let rounded = RoundedRect::from_rect(bounds, corner_radius);
                        painter.stroke(rounded, &stroke, brush).draw();
                    } else {
                        painter.stroke(bounds, &stroke, brush).draw();
                    }
                }
                // Shadow lowering — TODO when controls need it.
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
                        if plain_text_wraps(plain) {
                            text_max_advance_for_bounds(bounds)
                        } else {
                            None
                        },
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

/// Extracts the host-supplied monotonic time from a pointer event,
/// if the variant carries one. Pointer events all expose a
/// nanosecond timestamp on the underlying `PointerState`.
fn pointer_event_time(event: &PointerEvent) -> Option<TimerInstant> {
    match event {
        PointerEvent::Move(update) => Some(update.current.time),
        PointerEvent::Down(b) | PointerEvent::Up(b) => Some(b.state.time),
        PointerEvent::Scroll(s) => Some(s.state.time),
        PointerEvent::Enter(_) | PointerEvent::Leave(_) | PointerEvent::Cancel(_) => None,
        _ => None,
    }
}

fn focus_navigation_for_key(event: &KeyboardEvent) -> Option<Navigation> {
    use ui_events::keyboard::{Key, KeyState, NamedKey};

    if event.state != KeyState::Down || !matches!(&event.key, Key::Named(NamedKey::Tab)) {
        return None;
    }
    Some(if event.modifiers.shift() {
        Navigation::Prev
    } else {
        Navigation::Next
    })
}

fn available_to_f64(s: taffy::AvailableSpace) -> f64 {
    match s {
        taffy::AvailableSpace::Definite(v) => f64::from(v),
        taffy::AvailableSpace::MinContent | taffy::AvailableSpace::MaxContent => f64::INFINITY,
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
    use crate::TYPE_GENERIC;
    use kurbo::{Point, Rect};
    use peniko::{Brush, Color};
    use understory_style::{
        ClassId, ResolveCx, SelectorBuilder, SelectorStep, StyleBuilder, StyleCascadeBuilder,
        StyleOrigin,
    };

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
        let a = ui.insert_child(root);
        let b = ui.insert_child(root);

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
        let a = ui.insert_child(root);
        let grandchild = ui.insert_child(a);
        let b = ui.insert_child(root);

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
        let a = ui.insert_child(root);
        ui.remove(a);

        let c = ui.insert_child(root);
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

        ui.set_local(root, layout_prop, Layout::new().logical_size(10.0, 10.0));
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
        // A style for `Type(TYPE_GENERIC)` should supply the
        // background value for elements that don't have a local override.
        let mut ui = Ui::new();
        let root = ui.root();
        let layout_prop = ui.properties().layout;
        let background = ui.properties().background;

        // Build a cascade with one rule.
        let red = Some(Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff)));
        let style = StyleBuilder::new().set(background, red).build();
        let selector = SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC)).build();
        let cascade = StyleCascadeBuilder::new()
            .push_rule(StyleOrigin::Base, selector, style)
            .build();
        ui.set_cascade(cascade);

        ui.set_local(root, layout_prop, Layout::new().logical_size(10.0, 10.0));
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
        // Mirror the demo: rules at Base origin keyed on TYPE_GENERIC.
        use understory_style::{ClassId, SelectorBuilder, SelectorStep};

        const CLASS_FG: ClassId = ClassId(1);

        let mut ui = Ui::new();
        let root = ui.root();
        let fg = ui.insert_child(root);
        ui.add_class(fg, CLASS_FG);

        let dark = Some(Brush::Solid(Color::from_rgba8(0x1e, 0x1e, 0x24, 0xff)));
        let blue = Some(Brush::Solid(Color::from_rgba8(0x6b, 0x9d, 0xff, 0xff)));
        let bg_prop = ui.properties().background;

        let generic = StyleBuilder::new().set(bg_prop, dark.clone()).build();
        let foreground = StyleBuilder::new().set(bg_prop, blue.clone()).build();

        let cascade = StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC)).build(),
                generic,
            )
            .push_rule(
                StyleOrigin::Base,
                SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC).with_class(CLASS_FG))
                    .build(),
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

        let lookup = UiResolveLookup::new(&ui);
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
    fn styled_button_foreground_reaches_template_label() {
        use crate::{Button, PART_BUTTON_LABEL, TYPE_BUTTON, theme};

        let mut ui = Ui::new();
        ui.set_theme(theme::default_theme());
        let root = ui.root();
        let button = ui.append(root, Button::new("Run"));
        let label = ui.part(button, PART_BUTTON_LABEL).unwrap();

        let white = Brush::Solid(Color::from_rgba8(0xff, 0xff, 0xff, 0xff));
        let style = StyleBuilder::new()
            .set_resource(ui.properties().foreground, theme::ON_ACCENT_FG)
            .build();
        let cascade = StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                SelectorBuilder::new(SelectorStep::type_tag(TYPE_BUTTON)).build(),
                style,
            )
            .build();
        ui.set_cascade(cascade);

        let button_fg = ui.effective_value_for(ui.get(button).unwrap(), ui.properties().foreground);
        assert_eq!(
            button_fg, white,
            "button itself should resolve styled foreground"
        );
        let label_fg = ui.effective_value_for(ui.get(label).unwrap(), ui.properties().foreground);
        assert_eq!(
            label_fg, white,
            "label should inherit the button's styled foreground"
        );

        let mut output = Scene::new();
        ui.paint(&mut output);

        let label_node = ui.box_for(label).expect("label should have a box node");
        let foreground = ui
            .presentation
            .presentation(label_node)
            .expect("label should have presentation")
            .plain_text()
            .expect("label should present text")
            .foreground
            .clone();
        assert_eq!(foreground, Some(white));
    }

    #[test]
    fn text_measure_uses_styled_content_and_font_size() {
        use crate::{TYPE_TEXT, Text};
        use alloc::string::String;

        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_local(
            root,
            ui.properties().layout,
            Layout::flex().logical_size(200.0, 100.0),
        );
        let text = ui.append(root, Text::new(""));

        let style = StyleBuilder::new()
            .set(ui.properties().content, String::from("Styled text"))
            .set(ui.properties().font_size, 40.0_f64)
            .build();
        let cascade = StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                SelectorBuilder::new(SelectorStep::type_tag(TYPE_TEXT)).build(),
                style,
            )
            .build();
        ui.set_cascade(cascade);

        ui.compute_layout(Size::new(200.0, 100.0));

        let bounds = ui.effective_bounds(text);
        assert!(
            bounds.width() > 0.0 && bounds.height() > 30.0,
            "styled text content and font size should participate in measure; got {bounds:?}"
        );
    }

    #[test]
    fn paint_keeps_default_text_on_one_line() {
        use crate::Text;
        use imaging::record::{Command, Draw};

        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_local(
            root,
            ui.properties().layout,
            Layout::flex().logical_size(80.0, 200.0),
        );
        let text = ui.append(
            root,
            Text::new("alpha beta gamma delta epsilon").font_size(20.0),
        );
        ui.set_layout(text, Layout::new().logical_size(80.0, 200.0));

        ui.compute_layout(Size::new(80.0, 200.0));

        let mut output = Scene::new();
        ui.paint(&mut output);

        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for command in output.commands() {
            let Command::Draw(id) = command else {
                continue;
            };
            let Draw::GlyphRun(run) = output.draw_op(*id) else {
                continue;
            };
            for glyph in &run.glyphs {
                min_y = min_y.min(glyph.y);
                max_y = max_y.max(glyph.y);
            }
        }

        assert!(
            max_y - min_y < 20.0,
            "default text should paint on one line; glyph y span was {}",
            max_y - min_y
        );
    }

    #[test]
    fn paint_wraps_opted_in_text_to_element_bounds() {
        use crate::Text;
        use imaging::record::{Command, Draw};

        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_local(
            root,
            ui.properties().layout,
            Layout::flex().logical_size(80.0, 200.0),
        );
        let text = ui.append(
            root,
            Text::new("alpha beta gamma delta epsilon")
                .font_size(20.0)
                .wrap(),
        );
        ui.set_layout(text, Layout::new().logical_size(80.0, 200.0));

        ui.compute_layout(Size::new(80.0, 200.0));

        let bounds = ui.effective_bounds(text);
        assert!(
            bounds.width() <= 80.0,
            "text should be laid out within the constrained width; got {bounds:?}"
        );

        let mut output = Scene::new();
        ui.paint(&mut output);

        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for command in output.commands() {
            let Command::Draw(id) = command else {
                continue;
            };
            let Draw::GlyphRun(run) = output.draw_op(*id) else {
                continue;
            };
            for glyph in &run.glyphs {
                min_y = min_y.min(glyph.y);
                max_y = max_y.max(glyph.y);
            }
        }

        assert!(
            max_y - min_y > 20.0,
            "paint should render wrapped text lines; glyph y span was {}",
            max_y - min_y
        );
    }

    #[test]
    fn row_text_uses_natural_width_until_width_is_chosen() {
        use crate::{Panel, Text};
        use imaging::record::{Command, Draw};

        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(12.0).logical_size(284.0, 80.0));

        let spacer = ui.append(root, Panel::new());
        ui.set_layout(spacer, Layout::new().logical_size(64.0, 32.0));
        let label = ui.append(root, Text::new("Live rendering").font_size(15.0));

        ui.compute_layout(Size::new(284.0, 80.0));

        let label_bounds = ui.effective_bounds(label);
        assert!(
            label_bounds.width() > 90.0,
            "row label should keep its natural width instead of measuring wrapped; got {label_bounds:?}"
        );

        let mut output = Scene::new();
        ui.paint(&mut output);

        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for command in output.commands() {
            let Command::Draw(id) = command else {
                continue;
            };
            let Draw::GlyphRun(run) = output.draw_op(*id) else {
                continue;
            };
            for glyph in &run.glyphs {
                min_y = min_y.min(glyph.y);
                max_y = max_y.max(glyph.y);
            }
        }

        assert!(
            max_y - min_y < 20.0,
            "row label should paint on one line; glyph y span was {}",
            max_y - min_y
        );
    }

    #[test]
    fn control_row_labels_keep_single_line_widths() {
        use crate::{Spinner, Text, ToggleSwitch, theme};

        let mut ui = Ui::new();
        ui.set_theme(theme::default_theme());
        let root = ui.root();
        ui.set_layout(
            root,
            Layout::column(14.0)
                .logical_size(320.0, 350.0)
                .padding_all(18.0),
        );

        let toggle_row = ui.append(root, crate::Panel::row(12.0).align_center());
        ui.append(toggle_row, ToggleSwitch::new().checked(true));
        let toggle_label = ui.append(toggle_row, Text::new("Live rendering").font_size(15.0));

        let spinner_row = ui.append(root, crate::Panel::row(12.0).align_center());
        ui.append(spinner_row, Spinner::new());
        let spinner_label = ui.append(spinner_row, Text::new("Streaming updates").font_size(13.0));

        ui.compute_layout(Size::new(320.0, 350.0));

        let toggle_natural_width = f64::from(
            ui.text
                .borrow_mut()
                .shape("Live rendering", 15.0, Brush::default(), None)
                .width(),
        );
        let spinner_natural_width = f64::from(
            ui.text
                .borrow_mut()
                .shape("Streaming updates", 13.0, Brush::default(), None)
                .width(),
        );

        let toggle_bounds = ui.effective_bounds(toggle_label);
        let spinner_bounds = ui.effective_bounds(spinner_label);
        assert!(
            toggle_bounds.width() >= toggle_natural_width - 1.0,
            "toggle label should retain natural width; natural={toggle_natural_width}, bounds={toggle_bounds:?}"
        );
        assert!(
            spinner_bounds.width() >= spinner_natural_width - 1.0,
            "spinner label should retain natural width; natural={spinner_natural_width}, bounds={spinner_bounds:?}"
        );

        let mut output = Scene::new();
        ui.paint(&mut output);

        let mut max_span = 0.0_f32;
        for command in output.commands() {
            let imaging::record::Command::Draw(id) = command else {
                continue;
            };
            let imaging::record::Draw::GlyphRun(run) = output.draw_op(*id) else {
                continue;
            };
            let mut min_y = f32::INFINITY;
            let mut max_y = f32::NEG_INFINITY;
            for glyph in &run.glyphs {
                min_y = min_y.min(glyph.y);
                max_y = max_y.max(glyph.y);
            }
            if min_y.is_finite() && max_y.is_finite() {
                max_span = max_span.max(max_y - min_y);
            }
        }
        assert!(
            max_span < 20.0,
            "control row labels should paint on one line; max glyph y span was {max_span}"
        );
    }

    #[test]
    fn slider_initial_value_positions_thumb() {
        use crate::Slider;

        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_local(
            root,
            ui.properties().layout,
            Layout::flex().logical_size(300.0, 80.0),
        );

        let slider = ui.append(root, Slider::new().value(0.4));
        let thumb = ui.part(slider, crate::PART_SLIDER_THUMB).unwrap();

        ui.compute_layout(Size::new(300.0, 80.0));

        let slider_bounds = ui.effective_bounds(slider);
        let thumb_bounds = ui.effective_bounds(thumb);
        let thumb_offset = thumb_bounds.x0 - slider_bounds.x0;
        assert!(
            thumb_offset > 70.0,
            "initial value should move thumb along the track; got x offset {thumb_offset}"
        );
    }

    #[test]
    fn default_templates_label_named_parts() {
        use crate::{
            Button, PART_BUTTON_LABEL, PART_SLIDER_THUMB, PART_SLIDER_TRACK, PART_TOGGLE_THUMB,
            SLOT_BUTTON_CONTENT, SLOT_SLIDER_THUMB, SLOT_SLIDER_TRACK, SLOT_TOGGLE_THUMB, Slider,
            ToggleSwitch,
        };

        let mut ui = Ui::new();
        let root = ui.root();

        let button = ui.append(root, Button::new("Run"));
        let button_label = ui.part(button, PART_BUTTON_LABEL).unwrap();
        assert_eq!(
            ui.get(button_label).unwrap().part_tag(),
            Some(PART_BUTTON_LABEL)
        );
        assert_eq!(
            ui.template_instance(button)
                .unwrap()
                .part(PART_BUTTON_LABEL),
            Some(button_label)
        );
        assert_eq!(ui.slot(button, SLOT_BUTTON_CONTENT), Some(button_label));

        let slider = ui.append(root, Slider::new());
        let slider_track = ui.part(slider, PART_SLIDER_TRACK).unwrap();
        let slider_thumb = ui.part(slider, PART_SLIDER_THUMB).unwrap();
        assert_eq!(ui.slot(slider, SLOT_SLIDER_TRACK), Some(slider_track));
        assert_eq!(ui.slot(slider, SLOT_SLIDER_THUMB), Some(slider_thumb));
        assert_eq!(
            ui.get(slider_track).unwrap().part_tag(),
            Some(PART_SLIDER_TRACK)
        );
        assert_eq!(
            ui.get(slider_thumb).unwrap().part_tag(),
            Some(PART_SLIDER_THUMB)
        );

        let toggle = ui.append(root, ToggleSwitch::new());
        let toggle_thumb = ui.part(toggle, PART_TOGGLE_THUMB).unwrap();
        assert_eq!(ui.slot(toggle, SLOT_TOGGLE_THUMB), Some(toggle_thumb));
        assert_eq!(
            ui.get(toggle_thumb).unwrap().part_tag(),
            Some(PART_TOGGLE_THUMB)
        );
    }

    #[test]
    fn template_overrides_replace_visuals_not_control_behavior() {
        use crate::widget::SurfacePartWidget;
        use crate::{
            Button, SLOT_BUTTON_CONTENT, SLOT_SLIDER_THUMB, SLOT_TOGGLE_THUMB, Slider, Template,
            TextWidget, ToggleSwitch, set_slider_value,
        };

        const OUTER_FRAME: PartTag = PartTag(0x40);
        const INNER_FRAME: PartTag = PartTag(0x41);

        struct DoubleFrameButtonTemplate;
        impl Template for DoubleFrameButtonTemplate {
            fn instantiate(&self, cx: &mut TemplateContext<'_>) {
                let props = *cx.properties();
                let outer = cx.part_widget(OUTER_FRAME, Box::new(SurfacePartWidget));
                let inner = cx.part_widget_under(outer, INNER_FRAME, Box::new(SurfacePartWidget));
                let content =
                    cx.slot_widget_under(inner, SLOT_BUTTON_CONTENT, Box::new(TextWidget));
                cx.bind_owner_to_part(props.content, content, props.content)
                    .expect("content binding should install");
                cx.bind_owner_to_part(props.font_size, content, props.font_size)
                    .expect("font size binding should install");
            }
        }

        struct SingleSlotTemplate(TemplateSlot);
        impl Template for SingleSlotTemplate {
            fn instantiate(&self, cx: &mut TemplateContext<'_>) {
                let _part = cx.slot_widget(self.0, Box::new(SurfacePartWidget));
            }
        }

        static BUTTON_TEMPLATE: DoubleFrameButtonTemplate = DoubleFrameButtonTemplate;
        static TOGGLE_TEMPLATE: SingleSlotTemplate = SingleSlotTemplate(SLOT_TOGGLE_THUMB);
        static SLIDER_TEMPLATE: SingleSlotTemplate = SingleSlotTemplate(SLOT_SLIDER_THUMB);

        let mut ui = Ui::new();
        let root = ui.root();

        let button = ui.append(root, Button::new("Run").template(&BUTTON_TEMPLATE));
        let outer = ui.part(button, OUTER_FRAME).unwrap();
        let inner = ui.part(button, INNER_FRAME).unwrap();
        let content = ui.slot(button, SLOT_BUTTON_CONTENT).unwrap();
        assert_eq!(ui.get(outer).unwrap().parent(), Some(button));
        assert_eq!(ui.get(inner).unwrap().parent(), Some(outer));
        assert_eq!(ui.get(content).unwrap().parent(), Some(inner));
        ui.flush_bindings();
        assert_eq!(
            ui.get(content)
                .unwrap()
                .property_store()
                .effective_local_erased(ui.properties().content.id())
                .and_then(|value| value.downcast_ref::<String>())
                .map(String::as_str),
            Some("Run")
        );

        let toggle = ui.append(root, ToggleSwitch::new().template(&TOGGLE_TEMPLATE));
        ui.synthesize_click(toggle, Point::ZERO);
        let checked = ui
            .get(toggle)
            .and_then(|el| el.get_local_value(ui.properties().is_checked).copied())
            .unwrap_or(false);
        assert!(
            checked,
            "custom toggle template should keep toggle behavior"
        );

        let slider = ui.append(root, Slider::new().template(&SLIDER_TEMPLATE).value(0.75));
        assert!(ui.slot(slider, SLOT_SLIDER_THUMB).is_some());
        assert_eq!(
            ui.get(slider)
                .unwrap()
                .get_local_value(ui.properties().value)
                .copied(),
            Some(0.75)
        );
        set_slider_value(&mut ui, slider, 0.25);
        assert_eq!(
            ui.get(slider)
                .unwrap()
                .get_local_value(ui.properties().value)
                .copied(),
            Some(0.25)
        );
    }

    #[test]
    fn surface_template_parts_do_not_match_generic_type_selector() {
        use crate::Slider;

        let mut ui = Ui::new();
        let root = ui.root();
        let slider = ui.append(root, Slider::new());
        let track = ui.part(slider, crate::PART_SLIDER_TRACK).unwrap();

        let red = Some(Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff)));
        let style = StyleBuilder::new()
            .set(ui.properties().background, red)
            .build();
        let cascade = StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                crate::selectors::type_tag(TYPE_GENERIC),
                style,
            )
            .build();
        ui.set_cascade(cascade);

        let background = ui.effective_value_for(ui.get(track).unwrap(), ui.properties().background);
        assert_eq!(
            background, None,
            "surface-only template parts should not inherit TYPE_GENERIC as a fake type"
        );
    }

    #[test]
    fn slider_template_parts_match_owner_local_part_selectors() {
        use crate::{PART_SLIDER_TRACK, Slider, TYPE_SLIDER};

        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_local(
            root,
            ui.properties().layout,
            Layout::flex().logical_size(300.0, 80.0),
        );

        let slider = ui.append(root, Slider::new());
        let track = ui.part(slider, PART_SLIDER_TRACK).unwrap();
        let red = Some(Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff)));
        let style = StyleBuilder::new()
            .set(ui.properties().background, red.clone())
            .build();
        let cascade = StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                SelectorBuilder::new(SelectorStep::type_tag(TYPE_SLIDER))
                    .child(SelectorStep::part_tag(PART_SLIDER_TRACK))
                    .build(),
                style,
            )
            .build();
        ui.set_cascade(cascade);

        let background = ui.effective_value_for(ui.get(track).unwrap(), ui.properties().background);
        assert_eq!(
            background, red,
            "owner-local part rule should match the nested slider track"
        );

        ui.compute_layout(Size::new(300.0, 80.0));
        let mut output = Scene::new();
        ui.paint(&mut output);
        assert!(
            !output.is_empty(),
            "styled slider track should contribute painted output"
        );
    }

    #[test]
    fn slider_template_creates_keyed_major_and_minor_tick_parts() {
        use crate::{
            CLASS_SLIDER_MAJOR_TICK, CLASS_SLIDER_MINOR_TICK, PART_SLIDER_TICK,
            PART_SLIDER_TICK_LABEL, SLOT_SLIDER_TICK, SLOT_SLIDER_TICK_LABEL, Slider,
            TemplateItemKey,
        };

        let mut ui = Ui::new();
        let root = ui.root();
        let slider = ui.append(root, Slider::new().min(10.0).max(20.0));
        let ticks: Vec<_> = ui.parts(slider, PART_SLIDER_TICK).collect();
        let labels: Vec<_> = ui.parts(slider, PART_SLIDER_TICK_LABEL).collect();

        assert_eq!(ticks.len(), 5);
        assert_eq!(labels.len(), 5);

        let instance = ui.template_instance(slider).unwrap();
        let first_tick = instance
            .keyed_part(PART_SLIDER_TICK, TemplateItemKey::new(0))
            .unwrap();
        assert_eq!(
            instance.keyed_slot(SLOT_SLIDER_TICK, TemplateItemKey::new(0)),
            Some(first_tick)
        );
        let second_tick = instance
            .keyed_part(PART_SLIDER_TICK, TemplateItemKey::new(1))
            .unwrap();
        assert!(
            ui.get(first_tick)
                .unwrap()
                .classes()
                .contains(&CLASS_SLIDER_MAJOR_TICK)
        );
        assert!(
            ui.get(second_tick)
                .unwrap()
                .classes()
                .contains(&CLASS_SLIDER_MINOR_TICK)
        );

        let middle_label = instance
            .keyed_part(PART_SLIDER_TICK_LABEL, TemplateItemKey::new(2))
            .unwrap();
        assert_eq!(
            instance.keyed_slot(SLOT_SLIDER_TICK_LABEL, TemplateItemKey::new(2)),
            Some(middle_label)
        );
        let content = ui.properties().content;
        let label = ui
            .get(middle_label)
            .unwrap()
            .get_local_value(content)
            .cloned();
        assert_eq!(label.as_deref(), Some("15"));
    }

    #[test]
    fn slider_keyboard_uses_small_and_large_changes() {
        use crate::Slider;
        use ui_events::keyboard::{Code, Key, KeyboardEvent, Modifiers, NamedKey};

        let mut ui = Ui::new();
        let root = ui.root();
        let slider = ui.append(
            root,
            Slider::new()
                .value(0.5)
                .small_change(0.1)
                .large_change(0.25),
        );
        assert!(ui.set_focus(Some(slider)));

        let right = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
        assert!(ui.dispatch_key(&right).handled);
        assert_eq!(slider_value(&ui, slider), Some(0.6));

        let mut shift_right =
            KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
        shift_right.modifiers = Modifiers::SHIFT;
        assert!(ui.dispatch_key(&shift_right).handled);
        assert_eq!(slider_value(&ui, slider), Some(0.85));

        let page_down = KeyboardEvent::key_down(Key::Named(NamedKey::PageDown), Code::PageDown);
        assert!(ui.dispatch_key(&page_down).handled);
        assert_eq!(slider_value(&ui, slider), Some(0.6));

        let home = KeyboardEvent::key_down(Key::Named(NamedKey::Home), Code::Home);
        assert!(ui.dispatch_key(&home).handled);
        assert_eq!(slider_value(&ui, slider), Some(0.0));

        let end = KeyboardEvent::key_down(Key::Named(NamedKey::End), Code::End);
        assert!(ui.dispatch_key(&end).handled);
        assert_eq!(slider_value(&ui, slider), Some(1.0));
    }

    #[test]
    fn slider_thumb_click_and_drag_preserve_thumb_grab_offset() {
        use crate::{SLOT_SLIDER_THUMB, Slider};

        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_local(
            root,
            ui.properties().layout,
            Layout::flex().logical_size(300.0, 80.0),
        );
        let slider = ui.append(root, Slider::new().value(0.5));
        ui.compute_layout(Size::new(300.0, 80.0));

        let thumb = ui.slot(slider, SLOT_SLIDER_THUMB).unwrap();
        let before = ui.effective_bounds(thumb);
        let press = Point::new(before.x0 + 3.0, before.center().y);

        assert!(ui.dispatch_pointer_event(&pointer_down(press, 1)));
        assert!(ui.dispatch_pointer_event(&pointer_up(press, 2)));
        assert_eq!(
            slider_value(&ui, slider),
            Some(0.5),
            "clicking the thumb itself should not recenter it under the pointer"
        );

        assert!(ui.dispatch_pointer_event(&pointer_down(press, 3)));
        assert!(ui.dispatch_pointer_event(&pointer_move(
            Point::new(press.x + 20.0, press.y),
            4,
            true,
        )));
        ui.compute_layout(Size::new(300.0, 80.0));

        let after = ui.effective_bounds(thumb);
        assert!(
            ((after.center().x - before.center().x) - 20.0).abs() < 0.001,
            "thumb should move by the drag delta instead of snapping its center to the pointer"
        );
    }

    #[test]
    fn default_cascade_styles_template_parts_by_owner_part() {
        use crate::{PART_SLIDER_TICK, Slider, TemplateItemKey, theme};

        let mut ui = Ui::new();
        ui.set_theme(theme::default_theme());
        let cascade = theme::default_cascade(ui.properties());
        ui.set_cascade(cascade);

        let root = ui.root();
        let slider = ui.append(root, Slider::new());
        let track = ui.part(slider, crate::PART_SLIDER_TRACK).unwrap();
        let major_tick = ui
            .template_instance(slider)
            .unwrap()
            .keyed_part(PART_SLIDER_TICK, TemplateItemKey::new(0))
            .unwrap();
        let expected = Some(Brush::Solid(Color::from_rgba8(0x44, 0x44, 0x4a, 0xff)));
        let background = ui.effective_value_for(ui.get(track).unwrap(), ui.properties().background);
        let tick_background =
            ui.effective_value_for(ui.get(major_tick).unwrap(), ui.properties().background);

        assert_eq!(background, expected);
        assert_eq!(
            tick_background,
            Some(Brush::Solid(Color::from_rgba8(0xc8, 0xc8, 0xd2, 0xff)))
        );
    }

    #[test]
    fn owner_pseudo_selector_can_style_template_descendant() {
        use crate::{Button, PART_BUTTON_LABEL, TYPE_BUTTON};

        let mut ui = Ui::new();
        let root = ui.root();
        let button = ui.append(root, Button::new("Run"));
        let label = ui.part(button, PART_BUTTON_LABEL).unwrap();

        let red = Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff));
        let style = StyleBuilder::new()
            .set(ui.properties().foreground, red.clone())
            .build();
        let cascade = StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                SelectorBuilder::new(
                    SelectorStep::type_tag(TYPE_BUTTON).with_pseudo(PSEUDO_PRESSED),
                )
                .child(SelectorStep::part_tag(PART_BUTTON_LABEL))
                .build(),
                style,
            )
            .build();
        ui.set_cascade(cascade);
        ui.add_pseudo(button, PSEUDO_PRESSED);

        let mut output = Scene::new();
        ui.paint(&mut output);

        let label_node = ui.box_for(label).expect("label should have a box node");
        let foreground = ui
            .presentation
            .presentation(label_node)
            .expect("label should have presentation")
            .plain_text()
            .expect("label should present text")
            .foreground
            .clone();
        assert_eq!(foreground, Some(red));
    }

    #[test]
    fn hit_test_picks_topmost_child() {
        // Hit testing now goes through `understory_box_tree`'s
        // spatially-indexed box-tree hit testing, which reads the
        // tree's `local_bounds` populated by `compute_layout`. Drive
        // the test through the layout pipeline: root 200×200 with
        // 50px padding all around; child fills the content area,
        // giving it 50..150 × 50..150.
        let mut ui = Ui::new();
        let root = ui.root();
        let layout_prop = ui.properties().layout;
        ui.set_local(
            root,
            layout_prop,
            Layout::flex().logical_size(200.0, 200.0).padding_all(50.0),
        );
        let child = ui.insert_child(root);
        ui.set_local(
            child,
            layout_prop,
            Layout::new().flex_grow(1.0).align_self_stretch(),
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
        let a = ui.insert_child(root);
        let b = ui.insert_child(root);

        assert_eq!(ui.focused(), None);
        assert!(ui.set_focus(Some(a)));
        assert_eq!(ui.focused(), Some(a));
        assert!(ui.get(a).unwrap().pseudos().contains(&PSEUDO_FOCUSED));
        assert!(ui.get(a).unwrap().pseudos().contains(&PSEUDO_FOCUS_VISIBLE));

        // Move focus to b — a should lose focus pseudos, b should gain them.
        assert!(ui.set_focus(Some(b)));
        assert!(!ui.get(a).unwrap().pseudos().contains(&PSEUDO_FOCUSED));
        assert!(!ui.get(a).unwrap().pseudos().contains(&PSEUDO_FOCUS_VISIBLE));
        assert!(ui.get(b).unwrap().pseudos().contains(&PSEUDO_FOCUSED));
        assert!(ui.get(b).unwrap().pseudos().contains(&PSEUDO_FOCUS_VISIBLE));

        // Clear focus.
        assert!(ui.set_focus(None));
        assert!(!ui.get(b).unwrap().pseudos().contains(&PSEUDO_FOCUSED));
        assert!(!ui.get(b).unwrap().pseudos().contains(&PSEUDO_FOCUS_VISIBLE));
        assert_eq!(ui.focused(), None);
    }

    #[test]
    fn pointer_focus_does_not_set_focus_visible() {
        use crate::Button;

        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::flex().logical_size(200.0, 80.0));
        let button = ui.append(root, Button::new("Click"));
        ui.on_click(button, |_ui, event| event.mark_handled());
        ui.compute_layout(Size::new(200.0, 80.0));

        let center = ui.effective_bounds(button).center();
        assert!(ui.dispatch_pointer_event(&pointer_down(center, 1)));
        assert_eq!(ui.focused(), Some(button));
        assert!(ui.dispatch_pointer_event(&pointer_up(center, 2)));

        let pseudos = ui.get(button).unwrap().pseudos();
        assert_eq!(ui.focused(), Some(button));
        assert!(pseudos.contains(&PSEUDO_FOCUSED));
        assert!(!pseudos.contains(&PSEUDO_FOCUS_VISIBLE));
    }

    #[test]
    fn focus_navigate_walks_focusables() {
        // Lay out three focusable children. `Navigation::Next`
        // cycles through them in document order with WrapMode::Scope.
        let mut ui = Ui::new();
        let root = ui.root();
        let layout_prop = ui.properties().layout;
        ui.set_local(root, layout_prop, Layout::flex().logical_size(300.0, 50.0));
        let a = ui.insert_child(root);
        let b = ui.insert_child(root);
        let c = ui.insert_child(root);
        for id in [a, b, c] {
            ui.set_local(
                id,
                layout_prop,
                Layout::new().flex_grow(1.0).align_self_stretch(),
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
        let child = ui.insert_child(root);
        ui.set_focusable(child, true);
        ui.set_focus(Some(child));

        let fired = Rc::new(Cell::new(false));
        let inc = fired.clone();
        ui.on_key(child, move |_ui, event| {
            if matches!(event.phase, Phase::Target) {
                assert_eq!(event.kind, RoutedEventKind::Key);
                inc.set(true);
            }
        });

        let key = KeyboardEvent::key_down(Key::Named(NamedKey::Enter), Code::Enter);
        let result = ui.dispatch_key(&key);
        assert!(fired.get(), "target-phase key handler should fire");
        assert!(!result.handled, "handler did not mark the key handled");
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
        let result = ui.dispatch_key(&key);
        assert!(!fired.get(), "key handler should not fire without focus");
        assert_eq!(result, KeyDispatchResult::default());
    }

    #[test]
    fn dispatch_key_stops_route_when_handler_marks_handled() {
        use core::cell::Cell;
        use std::rc::Rc;
        use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.insert_child(root);
        ui.set_focusable(child, true);
        ui.set_focus(Some(child));

        let child_fired = Rc::new(Cell::new(false));
        let child_fired_for_handler = Rc::clone(&child_fired);
        ui.on_key(root, move |_ui, event| {
            if matches!(event.phase, Phase::Capture) {
                event.mark_handled();
            }
        });
        ui.on_key(child, move |_ui, _event| {
            child_fired_for_handler.set(true);
        });

        let key = KeyboardEvent::key_down(Key::Named(NamedKey::Enter), Code::Enter);
        let result = ui.dispatch_key(&key);

        assert!(result.handled);
        assert!(!result.propagation_stopped);
        assert!(
            !child_fired.get(),
            "handled capture-phase key should stop before target"
        );
    }

    #[test]
    fn dispatch_key_stops_route_when_handler_stops_propagation() {
        use core::cell::Cell;
        use std::rc::Rc;
        use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.insert_child(root);
        ui.set_focusable(child, true);
        ui.set_focus(Some(child));

        let child_fired = Rc::new(Cell::new(false));
        let child_fired_for_handler = Rc::clone(&child_fired);
        ui.on_key(root, move |_ui, event| {
            if matches!(event.phase, Phase::Capture) {
                event.stop_propagation();
            }
        });
        ui.on_key(child, move |_ui, _event| {
            child_fired_for_handler.set(true);
        });

        let key = KeyboardEvent::key_down(Key::Named(NamedKey::Enter), Code::Enter);
        let result = ui.dispatch_key(&key);

        assert!(!result.handled);
        assert!(result.propagation_stopped);
        assert!(
            !child_fired.get(),
            "stopped capture-phase key should stop before target"
        );
    }

    #[test]
    fn dispatch_key_handles_tab_focus_navigation() {
        use ui_events::keyboard::{Code, Key, KeyboardEvent, Modifiers, NamedKey};

        let mut ui = Ui::new();
        let root = ui.root();
        let layout_prop = ui.properties().layout;
        ui.set_local(root, layout_prop, Layout::flex().logical_size(300.0, 50.0));
        let a = ui.insert_child(root);
        let b = ui.insert_child(root);
        for id in [a, b] {
            ui.set_local(
                id,
                layout_prop,
                Layout::new().flex_grow(1.0).align_self_stretch(),
            );
            ui.set_focusable(id, true);
        }
        ui.compute_layout(Size::new(300.0, 50.0));

        let tab = KeyboardEvent::key_down(Key::Named(NamedKey::Tab), Code::Tab);
        let result = ui.dispatch_key(&tab);
        assert!(result.handled);
        assert!(result.focus_changed);
        assert_eq!(result.focused, Some(a));
        assert_eq!(ui.focused(), Some(a));

        let result = ui.dispatch_key(&tab);
        assert!(result.handled);
        assert!(result.focus_changed);
        assert_eq!(result.focused, Some(b));
        assert_eq!(ui.focused(), Some(b));

        let mut shift_tab = KeyboardEvent::key_down(Key::Named(NamedKey::Tab), Code::Tab);
        shift_tab.modifiers = Modifiers::SHIFT;
        let result = ui.dispatch_key(&shift_tab);
        assert!(result.handled);
        assert!(result.focus_changed);
        assert_eq!(result.focused, Some(a));
        assert_eq!(ui.focused(), Some(a));
    }

    #[test]
    fn built_in_controls_mark_consumed_keys_handled() {
        use core::cell::Cell;
        use std::rc::Rc;

        use crate::{Button, Slider, ToggleSwitch};
        use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

        let mut ui = Ui::new();
        let root = ui.root();
        let clicked = Rc::new(Cell::new(false));
        let clicked_for_handler = Rc::clone(&clicked);
        let button = ui.append(
            root,
            Button::new("Run").on_click(move |_ui, _event| clicked_for_handler.set(true)),
        );

        assert!(ui.set_focus(Some(button)));
        let enter = KeyboardEvent::key_down(Key::Named(NamedKey::Enter), Code::Enter);
        let result = ui.dispatch_key(&enter);
        assert!(result.handled);
        assert!(
            clicked.get(),
            "button Enter activation should synthesize a click"
        );

        let toggle = ui.append(root, ToggleSwitch::new());
        assert!(ui.set_focus(Some(toggle)));
        let result = ui.dispatch_key(&enter);
        assert!(result.handled);
        assert_eq!(
            ui.get(toggle)
                .and_then(|el| el.get_local_value(ui.properties().is_checked).copied()),
            Some(true)
        );

        let slider = ui.append(root, Slider::new().value(0.4));
        assert!(ui.set_focus(Some(slider)));
        let right = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
        let result = ui.dispatch_key(&right);
        assert!(result.handled);
        assert!(
            ui.get(slider)
                .and_then(|el| el.get_local_value(ui.properties().value).copied())
                .is_some_and(|value| value > 0.4),
            "slider ArrowRight should increase Value"
        );
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
        let child = ui.insert_child(root);

        let fired_with = Rc::new(Cell::new(None::<(Phase, ElementId, ElementId)>));
        let fired = fired_with.clone();
        ui.on_click(child, move |_ui, event| {
            if matches!(event.phase, Phase::Target) {
                assert_eq!(event.kind, RoutedEventKind::Click);
                fired.set(Some((event.phase, event.source, event.current)));
            }
        });

        ui.dispatch_click(child, Point::ZERO);

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
        let child = ui.insert_child(root);
        let grand = ui.insert_child(child);

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

        ui.dispatch_click(grand, Point::ZERO);

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
    fn dispatch_click_stops_route_when_handler_stops_propagation() {
        use core::cell::Cell;
        use std::rc::Rc;

        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.insert_child(root);

        let child_fired = Rc::new(Cell::new(false));
        let child_fired_for_handler = Rc::clone(&child_fired);
        ui.on_click(root, move |_ui, event| {
            if matches!(event.phase, Phase::Capture) {
                event.stop_propagation();
            }
        });
        ui.on_click(child, move |_ui, _event| {
            child_fired_for_handler.set(true);
        });

        let result = ui.dispatch_click(child, Point::ZERO);

        assert!(!result.handled);
        assert!(result.propagation_stopped);
        assert!(
            !child_fired.get(),
            "stopped capture-phase click should stop before target"
        );
    }

    #[test]
    fn dispatch_scroll_walks_capture_target_bubble() {
        use alloc::string::String;
        use core::cell::RefCell;
        use std::rc::Rc;
        use ui_events::ScrollDelta;

        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.insert_child(root);
        let grand = ui.insert_child(child);

        let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let target_delta = Rc::new(RefCell::new(Vec2::ZERO));
        for (name, id) in [("root", root), ("child", child), ("grand", grand)] {
            let log = Rc::clone(&log);
            let target_delta = Rc::clone(&target_delta);
            ui.on_scroll(id, move |_ui, event| {
                let phase = match event.phase {
                    Phase::Capture => "capture",
                    Phase::Target => "target",
                    Phase::Bubble => "bubble",
                };
                if matches!(event.phase, Phase::Target) {
                    assert_eq!(event.kind, RoutedEventKind::Scroll);
                    *target_delta.borrow_mut() =
                        event.logical_delta(Vec2::new(16.0, 20.0), Vec2::new(160.0, 200.0));
                    event.mark_handled();
                }
                log.borrow_mut().push(alloc::format!("{name}:{phase}"));
            });
        }

        let result = ui.synthesize_scroll(grand, Point::ZERO, ScrollDelta::LineDelta(0.0, 2.0));

        assert!(result.handled);
        assert_eq!(*target_delta.borrow(), Vec2::new(0.0, 40.0));
        assert_eq!(
            log.borrow().as_slice(),
            &[
                "root:capture".to_owned(),
                "child:capture".to_owned(),
                "grand:target".to_owned(),
            ]
        );
    }

    #[test]
    fn pointer_scroll_hit_tests_and_routes_to_target() {
        use core::cell::Cell;
        use std::rc::Rc;
        use ui_events::ScrollDelta;

        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::flex().logical_size(200.0, 200.0));
        let child = ui.insert_child(root);
        ui.set_layout(child, Layout::new().logical_size(80.0, 80.0));
        ui.compute_layout(Size::new(200.0, 200.0));

        let fired = Rc::new(Cell::new(false));
        let fired_for_handler = Rc::clone(&fired);
        ui.on_scroll(child, move |_ui, event| {
            if matches!(event.phase, Phase::Target) {
                assert_eq!(event.source, child);
                assert_eq!(event.position, Point::new(20.0, 20.0));
                fired_for_handler.set(true);
                event.mark_handled();
            }
        });

        let event = pointer_scroll(
            Point::new(20.0, 20.0),
            123,
            ScrollDelta::LineDelta(0.0, 1.0),
        );
        assert!(ui.dispatch_pointer_event(&event));
        assert!(fired.get());
        assert_eq!(ui.now(), 123);
    }

    #[test]
    fn dispatch_drag_stops_route_when_handler_marks_handled() {
        use core::cell::Cell;
        use std::rc::Rc;

        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.insert_child(root);
        ui.events.set_press_target(Some(child));
        ui.events.start_press(Point::ZERO);

        let child_fired = Rc::new(Cell::new(false));
        let child_fired_for_handler = Rc::clone(&child_fired);
        ui.on_drag(root, move |_ui, event| {
            if matches!(event.phase, Phase::Capture) {
                event.mark_handled();
            }
        });
        ui.on_drag(child, move |_ui, _event| {
            assert_eq!(_event.kind, RoutedEventKind::Drag);
            child_fired_for_handler.set(true);
        });

        let result = ui.dispatch_drag(Point::new(4.0, 0.0));

        assert!(result.handled);
        assert!(!result.propagation_stopped);
        assert!(
            !child_fired.get(),
            "handled capture-phase drag should stop before target"
        );
    }

    #[test]
    fn remove_click_handler_stops_firing() {
        use core::cell::Cell;
        use std::rc::Rc;

        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.insert_child(root);

        let count = Rc::new(Cell::new(0_u32));
        let inc = count.clone();
        ui.on_click(child, move |_ui, _event| {
            inc.set(inc.get() + 1);
        });

        ui.dispatch_click(child, Point::ZERO);
        assert_eq!(count.get(), 1);

        ui.remove_click_handler(child);
        ui.dispatch_click(child, Point::ZERO);
        assert_eq!(count.get(), 1, "removed handler should not fire");
    }

    #[test]
    fn click_handler_can_remove_itself_during_dispatch() {
        use core::cell::Cell;
        use std::rc::Rc;

        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.insert_child(root);

        let count = Rc::new(Cell::new(0_u32));
        let count_for_handler = Rc::clone(&count);
        ui.on_click(child, move |ui, event| {
            if matches!(event.phase, Phase::Target) {
                count_for_handler.set(count_for_handler.get() + 1);
                ui.remove_click_handler(child);
            }
        });

        ui.dispatch_click(child, Point::ZERO);
        ui.dispatch_click(child, Point::ZERO);

        assert_eq!(count.get(), 1, "self-removed handler should stay removed");
    }

    #[test]
    fn key_handler_can_remove_itself_during_dispatch() {
        use core::cell::Cell;
        use std::rc::Rc;
        use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.insert_child(root);
        ui.set_focusable(child, true);
        ui.set_focus(Some(child));

        let count = Rc::new(Cell::new(0_u32));
        let count_for_handler = Rc::clone(&count);
        ui.on_key(child, move |ui, event| {
            if matches!(event.phase, Phase::Target) {
                count_for_handler.set(count_for_handler.get() + 1);
                ui.remove_key_handler(child);
            }
        });

        let key = KeyboardEvent::key_down(Key::Named(NamedKey::Enter), Code::Enter);
        let _ = ui.dispatch_key(&key);
        let _ = ui.dispatch_key(&key);

        assert_eq!(count.get(), 1, "self-removed handler should stay removed");
    }

    #[test]
    fn drag_handler_can_remove_itself_during_dispatch() {
        use core::cell::Cell;
        use std::rc::Rc;

        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.insert_child(root);
        ui.events.set_press_target(Some(child));
        ui.events.start_press(Point::ZERO);

        let count = Rc::new(Cell::new(0_u32));
        let count_for_handler = Rc::clone(&count);
        ui.on_drag(child, move |ui, event| {
            if matches!(event.phase, Phase::Target) {
                count_for_handler.set(count_for_handler.get() + 1);
                ui.remove_drag_handler(child);
            }
        });

        ui.dispatch_drag(Point::new(1.0, 0.0));
        ui.dispatch_drag(Point::new(2.0, 0.0));

        assert_eq!(count.get(), 1, "self-removed handler should stay removed");
    }

    #[test]
    fn repeating_timer_rearms_while_handler_remains() {
        use core::cell::Cell;
        use std::rc::Rc;

        let mut ui = Ui::new();
        let target = ui.root();
        let count = Rc::new(Cell::new(0_u32));
        let count_for_handler = Rc::clone(&count);

        ui.on_timer(target, move |_ui, _event| {
            count_for_handler.set(count_for_handler.get() + 1);
        });
        ui.schedule_timer_repeating(target, 10);

        ui.pump_timers(10);
        assert_eq!(count.get(), 1, "first timer deadline should fire");
        assert_eq!(
            ui.next_timer_deadline(),
            Some(20),
            "live repeating timer should re-arm"
        );

        ui.pump_timers(20);
        assert_eq!(count.get(), 2, "re-armed timer should fire again");
    }

    #[test]
    fn repeating_timer_stops_when_handler_removes_itself() {
        use core::cell::Cell;
        use std::rc::Rc;

        let mut ui = Ui::new();
        let target = ui.root();
        let count = Rc::new(Cell::new(0_u32));
        let count_for_handler = Rc::clone(&count);

        ui.on_timer(target, move |ui, _event| {
            count_for_handler.set(count_for_handler.get() + 1);
            ui.remove_timer_handler(target);
        });
        ui.schedule_timer_repeating(target, 10);

        ui.pump_timers(10);
        assert_eq!(count.get(), 1, "first timer deadline should fire");
        assert_eq!(
            ui.next_timer_deadline(),
            None,
            "repeating timer should stop after handler self-removal"
        );

        ui.pump_timers(20);
        assert_eq!(count.get(), 1, "removed handler should not fire again");
    }

    #[test]
    fn repeating_timer_stops_when_target_is_removed() {
        use core::cell::Cell;
        use std::rc::Rc;

        let mut ui = Ui::new();
        let root = ui.root();
        let target = ui.insert_child(root);
        let count = Rc::new(Cell::new(0_u32));
        let count_for_handler = Rc::clone(&count);

        ui.on_timer(target, move |ui, _event| {
            count_for_handler.set(count_for_handler.get() + 1);
            ui.remove(target);
        });
        ui.schedule_timer_repeating(target, 10);

        ui.pump_timers(10);
        assert_eq!(count.get(), 1, "first timer deadline should fire");
        assert!(
            ui.get(target).is_none(),
            "timer target should have been removed"
        );
        assert_eq!(
            ui.next_timer_deadline(),
            None,
            "repeating timer should not re-arm for a removed target"
        );

        ui.pump_timers(20);
        assert_eq!(count.get(), 1, "removed target should not receive timers");
    }

    #[test]
    fn hover_path_marks_ancestor_pseudoclasses() {
        // A child hover should mark every element along the
        // root→target path as `:hover` — the more correct CSS/WPF
        // behavior we get from hover path tracking. Sanity-check
        // by laying out a parent with a smaller padded child, hovering
        // over the child, and verifying both elements have :hover.
        let mut ui = Ui::new();
        let root = ui.root();
        let layout_prop = ui.properties().layout;
        ui.set_local(
            root,
            layout_prop,
            Layout::flex().logical_size(200.0, 200.0).padding_all(50.0),
        );
        let child = ui.insert_child(root);
        ui.set_local(
            child,
            layout_prop,
            Layout::new().flex_grow(1.0).align_self_stretch(),
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
        let a = ui.insert_child(root);
        let b = ui.insert_child(root);

        // Every element has at least one node; `node_to_element`
        // recovers it.
        for elem in [root, a, b] {
            let nodes = ui
                .presentation
                .nodes_for(elem)
                .expect("element should have tree nodes");
            assert!(!nodes.is_empty(), "element should have a tree node");
            for &node in nodes {
                assert_eq!(ui.node_to_element(node), Some(elem));
            }
        }

        // Tree structure mirrors element-tree structure: `a` and `b`
        // are children of `root`'s tree node.
        let root_node = ui.presentation.nodes_for(root).unwrap()[0];
        let a_node = ui.presentation.nodes_for(a).unwrap()[0];
        let b_node = ui.presentation.nodes_for(b).unwrap()[0];
        let children = ui.presentation.children_of(root_node);
        assert_eq!(children, &[a_node, b_node]);
        assert_eq!(ui.presentation.parent_of(a_node), Some(root_node));
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
        let source = ui.insert_child(root);
        let target = ui.insert_child(root);

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
        let source = ui.insert_child(root);
        let target = ui.insert_child(root);

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
    fn try_flush_bindings_reports_type_mismatch() {
        use alloc::string::String;
        use understory_property::Property;
        use understory_property_binding::{BindingError, PropertyEndpoint};

        let mut ui = Ui::new();
        let root = ui.root();
        let source = ui.insert_child(root);
        let target = ui.insert_child(root);

        let content_prop = ui.properties().content;
        let content_as_u32: Property<u32> = Property::from_id(content_prop.id());
        ui.bind(
            PropertyEndpoint::new(source, content_as_u32),
            PropertyEndpoint::new(target, content_as_u32),
        )
        .expect("binding registers because typed handles carry the same property id");

        ui.set_local(source, content_prop, String::from("not a u32"));

        let error = ui
            .try_flush_bindings()
            .expect_err("wrong erased source type should be reported");
        assert!(matches!(
            error.error(),
            BindingError::SourceTypeMismatch { .. }
        ));
    }

    #[test]
    fn remove_clears_bindings_for_subtree() {
        use alloc::string::String;
        use understory_property_binding::PropertyEndpoint;

        let mut ui = Ui::new();
        let root = ui.root();
        let source = ui.insert_child(root);
        let target = ui.insert_child(root);

        let content_prop = ui.properties().content;
        ui.bind(
            PropertyEndpoint::new(source, content_prop),
            PropertyEndpoint::new(target, content_prop),
        )
        .expect("binding registers");

        ui.remove(target);
        ui.set_local(source, content_prop, String::from("after-remove"));

        let report = ui
            .try_flush_bindings()
            .expect("removed target binding should have been cleared");
        assert_eq!(report.evaluated_bindings(), 0);
    }

    #[test]
    fn template_expansion_inserts_parts_under_owner() {
        use crate::widget::{AppendSpec, SurfacePartWidget, Widget};
        use crate::{Template, TemplateContext};
        use understory_style::TypeTag;

        // A unit-test widget whose default_template inserts two
        // child panels. Both should have templated_parent = the
        // owner.
        const TEST_OWNER_TAG: TypeTag = TypeTag(0xFEED_BEEF);
        const TEST_PART_A: PartTag = PartTag(0xA);
        const TEST_PART_B: PartTag = PartTag(0xB);

        struct TestTemplate;
        impl Template for TestTemplate {
            fn instantiate(&self, cx: &mut TemplateContext<'_>) {
                let _a = cx.part_widget(TEST_PART_A, Box::new(SurfacePartWidget));
                let _b = cx.part_widget(TEST_PART_B, Box::new(SurfacePartWidget));
            }
        }
        static TEST_TEMPLATE: TestTemplate = TestTemplate;

        #[derive(Debug)]
        struct TestWidget;
        impl Widget for TestWidget {
            fn type_tag(&self) -> Option<TypeTag> {
                Some(TEST_OWNER_TAG)
            }
            fn default_template(&self) -> Option<&'static dyn Template> {
                Some(&TEST_TEMPLATE)
            }
        }
        struct TestSpec;
        impl AppendSpec for TestSpec {
            fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
                ui.append_widget(parent, Box::new(TestWidget))
            }
        }

        let mut ui = Ui::new();
        let root = ui.root();
        let owner = ui.append(root, TestSpec);

        // The template should have created two children under owner, each
        // tagged with templated_parent = Some(owner).
        let owner_children = ui.get(owner).unwrap().children().to_vec();
        assert_eq!(owner_children.len(), 2, "template should insert 2 parts");
        let instance = ui
            .template_instance(owner)
            .expect("template instance should be retained");
        assert_eq!(instance.owner(), owner);
        assert_eq!(instance.part(TEST_PART_A), Some(owner_children[0]));
        assert_eq!(instance.part(TEST_PART_B), Some(owner_children[1]));
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
    fn template_spec_builds_nested_parts_classes_and_keys() {
        use crate::widget::{AppendSpec, Widget};
        use crate::{Template, TemplateItemKey, TemplateNode, TemplateSlot, TemplateSpec};
        use understory_style::TypeTag;

        const TEST_OWNER_TAG: TypeTag = TypeTag(0xFEED_CAFE);
        const OUTER_PART: PartTag = PartTag(0x10);
        const INNER_PART: PartTag = PartTag(0x11);
        const OUTER_SLOT: TemplateSlot = TemplateSlot::new("outer").with_part_tag(OUTER_PART);
        const OUTER_KEY: TemplateItemKey = TemplateItemKey::new(7);
        const OUTER_CLASS: ClassId = ClassId(0x12);
        const OUTER_CLASSES: &[ClassId] = &[OUTER_CLASS];
        const CHILDREN: &[TemplateNode] = &[TemplateNode::surface().with_part(INNER_PART)];
        const NODES: &[TemplateNode] = &[TemplateNode::surface()
            .with_slot(OUTER_SLOT)
            .with_key(OUTER_KEY)
            .with_classes(OUTER_CLASSES)
            .with_children(CHILDREN)];
        static SPEC: TemplateSpec = TemplateSpec::new(NODES);

        #[derive(Debug)]
        struct TestWidget;
        impl Widget for TestWidget {
            fn type_tag(&self) -> Option<TypeTag> {
                Some(TEST_OWNER_TAG)
            }
            fn default_template(&self) -> Option<&'static dyn Template> {
                Some(&SPEC)
            }
        }
        struct TestSpec;
        impl AppendSpec for TestSpec {
            fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
                ui.append_widget(parent, Box::new(TestWidget))
            }
        }

        let mut ui = Ui::new();
        let owner = ui.append(ui.root(), TestSpec);
        let instance = ui.template_instance(owner).unwrap();
        let outer = instance.keyed_part(OUTER_PART, OUTER_KEY).unwrap();
        let inner = instance.part(INNER_PART).unwrap();

        assert_eq!(instance.keyed_slot(OUTER_SLOT, OUTER_KEY), Some(outer));
        assert_eq!(ui.get(outer).unwrap().parent(), Some(owner));
        assert_eq!(ui.get(inner).unwrap().parent(), Some(outer));
        assert_eq!(ui.get(outer).unwrap().templated_parent(), Some(owner));
        assert_eq!(ui.get(inner).unwrap().templated_parent(), Some(owner));
        assert!(ui.get(outer).unwrap().classes().contains(&OUTER_CLASS));
    }

    #[test]
    fn two_independent_bindings_both_propagate() {
        // Pure-binding analogue of the two-button scenario: two
        // source/target pairs registered + written in sequence.
        // Both targets should receive their bound value on flush.
        use alloc::string::String;
        use understory_property_binding::PropertyEndpoint;

        let mut ui = Ui::new();
        let root = ui.root();
        let s1 = ui.insert_child(root);
        let t1 = ui.insert_child(root);
        let s2 = ui.insert_child(root);
        let t2 = ui.insert_child(root);

        let content_prop = ui.properties().content;
        ui.bind(
            PropertyEndpoint::new(s1, content_prop),
            PropertyEndpoint::new(t1, content_prop),
        )
        .unwrap();
        ui.set_local(s1, content_prop, String::from("one"));

        ui.bind(
            PropertyEndpoint::new(s2, content_prop),
            PropertyEndpoint::new(t2, content_prop),
        )
        .unwrap();
        ui.set_local(s2, content_prop, String::from("two"));

        ui.flush_bindings();

        let read = |ui: &Ui, id: ElementId| -> Option<String> {
            ui.get(id)
                .unwrap()
                .property_store()
                .effective_local_erased(content_prop.id())
                .and_then(|v| v.downcast_ref::<String>().cloned())
        };
        assert_eq!(read(&ui, t1).as_deref(), Some("one"));
        assert_eq!(read(&ui, t2).as_deref(), Some("two"));
    }

    #[test]
    fn button_label_content_propagates_through_template_binding() {
        // The button's template creates a `Text` label, then binds
        // owner.content → label.content. If `Text::append_to` writes
        // `Local=""` on the label, that Local would shadow the
        // binding (Local outranks TemplateBinding) and the label
        // would never display the button's text. Regression guard
        // for two sibling buttons (each must propagate independently).
        use crate::Button;

        let mut ui = Ui::new();
        let root = ui.root();
        let b1 = ui.append(root, Button::new("Click me"));
        let b2 = ui.append(root, Button::new("Or me"));

        ui.flush_bindings();

        let content_prop = ui.properties().content;
        let read_label = |ui: &Ui, button: ElementId| -> Option<String> {
            let label = ui.part(button, crate::PART_BUTTON_LABEL).unwrap();
            ui.get(label)
                .unwrap()
                .property_store()
                .effective_local_erased(content_prop.id())
                .and_then(|v| v.downcast_ref::<String>().cloned())
        };
        assert_eq!(read_label(&ui, b1).as_deref(), Some("Click me"));
        assert_eq!(read_label(&ui, b2).as_deref(), Some("Or me"));
    }

    #[test]
    fn compute_layout_writes_bounds() {
        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.insert_child(root);

        let layout_prop = ui.properties().layout;
        // Root fills the viewport with 10px padding.
        ui.set_local(
            root,
            layout_prop,
            Layout::flex().percent_size(1.0, 1.0).padding_all(10.0),
        );
        // Child fills the padded content.
        ui.set_local(
            child,
            layout_prop,
            Layout::new().flex_grow(1.0).align_self_stretch(),
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
    fn press_marks_templated_control_not_inner_part() {
        // Pressing on a templated control's part (Button's label
        // `Text`) should put `:pressed` on the Button itself, not
        // on the label. Without this, the cascade rule keyed on
        // `TYPE_BUTTON:pressed` never matches and the button shows
        // no press feedback. Use `press_visual_target` directly so
        // the test doesn't depend on layout + hit-test plumbing.
        use crate::Button;

        let mut ui = Ui::new();
        let root = ui.root();
        let button = ui.append(root, Button::new("Click me"));
        let label = ui.part(button, crate::PART_BUTTON_LABEL).unwrap();

        assert_eq!(ui.press_visual_target(label), button);
        assert_eq!(ui.press_visual_target(button), button);
        assert_eq!(ui.press_visual_target(root), root);
    }

    #[test]
    fn disabled_button_carries_state_and_ignores_builder_click() {
        use alloc::rc::Rc;
        use core::cell::Cell;

        use crate::Button;

        let mut ui = Ui::new();
        let root = ui.root();
        let fired = Rc::new(Cell::new(false));
        let fired_for_handler = Rc::clone(&fired);
        let button = ui.append(
            root,
            Button::new("No")
                .disabled()
                .on_click(move |_ui, _event| fired_for_handler.set(true)),
        );

        assert!(!ui.is_enabled(button));
        assert!(ui.get(button).unwrap().pseudos().contains(&PSEUDO_DISABLED));

        ui.synthesize_click(button, Point::ZERO);
        assert!(
            !fired.get(),
            "disabled button click handler should not fire"
        );

        assert!(!ui.set_focus(Some(button)));
        assert_eq!(ui.focused(), None, "disabled button should reject focus");
    }

    #[test]
    fn set_enabled_false_clears_focus_and_pressed_state() {
        use crate::Button;

        let mut ui = Ui::new();
        let root = ui.root();
        let button = ui.append(root, Button::new("Run"));

        assert!(ui.set_focus(Some(button)));
        ui.set_press_target(Some(button));
        ui.set_enabled(button, false);

        assert!(!ui.is_enabled(button));
        assert_eq!(ui.focused(), None);
        assert_eq!(ui.events.press_target(), None);
        let pseudos = ui.get(button).unwrap().pseudos();
        assert!(pseudos.contains(&PSEUDO_DISABLED));
        assert!(!pseudos.contains(&PSEUDO_FOCUSED));
        assert!(!pseudos.contains(&PSEUDO_FOCUS_VISIBLE));
        assert!(!pseudos.contains(&PSEUDO_PRESSED));

        ui.set_enabled(button, true);
        assert!(ui.is_enabled(button));
        assert!(
            ui.is_focusable(button),
            "re-enabling should restore the stored focusable preference"
        );
        assert!(ui.set_focus(Some(button)));
    }

    #[test]
    fn disabled_toggle_and_slider_share_control_state_policy() {
        use alloc::rc::Rc;
        use core::cell::Cell;

        use crate::{Slider, ToggleSwitch};

        let mut ui = Ui::new();
        let root = ui.root();

        let toggle = ui.append(root, ToggleSwitch::new().disabled());
        assert!(!ui.is_enabled(toggle));
        assert!(!ui.is_focusable(toggle));
        assert!(ui.get(toggle).unwrap().pseudos().contains(&PSEUDO_DISABLED));
        ui.synthesize_click(toggle, Point::ZERO);
        let checked = ui
            .get(toggle)
            .and_then(|el| el.get_local_value(ui.properties().is_checked).copied())
            .unwrap_or(false);
        assert!(!checked, "disabled toggle should ignore user activation");

        let fired = Rc::new(Cell::new(false));
        let fired_for_handler = Rc::clone(&fired);
        let slider = ui.append(root, Slider::new().disabled());
        ui.on_click(slider, move |_ui, _event| fired_for_handler.set(true));
        assert!(!ui.is_enabled(slider));
        assert!(!ui.is_focusable(slider));
        assert!(ui.get(slider).unwrap().pseudos().contains(&PSEUDO_DISABLED));
        ui.synthesize_click(slider, Point::ZERO);
        assert!(
            !fired.get(),
            "disabled slider should suppress click dispatch"
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
    fn add_class_restyles_subtree() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.add_class(root, ClassId(1));
        // `add_class` restyles synchronously — the class is on the
        // element afterward and any cascade-driven channel changes
        // are already marked via `restyle_subtree`. With an empty
        // cascade (this test's case) no channels move.
        assert!(ui.get(root).unwrap().classes().contains(&ClassId(1)));
    }

    fn slider_value(ui: &Ui, slider: ElementId) -> Option<f64> {
        ui.get(slider)
            .and_then(|el| el.get_local_value(ui.properties().value).copied())
    }

    fn pointer_down(point: Point, time: u64) -> PointerEvent {
        use ui_events::pointer::{PointerButton, PointerButtonEvent, PointerEvent};

        PointerEvent::Down(PointerButtonEvent {
            button: Some(PointerButton::Primary),
            pointer: pointer_info(),
            state: pointer_state(point, time, true),
        })
    }

    fn pointer_up(point: Point, time: u64) -> PointerEvent {
        use ui_events::pointer::{PointerButton, PointerButtonEvent, PointerEvent};

        PointerEvent::Up(PointerButtonEvent {
            button: Some(PointerButton::Primary),
            pointer: pointer_info(),
            state: pointer_state(point, time, false),
        })
    }

    fn pointer_move(point: Point, time: u64, primary_down: bool) -> PointerEvent {
        use ui_events::pointer::{PointerEvent, PointerUpdate};

        PointerEvent::Move(PointerUpdate {
            pointer: pointer_info(),
            current: pointer_state(point, time, primary_down),
            coalesced: Vec::new(),
            predicted: Vec::new(),
        })
    }

    fn pointer_scroll(point: Point, time: u64, delta: ScrollDelta) -> PointerEvent {
        use ui_events::pointer::{PointerEvent, PointerScrollEvent};

        PointerEvent::Scroll(PointerScrollEvent {
            pointer: pointer_info(),
            delta,
            state: pointer_state(point, time, false),
        })
    }

    fn pointer_info() -> ui_events::pointer::PointerInfo {
        use ui_events::pointer::{PointerId, PointerInfo, PointerType};

        PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        }
    }

    fn pointer_state(
        point: Point,
        time: u64,
        primary_down: bool,
    ) -> ui_events::pointer::PointerState {
        use ui_events::pointer::{PointerButton, PointerButtons, PointerState};

        let mut buttons = PointerButtons::new();
        if primary_down {
            buttons.insert(PointerButton::Primary);
        }
        let mut state = PointerState {
            time,
            buttons,
            count: 1,
            pressure: if primary_down { 0.5 } else { 0.0 },
            ..PointerState::default()
        };
        state.position.x = point.x;
        state.position.y = point.y;
        state
    }
}
