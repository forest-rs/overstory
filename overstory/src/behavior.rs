// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Context types handed to widget trait methods.
//!
//! The widget trait itself lives in [`crate::widget`]. These are the
//! pieces that get passed to its `present` and `measure` hooks —
//! the resolve / cascade plumbing for property reads + the text
//! engine for measure-time shaping.

use core::cell::RefCell;
use core::fmt;

use kurbo::Size;
use understory_property::PropertyRegistry;
use understory_style::{MatchState, ResolveCx, StyleCascade, Theme};

use crate::element::ElementId;
use crate::lookup::UiStoreLookup;
use crate::properties::BuiltInProperties;
use crate::text::TextEngine;

/// Per-axis known dimensions handed to a behavior's `measure`
/// callback.
///
/// `Some(value)` means the parent has already chosen that axis's
/// size, and the behavior should respect it. `None` means the
/// behavior is free to decide that axis.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct KnownDimensions {
    /// Pre-chosen width, in logical pixels.
    pub width: Option<f64>,
    /// Pre-chosen height, in logical pixels.
    pub height: Option<f64>,
}

/// Constraints passed to a widget's `measure` callback.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MeasureConstraints {
    /// Maximum size the parent can give. `f64::INFINITY` on an axis
    /// means the parent imposes no upper bound.
    pub available: Size,
    /// Axes whose size the parent has already fixed.
    pub known: KnownDimensions,
}

/// Read-only context handed to a widget's `present` callback.
///
/// Bundles everything needed to resolve effective property values
/// through the full precedence chain (Animation → Local → Style →
/// Theme → Inherited → Default). The behavior typically builds a
/// transient [`ResolveCx`] via [`Self::resolve_cx`] and reads each
/// property it cares about, then writes the result into the
/// presentation store. Geometry, transforms, and text shaping
/// aren't visible here — those happen at lower time against the
/// box tree's world bounds.
#[derive(Copy, Clone)]
pub struct PresentContext<'a> {
    /// The property registry every property write is checked
    /// against.
    pub registry: &'a PropertyRegistry,
    /// Type-safe handles for the toolkit's built-in properties.
    pub properties: &'a BuiltInProperties,
    /// Active theme. Resources referenced by style entries resolve
    /// through it.
    pub theme: &'a Theme,
    /// The compiled style cascade rules apply through.
    pub cascade: &'a StyleCascade,
    /// The match state for the element currently being presented.
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

/// Read-only context handed to a widget's `measure` callback.
///
/// Mirrors [`PresentContext`] minus the cascade-inheritance lookup
/// — `measure` is called from inside the layout engine's split-borrow
/// walk so the toolkit can't hand out a `&Ui`. Behaviors that need
/// inherited cascade values in measure should read them from the
/// cascade and match state directly.
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
