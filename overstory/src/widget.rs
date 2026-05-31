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
//!
//! Roadmap:
//! - Keep `Widget` small: identity, presentation, optional measure/layout hooks,
//!   template discovery, cursor policy, hit testing, and input callbacks.
//! - Let widgets describe their accessibility role and semantic state directly;
//!   selector type tags are for styling, not for accessibility lowering.
//! - Prefer routed events, properties, templates, and runtimes over adding a
//!   hook for every new behavior.
//! - Keep visual output going through presentation primitives rather than
//!   direct renderer paint callbacks.
//! - Make the content-control and item-control patterns obvious without forcing
//!   every custom widget to subclass a large framework base type.
//! - Add diagnostics around slow hooks and re-entrant mutations before widget
//!   behavior becomes hard to reason about.

use core::any::Any;

use accesskit::{ActionRequest, Node, NodeId as AccessNodeId, Role};
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use cursor_icon::CursorIcon;
use kurbo::{Affine, Size, Vec2};
use parley::{FontFeature, FontVariation};
use peniko::Brush;
use smallvec::SmallVec;
use understory_box_tree::NodeId;
use understory_presentation::{
    FontFamily, FontStyle, FontWeight, FontWidth, Language, OverflowWrap, PresentationStore,
    TextLineHeight, TextWrapMode, WordBreak,
};
use understory_property::{ErasedValue, Property, PropertyId};
use understory_style::{ClassId, TypeTag};

use crate::accessibility::AccessibilityContext;
use crate::behavior::{
    LayoutContext, MeasureConstraints, MeasureContext, PresentContext, WidgetEventContext,
};
use crate::composition::CompositionPolicy;
use crate::element::{Element, ElementId};
use crate::event_runtime::{
    ClickEvent, DragEvent, FocusEvent, KeyEvent, PointerRoutedEvent, ScrollEvent,
};
use crate::generic::present_surface;
use crate::layouts::Layout;
use crate::motion::TransitionRules;
use crate::properties::BuiltInProperties;
use crate::template::{Template, TemplateSlot};
use crate::ui::{ControlState, Ui};

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
    fn type_tag(&self) -> Option<TypeTag> {
        None
    }

    /// Returns this widget's AccessKit role, if it should contribute one.
    ///
    /// The default produces a generic container. Controls should override this
    /// instead of asking the accessibility lowering pass to infer semantics from
    /// selector type tags.
    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &AccessibilityContext<'_>,
    ) -> Option<Role> {
        None
    }

    /// Adds widget-specific AccessKit state and supported actions to `node`.
    ///
    /// The accessibility lowering pass handles common bounds, focus, enabled,
    /// and content labeling. Widgets use this hook for control semantics such
    /// as checked, selected, range, popup, or custom actions.
    fn configure_accessibility(
        &self,
        _element: &Element,
        _ctx: &AccessibilityContext<'_>,
        _node: &mut Node,
    ) {
    }

    /// Appends widget-owned accessibility children to `node`.
    ///
    /// The core lowering pass traverses retained element children. Widgets use
    /// this hook for semantic child nodes that do not correspond to retained
    /// elements, such as text-run children inside an editable text input.
    fn append_accessibility_children(
        &self,
        _element: &Element,
        _ctx: &AccessibilityContext<'_>,
        _node: &mut Node,
        _nodes: &mut Vec<(AccessNodeId, Node)>,
    ) {
    }

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

    /// Called after a full layout pass has written bounds for the tree.
    ///
    /// Widgets that derive child structure from final geometry, such as
    /// virtualized controls, can synchronize their realized children here.
    /// Return `true` when the synchronization changed the tree or layout and
    /// the UI should run another layout pass before presenting.
    fn post_layout(&mut self, id: ElementId, ui: &mut Ui) -> bool {
        let _ = (id, ui);
        false
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
    fn cursor_icon(&self, _element: &Element) -> Option<CursorIcon> {
        None
    }

    /// Returns whether this widget participates in pointer hit testing.
    ///
    /// The default is hit-test visible. Passive overlays such as tooltips can
    /// opt out so they render above controls without becoming routed pointer
    /// targets.
    fn is_hit_test_visible(&self, _element: &Element) -> bool {
        true
    }

    /// Returns whether this widget clips its subtree to its arranged bounds.
    ///
    /// The default leaves descendants visible and pickable outside the
    /// element's own box. Viewport-like controls override this.
    fn clips_subtree(&self, _element: &Element) -> bool {
        false
    }

    /// Returns a layout-space offset applied to this widget's direct children.
    ///
    /// This is intentionally separate from layout sizing: layout still computes
    /// the normal child positions, then viewport-like controls can translate
    /// the arranged subtree for scrolling.
    fn child_layout_offset(&self, _element: &Element) -> Vec2 {
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
    fn handle_key(&mut self, _ctx: &mut WidgetEventContext<'_>, _event: KeyEvent<'_>) {}

    /// Handles the widget's built-in click behavior during routed dispatch.
    ///
    /// App handlers run first. If an app handler marks the event handled, this
    /// method is skipped for the remaining ordinary route.
    fn handle_click(&mut self, _ctx: &mut WidgetEventContext<'_>, _event: ClickEvent<'_>) {}

    /// Handles the widget's built-in drag behavior during routed dispatch.
    ///
    /// App handlers run first. If an app handler marks the event handled, this
    /// method is skipped for the remaining ordinary route.
    fn handle_drag(&mut self, _ctx: &mut WidgetEventContext<'_>, _event: DragEvent<'_>) {}

    /// Handles the widget's built-in scroll behavior during routed dispatch.
    ///
    /// App handlers run first. If an app handler marks the event handled, this
    /// method is skipped for the remaining ordinary route.
    fn handle_scroll(&mut self, _ctx: &mut WidgetEventContext<'_>, _event: ScrollEvent<'_>) {}

    /// Handles the widget's built-in focus behavior during routed dispatch.
    ///
    /// App handlers run first. If an app handler marks the event handled, this
    /// method is skipped for the remaining ordinary route.
    fn handle_focus(&mut self, _ctx: &mut WidgetEventContext<'_>, _event: FocusEvent<'_>) {}

    /// Handles the widget's built-in pointer behavior during routed dispatch.
    ///
    /// App handlers run first. If an app handler marks the event handled, this
    /// method is skipped for the remaining ordinary route.
    fn handle_pointer(
        &mut self,
        _ctx: &mut WidgetEventContext<'_>,
        _event: PointerRoutedEvent<'_>,
    ) {
    }

    /// Handles an AccessKit action request targeting this widget's element.
    ///
    /// The default leaves actions to the generic accessibility dispatcher.
    /// Widgets override this when an action needs widget-specific state, such as
    /// a slider increment that must respect its configured step size.
    fn handle_accessibility_action(
        &mut self,
        _ctx: &mut WidgetEventContext<'_>,
        _request: &ActionRequest,
    ) -> bool {
        false
    }
}

impl dyn Widget {
    pub(crate) fn downcast_ref<T: Widget>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref::<T>()
    }

    pub(crate) fn downcast_mut<T: Widget>(&mut self) -> Option<&mut T> {
        (self as &mut dyn Any).downcast_mut::<T>()
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

    fn accessibility_role(
        &self,
        element: &Element,
        ctx: &AccessibilityContext<'_>,
    ) -> Option<Role> {
        self.widget.accessibility_role(element, ctx)
    }

    fn configure_accessibility(
        &self,
        element: &Element,
        ctx: &AccessibilityContext<'_>,
        node: &mut Node,
    ) {
        self.widget.configure_accessibility(element, ctx, node);
    }

    fn append_accessibility_children(
        &self,
        element: &Element,
        ctx: &AccessibilityContext<'_>,
        node: &mut Node,
        nodes: &mut Vec<(AccessNodeId, Node)>,
    ) {
        self.widget
            .append_accessibility_children(element, ctx, node, nodes);
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

    fn post_layout(&mut self, id: ElementId, ui: &mut Ui) -> bool {
        self.widget.post_layout(id, ui)
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

    fn is_hit_test_visible(&self, element: &Element) -> bool {
        self.widget.is_hit_test_visible(element)
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

    fn handle_key(&mut self, ctx: &mut WidgetEventContext<'_>, event: KeyEvent<'_>) {
        self.widget.handle_key(ctx, event);
    }

    fn handle_click(&mut self, ctx: &mut WidgetEventContext<'_>, event: ClickEvent<'_>) {
        self.widget.handle_click(ctx, event);
    }

    fn handle_drag(&mut self, ctx: &mut WidgetEventContext<'_>, event: DragEvent<'_>) {
        self.widget.handle_drag(ctx, event);
    }

    fn handle_scroll(&mut self, ctx: &mut WidgetEventContext<'_>, event: ScrollEvent<'_>) {
        self.widget.handle_scroll(ctx, event);
    }

    fn handle_focus(&mut self, ctx: &mut WidgetEventContext<'_>, event: FocusEvent<'_>) {
        self.widget.handle_focus(ctx, event);
    }

    fn handle_pointer(&mut self, ctx: &mut WidgetEventContext<'_>, event: PointerRoutedEvent<'_>) {
        self.widget.handle_pointer(ctx, event);
    }

    fn handle_accessibility_action(
        &mut self,
        ctx: &mut WidgetEventContext<'_>,
        request: &ActionRequest,
    ) -> bool {
        self.widget.handle_accessibility_action(ctx, request)
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

/// Append spec with a declared default content slot.
///
/// This is the small common surface for content controls. It lets app code use
/// [`Ui::content`](crate::Ui::content) or
/// [`ChildUi::content`](crate::ChildUi::content) without spelling the control's
/// template slot constant at every call-site, while still preserving the
/// lower-level named-slot APIs for templates with multiple app-facing slots.
pub trait ContentSlotSpec: AppendSpec {
    /// Template slot that receives ordinary authored content for this spec.
    const CONTENT_SLOT: TemplateSlot;
}

/// Common construction-time element settings shared by append specs.
///
/// `ElementConfig` stores only settings that belong to the resulting element
/// regardless of its concrete widget: layout contribution, style classes,
/// initial local property values, and optional interactive control state.
/// Control-specific values still live on the control builder.
#[derive(Clone, Debug, Default)]
pub struct ElementConfig {
    layout: Option<Layout>,
    classes: SmallVec<[ClassId; 1]>,
    control_state: Option<ControlState>,
    local_properties: SmallVec<[InitialPropertyWrite; 4]>,
}

impl ElementConfig {
    /// Sets layout longhand contributions applied after insertion.
    ///
    /// This follows [`Ui::set_layout`]: only longhands explicitly carried by
    /// `layout` are written, so a size update does not erase independently
    /// styled padding, alignment, overflow, or placement.
    pub fn set_layout(&mut self, layout: impl Into<Layout>) {
        self.layout = Some(layout.into());
    }

    /// Returns the configured layout contribution, if any.
    #[must_use]
    pub fn layout(&self) -> Option<&Layout> {
        self.layout.as_ref()
    }

    /// Adds or replaces a construction-time local property value.
    ///
    /// These values are written after the element is inserted and after its
    /// explicit layout contribution, but before ordinary runtime interaction.
    /// They use dependency-property `Local` precedence, just like
    /// [`Ui::set_local`].
    pub fn set_local<T>(&mut self, property: Property<T>, value: T)
    where
        T: Clone + 'static,
    {
        self.set_initial_property(
            InitialPropertyTarget::Registered(property.id()),
            ErasedValue::new(value),
        );
    }

    /// Adds a style class to the resulting element.
    pub fn add_class(&mut self, class: ClassId) {
        self.classes.push(class);
    }

    /// Returns style classes applied to the resulting element.
    #[must_use]
    pub fn classes(&self) -> &[ClassId] {
        &self.classes
    }

    /// Sets the shared interactive control state applied after insertion.
    pub fn set_control_state(&mut self, state: ControlState) {
        self.control_state = Some(state);
    }

    /// Returns the configured interactive control state, if any.
    #[must_use]
    pub const fn control_state(&self) -> Option<ControlState> {
        self.control_state
    }

    /// Mutates the configured control state, inserting the interactive default
    /// when the spec did not explicitly carry control state yet.
    pub fn update_control_state(&mut self, f: impl FnOnce(ControlState) -> ControlState) {
        let state = self.control_state.unwrap_or_else(ControlState::interactive);
        self.control_state = Some(f(state));
    }

    /// Applies every configured setting to `id`.
    pub fn apply_to(self, ui: &mut Ui, id: ElementId) {
        if let Some(layout) = self.layout {
            ui.set_layout(id, layout);
        }
        for write in self.local_properties {
            let property = write.target.resolve(ui.properties());
            ui.set_local_erased(id, property, write.value);
        }
        for class in self.classes {
            ui.add_class(id, class);
        }
        if let Some(state) = self.control_state {
            ui.set_control_state(id, state);
        }
    }

    fn set_built_in_local<T>(&mut self, property: BuiltInPropertyKey, value: T)
    where
        T: Clone + 'static,
    {
        self.set_initial_property(
            InitialPropertyTarget::BuiltIn(property),
            ErasedValue::new(value),
        );
    }

    fn set_initial_property(&mut self, target: InitialPropertyTarget, value: ErasedValue) {
        if let Some(write) = self
            .local_properties
            .iter_mut()
            .find(|write| write.target == target)
        {
            write.value = value;
        } else {
            self.local_properties
                .push(InitialPropertyWrite { target, value });
        }
    }
}

#[derive(Clone, Debug)]
struct InitialPropertyWrite {
    target: InitialPropertyTarget,
    value: ErasedValue,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum InitialPropertyTarget {
    Registered(PropertyId),
    BuiltIn(BuiltInPropertyKey),
}

impl InitialPropertyTarget {
    fn resolve(self, properties: &BuiltInProperties) -> PropertyId {
        match self {
            Self::Registered(property) => property,
            Self::BuiltIn(property) => property.resolve(properties),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum BuiltInPropertyKey {
    Background,
    BorderBrush,
    BorderWidth,
    Content,
    CornerRadius,
    FontFamily,
    FontFeatures,
    FontSize,
    FontStyle,
    FontVariations,
    FontWeight,
    FontWidth,
    Foreground,
    IsChecked,
    IsExpanded,
    IsReadOnly,
    LargeChange,
    LetterSpacing,
    LineHeight,
    Maximum,
    Minimum,
    Opacity,
    OverflowWrap,
    SelectedIndex,
    SmallChange,
    TextLanguage,
    TextWrapMode,
    Transitions,
    Value,
    VisualTransform,
    WordBreak,
    WordSpacing,
}

impl BuiltInPropertyKey {
    fn resolve(self, properties: &BuiltInProperties) -> PropertyId {
        match self {
            Self::Background => properties.background.id(),
            Self::BorderBrush => properties.border_brush.id(),
            Self::BorderWidth => properties.border_width.id(),
            Self::Content => properties.content.id(),
            Self::CornerRadius => properties.corner_radius.id(),
            Self::FontFamily => properties.font_family.id(),
            Self::FontFeatures => properties.font_features.id(),
            Self::FontSize => properties.font_size.id(),
            Self::FontStyle => properties.font_style.id(),
            Self::FontVariations => properties.font_variations.id(),
            Self::FontWeight => properties.font_weight.id(),
            Self::FontWidth => properties.font_width.id(),
            Self::Foreground => properties.foreground.id(),
            Self::IsChecked => properties.is_checked.id(),
            Self::IsExpanded => properties.is_expanded.id(),
            Self::IsReadOnly => properties.is_read_only.id(),
            Self::LargeChange => properties.large_change.id(),
            Self::LetterSpacing => properties.letter_spacing.id(),
            Self::LineHeight => properties.line_height.id(),
            Self::Maximum => properties.maximum.id(),
            Self::Minimum => properties.minimum.id(),
            Self::Opacity => properties.opacity.id(),
            Self::OverflowWrap => properties.overflow_wrap.id(),
            Self::SelectedIndex => properties.selected_index.id(),
            Self::SmallChange => properties.small_change.id(),
            Self::TextLanguage => properties.text_language.id(),
            Self::TextWrapMode => properties.text_wrap_mode.id(),
            Self::Transitions => properties.transitions.id(),
            Self::Value => properties.value.id(),
            Self::VisualTransform => properties.visual_transform.id(),
            Self::WordBreak => properties.word_break.id(),
            Self::WordSpacing => properties.word_spacing.id(),
        }
    }
}

/// Append specs that expose the shared element authoring surface.
///
/// Implement this for builders that want the common extension methods from
/// [`ElementSpecExt`]. The trait is deliberately about construction-time
/// element settings, not widget behavior.
pub trait ElementSpec: AppendSpec {
    /// Returns mutable access to the spec's shared element config.
    fn element_config_mut(&mut self) -> &mut ElementConfig;
}

/// Core builder methods available to element specs.
///
/// Import this trait, or use `overstory::prelude::*`, to get a uniform
/// `.class(...)`, `.layout(...)`, `.enabled(...)`, `.disabled()`,
/// `.focusable(...)`, and low-level `.local(...)` surface on specs that
/// implement [`ElementSpec`].
pub trait ElementSpecExt: ElementSpec + Sized {
    /// Writes an initial local dependency-property value when the element is
    /// appended.
    ///
    /// Use this for custom properties or for built-ins that do not yet have a
    /// dedicated helper. Dedicated helpers such as [`Self::background`] and
    /// [`Self::font_features`] keep common call-sites independent from a
    /// particular [`Ui`]'s registered property handles.
    #[must_use]
    fn local<T>(mut self, property: Property<T>, value: T) -> Self
    where
        T: Clone + 'static,
    {
        self.element_config_mut().set_local(property, value);
        self
    }

    /// Adds a style class to the resulting element.
    #[must_use]
    fn class(mut self, class: ClassId) -> Self {
        self.element_config_mut().add_class(class);
        self
    }

    /// Adds several style classes to the resulting element.
    #[must_use]
    fn classes(mut self, classes: impl IntoIterator<Item = ClassId>) -> Self {
        for class in classes {
            self.element_config_mut().add_class(class);
        }
        self
    }

    /// Sets layout longhand contributions on the resulting element.
    ///
    /// Only the longhands explicitly carried by `layout` are written; this is
    /// the same partial-update behavior as [`Ui::set_layout`].
    #[must_use]
    fn layout(mut self, layout: impl Into<Layout>) -> Self {
        self.element_config_mut().set_layout(layout);
        self
    }

    /// Sets the resulting element's shared interactive control state.
    #[must_use]
    fn control_state(mut self, state: ControlState) -> Self {
        self.element_config_mut().set_control_state(state);
        self
    }

    /// Sets whether the resulting element starts enabled.
    #[must_use]
    fn enabled(mut self, enabled: bool) -> Self {
        self.element_config_mut()
            .update_control_state(|state| state.enabled(enabled));
        self
    }

    /// Convenience for [`Self::enabled`] with `false`.
    #[must_use]
    fn disabled(self) -> Self {
        self.enabled(false)
    }

    /// Sets whether the resulting element participates in focus traversal.
    #[must_use]
    fn focusable(mut self, focusable: bool) -> Self {
        self.element_config_mut()
            .update_control_state(|state| state.focusable(focusable));
        self
    }
}

impl<T> ElementSpecExt for T where T: ElementSpec {}

/// Surface and composition builder methods available to element specs.
///
/// This trait covers properties that affect the visual surface of an element
/// rather than its identity, layout tree position, or semantic state.
pub trait SurfaceSpecExt: ElementSpec + Sized {
    /// Sets the initial background brush.
    ///
    /// Pass `None` to explicitly author no background as a local value.
    #[must_use]
    fn background(mut self, brush: impl Into<Option<Brush>>) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::Background, brush.into());
        self
    }

    /// Sets the initial foreground brush.
    #[must_use]
    fn foreground(mut self, brush: impl Into<Brush>) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::Foreground, brush.into());
        self
    }

    /// Sets the initial border brush.
    ///
    /// Pass `None` to explicitly author no border brush as a local value.
    #[must_use]
    fn border_brush(mut self, brush: impl Into<Option<Brush>>) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::BorderBrush, brush.into());
        self
    }

    /// Sets the initial border width in logical pixels.
    #[must_use]
    fn border_width(mut self, width: f64) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::BorderWidth, width);
        self
    }

    /// Sets the initial corner radius in logical pixels.
    #[must_use]
    fn corner_radius(mut self, radius: f64) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::CornerRadius, radius);
        self
    }

    /// Sets the initial layer opacity.
    #[must_use]
    fn opacity(mut self, opacity: f32) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::Opacity, opacity);
        self
    }

    /// Sets the initial visual transform applied after layout.
    #[must_use]
    fn visual_transform(mut self, transform: Affine) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::VisualTransform, transform);
        self
    }

    /// Sets initial style-authored transition rules.
    #[must_use]
    fn transitions(mut self, transitions: TransitionRules) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::Transitions, transitions);
        self
    }
}

impl<T> SurfaceSpecExt for T where T: ElementSpec {}

/// Text and font builder methods available to element specs.
///
/// These helpers write Overstory's built-in text style properties without
/// requiring the caller to fetch a [`BuiltInProperties`] handle from a `Ui`.
pub trait TextSpecExt: ElementSpec + Sized {
    /// Sets the initial textual content property.
    #[must_use]
    fn content(mut self, content: impl Into<String>) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::Content, content.into());
        self
    }

    /// Sets the initial font size in logical pixels.
    #[must_use]
    fn font_size(mut self, font_size: f64) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::FontSize, font_size);
        self
    }

    /// Sets the initial font family list.
    #[must_use]
    fn font_family(mut self, family: FontFamily<'static>) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::FontFamily, family);
        self
    }

    /// Sets the initial font width.
    #[must_use]
    fn font_width(mut self, width: FontWidth) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::FontWidth, width);
        self
    }

    /// Sets the initial font weight.
    #[must_use]
    fn font_weight(mut self, weight: FontWeight) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::FontWeight, weight);
        self
    }

    /// Sets the initial font style.
    #[must_use]
    fn font_style(mut self, style: FontStyle) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::FontStyle, style);
        self
    }

    /// Sets initial font variation settings.
    #[must_use]
    fn font_variations(mut self, variations: impl Into<Box<[FontVariation]>>) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::FontVariations, variations.into());
        self
    }

    /// Sets initial OpenType font feature settings.
    #[must_use]
    fn font_features(mut self, features: impl Into<Box<[FontFeature]>>) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::FontFeatures, features.into());
        self
    }

    /// Sets the initial text language hint.
    #[must_use]
    fn text_language(mut self, language: impl Into<Option<Language>>) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::TextLanguage, language.into());
        self
    }

    /// Sets the initial text line-height policy.
    #[must_use]
    fn line_height(mut self, line_height: TextLineHeight) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::LineHeight, line_height);
        self
    }

    /// Sets the initial extra letter spacing in logical pixels.
    #[must_use]
    fn letter_spacing(mut self, spacing: f32) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::LetterSpacing, spacing);
        self
    }

    /// Sets the initial extra word spacing in logical pixels.
    #[must_use]
    fn word_spacing(mut self, spacing: f32) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::WordSpacing, spacing);
        self
    }

    /// Sets the initial word-break policy.
    #[must_use]
    fn word_break(mut self, word_break: WordBreak) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::WordBreak, word_break);
        self
    }

    /// Sets the initial emergency wrapping policy.
    #[must_use]
    fn overflow_wrap(mut self, overflow_wrap: OverflowWrap) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::OverflowWrap, overflow_wrap);
        self
    }

    /// Sets the initial text line-wrapping policy.
    #[must_use]
    fn text_wrap_mode(mut self, wrap_mode: TextWrapMode) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::TextWrapMode, wrap_mode);
        self
    }
}

impl<T> TextSpecExt for T where T: ElementSpec {}

/// Semantic state builder methods available to element specs.
///
/// Controls usually provide domain-specific inherent methods for these values.
/// This shared trait is useful for composed specs and generic authoring code.
pub trait ControlStateSpecExt: ElementSpec + Sized {
    /// Sets the initial checked state property.
    #[must_use]
    fn checked(mut self, checked: bool) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::IsChecked, checked);
        self
    }

    /// Sets the initial expanded state property.
    #[must_use]
    fn expanded(mut self, expanded: bool) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::IsExpanded, expanded);
        self
    }

    /// Sets whether an editable text-like control starts read-only.
    #[must_use]
    fn read_only(mut self, read_only: bool) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::IsReadOnly, read_only);
        self
    }

    /// Sets the initial selected index property.
    #[must_use]
    fn selected_index(mut self, index: impl Into<Option<usize>>) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::SelectedIndex, index.into());
        self
    }
}

impl<T> ControlStateSpecExt for T where T: ElementSpec {}

/// Numeric range builder methods available to element specs.
///
/// Range controls such as sliders and progress bars usually expose
/// control-specific inherent methods as well. This trait supplies the shared
/// dependency-property language for generic authoring code.
pub trait RangeSpecExt: ElementSpec + Sized {
    /// Sets the initial numeric value property.
    #[must_use]
    fn value(mut self, value: f64) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::Value, value);
        self
    }

    /// Sets the initial minimum numeric value.
    #[must_use]
    fn minimum(mut self, minimum: f64) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::Minimum, minimum);
        self
    }

    /// Sets the initial maximum numeric value.
    #[must_use]
    fn maximum(mut self, maximum: f64) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::Maximum, maximum);
        self
    }

    /// Sets the initial numeric value range.
    #[must_use]
    fn range(mut self, minimum: f64, maximum: f64) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::Minimum, minimum);
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::Maximum, maximum);
        self
    }

    /// Sets the initial small change value for range controls.
    #[must_use]
    fn small_change(mut self, small_change: f64) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::SmallChange, small_change);
        self
    }

    /// Sets the initial large change value for range controls.
    #[must_use]
    fn large_change(mut self, large_change: f64) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::LargeChange, large_change);
        self
    }
}

impl<T> RangeSpecExt for T where T: ElementSpec {}
