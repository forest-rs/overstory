// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The [`Widget`] trait + [`AppendSpec`] entry-point.
//!
//! Each control is a struct that implements both [`AppendSpec`] (the
//! builder / construction surface) and produces a [`Widget`] instance
//! that lives on the element. The widget is the toolkit-side runtime
//! object: it exposes the element's selector identity, runs the
//! present pass, runs custom measure for leaf-like controls, and
//! identifies the default template.
//!
//! Application code never instantiates widgets directly. The pattern
//! is always:
//!
//! ```ignore
//! ui.append(parent, Button::new("Click me").font_size(22.0).on_click(|ui, _| {
//!     // …
//! }));
//! ```
//!
//! `AppendSpec::append_to` creates the widget element, then writes the
//! builder's initial properties and handlers. Default templates expand during
//! element creation, so controls whose template layout depends on initial
//! builder state must sync their parts after those property writes.

use core::any::Any;

use kurbo::Size;
use understory_box_tree::NodeId;
use understory_presentation::PresentationStore;
use understory_style::TypeTag;

use crate::behavior::{MeasureConstraints, MeasureContext, PresentContext};
use crate::element::{Element, ElementId};
use crate::generic::present_surface;
use crate::template::Template;
use crate::ui::Ui;

/// Selector identity for an element that has no widget attached
/// (an anonymous container, e.g. the root or a layout-only panel).
pub const TYPE_GENERIC: TypeTag = TypeTag(0);

/// Per-element behavior + state.
///
/// A `Widget` lives in the element as `Box<dyn Widget>`. The trait
/// methods are how the toolkit walks the tree: `present` writes
/// drawing intent, `measure` answers leaf-style sizing, and
/// `default_template` lets templated controls hand back a `&'static`
/// template the first time their element is constructed.
///
/// The default `present` writes the standard surface (`Background`,
/// `BorderBrush`/`BorderWidth`, `CornerRadius`) through the
/// cascade — most controls' surface intent is exactly that.
/// Controls with bespoke drawing (e.g. shaped text) override.
pub trait Widget: Any + core::fmt::Debug + 'static {
    /// Selector identity for cascade matching. Style rules keyed on
    /// this tag match against the element this widget is attached
    /// to.
    fn type_tag(&self) -> TypeTag;

    /// Resolves drawing intent into the presentation store. The
    /// default implementation runs [`present_surface`], which reads
    /// `Background` / `Border*` / `CornerRadius` through the cascade
    /// and writes a `SurfacePrimitive` — what every "I am a box that
    /// fills my bounds" control wants.
    fn present(
        &self,
        element: &Element,
        ctx: &PresentContext<'_>,
        store: &mut PresentationStore<NodeId, ElementId>,
        node: NodeId,
    ) {
        present_surface(element, ctx, store, node);
    }

    /// Optional custom measure for leaf-like controls (text, images,
    /// custom-drawn primitives) that compute their own desired size.
    /// Containers and most controls don't override this — taffy's
    /// computed layout from the `Layout` style is enough.
    fn measure(
        &self,
        element: &Element,
        ctx: &MeasureContext<'_>,
        constraints: MeasureConstraints,
    ) -> Option<Size> {
        let _ = (element, ctx, constraints);
        None
    }

    /// Returns the default template this widget's element should
    /// expand on insertion, if any. Typical use is a templated
    /// control creating its parts (e.g. `ToggleSwitch`'s thumb,
    /// `Slider`'s track + thumb).
    fn default_template(&self) -> Option<&'static dyn Template> {
        None
    }
}

/// Construction-time builder + entry point for appending a control
/// to the tree.
///
/// Implementations:
///
/// * Hold the builder-style configuration (text, sizes, brushes,
///   handler closures) as fields on a single struct.
/// * Provide chained `.foo(value)` methods that update those fields.
/// * Implement `append_to` to: create the element via
///   `Ui::append_widget`, write the configured properties, register
///   handlers, then return the new element id.
///
/// The trait method takes `self` by value so the builder can move
/// handler closures and other unique resources out of itself before
/// passing the residue to the element.
pub trait AppendSpec {
    /// Inserts the configured control under `parent` and returns
    /// the resulting element id. Called by [`Ui::append`].
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId;
}
