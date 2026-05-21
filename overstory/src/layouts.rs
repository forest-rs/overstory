// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Small builders for common [`taffy::Style`] shapes.
//!
//! Hand-rolling a `taffy::Style` for a flex row or column is six
//! lines of boilerplate that obscure what the layout is meant to
//! do. These helpers cover the common cases — fixed-gap row /
//! column with optional padding — and return a plain
//! `taffy::Style`, so callers stay free to override any field with
//! struct-update syntax:
//!
//! ```ignore
//! use overstory::layouts;
//! use overstory::taffy;
//!
//! // Centered row of children with a 24 px gap.
//! let style = taffy::Style {
//!     justify_content: Some(taffy::JustifyContent::Center),
//!     align_items: Some(taffy::AlignItems::Center),
//!     ..layouts::row_with_gap(24.0)
//! };
//! ```

/// Returns a `taffy::Style` configured as a horizontal flex row
/// with a uniform inter-child gap of `gap` logical pixels.
///
/// Defaults are deliberately minimal: nothing about size, padding,
/// or alignment is set — callers add the bits they care about via
/// struct-update syntax.
#[must_use]
pub fn row_with_gap(gap: f32) -> taffy::Style {
    taffy::Style {
        display: taffy::Display::Flex,
        flex_direction: taffy::FlexDirection::Row,
        gap: taffy::Size {
            width: taffy::LengthPercentage::length(gap),
            height: taffy::LengthPercentage::length(gap),
        },
        ..taffy::Style::default()
    }
}

/// Returns a `taffy::Style` configured as a vertical flex column
/// with a uniform inter-child gap of `gap` logical pixels.
///
/// Same defaults caveat as [`row_with_gap`].
#[must_use]
pub fn column_with_gap(gap: f32) -> taffy::Style {
    taffy::Style {
        display: taffy::Display::Flex,
        flex_direction: taffy::FlexDirection::Column,
        gap: taffy::Size {
            width: taffy::LengthPercentage::length(gap),
            height: taffy::LengthPercentage::length(gap),
        },
        ..taffy::Style::default()
    }
}

/// Uniform `taffy::Rect<LengthPercentage>` from a single value, for
/// the common "same padding on all four sides" case.
#[must_use]
pub fn uniform_padding(value: f32) -> taffy::Rect<taffy::LengthPercentage> {
    let v = taffy::LengthPercentage::length(value);
    taffy::Rect {
        left: v,
        right: v,
        top: v,
        bottom: v,
    }
}
