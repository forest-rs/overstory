// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

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
use understory_responder::types::Phase;

use crate::element::ElementId;
use crate::ui::Ui;

pub(crate) type ClickHandler = Box<dyn FnMut(&mut Ui, ClickEvent<'_>)>;
pub(crate) type DragHandler = Box<dyn FnMut(&mut Ui, DragEvent<'_>)>;
pub(crate) type ScrollHandler = Box<dyn FnMut(&mut Ui, ScrollEvent<'_>)>;
pub(crate) type KeyHandler = Box<dyn FnMut(&mut Ui, KeyEvent<'_>)>;
pub(crate) type FocusHandler = Box<dyn FnMut(&mut Ui, FocusEvent<'_>)>;
pub(crate) type PointerHandler = Box<dyn FnMut(&mut Ui, PointerRoutedEvent<'_>)>;

/// Handler ownership lane.
///
/// Control handlers implement built-in widget behavior; user handlers are
/// registered through public `Ui::on_*` APIs. Dispatch can run both lanes while
/// preserving separate removal/replacement state.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum HandlerLane {
    /// Application-supplied handler registered through public APIs.
    User,
    /// Framework/control behavior handler installed by built-in widgets.
    Control,
}

/// Options controlling how a routed handler participates in dispatch.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct RoutedHandlerOptions {
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
            handled_events_too: false,
        }
    }

    /// Sets whether the handler should observe already-handled events.
    #[must_use]
    pub const fn handled_events_too(mut self, handled_events_too: bool) -> Self {
        self.handled_events_too = handled_events_too;
        self
    }
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

/// Common route metadata attached to every routed event payload.
#[derive(Copy, Clone, Debug)]
pub struct RoutedEventContext<'a> {
    /// The event family being dispatched.
    pub kind: RoutedEventKind,
    /// The element the route is rooted at.
    pub source: ElementId,
    /// The element this handler is registered on.
    pub current: ElementId,
    /// Current propagation phase.
    pub phase: Phase,
    state: &'a RoutedEventState,
}

impl<'a> RoutedEventContext<'a> {
    pub(crate) fn new(
        kind: RoutedEventKind,
        source: ElementId,
        current: ElementId,
        phase: Phase,
        state: &'a RoutedEventState,
    ) -> Self {
        Self {
            kind,
            source,
            current,
            phase,
            state,
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
#[derive(Copy, Clone, Debug)]
pub struct ScrollEvent<'a> {
    /// Shared routed-event metadata.
    pub route: RoutedEventContext<'a>,
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
    /// Converts the event's raw delta into logical pixels.
    #[must_use]
    pub fn logical_delta(&self, line_size: Vec2, page_size: Vec2) -> Vec2 {
        match self.delta {
            ScrollDelta::PixelDelta(delta) => Vec2::new(delta.x, delta.y),
            ScrollDelta::LineDelta(x, y) => {
                Vec2::new(f64::from(x) * line_size.x, f64::from(y) * line_size.y)
            }
            ScrollDelta::PageDelta(x, y) => {
                Vec2::new(f64::from(x) * page_size.x, f64::from(y) * page_size.y)
            }
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
    /// User/control click handlers keyed by current route element.
    click: HandlerPair<ClickHandler>,
    /// User/control drag handlers keyed by current route element.
    drag: HandlerPair<DragHandler>,
    /// User scroll handlers keyed by current route element.
    scroll: HandlerPair<ScrollHandler>,
    /// User/control keyboard handlers keyed by current route element.
    key: HandlerPair<KeyHandler>,
    /// Focus gained/lost handlers keyed by current route element.
    focus: HandlerPair<FocusHandler>,
    /// Pointer down handlers keyed by current route element.
    pointer_down: HandlerPair<PointerHandler>,
    /// Pointer move handlers keyed by current route element.
    pointer_move: HandlerPair<PointerHandler>,
    /// Pointer up handlers keyed by current route element.
    pointer_up: HandlerPair<PointerHandler>,
    /// Pointer enter handlers keyed by current route element.
    pointer_enter: HandlerPair<PointerHandler>,
    /// Pointer leave handlers keyed by current route element.
    pointer_leave: HandlerPair<PointerHandler>,
    /// Pointer cancel handlers keyed by current route element.
    pointer_cancel: HandlerPair<PointerHandler>,
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
            click: HandlerPair::new(),
            drag: HandlerPair::new(),
            scroll: HandlerPair::new(),
            key: HandlerPair::new(),
            focus: HandlerPair::new(),
            pointer_down: HandlerPair::new(),
            pointer_move: HandlerPair::new(),
            pointer_up: HandlerPair::new(),
            pointer_enter: HandlerPair::new(),
            pointer_leave: HandlerPair::new(),
            pointer_cancel: HandlerPair::new(),
            hover: HoverState::new(),
            clicks: ClickState::new(),
            press_target: None,
            pointer_capture: None,
            press_position: None,
            last_drag_position: None,
        }
    }

    pub(crate) fn on_click(&mut self, id: ElementId, handler: ClickHandler) {
        self.on_click_with_options(id, RoutedHandlerOptions::default(), handler);
    }

    pub(crate) fn on_click_with_options(
        &mut self,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: ClickHandler,
    ) {
        self.click.insert(HandlerLane::User, id, options, handler);
    }

    pub(crate) fn on_control_click(&mut self, id: ElementId, handler: ClickHandler) {
        self.click.insert(
            HandlerLane::Control,
            id,
            RoutedHandlerOptions::default(),
            handler,
        );
    }

    pub(crate) fn remove_click(&mut self, id: ElementId) {
        self.click.remove(HandlerLane::User, id);
    }

    pub(crate) fn on_drag(&mut self, id: ElementId, handler: DragHandler) {
        self.on_drag_with_options(id, RoutedHandlerOptions::default(), handler);
    }

    pub(crate) fn on_drag_with_options(
        &mut self,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: DragHandler,
    ) {
        self.drag.insert(HandlerLane::User, id, options, handler);
    }

    pub(crate) fn on_control_drag(&mut self, id: ElementId, handler: DragHandler) {
        self.drag.insert(
            HandlerLane::Control,
            id,
            RoutedHandlerOptions::default(),
            handler,
        );
    }

    pub(crate) fn remove_drag(&mut self, id: ElementId) {
        self.drag.remove(HandlerLane::User, id);
    }

    pub(crate) fn on_scroll(&mut self, id: ElementId, handler: ScrollHandler) {
        self.on_scroll_with_options(id, RoutedHandlerOptions::default(), handler);
    }

    pub(crate) fn on_scroll_with_options(
        &mut self,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: ScrollHandler,
    ) {
        self.scroll.insert(HandlerLane::User, id, options, handler);
    }

    pub(crate) fn remove_scroll(&mut self, id: ElementId) {
        self.scroll.remove(HandlerLane::User, id);
    }

    pub(crate) fn on_key(&mut self, id: ElementId, handler: KeyHandler) {
        self.on_key_with_options(id, RoutedHandlerOptions::default(), handler);
    }

    pub(crate) fn on_key_with_options(
        &mut self,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: KeyHandler,
    ) {
        self.key.insert(HandlerLane::User, id, options, handler);
    }

    pub(crate) fn on_control_key(&mut self, id: ElementId, handler: KeyHandler) {
        self.key.insert(
            HandlerLane::Control,
            id,
            RoutedHandlerOptions::default(),
            handler,
        );
    }

    pub(crate) fn remove_key(&mut self, id: ElementId) {
        self.key.remove(HandlerLane::User, id);
    }

    pub(crate) fn on_focus(&mut self, id: ElementId, handler: FocusHandler) {
        self.on_focus_with_options(id, RoutedHandlerOptions::default(), handler);
    }

    pub(crate) fn on_focus_with_options(
        &mut self,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: FocusHandler,
    ) {
        self.focus.insert(HandlerLane::User, id, options, handler);
    }

    pub(crate) fn remove_focus(&mut self, id: ElementId) {
        self.focus.remove(HandlerLane::User, id);
    }

    pub(crate) fn on_pointer(
        &mut self,
        kind: PointerEventKind,
        id: ElementId,
        handler: PointerHandler,
    ) {
        self.on_pointer_with_options(kind, id, RoutedHandlerOptions::default(), handler);
    }

    pub(crate) fn on_pointer_with_options(
        &mut self,
        kind: PointerEventKind,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: PointerHandler,
    ) {
        self.pointer_pair_mut(kind)
            .insert(HandlerLane::User, id, options, handler);
    }

    pub(crate) fn remove_pointer(&mut self, kind: PointerEventKind, id: ElementId) {
        self.pointer_pair_mut(kind).remove(HandlerLane::User, id);
    }

    pub(crate) fn remove_target(&mut self, id: ElementId) {
        self.click.remove_target(id);
        self.drag.remove_target(id);
        self.scroll.remove_target(id);
        self.key.remove_target(id);
        self.focus.remove_target(id);
        self.pointer_down.remove_target(id);
        self.pointer_move.remove_target(id);
        self.pointer_up.remove_target(id);
        self.pointer_enter.remove_target(id);
        self.pointer_leave.remove_target(id);
        self.pointer_cancel.remove_target(id);
        if self.press_target == Some(id) {
            self.press_target = None;
        }
        if self.pointer_capture == Some(id) {
            self.pointer_capture = None;
            self.clear_press();
            self.cancel_click();
        }
    }

    pub(crate) fn take_click(
        &mut self,
        lane: HandlerLane,
        id: ElementId,
        handled: bool,
    ) -> Option<HandlerRegistration<ClickHandler>> {
        self.click.take(lane, id, handled)
    }

    pub(crate) fn begin_click(&mut self, lane: HandlerLane, id: ElementId) {
        self.click.begin(lane, id);
    }

    pub(crate) fn end_click(&mut self, lane: HandlerLane, id: ElementId) {
        self.click.end(lane, id);
    }

    pub(crate) fn click_removed(&mut self, lane: HandlerLane, id: ElementId) -> bool {
        self.click.take_removed(lane, id)
    }

    pub(crate) fn insert_click_if_absent(
        &mut self,
        lane: HandlerLane,
        id: ElementId,
        handler: HandlerRegistration<ClickHandler>,
    ) {
        self.click.insert_if_absent(lane, id, handler);
    }

    pub(crate) fn has_click(&self, lane: HandlerLane, id: ElementId) -> bool {
        self.click.has(lane, id)
    }

    pub(crate) fn take_drag(
        &mut self,
        lane: HandlerLane,
        id: ElementId,
        handled: bool,
    ) -> Option<HandlerRegistration<DragHandler>> {
        self.drag.take(lane, id, handled)
    }

    pub(crate) fn begin_drag(&mut self, lane: HandlerLane, id: ElementId) {
        self.drag.begin(lane, id);
    }

    pub(crate) fn end_drag(&mut self, lane: HandlerLane, id: ElementId) {
        self.drag.end(lane, id);
    }

    pub(crate) fn drag_removed(&mut self, lane: HandlerLane, id: ElementId) -> bool {
        self.drag.take_removed(lane, id)
    }

    pub(crate) fn insert_drag_if_absent(
        &mut self,
        lane: HandlerLane,
        id: ElementId,
        handler: HandlerRegistration<DragHandler>,
    ) {
        self.drag.insert_if_absent(lane, id, handler);
    }

    pub(crate) fn has_drag(&self, lane: HandlerLane, id: ElementId) -> bool {
        self.drag.has(lane, id)
    }

    pub(crate) fn take_scroll(
        &mut self,
        lane: HandlerLane,
        id: ElementId,
        handled: bool,
    ) -> Option<HandlerRegistration<ScrollHandler>> {
        self.scroll.take(lane, id, handled)
    }

    pub(crate) fn begin_scroll(&mut self, lane: HandlerLane, id: ElementId) {
        self.scroll.begin(lane, id);
    }

    pub(crate) fn end_scroll(&mut self, lane: HandlerLane, id: ElementId) {
        self.scroll.end(lane, id);
    }

    pub(crate) fn scroll_removed(&mut self, lane: HandlerLane, id: ElementId) -> bool {
        self.scroll.take_removed(lane, id)
    }

    pub(crate) fn insert_scroll_if_absent(
        &mut self,
        lane: HandlerLane,
        id: ElementId,
        handler: HandlerRegistration<ScrollHandler>,
    ) {
        self.scroll.insert_if_absent(lane, id, handler);
    }

    pub(crate) fn has_scroll(&self, lane: HandlerLane, id: ElementId) -> bool {
        self.scroll.has(lane, id)
    }

    pub(crate) fn take_key(
        &mut self,
        lane: HandlerLane,
        id: ElementId,
        handled: bool,
    ) -> Option<HandlerRegistration<KeyHandler>> {
        self.key.take(lane, id, handled)
    }

    pub(crate) fn begin_key(&mut self, lane: HandlerLane, id: ElementId) {
        self.key.begin(lane, id);
    }

    pub(crate) fn end_key(&mut self, lane: HandlerLane, id: ElementId) {
        self.key.end(lane, id);
    }

    pub(crate) fn key_removed(&mut self, lane: HandlerLane, id: ElementId) -> bool {
        self.key.take_removed(lane, id)
    }

    pub(crate) fn insert_key_if_absent(
        &mut self,
        lane: HandlerLane,
        id: ElementId,
        handler: HandlerRegistration<KeyHandler>,
    ) {
        self.key.insert_if_absent(lane, id, handler);
    }

    pub(crate) fn has_key(&self, lane: HandlerLane, id: ElementId) -> bool {
        self.key.has(lane, id)
    }

    pub(crate) fn take_focus(
        &mut self,
        lane: HandlerLane,
        id: ElementId,
        handled: bool,
    ) -> Option<HandlerRegistration<FocusHandler>> {
        self.focus.take(lane, id, handled)
    }

    pub(crate) fn begin_focus(&mut self, lane: HandlerLane, id: ElementId) {
        self.focus.begin(lane, id);
    }

    pub(crate) fn end_focus(&mut self, lane: HandlerLane, id: ElementId) {
        self.focus.end(lane, id);
    }

    pub(crate) fn focus_removed(&mut self, lane: HandlerLane, id: ElementId) -> bool {
        self.focus.take_removed(lane, id)
    }

    pub(crate) fn insert_focus_if_absent(
        &mut self,
        lane: HandlerLane,
        id: ElementId,
        handler: HandlerRegistration<FocusHandler>,
    ) {
        self.focus.insert_if_absent(lane, id, handler);
    }

    pub(crate) fn has_focus(&self, lane: HandlerLane, id: ElementId) -> bool {
        self.focus.has(lane, id)
    }

    pub(crate) fn take_pointer(
        &mut self,
        lane: HandlerLane,
        kind: PointerEventKind,
        id: ElementId,
        handled: bool,
    ) -> Option<HandlerRegistration<PointerHandler>> {
        self.pointer_pair_mut(kind).take(lane, id, handled)
    }

    pub(crate) fn begin_pointer(
        &mut self,
        lane: HandlerLane,
        kind: PointerEventKind,
        id: ElementId,
    ) {
        self.pointer_pair_mut(kind).begin(lane, id);
    }

    pub(crate) fn end_pointer(&mut self, lane: HandlerLane, kind: PointerEventKind, id: ElementId) {
        self.pointer_pair_mut(kind).end(lane, id);
    }

    pub(crate) fn pointer_removed(
        &mut self,
        lane: HandlerLane,
        kind: PointerEventKind,
        id: ElementId,
    ) -> bool {
        self.pointer_pair_mut(kind).take_removed(lane, id)
    }

    pub(crate) fn insert_pointer_if_absent(
        &mut self,
        lane: HandlerLane,
        kind: PointerEventKind,
        id: ElementId,
        handler: HandlerRegistration<PointerHandler>,
    ) {
        self.pointer_pair_mut(kind)
            .insert_if_absent(lane, id, handler);
    }

    pub(crate) fn has_pointer(
        &self,
        lane: HandlerLane,
        kind: PointerEventKind,
        id: ElementId,
    ) -> bool {
        self.pointer_pair(kind).has(lane, id)
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

    fn pointer_pair(&self, kind: PointerEventKind) -> &HandlerPair<PointerHandler> {
        match kind {
            PointerEventKind::Down => &self.pointer_down,
            PointerEventKind::Move => &self.pointer_move,
            PointerEventKind::Up => &self.pointer_up,
            PointerEventKind::Enter => &self.pointer_enter,
            PointerEventKind::Leave => &self.pointer_leave,
            PointerEventKind::Cancel => &self.pointer_cancel,
        }
    }

    fn pointer_pair_mut(&mut self, kind: PointerEventKind) -> &mut HandlerPair<PointerHandler> {
        match kind {
            PointerEventKind::Down => &mut self.pointer_down,
            PointerEventKind::Move => &mut self.pointer_move,
            PointerEventKind::Up => &mut self.pointer_up,
            PointerEventKind::Enter => &mut self.pointer_enter,
            PointerEventKind::Leave => &mut self.pointer_leave,
            PointerEventKind::Cancel => &mut self.pointer_cancel,
        }
    }
}

struct HandlerPair<H> {
    /// Host-registered application handler slot.
    user: HandlerSlot<H>,
    /// Built-in control behavior handler slot.
    control: HandlerSlot<H>,
}

impl<H> HandlerPair<H> {
    fn new() -> Self {
        Self {
            user: HandlerSlot::new(),
            control: HandlerSlot::new(),
        }
    }

    fn insert(
        &mut self,
        lane: HandlerLane,
        id: ElementId,
        options: RoutedHandlerOptions,
        handler: H,
    ) {
        self.slot_mut(lane).insert(id, options, handler);
    }

    fn remove(&mut self, lane: HandlerLane, id: ElementId) {
        self.slot_mut(lane).remove(id);
    }

    fn remove_target(&mut self, id: ElementId) {
        self.user.remove(id);
        self.control.remove(id);
    }

    fn take(
        &mut self,
        lane: HandlerLane,
        id: ElementId,
        handled: bool,
    ) -> Option<HandlerRegistration<H>> {
        self.slot_mut(lane).take(id, handled)
    }

    fn begin(&mut self, lane: HandlerLane, id: ElementId) {
        self.slot_mut(lane).begin(id);
    }

    fn end(&mut self, lane: HandlerLane, id: ElementId) {
        self.slot_mut(lane).end(id);
    }

    fn take_removed(&mut self, lane: HandlerLane, id: ElementId) -> bool {
        self.slot_mut(lane).take_removed(id)
    }

    fn insert_if_absent(
        &mut self,
        lane: HandlerLane,
        id: ElementId,
        handler: HandlerRegistration<H>,
    ) {
        self.slot_mut(lane).insert_if_absent(id, handler);
    }

    fn has(&self, lane: HandlerLane, id: ElementId) -> bool {
        self.slot(lane).has(id)
    }

    fn slot(&self, lane: HandlerLane) -> &HandlerSlot<H> {
        match lane {
            HandlerLane::User => &self.user,
            HandlerLane::Control => &self.control,
        }
    }

    fn slot_mut(&mut self, lane: HandlerLane) -> &mut HandlerSlot<H> {
        match lane {
            HandlerLane::User => &mut self.user,
            HandlerLane::Control => &mut self.control,
        }
    }
}

struct HandlerSlot<H> {
    /// Registered handlers keyed by element id.
    handlers: HashMap<ElementId, HandlerRegistration<H>>,
    /// Tracks removals while a handler is temporarily taken for dispatch.
    dispatch: HandlerDispatchTracker,
}

/// Handler plus dispatch options.
pub(crate) struct HandlerRegistration<H> {
    /// Dispatch options for this handler.
    pub(crate) options: RoutedHandlerOptions,
    /// Callback body.
    pub(crate) handler: H,
}

impl<H> HandlerSlot<H> {
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
