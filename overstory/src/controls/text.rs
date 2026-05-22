// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Text`: leaf widget that displays a shaped string.
//!
//! ```ignore
//! ui.append(parent, Text::new("Loading…").font_size(13.0));
//! ```

use alloc::boxed::Box;
use alloc::string::String;

use kurbo::Size;
use peniko::Brush;
use understory_box_tree::NodeId;
use understory_presentation::{PresentationStore, TextContent};
use understory_style::TypeTag;

use crate::behavior::{MeasureConstraints, MeasureContext, PresentContext};
use crate::element::{Element, ElementId};
use crate::ui::Ui;
use crate::widget::{AppendSpec, Widget};

/// Selector identity for `Text`.
pub const TYPE_TEXT: TypeTag = TypeTag(1);

/// Builder + spec for a `Text` label.
pub struct Text {
    content: String,
    font_size: Option<f64>,
    foreground: Option<Brush>,
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
}

impl AppendSpec for Text {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget(parent, Box::new(TextWidget));
        let content_prop = ui.properties().content;
        let font_size_prop = ui.properties().font_size;
        let fg_prop = ui.properties().foreground;
        // Skip writing `Local` for empty content so a parent's
        // binding (Button's template binds owner.content →
        // label.content) isn't shadowed by `Local`-precedence over
        // `TemplateBinding`.
        if !self.content.is_empty() {
            ui.set_local(id, content_prop, self.content);
        }
        if let Some(size) = self.font_size {
            ui.set_local(id, font_size_prop, size);
        }
        if let Some(brush) = self.foreground {
            ui.set_local(id, fg_prop, brush);
        }
        id
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

    fn present(
        &self,
        element: &Element,
        ctx: &PresentContext<'_>,
        store: &mut PresentationStore<NodeId, ElementId>,
        node: NodeId,
    ) {
        let content: String = ctx.get_value(element, ctx.properties.content);
        let foreground: Brush = ctx.get_value(element, ctx.properties.foreground);
        #[expect(clippy::cast_possible_truncation, reason = "font size fits f32")]
        let font_size = ctx.get_value(element, ctx.properties.font_size) as f32;

        let Some(text) = store.plain_text_mut(node) else {
            return;
        };
        text.content = TextContent::plain(content);
        text.foreground = Some(foreground);
        text.style.font_size = font_size;
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
        #[expect(clippy::cast_possible_truncation, reason = "font size fits f32")]
        let font_size = ctx.get_value(element, ctx.properties.font_size) as f32;
        let max_advance = text_wrap_width(constraints);
        let layout =
            ctx.text
                .borrow_mut()
                .shape(&content, font_size, Brush::default(), max_advance);
        let width = f64::from(layout.width());
        let height = f64::from(layout.height());
        Some(Size::new(width, height))
    }
}

fn text_wrap_width(constraints: MeasureConstraints) -> Option<f32> {
    constraints
        .known
        .width
        .filter(|width| width.is_finite() && *width > 0.0)
        .and_then(crate::text::max_advance_for_logical_width)
}
