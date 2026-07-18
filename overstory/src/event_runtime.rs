// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Routed input event state and handler storage.
//!
//! Roadmap:
//! - Keep routed events as the raw input substrate: pointer, keyboard, focus,
//!   scroll, click, and drag share routing, handled state, and handler options.
//!   Controls then expose semantic callbacks from widget behavior instead of
//!   making app code speak raw routes for ordinary use.
//! - Keep raw host event translation outside this runtime. This module should
//!   work with normalized `ui-events` payloads and Overstory element ids.
//! - Use `understory_responder` for route shape, propagation phases, and
//!   dispatch-step metadata while Overstory owns app handler storage, semantic
//!   target interpretation, pointer capture, hover, click, and drag state.
//! - Add capture/release diagnostics, event tracing, and input-modality state
//!   so focus-visible, tooltips, menus, and gestures remain explainable.
//! - Push reusable low-level helpers down to `ui-events` or `understory_*`
//!   crates when several controls need the same construction code.

use alloc::boxed::Box;
use alloc::vec::Vec;

use hashbrown::{HashMap, HashSet};
use kurbo::{Point, Vec2};
use ui_events::ScrollDelta;
use ui_events::keyboard::{KeyboardEvent, Modifiers};
use ui_events::pointer::{
    PointerButton, PointerEvent as RawPointerEvent, PointerInfo, PointerState,
};
use understory_event_state::click::{ClickResult, ClickState};
use understory_event_state::hover::{HoverEvent, HoverState};
use understory_responder::types::{Dispatch, Phase};

use crate::element::ElementId;
use crate::ui::Ui;

pub(crate) type ClickHandler = Box<dyn FnMut(&mut Ui, ClickEvent<'_>)>;
pub(crate) type DragHandler = Box<dyn FnMut(&mut Ui, DragEvent<'_>)>;
pub(crate) type ScrollHandler = Box<dyn FnMut(&mut Ui, ScrollEvent<'_>)>;
pub(crate) type KeyHandler = Box<dyn FnMut(&mut Ui, KeyEvent<'_>)>;
pub(crate) type FocusHandler = Box<dyn FnMut(&mut Ui, FocusEvent<'_>)>;
pub(crate) type PointerHandler = Box<dyn FnMut(&mut Ui, PointerRoutedEvent<'_>)>;

/// Responder dispatch step delivered to a routed Overstory event.
///
/// The step records the element currently being visited, the propagation
/// phase, and responder metadata for the route. Most app and control code
/// should prefer [`RoutedEventContext::source`],
/// [`RoutedEventContext::current`], and [`RoutedEventContext::phase`];
/// [`RoutedEventContext::step`] is for advanced diagnostics or integrations
/// that need the raw responder data.
pub type RoutedDispatchStep = Dispatch<ElementId, ElementId, ()>;

pub(crate) trait HandlerFamily: Copy {
    type Handler;

    fn store(self, runtime: &mut EventRuntime) -> &mut HandlerStore<Self::Handler>;
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct ClickHandlers;

impl HandlerFamily for ClickHandlers {
    type Handler = ClickHandler;

    fn store(self, runtime: &mut EventRuntime) -> &mut HandlerStore<Self::Handler> {
        &mut runtime.click
    }
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct DragHandlers;

impl HandlerFamily for DragHandlers {
    type Handler = DragHandler;

    fn store(self, runtime: &mut EventRuntime) -> &mut HandlerStore<Self::Handler> {
        &mut runtime.drag
    }
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct ScrollHandlers;

impl HandlerFamily for ScrollHandlers {
    type Handler = ScrollHandler;

    fn store(self, runtime: &mut EventRuntime) -> &mut HandlerStore<Self::Handler> {
        &mut runtime.scroll
    }
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct KeyHandlers;

impl HandlerFamily for KeyHandlers {
    type Handler = KeyHandler;

    fn store(self, runtime: &mut EventRuntime) -> &mut HandlerStore<Self::Handler> {
        &mut runtime.key
    }
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct FocusHandlers;

impl HandlerFamily for FocusHandlers {
    type Handler = FocusHandler;

    fn store(self, runtime: &mut EventRuntime) -> &mut HandlerStore<Self::Handler> {
        &mut runtime.focus
    }
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct PointerHandlers {
    kind: PointerEventKind,
}

impl PointerHandlers {
    pub(crate) const fn new(kind: PointerEventKind) -> Self {
        Self { kind }
    }
}

impl HandlerFamily for PointerHandlers {
    type Handler = PointerHandler;

    fn store(self, runtime: &mut EventRuntime) -> &mut HandlerStore<Self::Handler> {
        runtime.pointer_pair_mut(self.kind)
    }
}

/// Options controlling how a routed handler participates in dispatch.
///
/// Handler options are the raw-input escape hatch. Ordinary app code should
/// prefer semantic callbacks such as `Button::on_activate` or
/// `Slider::on_value_changed`; use these options when code needs capture,
/// bubble, handled-event observation, or custom gesture routing.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct RoutedHandlerOptions {
    /// Which part of a responder route should invoke this handler.
    pub scope: RoutedHandlerScope,
    /// Run this handler even when an earlier handler has already marked the
    /// event handled.
    pub handled_events_too: bool,
}

impl RoutedHandlerOptions {
    /// Returns default routed handler behavior.
    ///
    /// Default handlers do not run after an event has already been handled.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            scope: RoutedHandlerScope::EntireRoute,
            handled_events_too: false,
        }
    }

    /// Sets which part of a responder route should invoke the handler.
    #[must_use]
    pub const fn scope(mut self, scope: RoutedHandlerScope) -> Self {
        self.scope = scope;
        self
    }

    /// Sets whether the handler should observe already-handled events.
    #[must_use]
    pub const fn handled_events_too(mut self, handled_events_too: bool) -> Self {
        self.handled_events_too = handled_events_too;
        self
    }
}

/// Route participation policy for a registered routed handler.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum RoutedHandlerScope {
    /// Invoke for every matching element visit along the route.
    #[default]
    EntireRoute,
    /// Invoke only during the capture phase.
    Capture,
    /// Invoke only when the current element is the physical route target.
    Target,
    /// Invoke when the current element is the physical target or is reached
    /// during the bubble phase.
    ///
    /// This is the default app-facing click shape: handlers do not run during
    /// capture, but parent elements still observe clicks that bubble out of
    /// their ordinary descendants.
    TargetOrBubble,
    /// Invoke only during the bubble phase.
    Bubble,
    /// Invoke when the current element is the semantic event target.
    ///
    /// For ordinary elements this is the physical target during
    /// [`Phase::Target`]. For a template-generated source, or retained content
    /// hosted by a content presenter in a template, this is the owning control
    /// during [`Phase::Bubble`], matching the control API's view of a click on
    /// one of its generated or presented parts.
    SemanticTarget,
}

/// The routed event family currently being dispatched.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RoutedEventKind {
    /// Keyboard input routed from the focused element.
    Key,
    /// Pointer button press routed from the hit element.
    PointerDown,
    /// Pointer movement routed from the captured or hit element.
    PointerMove,
    /// Pointer button release routed from the captured or hit element.
    PointerUp,
    /// Pointer entered a hit-tested element.
    PointerEnter,
    /// Pointer left a hit-tested element.
    PointerLeave,
    /// Pointer interaction was canceled.
    PointerCancel,
    /// Primary pointer click routed from the clicked element.
    Click,
    /// Primary pointer drag routed from the original press target.
    Drag,
    /// Pointer wheel/scroll routed from the hit element.
    Scroll,
    /// Focus was gained by the routed source element.
    FocusGained,
    /// Focus was lost by the routed source element.
    FocusLost,
}

/// Routed pointer event family accepted by pointer handler registration APIs.
///
/// This is intentionally narrower than [`ui_events::pointer::PointerEvent`]:
/// scroll is routed through [`ScrollEvent`], and gesture events are not yet a
/// first-class Overstory routed event family.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PointerEventKind {
    /// Pointer button press routed from the hit element.
    Down,
    /// Pointer movement routed from the captured or hit element.
    Move,
    /// Pointer button release routed from the captured or hit element.
    Up,
    /// Pointer entered a hit-tested element.
    Enter,
    /// Pointer left a hit-tested element.
    Leave,
    /// Pointer interaction was canceled.
    Cancel,
}

impl PointerEventKind {
    pub(crate) const COUNT: usize = 6;

    /// Returns the corresponding routed event kind.
    #[must_use]
    pub const fn routed(self) -> RoutedEventKind {
        match self {
            Self::Down => RoutedEventKind::PointerDown,
            Self::Move => RoutedEventKind::PointerMove,
            Self::Up => RoutedEventKind::PointerUp,
            Self::Enter => RoutedEventKind::PointerEnter,
            Self::Leave => RoutedEventKind::PointerLeave,
            Self::Cancel => RoutedEventKind::PointerCancel,
        }
    }

    pub(crate) const fn from_routed(kind: RoutedEventKind) -> Option<Self> {
        match kind {
            RoutedEventKind::PointerDown => Some(Self::Down),
            RoutedEventKind::PointerMove => Some(Self::Move),
            RoutedEventKind::PointerUp => Some(Self::Up),
            RoutedEventKind::PointerEnter => Some(Self::Enter),
            RoutedEventKind::PointerLeave => Some(Self::Leave),
            RoutedEventKind::PointerCancel => Some(Self::Cancel),
            _ => None,
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Down => 0,
            Self::Move => 1,
            Self::Up => 2,
            Self::Enter => 3,
            Self::Leave => 4,
            Self::Cancel => 5,
        }
    }
}

/// Original event source, including template ownership when the source is a
/// generated template element or retained content hosted by a template content
/// presenter.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RoutedEventOrigin {
    /// The route started at an ordinary retained element.
    Element(ElementId),
    /// The route started at an element generated by `owner`'s template, or a
    /// retained descendant hosted by a template content presenter.
    TemplateChild {
        /// Control whose template generated `child`.
        owner: ElementId,
        /// Physical element that started the route.
        child: ElementId,
    },
}

impl RoutedEventOrigin {
    /// Returns the physical element that started the route.
    #[must_use]
    pub const fn source(self) -> ElementId {
        match self {
            Self::Element(source) | Self::TemplateChild { child: source, .. } => source,
        }
    }

    /// Returns the element app/control authors usually consider targeted.
    ///
    /// For template-generated children this is the owning control, not the
    /// generated visual part.
    #[must_use]
    pub const fn semantic_source(self) -> ElementId {
        match self {
            Self::Element(source) => source,
            Self::TemplateChild { owner, .. } => owner,
        }
    }

    /// Returns the owner when the physical source came from a template.
    #[must_use]
    pub const fn template_owner(self) -> Option<ElementId> {
        match self {
            Self::Element(_) => None,
            Self::TemplateChild { owner, .. } => Some(owner),
        }
    }
}

/// Why focus changed.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FocusChangeReason {
    /// Focus changed through a direct [`Ui::set_focus`](crate::Ui::set_focus)
    /// style request.
    Programmatic,
    /// Focus changed because keyboard navigation moved it.
    Keyboard,
    /// Focus changed because pointer input targeted a focusable element.
    Pointer,
}

/// Shared state carried by one routed event dispatch.
///
/// The state is created for a single responder route and is shared by every
/// handler invocation for that route. Calling [`Self::mark_handled`] skips
/// ordinary handlers that did not opt into handled events. Calling
/// [`Self::stop_propagation`] ends dispatch entirely.
#[derive(Debug)]
pub(crate) struct RoutedEventState {
    handled: core::cell::Cell<bool>,
    propagation_stopped: core::cell::Cell<bool>,
}

impl Default for RoutedEventState {
    fn default() -> Self {
        Self::new()
    }
}

impl RoutedEventState {
    /// Creates a fresh unhandled route state.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            handled: core::cell::Cell::new(false),
            propagation_stopped: core::cell::Cell::new(false),
        }
    }

    /// Marks this event handled.
    pub(crate) fn mark_handled(&self) {
        self.handled.set(true);
    }

    /// Returns whether this event has been marked handled.
    #[must_use]
    pub(crate) fn is_handled(&self) -> bool {
        self.handled.get()
    }

    /// Stops propagation without marking the event handled.
    pub(crate) fn stop_propagation(&self) {
        self.propagation_stopped.set(true);
    }

    /// Returns whether propagation has been explicitly stopped.
    #[must_use]
    pub(crate) fn is_propagation_stopped(&self) -> bool {
        self.propagation_stopped.get()
    }

    pub(crate) fn should_stop_route(&self) -> bool {
        self.is_propagation_stopped()
    }
}

/// Scroll-specific state shared by one routed scroll dispatch.
#[derive(Debug)]
pub(crate) struct ScrollRouteState {
    remaining: core::cell::Cell<Vec2>,
}

impl ScrollRouteState {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            remaining: core::cell::Cell::new(Vec2::new(1.0, 1.0)),
        }
    }

    fn remaining(&self) -> Vec2 {
        self.remaining.get()
    }

    fn consume(&self, requested: Vec2, consumed: Vec2) {
        let mut remaining = self.remaining.get();
        remaining.x *= remaining_fraction(requested.x, consumed.x);
        remaining.y *= remaining_fraction(requested.y, consumed.y);
        self.remaining.set(remaining);
    }
}

fn remaining_fraction(requested: f64, consumed: f64) -> f64 {
    if requested.abs() <= f64::EPSILON {
        return 1.0;
    }
    (1.0 - (consumed / requested).clamp(0.0, 1.0)).clamp(0.0, 1.0)
}

/// Common route metadata attached to every routed event payload.
///
/// A context is a thin Overstory view over one responder dispatch step plus
/// shared route state. Use [`Self::source`] for the physical event source,
/// [`Self::current`] for the element currently being visited, and
/// [`Self::phase`] for capture/target/bubble routing. Use
/// [`Self::semantic_source`] or [`Self::is_semantic_target`] when handling
/// control/template intent rather than raw visual parts.
#[derive(Copy, Clone, Debug)]
pub struct RoutedEventContext<'a> {
    /// The event family being dispatched.
    pub kind: RoutedEventKind,
    /// Source classification for the route.
    pub origin: RoutedEventOrigin,
    step: &'a RoutedDispatchStep,
    state: &'a RoutedEventState,
}

impl<'a> RoutedEventContext<'a> {
    pub(crate) fn new(
        kind: RoutedEventKind,
        origin: RoutedEventOrigin,
        step: &'a RoutedDispatchStep,
        state: &'a RoutedEventState,
    ) -> Self {
        Self {
            kind,
            origin,
            step,
            state,
        }
    }

    /// Returns the physical element where this route started.
    ///
    /// For a templated control this can be a generated template part or
    /// retained content hosted by a content presenter. Use
    /// [`Self::semantic_source`] when the handler wants the owning control.
    #[must_use]
    pub const fn source(&self) -> ElementId {
        self.origin.source()
    }

    /// Returns the element currently being visited by the responder route.
    ///
    /// Handlers are registered on this element for the current dispatch step.
    #[must_use]
    pub const fn current(&self) -> ElementId {
        self.step.node
    }

    /// Returns the current propagation phase.
    ///
    /// The phase comes from `understory_responder` and follows
    /// capture → target → bubble ordering.
    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.step.phase
    }

    /// Returns the current responder dispatch step.
    ///
    /// Most app and control code should prefer the context helpers. This raw
    /// step is exposed for diagnostics and integrations that need responder
    /// metadata beyond the ordinary source/current/phase view.
    #[must_use]
    pub const fn step(&self) -> &RoutedDispatchStep {
        self.step
    }

    /// Returns the element app/control authors usually consider targeted.
    ///
    /// This is the route source for ordinary elements and the template owner
    /// when the route source is a generated template child.
    #[must_use]
    pub const fn semantic_source(&self) -> ElementId {
        self.origin.semantic_source()
    }

    /// Returns whether this handler is running at the physical route target.
    #[must_use]
    pub fn is_target(&self) -> bool {
        self.current() == self.source() && self.step.is_target()
    }

    /// Returns whether this handler is running at the semantic target.
    ///
    /// A template owner is the semantic target for events whose physical source
    /// is one of its generated template children. This lets controls handle
    /// their generated parts without matching raw bubble phases in every
    /// handler.
    #[must_use]
    pub fn is_semantic_target(&self) -> bool {
        match self.origin {
            RoutedEventOrigin::Element(source) => self.current() == source && self.step.is_target(),
            RoutedEventOrigin::TemplateChild { owner, .. } => {
                self.current() == owner && self.step.is_bubble()
            }
        }
    }

    /// Marks this event handled.
    ///
    /// Handled events skip ordinary handlers that did not opt into handled
    /// events.
    pub fn mark_handled(&self) {
        self.state.mark_handled();
    }

    /// Returns whether this event has been marked handled.
    #[must_use]
    pub fn is_handled(&self) -> bool {
        self.state.is_handled()
    }

    /// Stops this event's remaining route without marking it handled.
    pub fn stop_propagation(&self) {
        self.state.stop_propagation();
    }

    /// Returns whether this event's propagation has been stopped.
    #[must_use]
    pub fn is_propagation_stopped(&self) -> bool {
        self.state.is_propagation_stopped()
    }
}

impl RoutedHandlerScope {
    /// Returns whether `route` should invoke a handler with this scope.
    #[must_use]
    pub fn matches(self, route: RoutedEventContext<'_>) -> bool {
        match self {
            Self::EntireRoute => true,
            Self::Capture => route.step().is_capture(),
            Self::Target => route.is_target(),
            Self::TargetOrBubble => route.is_target() || route.step().is_bubble(),
            Self::Bubble => route.step().is_bubble(),
            Self::SemanticTarget => route.is_semantic_target(),
        }
    }
}

/// Result returned by routed event dispatch helpers.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct RoutedDispatchResult {
    /// Whether the routed event was consumed by a handler.
    pub handled: bool,
    /// Whether a handler explicitly stopped propagation.
    pub propagation_stopped: bool,
}

impl RoutedDispatchResult {
    pub(crate) fn from_state(state: &RoutedEventState) -> Self {
        Self {
            handled: state.is_handled(),
            propagation_stopped: state.is_propagation_stopped(),
        }
    }
}

/// Payload delivered to a key handler.
#[derive(Debug)]
pub struct KeyEvent<'a> {
    /// Shared routed-event metadata.
    pub route: RoutedEventContext<'a>,
    /// The full normalized keyboard event.
    pub key: &'a KeyboardEvent,
}

impl<'a> core::ops::Deref for KeyEvent<'a> {
    type Target = RoutedEventContext<'a>;

    fn deref(&self) -> &Self::Target {
        &self.route
    }
}

impl KeyEvent<'_> {
    /// Marks this key event handled.
    pub fn mark_handled(&self) {
        self.route.mark_handled();
    }

    /// Returns whether this key event has been marked handled.
    #[must_use]
    pub fn is_handled(&self) -> bool {
        self.route.is_handled()
    }

    /// Stops this key event's remaining route without marking it handled.
    pub fn stop_propagation(&self) {
        self.route.stop_propagation();
    }

    /// Returns whether this key event's propagation has been stopped.
    #[must_use]
    pub fn is_propagation_stopped(&self) -> bool {
        self.route.is_propagation_stopped()
    }
}

/// Result returned by [`Ui::dispatch_key`].
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyDispatchResult {
    /// Whether the key was consumed by framework policy or a key handler.
    pub handled: bool,
    /// Whether a key handler explicitly stopped propagation.
    pub propagation_stopped: bool,
    /// Whether dispatch changed the focused element.
    pub focus_changed: bool,
    /// Focused element after dispatch, if any.
    pub focused: Option<ElementId>,
}

/// Result returned by [`Ui::dispatch_pointer_event`](crate::Ui::dispatch_pointer_event).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct PointerDispatchResult {
    /// Whether a routed pointer-derived event was handled.
    pub handled: bool,
    /// Whether a routed pointer-derived event explicitly stopped propagation.
    pub propagation_stopped: bool,
    /// Whether pointer dispatch opened or closed transient overlay UI.
    pub overlay_changed: bool,
    /// Whether the event changed focus.
    pub focus_changed: bool,
    /// Whether the event changed hover state.
    pub hover_changed: bool,
    /// Whether the event changed pressed visual state.
    pub press_changed: bool,
    /// Whether the event changed the current primary pointer capture target.
    pub capture_changed: bool,
}

impl PointerDispatchResult {
    /// Returns whether the event changed UI state or was consumed by a routed
    /// handler.
    #[must_use]
    pub const fn changed(self) -> bool {
        self.handled
            || self.propagation_stopped
            || self.overlay_changed
            || self.focus_changed
            || self.hover_changed
            || self.press_changed
            || self.capture_changed
    }

    pub(crate) fn include_routed(&mut self, routed: RoutedDispatchResult) {
        self.handled |= routed.handled;
        self.propagation_stopped |= routed.propagation_stopped;
    }
}

/// Payload delivered to a pointer handler.
#[derive(Copy, Clone, Debug)]
pub struct PointerRoutedEvent<'a> {
    /// Shared routed-event metadata.
    pub route: RoutedEventContext<'a>,
    /// Pointer identity and device type.
    pub pointer: PointerInfo,
    /// Current pointer state, when this event carries coordinates.
    pub state: Option<&'a PointerState>,
    /// Current logical pointer position, when available.
    pub position: Option<Point>,
    /// Button associated with this event, if any.
    pub button: Option<PointerButton>,
    /// Keyboard modifiers active when the event was produced.
    pub modifiers: Modifiers,
    /// Original normalized pointer event from `ui_events`, when this routed
    /// event came directly from host input.
    pub raw: Option<&'a RawPointerEvent>,
}

impl<'a> core::ops::Deref for PointerRoutedEvent<'a> {
    type Target = RoutedEventContext<'a>;

    fn deref(&self) -> &Self::Target {
        &self.route
    }
}

impl PointerRoutedEvent<'_> {
    /// Marks this pointer event handled and skips ordinary handlers that did
    /// not opt into handled events.
    pub fn mark_handled(&self) {
        self.route.mark_handled();
    }

    /// Returns whether this pointer event has been marked handled.
    #[must_use]
    pub fn is_handled(&self) -> bool {
        self.route.is_handled()
    }

    /// Stops this pointer event's remaining route without marking it handled.
    pub fn stop_propagation(&self) {
        self.route.stop_propagation();
    }

    /// Returns whether this pointer event's propagation has been stopped.
    #[must_use]
    pub fn is_propagation_stopped(&self) -> bool {
        self.route.is_propagation_stopped()
    }
}

/// Payload delivered to a focus handler.
#[derive(Copy, Clone, Debug)]
pub struct FocusEvent<'a> {
    /// Shared routed-event metadata.
    pub route: RoutedEventContext<'a>,
    /// Element that had focus before the change.
    pub previous: Option<ElementId>,
    /// Element that has focus after the change.
    pub focused: Option<ElementId>,
    /// Whether the new focus should be rendered as keyboard-visible focus.
    pub focus_visible: bool,
    /// Why focus changed.
    pub reason: FocusChangeReason,
}

impl<'a> core::ops::Deref for FocusEvent<'a> {
    type Target = RoutedEventContext<'a>;

    fn deref(&self) -> &Self::Target {
        &self.route
    }
}

impl FocusEvent<'_> {
    /// Marks this focus event handled and stops ordinary handlers for the
    /// remaining route unless they opted into handled events.
    pub fn mark_handled(&self) {
        self.route.mark_handled();
    }

    /// Returns whether this focus event has been marked handled.
    #[must_use]
    pub fn is_handled(&self) -> bool {
        self.route.is_handled()
    }

    /// Stops this focus event's remaining route without marking it handled.
    pub fn stop_propagation(&self) {
        self.route.stop_propagation();
    }

    /// Returns whether this focus event's propagation has been stopped.
    #[must_use]
    pub fn is_propagation_stopped(&self) -> bool {
        self.route.is_propagation_stopped()
    }
}

/// Payload delivered to a drag handler as the primary pointer moves while
/// pressed.
#[derive(Copy, Clone, Debug)]
pub struct DragEvent<'a> {
    /// Shared routed-event metadata.
    pub route: RoutedEventContext<'a>,
    /// Current pointer position in physical pixels.
    pub position: Point,
    /// Pointer position when the press started, in physical pixels.
    pub press_position: Point,
    /// Movement since the previous drag dispatch in physical pixels.
    pub delta: Vec2,
}

impl<'a> core::ops::Deref for DragEvent<'a> {
    type Target = RoutedEventContext<'a>;

    fn deref(&self) -> &Self::Target {
        &self.route
    }
}

impl DragEvent<'_> {
    /// Marks this drag event handled and skips ordinary handlers that did not
    /// opt into handled events.
    pub fn mark_handled(&self) {
        self.route.mark_handled();
    }

    /// Returns whether this drag event has been marked handled.
    #[must_use]
    pub fn is_handled(&self) -> bool {
        self.route.is_handled()
    }

    /// Stops this drag event's remaining route without marking it handled.
    pub fn stop_propagation(&self) {
        self.route.stop_propagation();
    }

    /// Returns whether this drag event's propagation has been stopped.
    #[must_use]
    pub fn is_propagation_stopped(&self) -> bool {
        self.route.is_propagation_stopped()
    }
}

/// Payload delivered to a click handler.
#[derive(Copy, Clone, Debug)]
pub struct ClickEvent<'a> {
    /// Shared routed-event metadata.
    pub route: RoutedEventContext<'a>,
    /// Pointer position at click-recognition time, in physical pixels.
    pub position: Point,
    /// Keyboard modifiers active when this click was recognized.
    pub modifiers: Modifiers,
}

impl<'a> core::ops::Deref for ClickEvent<'a> {
    type Target = RoutedEventContext<'a>;

    fn deref(&self) -> &Self::Target {
        &self.route
    }
}

impl ClickEvent<'_> {
    /// Marks this click event handled and skips ordinary handlers that did not
    /// opt into handled events.
    pub fn mark_handled(&self) {
        self.route.mark_handled();
    }

    /// Returns whether this click event has been marked handled.
    #[must_use]
    pub fn is_handled(&self) -> bool {
        self.route.is_handled()
    }

    /// Stops this click event's remaining route without marking it handled.
    pub fn stop_propagation(&self) {
        self.route.stop_propagation();
    }

    /// Returns whether this click event's propagation has been stopped.
    #[must_use]
    pub fn is_propagation_stopped(&self) -> bool {
        self.route.is_propagation_stopped()
    }
}

/// Payload delivered to a scroll handler.
///
/// This framework-created payload is non-exhaustive because routed scroll
/// consumption carries dispatch-private state. Callers migrating from direct
/// struct construction should synthesize or dispatch scroll input through
/// [`Ui`](crate::Ui) instead.
#[derive(Copy, Clone, Debug)]
#[non_exhaustive]
pub struct ScrollEvent<'a> {
    /// Shared routed-event metadata.
    pub route: RoutedEventContext<'a>,
    pub(crate) scroll: &'a ScrollRouteState,
    /// Pointer position at scroll time, in physical pixels.
    pub position: Point,
    /// Raw scroll delta from `ui_events`.
    pub delta: ScrollDelta,
    /// Keyboard modifiers active when the scroll was requested.
    pub modifiers: Modifiers,
}

impl<'a> core::ops::Deref for ScrollEvent<'a> {
    type Target = RoutedEventContext<'a>;

    fn deref(&self) -> &Self::Target {
        &self.route
    }
}

impl ScrollEvent<'_> {
    /// Converts the event's unconsumed delta into logical pixels.
    ///
    /// A scrollable descendant can consume part of a routed delta. Ancestors
    /// then observe only the remaining fraction, converted with their own line
    /// and page extents.
    #[must_use]
    pub fn logical_delta(&self, line_size: Vec2, page_size: Vec2) -> Vec2 {
        let delta = match self.delta {
            ScrollDelta::PixelDelta(delta) => Vec2::new(delta.x, delta.y),
            ScrollDelta::LineDelta(x, y) => {
                Vec2::new(f64::from(x) * line_size.x, f64::from(y) * line_size.y)
            }
            ScrollDelta::PageDelta(x, y) => {
                Vec2::new(f64::from(x) * page_size.x, f64::from(y) * page_size.y)
            }
        };
        let remaining = self.scroll.remaining();
        Vec2::new(delta.x * remaining.x, delta.y * remaining.y)
    }

    pub(crate) fn consume_logical_delta(&self, requested: Vec2, consumed: Vec2) {
        self.scroll.consume(requested, consumed);
        let unconsumed = requested - consumed;
        if unconsumed.x.abs() <= f64::EPSILON && unconsumed.y.abs() <= f64::EPSILON {
            self.mark_handled();
        }
    }

    /// Marks this scroll event handled and skips ordinary handlers that did not
    /// opt into handled events.
    pub fn mark_handled(&self) {
        self.route.mark_handled();
    }

    /// Returns whether this scroll event has been marked handled.
    #[must_use]
    pub fn is_handled(&self) -> bool {
        self.route.is_handled()
    }

    /// Stops this scroll event's remaining route without marking it handled.
    pub fn stop_propagation(&self) {
        self.route.stop_propagation();
    }

    /// Returns whether this scroll event's propagation has been stopped.
    #[must_use]
    pub fn is_propagation_stopped(&self) -> bool {
        self.route.is_propagation_stopped()
    }
}

/// Routed event handler stores and in-flight removal tracking.
pub(crate) struct EventRuntime {
    /// App raw click handlers keyed by current route element.
    click: HandlerStore<ClickHandler>,
    /// App raw drag handlers keyed by current route element.
    drag: HandlerStore<DragHandler>,
    /// User scroll handlers keyed by current route element.
    scroll: HandlerStore<ScrollHandler>,
    /// App raw keyboard handlers keyed by current route element.
    key: HandlerStore<KeyHandler>,
    /// Focus gained/lost handlers keyed by current route element.
    focus: HandlerStore<FocusHandler>,
    /// Pointer handlers keyed first by routed pointer event kind, then route element.
    pointer: [HandlerStore<PointerHandler>; PointerEventKind::COUNT],
    /// Last hover path, used to synthesize pointer enter/leave routes.
    hover: HoverState<ElementId>,
    /// Click recognizer state derived from pointer press/move/up events.
    clicks: ClickState<ElementId>,
    /// Element that received the primary pointer press being tracked.
    press_target: Option<ElementId>,
    /// Explicit pointer capture target for move/up/cancel routing.
    pointer_capture: Option<ElementId>,
    /// Pointer position at the tracked press start.
    press_position: Option<Point>,
    /// Previous pointer position used to compute drag deltas.
    last_drag_position: Option<Point>,
}

impl EventRuntime {
    pub(crate) fn new() -> Self {
        Self {
            click: HandlerStore::new(),
            drag: HandlerStore::new(),
            scroll: HandlerStore::new(),
            key: HandlerStore::new(),
            focus: HandlerStore::new(),
            pointer: core::array::from_fn(|_| HandlerStore::new()),
            hover: HoverState::new(),
            clicks: ClickState::new(),
            press_target: None,
            pointer_capture: None,
            press_position: None,
            last_drag_position: None,
        }
    }

    pub(crate) fn on_click_with_options(
        &mut self,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: ClickHandler,
    ) {
        self.click.insert(id, options, handler);
    }

    pub(crate) fn remove_click(&mut self, id: ElementId) {
        self.click.remove(id);
    }

    pub(crate) fn on_drag_with_options(
        &mut self,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: DragHandler,
    ) {
        self.drag.insert(id, options, handler);
    }

    pub(crate) fn remove_drag(&mut self, id: ElementId) {
        self.drag.remove(id);
    }

    pub(crate) fn on_scroll_with_options(
        &mut self,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: ScrollHandler,
    ) {
        self.scroll.insert(id, options, handler);
    }

    pub(crate) fn remove_scroll(&mut self, id: ElementId) {
        self.scroll.remove(id);
    }

    pub(crate) fn on_key_with_options(
        &mut self,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: KeyHandler,
    ) {
        self.key.insert(id, options, handler);
    }

    pub(crate) fn remove_key(&mut self, id: ElementId) {
        self.key.remove(id);
    }

    pub(crate) fn on_focus_with_options(
        &mut self,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: FocusHandler,
    ) {
        self.focus.insert(id, options, handler);
    }

    pub(crate) fn remove_focus(&mut self, id: ElementId) {
        self.focus.remove(id);
    }

    pub(crate) fn on_pointer_with_options(
        &mut self,
        kind: PointerEventKind,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: PointerHandler,
    ) {
        self.pointer_pair_mut(kind).insert(id, options, handler);
    }

    pub(crate) fn remove_pointer(&mut self, kind: PointerEventKind, id: ElementId) {
        self.pointer_pair_mut(kind).remove(id);
    }

    pub(crate) fn remove_target(&mut self, id: ElementId) {
        self.click.remove_target(id);
        self.drag.remove_target(id);
        self.scroll.remove_target(id);
        self.key.remove_target(id);
        self.focus.remove_target(id);
        for pointer in &mut self.pointer {
            pointer.remove_target(id);
        }
        if self.press_target == Some(id) {
            self.press_target = None;
        }
        if self.pointer_capture == Some(id) {
            self.pointer_capture = None;
            self.clear_press();
            self.cancel_click();
        }
    }

    pub(crate) fn take_handler<F>(
        &mut self,
        family: F,
        id: ElementId,
        handled: bool,
    ) -> Option<HandlerRegistration<F::Handler>>
    where
        F: HandlerFamily,
    {
        family.store(self).take(id, handled)
    }

    pub(crate) fn begin_handler<F>(&mut self, family: F, id: ElementId)
    where
        F: HandlerFamily,
    {
        family.store(self).begin(id);
    }

    pub(crate) fn end_handler<F>(&mut self, family: F, id: ElementId)
    where
        F: HandlerFamily,
    {
        family.store(self).end(id);
    }

    pub(crate) fn handler_removed<F>(&mut self, family: F, id: ElementId) -> bool
    where
        F: HandlerFamily,
    {
        family.store(self).take_removed(id)
    }

    pub(crate) fn insert_handler_if_absent<F>(
        &mut self,
        family: F,
        id: ElementId,
        handler: HandlerRegistration<F::Handler>,
    ) where
        F: HandlerFamily,
    {
        family.store(self).insert_if_absent(id, handler);
    }

    pub(crate) fn has_handler<F>(&mut self, family: F, id: ElementId) -> bool
    where
        F: HandlerFamily,
    {
        family.store(self).has(id)
    }

    pub(crate) fn hovered(&self) -> Option<ElementId> {
        self.hover.current_path().last().copied()
    }

    pub(crate) fn update_hover_path(&mut self, path: &[ElementId]) -> Vec<HoverEvent<ElementId>> {
        self.hover.update_path(path)
    }

    pub(crate) fn clear_hover(&mut self) -> Vec<HoverEvent<ElementId>> {
        self.hover.clear()
    }

    pub(crate) fn click_down(&mut self, target: ElementId, position: Point, time_ms: u64) {
        self.clicks.on_down(None, None, target, position, time_ms);
    }

    pub(crate) fn click_up(
        &mut self,
        target: &ElementId,
        position: Point,
        time_ms: u64,
    ) -> ClickResult<ElementId> {
        self.clicks.on_up(None, None, target, position, time_ms)
    }

    pub(crate) fn cancel_click(&mut self) {
        let _ = self.clicks.cancel(None);
    }

    pub(crate) fn press_target(&self) -> Option<ElementId> {
        self.press_target
    }

    pub(crate) fn take_press_target(&mut self) -> Option<ElementId> {
        self.press_target.take()
    }

    pub(crate) fn set_press_target(&mut self, target: Option<ElementId>) {
        self.press_target = target;
    }

    pub(crate) fn pointer_capture(&self) -> Option<ElementId> {
        self.pointer_capture
    }

    pub(crate) fn set_pointer_capture(&mut self, target: Option<ElementId>) {
        self.pointer_capture = target;
    }

    pub(crate) fn start_press(&mut self, position: Point) {
        self.press_position = Some(position);
        self.last_drag_position = Some(position);
    }

    pub(crate) fn clear_press(&mut self) {
        self.press_position = None;
        self.last_drag_position = None;
    }

    pub(crate) fn press_position(&self) -> Option<Point> {
        self.press_position
    }

    pub(crate) fn last_drag_position(&self) -> Option<Point> {
        self.last_drag_position
    }

    pub(crate) fn set_last_drag_position(&mut self, position: Point) {
        self.last_drag_position = Some(position);
    }

    fn pointer_pair_mut(&mut self, kind: PointerEventKind) -> &mut HandlerStore<PointerHandler> {
        &mut self.pointer[kind.index()]
    }
}

pub(crate) struct HandlerStore<H> {
    /// Registered handlers keyed by element id.
    handlers: HashMap<ElementId, HandlerRegistration<H>>,
    /// Tracks removals while a handler is temporarily taken for dispatch.
    dispatch: HandlerDispatchTracker,
}

impl<H> HandlerStore<H> {
    fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            dispatch: HandlerDispatchTracker::default(),
        }
    }

    fn insert(&mut self, id: ElementId, options: RoutedHandlerOptions, handler: H) {
        self.handlers
            .insert(id, HandlerRegistration { options, handler });
    }

    fn remove(&mut self, id: ElementId) {
        self.handlers.remove(&id);
        self.dispatch.note_removal(id);
    }

    fn remove_target(&mut self, id: ElementId) {
        self.remove(id);
    }

    fn take(&mut self, id: ElementId, handled: bool) -> Option<HandlerRegistration<H>> {
        if handled
            && !self
                .handlers
                .get(&id)
                .is_some_and(|entry| entry.options.handled_events_too)
        {
            return None;
        }
        self.handlers.remove(&id)
    }

    fn begin(&mut self, id: ElementId) {
        self.dispatch.begin(id);
    }

    fn end(&mut self, id: ElementId) {
        self.dispatch.end(id);
    }

    fn take_removed(&mut self, id: ElementId) -> bool {
        self.dispatch.take_removed(id)
    }

    fn insert_if_absent(&mut self, id: ElementId, handler: HandlerRegistration<H>) {
        self.handlers.entry(id).or_insert(handler);
    }

    fn has(&self, id: ElementId) -> bool {
        self.handlers.contains_key(&id)
    }
}

/// Handler plus dispatch options.
pub(crate) struct HandlerRegistration<H> {
    /// Dispatch options for this handler.
    pub(crate) options: RoutedHandlerOptions,
    /// Callback body.
    pub(crate) handler: H,
}

#[derive(Default)]
struct HandlerDispatchTracker {
    /// Stack of currently dispatching handlers.
    ///
    /// A stack allows callback-driven nested dispatch to remove the correct
    /// handler without losing the outer dispatch context.
    active: Vec<ElementId>,
    /// Handlers removed while present in `active`.
    removals: HashSet<ElementId>,
}

impl HandlerDispatchTracker {
    fn begin(&mut self, id: ElementId) {
        self.active.push(id);
    }

    fn end(&mut self, id: ElementId) {
        let popped = self.active.pop();
        debug_assert!(
            popped == Some(id),
            "handler dispatch tracker should end the current handler"
        );
    }

    fn note_removal(&mut self, id: ElementId) {
        if self.active.contains(&id) {
            self.removals.insert(id);
        }
    }

    fn take_removed(&mut self, id: ElementId) -> bool {
        self.removals.remove(&id)
    }
}
