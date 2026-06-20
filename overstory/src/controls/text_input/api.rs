use alloc::string::String;

use crate::element::ElementId;
use crate::ui::Ui;

use super::events::TextInputTextChangeSource;
use super::widget::TextInputWidget;

/// Sets whether `input` is read-only.
pub fn set_text_input_read_only(ui: &mut Ui, input: ElementId, read_only: bool) {
    ui.set_local(input, ui.properties().is_read_only, read_only);
}

/// Returns the current editable text for a text input.
#[must_use]
pub fn text_input_text(ui: &Ui, input: ElementId) -> Option<String> {
    ui.get(input)
        .map(|element| ui.resolve_value_for(element, ui.properties().content))
}

/// Replaces the editable text for a text input.
///
/// This updates both the semantic `Content` property and the `PlainEditor`
/// state owned by the widget, then refreshes the text, selection, and caret
/// template parts. Use this for programmatic edits such as clearing a composer
/// after a send action.
pub fn set_text_input_text(ui: &mut Ui, input: ElementId, text: impl Into<String>) -> bool {
    let text = text.into();
    let mut changed = false;
    ui.with_widget_mut(input, |widget, ui| {
        let Some(widget) = widget.downcast_mut::<TextInputWidget>() else {
            return;
        };
        if widget.edit.text() == text {
            return;
        }
        widget.replace_text(ui, input, text, TextInputTextChangeSource::Programmatic);
        widget.edit_session_start = String::from(widget.edit.text());
        widget.refresh_parts(ui, input);
        changed = true;
    });
    changed
}
