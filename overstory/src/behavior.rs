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

use crate::element::{Element, ElementKind};
use crate::properties::BuiltInProperties;

/// Read-only context handed to [`ElementBehavior::paint`].
///
/// Holds the registry and built-in property handles so behaviors can resolve
/// their effective values via the [`understory_property`] precedence chain.
#[derive(Copy, Clone)]
pub struct PaintContext<'a> {
    /// The property registry every property write is checked against.
    pub registry: &'a PropertyRegistry,
    /// Type-safe handles for the toolkit's built-in properties.
    pub properties: &'a BuiltInProperties,
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
