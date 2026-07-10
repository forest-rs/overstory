// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Context types handed to widget trait methods.
//!
//! The widget trait itself lives in [`crate::widget`]. These are the
//! pieces that get passed to its `present` and `measure` hooks —
//! the resolve / cascade plumbing for property reads + the text
//! engine for measure-time shaping.
//!
//! Roadmap:
//! - Keep context types capability-scoped: measurement reads measurement inputs,
//!   presentation reads resolved properties, and layout-update hooks read final
//!   geometry without mutating the tree.
//! - Make style-aware property resolution live here and in `understory_style`,
//!   not as duplicated cascade logic inside controls.
//! - Add new capabilities only when several widgets need them; prefer explicit
//!   narrow contexts over handing every hook broad access to `Ui`.
//! - Prepare a clean path for reusable prepared resources, such as shaped text
//!   and keyed picture primitives, without adding renderer-specific paint APIs.

use core::cell::RefCell;
use core::fmt;
use core::ops::{Deref, DerefMut};

use kurbo::{Rect, Size};
use understory_property::{Property, PropertyRegistry};
use understory_style::{ExpressionLayer, MatchState, ResolveCx, StyleCascade, ThemeResolveOptions};

use crate::element::{Element, ElementId};
use crate::lookup::{ElementResolveLookup, UiResolveLookup};
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

/// Context handed to a widget after layout has arranged element bounds.
///
/// Widgets use this for derived geometry state that depends on both their own
/// bounds and their children's final arranged bounds, such as scroll range
/// metrics and geometry-derived presentation targets. Tree mutation remains a
/// separate realization concern.
pub struct ArrangeContext<'a> {
    id: ElementId,
    ui: &'a mut Ui,
}

impl<'a> ArrangeContext<'a> {
    pub(crate) fn new(id: ElementId, ui: &'a mut Ui) -> Self {
        Self { id, ui }
    }

    /// Returns the element whose widget is being arranged.
    #[must_use]
    pub fn id(&self) -> ElementId {
        self.id
    }

    /// Returns shared access to the owning UI.
    #[must_use]
    pub fn ui(&self) -> &Ui {
        self.ui
    }

    /// Returns the latest computed bounds for `element`, if it is still live.
    #[must_use]
    pub fn effective_bounds(&self, element: ElementId) -> Option<Rect> {
        self.ui
            .get(element)
            .map(|_| self.ui.effective_bounds(element))
    }

    pub(crate) fn ui_mut(&mut self) -> &mut Ui {
        self.ui
    }
}

impl fmt::Debug for ArrangeContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArrangeContext").finish_non_exhaustive()
    }
}

/// Result returned from an arrange hook.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArrangeResult {
    layout_changed: bool,
}

impl ArrangeResult {
    /// No layout-affecting arranged state changed.
    pub const UNCHANGED: Self = Self {
        layout_changed: false,
    };

    /// Layout-affecting arranged state changed.
    pub const LAYOUT_CHANGED: Self = Self {
        layout_changed: true,
    };

    /// Creates a result from a layout-change flag.
    #[must_use]
    pub const fn from_layout_changed(layout_changed: bool) -> Self {
        Self { layout_changed }
    }

    /// Returns whether arrangement changed layout-affecting state and the UI
    /// should run another layout pass.
    #[must_use]
    pub const fn layout_changed(self) -> bool {
        self.layout_changed
    }
}

/// Mutable context handed to a widget's built-in routed event handlers.
///
/// Routed event payloads such as [`crate::ClickEvent`] and
/// [`crate::DragEvent`] describe the input event and route. `WidgetEventContext`
/// carries the widget-side execution context: the element whose widget is
/// handling the event, plus access to the owning [`Ui`].
pub struct WidgetEventContext<'a> {
    id: ElementId,
    ui: &'a mut Ui,
}

impl<'a> WidgetEventContext<'a> {
    pub(crate) fn new(id: ElementId, ui: &'a mut Ui) -> Self {
        Self { id, ui }
    }

    /// Returns the element whose widget is handling the event.
    #[must_use]
    pub fn id(&self) -> ElementId {
        self.id
    }

    /// Returns the handling element, if it is still live.
    #[must_use]
    pub fn element(&self) -> Option<&Element> {
        self.ui.get(self.id)
    }

    /// Returns the owning UI.
    #[must_use]
    pub fn ui(&self) -> &Ui {
        self.ui
    }

    /// Returns mutable access to the owning UI.
    pub fn ui_mut(&mut self) -> &mut Ui {
        self.ui
    }

    /// Resolves `property` for `element` through the active property stores,
    /// style cascade, theme resources, and inherited ancestor values.
    #[must_use]
    pub fn get_value<T>(&self, element: &Element, property: Property<T>) -> T
    where
        T: Clone + 'static,
    {
        self.ui.resolve_value_for(element, property)
    }
}

impl Deref for WidgetEventContext<'_> {
    type Target = Ui;

    fn deref(&self) -> &Self::Target {
        self.ui
    }
}

impl DerefMut for WidgetEventContext<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ui
    }
}

impl fmt::Debug for WidgetEventContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WidgetEventContext")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// Mutable context handed to a widget's element-realization hook.
///
/// Element realization runs after arrangement when a widget's final viewport
/// geometry can determine which retained element children should exist. This is
/// the hook used by virtualized controls: they inspect the arranged viewport,
/// reconcile only the child elements that should be retained for that viewport,
/// and then return whether that retained subtree changed.
///
/// Visual or presentation-node realization is deliberately a separate concern.
/// This context is for retained elements, not renderer display lists.
pub struct ElementRealizeContext<'a> {
    id: ElementId,
    ui: &'a mut Ui,
}

impl<'a> ElementRealizeContext<'a> {
    pub(crate) fn new(id: ElementId, ui: &'a mut Ui) -> Self {
        Self { id, ui }
    }

    /// Returns the element whose widget is realizing children.
    #[must_use]
    pub fn id(&self) -> ElementId {
        self.id
    }

    /// Returns shared access to the owning UI.
    #[must_use]
    pub fn ui(&self) -> &Ui {
        self.ui
    }

    /// Returns mutable access to the owning UI.
    ///
    /// External virtualized controls use this to append, remove, and lay out
    /// realized child elements after their own viewport math decides which rows
    /// should exist. Keep mutations scoped to the realizing element's subtree;
    /// broad application state changes belong in routed event handlers,
    /// bindings, or timers.
    pub fn ui_mut(&mut self) -> &mut Ui {
        self.ui
    }
}

impl fmt::Debug for ElementRealizeContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ElementRealizeContext")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// Result returned from an element-realization hook.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ElementRealizeResult {
    changed: bool,
}

impl ElementRealizeResult {
    /// No retained children or layout-affecting state changed.
    pub const UNCHANGED: Self = Self { changed: false };

    /// Retained children or layout-affecting state changed.
    pub const CHANGED: Self = Self { changed: true };

    /// Creates a result from a boolean change flag.
    #[must_use]
    pub const fn from_changed(changed: bool) -> Self {
        Self { changed }
    }

    /// Returns whether realization changed retained children or layout state.
    #[must_use]
    pub const fn changed(self) -> bool {
        self.changed
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
    pub theme: ThemeResolveOptions<'a>,
    /// Expression defaults and functions used by style resolution.
    pub(crate) expressions: &'a ExpressionLayer,
    /// The compiled style cascade rules apply through.
    pub cascade: &'a StyleCascade,
    /// The match state for the element currently being presented.
    pub match_state: MatchState,
    /// Parent-walking lookup the [`ResolveCx`] uses for style-aware inheritance.
    pub(crate) lookup: UiResolveLookup<'a>,
}

impl<'a> PresentContext<'a> {
    /// Returns a live element by id.
    #[must_use]
    #[inline]
    pub fn element(&self, id: ElementId) -> Option<&'a Element> {
        self.lookup.element(id)
    }

    /// Resolves `property` for `element` through [`understory_style::ResolveCx`],
    /// including inherited styled values from ancestors.
    #[must_use]
    pub fn get_value<T>(&self, element: &Element, property: Property<T>) -> T
    where
        T: Clone + 'static,
    {
        ResolveCx::new(self.registry, self.theme, self.expressions, self.lookup).get_value(
            element,
            property,
            Some((self.cascade, self.lookup.match_state(element.id()))),
        )
    }
}

impl fmt::Debug for PresentContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PresentContext").finish_non_exhaustive()
    }
}

/// Read-only context handed to a widget's `measure` callback.
///
/// Mirrors [`PresentContext`] while using a split-borrow lookup for inherited
/// values.
#[derive(Copy, Clone)]
pub struct MeasureContext<'a> {
    /// The property registry every property write is checked against.
    pub registry: &'a PropertyRegistry,
    /// Type-safe handles for the toolkit's built-in properties.
    pub properties: &'a BuiltInProperties,
    /// Active theme.
    pub theme: ThemeResolveOptions<'a>,
    /// Expression defaults and functions used by style resolution.
    pub(crate) expressions: &'a ExpressionLayer,
    /// The compiled style cascade.
    pub cascade: &'a StyleCascade,
    /// The match state for the element currently being measured.
    pub match_state: MatchState,
    /// Parent-walking lookup the [`ResolveCx`] uses for style-aware inheritance.
    pub(crate) lookup: ElementResolveLookup<'a>,
    /// Shared text shaping engine.
    pub text: &'a RefCell<TextEngine>,
}

impl<'a> MeasureContext<'a> {
    /// Returns a live element by id.
    #[must_use]
    #[inline]
    pub fn element(&self, id: ElementId) -> Option<&'a Element> {
        self.lookup.element(id)
    }

    /// Resolves a measure-time property value through [`understory_style::ResolveCx`],
    /// including inherited styled values from ancestors.
    #[must_use]
    pub fn get_value<T>(&self, element: &Element, property: Property<T>) -> T
    where
        T: Clone + 'static,
    {
        ResolveCx::new(self.registry, self.theme, self.expressions, self.lookup).get_value(
            element,
            property,
            Some((self.cascade, self.lookup.match_state(element.id()))),
        )
    }
}

impl fmt::Debug for MeasureContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MeasureContext").finish_non_exhaustive()
    }
}
