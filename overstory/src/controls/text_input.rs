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

use alloc::{boxed::Box, string::String};

use kurbo::Rect;
use parley::{FontFamily, FontWeight, LineHeight, PlainEditor, StyleProperty};
use peniko::{Brush, Color};
use understory_style::{PartTag, TypeTag};

use crate::Phase;
use crate::controls::support::{f32_from_logical, read_local_or};
use crate::controls::text::TextWidget;
use crate::element::ElementId;
use crate::event_runtime::{ClickEvent, DragEvent, KeyEvent};
use crate::layouts::Layout;
use crate::pseudos::{PSEUDO_PLACEHOLDER_SHOWN, PSEUDO_READ_ONLY};
use crate::template::{Template, TemplateChild, TemplateContext, TemplateSlot};
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, SurfacePartWidget, TemplateOverrideWidget, Widget};

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
const DEFAULT_FONT_SIZE: f64 = 15.0;

/// Builder + spec for a single-line text input.
pub struct TextInput {
    text: String,
    placeholder: Option<String>,
    width: f32,
    font_size: Option<f64>,
    read_only: bool,
    state: ControlState,
    template: Option<&'static dyn Template>,
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
        Self {
            text: String::new(),
            placeholder: None,
            width: TEXT_INPUT_WIDTH,
            font_size: None,
            read_only: false,
            state: ControlState::interactive(),
            template: None,
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
        self.width = width.max(1.0);
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
        self.state = self.state.focusable(focusable);
        self
    }

    /// Sets whether the input starts enabled.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.state = self.state.enabled(enabled);
        self
    }

    /// Convenience for [`Self::enabled`] with `false`.
    #[must_use]
    pub fn disabled(self) -> Self {
        self.enabled(false)
    }

    /// Overrides the visual template used when this input is inserted.
    ///
    /// Custom templates should create [`SLOT_TEXT_INPUT_TEXT`] and
    /// [`SLOT_TEXT_INPUT_SELECTION`] / [`SLOT_TEXT_INPUT_CARET`] when they
    /// want built-in editing behavior to synchronize visible text, selection,
    /// and caret placement.
    #[must_use]
    pub fn template(mut self, template: &'static dyn Template) -> Self {
        self.template = Some(template);
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
            width,
            font_size,
            read_only,
            state,
            template,
        } = self;
        let mut edit =
            TextInputEditState::new(text.clone(), font_size.unwrap_or(DEFAULT_FONT_SIZE));
        ui.with_plain_editor(&mut edit.editor, |driver| driver.move_to_text_end());
        let input_widget = TextInputWidget {
            placeholder,
            edit,
            pointer: TextInputPointerState::default(),
        };
        let widget: Box<dyn Widget> = match template {
            Some(template) => Box::new(TemplateOverrideWidget::new(input_widget, template)),
            None => Box::new(input_widget),
        };
        let id = ui.append_widget_deferred_template(parent, widget);
        let content_prop = ui.properties().content;
        let font_size_prop = ui.properties().font_size;
        let layout_prop = ui.properties().layout;
        ui.set_local(id, content_prop, text);
        if let Some(font_size) = font_size {
            ui.set_local(id, font_size_prop, font_size);
        }
        ui.set_local(
            id,
            layout_prop,
            Layout::new()
                .relative()
                .logical_size(width, TEXT_INPUT_HEIGHT),
        );
        ui.set_control_state(id, state);
        set_text_input_read_only(ui, id, read_only);
        ui.expand_default_template(id);
        ui.with_widget_mut(id, |widget, ui| widget.template_expanded(id, ui));
        id
    }
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

    fn default_template(&self) -> Option<&'static dyn Template> {
        Some(&TEXT_INPUT_TEMPLATE)
    }

    fn template_expanded(&mut self, input: ElementId, ui: &mut Ui) {
        self.refresh_parts(ui, input);
    }

    fn handle_click(&mut self, input: ElementId, ui: &mut Ui, event: ClickEvent<'_>) {
        if !matches!(event.phase, Phase::Target | Phase::Bubble) {
            return;
        }
        if !ui.is_enabled(input) {
            return;
        }
        if self.pointer.take_dragged() {
            event.mark_handled();
            return;
        }
        let font_size = read_local_or(ui, input, ui.properties().font_size, DEFAULT_FONT_SIZE);
        self.edit.sync_style(font_size);
        move_editor_to_point(ui, input, event.position, font_size, &mut self.edit.editor);
        self.refresh_parts(ui, input);
        event.mark_handled();
    }

    fn handle_drag(&mut self, input: ElementId, ui: &mut Ui, event: DragEvent<'_>) {
        if !matches!(event.phase, Phase::Target | Phase::Bubble) {
            return;
        }
        if !ui.is_enabled(input) {
            return;
        }
        let font_size = read_local_or(ui, input, ui.properties().font_size, DEFAULT_FONT_SIZE);
        self.edit.sync_style(font_size);
        if !self.pointer.dragging {
            let (x, y) = editor_local_point(ui, input, event.press_position, font_size);
            ui.with_plain_editor(&mut self.edit.editor, |driver| {
                driver.move_to_point(x, y);
            });
            self.pointer.dragging = true;
        }
        self.pointer.dragged = true;
        let (x, y) = editor_local_point(ui, input, event.position, font_size);
        ui.with_plain_editor(&mut self.edit.editor, |driver| {
            driver.extend_selection_to_point(x, y);
        });
        self.refresh_parts(ui, input);
        event.mark_handled();
    }

    fn handle_key(&mut self, input: ElementId, ui: &mut Ui, event: KeyEvent<'_>) {
        use ui_events::keyboard::KeyState;
        if !matches!(event.phase, Phase::Target) {
            return;
        }
        if event.key.state != KeyState::Down {
            return;
        }
        if !ui.is_enabled(input) {
            return;
        }
        let font_size = read_local_or(ui, input, ui.properties().font_size, DEFAULT_FONT_SIZE);
        let read_only = read_local_or(ui, input, ui.properties().is_read_only, false);
        self.edit.sync_style(font_size);
        let edit = self.edit.edit_from_key(ui, event.key, read_only);
        if edit.handled {
            if edit.changed {
                ui.set_local(
                    input,
                    ui.properties().content,
                    String::from(self.edit.text()),
                );
            }
            self.refresh_parts(ui, input);
            event.mark_handled();
        }
    }
}

impl TextInputWidget {
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
        let selection = cx.insert(TemplateChild::slot(
            SLOT_TEXT_INPUT_SELECTION,
            Box::new(SurfacePartWidget),
        ));
        cx.set_local(
            selection,
            cx.properties().layout,
            editor_rect_layout(Rect::ZERO, DEFAULT_FONT_SIZE),
        );

        // TODO(text): Editable text is still rendered through a `TextWidget`
        // template part, while `PlainEditor` owns editing, caret, and selection
        // geometry. That can shape the same text twice after edits. Keep this
        // conservative until editor-backed rendering can flow through the
        // presentation model, likely as a keyed prepared-text primitive/store.
        let _ = cx.insert(TemplateChild::slot(
            SLOT_TEXT_INPUT_TEXT,
            Box::new(TextWidget),
        ));

        let caret = cx.insert(TemplateChild::slot(
            SLOT_TEXT_INPUT_CARET,
            Box::new(SurfacePartWidget),
        ));
        cx.set_local(
            caret,
            cx.properties().layout,
            editor_rect_layout(
                Rect::new(0.0, 0.0, 1.5, DEFAULT_FONT_SIZE * 1.25),
                DEFAULT_FONT_SIZE,
            ),
        );
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
    font_size: f64,
}

impl TextInputEditState {
    fn new(text: String, font_size: f64) -> Self {
        let mut editor = PlainEditor::new(f32_from_logical(font_size));
        apply_editor_style(&mut editor, font_size);
        editor.set_text(&text);
        Self { editor, font_size }
    }

    fn text(&self) -> &str {
        self.editor.raw_text()
    }

    fn sync_style(&mut self, font_size: f64) {
        if (self.font_size - font_size).abs() > f64::EPSILON {
            self.font_size = font_size;
            apply_editor_style(&mut self.editor, font_size);
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
    let font_size = read_local_or(ui, input, ui.properties().font_size, DEFAULT_FONT_SIZE);
    state.sync_style(font_size);
    let text = state.text();
    let display = if text.is_empty() {
        placeholder.unwrap_or("")
    } else {
        text
    };
    ui.set_local(text_part, ui.properties().content, String::from(display));
    ui.set_local(text_part, ui.properties().font_size, font_size);
    ui.set_local(
        text_part,
        ui.properties().layout,
        Layout::new().absolute_at(TEXT_INPUT_PADDING_X, text_top(font_size)),
    );
    if text.is_empty() && !display.is_empty() {
        ui.add_pseudo(input, PSEUDO_PLACEHOLDER_SHOWN);
    } else {
        ui.remove_pseudo(input, PSEUDO_PLACEHOLDER_SHOWN);
    }
    ui.set_local(
        selection_part,
        ui.properties().layout,
        selection_layout(ui, &mut state.editor, font_size),
    );
    ui.set_local(
        caret_part,
        ui.properties().layout,
        caret_layout(ui, &mut state.editor, font_size),
    );
}

fn editable_character(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|ch| !ch.is_control() || ch == ' ')
}

fn move_editor_to_point(
    ui: &Ui,
    input: ElementId,
    position: kurbo::Point,
    font_size: f64,
    editor: &mut PlainEditor<Brush>,
) -> usize {
    let (local_x, local_y) = editor_local_point(ui, input, position, font_size);
    ui.with_plain_editor(editor, |driver| {
        driver.move_to_point(local_x, local_y);
    });
    editor.raw_selection().focus().index()
}

fn editor_local_point(
    ui: &Ui,
    input: ElementId,
    position: kurbo::Point,
    font_size: f64,
) -> (f32, f32) {
    let bounds = ui.effective_bounds(input);
    let local_x = (position.x - bounds.x0 - f64::from(TEXT_INPUT_PADDING_X)).max(0.0);
    let local_y = (position.y - bounds.y0 - f64::from(text_top(font_size))).max(0.0);
    (f32_from_logical(local_x), f32_from_logical(local_y))
}

fn caret_layout(ui: &Ui, editor: &mut PlainEditor<Brush>, font_size: f64) -> Layout {
    refresh_editor_layout(ui, editor);
    let rect = editor
        .cursor_geometry(1.5)
        .map(rect_from_bounding_box)
        .unwrap_or_else(|| Rect::new(0.0, 0.0, 1.5, font_size * 1.25));
    editor_rect_layout(rect, font_size)
}

fn selection_layout(ui: &Ui, editor: &mut PlainEditor<Brush>, font_size: f64) -> Layout {
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
    editor_rect_layout(rect, font_size)
}

fn refresh_editor_layout(ui: &Ui, editor: &mut PlainEditor<Brush>) {
    ui.with_plain_editor(editor, |driver| driver.refresh_layout());
}

fn rect_from_bounding_box(bounds: parley::BoundingBox) -> Rect {
    Rect::new(bounds.x0, bounds.y0, bounds.x1, bounds.y1)
}

fn editor_rect_layout(rect: Rect, font_size: f64) -> Layout {
    Layout::new()
        .absolute_at(
            f32_from_logical(f64::from(TEXT_INPUT_PADDING_X) + rect.x0),
            f32_from_logical(f64::from(text_top(font_size)) + rect.y0),
        )
        .logical_size(
            f32_from_logical(rect.width()),
            f32_from_logical(rect.height()),
        )
}

fn apply_editor_style(editor: &mut PlainEditor<Brush>, font_size: f64) {
    let styles = editor.edit_styles();
    styles.insert(StyleProperty::FontFamily(FontFamily::Source(
        "system-ui, sans-serif".into(),
    )));
    styles.insert(StyleProperty::FontWeight(FontWeight::NORMAL));
    styles.insert(StyleProperty::FontSize(f32_from_logical(font_size)));
    styles.insert(StyleProperty::LineHeight(LineHeight::FontSizeRelative(1.2)));
    styles.insert(StyleProperty::Brush(Brush::Solid(Color::from_rgba8(
        0, 0, 0, 0xff,
    ))));
}

fn text_top(font_size: f64) -> f32 {
    f32_from_logical((f64::from(TEXT_INPUT_HEIGHT) - font_size * 1.35) * 0.5)
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
    use crate::test_support::{pointer_down, pointer_move, pointer_up};
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
}
