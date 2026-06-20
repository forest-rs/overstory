// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::boxed::Box;

use crate::element::ElementId;
use crate::event_runtime::{
    ClickEvent, DragEvent, FocusEvent, KeyEvent, PointerEventKind, PointerRoutedEvent,
    RoutedHandlerOptions, RoutedHandlerScope, ScrollEvent,
};

use super::super::Ui;

/// Advanced raw routed-event registration for one element.
///
/// This is the escape hatch for app code that needs pointer, key, focus,
/// scroll, click, or drag routing directly. Prefer semantic control callbacks
/// such as `Button::on_activate`, `Slider::on_value_changed`, or
/// `TextInput::on_submit` when the code is responding to control intent rather
/// than low-level input.
pub struct RoutedEvents<'a> {
    ui: &'a mut Ui,
    id: ElementId,
}

impl core::fmt::Debug for RoutedEvents<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RoutedEvents")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl RoutedEvents<'_> {
    /// Registers a key handler on this element.
    pub fn key<F>(self, handler: F)
    where
        F: FnMut(&mut Ui, KeyEvent<'_>) + 'static,
    {
        self.key_with_options(RoutedHandlerOptions::default(), handler);
    }

    /// Registers a key handler with explicit routed handler options.
    pub fn key_with_options<F>(self, options: RoutedHandlerOptions, handler: F)
    where
        F: FnMut(&mut Ui, KeyEvent<'_>) + 'static,
    {
        self.ui
            .events
            .on_key_with_options(self.id, options, Box::new(handler));
    }

    /// Removes the key handler previously registered on this element.
    pub fn remove_key(self) {
        self.ui.events.remove_key(self.id);
    }

    /// Registers a focus handler on this element.
    ///
    /// The handler receives both focus gained and focus lost events; inspect
    /// `event.kind` to distinguish them.
    pub fn focus<F>(self, handler: F)
    where
        F: FnMut(&mut Ui, FocusEvent<'_>) + 'static,
    {
        self.focus_with_options(RoutedHandlerOptions::default(), handler);
    }

    /// Registers a focus handler with explicit routed handler options.
    pub fn focus_with_options<F>(self, options: RoutedHandlerOptions, handler: F)
    where
        F: FnMut(&mut Ui, FocusEvent<'_>) + 'static,
    {
        self.ui
            .events
            .on_focus_with_options(self.id, options, Box::new(handler));
    }

    /// Removes the focus handler previously registered on this element.
    pub fn remove_focus(self) {
        self.ui.events.remove_focus(self.id);
    }

    /// Registers a pointer handler for `kind` on this element.
    pub fn pointer<F>(self, kind: PointerEventKind, handler: F)
    where
        F: FnMut(&mut Ui, PointerRoutedEvent<'_>) + 'static,
    {
        self.pointer_with_options(kind, RoutedHandlerOptions::default(), handler);
    }

    /// Registers a pointer handler with explicit routed handler options.
    pub fn pointer_with_options<F>(
        self,
        kind: PointerEventKind,
        options: RoutedHandlerOptions,
        handler: F,
    ) where
        F: FnMut(&mut Ui, PointerRoutedEvent<'_>) + 'static,
    {
        self.ui
            .events
            .on_pointer_with_options(kind, self.id, options, Box::new(handler));
    }

    /// Removes the pointer handler for `kind` on this element.
    pub fn remove_pointer(self, kind: PointerEventKind) {
        self.ui.events.remove_pointer(kind, self.id);
    }

    /// Registers a click handler on this element.
    ///
    /// The default scope is target-or-bubble: capture is skipped, but parents
    /// still observe clicks from ordinary descendants. Use
    /// [`Self::click_with_options`] for capture-only observation, handled
    /// events, or other explicit routed-event policies.
    pub fn click<F>(self, handler: F)
    where
        F: FnMut(&mut Ui, ClickEvent<'_>) + 'static,
    {
        self.click_with_options(
            RoutedHandlerOptions::new().scope(RoutedHandlerScope::TargetOrBubble),
            handler,
        );
    }

    /// Registers a click handler with explicit routed handler options.
    pub fn click_with_options<F>(self, options: RoutedHandlerOptions, handler: F)
    where
        F: FnMut(&mut Ui, ClickEvent<'_>) + 'static,
    {
        self.ui
            .events
            .on_click_with_options(self.id, options, Box::new(handler));
    }

    /// Removes the click handler previously registered on this element.
    pub fn remove_click(self) {
        self.ui.events.remove_click(self.id);
    }

    /// Registers a drag handler on this element.
    pub fn drag<F>(self, handler: F)
    where
        F: FnMut(&mut Ui, DragEvent<'_>) + 'static,
    {
        self.drag_with_options(RoutedHandlerOptions::default(), handler);
    }

    /// Registers a drag handler with explicit routed handler options.
    pub fn drag_with_options<F>(self, options: RoutedHandlerOptions, handler: F)
    where
        F: FnMut(&mut Ui, DragEvent<'_>) + 'static,
    {
        self.ui
            .events
            .on_drag_with_options(self.id, options, Box::new(handler));
    }

    /// Removes the drag handler previously registered on this element.
    pub fn remove_drag(self) {
        self.ui.events.remove_drag(self.id);
    }

    /// Registers a scroll handler on this element.
    pub fn scroll<F>(self, handler: F)
    where
        F: FnMut(&mut Ui, ScrollEvent<'_>) + 'static,
    {
        self.scroll_with_options(RoutedHandlerOptions::default(), handler);
    }

    /// Registers a scroll handler with explicit routed handler options.
    pub fn scroll_with_options<F>(self, options: RoutedHandlerOptions, handler: F)
    where
        F: FnMut(&mut Ui, ScrollEvent<'_>) + 'static,
    {
        self.ui
            .events
            .on_scroll_with_options(self.id, options, Box::new(handler));
    }

    /// Removes the scroll handler previously registered on this element.
    pub fn remove_scroll(self) {
        self.ui.events.remove_scroll(self.id);
    }
}

impl Ui {
    /// Returns advanced raw routed-event registration for `id`.
    ///
    /// This is for app code that needs direct routed input. Ordinary control
    /// authoring should prefer semantic callbacks on the control builders.
    /// Routed handlers receive a context with `source()`, `current()`,
    /// `phase()`, and semantic-target helpers for cases where raw routing is
    /// the right abstraction.
    #[must_use]
    pub fn events(&mut self, id: ElementId) -> RoutedEvents<'_> {
        RoutedEvents { ui: self, id }
    }
}
