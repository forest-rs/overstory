// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::format;
use alloc::string::String;

use super::inspection::scalar_summary;
use super::taffy_scalar;

/// Two-dimensional layout dimension.
///
/// `None` on a containing longhand means "no opinion". `Auto` means an
/// explicit auto-sized opinion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayoutDimension {
    /// Let the layout engine choose the dimension.
    Auto,
    /// Fixed logical-pixel dimension.
    Logical(f64),
    /// Fraction of the containing block, where `1.0` means 100%.
    Percent(f64),
}

impl LayoutDimension {
    pub(crate) fn inspection_summary(self) -> String {
        match self {
            Self::Auto => String::from("auto"),
            Self::Logical(value) => format!("{}px", scalar_summary(value)),
            Self::Percent(value) => format!("{}%", scalar_summary(value * 100.0)),
        }
    }

    pub(super) fn to_taffy(self) -> taffy::Dimension {
        match self {
            Self::Auto => taffy::Dimension::auto(),
            Self::Logical(value) => taffy::Dimension::length(taffy_scalar(value)),
            Self::Percent(value) => taffy::Dimension::percent(taffy_scalar(value)),
        }
    }
}
