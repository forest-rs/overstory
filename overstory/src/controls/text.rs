// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Text`: leaf widget that displays a shaped string.
//!
//! ```ignore
//! ui.append(parent, Text::new("Loading…").font_size(13.0));
//! ```
//!
//! Roadmap:
//! - Keep `Text` as the simple label / run-of-text primitive, distinct from
//!   editable text and future rich document controls.
//! - Grow style coverage deliberately: spans, weight, style, line height,
//!   alignment, locale, bidi, overflow, ellipsis, and max-lines all need a
//!   coherent layout-style key.
//! - Keep shaping and prepared-layout reuse centralized in the text engine /
//!   presentation path so repeated labels share work across controls.
//! - Add richer text content only when the presentation model can represent it
//!   without bypassing templates, layout, or accessibility.

use alloc::boxed::Box;
use alloc::string::String;

use kurbo::Size;
use peniko::Brush;
use understory_box_tree::NodeId;
use understory_presentation::{
    PresentationStore, TextContent, TextLayout, TextStyle, TextWrapMode,
};
use understory_style::TypeTag;

use crate::AccessibilityContext;
use crate::behavior::{MeasureConstraints, MeasureContext, PresentContext};
use crate::element::{Element, ElementId};
use crate::paragraph_direction::paragraph_base_direction;
use crate::resolved_properties::ResolvedPropertySource;
use crate::style_class::StyleClass;
use crate::text::parley_scalar;
use crate::ui::Ui;
use crate::widget::{AppendSpec, ElementConfig, ElementSpec, Widget};

/// Selector identity for `Text`.
pub const TYPE_TEXT: TypeTag = TypeTag(1);

/// Builder + spec for a text element.
///
/// Text defaults to single-line label behavior. Use [`Self::wrap`] for body
/// text that should flow inside a chosen layout width.
pub struct Text {
    text: String,
    font_size: Option<f64>,
    foreground: Option<Brush>,
    wrap_mode: Option<TextWrapMode>,
    element: ElementConfig,
}

impl core::fmt::Debug for Text {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Text")
            .field("text", &self.text)
            .finish_non_exhaustive()
    }
}

impl Text {
    /// Starts a new `Text` builder with the given rendered text.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            font_size: None,
            foreground: None,
            wrap_mode: None,
            element: ElementConfig::default(),
        }
    }

    /// Overrides the text font size (logical pixels).
    #[must_use]
    pub fn font_size(mut self, font_size: f64) -> Self {
        self.font_size = Some(font_size);
        self
    }

    /// Overrides the text's foreground brush.
    #[must_use]
    pub fn foreground(mut self, brush: impl Into<Brush>) -> Self {
        self.foreground = Some(brush.into());
        self
    }

    /// Sets the text wrapping policy.
    ///
    /// Text defaults to [`TextWrapMode::NoWrap`]. Use [`Self::wrap`] as a
    /// shorthand for ordinary body text that should flow inside its chosen
    /// width.
    #[must_use]
    pub fn wrap_mode(mut self, wrap_mode: TextWrapMode) -> Self {
        self.wrap_mode = Some(wrap_mode);
        self
    }

    /// Enables soft line wrapping for this text element.
    #[must_use]
    pub fn wrap(self) -> Self {
        self.wrap_mode(TextWrapMode::Wrap)
    }

    /// Attaches a style class to the resulting text element.
    ///
    /// Style rules keyed on this class can provide foreground, font size,
    /// wrapping policy, or future text style properties without writing local
    /// values into each label.
    #[must_use]
    pub fn class(mut self, class: impl Into<StyleClass>) -> Self {
        self.element.add_class(class);
        self
    }
}

impl AppendSpec for Text {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget(parent, Box::new(TextWidget));
        let text_prop = ui.properties().text;
        let font_size_prop = ui.properties().font_size;
        let fg_prop = ui.properties().foreground;
        let wrap_mode_prop = ui.properties().text_wrap_mode;
        // Skip writing `Local` for empty text so `Text::new("")` can be a
        // placeholder for bound text without an initial local value.
        if !self.text.is_empty() {
            ui.set_local(id, text_prop, self.text);
        }
        if let Some(size) = self.font_size {
            ui.set_local(id, font_size_prop, size);
        }
        if let Some(brush) = self.foreground {
            ui.set_local(id, fg_prop, brush);
        }
        if let Some(wrap_mode) = self.wrap_mode {
            ui.set_local(id, wrap_mode_prop, wrap_mode);
        }
        self.element.apply_to(ui, id);
        id
    }
}

impl ElementSpec for Text {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

/// Per-element widget for `Text`. Stateless — the text / size /
/// foreground all live in the property store.
#[derive(Debug, Default)]
pub struct TextWidget;

impl Widget for TextWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_TEXT)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::Label)
    }

    fn present(
        &self,
        element: &Element,
        ctx: &PresentContext<'_>,
        store: &mut PresentationStore<NodeId, ElementId>,
        node: NodeId,
    ) {
        let rendered_text: String = ctx.get_value(element, ctx.properties.text);
        present_plain_text(element, ctx, store, node, rendered_text);
    }

    fn measure(
        &self,
        element: &Element,
        ctx: &MeasureContext<'_>,
        constraints: MeasureConstraints,
    ) -> Option<Size> {
        let text: String = ctx.get_value(element, ctx.properties.text);
        Some(measure_plain_text(element, ctx, constraints, &text))
    }
}

/// Template-generated content part that presents its templated owner's content.
///
/// The current presenter has two modes. When it has no authored children, it
/// presents the templated owner's `Content` string as a text fallback. When
/// app code adds retained children into the slot, the presenter becomes a
/// transparent host and lets that subtree provide the content. Future data
/// templates and non-text content should grow this widget instead of bringing
/// back template-local copies of owner properties or bindings.
#[derive(Debug, Default)]
pub(crate) struct ContentPresenterWidget;

impl Widget for ContentPresenterWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_TEXT)
    }

    fn present(
        &self,
        element: &Element,
        ctx: &PresentContext<'_>,
        store: &mut PresentationStore<NodeId, ElementId>,
        node: NodeId,
    ) {
        if !element.children().is_empty() {
            if let Some(presentation) = store.node_mut(node) {
                presentation.clear_primitives();
            }
            return;
        }

        let content = content_presenter_text(element, ctx);
        present_plain_text(element, ctx, store, node, content);
    }

    fn measure(
        &self,
        element: &Element,
        ctx: &MeasureContext<'_>,
        constraints: MeasureConstraints,
    ) -> Option<Size> {
        if !element.children().is_empty() {
            return None;
        }

        let content = content_presenter_text(element, ctx);
        Some(measure_plain_text(element, ctx, constraints, &content))
    }

    fn presentation_depends_on_children(&self, _element: &Element) -> bool {
        true
    }

    fn routes_descendants_to_template_owner(&self, _element: &Element) -> bool {
        true
    }
}

fn present_plain_text(
    element: &Element,
    ctx: &PresentContext<'_>,
    store: &mut PresentationStore<NodeId, ElementId>,
    node: NodeId,
    content: String,
) {
    let foreground: Brush = ctx.get_value(element, ctx.properties.foreground);
    let style = text_style(element, ctx);
    let layout = text_layout(element, ctx);

    let Some(text) = store.plain_text_mut(node) else {
        return;
    };
    text.content = TextContent::plain(content);
    text.foreground = Some(foreground);
    text.style = style;
    text.layout = layout;
}

fn measure_plain_text(
    element: &Element,
    ctx: &MeasureContext<'_>,
    constraints: MeasureConstraints,
    content: &str,
) -> Size {
    if content.is_empty() {
        return Size::ZERO;
    }
    let style = text_style(element, ctx);
    let text_layout = text_layout(element, ctx);
    let wrap_width = match style.text_wrap_mode {
        TextWrapMode::NoWrap => None,
        TextWrapMode::Wrap => constraints.known.width,
    };
    let layout = ctx
        .text
        .borrow_mut()
        .shape(content, &style, text_layout, wrap_width);
    Size::new(f64::from(layout.width()), f64::from(layout.height()))
}

fn content_presenter_text(element: &Element, source: &impl ResolvedPropertySource) -> String {
    let content = source.properties().content;
    element
        .templated_parent()
        .and_then(|owner| source.element(owner))
        .map_or_else(
            || source.resolve_value(element, content),
            |owner| source.resolve_value(owner, content),
        )
}

pub(crate) fn text_style(element: &Element, source: &impl ResolvedPropertySource) -> TextStyle {
    text_style_from_source(element, source)
}

pub(crate) fn text_layout(element: &Element, source: &impl ResolvedPropertySource) -> TextLayout {
    let properties = source.properties();
    TextLayout {
        align: source.resolve_value(element, properties.text_align),
        base_direction: paragraph_base_direction(element, source),
        ..TextLayout::default()
    }
}

fn text_style_from_source(element: &Element, source: &impl ResolvedPropertySource) -> TextStyle {
    let properties = source.properties();
    TextStyle {
        font_size: parley_scalar(source.resolve_value::<f64>(element, properties.font_size)),
        font_family: source.resolve_value(element, properties.font_family),
        font_width: source.resolve_value(element, properties.font_width),
        font_weight: source.resolve_value(element, properties.font_weight),
        font_style: source.resolve_value(element, properties.font_style),
        font_variations: source.resolve_value(element, properties.font_variations),
        font_features: source.resolve_value(element, properties.font_features),
        language: source.resolve_value(element, properties.text_language),
        line_height: source.resolve_value(element, properties.line_height),
        letter_spacing: source.resolve_value(element, properties.letter_spacing),
        word_spacing: source.resolve_value(element, properties.word_spacing),
        word_break: source.resolve_value(element, properties.word_break),
        overflow_wrap: source.resolve_value(element, properties.overflow_wrap),
        text_wrap_mode: source.resolve_value(element, properties.text_wrap_mode),
    }
}
