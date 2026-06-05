// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text and font authoring extension traits.
//!
//! `content(...)` and `text(...)` are deliberately different:
//! - `content(...)` writes semantic owner content that a control template can
//!   project through a content presenter.
//! - `text(...)` writes the rendered string consumed by a leaf `Text` element
//!   or a template-owned glyph/label part.

use alloc::boxed::Box;
use alloc::string::String;

use parley::{FontFeature, FontVariation};
use understory_presentation::{
    FontFamily, FontStyle, FontWeight, FontWidth, Language, OverflowWrap, TextLineHeight,
    TextWrapMode, WordBreak,
};

use crate::spec_ext::ElementSpec;
use crate::widget::BuiltInPropertyKey;

/// Text and font builder methods available to element specs.
///
/// These helpers write Overstory's built-in text style properties without
/// requiring the caller to fetch a [`BuiltInProperties`](crate::BuiltInProperties)
/// handle from a [`Ui`](crate::Ui).
pub trait TextSpecExt: ElementSpec + Sized {
    /// Sets the initial semantic owner content property.
    ///
    /// Use this for controls that present author-facing content through a
    /// content presenter, such as button labels, expander headers, list items,
    /// and selected values. Use [`Self::text`] for the actual string rendered by
    /// `Text` elements.
    #[must_use]
    fn content(mut self, content: impl Into<String>) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::Content, content.into());
        self
    }

    /// Sets the initial rendered text property.
    ///
    /// Use this for leaf `Text` elements and for template-owned glyph/label
    /// parts. Owner controls should usually expose their labels through
    /// [`Self::content`] so replacement templates can choose how to present
    /// them.
    #[must_use]
    fn text(mut self, text: impl Into<String>) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::Text, text.into());
        self
    }

    /// Sets the initial font size in logical pixels.
    #[must_use]
    fn font_size(mut self, font_size: f64) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::FontSize, font_size);
        self
    }

    /// Sets the initial font family list.
    #[must_use]
    fn font_family(mut self, family: FontFamily<'static>) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::FontFamily, family);
        self
    }

    /// Sets the initial font width.
    #[must_use]
    fn font_width(mut self, width: FontWidth) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::FontWidth, width);
        self
    }

    /// Sets the initial font weight.
    #[must_use]
    fn font_weight(mut self, weight: FontWeight) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::FontWeight, weight);
        self
    }

    /// Sets the initial font style.
    #[must_use]
    fn font_style(mut self, style: FontStyle) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::FontStyle, style);
        self
    }

    /// Sets initial font variation settings.
    #[must_use]
    fn font_variations(mut self, variations: impl Into<Box<[FontVariation]>>) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::FontVariations, variations.into());
        self
    }

    /// Sets initial OpenType font feature settings.
    #[must_use]
    fn font_features(mut self, features: impl Into<Box<[FontFeature]>>) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::FontFeatures, features.into());
        self
    }

    /// Sets the initial text language hint.
    #[must_use]
    fn text_language(mut self, language: impl Into<Option<Language>>) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::TextLanguage, language.into());
        self
    }

    /// Sets the initial text line-height policy.
    #[must_use]
    fn line_height(mut self, line_height: TextLineHeight) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::LineHeight, line_height);
        self
    }

    /// Sets the initial extra letter spacing in logical pixels.
    #[must_use]
    fn letter_spacing(mut self, spacing: f32) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::LetterSpacing, spacing);
        self
    }

    /// Sets the initial extra word spacing in logical pixels.
    #[must_use]
    fn word_spacing(mut self, spacing: f32) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::WordSpacing, spacing);
        self
    }

    /// Sets the initial word-break policy.
    #[must_use]
    fn word_break(mut self, word_break: WordBreak) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::WordBreak, word_break);
        self
    }

    /// Sets the initial emergency wrapping policy.
    #[must_use]
    fn overflow_wrap(mut self, overflow_wrap: OverflowWrap) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::OverflowWrap, overflow_wrap);
        self
    }

    /// Sets the initial text line-wrapping policy.
    #[must_use]
    fn text_wrap_mode(mut self, wrap_mode: TextWrapMode) -> Self {
        self.element_config_mut()
            .set_built_in_local(BuiltInPropertyKey::TextWrapMode, wrap_mode);
        self
    }
}

impl<T> TextSpecExt for T where T: ElementSpec {}
