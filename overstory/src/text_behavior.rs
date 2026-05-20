// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Behavior for the [`ELEMENT_KIND_TEXT`] kind: shapes a `Content` string
//! via parley and emits glyph runs through imaging.
//!
//! Phase 1f-4 uses a fresh layout for each measure/paint call (no caching).
//! A per-element layout cache lands in a later phase to amortize shaping
//! across measure → paint within one frame.

use alloc::string::String;

use imaging::{Painter, record::Scene};
use kurbo::{Affine, Size};
use peniko::Brush;
use understory_property::DependencyObjectExt;

use crate::behavior::{ElementBehavior, MeasureConstraints, MeasureContext, PaintContext};
use crate::element::{Element, ElementKind};
use crate::text::render_text;

/// Element kind for a text leaf. Reads `Content`, `Foreground`, `FontSize`
/// from properties (and the style cascade); shapes via parley; emits glyph
/// runs via imaging.
pub const ELEMENT_KIND_TEXT: ElementKind = ElementKind::new(1);

/// Stateless text behavior; instantiated as a `'static`.
#[derive(Debug, Default)]
pub struct TextBehavior;

/// `'static` instance registered in [`crate::Ui::new`].
pub(crate) static TEXT_BEHAVIOR: TextBehavior = TextBehavior;

impl ElementBehavior for TextBehavior {
    fn paint(&self, element: &Element, ctx: &PaintContext<'_>, scene: &mut Scene) {
        let cx = ctx.resolve_cx();
        let style = Some((ctx.cascade, ctx.match_state));

        let content: String = cx.get_value(element, ctx.properties.content, style);
        if content.is_empty() {
            return;
        }
        let foreground: Brush = cx.get_value(element, ctx.properties.foreground, style);
        #[expect(clippy::cast_possible_truncation, reason = "font size fits f32")]
        let font_size = cx.get_value(element, ctx.properties.font_size, style) as f32;

        let bounds = cx.get_value(element, ctx.properties.bounds, style);

        // Shape without a wrap constraint at paint time. The element's bounds
        // were sized by the layout pass from the same measure that runs
        // here, so the natural unwrapped layout fits. Feeding `bounds.width`
        // back as `max_advance` reintroduces sub-pixel rounding wobble that
        // can flip wrap decisions and break "Click me" into "Click" / "me".
        let layout = ctx
            .text
            .borrow_mut()
            .shape(&content, font_size, foreground, None);

        let mut painter = Painter::new(scene);
        let origin = bounds.origin();
        render_text(
            &mut painter,
            Affine::translate((origin.x, origin.y)),
            &layout,
        );
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

        // We don't care about brush at measure time; use the default.
        let layout =
            ctx.text
                .borrow_mut()
                .shape(&content, font_size, Brush::default(), max_advance);
        let width = f64::from(layout.width());
        let height = f64::from(layout.height());
        Some(Size::new(width, height))
    }
}
