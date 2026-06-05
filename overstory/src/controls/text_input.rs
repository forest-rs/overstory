// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `TextInput`: single-line editable text field.
//!
//! Roadmap:
//! - Keep Parley's [`PlainEditor`] as the editing, cursor, selection, and IME
//!   layout core for plain text.
//! - Make the control feel complete through Overstory-owned behavior: platform
//!   keybindings, single-line policy, caret reveal / horizontal scrolling,
//!   selection gestures, focus states, and read-only / disabled semantics.
//! - Integrate host-backed capabilities in `overstory_host` / platform hosts:
//!   clipboard, IME event bridging, accessibility, cursor blink, and platform
//!   modifier conventions.
//! - Revisit rendering so editable text can use the editor's cached layout via
//!   the presentation model instead of reshaping through a `TextWidget` part.

use alloc::{borrow::Cow, boxed::Box, string::String, vec::Vec};

use kurbo::Rect;
use parley::{FontFeatures, FontVariations, PlainEditor, StyleProperty};
use peniko::{Brush, Color};
use understory_presentation::{TextLineHeight, TextStyle};
use understory_style::{PartTag, TypeTag};

use crate::behavior::WidgetEventContext;
use crate::controls::support::f32_from_logical;
use crate::controls::text::{TextWidget, text_style};
use crate::element::{Element, ElementId};
use crate::event_runtime::{ClickEvent, DragEvent, KeyEvent};
use crate::layouts::{Layout, LayoutPlacement, LayoutSize};
use crate::pseudos::{PSEUDO_PLACEHOLDER_SHOWN, PSEUDO_READ_ONLY};
use crate::template::{Template, TemplateChild, TemplateContext, TemplateContract, TemplateSlot};
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, ControlTemplateSpec, ElementConfig, ElementSpec, Widget};

/// Selector identity for `TextInput`.
pub const TYPE_TEXT_INPUT: TypeTag = TypeTag(9);
/// Owner-local part identity for the rendered text.
pub const PART_TEXT_INPUT_TEXT: PartTag = PartTag(1);
/// Owner-local part identity for the caret.
pub const PART_TEXT_INPUT_CARET: PartTag = PartTag(2);
/// Owner-local part identity for the current selection highlight.
pub const PART_TEXT_INPUT_SELECTION: PartTag = PartTag(3);
/// Semantic slot for the rendered text.
pub const SLOT_TEXT_INPUT_TEXT: TemplateSlot =
    TemplateSlot::new("text-input-text").with_part_tag(PART_TEXT_INPUT_TEXT);
/// Semantic slot for the caret.
pub const SLOT_TEXT_INPUT_CARET: TemplateSlot =
    TemplateSlot::new("text-input-caret").with_part_tag(PART_TEXT_INPUT_CARET);
/// Semantic slot for the current selection highlight.
pub const SLOT_TEXT_INPUT_SELECTION: TemplateSlot =
    TemplateSlot::new("text-input-selection").with_part_tag(PART_TEXT_INPUT_SELECTION);

const TEXT_INPUT_WIDTH: f32 = 280.0;
const TEXT_INPUT_HEIGHT: f32 = 40.0;
const TEXT_INPUT_PADDING_X: f32 = 12.0;
const DEFAULT_FONT_SIZE: f32 = 15.0;

/// Builder + spec for a single-line text input.
pub struct TextInput {
    text: String,
    placeholder: Option<String>,
    font_size: Option<f64>,
    read_only: bool,
    element: ElementConfig,
}

impl core::fmt::Debug for TextInput {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TextInput")
            .field("text_len", &self.text.len())
            .field("placeholder", &self.placeholder)
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
    pub fn width(mut self, width: f32) -> Self {
        self.element.set_layout(text_input_layout(width));
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
        ui.expand_default_template(id);
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

fn text_input_layout(width: f32) -> Layout {
    Layout::new()
        .relative()
        .logical_size(width.max(1.0), TEXT_INPUT_HEIGHT)
}

/// Per-element widget for `TextInput`.
pub(crate) struct TextInputWidget {
    placeholder: Option<String>,
    edit: TextInputEditState,
    pointer: TextInputPointerState,
}

impl core::fmt::Debug for TextInputWidget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TextInputWidget")
            .field("placeholder", &self.placeholder)
            .finish_non_exhaustive()
    }
}

impl Widget for TextInputWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_TEXT_INPUT)
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

    fn default_template(&self) -> Option<crate::ControlTemplate> {
        Some(crate::ControlTemplate::from_static(&TEXT_INPUT_TEMPLATE))
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
        self.edit.sync_style(&style);
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
        self.edit.sync_style(&style);
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
        let Some(element) = ctx.element() else {
            return;
        };
        let style = text_style(element, ctx);
        let read_only = read_only_from_ctx(ctx);
        self.edit.sync_style(&style);
        let edit = self.edit.edit_from_key(ctx, event.key, read_only);
        if edit.handled {
            if edit.changed {
                self.commit_content(ctx, input);
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
                ctx.with_plain_editor(&mut self.edit.editor, |driver| {
                    driver.insert_or_replace_selection(value);
                });
                self.commit_content(ctx, input);
            }
            accesskit::Action::SetValue => {
                if read_only_from_ctx(ctx) {
                    return false;
                }
                let Some(accesskit::ActionData::Value(value)) = request.data.as_ref() else {
                    return false;
                };
                ctx.with_plain_editor(&mut self.edit.editor, |driver| {
                    driver.editor.set_text(value);
                    driver.move_to_text_end();
                });
                self.commit_content(ctx, input);
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

fn text_input_text_run_node_id(input: ElementId) -> accesskit::NodeId {
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
    let style = text_style(element, ctx);
    let bounds = ctx.bounds(element.id());
    let y0 = bounds.y0 + f64::from(text_top(style.font_size));
    node.set_bounds(accesskit::Rect {
        x0: bounds.x0 + f64::from(TEXT_INPUT_PADDING_X),
        y0,
        x1: bounds.x1,
        y1: y0 + f64::from(style.font_size * 1.35),
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
    fn commit_content(&self, ui: &mut Ui, input: ElementId) {
        let content = ui.properties().content;
        ui.set_local(input, content, String::from(self.edit.text()));
    }

    fn refresh_parts(&mut self, ui: &mut Ui, input: ElementId) {
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

static TEXT_INPUT_TEMPLATE: TextInputTemplate = TextInputTemplate;

#[derive(Debug, Default)]
struct TextInputTemplate;

impl Template for TextInputTemplate {
    fn instantiate(&self, cx: &mut TemplateContext<'_>) {
        let selection = cx.insert(TemplateChild::surface_slot(SLOT_TEXT_INPUT_SELECTION));
        let (selection_placement, selection_size) =
            editor_rect_parts(Rect::ZERO, DEFAULT_FONT_SIZE);
        cx.set_layout_placement(selection, selection_placement);
        cx.set_layout_size(selection, selection_size);

        // TODO(text): Editable text is still rendered through a `TextWidget`
        // template part, while `PlainEditor` owns editing, caret, and selection
        // geometry. That can shape the same text twice after edits. Keep this
        // conservative until editor-backed rendering can flow through the
        // presentation model, likely as a keyed prepared-text primitive/store.
        let _ = cx.insert(TemplateChild::slot(
            SLOT_TEXT_INPUT_TEXT,
            Box::new(TextWidget),
        ));

        let caret = cx.insert(TemplateChild::surface_slot(SLOT_TEXT_INPUT_CARET));
        let (caret_placement, caret_size) = editor_rect_parts(
            Rect::new(0.0, 0.0, 1.5, f64::from(DEFAULT_FONT_SIZE) * 1.25),
            DEFAULT_FONT_SIZE,
        );
        cx.set_layout_placement(caret, caret_placement);
        cx.set_layout_size(caret, caret_size);
    }
}

#[derive(Debug, Default)]
struct TextInputPointerState {
    dragging: bool,
    dragged: bool,
}

impl TextInputPointerState {
    fn take_dragged(&mut self) -> bool {
        let dragged = self.dragged;
        self.dragging = false;
        self.dragged = false;
        dragged
    }
}

struct TextInputEditState {
    editor: PlainEditor<Brush>,
    style: TextStyle,
}

impl TextInputEditState {
    fn new(text: String, style: TextStyle) -> Self {
        let mut editor = PlainEditor::new(style.font_size);
        apply_editor_style(&mut editor, &style);
        editor.set_text(&text);
        Self { editor, style }
    }

    fn text(&self) -> &str {
        self.editor.raw_text()
    }

    fn sync_style(&mut self, style: &TextStyle) {
        if self.style != *style {
            self.style = style.clone();
            apply_editor_style(&mut self.editor, style);
        }
    }

    fn edit_from_key(
        &mut self,
        ui: &Ui,
        event: &ui_events::keyboard::KeyboardEvent,
        read_only: bool,
    ) -> TextInputEditOutcome {
        use ui_events::keyboard::{Key, NamedKey};

        let action_mod = event.modifiers.ctrl() || event.modifiers.meta();
        let shift = event.modifiers.shift();
        ui.with_plain_editor(&mut self.editor, |driver| match &event.key {
            Key::Character(s) if action_mod && s.eq_ignore_ascii_case("a") => {
                driver.select_all();
                TextInputEditOutcome::handled()
            }
            Key::Character(_) if action_mod => TextInputEditOutcome::ignored(),
            Key::Character(s) if !event.is_composing && editable_character(s) => {
                if read_only {
                    TextInputEditOutcome::handled()
                } else {
                    driver.insert_or_replace_selection(s);
                    TextInputEditOutcome::changed()
                }
            }
            Key::Named(NamedKey::Backspace) if action_mod => {
                if read_only {
                    TextInputEditOutcome::handled()
                } else {
                    driver.backdelete_word();
                    TextInputEditOutcome::changed()
                }
            }
            Key::Named(NamedKey::Backspace) => {
                if read_only {
                    TextInputEditOutcome::handled()
                } else {
                    driver.backdelete();
                    TextInputEditOutcome::changed()
                }
            }
            Key::Named(NamedKey::Delete) if action_mod => {
                if read_only {
                    TextInputEditOutcome::handled()
                } else {
                    driver.delete_word();
                    TextInputEditOutcome::changed()
                }
            }
            Key::Named(NamedKey::Delete) => {
                if read_only {
                    TextInputEditOutcome::handled()
                } else {
                    driver.delete();
                    TextInputEditOutcome::changed()
                }
            }
            Key::Named(NamedKey::ArrowLeft) if action_mod && shift => {
                driver.select_to_text_start();
                TextInputEditOutcome::handled()
            }
            Key::Named(NamedKey::ArrowLeft) if action_mod => {
                driver.move_to_text_start();
                TextInputEditOutcome::handled()
            }
            Key::Named(NamedKey::ArrowLeft) if shift => {
                driver.select_left();
                TextInputEditOutcome::handled()
            }
            Key::Named(NamedKey::ArrowLeft) => {
                driver.move_left();
                TextInputEditOutcome::handled()
            }
            Key::Named(NamedKey::ArrowRight) if action_mod && shift => {
                driver.select_to_text_end();
                TextInputEditOutcome::handled()
            }
            Key::Named(NamedKey::ArrowRight) if action_mod => {
                driver.move_to_text_end();
                TextInputEditOutcome::handled()
            }
            Key::Named(NamedKey::ArrowRight) if shift => {
                driver.select_right();
                TextInputEditOutcome::handled()
            }
            Key::Named(NamedKey::ArrowRight) => {
                driver.move_right();
                TextInputEditOutcome::handled()
            }
            Key::Named(NamedKey::ArrowUp) if shift => {
                driver.select_to_text_start();
                TextInputEditOutcome::handled()
            }
            Key::Named(NamedKey::ArrowUp) => {
                driver.move_to_text_start();
                TextInputEditOutcome::handled()
            }
            Key::Named(NamedKey::ArrowDown) if shift => {
                driver.select_to_text_end();
                TextInputEditOutcome::handled()
            }
            Key::Named(NamedKey::ArrowDown) => {
                driver.move_to_text_end();
                TextInputEditOutcome::handled()
            }
            Key::Named(NamedKey::Home) if shift => {
                driver.select_to_text_start();
                TextInputEditOutcome::handled()
            }
            Key::Named(NamedKey::Home) => {
                driver.move_to_text_start();
                TextInputEditOutcome::handled()
            }
            Key::Named(NamedKey::End) if shift => {
                driver.select_to_text_end();
                TextInputEditOutcome::handled()
            }
            Key::Named(NamedKey::End) => {
                driver.move_to_text_end();
                TextInputEditOutcome::handled()
            }
            Key::Named(NamedKey::Enter) => TextInputEditOutcome::handled(),
            _ => TextInputEditOutcome::ignored(),
        })
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
struct TextInputEditOutcome {
    handled: bool,
    changed: bool,
}

impl TextInputEditOutcome {
    const fn ignored() -> Self {
        Self {
            handled: false,
            changed: false,
        }
    }

    const fn handled() -> Self {
        Self {
            handled: true,
            changed: false,
        }
    }

    const fn changed() -> Self {
        Self {
            handled: true,
            changed: true,
        }
    }
}

fn refresh_text_input_parts(
    ui: &mut Ui,
    input: ElementId,
    text_part: ElementId,
    selection_part: ElementId,
    caret_part: ElementId,
    state: &mut TextInputEditState,
    placeholder: Option<&str>,
) {
    let Some(element) = ui.get(input) else {
        return;
    };
    let style = text_style(element, &*ui);
    state.sync_style(&style);
    let text = state.text();
    let display = if text.is_empty() {
        placeholder.unwrap_or("")
    } else {
        text
    };
    ui.set_local(text_part, ui.properties().content, String::from(display));
    ui.set_layout_placement(
        text_part,
        LayoutPlacement::absolute_at(TEXT_INPUT_PADDING_X, text_top(style.font_size)),
    );
    if text.is_empty() && !display.is_empty() {
        ui.add_pseudo(input, PSEUDO_PLACEHOLDER_SHOWN);
    } else {
        ui.remove_pseudo(input, PSEUDO_PLACEHOLDER_SHOWN);
    }
    let (selection_placement, selection_size) = selection_layout(ui, &mut state.editor, &style);
    ui.set_layout_placement(selection_part, selection_placement);
    ui.set_layout_size(selection_part, selection_size);
    let (caret_placement, caret_size) = caret_layout(ui, &mut state.editor, &style);
    ui.set_layout_placement(caret_part, caret_placement);
    ui.set_layout_size(caret_part, caret_size);
}

fn editable_character(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|ch| !ch.is_control() || ch == ' ')
}

fn move_editor_to_point(
    ui: &Ui,
    input: ElementId,
    position: kurbo::Point,
    style: &TextStyle,
    editor: &mut PlainEditor<Brush>,
) -> usize {
    let (local_x, local_y) = editor_local_point(ui, input, position, style);
    ui.with_plain_editor(editor, |driver| {
        driver.move_to_point(local_x, local_y);
    });
    editor.raw_selection().focus().index()
}

fn editor_local_point(
    ui: &Ui,
    input: ElementId,
    position: kurbo::Point,
    style: &TextStyle,
) -> (f32, f32) {
    let bounds = ui.effective_bounds(input);
    let local_x = (position.x - bounds.x0 - f64::from(TEXT_INPUT_PADDING_X)).max(0.0);
    let local_y = (position.y - bounds.y0 - f64::from(text_top(style.font_size))).max(0.0);
    (f32_from_logical(local_x), f32_from_logical(local_y))
}

fn caret_layout(
    ui: &Ui,
    editor: &mut PlainEditor<Brush>,
    style: &TextStyle,
) -> (LayoutPlacement, LayoutSize) {
    refresh_editor_layout(ui, editor);
    let font_size = f64::from(style.font_size);
    let rect = editor
        .cursor_geometry(1.5)
        .map(rect_from_bounding_box)
        .unwrap_or_else(|| Rect::new(0.0, 0.0, 1.5, font_size * 1.25));
    editor_rect_parts(rect, style.font_size)
}

fn selection_layout(
    ui: &Ui,
    editor: &mut PlainEditor<Brush>,
    style: &TextStyle,
) -> (LayoutPlacement, LayoutSize) {
    refresh_editor_layout(ui, editor);
    let mut selection = None::<Rect>;
    editor.selection_geometry_with(|bounds, _| {
        let rect = rect_from_bounding_box(bounds);
        selection = Some(selection.map_or(rect, |current| current.union(rect)));
    });
    let rect = selection.unwrap_or_else(|| {
        let cursor = editor
            .cursor_geometry(0.0)
            .map(rect_from_bounding_box)
            .unwrap_or(Rect::ZERO);
        Rect::new(cursor.x0, cursor.y0, cursor.x0, cursor.y1)
    });
    editor_rect_parts(rect, style.font_size)
}

fn refresh_editor_layout(ui: &Ui, editor: &mut PlainEditor<Brush>) {
    ui.with_plain_editor(editor, |driver| driver.refresh_layout());
}

fn rect_from_bounding_box(bounds: parley::BoundingBox) -> Rect {
    Rect::new(bounds.x0, bounds.y0, bounds.x1, bounds.y1)
}

fn editor_rect_parts(rect: Rect, font_size: f32) -> (LayoutPlacement, LayoutSize) {
    (
        LayoutPlacement::absolute_at(
            f32_from_logical(f64::from(TEXT_INPUT_PADDING_X) + rect.x0),
            f32_from_logical(f64::from(text_top(font_size)) + rect.y0),
        ),
        LayoutSize::logical(
            f32_from_logical(rect.width()),
            f32_from_logical(rect.height()),
        ),
    )
}

fn apply_editor_style(editor: &mut PlainEditor<Brush>, style: &TextStyle) {
    let styles = editor.edit_styles();
    styles.insert(StyleProperty::FontFamily(style.font_family.clone()));
    styles.insert(StyleProperty::FontSize(style.font_size));
    styles.insert(StyleProperty::FontWidth(style.font_width));
    styles.insert(StyleProperty::FontStyle(style.font_style));
    styles.insert(StyleProperty::FontWeight(style.font_weight));
    styles.insert(StyleProperty::FontVariations(editor_font_variations(style)));
    styles.insert(StyleProperty::FontFeatures(editor_font_features(style)));
    styles.insert(StyleProperty::Locale(style.language));
    styles.insert(StyleProperty::LineHeight(crate::text::parley_line_height(
        style.line_height,
    )));
    styles.insert(StyleProperty::LetterSpacing(style.letter_spacing));
    styles.insert(StyleProperty::WordSpacing(style.word_spacing));
    styles.insert(StyleProperty::WordBreak(style.word_break));
    styles.insert(StyleProperty::OverflowWrap(style.overflow_wrap));
    styles.insert(StyleProperty::TextWrapMode(style.text_wrap_mode));
    styles.insert(StyleProperty::Brush(Brush::Solid(Color::from_rgba8(
        0, 0, 0, 0xff,
    ))));
}

fn initial_text_input_style(font_size: f64) -> TextStyle {
    TextStyle {
        font_size: f32_from_logical(font_size),
        line_height: TextLineHeight::FontSizeRelative(1.2),
        ..TextStyle::default()
    }
}

fn editor_font_variations(style: &TextStyle) -> FontVariations<'static> {
    if style.font_variations.is_empty() {
        FontVariations::empty()
    } else {
        FontVariations::List(Cow::Owned(style.font_variations.to_vec()))
    }
}

fn editor_font_features(style: &TextStyle) -> FontFeatures<'static> {
    if style.font_features.is_empty() {
        FontFeatures::empty()
    } else {
        FontFeatures::List(Cow::Owned(style.font_features.to_vec()))
    }
}

fn text_top(font_size: f32) -> f32 {
    (TEXT_INPUT_HEIGHT - font_size * 1.35) * 0.5
}

/// Sets `IsReadOnly` on `input` and keeps the `:read-only` pseudoclass in sync.
pub fn set_text_input_read_only(ui: &mut Ui, input: ElementId, read_only: bool) {
    ui.set_local(input, ui.properties().is_read_only, read_only);
    if read_only {
        ui.add_pseudo(input, PSEUDO_READ_ONLY);
    } else {
        ui.remove_pseudo(input, PSEUDO_READ_ONLY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accessibility::accesskit_node_id;
    use crate::test_support::{pointer_down, pointer_move, pointer_up};
    use accesskit::{
        Action, ActionData, ActionRequest, Node, NodeId, Role, TextPosition, TextSelection, TreeId,
        TreeUpdate,
    };
    use kurbo::{Point, Size};
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};
    use understory_property::DependencyObjectExt;

    #[test]
    fn text_input_edits_content_from_keyboard() {
        let mut ui = Ui::new();
        let root = ui.root();
        let input = ui.append(root, TextInput::new().placeholder("Search"));
        ui.set_focus(Some(input));

        let key = KeyboardEvent::key_down(Key::Character(String::from("a")), Code::KeyA);
        let result = ui.dispatch_key(&key);
        assert!(result.handled);
        assert_eq!(
            ui.get(input)
                .and_then(|el| el.get_local_value(ui.properties().content))
                .map(String::as_str),
            Some("a")
        );

        let backspace = KeyboardEvent::key_down(Key::Named(NamedKey::Backspace), Code::Backspace);
        let result = ui.dispatch_key(&backspace);
        assert!(result.handled);
        assert_eq!(
            ui.get(input)
                .and_then(|el| el.get_local_value(ui.properties().content))
                .map(String::as_str),
            Some("")
        );
    }

    #[test]
    fn text_input_placeholder_shown_tracks_empty_text() {
        let mut ui = Ui::new();
        let root = ui.root();
        let input = ui.append(root, TextInput::new().placeholder("Search"));

        assert!(
            ui.get(input)
                .map(|el| el.pseudos().contains(&PSEUDO_PLACEHOLDER_SHOWN))
                .unwrap_or(false)
        );

        ui.set_focus(Some(input));
        let key = KeyboardEvent::key_down(Key::Character(String::from("a")), Code::KeyA);
        assert!(ui.dispatch_key(&key).handled);
        assert!(
            !ui.get(input)
                .map(|el| el.pseudos().contains(&PSEUDO_PLACEHOLDER_SHOWN))
                .unwrap_or(false)
        );

        let backspace = KeyboardEvent::key_down(Key::Named(NamedKey::Backspace), Code::Backspace);
        assert!(ui.dispatch_key(&backspace).handled);
        assert!(
            ui.get(input)
                .map(|el| el.pseudos().contains(&PSEUDO_PLACEHOLDER_SHOWN))
                .unwrap_or(false)
        );
    }

    #[test]
    fn text_input_select_all_replaces_text() {
        use ui_events::keyboard::Modifiers;

        for modifiers in [Modifiers::CONTROL, Modifiers::META] {
            let mut ui = Ui::new();
            let root = ui.root();
            let input = ui.append(root, TextInput::new().text("hello"));
            ui.set_focus(Some(input));

            let mut select_all =
                KeyboardEvent::key_down(Key::Character(String::from("a")), Code::KeyA);
            select_all.modifiers = modifiers;
            assert!(ui.dispatch_key(&select_all).handled);

            let replacement =
                KeyboardEvent::key_down(Key::Character(String::from("x")), Code::KeyX);
            assert!(ui.dispatch_key(&replacement).handled);
            assert_eq!(
                ui.get(input)
                    .and_then(|el| el.get_local_value(ui.properties().content))
                    .map(String::as_str),
                Some("x")
            );
        }
    }

    #[test]
    fn text_input_shift_arrows_extend_selection_for_replacement() {
        let mut ui = Ui::new();
        let root = ui.root();
        let input = ui.append(root, TextInput::new().text("hello"));
        ui.set_focus(Some(input));

        let mut left = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowLeft), Code::ArrowLeft);
        left.modifiers = ui_events::keyboard::Modifiers::SHIFT;
        assert!(ui.dispatch_key(&left).handled);
        assert!(ui.dispatch_key(&left).handled);

        let replacement = KeyboardEvent::key_down(Key::Character(String::from("y")), Code::KeyY);
        assert!(ui.dispatch_key(&replacement).handled);
        assert_eq!(
            ui.get(input)
                .and_then(|el| el.get_local_value(ui.properties().content))
                .map(String::as_str),
            Some("hely")
        );
    }

    #[test]
    fn text_input_caret_tracks_shaped_text_width() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::flex().logical_size(360.0, 80.0));
        let input = ui.append(root, TextInput::new().text("George of he Jungle"));
        let text = ui.slot(input, SLOT_TEXT_INPUT_TEXT).unwrap();
        let caret = ui.slot(input, SLOT_TEXT_INPUT_CARET).unwrap();

        ui.compute_layout(Size::new(360.0, 80.0));

        let text_bounds = ui.effective_bounds(text);
        let caret_bounds = ui.effective_bounds(caret);
        assert!(
            (caret_bounds.x0 - text_bounds.x1).abs() <= 1.0,
            "caret should land at shaped text end; text={text_bounds:?}, caret={caret_bounds:?}"
        );
    }

    #[test]
    fn text_input_collapses_selection_geometry_after_typing() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::flex().logical_size(360.0, 80.0));
        let input = ui.append(root, TextInput::new().text("hello"));
        let selection = ui.slot(input, SLOT_TEXT_INPUT_SELECTION).unwrap();
        ui.set_focus(Some(input));

        let mut left = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowLeft), Code::ArrowLeft);
        left.modifiers = ui_events::keyboard::Modifiers::SHIFT;
        assert!(ui.dispatch_key(&left).handled);
        ui.compute_layout(Size::new(360.0, 80.0));
        assert!(ui.effective_bounds(selection).width() > 0.0);

        let replacement = KeyboardEvent::key_down(Key::Character(String::from("y")), Code::KeyY);
        assert!(ui.dispatch_key(&replacement).handled);
        ui.compute_layout(Size::new(360.0, 80.0));
        assert_eq!(ui.effective_bounds(selection).width(), 0.0);
    }

    #[test]
    fn read_only_text_input_allows_selection_without_mutating_content() {
        let mut ui = Ui::new();
        let root = ui.root();
        let input = ui.append(root, TextInput::new().text("locked").read_only(true));
        ui.set_focus(Some(input));

        let key = KeyboardEvent::key_down(Key::Character(String::from("x")), Code::KeyX);
        let result = ui.dispatch_key(&key);

        assert!(result.handled);
        assert_eq!(
            ui.get(input)
                .and_then(|el| el.get_local_value(ui.properties().content))
                .map(String::as_str),
            Some("locked")
        );
        assert!(
            ui.get(input)
                .map(|el| el.pseudos().contains(&PSEUDO_READ_ONLY))
                .unwrap_or(false)
        );
    }

    #[test]
    fn text_input_accessibility_exposes_editable_value_and_placeholder() {
        let mut ui = Ui::new();
        let root = ui.root();
        let input = ui.append(
            root,
            TextInput::new()
                .text("George")
                .placeholder("Character name"),
        );
        ui.compute_layout(Size::new(360.0, 80.0));

        let update = ui.accesskit_tree_update();
        let node = accesskit_node_for(&update, input);

        assert_eq!(node.role(), Role::TextInput);
        assert_eq!(node.value(), Some("George"));
        assert_eq!(node.placeholder(), Some("Character name"));
        assert_eq!(node.label(), None);
        assert!(node.supports_action(Action::ReplaceSelectedText));
        assert!(node.supports_action(Action::SetValue));
        assert!(node.supports_action(Action::SetTextSelection));
        assert!(!node.is_read_only());

        let text_run_id = text_input_text_run_node_id(input);
        assert_eq!(node.children(), &[text_run_id]);
        assert_eq!(
            node.text_selection(),
            Some(&TextSelection {
                anchor: TextPosition {
                    node: text_run_id,
                    character_index: 6
                },
                focus: TextPosition {
                    node: text_run_id,
                    character_index: 6
                }
            })
        );

        let text_run = accesskit_node_for_id(&update, text_run_id);
        assert_eq!(text_run.role(), Role::TextRun);
        assert_eq!(text_run.value(), Some("George"));
        assert_eq!(text_run.character_lengths(), &[1, 1, 1, 1, 1, 1]);
    }

    #[test]
    fn read_only_text_input_accessibility_rejects_mutating_actions() {
        let mut ui = Ui::new();
        let root = ui.root();
        let input = ui.append(root, TextInput::new().text("locked").read_only(true));
        ui.compute_layout(Size::new(360.0, 80.0));

        let update = ui.accesskit_tree_update();
        let node = accesskit_node_for(&update, input);
        assert_eq!(node.value(), Some("locked"));
        assert!(node.is_read_only());
        assert!(node.supports_action(Action::SetTextSelection));
        assert!(!node.supports_action(Action::ReplaceSelectedText));
        assert!(!node.supports_action(Action::SetValue));

        let handled = ui.handle_accesskit_action(&text_action(input, Action::SetValue, "changed"));

        assert!(!handled);
        assert_eq!(
            ui.get(input)
                .and_then(|el| el.get_local_value(ui.properties().content))
                .map(String::as_str),
            Some("locked")
        );
    }

    #[test]
    fn accesskit_set_value_updates_text_input_content_and_parts() {
        let mut ui = Ui::new();
        let root = ui.root();
        let input = ui.append(root, TextInput::new().text("old"));
        let text = ui.slot(input, SLOT_TEXT_INPUT_TEXT).unwrap();
        ui.compute_layout(Size::new(360.0, 80.0));

        let handled = ui.handle_accesskit_action(&text_action(input, Action::SetValue, "new"));

        assert!(handled);
        assert_eq!(
            ui.get(input)
                .and_then(|el| el.get_local_value(ui.properties().content))
                .map(String::as_str),
            Some("new")
        );
        assert_eq!(
            ui.get(text)
                .and_then(|el| el.get_local_value(ui.properties().content))
                .map(String::as_str),
            Some("new")
        );
    }

    #[test]
    fn accesskit_replace_selected_text_uses_plain_editor_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let input = ui.append(root, TextInput::new().text("old"));
        ui.set_focus(Some(input));

        let mut select_all = KeyboardEvent::key_down(Key::Character(String::from("a")), Code::KeyA);
        select_all.modifiers = ui_events::keyboard::Modifiers::CONTROL;
        assert!(ui.dispatch_key(&select_all).handled);

        let handled =
            ui.handle_accesskit_action(&text_action(input, Action::ReplaceSelectedText, "new"));

        assert!(handled);
        assert_eq!(
            ui.get(input)
                .and_then(|el| el.get_local_value(ui.properties().content))
                .map(String::as_str),
            Some("new")
        );
    }

    #[test]
    fn accesskit_set_text_selection_updates_plain_editor_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let input = ui.append(root, TextInput::new().text("old"));
        ui.compute_layout(Size::new(360.0, 80.0));

        let selection = TextSelection {
            anchor: TextPosition {
                node: text_input_text_run_node_id(input),
                character_index: 1,
            },
            focus: TextPosition {
                node: text_input_text_run_node_id(input),
                character_index: 3,
            },
        };
        let selected = ui.handle_accesskit_action(&selection_action(input, selection));
        let replaced =
            ui.handle_accesskit_action(&text_action(input, Action::ReplaceSelectedText, "X"));

        assert!(selected);
        assert!(replaced);
        assert_eq!(
            ui.get(input)
                .and_then(|el| el.get_local_value(ui.properties().content))
                .map(String::as_str),
            Some("oX")
        );
    }

    #[test]
    fn text_input_drag_extends_selection_and_ignores_trailing_click() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::flex().logical_size(360.0, 80.0));
        let input = ui.append(root, TextInput::new().text("drag selection"));
        let selection = ui.slot(input, SLOT_TEXT_INPUT_SELECTION).unwrap();
        ui.compute_layout(Size::new(360.0, 80.0));

        let bounds = ui.effective_bounds(input);
        let press = Point::new(bounds.x0 + 16.0, bounds.center().y);
        let drag = Point::new(bounds.x0 + 96.0, bounds.center().y);
        assert!(ui.dispatch_pointer_event(&pointer_down(press, 1)).changed());
        assert!(
            ui.dispatch_pointer_event(&pointer_move(drag, 2, true))
                .changed()
        );
        assert!(ui.dispatch_pointer_event(&pointer_up(drag, 3)).changed());
        ui.compute_layout(Size::new(360.0, 80.0));

        assert!(ui.effective_bounds(selection).width() > 0.0);
    }

    fn text_action(input: ElementId, action: Action, value: &str) -> ActionRequest {
        ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: accesskit_node_id(input),
            data: Some(ActionData::Value(String::from(value).into_boxed_str())),
        }
    }

    fn selection_action(input: ElementId, selection: TextSelection) -> ActionRequest {
        ActionRequest {
            action: Action::SetTextSelection,
            target_tree: TreeId::ROOT,
            target_node: accesskit_node_id(input),
            data: Some(ActionData::SetTextSelection(selection)),
        }
    }

    fn accesskit_node_for(update: &TreeUpdate, element: ElementId) -> &Node {
        accesskit_node_for_id(update, accesskit_node_id(element))
    }

    fn accesskit_node_for_id(update: &TreeUpdate, node_id: NodeId) -> &Node {
        update
            .nodes
            .iter()
            .find(|(id, _)| *id == node_id)
            .map(|(_, node)| node)
            .expect("node should exist")
    }
}
