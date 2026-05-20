// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Per-`ElementKind` behavior dispatch.
//!
//! Phase 1b only models paint; later phases add measure / arrange / event
//! handlers / template-apply hooks. The trait stays narrow until the toolkit
//! actually needs each surface.

use core::fmt;

use hashbrown::HashMap;
use imaging::record::Scene;
use understory_property::PropertyRegistry;
use understory_style::{MatchState, ResolveCx, StyleCascade, Theme};

use crate::element::{Element, ElementId, ElementKind};
use crate::lookup::UiStoreLookup;
use crate::properties::BuiltInProperties;

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
