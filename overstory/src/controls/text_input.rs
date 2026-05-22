// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `TextInput`: single-line editable text field.

use alloc::{boxed::Box, rc::Rc, string::String};
use core::cell::RefCell;

use kurbo::Rect;
use parley::{FontFamily, FontWeight, LineHeight, PlainEditor, StyleProperty};
use peniko::{Brush, Color};
use understory_property::DependencyObjectExt;
use understory_style::{ClassId, PartTag, TypeTag};

use crate::Phase;
use crate::controls::text::TextWidget;
use crate::element::ElementId;
use crate::layouts::Layout;
use crate::template::{Template, TemplateContext, TemplateSlot};
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
/// Class applied to the text part while it is showing placeholder text.
pub const CLASS_TEXT_INPUT_PLACEHOLDER: ClassId = ClassId(3);

const TEXT_INPUT_WIDTH: f32 = 280.0;
const TEXT_INPUT_HEIGHT: f32 = 40.0;
const TEXT_INPUT_PADDING_X: f32 = 12.0;
const DEFAULT_FONT_SIZE: f64 = 15.0;

/// Builder + spec for a single-line text input.
pub struct TextInput {
    text: String,
    placeholder: Option<String>,
    width: f32,
    font_size: f64,
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
            font_size: DEFAULT_FONT_SIZE,
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
    #[must_use]
    pub fn font_size(mut self, font_size: f64) -> Self {
        self.font_size = font_size.max(1.0);
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

    /// Convenience for [`Self::enabled(false)`].
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
        let widget: Box<dyn Widget> = match self.template {
            Some(template) => Box::new(TemplateOverrideWidget::new(TextInputWidget, template)),
            None => Box::new(TextInputWidget),
        };
        let id = ui.append_widget_deferred_template(parent, widget);
        let content_prop = ui.properties().content;
        let font_size_prop = ui.properties().font_size;
        let layout_prop = ui.properties().layout;
        ui.set_local(id, content_prop, self.text.clone());
        ui.set_local(id, font_size_prop, self.font_size);
        ui.set_local(
            id,
            layout_prop,
            Layout::new()
                .relative()
                .logical_size(self.width, TEXT_INPUT_HEIGHT),
        );
        ui.set_control_state(id, self.state);
        ui.expand_default_template(id);

        let text_part = ui
            .slot(id, SLOT_TEXT_INPUT_TEXT)
            .expect("text input template should create SLOT_TEXT_INPUT_TEXT");
        let selection_part = ui
            .slot(id, SLOT_TEXT_INPUT_SELECTION)
            .expect("text input template should create SLOT_TEXT_INPUT_SELECTION");
        let caret_part = ui
            .slot(id, SLOT_TEXT_INPUT_CARET)
            .expect("text input template should create SLOT_TEXT_INPUT_CARET");
        let placeholder = self.placeholder;
        let mut state = TextInputEditState::new(self.text, self.font_size);
        ui.with_plain_editor(&mut state.editor, |driver| driver.move_to_text_end());
        refresh_text_input_parts(
            ui,
            id,
            text_part,
            selection_part,
            caret_part,
            &mut state,
            placeholder.as_deref(),
        );
        install_text_input_behavior(
            ui,
            id,
            text_part,
            selection_part,
            caret_part,
            placeholder,
            state,
        );
        id
    }
}

/// Per-element widget for `TextInput`.
#[derive(Debug, Default)]
pub struct TextInputWidget;

impl Widget for TextInputWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_TEXT_INPUT)
    }

    fn default_template(&self) -> Option<&'static dyn Template> {
        Some(&TEXT_INPUT_TEMPLATE)
    }
}

static TEXT_INPUT_TEMPLATE: TextInputTemplate = TextInputTemplate;

#[derive(Debug, Default)]
struct TextInputTemplate;

impl Template for TextInputTemplate {
    fn instantiate(&self, cx: &mut TemplateContext<'_>) {
        let selection = cx.slot_widget(SLOT_TEXT_INPUT_SELECTION, Box::new(SurfacePartWidget));
        cx.set_local(
            selection,
            cx.properties().layout,
            editor_rect_layout(Rect::ZERO, DEFAULT_FONT_SIZE),
        );

        let text = cx.slot_widget(SLOT_TEXT_INPUT_TEXT, Box::new(TextWidget));
        cx.set_local(text, cx.properties().font_size, DEFAULT_FONT_SIZE);

        let caret = cx.slot_widget(SLOT_TEXT_INPUT_CARET, Box::new(SurfacePartWidget));
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

fn install_text_input_behavior(
    ui: &mut Ui,
    input: ElementId,
    text_part: ElementId,
    selection_part: ElementId,
    caret_part: ElementId,
    placeholder: Option<String>,
    state: TextInputEditState,
) {
    let state = Rc::new(RefCell::new(state));
    let click_placeholder = placeholder.clone();
    let click_state = Rc::clone(&state);
    ui.on_click(input, move |ui, event| {
        if !matches!(event.phase, Phase::Target | Phase::Bubble) {
            return;
        }
        if !ui.is_enabled(input) {
            return;
        }
        ui.set_focus(Some(input));
        let font_size = read_or(ui, input, ui.properties().font_size, DEFAULT_FONT_SIZE);
        let mut state = click_state.borrow_mut();
        state.sync_style(font_size);
        move_editor_to_point(ui, input, event.position, font_size, &mut state.editor);
        refresh_text_input_parts(
            ui,
            input,
            text_part,
            selection_part,
            caret_part,
            &mut state,
            click_placeholder.as_deref(),
        );
        event.mark_handled();
    });

    let key_state = Rc::clone(&state);
    ui.on_key(input, move |ui, event| {
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
        let mut state = key_state.borrow_mut();
        let font_size = read_or(ui, input, ui.properties().font_size, DEFAULT_FONT_SIZE);
        state.sync_style(font_size);
        if state.edit_from_key(ui, event.key) {
            ui.set_local(input, ui.properties().content, String::from(state.text()));
            refresh_text_input_parts(
                ui,
                input,
                text_part,
                selection_part,
                caret_part,
                &mut state,
                placeholder.as_deref(),
            );
            event.mark_handled();
        }
    });
}

struct TextInputEditState {
    editor: PlainEditor<Brush>,
    font_size: f64,
}

impl TextInputEditState {
    fn new(text: String, font_size: f64) -> Self {
        let mut editor = PlainEditor::new(f32_from_f64(font_size));
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

    fn edit_from_key(&mut self, ui: &Ui, event: &ui_events::keyboard::KeyboardEvent) -> bool {
        use ui_events::keyboard::{Key, NamedKey};

        let action_mod = event.modifiers.ctrl() || event.modifiers.meta();
        let shift = event.modifiers.shift();
        ui.with_plain_editor(&mut self.editor, |driver| match &event.key {
            Key::Character(s) if action_mod && s.eq_ignore_ascii_case("a") => {
                driver.select_all();
                true
            }
            Key::Character(_) if action_mod => false,
            Key::Character(s) if !event.is_composing && editable_character(s) => {
                driver.insert_or_replace_selection(s);
                true
            }
            Key::Named(NamedKey::Backspace) if action_mod => {
                driver.backdelete_word();
                true
            }
            Key::Named(NamedKey::Backspace) => {
                driver.backdelete();
                true
            }
            Key::Named(NamedKey::Delete) if action_mod => {
                driver.delete_word();
                true
            }
            Key::Named(NamedKey::Delete) => {
                driver.delete();
                true
            }
            Key::Named(NamedKey::ArrowLeft) if action_mod && shift => {
                driver.select_to_text_start();
                true
            }
            Key::Named(NamedKey::ArrowLeft) if action_mod => {
                driver.move_to_text_start();
                true
            }
            Key::Named(NamedKey::ArrowLeft) if shift => {
                driver.select_left();
                true
            }
            Key::Named(NamedKey::ArrowLeft) => {
                driver.move_left();
                true
            }
            Key::Named(NamedKey::ArrowRight) if action_mod && shift => {
                driver.select_to_text_end();
                true
            }
            Key::Named(NamedKey::ArrowRight) if action_mod => {
                driver.move_to_text_end();
                true
            }
            Key::Named(NamedKey::ArrowRight) if shift => {
                driver.select_right();
                true
            }
            Key::Named(NamedKey::ArrowRight) => {
                driver.move_right();
                true
            }
            Key::Named(NamedKey::ArrowUp) if shift => {
                driver.select_to_text_start();
                true
            }
            Key::Named(NamedKey::ArrowUp) => {
                driver.move_to_text_start();
                true
            }
            Key::Named(NamedKey::ArrowDown) if shift => {
                driver.select_to_text_end();
                true
            }
            Key::Named(NamedKey::ArrowDown) => {
                driver.move_to_text_end();
                true
            }
            Key::Named(NamedKey::Home) if shift => {
                driver.select_to_text_start();
                true
            }
            Key::Named(NamedKey::Home) => {
                driver.move_to_text_start();
                true
            }
            Key::Named(NamedKey::End) if shift => {
                driver.select_to_text_end();
                true
            }
            Key::Named(NamedKey::End) => {
                driver.move_to_text_end();
                true
            }
            Key::Named(NamedKey::Enter) => true,
            _ => false,
        })
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
    let font_size = read_or(ui, input, ui.properties().font_size, DEFAULT_FONT_SIZE);
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
        ui.add_class(text_part, CLASS_TEXT_INPUT_PLACEHOLDER);
    } else {
        ui.remove_class(text_part, CLASS_TEXT_INPUT_PLACEHOLDER);
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
    let bounds = ui.effective_bounds(input);
    let local_x = (position.x - bounds.x0 - f64::from(TEXT_INPUT_PADDING_X)).max(0.0);
    let local_y = (position.y - bounds.y0 - f64::from(text_top(font_size))).max(0.0);
    ui.with_plain_editor(editor, |driver| {
        driver.move_to_point(f32_from_f64(local_x), f32_from_f64(local_y));
    });
    editor.raw_selection().focus().index()
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
            f32_from_f64(f64::from(TEXT_INPUT_PADDING_X) + rect.x0),
            f32_from_f64(f64::from(text_top(font_size)) + rect.y0),
        )
        .logical_size(f32_from_f64(rect.width()), f32_from_f64(rect.height()))
}

fn apply_editor_style(editor: &mut PlainEditor<Brush>, font_size: f64) {
    let styles = editor.edit_styles();
    styles.insert(StyleProperty::FontFamily(FontFamily::Source(
        "system-ui, sans-serif".into(),
    )));
    styles.insert(StyleProperty::FontWeight(FontWeight::MEDIUM));
    styles.insert(StyleProperty::FontSize(f32_from_f64(font_size)));
    styles.insert(StyleProperty::LineHeight(LineHeight::FontSizeRelative(1.2)));
    styles.insert(StyleProperty::LetterSpacing(0.4));
    styles.insert(StyleProperty::Brush(Brush::Solid(Color::from_rgba8(
        0, 0, 0, 0xff,
    ))));
}

fn text_top(font_size: f64) -> f32 {
    f32_from_f64((f64::from(TEXT_INPUT_HEIGHT) - font_size * 1.35) * 0.5)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "control-local logical coordinates fit f32"
)]
fn f32_from_f64(value: f64) -> f32 {
    value as f32
}

fn read_or<T: Clone + 'static>(
    ui: &Ui,
    element: ElementId,
    property: understory_property::Property<T>,
    default: T,
) -> T {
    ui.get(element)
        .and_then(|el| el.get_local_value(property).cloned())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

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

        ui.compute_layout(kurbo::Size::new(360.0, 80.0));

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
        ui.compute_layout(kurbo::Size::new(360.0, 80.0));
        assert!(ui.effective_bounds(selection).width() > 0.0);

        let replacement = KeyboardEvent::key_down(Key::Character(String::from("y")), Code::KeyY);
        assert!(ui.dispatch_key(&replacement).handled);
        ui.compute_layout(kurbo::Size::new(360.0, 80.0));
        assert_eq!(ui.effective_bounds(selection).width(), 0.0);
    }
}
