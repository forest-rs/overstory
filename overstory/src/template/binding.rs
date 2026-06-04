// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Bindings used by data-driven template specs.

use crate::element::ElementId;

use super::TemplateContext;

/// Built-in properties addressable from data-driven template bindings.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TemplateBuiltInProperty {
    /// `Background`.
    Background,
    /// `Bounds`.
    Bounds,
    /// `CornerRadius`.
    CornerRadius,
    /// `Layout`.
    Layout,
    /// `LayoutContainer`.
    LayoutContainer,
    /// `LayoutSize`.
    LayoutSize,
    /// `LayoutSpacing`.
    LayoutSpacing,
    /// `LayoutAlignment`.
    LayoutAlignment,
    /// `LayoutOverflow`.
    LayoutOverflow,
    /// `LayoutPlacement`.
    LayoutPlacement,
    /// `Content`.
    Content,
    /// `Foreground`.
    Foreground,
    /// `FontSize`.
    FontSize,
    /// `FontFamily`.
    FontFamily,
    /// `FontWidth`.
    FontWidth,
    /// `FontWeight`.
    FontWeight,
    /// `FontStyle`.
    FontStyle,
    /// `FontVariations`.
    FontVariations,
    /// `FontFeatures`.
    FontFeatures,
    /// `TextLanguage`.
    TextLanguage,
    /// `LineHeight`.
    LineHeight,
    /// `LetterSpacing`.
    LetterSpacing,
    /// `WordSpacing`.
    WordSpacing,
    /// `WordBreak`.
    WordBreak,
    /// `OverflowWrap`.
    OverflowWrap,
    /// `TextWrapMode`.
    TextWrapMode,
    /// `BorderBrush`.
    BorderBrush,
    /// `BorderWidth`.
    BorderWidth,
    /// `IsChecked`.
    IsChecked,
    /// `IsEnabled`.
    IsEnabled,
    /// `Value`.
    Value,
    /// `Minimum`.
    Minimum,
    /// `Maximum`.
    Maximum,
}

/// One owner-to-generated-node binding in a [`super::TemplateSpec`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TemplateBinding {
    source: TemplateBuiltInProperty,
    target: TemplateBuiltInProperty,
}

impl TemplateBinding {
    /// Creates a binding from an owner property to the same generated-node
    /// property.
    #[must_use]
    #[inline]
    pub const fn same(property: TemplateBuiltInProperty) -> Self {
        Self::owner_to_part(property, property)
    }

    /// Creates a binding from an owner property to the generated node.
    #[must_use]
    #[inline]
    pub const fn owner_to_part(
        source: TemplateBuiltInProperty,
        target: TemplateBuiltInProperty,
    ) -> Self {
        Self { source, target }
    }

    /// Returns the owner-side property.
    #[must_use]
    #[inline]
    pub const fn source(&self) -> TemplateBuiltInProperty {
        self.source
    }

    /// Returns the generated-node-side property.
    #[must_use]
    #[inline]
    pub const fn target(&self) -> TemplateBuiltInProperty {
        self.target
    }
}

pub(super) fn apply_template_binding(
    cx: &mut TemplateContext<'_>,
    target: ElementId,
    binding: TemplateBinding,
) {
    use TemplateBuiltInProperty::{
        Background, BorderBrush, BorderWidth, Bounds, Content, CornerRadius, FontFamily,
        FontFeatures, FontSize, FontStyle, FontVariations, FontWeight, FontWidth, Foreground,
        IsChecked, IsEnabled, Layout, LayoutAlignment, LayoutContainer, LayoutOverflow,
        LayoutPlacement, LayoutSize, LayoutSpacing, LetterSpacing, LineHeight, Maximum, Minimum,
        OverflowWrap, TextLanguage, TextWrapMode, Value, WordBreak, WordSpacing,
    };

    let props = *cx.properties();
    let result = match (binding.source, binding.target) {
        (Background, Background) => cx
            .bind_owner_to_part(props.background, target, props.background)
            .map(|_| ()),
        (Bounds, Bounds) => cx
            .bind_owner_to_part(props.bounds, target, props.bounds)
            .map(|_| ()),
        (CornerRadius, CornerRadius) => cx
            .bind_owner_to_part(props.corner_radius, target, props.corner_radius)
            .map(|_| ()),
        (Layout, Layout) => cx
            .bind_owner_to_part(props.layout, target, props.layout)
            .map(|_| ()),
        (LayoutContainer, LayoutContainer) => cx
            .bind_owner_to_part(props.layout_container, target, props.layout_container)
            .map(|_| ()),
        (LayoutSize, LayoutSize) => cx
            .bind_owner_to_part(props.layout_size, target, props.layout_size)
            .map(|_| ()),
        (LayoutSpacing, LayoutSpacing) => cx
            .bind_owner_to_part(props.layout_spacing, target, props.layout_spacing)
            .map(|_| ()),
        (LayoutAlignment, LayoutAlignment) => cx
            .bind_owner_to_part(props.layout_alignment, target, props.layout_alignment)
            .map(|_| ()),
        (LayoutOverflow, LayoutOverflow) => cx
            .bind_owner_to_part(props.layout_overflow, target, props.layout_overflow)
            .map(|_| ()),
        (LayoutPlacement, LayoutPlacement) => cx
            .bind_owner_to_part(props.layout_placement, target, props.layout_placement)
            .map(|_| ()),
        (Content, Content) => cx
            .bind_owner_to_part(props.content, target, props.content)
            .map(|_| ()),
        (Foreground, Foreground) => cx
            .bind_owner_to_part(props.foreground, target, props.foreground)
            .map(|_| ()),
        (FontSize, FontSize) => cx
            .bind_owner_to_part(props.font_size, target, props.font_size)
            .map(|_| ()),
        (FontFamily, FontFamily) => cx
            .bind_owner_to_part(props.font_family, target, props.font_family)
            .map(|_| ()),
        (FontWidth, FontWidth) => cx
            .bind_owner_to_part(props.font_width, target, props.font_width)
            .map(|_| ()),
        (FontWeight, FontWeight) => cx
            .bind_owner_to_part(props.font_weight, target, props.font_weight)
            .map(|_| ()),
        (FontStyle, FontStyle) => cx
            .bind_owner_to_part(props.font_style, target, props.font_style)
            .map(|_| ()),
        (FontVariations, FontVariations) => cx
            .bind_owner_to_part(props.font_variations, target, props.font_variations)
            .map(|_| ()),
        (FontFeatures, FontFeatures) => cx
            .bind_owner_to_part(props.font_features, target, props.font_features)
            .map(|_| ()),
        (TextLanguage, TextLanguage) => cx
            .bind_owner_to_part(props.text_language, target, props.text_language)
            .map(|_| ()),
        (LineHeight, LineHeight) => cx
            .bind_owner_to_part(props.line_height, target, props.line_height)
            .map(|_| ()),
        (LetterSpacing, LetterSpacing) => cx
            .bind_owner_to_part(props.letter_spacing, target, props.letter_spacing)
            .map(|_| ()),
        (WordSpacing, WordSpacing) => cx
            .bind_owner_to_part(props.word_spacing, target, props.word_spacing)
            .map(|_| ()),
        (WordBreak, WordBreak) => cx
            .bind_owner_to_part(props.word_break, target, props.word_break)
            .map(|_| ()),
        (OverflowWrap, OverflowWrap) => cx
            .bind_owner_to_part(props.overflow_wrap, target, props.overflow_wrap)
            .map(|_| ()),
        (TextWrapMode, TextWrapMode) => cx
            .bind_owner_to_part(props.text_wrap_mode, target, props.text_wrap_mode)
            .map(|_| ()),
        (BorderBrush, BorderBrush) => cx
            .bind_owner_to_part(props.border_brush, target, props.border_brush)
            .map(|_| ()),
        (BorderWidth, BorderWidth) => cx
            .bind_owner_to_part(props.border_width, target, props.border_width)
            .map(|_| ()),
        (IsChecked, IsChecked) => cx
            .bind_owner_to_part(props.is_checked, target, props.is_checked)
            .map(|_| ()),
        (IsEnabled, IsEnabled) => cx
            .bind_owner_to_part(props.is_enabled, target, props.is_enabled)
            .map(|_| ()),
        (Value, Value) => cx
            .bind_owner_to_part(props.value, target, props.value)
            .map(|_| ()),
        (Minimum, Minimum) => cx
            .bind_owner_to_part(props.minimum, target, props.minimum)
            .map(|_| ()),
        (Maximum, Maximum) => cx
            .bind_owner_to_part(props.maximum, target, props.maximum)
            .map(|_| ()),
        _ => panic!(
            "TemplateSpec binding requires matching built-in property types: {:?} -> {:?}",
            binding.source, binding.target
        ),
    };
    result.expect("TemplateSpec binding should be valid");
}
