// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::format;
use alloc::string::String;

use super::inspection::scalar_summary;

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
        left: f32,
        /// Top inset in logical pixels.
        top: f32,
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
    pub const fn absolute_at(left: f32, top: f32) -> Self {
        Self::AbsoluteAt { left, top }
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
        }
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
                    left: taffy::LengthPercentageAuto::length(left),
                    top: taffy::LengthPercentageAuto::length(top),
                    right: taffy::LengthPercentageAuto::auto(),
                    bottom: taffy::LengthPercentageAuto::auto(),
                };
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
