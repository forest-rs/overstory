// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text shaping and rendering via parley + imaging.
//!
//! This module owns the parley `FontContext` + `LayoutContext` that Ui
//! holds, caches shaped text layouts, and provides a helper that walks a
//! built parley `Layout` and emits glyph runs through imaging's `Painter`.
//!
//! Roadmap:
//! - Keep Parley as the shaping and editing core while Overstory owns layout
//!   cache policy, style-key construction, and presentation lowering.
//! - Let repeated labels share shaped layouts across controls when text, style,
//!   and compatible width inputs match.
//! - Add prepared editable-text rendering so `TextInput` and future `TextArea`
//!   can use `PlainEditor` layout state without bypassing presentation.
//! - Grow toward richer text through spans, locale, bidi, font fallback,
//!   wrapping, overflow, and accessibility without making simple labels slow.
//! - Keep font discovery host-explicit where possible so no-std and browser
//!   targets can choose their own font story.

use alloc::collections::VecDeque;
use alloc::rc::Rc;
use hashbrown::{Equivalent, HashMap};
use imaging::{PaintSink, Painter, record::Glyph};
use kurbo::Affine;
use parley::{
    Alignment, AlignmentOptions, FontContext, FontFeatures, FontVariations, Layout, LayoutContext,
    LineHeight, PlainEditor, PlainEditorDriver, PositionedLayoutItem, StyleProperty,
};
use peniko::{Brush, Fill, Style};
use understory_presentation::{TextLineHeight, TextStyle as PresentationTextStyle};

const DEFAULT_LAYOUT_CACHE_CAPACITY: usize = 512;
const MAX_ADVANCE_COMPATIBILITY_EPSILON: f32 = 0.01;

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
    layout_cache: TextLayoutCache,
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
            layout_cache: TextLayoutCache::with_capacity(DEFAULT_LAYOUT_CACHE_CAPACITY),
        }
    }

    /// Shapes `text` with the given single-run text style.
    ///
    /// If `wrap_width` is `Some`, it is treated as a logical-pixel line width.
    /// The conversion to Parley's `f32` max advance stays inside the text
    /// engine so layout callers do not have to depend on Parley's coordinate
    /// details. Returns the built parley [`Layout`] ready for rendering or
    /// measuring.
    ///
    /// The cache key includes geometry-affecting typography so repeated labels
    /// share work without reusing a layout shaped for a different style.
    pub fn shape(
        &mut self,
        text: &str,
        style: &PresentationTextStyle,
        wrap_width: Option<f64>,
    ) -> TextLayout {
        let max_advance = parley_max_advance(wrap_width);
        let lookup = TextLayoutLookupKey::new(text, style, max_advance);
        if let Some(layout) = self.layout_cache.get(&lookup) {
            return layout;
        }
        let layout = self.shape_uncached(text, style, max_advance);
        let key = TextLayoutKey::new(text, style, max_advance);
        self.layout_cache.insert(key, layout)
    }

    /// Clears all cached text layouts.
    pub fn clear_layout_cache(&mut self) {
        self.layout_cache.clear();
    }

    /// Returns the number of cached text layouts.
    #[must_use]
    pub fn layout_cache_len(&self) -> usize {
        self.layout_cache.len()
    }

    /// Returns cache hit/miss counters for shaped text layouts.
    #[must_use]
    pub fn layout_cache_stats(&self) -> TextLayoutCacheStats {
        self.layout_cache.stats()
    }

    fn shape_uncached(
        &mut self,
        text: &str,
        style: &PresentationTextStyle,
        max_advance: Option<f32>,
    ) -> Layout<Brush> {
        let mut builder =
            self.layout_context
                .ranged_builder(&mut self.font_context, text, 1.0, true);
        builder.push_default(style.font_family.clone());
        builder.push_default(StyleProperty::FontSize(style.font_size));
        builder.push_default(StyleProperty::FontWidth(style.font_width));
        builder.push_default(StyleProperty::FontStyle(style.font_style));
        builder.push_default(StyleProperty::FontWeight(style.font_weight));
        builder.push_default(StyleProperty::FontVariations(FontVariations::from(
            style.font_variations.as_ref(),
        )));
        builder.push_default(StyleProperty::FontFeatures(FontFeatures::from(
            style.font_features.as_ref(),
        )));
        builder.push_default(StyleProperty::Locale(style.language));
        builder.push_default(StyleProperty::LineHeight(parley_line_height(
            style.line_height,
        )));
        builder.push_default(StyleProperty::LetterSpacing(style.letter_spacing));
        builder.push_default(StyleProperty::WordSpacing(style.word_spacing));
        builder.push_default(StyleProperty::WordBreak(style.word_break));
        builder.push_default(StyleProperty::OverflowWrap(style.overflow_wrap));
        builder.push_default(StyleProperty::TextWrapMode(style.text_wrap_mode));
        builder.push_default(StyleProperty::Brush(Brush::default()));
        let mut layout = builder.build(text);
        layout.break_all_lines(max_advance);
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

pub(crate) fn parley_line_height(line_height: TextLineHeight) -> LineHeight {
    match line_height {
        TextLineHeight::MetricsRelative(value) => LineHeight::MetricsRelative(value),
        TextLineHeight::FontSizeRelative(value) => LineHeight::FontSizeRelative(value),
        TextLineHeight::Absolute(value) => LineHeight::Absolute(value),
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

/// Shared shaped text layout returned by [`TextEngine::shape`].
///
/// Cloning this handle is cheap. The underlying parley layout may be shared by
/// multiple widgets with the same text and geometry-affecting style.
#[derive(Clone)]
pub struct TextLayout {
    layout: Rc<Layout<Brush>>,
}

impl TextLayout {
    fn new(layout: Rc<Layout<Brush>>) -> Self {
        Self { layout }
    }

    /// Returns the shaped parley layout.
    #[must_use]
    pub fn layout(&self) -> &Layout<Brush> {
        self.layout.as_ref()
    }
}

impl core::ops::Deref for TextLayout {
    type Target = Layout<Brush>;

    fn deref(&self) -> &Self::Target {
        self.layout()
    }
}

impl core::fmt::Debug for TextLayout {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TextLayout")
            .field("width", &self.width())
            .field("height", &self.height())
            .finish_non_exhaustive()
    }
}

/// Runtime counters for shaped text layout caching.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextLayoutCacheStats {
    /// Number of cached layout hits.
    pub hits: usize,
    /// Number of uncached layout builds.
    pub misses: usize,
    /// Number of layouts evicted after the cache reached capacity.
    pub evictions: usize,
}

struct TextLayoutCache {
    entries: HashMap<TextLayoutKey, Rc<Layout<Brush>>>,
    insertion_order: VecDeque<TextLayoutKey>,
    capacity: usize,
    stats: TextLayoutCacheStats,
}

impl TextLayoutCache {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            insertion_order: VecDeque::new(),
            capacity,
            stats: TextLayoutCacheStats::default(),
        }
    }

    fn get(&mut self, key: &TextLayoutLookupKey<'_>) -> Option<TextLayout> {
        let layout = self.entries.get(key).cloned().or_else(|| {
            self.entries
                .iter()
                .find(|(candidate, layout)| key.is_satisfied_by(candidate, layout))
                .map(|(_, layout)| Rc::clone(layout))
        })?;
        self.stats.hits += 1;
        Some(TextLayout::new(layout))
    }

    fn insert(&mut self, key: TextLayoutKey, layout: Layout<Brush>) -> TextLayout {
        self.stats.misses += 1;
        let layout = Rc::new(layout);
        if self.capacity == 0 {
            return TextLayout::new(layout);
        }
        while self.entries.len() >= self.capacity {
            let Some(evicted) = self.insertion_order.pop_front() else {
                break;
            };
            if self.entries.remove(&evicted).is_some() {
                self.stats.evictions += 1;
            }
        }
        self.entries.insert(key.clone(), Rc::clone(&layout));
        self.insertion_order.push_back(key);
        TextLayout::new(layout)
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.insertion_order.clear();
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn stats(&self) -> TextLayoutCacheStats {
        self.stats
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct TextLayoutKey {
    text: Rc<str>,
    style: PresentationTextStyle,
    max_advance: Option<ScalarKey>,
}

impl TextLayoutKey {
    fn new(text: &str, style: &PresentationTextStyle, max_advance: Option<f32>) -> Self {
        Self {
            text: Rc::from(text),
            style: style.clone(),
            max_advance: max_advance.map(ScalarKey::new),
        }
    }
}

#[derive(Debug, Eq, Hash, PartialEq)]
struct TextLayoutLookupKey<'a> {
    text: &'a str,
    style: PresentationTextStyle,
    max_advance: Option<ScalarKey>,
}

impl<'a> TextLayoutLookupKey<'a> {
    fn new(text: &'a str, style: &PresentationTextStyle, max_advance: Option<f32>) -> Self {
        Self {
            text,
            style: style.clone(),
            max_advance: max_advance.map(ScalarKey::new),
        }
    }
}

impl Equivalent<TextLayoutKey> for TextLayoutLookupKey<'_> {
    fn equivalent(&self, key: &TextLayoutKey) -> bool {
        self.text == key.text.as_ref()
            && self.style == key.style
            && self.max_advance == key.max_advance
    }
}

impl TextLayoutLookupKey<'_> {
    fn is_satisfied_by(&self, key: &TextLayoutKey, layout: &Layout<Brush>) -> bool {
        self.text == key.text.as_ref()
            && self.style == key.style
            && max_advance_satisfies(self.max_advance, key.max_advance, layout.width())
    }
}

fn max_advance_satisfies(
    requested: Option<ScalarKey>,
    cached: Option<ScalarKey>,
    layout_width: f32,
) -> bool {
    cached.is_none_or(|cached| {
        requested.is_some_and(|requested| {
            cached.value() - requested.value() + MAX_ADVANCE_COMPATIBILITY_EPSILON >= 0.0
        })
    }) && requested.is_none_or(|requested| {
        requested.value() - layout_width + MAX_ADVANCE_COMPATIBILITY_EPSILON >= 0.0
    })
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ScalarKey(u32);

impl ScalarKey {
    fn new(value: f32) -> Self {
        if value == 0.0 {
            Self(0)
        } else {
            Self(value.to_bits())
        }
    }

    fn value(self) -> f32 {
        f32::from_bits(self.0)
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
/// Underlines and strikethroughs are omitted for now; they'll come back when
/// richer text styling lands.
pub(crate) fn render_text<S>(
    painter: &mut Painter<'_, S>,
    transform: Affine,
    layout: &Layout<Brush>,
    brush: &Brush,
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
                .glyphs(run.font(), brush)
                .transform(transform)
                .font_size(run.font_size())
                .hint(true)
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

#[cfg(test)]
mod tests {
    use super::{TextEngine, TextResources};
    use understory_presentation::{FontWeight, TextStyle};

    #[test]
    fn shape_reuses_cached_layout_for_same_geometry_inputs() {
        let resources = TextResources::empty();
        let mut engine = TextEngine::from_resources(&resources);
        let style = TextStyle::default();

        let first = engine.shape("Shared label", &style, None);
        let second = engine.shape("Shared label", &style, None);

        assert_eq!(first.width(), second.width());
        assert_eq!(engine.layout_cache_len(), 1);
        assert_eq!(engine.layout_cache_stats().misses, 1);
        assert_eq!(engine.layout_cache_stats().hits, 1);
    }

    #[test]
    fn shape_wrap_width_participates_in_cache_key() {
        let resources = TextResources::empty();
        let mut engine = TextEngine::from_resources(&resources);
        let style = TextStyle::default();

        let _ = engine.shape("Shared label", &style, Some(80.0));
        let _ = engine.shape("Shared label", &style, Some(120.0));

        assert_eq!(engine.layout_cache_len(), 2);
        assert_eq!(engine.layout_cache_stats().misses, 2);
        assert_eq!(engine.layout_cache_stats().hits, 0);
    }

    #[test]
    fn shape_reuses_layout_for_compatible_narrower_wrap_width() {
        let resources = TextResources::empty();
        let mut engine = TextEngine::from_resources(&resources);
        let style = TextStyle::default();

        let wide = engine.shape("Label", &style, Some(120.0));
        let narrower = engine.shape("Label", &style, Some(80.0));

        assert_eq!(wide.width(), narrower.width());
        assert_eq!(engine.layout_cache_len(), 1);
        assert_eq!(engine.layout_cache_stats().misses, 1);
        assert_eq!(engine.layout_cache_stats().hits, 1);
    }

    #[test]
    fn shape_does_not_use_constrained_layout_for_unbounded_width() {
        let resources = TextResources::empty();
        let mut engine = TextEngine::from_resources(&resources);
        let style = TextStyle::default();

        let _ = engine.shape("Shared label", &style, Some(120.0));
        let _ = engine.shape("Shared label", &style, None);

        assert_eq!(engine.layout_cache_len(), 2);
        assert_eq!(engine.layout_cache_stats().misses, 2);
        assert_eq!(engine.layout_cache_stats().hits, 0);
    }

    #[test]
    fn shape_style_participates_in_cache_key() {
        let resources = TextResources::empty();
        let mut engine = TextEngine::from_resources(&resources);
        let normal = TextStyle::default();
        let bold = TextStyle {
            font_weight: FontWeight::BOLD,
            ..TextStyle::default()
        };

        let _ = engine.shape("Shared label", &normal, None);
        let _ = engine.shape("Shared label", &bold, None);

        assert_eq!(engine.layout_cache_len(), 2);
        assert_eq!(engine.layout_cache_stats().misses, 2);
        assert_eq!(engine.layout_cache_stats().hits, 0);
    }
}
