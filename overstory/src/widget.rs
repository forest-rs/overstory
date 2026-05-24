// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The [`Widget`] trait + [`AppendSpec`] entry-point.
//!
//! Each control is a struct that implements both [`AppendSpec`] (the
//! builder / construction surface) and produces a [`Widget`] instance
//! that lives on the element. The widget is the toolkit-side runtime
//! object: it may expose the element's selector identity, runs the
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
//! `AppendSpec::append_to` creates the widget element, writes the builder's
//! initial properties and handlers, then relies on the widget's default
//! template for any visual parts. Most controls expand during insertion;
//! controls whose template structure depends on initial builder state can use
//! Overstory's crate-internal deferred expansion path.

use core::any::Any;

use cursor_icon::CursorIcon;
use kurbo::{Size, Vec2};
use understory_box_tree::NodeId;
use understory_presentation::PresentationStore;
use understory_style::TypeTag;

use crate::behavior::{LayoutContext, MeasureConstraints, MeasureContext, PresentContext};
use crate::composition::CompositionPolicy;
use crate::element::{Element, ElementId};
use crate::event_runtime::{
    ClickEvent, DragEvent, FocusEvent, KeyEvent, PointerRoutedEvent, ScrollEvent,
};
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
    /// Selector identity for cascade matching, if this widget is a
    /// globally addressable control kind.
    ///
    /// Pure template parts should usually return `None` and rely on
    /// the element's owner-local `PartTag` instead. That keeps parts
    /// from becoming accidental app-wide control types.
    fn type_tag(&self) -> Option<TypeTag>;

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
    /// Containers and most controls don't override this — the computed layout
    /// from the `Layout` style is enough.
    fn measure(
        &self,
        element: &Element,
        ctx: &MeasureContext<'_>,
        constraints: MeasureConstraints,
    ) -> Option<Size> {
        let _ = (element, ctx, constraints);
        None
    }

    /// Called after layout has written this element's bounds and recursively
    /// written all descendant bounds.
    ///
    /// The default does nothing. Viewport-like controls use this to derive
    /// geometry state, such as scroll extents, from their arranged children.
    fn layout_updated(&self, element: &Element, ctx: &LayoutContext<'_>) {
        let _ = (element, ctx);
    }

    /// Returns the default template this widget's element should
    /// expand on insertion, if any. Typical use is a templated
    /// control creating its parts (e.g. `ToggleSwitch`'s thumb,
    /// `Slider`'s track + thumb).
    fn default_template(&self) -> Option<&'static dyn Template> {
        None
    }

    /// Called after this widget's default template has expanded.
    ///
    /// Most widgets have all state in properties or template bindings and do
    /// nothing here. Stateful controls use this to synchronize freshly-created
    /// template parts with widget-owned state.
    fn template_expanded(&mut self, _id: ElementId, _ui: &mut Ui) {}

    /// Returns the cursor icon this widget prefers while it is the
    /// active hover or press target.
    ///
    /// The default leaves cursor selection to ancestors or the host's
    /// platform default.
    fn cursor_icon(&self, element: &Element) -> Option<CursorIcon> {
        let _ = element;
        None
    }

    /// Returns whether this widget participates in pointer hit testing.
    ///
    /// The default is hit-test visible. Passive overlays such as tooltips can
    /// opt out so they render above controls without becoming routed pointer
    /// targets.
    fn is_hit_test_visible(&self, element: &Element) -> bool {
        let _ = element;
        true
    }

    /// Returns whether this widget clips its subtree to its arranged bounds.
    ///
    /// The default leaves descendants visible and pickable outside the
    /// element's own box. Viewport-like controls override this.
    fn clips_subtree(&self, element: &Element) -> bool {
        let _ = element;
        false
    }

    /// Returns a layout-space offset applied to this widget's direct children.
    ///
    /// This is intentionally separate from layout sizing: layout still computes
    /// the normal child positions, then viewport-like controls can translate
    /// the arranged subtree for scrolling.
    fn child_layout_offset(&self, element: &Element) -> Vec2 {
        let _ = element;
        Vec2::ZERO
    }

    /// Returns how this widget wants its element projected into composition.
    ///
    /// The default records the element into an Overstory-owned scene surface.
    /// Container-only or host-backed widgets can override this when their
    /// layer should not own an ordinary scene recording.
    fn composition_policy(&self) -> CompositionPolicy {
        CompositionPolicy::InlineScene
    }

    /// Handles the widget's built-in key behavior during routed dispatch.
    ///
    /// App handlers run first. If an app handler marks the event handled, this
    /// method is skipped for the remaining ordinary route.
    fn handle_key(&mut self, _id: ElementId, _ui: &mut Ui, _event: KeyEvent<'_>) {}

    /// Handles the widget's built-in click behavior during routed dispatch.
    ///
    /// App handlers run first. If an app handler marks the event handled, this
    /// method is skipped for the remaining ordinary route.
    fn handle_click(&mut self, _id: ElementId, _ui: &mut Ui, _event: ClickEvent<'_>) {}

    /// Handles the widget's built-in drag behavior during routed dispatch.
    ///
    /// App handlers run first. If an app handler marks the event handled, this
    /// method is skipped for the remaining ordinary route.
    fn handle_drag(&mut self, _id: ElementId, _ui: &mut Ui, _event: DragEvent<'_>) {}

    /// Handles the widget's built-in scroll behavior during routed dispatch.
    ///
    /// App handlers run first. If an app handler marks the event handled, this
    /// method is skipped for the remaining ordinary route.
    fn handle_scroll(&mut self, _id: ElementId, _ui: &mut Ui, _event: ScrollEvent<'_>) {}

    /// Handles the widget's built-in focus behavior during routed dispatch.
    ///
    /// App handlers run first. If an app handler marks the event handled, this
    /// method is skipped for the remaining ordinary route.
    fn handle_focus(&mut self, _id: ElementId, _ui: &mut Ui, _event: FocusEvent<'_>) {}

    /// Handles the widget's built-in pointer behavior during routed dispatch.
    ///
    /// App handlers run first. If an app handler marks the event handled, this
    /// method is skipped for the remaining ordinary route.
    fn handle_pointer(&mut self, _id: ElementId, _ui: &mut Ui, _event: PointerRoutedEvent<'_>) {}
}

impl dyn Widget {
    pub(crate) fn downcast_ref<T: Widget>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref::<T>()
    }
}

/// Widget wrapper that replaces a control's default template without changing
/// the wrapped widget's selector identity, presentation, or measure behavior.
pub(crate) struct TemplateOverrideWidget<W> {
    widget: W,
    template: &'static dyn Template,
}

impl<W> TemplateOverrideWidget<W> {
    pub(crate) const fn new(widget: W, template: &'static dyn Template) -> Self {
        Self { widget, template }
    }
}

impl<W: core::fmt::Debug> core::fmt::Debug for TemplateOverrideWidget<W> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TemplateOverrideWidget")
            .field("widget", &self.widget)
            .finish_non_exhaustive()
    }
}

impl<W: Widget> Widget for TemplateOverrideWidget<W> {
    fn type_tag(&self) -> Option<TypeTag> {
        self.widget.type_tag()
    }

    fn present(
        &self,
        element: &Element,
        ctx: &PresentContext<'_>,
        store: &mut PresentationStore<NodeId, ElementId>,
        node: NodeId,
    ) {
        self.widget.present(element, ctx, store, node);
    }

    fn measure(
        &self,
        element: &Element,
        ctx: &MeasureContext<'_>,
        constraints: MeasureConstraints,
    ) -> Option<Size> {
        self.widget.measure(element, ctx, constraints)
    }

    fn layout_updated(&self, element: &Element, ctx: &LayoutContext<'_>) {
        self.widget.layout_updated(element, ctx);
    }

    fn default_template(&self) -> Option<&'static dyn Template> {
        Some(self.template)
    }

    fn template_expanded(&mut self, id: ElementId, ui: &mut Ui) {
        self.widget.template_expanded(id, ui);
    }

    fn cursor_icon(&self, element: &Element) -> Option<CursorIcon> {
        self.widget.cursor_icon(element)
    }

    fn clips_subtree(&self, element: &Element) -> bool {
        self.widget.clips_subtree(element)
    }

    fn child_layout_offset(&self, element: &Element) -> Vec2 {
        self.widget.child_layout_offset(element)
    }

    fn composition_policy(&self) -> CompositionPolicy {
        self.widget.composition_policy()
    }

    fn handle_key(&mut self, id: ElementId, ui: &mut Ui, event: KeyEvent<'_>) {
        self.widget.handle_key(id, ui, event);
    }

    fn handle_click(&mut self, id: ElementId, ui: &mut Ui, event: ClickEvent<'_>) {
        self.widget.handle_click(id, ui, event);
    }

    fn handle_drag(&mut self, id: ElementId, ui: &mut Ui, event: DragEvent<'_>) {
        self.widget.handle_drag(id, ui, event);
    }

    fn handle_scroll(&mut self, id: ElementId, ui: &mut Ui, event: ScrollEvent<'_>) {
        self.widget.handle_scroll(id, ui, event);
    }

    fn handle_focus(&mut self, id: ElementId, ui: &mut Ui, event: FocusEvent<'_>) {
        self.widget.handle_focus(id, ui, event);
    }

    fn handle_pointer(&mut self, id: ElementId, ui: &mut Ui, event: PointerRoutedEvent<'_>) {
        self.widget.handle_pointer(id, ui, event);
    }
}

/// Generic surface-only widget for template parts.
///
/// It has no global selector type. Style rules should target it with
/// an owner-local `PartTag`, usually anchored under the owning control
/// type.
#[derive(Debug, Default)]
pub(crate) struct SurfacePartWidget;

impl Widget for SurfacePartWidget {
    fn type_tag(&self) -> Option<TypeTag> {
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
