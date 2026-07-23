use alloc::{string::String, vec::Vec};

use cursor_icon::CursorIcon;
use understory_style::TypeTag;

use crate::behavior::{ArrangeContext, ArrangeResult, WidgetEventContext};
use crate::controls::text::text_style;
use crate::element::{Element, ElementId};
use crate::event_runtime::{ClickEvent, DragEvent, FocusEvent, KeyEvent};
use crate::paragraph_direction::paragraph_base_direction;
use crate::template::TemplateContract;
use crate::ui::Ui;
use crate::widget::Widget;

use super::editor::{
    TextInputEditState, TextInputPointerState, editor_local_point, move_editor_to_point,
    refresh_text_input_parts, sync_text_input_editor, text_top,
};
use super::events::{
    TextInputEditCommitReason, TextInputEditCommittedEvent, TextInputEditCommittedSlot,
    TextInputEditRejectReason, TextInputEditRejectedEvent, TextInputEditRejectedSlot,
    TextInputSubmitEvent, TextInputSubmitSlot, TextInputTextChangeSource,
    TextInputTextChangedEvent, TextInputTextChangedSlot,
};
use super::ids::{
    SLOT_TEXT_INPUT_CARET, SLOT_TEXT_INPUT_SELECTION, SLOT_TEXT_INPUT_TEXT, TEXT_INPUT_PADDING_X,
    TYPE_TEXT_INPUT,
};

/// Per-element widget for `TextInput`.
pub(crate) struct TextInputWidget {
    pub(super) placeholder: Option<String>,
    pub(super) edit: TextInputEditState,
    pub(super) pointer: TextInputPointerState,
    pub(super) edit_session_start: String,
    pub(super) submit: TextInputSubmitSlot,
    pub(super) text_changed: TextInputTextChangedSlot,
    pub(super) edit_committed: TextInputEditCommittedSlot,
    pub(super) edit_rejected: TextInputEditRejectedSlot,
}

impl core::fmt::Debug for TextInputWidget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TextInputWidget")
            .field("placeholder", &self.placeholder)
            .field("has_submit_handler", &self.submit.has_handler())
            .field("has_text_changed_handler", &self.text_changed.has_handler())
            .field(
                "has_edit_committed_handler",
                &self.edit_committed.has_handler(),
            )
            .field(
                "has_edit_rejected_handler",
                &self.edit_rejected.has_handler(),
            )
            .finish_non_exhaustive()
    }
}

impl Widget for TextInputWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_TEXT_INPUT)
    }

    fn cursor_icon(&self, _element: &Element) -> Option<CursorIcon> {
        Some(CursorIcon::Text)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::TextInput)
    }

    fn configure_accessibility(
        &self,
        element: &Element,
        ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        let input = element.id();
        let text_run = text_input_text_run_node_id(input);
        node.clear_label();
        node.set_value(String::from(self.edit.text()));
        node.set_text_selection(accesskit_selection_for_byte_range(
            self.edit.text(),
            text_run,
            self.edit.editor.raw_selection().anchor().index(),
            self.edit.editor.raw_selection().focus().index(),
        ));
        if let Some(placeholder) = self.placeholder.as_deref() {
            node.set_placeholder(placeholder);
        }

        let read_only = ctx.is_read_only(element);
        if read_only {
            node.set_read_only();
        }

        if ctx.is_enabled(input) {
            node.add_action(accesskit::Action::SetTextSelection);
            if !read_only {
                node.add_action(accesskit::Action::ReplaceSelectedText);
                node.add_action(accesskit::Action::SetValue);
            }
        }
    }

    fn append_accessibility_children(
        &self,
        element: &Element,
        ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
        nodes: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        let text_run = text_input_text_run_node_id(element.id());
        node.push_child(text_run);
        nodes.push((
            text_run,
            text_input_text_run_node(self.edit.text(), element, ctx),
        ));
    }

    fn template_contract(&self) -> TemplateContract {
        TemplateContract::new()
            .with_required_slot(SLOT_TEXT_INPUT_TEXT)
            .with_required_slot(SLOT_TEXT_INPUT_CARET)
            .with_optional_slot(SLOT_TEXT_INPUT_SELECTION)
    }

    fn template_expanded(&mut self, input: ElementId, ui: &mut Ui) {
        self.refresh_parts(ui, input);
    }

    fn arrange(&mut self, ctx: &mut ArrangeContext<'_>) -> ArrangeResult {
        let input = ctx.id();
        let Some(element) = ctx.ui().get(input) else {
            return ArrangeResult::UNCHANGED;
        };
        let style = text_style(element, ctx.ui());
        if !sync_text_input_editor(ctx.ui(), input, &mut self.edit, &style) {
            return ArrangeResult::UNCHANGED;
        }
        self.refresh_parts(ctx.ui_mut(), input);
        ArrangeResult::LAYOUT_CHANGED
    }

    fn handle_focus(&mut self, ctx: &mut WidgetEventContext<'_>, event: FocusEvent<'_>) {
        let input = ctx.id();
        if event.focused == Some(input) {
            self.edit_session_start = String::from(self.edit.text());
        }
        if event.previous == Some(input) && event.focused != Some(input) {
            self.commit_edit_session(ctx, input, TextInputEditCommitReason::FocusLost);
        }
    }

    fn handle_click(&mut self, ctx: &mut WidgetEventContext<'_>, event: ClickEvent<'_>) {
        let input = ctx.id();
        if !event.is_semantic_target() {
            return;
        }
        if !ctx.is_enabled(input) {
            return;
        }
        if self.pointer.take_dragged() {
            event.mark_handled();
            return;
        }
        let Some(element) = ctx.element() else {
            return;
        };
        let style = text_style(element, ctx);
        sync_text_input_editor(ctx, input, &mut self.edit, &style);
        move_editor_to_point(ctx, input, event.position, &style, &mut self.edit.editor);
        self.refresh_parts(ctx, input);
        event.mark_handled();
    }

    fn handle_drag(&mut self, ctx: &mut WidgetEventContext<'_>, event: DragEvent<'_>) {
        let input = ctx.id();
        if !event.is_semantic_target() {
            return;
        }
        if !ctx.is_enabled(input) {
            return;
        }
        let Some(element) = ctx.element() else {
            return;
        };
        let style = text_style(element, ctx);
        sync_text_input_editor(ctx, input, &mut self.edit, &style);
        if !self.pointer.dragging {
            let (x, y) = editor_local_point(ctx, input, event.press_position, &style);
            ctx.with_plain_editor(&mut self.edit.editor, |driver| {
                driver.move_to_point(x, y);
            });
            self.pointer.dragging = true;
        }
        self.pointer.dragged = true;
        let (x, y) = editor_local_point(ctx, input, event.position, &style);
        ctx.with_plain_editor(&mut self.edit.editor, |driver| {
            driver.extend_selection_to_point(x, y);
        });
        self.refresh_parts(ctx, input);
        event.mark_handled();
    }

    fn handle_key(&mut self, ctx: &mut WidgetEventContext<'_>, event: KeyEvent<'_>) {
        use ui_events::keyboard::KeyState;
        use ui_events::keyboard::{Key, NamedKey};
        let input = ctx.id();
        if !event.is_target() {
            return;
        }
        if event.key.state != KeyState::Down {
            return;
        }
        if !ctx.is_enabled(input) {
            return;
        }
        if matches!(event.key.key, Key::Named(NamedKey::Enter)) {
            self.submit(ctx, input);
            event.mark_handled();
            return;
        }
        if matches!(event.key.key, Key::Named(NamedKey::Escape)) {
            self.reject_edit_session(ctx, input, TextInputEditRejectReason::Escape);
            event.mark_handled();
            return;
        }
        let Some(element) = ctx.element() else {
            return;
        };
        let style = text_style(element, ctx);
        let read_only = read_only_from_ctx(ctx);
        sync_text_input_editor(ctx, input, &mut self.edit, &style);
        let previous_text = String::from(self.edit.text());
        let edit = self.edit.edit_from_key(ctx, event.key, read_only);
        if edit.handled {
            if edit.changed {
                self.commit_content_changed(
                    ctx,
                    input,
                    previous_text,
                    TextInputTextChangeSource::Keyboard,
                );
            }
            self.refresh_parts(ctx, input);
            event.mark_handled();
        }
    }

    fn handle_accessibility_action(
        &mut self,
        ctx: &mut WidgetEventContext<'_>,
        request: &accesskit::ActionRequest,
    ) -> bool {
        let input = ctx.id();
        if !ctx.is_enabled(input) {
            return false;
        }

        match request.action {
            accesskit::Action::SetTextSelection => {
                let Some(accesskit::ActionData::SetTextSelection(selection)) =
                    request.data.as_ref()
                else {
                    return false;
                };
                let Some((anchor, focus)) =
                    byte_range_from_accesskit_selection(self.edit.text(), input, selection)
                else {
                    return false;
                };
                ctx.with_plain_editor(&mut self.edit.editor, |driver| {
                    driver.select_byte_range(anchor, focus);
                });
            }
            accesskit::Action::ReplaceSelectedText => {
                if read_only_from_ctx(ctx) {
                    return false;
                }
                let Some(accesskit::ActionData::Value(value)) = request.data.as_ref() else {
                    return false;
                };
                let previous_text = String::from(self.edit.text());
                ctx.with_plain_editor(&mut self.edit.editor, |driver| {
                    driver.insert_or_replace_selection(value);
                });
                self.commit_content_changed(
                    ctx,
                    input,
                    previous_text,
                    TextInputTextChangeSource::Accessibility,
                );
            }
            accesskit::Action::SetValue => {
                if read_only_from_ctx(ctx) {
                    return false;
                }
                let Some(accesskit::ActionData::Value(value)) = request.data.as_ref() else {
                    return false;
                };
                let previous_text = String::from(self.edit.text());
                ctx.with_plain_editor(&mut self.edit.editor, |driver| {
                    driver.editor.set_text(value);
                    driver.move_to_text_end();
                });
                self.commit_content_changed(
                    ctx,
                    input,
                    previous_text,
                    TextInputTextChangeSource::Accessibility,
                );
            }
            _ => return false,
        }

        self.refresh_parts(ctx, input);
        true
    }
}

fn read_only_from_ctx(ctx: &WidgetEventContext<'_>) -> bool {
    ctx.element()
        .is_some_and(|element| ctx.get_value(element, ctx.properties().is_read_only))
}

pub(super) fn text_input_text_run_node_id(input: ElementId) -> accesskit::NodeId {
    // Element nodes use generation:slot. The editable text run is a synthetic
    // child node, so set the high generation bit as a local convention; generic
    // actions only dispatch when decoding the node id finds a live element.
    accesskit::NodeId(
        (0x8000_0000_u64 | u64::from(input.generation())) << 32 | u64::from(input.slot()),
    )
}

fn text_input_text_run_node(
    text: &str,
    element: &Element,
    ctx: &crate::AccessibilityContext<'_>,
) -> accesskit::Node {
    let mut node = accesskit::Node::new(accesskit::Role::TextRun);
    if let Some(direction) =
        crate::accessibility::accesskit_text_direction(paragraph_base_direction(element, ctx))
    {
        node.set_text_direction(direction);
    }
    let style = text_style(element, ctx);
    let bounds = ctx.bounds(element.id());
    let y0 = bounds.y0 + text_top(style.font_size);
    node.set_bounds(accesskit::Rect {
        x0: bounds.x0 + TEXT_INPUT_PADDING_X,
        y0,
        x1: bounds.x1,
        y1: y0 + f64::from(style.font_size) * 1.35,
    });
    node.set_value(String::from(text));
    node.set_character_lengths(text_character_lengths(text));
    let word_starts = text_word_starts(text);
    if !word_starts.is_empty() {
        node.set_word_starts(word_starts);
    }
    node
}

fn accesskit_selection_for_byte_range(
    text: &str,
    node: accesskit::NodeId,
    anchor: usize,
    focus: usize,
) -> accesskit::TextSelection {
    accesskit::TextSelection {
        anchor: accesskit::TextPosition {
            node,
            character_index: character_index_for_byte(text, anchor),
        },
        focus: accesskit::TextPosition {
            node,
            character_index: character_index_for_byte(text, focus),
        },
    }
}

fn byte_range_from_accesskit_selection(
    text: &str,
    input: ElementId,
    selection: &accesskit::TextSelection,
) -> Option<(usize, usize)> {
    let text_run = text_input_text_run_node_id(input);
    if selection.anchor.node != text_run || selection.focus.node != text_run {
        return None;
    }
    let anchor = byte_index_for_character_index(text, selection.anchor.character_index)?;
    let focus = byte_index_for_character_index(text, selection.focus.character_index)?;
    Some((anchor, focus))
}

fn character_index_for_byte(text: &str, byte: usize) -> usize {
    let mut byte = byte.min(text.len());
    while !text.is_char_boundary(byte) {
        byte -= 1;
    }
    text[..byte].chars().count()
}

fn byte_index_for_character_index(text: &str, character_index: usize) -> Option<usize> {
    if character_index == text.chars().count() {
        return Some(text.len());
    }
    text.char_indices()
        .nth(character_index)
        .map(|(index, _)| index)
}

fn text_character_lengths(text: &str) -> Vec<u8> {
    text.chars()
        .map(|ch| {
            u8::try_from(ch.len_utf8()).expect("a Unicode scalar value is at most four UTF-8 bytes")
        })
        .collect()
}

fn text_word_starts(text: &str) -> Vec<u8> {
    let mut starts = Vec::new();
    let mut in_word = false;
    for (index, ch) in text.chars().enumerate() {
        if ch.is_whitespace() {
            in_word = false;
        } else if !in_word {
            if let Ok(index) = u8::try_from(index) {
                starts.push(index);
            }
            in_word = true;
        }
    }
    starts
}

impl TextInputWidget {
    fn submit(&mut self, ctx: &mut WidgetEventContext<'_>, input: ElementId) {
        let submit = TextInputSubmitEvent::new(input, self.edit.text());
        self.submit.dispatch(ctx, submit);
    }

    fn commit_content(&self, ui: &mut Ui, input: ElementId) {
        let content = ui.properties().content;
        ui.set_local(input, content, String::from(self.edit.text()));
    }

    fn commit_content_changed(
        &self,
        ui: &mut Ui,
        input: ElementId,
        previous_text: String,
        source: TextInputTextChangeSource,
    ) -> bool {
        let text = String::from(self.edit.text());
        if text == previous_text {
            return false;
        }
        self.commit_content(ui, input);
        self.text_changed.dispatch(
            ui,
            input,
            TextInputTextChangedEvent::new(input, previous_text, text, source),
        );
        true
    }

    pub(super) fn replace_text(
        &mut self,
        ui: &mut Ui,
        input: ElementId,
        text: String,
        source: TextInputTextChangeSource,
    ) -> bool {
        let previous_text = String::from(self.edit.text());
        if previous_text == text {
            return false;
        }
        self.edit.editor.set_text(&text);
        ui.with_plain_editor(&mut self.edit.editor, |driver| {
            driver.move_to_text_end();
        });
        self.commit_content_changed(ui, input, previous_text, source)
    }

    fn commit_edit_session(
        &mut self,
        ui: &mut Ui,
        input: ElementId,
        reason: TextInputEditCommitReason,
    ) {
        if self.edit_session_start == self.edit.text() {
            return;
        }
        let previous_text = self.edit_session_start.clone();
        let text = String::from(self.edit.text());
        self.edit_session_start = text.clone();
        self.edit_committed.dispatch(
            ui,
            input,
            TextInputEditCommittedEvent {
                input,
                previous_text,
                text,
                reason,
            },
        );
    }

    fn reject_edit_session(
        &mut self,
        ui: &mut Ui,
        input: ElementId,
        reason: TextInputEditRejectReason,
    ) {
        let restored_text = self.edit_session_start.clone();
        let rejected_text = String::from(self.edit.text());
        if restored_text == rejected_text {
            return;
        }
        self.replace_text(
            ui,
            input,
            restored_text.clone(),
            TextInputTextChangeSource::Reject,
        );
        self.edit_session_start = restored_text.clone();
        self.edit_rejected.dispatch(
            ui,
            input,
            TextInputEditRejectedEvent {
                input,
                restored_text,
                rejected_text,
                reason,
            },
        );
        self.refresh_parts(ui, input);
    }

    pub(super) fn refresh_parts(&mut self, ui: &mut Ui, input: ElementId) {
        let Some(text_part) = ui.slot(input, SLOT_TEXT_INPUT_TEXT) else {
            return;
        };
        let Some(selection_part) = ui.slot(input, SLOT_TEXT_INPUT_SELECTION) else {
            return;
        };
        let Some(caret_part) = ui.slot(input, SLOT_TEXT_INPUT_CARET) else {
            return;
        };
        refresh_text_input_parts(
            ui,
            input,
            text_part,
            selection_part,
            caret_part,
            &mut self.edit,
            self.placeholder.as_deref(),
        );
    }
}
