// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Per-`ElementKind` behavior dispatch.
//!
//! Phase 1b only models paint; later phases add measure / arrange / event
//! handlers / template-apply hooks. The trait stays narrow until the toolkit
//! actually needs each surface.

use core::cell::RefCell;
use core::fmt;

use hashbrown::HashMap;
use imaging::record::Scene;
use kurbo::Size;
use understory_property::PropertyRegistry;
use understory_style::{MatchState, ResolveCx, StyleCascade, Theme};

use crate::element::{Element, ElementId, ElementKind};
use crate::lookup::UiStoreLookup;
use crate::properties::BuiltInProperties;
use crate::template::Template;
use crate::text::TextEngine;

/// Per-axis known dimensions handed to a behavior's `measure` callback.
///
/// `Some(value)` means the parent has already chosen that axis's size, and
/// the behavior should respect it. `None` means the behavior is free to
/// decide that axis.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct KnownDimensions {
    /// Pre-chosen width, in logical pixels.
    pub width: Option<f64>,
    /// Pre-chosen height, in logical pixels.
    pub height: Option<f64>,
}

/// Constraints passed to [`ElementBehavior::measure`].
///
/// `available` reports how much room the parent has on each axis;
/// `f64::INFINITY` means "unconstrained." `known` carries any axis that the
/// parent has already decided.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MeasureConstraints {
    /// Maximum size the parent can give. `f64::INFINITY` on an axis means the
    /// parent imposes no upper bound.
    pub available: Size,
    /// Axes whose size the parent has already fixed.
    pub known: KnownDimensions,
}

/// Read-only context handed to [`ElementBehavior::paint`].
///
/// Bundles everything a behavior needs to resolve effective values through
/// the full precedence chain (Animation → Local → Style → Theme →
/// Inherited → Default). The behavior typically builds a transient
/// [`ResolveCx`] via [`Self::resolve_cx`].
#[derive(Copy, Clone)]
pub struct PaintContext<'a> {
    /// The property registry every property write is checked against.
    pub registry: &'a PropertyRegistry,
    /// Type-safe handles for the toolkit's built-in properties.
    pub properties: &'a BuiltInProperties,
    /// Active theme. Resources referenced by style entries resolve through it.
    pub theme: &'a Theme,
    /// The compiled style cascade rules apply through.
    pub cascade: &'a StyleCascade,
    /// The match state for the element currently being painted. Behaviors
    /// pass this into [`ResolveCx::get_value`] alongside `cascade` to read
    /// styled values.
    pub match_state: MatchState,
    /// Parent-walking lookup the [`ResolveCx`] uses for inheritance.
    pub(crate) lookup: UiStoreLookup<'a>,
    /// Shared text shaping engine (parley). Behaviors that render text
    /// `borrow_mut()` it for the duration of a shape/paint call.
    pub text: &'a RefCell<TextEngine>,
}

impl<'a> PaintContext<'a> {
    /// Builds a [`ResolveCx`] tied to this context.
    ///
    /// The returned `ResolveCx` borrows from the context for the lifetime of
    /// the surrounding `Ui` reference, so it can be used freely within a
    /// behavior's `paint` call.
    #[must_use]
    pub fn resolve_cx(&self) -> ResolveCx<'a, ElementId, UiStoreLookup<'a>> {
        ResolveCx::new(self.registry, self.theme, self.lookup)
    }
}

impl fmt::Debug for PaintContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PaintContext").finish_non_exhaustive()
    }
}

/// Read-only context handed to [`ElementBehavior::measure`].
///
/// Mirrors [`PaintContext`] minus the cascade-inheritance lookup —
/// `measure` is called from inside the layout engine's split-borrow walk so
/// the toolkit can't hand out a `&Ui`. Behaviors that need inherited
/// cascade values in measure should read them from the cascade and match
/// state directly; the simple per-property local read covers most cases.
#[derive(Copy, Clone)]
pub struct MeasureContext<'a> {
    /// The property registry every property write is checked against.
    pub registry: &'a PropertyRegistry,
    /// Type-safe handles for the toolkit's built-in properties.
    pub properties: &'a BuiltInProperties,
    /// Active theme.
    pub theme: &'a Theme,
    /// The compiled style cascade.
    pub cascade: &'a StyleCascade,
    /// The match state for the element currently being measured.
    pub match_state: MatchState,
    /// Shared text shaping engine.
    pub text: &'a RefCell<TextEngine>,
}

impl fmt::Debug for MeasureContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MeasureContext").finish_non_exhaustive()
    }
}

/// Per-kind behavior contract.
///
/// Behaviors are registered once at startup as `&'static dyn ElementBehavior`
/// and looked up by [`ElementKind`]; an [`Element`] holds no behavior storage
/// of its own. Implementations are expected to be stateless — any per-element
/// state lives in the element's [`understory_property::PropertyStore`].
pub trait ElementBehavior: Sync {
    /// Records the element's contribution to its own per-element scene cache.
    ///
    /// `scene` is a fresh, owned [`Scene`] dedicated to this element; the
    /// caller will compose it into the frame's output scene.
    fn paint(&self, element: &Element, ctx: &PaintContext<'_>, scene: &mut Scene);

    /// Returns the default template factory for this kind, if any.
    ///
    /// When an element of this kind is inserted, the toolkit invokes the
    /// returned template's `instantiate` method to build its visual subtree.
    /// Primitive kinds (e.g. `ELEMENT_KIND_GENERIC`) return `None`.
    fn default_template(&self) -> Option<&'static dyn Template> {
        None
    }

    /// Optional custom measure callback for leaf-like behaviors that compute
    /// their own desired size (text, images, custom-drawn primitives).
    ///
    /// Returns `None` (the default) to let the layout engine size the element
    /// purely from its `Layout` style. Returns `Some(size)` to override.
    ///
    /// Behaviors receive kurbo-based [`MeasureConstraints`]; the toolkit
    /// translates between these and the underlying layout engine's types
    /// (currently `taffy`), so behaviors don't depend on a specific engine.
    fn measure(
        &self,
        element: &Element,
        ctx: &MeasureContext<'_>,
        constraints: MeasureConstraints,
    ) -> Option<Size> {
        let _ = (element, ctx, constraints);
        None
    }
}

/// Maps each [`ElementKind`] to its `'static` behavior implementation.
///
/// The toolkit registers built-in kinds during [`crate::Ui::new`]; application
/// code can extend the registry via [`Self::register`].
#[derive(Default)]
pub struct BehaviorRegistry {
    behaviors: HashMap<ElementKind, &'static dyn ElementBehavior>,
}

impl fmt::Debug for BehaviorRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BehaviorRegistry")
            .field("registered_kinds", &self.behaviors.len())
            .finish()
    }
}

impl BehaviorRegistry {
    /// Constructs an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `behavior` for `kind`, replacing any previous registration.
    pub fn register(&mut self, kind: ElementKind, behavior: &'static dyn ElementBehavior) {
        self.behaviors.insert(kind, behavior);
    }

    /// Returns the behavior registered for `kind`, if any.
    #[must_use]
    pub fn get(&self, kind: ElementKind) -> Option<&'static dyn ElementBehavior> {
        self.behaviors.get(&kind).copied()
    }
}
