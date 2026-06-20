use crate::layouts::{Layout, LayoutSize};
use crate::template::{TemplateNode, TemplateSpec, TemplateSpecRecipe};

use super::editor::text_top;
use super::ids::{
    DEFAULT_FONT_SIZE, SLOT_TEXT_INPUT_CARET, SLOT_TEXT_INPUT_SELECTION, SLOT_TEXT_INPUT_TEXT,
    TEXT_INPUT_PADDING_X,
};

static TEXT_INPUT_TEMPLATE: TemplateSpecRecipe = TemplateSpecRecipe::new(text_input_template_spec);

pub(crate) fn default_text_input_template() -> crate::ControlTemplate {
    crate::ControlTemplate::from_static(&TEXT_INPUT_TEMPLATE)
}

fn text_input_template_spec() -> TemplateSpec {
    // TODO(text): Editable text is still rendered through a `TextWidget`
    // template part, while `PlainEditor` owns editing, caret, and selection
    // geometry. That can shape the same text twice after edits. Keep this
    // conservative until editor-backed rendering can flow through the
    // presentation model, likely as a keyed prepared-text primitive/store.
    TemplateSpec::from_nodes([
        TemplateNode::surface_slot(SLOT_TEXT_INPUT_SELECTION)
            .with_layout(text_input_selection_layout()),
        TemplateNode::text_slot(SLOT_TEXT_INPUT_TEXT),
        TemplateNode::surface_slot(SLOT_TEXT_INPUT_CARET).with_layout(text_input_caret_layout()),
    ])
}

fn text_input_selection_layout() -> Layout {
    Layout::new()
        .absolute_at(TEXT_INPUT_PADDING_X, text_top(DEFAULT_FONT_SIZE))
        .size(LayoutSize::logical(0.0, 0.0))
}

fn text_input_caret_layout() -> Layout {
    Layout::new()
        .absolute_at(TEXT_INPUT_PADDING_X, text_top(DEFAULT_FONT_SIZE))
        .size(LayoutSize::logical(
            1.5,
            f64::from(DEFAULT_FONT_SIZE) * 1.25,
        ))
}
