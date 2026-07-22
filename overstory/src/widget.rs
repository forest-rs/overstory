// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The [`Widget`] trait + [`AppendSpec`] entry-point.
//!
//! Each control is a struct that implements both [`AppendSpec`] (the
//! builder / construction surface) and produces a [`Widget`] instance
//! that lives on the element. The widget is the toolkit-side runtime
//! object: it may expose the element's selector identity, runs the
//! present pass, runs custom measure for leaf-like controls, and
//! declares the semantic contract its template must satisfy.
//!
//! Application code never instantiates widgets directly. The pattern
//! is always:
//!
//! ```ignore
//! ui.append(parent, Button::new("Click me").font_size(22.0).on_activate(|ui, _| {
//!     // …
//! }));
//! ```
//!
//! `AppendSpec::append_to` creates the widget element, writes the builder's
//! initial properties and handlers, then the UI resolves the
//! [`ControlTemplate`](crate::ControlTemplate) property through local values,
//! app styles, and framework defaults. Most controls expand during insertion;
//! controls whose template structure depends on initial builder state can use
//! Overstory's crate-internal deferred expansion path.
//!
//! Widget input methods are toolkit-side behavior hooks. They are where a
//! control turns routed pointer/key/focus/scroll events into semantic state
//! changes and callbacks. App code that needs raw routed input uses
//! [`Ui::events`](crate::Ui::events); ordinary app code uses semantic builder
//! callbacks such as [`Button::on_activate`](crate::Button::on_activate).
//!
//! Roadmap:
//! - Keep `Widget` small: identity, presentation, optional measure/layout hooks,
//!   template contract, cursor policy, hit testing, and input callbacks.
//! - Let widgets describe their accessibility role and semantic state directly;
//!   selector type tags are for styling, not for accessibility lowering.
//! - Prefer semantic control callbacks, properties, templates, and runtimes
//!   over adding a public raw-input hook for every new behavior.
//! - Keep visual output going through presentation primitives rather than
//!   direct renderer paint callbacks.
//! - Make the content-control and item-control patterns obvious without forcing
//!   every custom widget to subclass a large framework base type.
//! - Add diagnostics around slow hooks and re-entrant mutations before widget
//!   behavior becomes hard to reason about.

use alloc::vec::Vec;
use core::any::Any;

use accesskit::{ActionRequest, Node, NodeId as AccessNodeId, Role};
use cursor_icon::CursorIcon;
use kurbo::{Size, Vec2};
use smallvec::SmallVec;
use understory_box_tree::NodeId;
use understory_presentation::PresentationStore;
use understory_property::{ErasedValue, Property, PropertyId};
use understory_style::TypeTag;

use crate::accessibility::AccessibilityContext;
use crate::activation::ActivationDeniedEvent;
use crate::behavior::{
    ArrangeContext, ArrangeResult, ElementRealizeContext, ElementRealizeResult, MeasureConstraints,
    MeasureContext, PresentContext, WidgetEventContext,
};
use crate::composition::CompositionPolicy;
use crate::element::{Element, ElementId};
use crate::event_runtime::{
    ClickEvent, DragEvent, FocusEvent, KeyEvent, PointerRoutedEvent, ScrollEvent,
};
use crate::generic::present_surface;
use crate::layouts::Layout;
use crate::properties::BuiltInProperties;
pub use crate::spec_ext::{
    ControlStateSpecExt, ControlTemplateSpec, ControlTemplateSpecExt, ElementSpec, ElementSpecExt,
    RangeSpecExt, SurfaceSpecExt, SurfaceStyleExt, TextSpecExt,
};
use crate::style_class::StyleClass;
use crate::template::{TemplateContract, TemplateSlot};
use crate::ui::{ControlState, Ui};

/// Selector identity for an element that has no widget attached
/// (an anonymous container, e.g. a layout-only panel).
pub const TYPE_GENERIC: TypeTag = TypeTag(0);

/// Selector identity for the [`Ui`] root element.
///
/// The root carries its own type rather than [`TYPE_GENERIC`] so the default
/// cascade can give the window surface its themed background and seed the
/// inherited default foreground, without those rules also matching every
/// ordinary layout panel. See [`crate::theme::default_cascade`].
pub const TYPE_ROOT: TypeTag = TypeTag(30);

/// Per-element behavior + state.
///
/// A `Widget` lives in the element as `Box<dyn Widget>`. The trait
/// methods are how the toolkit walks the tree: `present` writes
/// drawing intent, `measure` answers leaf-style sizing, and
/// `template_contract` describes the slots the resolved template should
/// expose.
///
/// The default `present` resolves the canonical `Surface.*` longhands through
/// the cascade and writes a box decoration primitive — most controls' surface
/// intent is exactly that. Controls with bespoke drawing (e.g. shaped text)
/// override.
pub trait Widget: Any + core::fmt::Debug + 'static {
    /// Selector identity for cascade matching, if this widget is a
    /// globally addressable control kind.
    ///
    /// Pure template parts should usually return `None` and rely on
    /// the element's owner-local `PartTag` instead. That keeps parts
    /// from becoming accidental app-wide control types. See
    /// [`crate::template`] for the template identity model.
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

    /// Resolves drawing intent into the presentation store.
    ///
    /// The default implementation runs [`present_surface`], which resolves the
    /// canonical `Surface.*` longhands through the cascade and writes the
    /// element's box decoration primitive. This is the expected path for
    /// controls whose own visual is "a box that fills my bounds"; specialized
    /// controls should override only when they need a different primitive.
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

    /// Returns whether this widget's own presentation changes when its direct
    /// retained child set changes.
    ///
    /// Most widgets paint independently from their children; child insertion or
    /// removal only affects composition and layout. Slot-hosting widgets such
    /// as content presenters use this to switch between their own fallback
    /// presentation and the authored child subtree.
    fn presentation_depends_on_children(&self, _element: &Element) -> bool {
        false
    }

    /// Returns whether retained descendants should route as semantic content
    /// of this element's template owner.
    ///
    /// Most template parts are only visual structure: authored descendants
    /// inside them remain ordinary app content. Content presenters are the
    /// exception for label/content slots such as a button label. A click on
    /// retained text inside the presenter should still be a semantic click on
    /// the owning control.
    fn routes_descendants_to_template_owner(&self, _element: &Element) -> bool {
        false
    }

    /// Called after layout has arranged this element's bounds and recursively
    /// written all descendant bounds.
    ///
    /// The default does nothing. Widgets use this to derive non-tree state
    /// from final geometry, such as scroll extents or presentation timeline
    /// targets. Return [`ArrangeResult::LAYOUT_CHANGED`] when the derived
    /// state writes layout-affecting properties and the UI should run another
    /// layout pass before element realization and presentation.
    fn arrange(&mut self, _ctx: &mut ArrangeContext<'_>) -> ArrangeResult {
        ArrangeResult::UNCHANGED
    }

    /// Called after a full arrange pass has written bounds for the tree.
    ///
    /// Widgets whose retained element child set depends on final viewport
    /// geometry, such as virtualized controls, can synchronize those children
    /// here. Return [`ElementRealizeResult::CHANGED`] when synchronization
    /// changed the element tree or layout and the UI should run another layout
    /// pass before presenting.
    fn realize_elements(&mut self, _ctx: &mut ElementRealizeContext<'_>) -> ElementRealizeResult {
        ElementRealizeResult::UNCHANGED
    }

    /// Returns the semantic slots this widget expects its template to expose.
    ///
    /// The default empty contract is appropriate for non-templated widgets and
    /// controls whose behavior does not depend on named generated parts.
    /// Templated controls override this so custom templates can be diagnosed
    /// without the inspector knowing control-specific details.
    fn template_contract(&self) -> TemplateContract {
        TemplateContract::new()
    }

    /// Called after this widget's resolved template has expanded.
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

    /// Returns whether retained descendants participate in pointer hit
    /// testing.
    ///
    /// This is independent from [`Self::is_hit_test_visible`]: a transparent
    /// container may decline hits on its own box while keeping interactive
    /// children, whereas a decorative subtree such as a badge can disable
    /// both itself and every descendant.
    fn is_hit_test_subtree_visible(&self, _element: &Element) -> bool {
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

    /// Handles a semantic activation attempt denied before raw click dispatch.
    ///
    /// This is intentionally not a routed event and it does not re-enable
    /// disabled controls. Widgets use it only for local denial affordances,
    /// such as a segmented control shaking when the user tries to choose a
    /// disabled segment.
    fn handle_activation_denied(
        &mut self,
        _ctx: &mut WidgetEventContext<'_>,
        _event: ActivationDeniedEvent,
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

/// Generic surface-only widget for template parts.
///
/// It has no global selector type. Style rules should target it with
/// an owner-local `PartTag`, usually anchored under the owning control
/// type. See [`crate::template`] for the distinction between global
/// widget identity and owner-local template part identity.
#[derive(Debug, Default)]
pub(crate) struct SurfacePartWidget;

impl Widget for SurfacePartWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        None
    }
}

/// Widget for the [`Ui`] root element.
///
/// It behaves like an anonymous container — no measurement, the default
/// surface presentation — but reports [`TYPE_ROOT`] so the default cascade can
/// target the window surface specifically. See [`crate::theme`].
#[derive(Debug, Default)]
pub(crate) struct RootWidget;

impl Widget for RootWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_ROOT)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &AccessibilityContext<'_>,
    ) -> Option<Role> {
        Some(Role::GenericContainer)
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
    classes: SmallVec<[StyleClass; 1]>,
    control_state: Option<ControlState>,
    local_properties: SmallVec<[InitialPropertyWrite; 4]>,
}

impl ElementConfig {
    /// Sets layout longhand contributions applied after insertion.
    ///
    /// This follows [`Ui::set_layout`]: only longhands explicitly carried by
    /// `layout` are written, so a size update does not erase independently
    /// styled padding, alignment, overflow, or placement. Padding carried by
    /// the layout shorthand is written to the canonical `Surface.Padding*`
    /// properties before layout spacing is stored.
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
    pub fn add_class(&mut self, class: impl Into<StyleClass>) {
        self.classes.push(class.into());
    }

    /// Returns style classes applied to the resulting element.
    #[must_use]
    pub fn classes(&self) -> &[StyleClass] {
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
            ui.set_initial_local_erased(id, property, write.value);
        }
        for class in self.classes {
            let class = ui.resolve_style_class(class);
            ui.add_initial_class(id, class);
        }
        if let Some(state) = self.control_state {
            ui.set_initial_control_state(id, state);
        }
    }

    pub(crate) fn set_built_in_local<T>(&mut self, property: BuiltInPropertyKey, value: T)
    where
        T: Clone + 'static,
    {
        self.set_initial_property(
            InitialPropertyTarget::BuiltIn(property),
            ErasedValue::new(value),
        );
    }

    pub(crate) fn set_built_in_local_if_absent<T>(&mut self, property: BuiltInPropertyKey, value: T)
    where
        T: Clone + 'static,
    {
        let target = InitialPropertyTarget::BuiltIn(property);
        if self
            .local_properties
            .iter()
            .any(|write| write.target == target)
        {
            return;
        }
        self.set_initial_property(target, ErasedValue::new(value));
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
pub(crate) enum BuiltInPropertyKey {
    Content,
    ControlTemplate,
    Effects,
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
    Text,
    TextLanguage,
    TextWrapMode,
    Transitions,
    Value,
    VisualTransform,
    WordBreak,
    WordSpacing,
    SurfaceBackground,
    SurfaceBorderTopBrush,
    SurfaceBorderRightBrush,
    SurfaceBorderBottomBrush,
    SurfaceBorderLeftBrush,
    SurfaceBorderTopStyle,
    SurfaceBorderRightStyle,
    SurfaceBorderBottomStyle,
    SurfaceBorderLeftStyle,
    SurfaceBorderTopWidth,
    SurfaceBorderRightWidth,
    SurfaceBorderBottomWidth,
    SurfaceBorderLeftWidth,
    SurfaceCornerTopLeftRadius,
    SurfaceCornerTopRightRadius,
    SurfaceCornerBottomRightRadius,
    SurfaceCornerBottomLeftRadius,
    SurfaceCornerTopLeftShape,
    SurfaceCornerTopRightShape,
    SurfaceCornerBottomRightShape,
    SurfaceCornerBottomLeftShape,
    SurfacePaddingTop,
    SurfacePaddingRight,
    SurfacePaddingBottom,
    SurfacePaddingLeft,
}

impl BuiltInPropertyKey {
    fn resolve(self, properties: &BuiltInProperties) -> PropertyId {
        match self {
            Self::Content => properties.content.id(),
            Self::ControlTemplate => properties.control_template.id(),
            Self::Effects => properties.effects.id(),
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
            Self::Text => properties.text.id(),
            Self::TextLanguage => properties.text_language.id(),
            Self::TextWrapMode => properties.text_wrap_mode.id(),
            Self::Transitions => properties.transitions.id(),
            Self::Value => properties.value.id(),
            Self::VisualTransform => properties.visual_transform.id(),
            Self::WordBreak => properties.word_break.id(),
            Self::WordSpacing => properties.word_spacing.id(),
            Self::SurfaceBackground => properties.surface.background.id(),
            Self::SurfaceBorderTopBrush => properties.surface.border_brushes.top.id(),
            Self::SurfaceBorderRightBrush => properties.surface.border_brushes.right.id(),
            Self::SurfaceBorderBottomBrush => properties.surface.border_brushes.bottom.id(),
            Self::SurfaceBorderLeftBrush => properties.surface.border_brushes.left.id(),
            Self::SurfaceBorderTopStyle => properties.surface.border_styles.top.id(),
            Self::SurfaceBorderRightStyle => properties.surface.border_styles.right.id(),
            Self::SurfaceBorderBottomStyle => properties.surface.border_styles.bottom.id(),
            Self::SurfaceBorderLeftStyle => properties.surface.border_styles.left.id(),
            Self::SurfaceBorderTopWidth => properties.surface.border_widths.top.id(),
            Self::SurfaceBorderRightWidth => properties.surface.border_widths.right.id(),
            Self::SurfaceBorderBottomWidth => properties.surface.border_widths.bottom.id(),
            Self::SurfaceBorderLeftWidth => properties.surface.border_widths.left.id(),
            Self::SurfaceCornerTopLeftRadius => properties.surface.corner_radii.top_left.id(),
            Self::SurfaceCornerTopRightRadius => properties.surface.corner_radii.top_right.id(),
            Self::SurfaceCornerBottomRightRadius => {
                properties.surface.corner_radii.bottom_right.id()
            }
            Self::SurfaceCornerBottomLeftRadius => properties.surface.corner_radii.bottom_left.id(),
            Self::SurfaceCornerTopLeftShape => properties.surface.corner_shapes.top_left.id(),
            Self::SurfaceCornerTopRightShape => properties.surface.corner_shapes.top_right.id(),
            Self::SurfaceCornerBottomRightShape => {
                properties.surface.corner_shapes.bottom_right.id()
            }
            Self::SurfaceCornerBottomLeftShape => properties.surface.corner_shapes.bottom_left.id(),
            Self::SurfacePaddingTop => properties.surface.padding_widths.top.id(),
            Self::SurfacePaddingRight => properties.surface.padding_widths.right.id(),
            Self::SurfacePaddingBottom => properties.surface.padding_widths.bottom.id(),
            Self::SurfacePaddingLeft => properties.surface.padding_widths.left.id(),
        }
    }
}
