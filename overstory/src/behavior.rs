// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Per-`ElementKind` behavior dispatch.
//!
//! A behavior's responsibilities are split across two passes:
//!
//! * [`ElementBehavior::measure`] — leaf-like behaviors that compute their
//!   own desired size from properties (text, images, …) override this so
//!   the layout engine can ask them.
//! * [`ElementBehavior::present`] — every paintable behavior resolves its
//!   styled properties (background, border, text content, foreground, …)
//!   through the cascade and writes the resolved values into a
//!   [`understory_presentation::PresentationStore`] node. Paint is then a
//!   geometry-only walk that lowers presentation primitives + box-tree
//!   bounds into draws — it reads no properties.

use core::cell::RefCell;
use core::fmt;

use hashbrown::HashMap;
use kurbo::Size;
use understory_box_tree::NodeId;
use understory_presentation::PresentationStore;
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

/// Read-only context handed to [`ElementBehavior::present`].
///
/// Bundles everything a behavior needs to resolve effective property
/// values through the full precedence chain (Animation → Local → Style →
/// Theme → Inherited → Default). The behavior typically builds a
/// transient [`ResolveCx`] via [`Self::resolve_cx`] and reads each
/// property it cares about, then writes the result into the presentation
/// store. Geometry, transforms, and text shaping aren't visible here —
/// those happen at lower time against the box tree's world bounds.
#[derive(Copy, Clone)]
pub struct PresentContext<'a> {
    /// The property registry every property write is checked against.
    pub registry: &'a PropertyRegistry,
    /// Type-safe handles for the toolkit's built-in properties.
    pub properties: &'a BuiltInProperties,
    /// Active theme. Resources referenced by style entries resolve through it.
    pub theme: &'a Theme,
    /// The compiled style cascade rules apply through.
    pub cascade: &'a StyleCascade,
    /// The match state for the element currently being presented.
    /// Behaviors pass this into [`ResolveCx::get_value`] alongside
    /// `cascade` to read styled values.
    pub match_state: MatchState,
    /// Parent-walking lookup the [`ResolveCx`] uses for inheritance.
    pub(crate) lookup: UiStoreLookup<'a>,
}

impl<'a> PresentContext<'a> {
    /// Builds a [`ResolveCx`] tied to this context.
    #[must_use]
    pub fn resolve_cx(&self) -> ResolveCx<'a, ElementId, UiStoreLookup<'a>> {
        ResolveCx::new(self.registry, self.theme, self.lookup)
    }
}

impl fmt::Debug for PresentContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PresentContext").finish_non_exhaustive()
    }
}

/// Read-only context handed to [`ElementBehavior::measure`].
///
/// Mirrors [`PresentContext`] minus the cascade-inheritance lookup —
/// `measure` is called from inside the layout engine's split-borrow walk
/// so the toolkit can't hand out a `&Ui`. Behaviors that need inherited
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
/// and looked up by [`ElementKind`]; an [`Element`] holds no behavior
/// storage of its own. Implementations are expected to be stateless — any
/// per-element state lives in the element's
/// [`understory_property::PropertyStore`].
pub trait ElementBehavior: Sync {
    /// Resolves the element's styled properties through the cascade and
    /// writes the result into the presentation store for `node`.
    ///
    /// `node` is the element's primary box-tree node. Most behaviors will
    /// take `store.surface_mut(node)` / `store.plain_text_mut(node)` and
    /// populate the corresponding primitive's fields. The default
    /// implementation is a no-op for behaviors that contribute no draws
    /// (e.g. layout-only containers — though most still want a surface).
    fn present(
        &self,
        element: &Element,
        ctx: &PresentContext<'_>,
        store: &mut PresentationStore<NodeId, ElementId>,
        node: NodeId,
    ) {
        let _ = (element, ctx, store, node);
    }

    /// Returns the default template factory for this kind, if any.
    fn default_template(&self) -> Option<&'static dyn Template> {
        None
    }

    /// Optional custom measure callback for leaf-like behaviors that
    /// compute their own desired size (text, images, custom-drawn
    /// primitives).
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
