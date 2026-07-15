use alloc::{boxed::Box, rc::Rc, string::String};
use core::cell::RefCell;

use crate::controls::support::{CallbackSlot, EventHandler};
use crate::element::ElementId;
use crate::ui::Ui;

pub(super) type TextInputSubmitHandler =
    Box<dyn FnMut(&mut Ui, TextInputSubmitEvent) -> TextInputSubmitAction>;

#[derive(Clone)]
pub(super) struct TextInputSubmitSlot {
    handler: Rc<RefCell<Option<TextInputSubmitHandler>>>,
}

impl TextInputSubmitSlot {
    pub(super) fn new(handler: Option<TextInputSubmitHandler>) -> Self {
        Self {
            handler: Rc::new(RefCell::new(handler)),
        }
    }

    pub(super) fn has_handler(&self) -> bool {
        self.handler.borrow().is_some()
    }

    pub(super) fn dispatch(&self, ui: &mut Ui, event: TextInputSubmitEvent) {
        let slot = self.clone();
        ui.enqueue_semantic_callback(event.input, move |ui| slot.dispatch_now(ui, event));
    }

    fn dispatch_now(&self, ui: &mut Ui, event: TextInputSubmitEvent) {
        let input = event.input;
        let callback = self.handler.borrow_mut().take();
        let action = if let Some(mut callback) = callback {
            let action = callback(ui, event);
            if self.handler.borrow().is_none() {
                *self.handler.borrow_mut() = Some(callback);
            }
            action
        } else {
            TextInputSubmitAction::Keep
        };
        super::api::apply_text_input_submit_action(ui, input, action);
    }
}
pub(super) type TextInputTextChangedHandler = EventHandler<TextInputTextChangedEvent>;
pub(super) type TextInputTextChangedSlot = CallbackSlot<TextInputTextChangedEvent>;
pub(super) type TextInputEditCommittedHandler = EventHandler<TextInputEditCommittedEvent>;
pub(super) type TextInputEditCommittedSlot = CallbackSlot<TextInputEditCommittedEvent>;
pub(super) type TextInputEditRejectedHandler = EventHandler<TextInputEditRejectedEvent>;
pub(super) type TextInputEditRejectedSlot = CallbackSlot<TextInputEditRejectedEvent>;

/// Action returned by a [`TextInput::on_submit`] handler.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum TextInputSubmitAction {
    /// Leave the editable text unchanged.
    #[default]
    Keep,
    /// Clear the editable text after the submit handler returns.
    Clear,
    /// Replace the editable text after the submit handler returns.
    Replace(String),
}

/// Source of a text change reported by [`TextInput::on_text_changed`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextInputTextChangeSource {
    /// Text changed through keyboard editing.
    Keyboard,
    /// Text changed through an accessibility action.
    Accessibility,
    /// Text changed through [`set_text_input_text`].
    Programmatic,
    /// Text changed by the action returned from [`TextInput::on_submit`].
    SubmitAction,
    /// Text changed because the current edit session was rejected.
    Reject,
}

/// Reason an edit session was committed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextInputEditCommitReason {
    /// Focus moved away from the text input.
    FocusLost,
}

/// Reason an edit session was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextInputEditRejectReason {
    /// Escape was pressed while the text input had focus.
    Escape,
}

/// Submit event emitted by a single-line [`TextInput`].
///
/// The event carries a snapshot of the editable text at the moment Enter was
/// pressed. Return a [`TextInputSubmitAction`] from the handler to keep, clear,
/// or replace the editor content after the handler runs.
pub struct TextInputSubmitEvent {
    input: ElementId,
    text: String,
}

impl TextInputSubmitEvent {
    pub(super) fn new(input: ElementId, text: &str) -> Self {
        Self {
            input,
            text: String::from(text),
        }
    }

    /// Returns the text input element that emitted this submit event.
    #[must_use]
    pub const fn input(&self) -> ElementId {
        self.input
    }

    /// Returns the submitted text snapshot.
    #[must_use]
    pub fn text(&self) -> &str {
        self.text.as_str()
    }
}

impl core::fmt::Debug for TextInputSubmitEvent {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TextInputSubmitEvent")
            .field("input", &self.input)
            .field("text_len", &self.text.len())
            .finish()
    }
}

/// Event delivered after a [`TextInput`] changes text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextInputTextChangedEvent {
    /// The text input whose content changed.
    pub input: ElementId,
    /// The editable text before the change.
    pub previous_text: String,
    /// The editable text after the change.
    pub text: String,
    /// Where the change came from.
    pub source: TextInputTextChangeSource,
}

/// Event delivered when a [`TextInput`] commits an edit session without submit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextInputEditCommittedEvent {
    /// The text input whose edit session was committed.
    pub input: ElementId,
    /// The editable text when the edit session started.
    pub previous_text: String,
    /// The text committed by the edit session.
    pub text: String,
    /// Why the edit session committed.
    pub reason: TextInputEditCommitReason,
}

/// Event delivered when a [`TextInput`] rejects an edit session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextInputEditRejectedEvent {
    /// The text input whose edit session was rejected.
    pub input: ElementId,
    /// The text restored by the rejection.
    pub restored_text: String,
    /// The text discarded by the rejection.
    pub rejected_text: String,
    /// Why the edit session was rejected.
    pub reason: TextInputEditRejectReason,
}

impl TextInputTextChangedEvent {
    pub(super) fn new(
        input: ElementId,
        previous_text: String,
        text: String,
        source: TextInputTextChangeSource,
    ) -> Self {
        Self {
            input,
            previous_text,
            text,
            source,
        }
    }
}
