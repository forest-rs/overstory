// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text shaping and rendering via parley + imaging.
//!
//! This module owns the parley `FontContext` + `LayoutContext` that Ui
//! holds, and provides a helper that walks a built parley `Layout` and
//! emits glyph runs through imaging's `Painter`.

use imaging::{PaintSink, Painter, record::Glyph};
use kurbo::Affine;
use parley::{
    Alignment, AlignmentOptions, FontContext, FontFamily, FontWeight, Layout, LayoutContext,
    LineHeight, PlainEditor, PlainEditorDriver, PositionedLayoutItem, StyleProperty,
};
use peniko::{Brush, Fill, Style};

/// Shared font discovery and source-cache state for text engines.
///
/// Hosts that create many [`crate::Ui`] instances should keep one
/// `TextResources` and pass it to [`crate::Ui::with_text_resources`]. Each
/// UI still gets its own mutable layout context, while font discovery and
/// source data are shared by cloning Parley's shared font context internals.
pub struct TextResources {
    font_context: FontContext,
}

impl TextResources {
    /// Constructs reusable text resources from an existing Parley font context.
    ///
    /// The context is converted to Parley's shared backing stores before being
    /// retained so that cloned [`TextEngine`] instances can share discovered
    /// font collection and source-cache data.
    #[must_use]
    pub fn from_font_context(font_context: FontContext) -> Self {
        Self {
            font_context: make_shared_font_context(font_context),
        }
    }

    /// Constructs text resources with an initially empty font context.
    ///
    /// This is the no-std-friendly constructor. Hosts that want platform font
    /// discovery should use [`Self::system_fonts`] when the `system-fonts`
    /// feature is enabled, or pass their own populated [`FontContext`] through
    /// [`Self::from_font_context`].
    #[must_use]
    pub fn empty() -> Self {
        Self::from_font_context(FontContext::default())
    }

    /// Constructs text resources backed by Parley's platform font discovery.
    #[cfg(feature = "system-fonts")]
    #[must_use]
    pub fn system_fonts() -> Self {
        Self::from_font_context(FontContext::default())
    }
}

#[cfg(feature = "std")]
fn make_shared_font_context(mut font_context: FontContext) -> FontContext {
    font_context.collection.make_shared();
    font_context.source_cache.make_shared();
    font_context
}

#[cfg(not(feature = "std"))]
fn make_shared_font_context(font_context: FontContext) -> FontContext {
    font_context
}

impl core::fmt::Debug for TextResources {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TextResources").finish_non_exhaustive()
    }
}

/// Owns parley's font and layout context state. One per [`crate::Ui`].
pub struct TextEngine {
    /// Font discovery + source cache.
    pub font_context: FontContext,
    /// Working context for building text layouts.
    pub layout_context: LayoutContext<Brush>,
}

impl TextEngine {
    /// Constructs a new text engine backed by platform font discovery.
    ///
    /// Prefer [`Self::from_resources`] when constructing many UIs; that keeps
    /// platform font discovery explicit and reusable.
    #[cfg(feature = "system-fonts")]
    #[must_use]
    pub fn new() -> Self {
        Self::from_resources(&TextResources::system_fonts())
    }

    /// Constructs a text engine by cloning shared font resources.
    ///
    /// The returned engine owns an independent Parley layout context while
    /// sharing font collection and source-cache backing stores with
    /// `resources`.
    #[must_use]
    pub fn from_resources(resources: &TextResources) -> Self {
        Self {
            font_context: resources.font_context.clone(),
            layout_context: LayoutContext::new(),
        }
    }

    /// Shapes `text` with the given font size and brush.
    ///
    /// If `wrap_width` is `Some`, it is treated as a logical-pixel line width.
    /// The conversion to Parley's `f32` max advance stays inside the text
    /// engine so layout callers do not have to depend on Parley's coordinate
    /// details. Returns the built parley [`Layout`] ready for rendering or
    /// measuring.
    ///
    /// Phase 1f-4 uses platform-default UI font, medium weight, and a
    /// touch of positive letter-spacing for crisp button labels. Apps
    /// will eventually be able to override these via per-element style
    /// properties; for now the defaults are baked in here.
    pub fn shape(
        &mut self,
        text: &str,
        font_size: f32,
        brush: Brush,
        wrap_width: Option<f64>,
    ) -> Layout<Brush> {
        let mut builder =
            self.layout_context
                .ranged_builder(&mut self.font_context, text, 1.0, true);
        builder.push_default(FontFamily::from("system-ui, sans-serif"));
        builder.push_default(StyleProperty::FontWeight(FontWeight::MEDIUM));
        builder.push_default(StyleProperty::FontSize(font_size));
        builder.push_default(LineHeight::FontSizeRelative(1.2));
        builder.push_default(StyleProperty::LetterSpacing(0.4));
        builder.push_default(StyleProperty::Brush(brush));
        let mut layout = builder.build(text);
        layout.break_all_lines(parley_max_advance(wrap_width));
        layout.align(Alignment::Start, AlignmentOptions::default());
        layout
    }

    pub(crate) fn with_plain_editor<R>(
        &mut self,
        editor: &mut PlainEditor<Brush>,
        f: impl FnOnce(&mut PlainEditorDriver<'_, Brush>) -> R,
    ) -> R {
        let mut driver = editor.driver(&mut self.font_context, &mut self.layout_context);
        f(&mut driver)
    }
}

impl core::fmt::Debug for TextEngine {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TextEngine").finish_non_exhaustive()
    }
}

#[cfg(feature = "system-fonts")]
impl Default for TextEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Converts a logical wrap width into Parley's max advance.
///
/// Taffy stores measured widths as `f32`, then Overstory lowers them back
/// through `f64` bounds before painting. A text run measured at its natural
/// width can otherwise be re-shaped against a maximum advance that is a hair
/// smaller than Parley's own line width and wrap spuriously.
#[expect(
    clippy::cast_possible_truncation,
    reason = "UI logical text widths fit parley's f32 coordinates"
)]
fn parley_max_advance(wrap_width: Option<f64>) -> Option<f32> {
    const WRAP_WIDTH_TOLERANCE: f32 = 0.5;

    let width = wrap_width?;
    if width.is_finite() && width > 0.0 {
        Some(width as f32 + WRAP_WIDTH_TOLERANCE)
    } else {
        None
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
            let run = glyph_run.run();
            painter
                .glyphs(run.font(), &glyph_run.style().brush)
                .transform(transform)
                .font_size(run.font_size())
                .normalized_coords(run.normalized_coords())
                .draw(
                    &Style::Fill(Fill::NonZero),
                    glyph_run.positioned_glyphs().map(|g| Glyph {
                        id: g.id,
                        x: g.x,
                        y: g.y,
                    }),
                );
        }
    }
}
