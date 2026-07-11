// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use super::inspection::scalar_summary;
use super::taffy_scalar;

/// Placement contribution for an element.
///
/// `LayoutPlacement` covers how the element itself is positioned in its
/// parent. It is deliberately separate from [`crate::Layout`], which describes
/// sizing and child layout. That lets virtualized controls own scroll-relative
/// row placement without overwriting themed padding or alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum LayoutPlacement {
    /// No placement opinion. Lowering falls back to placement carried by
    /// [`crate::Layout`], then to [`Self::Relative`]. [`crate::Ui::set_layout`]
    /// does not write this variant when the layout carries no explicit
    /// placement, so styled placement can still contribute independently.
    #[default]
    Auto,
    /// Normal in-flow positioning. This is Taffy's relative positioning mode.
    Relative,
    /// Removed from normal flow with automatic insets.
    Absolute,
    /// Removed from normal flow and placed by its top-left corner.
    AbsoluteAt {
        /// Left inset in logical pixels.
        left: f64,
        /// Top inset in logical pixels.
        top: f64,
    },
    /// Places an in-flow item into one explicit grid cell.
    ///
    /// Grid line numbers are one-based and must not be zero. Multiple
    /// children assigned to the same cell overlap in document order.
    GridCell {
        /// One-based grid column line.
        column: i16,
        /// One-based grid row line.
        row: i16,
    },
}

impl LayoutPlacement {
    /// Creates normal in-flow placement.
    #[must_use]
    pub const fn relative() -> Self {
        Self::Relative
    }

    /// Creates absolute placement with automatic insets.
    #[must_use]
    pub const fn absolute() -> Self {
        Self::Absolute
    }

    /// Creates absolute placement with a fixed top-left inset.
    #[must_use]
    pub const fn absolute_at(left: f64, top: f64) -> Self {
        Self::AbsoluteAt { left, top }
    }

    /// Places an item into a one-based grid cell.
    ///
    /// # Panics
    ///
    /// Panics when `column` or `row` is zero.
    #[must_use]
    pub const fn grid_cell(column: i16, row: i16) -> Self {
        assert!(column != 0, "grid column must not be zero");
        assert!(row != 0, "grid row must not be zero");
        Self::GridCell { column, row }
    }

    pub(crate) fn inspection_summary(self) -> String {
        match self {
            Self::Auto => String::from("auto"),
            Self::Relative => String::from("relative"),
            Self::Absolute => String::from("absolute"),
            Self::AbsoluteAt { left, top } => {
                format!(
                    "absolute at {}, {}",
                    scalar_summary(left),
                    scalar_summary(top)
                )
            }
            Self::GridCell { column, row } => format!("grid cell {column}, {row}"),
        }
    }

    pub(crate) fn inspection_fields(self) -> Vec<(&'static str, String)> {
        let mut fields = Vec::new();
        match self {
            Self::Auto => fields.push(("Mode", String::from("auto"))),
            Self::Relative => fields.push(("Mode", String::from("relative"))),
            Self::Absolute => fields.push(("Mode", String::from("absolute"))),
            Self::AbsoluteAt { left, top } => {
                fields.push(("Mode", String::from("absolute")));
                fields.push(("Left", format!("{}px", scalar_summary(left))));
                fields.push(("Top", format!("{}px", scalar_summary(top))));
            }
            Self::GridCell { column, row } => {
                fields.push(("Mode", String::from("grid cell")));
                fields.push(("Column", format!("{column}")));
                fields.push(("Row", format!("{row}")));
            }
        }
        fields
    }

    pub(super) fn apply_to(self, style: &mut taffy::Style) {
        match self {
            Self::Auto => Self::Relative.apply_to(style),
            Self::Relative => {
                style.position = taffy::Position::Relative;
                style.inset = auto_insets();
            }
            Self::Absolute => {
                style.position = taffy::Position::Absolute;
                style.inset = auto_insets();
            }
            Self::AbsoluteAt { left, top } => {
                style.position = taffy::Position::Absolute;
                style.inset = taffy::Rect {
                    left: taffy::LengthPercentageAuto::length(taffy_scalar(left)),
                    top: taffy::LengthPercentageAuto::length(taffy_scalar(top)),
                    right: taffy::LengthPercentageAuto::auto(),
                    bottom: taffy::LengthPercentageAuto::auto(),
                };
            }
            Self::GridCell { column, row } => {
                style.position = taffy::Position::Relative;
                style.inset = auto_insets();
                style.grid_column = taffy::style_helpers::line(column);
                style.grid_row = taffy::style_helpers::line(row);
            }
        }
    }
}

fn auto_insets() -> taffy::Rect<taffy::LengthPercentageAuto> {
    taffy::Rect {
        left: taffy::LengthPercentageAuto::auto(),
        right: taffy::LengthPercentageAuto::auto(),
        top: taffy::LengthPercentageAuto::auto(),
        bottom: taffy::LengthPercentageAuto::auto(),
    }
}
