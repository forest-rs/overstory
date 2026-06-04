// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The root [`Ui`] context: public facade and coordinator for one retained
//! Overstory tree.
//!
//! Phase 1c added the style cascade integration; Phase 1d added pointer
//! dispatch + `:hover` lifecycle; Phase 1e added layout; Phase 1f
//! added template substrate, text rendering, `:pressed`, the Button
//! control, and property bindings via `understory_property_binding`. Later
//! phases bring the full responder routing, more controls, and timeline.
//!
//! Roadmap:
//! - Keep `Ui` as the app-facing facade while pushing owned subsystems into
//!   focused runtimes. Public convenience lives here; subsystem policy should
//!   not collapse back into one large file.
//! - Make the frame pipeline explicit: flush bindings, restyle, expand
//!   templates, compute layout, update presentation, update composition, and
//!   expose host-consumable changes.
//! - Add diagnostics APIs for invalidation, layout, text cache, event routing,
//!   focus, timers, overlays, presentation, and composition.
//! - Keep platform and renderer work in host crates while `Ui` exposes enough
//!   normalized hooks for hosts to drive input, timers, accessibility, and
//!   frame presentation.
//! - Continue splitting implementation modules by system as the facade grows,
//!   and promote repeated patterns down into Understory crates when they stop
//!   being Overstory-specific.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cell::RefCell;

use invalidation::{ChannelSet, InvalidationTracker};
use understory_property::PropertyRegistry;

use crate::binding::BindingSourceRuntime;
use crate::binding_runtime::BindingRuntime;
use crate::channels::{COMPOSE, LAYOUT, PAINT, STYLE, TEMPLATE};
use crate::composition::{CompositionRuntime, CompositionSurface};
use crate::controls::panel::PanelWidget;
use crate::diagnostics::{UiFrameStats, UiFrameTimings, UiRendererTimings};
use crate::element::{Element, ElementId};
use crate::element_store::ElementStore;
use crate::event_runtime::EventRuntime;
use crate::focus_runtime::FocusRuntime;
use crate::geometry_runtime::GeometryRuntime;
use crate::inspection_runtime::InspectionChangeRuntime;
use crate::layout_runtime::LayoutRuntime;
use crate::overlay::OverlayRuntime;
use crate::presentation_runtime::PresentationRuntime;
use crate::properties::BuiltInProperties;
use crate::scroll_runtime::ScrollRuntime;
use crate::style_runtime::StyleRuntime;
use crate::template::TemplateRuntime;
use crate::text::{TextEngine, TextLayoutCacheStats, TextResources};
use crate::timeline_properties::TimelinePropertyRegistry;
use crate::timeline_runtime::TimelineRuntime;
use crate::timer_runtime::TimerRuntime;
use crate::ui::debug_names::StyleTokenDebugNames;

mod bindings;
mod debug_names;
mod events;
mod inspection;
mod layout;
mod overlays;
mod paint;
mod popups;
mod scroll;
mod state;
mod style;
mod timeline;
mod timers;
mod tooltips;
mod transition_planner;
mod tree;
mod visual;

pub use tree::ChildUi;

use transition_planner::MotionPolicy;

#[cfg(test)]
mod tests;

/// Public facade for an Overstory UI instance.
///
/// `Ui` keeps the app-facing API concentrated in one object while delegating
/// narrow responsibilities to internal runtimes such as `ElementStore` and
/// `TemplateRuntime`.
pub struct Ui {
    /// Retained element tree and per-element widget/property storage.
    elements: ElementStore,
    /// Property metadata registry used for typed property storage and
    /// invalidation-channel lookup.
    registry: PropertyRegistry,
    /// Built-in property ids registered into `registry`.
    properties: BuiltInProperties,
    /// Dirty-channel tracker keyed by element id.
    invalidation: InvalidationTracker<ElementId>,
    /// Box-tree geometry, hit testing, spatial index, and focus flags.
    geometry: GeometryRuntime,
    /// Retained presentation primitives keyed by geometry nodes.
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
    /// One-way property binding graph and binding invalidation state.
    bindings: BindingRuntime,
    /// Local app state and external value adapters that feed bindings.
    binding_sources: BindingSourceRuntime,
    /// Routed event handlers, pointer capture, hover, click, and drag state.
    events: EventRuntime,
    /// Focused element, focusable preferences, and keyboard navigation policy.
    focus: FocusRuntime,
    /// Internal presentation timeline tracks sampled from host time.
    timeline: TimelineRuntime,
    /// Internal policy for planning presentation transitions from state changes.
    motion_policy: MotionPolicy,
    /// Property adapters that can receive timeline samples.
    timeline_properties: TimelinePropertyRegistry,
    /// Retained timer queue and per-element timer handlers.
    timers: TimerRuntime,
    /// Tooltip and popup declarations plus currently active overlay stack.
    overlays: OverlayRuntime,
    /// Scrollable target registry shared by scrollbars and semantic actions.
    scrolling: ScrollRuntime,
    /// Cheap counters for the current or most recently prepared frame.
    frame_stats: UiFrameStats,
    /// Live inspection change accumulator drained by devtools/hosts.
    inspection_changes: InspectionChangeRuntime,
    /// Human-readable debug names for style tokens.
    style_debug_names: StyleTokenDebugNames,
    /// Last viewport size used during layout/paint.
    last_viewport: kurbo::Size,
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
    /// This constructs platform text resources for this UI. Use
    /// [`Self::with_text_resources`] when constructing many UIs that should
    /// share font discovery and source-cache state.
    ///
    /// Use [`Self::set_cascade`] / [`Self::set_theme`] to install
    /// application-supplied styling.
    #[cfg(feature = "system-fonts")]
    #[must_use]
    pub fn new() -> Self {
        Self::with_text_resources(&TextResources::system_fonts())
    }

    /// Creates a new [`Ui`] with explicit shared text resources.
    ///
    /// Use this when a host or benchmark constructs multiple UIs and wants
    /// font discovery and source-cache ownership to stay outside the UI
    /// instance. Each UI still receives its own mutable text layout context.
    #[must_use]
    pub fn with_text_resources(text_resources: &TextResources) -> Self {
        Self::with_text_engine(TextEngine::from_resources(text_resources))
    }

    /// Clears cached shaped text layouts.
    pub fn clear_text_layout_cache(&mut self) {
        self.text.borrow_mut().clear_layout_cache();
    }

    /// Returns the number of cached shaped text layouts.
    #[must_use]
    pub fn text_layout_cache_len(&self) -> usize {
        self.text.borrow().layout_cache_len()
    }

    /// Returns shaped text layout cache counters.
    #[must_use]
    pub fn text_layout_cache_stats(&self) -> TextLayoutCacheStats {
        self.text.borrow().layout_cache_stats()
    }

    fn with_text_engine(text_engine: TextEngine) -> Self {
        let mut registry = PropertyRegistry::new();
        let properties = BuiltInProperties::register(&mut registry);
        let mut timeline_properties = TimelinePropertyRegistry::new();
        timeline_properties.register_solid_brush_color(properties.background);
        timeline_properties.register_solid_brush_color(properties.border_brush);
        timeline_properties.register(properties.opacity);
        timeline_properties.register(properties.border_width);
        timeline_properties.register(properties.corner_radius);
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
            geometry: GeometryRuntime::new(),
            presentation: PresentationRuntime::new(),
            composition: CompositionRuntime::new(),
            style: StyleRuntime::new(),
            layout: LayoutRuntime::new(),
            templates: TemplateRuntime::new(),
            text: RefCell::new(text_engine),
            bindings: BindingRuntime::new(),
            binding_sources: BindingSourceRuntime::new(),
            events: EventRuntime::new(),
            focus: FocusRuntime::new(),
            timeline: TimelineRuntime::new(),
            motion_policy: MotionPolicy::new(),
            timeline_properties,
            timers: TimerRuntime::new(),
            overlays: OverlayRuntime::default(),
            scrolling: ScrollRuntime::new(),
            frame_stats: UiFrameStats::default(),
            inspection_changes: InspectionChangeRuntime::default(),
            style_debug_names: StyleTokenDebugNames::default(),
            last_viewport: kurbo::Size::ZERO,
        };
        me.register_builtin_style_debug_names();
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
        if set.is_empty() {
            return;
        }
        for channel in set.iter() {
            self.frame_stats.channel_marks.record(channel);
            self.invalidation.mark(id, channel);
        }
        self.record_element_tree_upsert(id);
    }

    /// Clears frame-pipeline counters before a host prepares a new frame.
    pub fn reset_frame_stats(&mut self) {
        let renderer_timings = self.frame_stats.renderer_timings;
        self.frame_stats = UiFrameStats {
            renderer_timings,
            ..UiFrameStats::default()
        };
    }

    /// Records host-measured timings for the most recently prepared frame.
    ///
    /// Core Overstory owns frame counters, but hosts own wall-clock time
    /// collection. Host crates call this after preparing a frame so inspection
    /// snapshots can report the same timing data as host diagnostics.
    pub fn set_frame_timings(&mut self, timings: UiFrameTimings) {
        self.frame_stats.timings = timings;
        self.inspection_changes.record_frame();
    }

    /// Records host-measured backend renderer timings for the most recently
    /// submitted frame.
    ///
    /// These timings are separate from [`Self::set_frame_timings`] because
    /// platform hosts usually learn renderer submission costs only after
    /// `prepare_frame` has returned the scene to render.
    pub fn set_renderer_timings(&mut self, timings: UiRendererTimings) {
        self.frame_stats.renderer_timings = timings;
        self.inspection_changes.record_frame();
    }

    /// Returns a snapshot of the current frame-pipeline counters.
    #[must_use]
    pub fn frame_stats(&self) -> UiFrameStats {
        let mut stats = self.frame_stats;
        stats.elements = self.len();
        stats.composition_surfaces = self.composition_surfaces().count();
        stats.composition_surface_updates = self.composition_surface_updates().count();
        stats.text_layout_cache = self.text_layout_cache_stats();
        stats
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

    /// Returns every Overstory-owned scene currently attached to a composition
    /// surface.
    ///
    /// These scenes are keyed by the [`subduction_core::layer::SurfaceId`]
    /// attached to layers in [`Self::composition_layers`]. Hosts can use this
    /// as the scene content registry until a backend replaces selected
    /// surfaces with GPU or platform resources.
    pub fn composition_surfaces(&self) -> impl Iterator<Item = CompositionSurface<'_>> + '_ {
        self.composition.surfaces()
    }

    /// Returns surfaces whose Overstory-owned scenes were rewritten by the
    /// most recent [`Self::paint`] call.
    pub fn composition_surface_updates(&self) -> impl Iterator<Item = CompositionSurface<'_>> + '_ {
        self.composition.surface_updates()
    }

    /// Evaluates Subduction layer dirty state and refreshes
    /// [`Self::composition_changes`].
    pub fn flush_composition(&mut self) {
        self.sync_composition_values();
        self.composition.evaluate();
    }
}

#[cfg(feature = "system-fonts")]
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
