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
//! - Add richer content only when the presentation model can represent it
//!   without bypassing templates, layout, or accessibility.

use alloc::boxed::Box;
use alloc::string::String;

use kurbo::Size;
use peniko::Brush;
use understory_box_tree::NodeId;
use understory_presentation::{PresentationStore, TextContent, TextStyle, TextWrapMode};
use understory_property::Property;
use understory_style::{ClassId, TypeTag};

use crate::AccessibilityContext;
use crate::behavior::{MeasureConstraints, MeasureContext, PresentContext, WidgetEventContext};
use crate::element::{Element, ElementId};
use crate::properties::BuiltInProperties;
use crate::ui::Ui;
use crate::widget::{AppendSpec, ElementConfig, ElementSpec, Widget};

/// Selector identity for `Text`.
pub const TYPE_TEXT: TypeTag = TypeTag(1);

/// Builder + spec for a text element.
///
/// Text defaults to single-line label behavior. Use [`Self::wrap`] for body
/// text that should flow inside a chosen layout width.
pub struct Text {
    content: String,
    font_size: Option<f64>,
    foreground: Option<Brush>,
    wrap_mode: Option<TextWrapMode>,
    element: ElementConfig,
}

impl core::fmt::Debug for Text {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Text")
            .field("content", &self.content)
            .finish_non_exhaustive()
    }
}

impl Text {
    /// Starts a new `Text` builder with the given content.
    #[must_use]
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
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

    /// Attaches a class id to the resulting text element.
    ///
    /// Style rules keyed on this class can provide foreground, font size,
    /// wrapping policy, or future text style properties without writing local
    /// values into each label.
    #[must_use]
    pub fn class(mut self, class: ClassId) -> Self {
        self.element.add_class(class);
        self
    }
}

impl AppendSpec for Text {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget(parent, Box::new(TextWidget));
        let content_prop = ui.properties().content;
        let font_size_prop = ui.properties().font_size;
        let fg_prop = ui.properties().foreground;
        let wrap_mode_prop = ui.properties().text_wrap_mode;
        // Skip writing `Local` for empty content so `Text::new("")` can be a
        // placeholder for bound content without an initial local value.
        if !self.content.is_empty() {
            ui.set_local(id, content_prop, self.content);
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

/// Per-element widget for `Text`. Stateless — the content / size /
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
        let content: String = ctx.get_value(element, ctx.properties.content);
        let foreground: Brush = ctx.get_value(element, ctx.properties.foreground);
        let style = text_style(element, ctx);

        let Some(text) = store.plain_text_mut(node) else {
            return;
        };
        text.content = TextContent::plain(content);
        text.foreground = Some(foreground);
        text.style = style;
    }

    fn measure(
        &self,
        element: &Element,
        ctx: &MeasureContext<'_>,
        constraints: MeasureConstraints,
    ) -> Option<Size> {
        let content: String = ctx.get_value(element, ctx.properties.content);
        if content.is_empty() {
            return Some(Size::ZERO);
        }
        let style = text_style(element, ctx);
        let wrap_width = match style.text_wrap_mode {
            TextWrapMode::NoWrap => None,
            TextWrapMode::Wrap => constraints.known.width,
        };
        let layout = ctx.text.borrow_mut().shape(&content, &style, wrap_width);
        let width = f64::from(layout.width());
        let height = f64::from(layout.height());
        Some(Size::new(width, height))
    }
}

/// Template-generated content part that presents its templated owner's content.
///
/// The current presenter handles string content slots such as a button label.
/// It reads `Content` from the template owner, while text style and foreground
/// still resolve on the presenter element so part selectors and inheritance
/// remain useful. Future content forms should grow this widget instead of
/// bringing back template-local copies of owner properties.
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
        let content = content_presenter_text(element, ctx);
        let foreground: Brush = ctx.get_value(element, ctx.properties.foreground);
        let style = text_style(element, ctx);

        let Some(text) = store.plain_text_mut(node) else {
            return;
        };
        text.content = TextContent::plain(content);
        text.foreground = Some(foreground);
        text.style = style;
    }

    fn measure(
        &self,
        element: &Element,
        ctx: &MeasureContext<'_>,
        constraints: MeasureConstraints,
    ) -> Option<Size> {
        let content = content_presenter_text(element, ctx);
        if content.is_empty() {
            return Some(Size::ZERO);
        }
        let style = text_style(element, ctx);
        let wrap_width = match style.text_wrap_mode {
            TextWrapMode::NoWrap => None,
            TextWrapMode::Wrap => constraints.known.width,
        };
        let layout = ctx.text.borrow_mut().shape(&content, &style, wrap_width);
        let width = f64::from(layout.width());
        let height = f64::from(layout.height());
        Some(Size::new(width, height))
    }
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

// TODO(style): Move this out of text once another subsystem needs the same
// resolved-property source abstraction.
pub(crate) trait ResolvedPropertySource {
    fn properties(&self) -> &BuiltInProperties;

    /// Returns a live element by id.
    fn element(&self, id: ElementId) -> Option<&Element>;

    /// Resolves a typed property for `element` through this source's cascade,
    /// match state, theme, inheritance lookup, and local/animation stores.
    fn resolve_value<T>(&self, element: &Element, property: Property<T>) -> T
    where
        T: Clone + 'static;
}

impl ResolvedPropertySource for PresentContext<'_> {
    fn properties(&self) -> &BuiltInProperties {
        self.properties
    }

    fn element(&self, id: ElementId) -> Option<&Element> {
        PresentContext::element(self, id)
    }

    fn resolve_value<T>(&self, element: &Element, property: Property<T>) -> T
    where
        T: Clone + 'static,
    {
        PresentContext::get_value(self, element, property)
    }
}

impl ResolvedPropertySource for MeasureContext<'_> {
    fn properties(&self) -> &BuiltInProperties {
        self.properties
    }

    fn element(&self, id: ElementId) -> Option<&Element> {
        MeasureContext::element(self, id)
    }

    fn resolve_value<T>(&self, element: &Element, property: Property<T>) -> T
    where
        T: Clone + 'static,
    {
        MeasureContext::get_value(self, element, property)
    }
}

impl ResolvedPropertySource for WidgetEventContext<'_> {
    fn properties(&self) -> &BuiltInProperties {
        self.ui().properties()
    }

    fn element(&self, id: ElementId) -> Option<&Element> {
        self.ui().get(id)
    }

    fn resolve_value<T>(&self, element: &Element, property: Property<T>) -> T
    where
        T: Clone + 'static,
    {
        self.ui().resolve_value_for(element, property)
    }
}

impl ResolvedPropertySource for AccessibilityContext<'_> {
    fn properties(&self) -> &BuiltInProperties {
        self.properties()
    }

    fn element(&self, id: ElementId) -> Option<&Element> {
        AccessibilityContext::element(self, id)
    }

    fn resolve_value<T>(&self, element: &Element, property: Property<T>) -> T
    where
        T: Clone + 'static,
    {
        self.get_value(element, property)
    }
}

impl ResolvedPropertySource for Ui {
    fn properties(&self) -> &BuiltInProperties {
        self.properties()
    }

    fn element(&self, id: ElementId) -> Option<&Element> {
        self.get(id)
    }

    fn resolve_value<T>(&self, element: &Element, property: Property<T>) -> T
    where
        T: Clone + 'static,
    {
        self.resolve_value_for(element, property)
    }
}

fn text_style_from_source(element: &Element, source: &impl ResolvedPropertySource) -> TextStyle {
    let properties = source.properties();
    let mut style = TextStyle::default();
    #[expect(clippy::cast_possible_truncation, reason = "font size fits f32")]
    {
        style.font_size = source.resolve_value::<f64>(element, properties.font_size) as f32;
    }
    style.font_family = source.resolve_value(element, properties.font_family);
    style.font_width = source.resolve_value(element, properties.font_width);
    style.font_weight = source.resolve_value(element, properties.font_weight);
    style.font_style = source.resolve_value(element, properties.font_style);
    style.font_variations = source.resolve_value(element, properties.font_variations);
    style.font_features = source.resolve_value(element, properties.font_features);
    style.language = source.resolve_value(element, properties.text_language);
    style.line_height = source.resolve_value(element, properties.line_height);
    style.letter_spacing = source.resolve_value(element, properties.letter_spacing);
    style.word_spacing = source.resolve_value(element, properties.word_spacing);
    style.word_break = source.resolve_value(element, properties.word_break);
    style.overflow_wrap = source.resolve_value(element, properties.overflow_wrap);
    style.text_wrap_mode = source.resolve_value(element, properties.text_wrap_mode);
    style
}
