// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Two-dimensional layout dimension.
///
/// `None` on a containing longhand means "no opinion". `Auto` means an
/// explicit auto-sized opinion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayoutDimension {
    /// Let the layout engine choose the dimension.
    Auto,
    /// Fixed logical-pixel dimension.
    Logical(f32),
    /// Fraction of the containing block, where `1.0` means 100%.
    Percent(f32),
}

impl LayoutDimension {
    pub(super) fn to_taffy(self) -> taffy::Dimension {
        match self {
            Self::Auto => taffy::Dimension::auto(),
            Self::Logical(value) => taffy::Dimension::length(value),
            Self::Percent(value) => taffy::Dimension::percent(value),
        }
    }
}
