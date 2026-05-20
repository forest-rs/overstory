// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text shaping and rendering via parley + imaging.
//!
//! This module owns the parley `FontContext` + `LayoutContext` that Ui
//! holds, and provides a helper that walks a built parley `Layout` and
//! emits glyph runs through imaging's `Painter`.

use imaging::{PaintSink, Painter, record::Glyph};
use kurbo::Affine;
use parley::{FontContext, Layout, LayoutContext, PositionedLayoutItem};
use peniko::{Brush, Fill};
use smallvec::SmallVec;

/// Owns parley's font and layout context state. One per [`crate::Ui`].
pub struct TextEngine {
    /// Font discovery + source cache.
    pub font_context: FontContext,
    /// Working context for building text layouts.
    pub layout_context: LayoutContext<Brush>,
}

impl core::fmt::Debug for TextEngine {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TextEngine").finish_non_exhaustive()
    }
}

impl Default for TextEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TextEngine {
    /// Constructs a new text engine with default system font discovery.
    #[must_use]
    pub fn new() -> Self {
        Self {
            font_context: FontContext::default(),
            layout_context: LayoutContext::new(),
        }
    }

    /// Shapes `text` with the given font size and brush, applying line
    /// wrapping if `max_advance` is `Some`. Returns the built parley
    /// `Layout` ready for rendering or measuring.
    pub fn shape(
        &mut self,
        text: &str,
        font_size: f32,
        brush: Brush,
        max_advance: Option<f32>,
    ) -> Layout<Brush> {
        let mut builder =
            self.layout_context
                .ranged_builder(&mut self.font_context, text, 1.0, true);
        builder.push_default(parley::StyleProperty::FontSize(font_size));
        builder.push_default(parley::StyleProperty::Brush(brush));
        let mut layout = builder.build(text);
        layout.break_all_lines(max_advance);
        layout.align(
            parley::Alignment::Start,
            parley::AlignmentOptions::default(),
        );
        layout
    }
}

/// Emits a built parley `Layout` as glyph runs through `painter`.
///
/// Underlines / strikethroughs / hinting are omitted for now; they'll
/// come back when richer text styling lands.
pub(crate) fn render_text<S>(
    painter: &mut Painter<'_, S>,
    transform: Affine,
    layout: &Layout<Brush>,
) where
    S: PaintSink + ?Sized,
{
    for line in layout.lines() {
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
            };
            let style = glyph_run.style();
            let mut x = glyph_run.offset();
            let y = glyph_run.baseline();
            let run = glyph_run.run();
            let font = run.font();
            let font_size = run.font_size();
            let coords = run.normalized_coords();
            let brush = &style.brush;
            let glyphs: SmallVec<[Glyph; 16]> = glyph_run
                .glyphs()
                .map(|glyph| {
                    let gx = x + glyph.x;
                    let gy = y + glyph.y;
                    x += glyph.advance;
                    Glyph {
                        id: glyph.id,
                        x: gx,
                        y: gy,
                    }
                })
                .collect();
            painter
                .glyphs(font, brush)
                .transform(transform)
                .font_size(font_size)
                .normalized_coords(coords)
                .draw(&peniko::Style::Fill(Fill::NonZero), &glyphs);
        }
    }
}
