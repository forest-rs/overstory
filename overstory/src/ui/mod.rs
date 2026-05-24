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

use invalidation::{ChannelSet, InvalidationTracker};
use understory_property::PropertyRegistry;

use crate::binding_runtime::BindingRuntime;
use crate::channels::{COMPOSE, LAYOUT, PAINT, STYLE, TEMPLATE};
use crate::composition::{CompositionRuntime, CompositionSurface};
use crate::controls::panel::PanelWidget;
use crate::element::{Element, ElementId};
use crate::element_store::ElementStore;
use crate::event_runtime::EventRuntime;
use crate::focus_runtime::FocusRuntime;
use crate::layout_runtime::LayoutRuntime;
use crate::presentation_runtime::PresentationRuntime;
use crate::properties::BuiltInProperties;
use crate::style_runtime::StyleRuntime;
use crate::template_runtime::TemplateRuntime;
use crate::text::TextEngine;
use crate::timer_runtime::TimerRuntime;

mod bindings;
mod events;
mod layout;
mod paint;
mod state;
mod style;
mod timers;
mod tree;

#[cfg(test)]
mod tests;

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
    /// Compositor-neutral layer topology and geometry.
    composition: CompositionRuntime,
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
            composition: CompositionRuntime::new(),
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

    /// Returns the retained Subduction layer store derived from this UI tree.
    ///
    /// Layer state is updated by [`Self::compute_layout`]. Hosts that realize
    /// compositor-backed presentation can read this store after layout and pass
    /// it to a Subduction presenter along with the evaluated frame changes.
    #[must_use]
    #[inline]
    pub fn composition_layers(&self) -> &subduction_core::layer::LayerStore {
        self.composition.layers()
    }

    /// Returns the most recent evaluated Subduction layer changes.
    ///
    /// This is refreshed by [`Self::flush_composition`]. Layout calls flush
    /// composition after synchronizing geometry, and hosts can explicitly flush
    /// before applying a Subduction presenter.
    #[must_use]
    #[inline]
    pub fn composition_changes(&self) -> &subduction_core::layer::FrameChanges {
        self.composition.last_changes()
    }

    /// Returns every software scene currently attached to a composition
    /// surface.
    ///
    /// These scenes are keyed by the [`subduction_core::layer::SurfaceId`]
    /// attached to layers in [`Self::composition_layers`]. Hosts can use this
    /// as the software content registry until a backend replaces selected
    /// surfaces with GPU or platform resources.
    pub fn composition_surfaces(&self) -> impl Iterator<Item = CompositionSurface<'_>> + '_ {
        self.composition.surfaces()
    }

    /// Returns surfaces whose software scenes were rewritten by the most
    /// recent [`Self::paint`] call.
    pub fn composition_surface_updates(&self) -> impl Iterator<Item = CompositionSurface<'_>> + '_ {
        self.composition.surface_updates()
    }

    /// Evaluates Subduction layer dirty state and refreshes
    /// [`Self::composition_changes`].
    pub fn flush_composition(&mut self) {
        self.composition.evaluate();
    }
}

impl Default for Ui {
    fn default() -> Self {
        Self::new()
    }
}

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
