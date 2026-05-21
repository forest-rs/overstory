// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Behavior for the [`ELEMENT_KIND_TEXT`] kind.
//!
//! `present` resolves `Content` / `Foreground` / `FontSize` through the
//! cascade and writes them into a `PlainTextPrimitive` on the element's
//! presentation node. Text shaping (parley) happens later, at lower
//! time, using the box tree's world bounds for sizing — keeping the
//! presentation tree geometry-free.

use alloc::string::String;

use kurbo::Size;
use peniko::Brush;
use understory_box_tree::NodeId;
use understory_presentation::{PresentationStore, TextContent};
use understory_property::DependencyObjectExt;

use crate::behavior::{ElementBehavior, MeasureConstraints, MeasureContext, PresentContext};
use crate::element::{Element, ElementId, ElementKind};

/// Element kind for a text leaf.
pub const ELEMENT_KIND_TEXT: ElementKind = ElementKind::new(1);

/// Stateless text behavior; instantiated as a `'static`.
#[derive(Debug, Default)]
pub struct TextBehavior;

/// `'static` instance registered in [`crate::Ui::new`].
pub(crate) static TEXT_BEHAVIOR: TextBehavior = TextBehavior;

impl ElementBehavior for TextBehavior {
    fn present(
        &self,
        element: &Element,
        ctx: &PresentContext<'_>,
        store: &mut PresentationStore<NodeId, ElementId>,
        node: NodeId,
    ) {
        let cx = ctx.resolve_cx();
        let style = Some((ctx.cascade, ctx.match_state));

        let content: String = cx.get_value(element, ctx.properties.content, style);
        let foreground: Brush = cx.get_value(element, ctx.properties.foreground, style);
        #[expect(clippy::cast_possible_truncation, reason = "font size fits f32")]
        let font_size = cx.get_value(element, ctx.properties.font_size, style) as f32;

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
        let content: String = element.get_local_value(ctx.properties.content).cloned()?;
        if content.is_empty() {
            return Some(Size::ZERO);
        }
        #[expect(clippy::cast_possible_truncation, reason = "font size fits f32")]
        let font_size = element
            .get_local_value(ctx.properties.font_size)
            .copied()
            .unwrap_or(14.0_f64) as f32;
        #[expect(clippy::cast_possible_truncation, reason = "width fits f32")]
        let max_advance = if constraints.available.width.is_finite() {
            Some(constraints.available.width as f32)
        } else {
            None
        };

        let layout =
            ctx.text
                .borrow_mut()
                .shape(&content, font_size, Brush::default(), max_advance);
        let width = f64::from(layout.width());
        let height = f64::from(layout.height());
        Some(Size::new(width, height))
    }
}
