use alloc::{borrow::Cow, string::String};

use kurbo::Rect;
use parley::{FontFeatures, FontVariations, PlainEditor, StyleProperty};
use peniko::{Brush, Color};
use understory_presentation::{TextLineHeight, TextStyle};

use crate::controls::text::text_style;
use crate::element::ElementId;
use crate::layouts::{LayoutPlacement, LayoutSize};
use crate::pseudos::PSEUDO_PLACEHOLDER_SHOWN;
use crate::text::parley_scalar;
use crate::ui::Ui;

use super::ids::{TEXT_INPUT_HEIGHT, TEXT_INPUT_PADDING_X};

#[derive(Debug, Default)]
pub(super) struct TextInputPointerState {
    pub(super) dragging: bool,
    pub(super) dragged: bool,
}

impl TextInputPointerState {
    pub(super) fn take_dragged(&mut self) -> bool {
        let dragged = self.dragged;
        self.dragging = false;
        self.dragged = false;
        dragged
    }
}

pub(super) struct TextInputEditState {
    pub(super) editor: PlainEditor<Brush>,
    style: TextStyle,
}

impl TextInputEditState {
    pub(super) fn new(text: String, style: TextStyle) -> Self {
        let mut editor = PlainEditor::new(style.font_size);
        apply_editor_style(&mut editor, &style);
        editor.set_text(&text);
        Self { editor, style }
    }

    pub(super) fn text(&self) -> &str {
        self.editor.raw_text()
    }

    pub(super) fn sync_style(&mut self, style: &TextStyle) {
        if self.style != *style {
            self.style = style.clone();
            apply_editor_style(&mut self.editor, style);
        }
    }

    pub(super) fn edit_from_key(
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
pub(super) struct TextInputEditOutcome {
    pub(super) handled: bool,
    pub(super) changed: bool,
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

pub(super) fn refresh_text_input_parts(
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
    ui.set_local(text_part, ui.properties().text, String::from(display));
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

pub(super) fn move_editor_to_point(
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

pub(super) fn editor_local_point(
    ui: &Ui,
    input: ElementId,
    position: kurbo::Point,
    style: &TextStyle,
) -> (f32, f32) {
    let bounds = ui.effective_bounds(input);
    let local_x = (position.x - bounds.x0 - TEXT_INPUT_PADDING_X).max(0.0);
    let local_y = (position.y - bounds.y0 - text_top(style.font_size)).max(0.0);
    (parley_scalar(local_x), parley_scalar(local_y))
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
            TEXT_INPUT_PADDING_X + rect.x0,
            text_top(font_size) + rect.y0,
        ),
        LayoutSize::logical(rect.width(), rect.height()),
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

pub(super) fn initial_text_input_style(font_size: f64) -> TextStyle {
    TextStyle {
        font_size: parley_scalar(font_size),
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

pub(super) fn text_top(font_size: f32) -> f64 {
    (TEXT_INPUT_HEIGHT - f64::from(font_size) * 1.35) * 0.5
}
