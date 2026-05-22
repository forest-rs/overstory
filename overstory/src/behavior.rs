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

use kurbo::{Rect, Size};
use understory_property::{DependencyObject, Property, PropertyRegistry};
use understory_style::{MatchState, ResolveCx, StyleCascade, StyleValueRef, Theme};

use crate::element::{Element, ElementId};
use crate::lookup::UiResolveLookup;
use crate::properties::BuiltInProperties;
use crate::text::TextEngine;
use crate::ui::Ui;

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

/// Read-only context handed to a widget after layout has written element bounds.
///
/// Widgets use this for derived geometry state that depends on both their own
/// bounds and their children's final arranged bounds, such as scroll range
/// metrics. It is intentionally read-only: layout hooks can update interior
/// widget state, but they do not mutate the tree or the layout engine.
#[derive(Copy, Clone)]
pub struct LayoutContext<'a> {
    pub(crate) ui: &'a Ui,
}

impl LayoutContext<'_> {
    /// Returns the latest computed bounds for `element`, if it is still live.
    #[must_use]
    pub fn effective_bounds(&self, element: ElementId) -> Option<Rect> {
        self.ui
            .get(element)
            .map(|_| self.ui.effective_bounds(element))
    }
}

impl fmt::Debug for LayoutContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LayoutContext").finish_non_exhaustive()
    }
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
/// Bundles everything needed to resolve effective property values through the
/// full precedence chain (Animation → Local → Style → Theme → Inherited →
/// Default). Widgets should usually read properties with [`Self::get_value`],
/// then write resolved intent into the presentation store. Geometry, transforms,
/// and text shaping aren't visible here — those happen at lower time against
/// the box tree's world bounds.
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
    /// Parent-walking lookup the [`ResolveCx`] uses for style-aware inheritance.
    pub(crate) lookup: UiResolveLookup<'a>,
}

impl<'a> PresentContext<'a> {
    /// Builds a raw [`ResolveCx`] tied to this context.
    #[must_use]
    pub fn resolve_cx(&self) -> ResolveCx<'a, ElementId, UiResolveLookup<'a>> {
        ResolveCx::new(self.registry, self.theme, self.lookup)
    }

    /// Resolves `property` for `element` through [`understory_style::ResolveCx`],
    /// including inherited styled values from ancestors.
    #[must_use]
    pub fn get_value<T>(&self, element: &Element, property: Property<T>) -> T
    where
        T: Clone + 'static,
    {
        self.resolve_cx()
            .get_value(element, property, Some((self.cascade, self.match_state)))
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

impl<'a> MeasureContext<'a> {
    /// Resolves a measure-time property value using animation/local, then the
    /// element's own styled value, then the registry default.
    ///
    /// Layout measurement runs inside the layout engine's split-borrow
    /// callback and cannot currently walk ancestor elements. This helper is
    /// intentionally limited to properties whose measurement behavior depends
    /// on the measured element's own value, such as `Content` and `FontSize`.
    #[must_use]
    pub fn get_value<T>(&self, element: &Element, property: Property<T>) -> T
    where
        T: Clone + 'static,
    {
        if let Some(value) = element.property_store().get_animation(property) {
            return value.clone();
        }
        if let Some(value) = element.property_store().get_local(property) {
            return value.clone();
        }
        if let Some(entry) = self.cascade.get_entry_ref(self.match_state, property) {
            match entry {
                StyleValueRef::Value(value) => return value.clone(),
                StyleValueRef::Resource(key) => {
                    if let Some(value) = self.theme.get::<T>(key) {
                        return value.clone();
                    }
                }
            }
        }
        self.registry
            .get_metadata(property)
            .expect("property should be registered")
            .default_value()
            .clone()
    }
}

impl fmt::Debug for MeasureContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MeasureContext").finish_non_exhaustive()
    }
}
