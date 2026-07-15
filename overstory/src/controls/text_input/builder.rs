use alloc::{boxed::Box, string::String};

use crate::controls::support::control_size;
use crate::element::ElementId;
use crate::layouts::Layout;
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, ControlTemplateSpec, ElementConfig, ElementSpec};

use super::api::set_text_input_read_only;
use super::editor::{TextInputEditState, TextInputPointerState, initial_text_input_style};
use super::events::{
    TextInputEditCommittedEvent, TextInputEditCommittedHandler, TextInputEditCommittedSlot,
    TextInputEditRejectedEvent, TextInputEditRejectedHandler, TextInputEditRejectedSlot,
    TextInputSubmitAction, TextInputSubmitEvent, TextInputSubmitHandler, TextInputSubmitSlot,
    TextInputTextChangedEvent, TextInputTextChangedHandler, TextInputTextChangedSlot,
};
use super::ids::{DEFAULT_FONT_SIZE, TEXT_INPUT_HEIGHT, TEXT_INPUT_WIDTH};
use super::widget::TextInputWidget;

/// Builder + spec for a single-line text input.
pub struct TextInput {
    text: String,
    placeholder: Option<String>,
    font_size: Option<f64>,
    read_only: bool,
    on_submit: Option<TextInputSubmitHandler>,
    on_text_changed: Option<TextInputTextChangedHandler>,
    on_edit_committed: Option<TextInputEditCommittedHandler>,
    on_edit_rejected: Option<TextInputEditRejectedHandler>,
    element: ElementConfig,
}

impl core::fmt::Debug for TextInput {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TextInput")
            .field("text_len", &self.text.len())
            .field("placeholder", &self.placeholder)
            .field("has_submit_handler", &self.on_submit.is_some())
            .field("has_text_changed_handler", &self.on_text_changed.is_some())
            .field(
                "has_edit_committed_handler",
                &self.on_edit_committed.is_some(),
            )
            .field(
                "has_edit_rejected_handler",
                &self.on_edit_rejected.is_some(),
            )
            .finish_non_exhaustive()
    }
}

impl TextInput {
    /// Starts a new empty text input.
    #[must_use]
    pub fn new() -> Self {
        let mut element = ElementConfig::default();
        element.set_layout(text_input_layout(TEXT_INPUT_WIDTH));
        element.set_control_state(ControlState::interactive());
        Self {
            text: String::new(),
            placeholder: None,
            font_size: None,
            read_only: false,
            on_submit: None,
            on_text_changed: None,
            on_edit_committed: None,
            on_edit_rejected: None,
            element,
        }
    }

    /// Sets the initial editable text.
    #[must_use]
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    /// Sets placeholder text shown while the editable text is empty.
    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Sets the fixed logical width.
    #[must_use]
    pub fn width(mut self, width: f64) -> Self {
        self.element.set_layout(text_input_layout(width));
        self
    }

    /// Stretches the input to fill its container width, keeping its intrinsic
    /// height, instead of the fixed pixel width set by [`Self::width`].
    #[must_use]
    pub fn fill_width(mut self) -> Self {
        self.element.set_layout(text_input_fill_layout());
        self
    }

    /// Overrides the text font size.
    ///
    /// When unset, the value comes from the active style cascade.
    #[must_use]
    pub fn font_size(mut self, font_size: f64) -> Self {
        self.font_size = Some(font_size.max(1.0));
        self
    }

    /// Sets whether the input allows text mutation.
    ///
    /// Read-only inputs stay focusable and selectable, but keyboard text
    /// insertion and deletion leave their content unchanged.
    #[must_use]
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Runs `handler` when Enter is pressed while the input has keyboard focus.
    ///
    /// This is intentionally a text-input affordance, not a general command or
    /// form-submit system. The handler receives the submitted text snapshot and
    /// returns a [`TextInputSubmitAction`] that decides whether the editor keeps,
    /// clears, or replaces its content after the handler runs.
    #[must_use]
    pub fn on_submit<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, TextInputSubmitEvent) -> TextInputSubmitAction + 'static,
    {
        self.on_submit = Some(Box::new(handler));
        self
    }

    /// Runs `handler` after the editable text changes.
    ///
    /// The event reports both previous and current text and identifies whether
    /// the change came from keyboard editing, accessibility, a programmatic
    /// write, submit post-processing, or edit rejection.
    #[must_use]
    pub fn on_text_changed<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, TextInputTextChangedEvent) + 'static,
    {
        self.on_text_changed = Some(Box::new(handler));
        self
    }

    /// Runs `handler` when the current edit session commits without submit.
    ///
    /// Pressing Enter is reported through [`Self::on_submit`]. This callback is
    /// for session-level acceptance such as moving focus away after editing.
    #[must_use]
    pub fn on_edit_committed<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, TextInputEditCommittedEvent) + 'static,
    {
        self.on_edit_committed = Some(Box::new(handler));
        self
    }

    /// Runs `handler` when the current edit session is rejected.
    ///
    /// Today this happens when Escape restores the text that was present when
    /// the text input gained focus.
    #[must_use]
    pub fn on_edit_rejected<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, TextInputEditRejectedEvent) + 'static,
    {
        self.on_edit_rejected = Some(Box::new(handler));
        self
    }

    /// Sets whether the input participates in focus traversal.
    #[must_use]
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.element
            .update_control_state(|state| state.focusable(focusable));
        self
    }

    /// Sets whether the input starts enabled.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.element
            .update_control_state(|state| state.enabled(enabled));
        self
    }

    /// Convenience for [`Self::enabled`] with `false`.
    #[must_use]
    pub fn disabled(self) -> Self {
        self.enabled(false)
    }

    /// Sets the shared construction-time control state.
    #[must_use]
    pub fn control_state(mut self, state: ControlState) -> Self {
        self.element.set_control_state(state);
        self
    }
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}

impl AppendSpec for TextInput {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let Self {
            text,
            placeholder,
            font_size,
            read_only,
            on_submit,
            on_text_changed,
            on_edit_committed,
            on_edit_rejected,
            element,
        } = self;
        let mut edit = TextInputEditState::new(
            text.clone(),
            initial_text_input_style(font_size.unwrap_or(f64::from(DEFAULT_FONT_SIZE))),
        );
        ui.with_plain_editor(&mut edit.editor, |driver| driver.move_to_text_end());
        let input_widget = TextInputWidget {
            placeholder,
            edit,
            pointer: TextInputPointerState::default(),
            edit_session_start: text.clone(),
            submit: TextInputSubmitSlot::new(on_submit),
            text_changed: TextInputTextChangedSlot::new(on_text_changed),
            edit_committed: TextInputEditCommittedSlot::new(on_edit_committed),
            edit_rejected: TextInputEditRejectedSlot::new(on_edit_rejected),
        };
        let id = ui.append_widget_deferred_template(parent, Box::new(input_widget));
        let content_prop = ui.properties().content;
        let font_size_prop = ui.properties().font_size;
        ui.set_local(id, content_prop, text);
        if let Some(font_size) = font_size {
            ui.set_local(id, font_size_prop, font_size);
        }
        element.apply_to(ui, id);
        set_text_input_read_only(ui, id, read_only);
        ui.expand_resolved_template(id);
        ui.with_widget_mut(id, |widget, ui| widget.template_expanded(id, ui));
        id
    }
}

impl ControlTemplateSpec for TextInput {}

impl ElementSpec for TextInput {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

fn text_input_layout(width: f64) -> Layout {
    Layout::new()
        .relative()
        .logical_size(width.max(1.0), TEXT_INPUT_HEIGHT)
}

fn text_input_fill_layout() -> Layout {
    Layout::new()
        .relative()
        .size(control_size(true, TEXT_INPUT_WIDTH, TEXT_INPUT_HEIGHT))
}
