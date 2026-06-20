use understory_style::{PartTag, TypeTag};

use crate::template::TemplateSlot;

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

pub(super) const TEXT_INPUT_WIDTH: f64 = 280.0;
pub(super) const TEXT_INPUT_HEIGHT: f64 = 40.0;
pub(super) const TEXT_INPUT_PADDING_X: f64 = 12.0;
pub(super) const DEFAULT_FONT_SIZE: f32 = 15.0;
