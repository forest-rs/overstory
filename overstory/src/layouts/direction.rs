// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Inherited inline direction for application layout.
///
/// Direction resolves logical start/end behavior throughout a retained
/// subtree. It affects layout, the default paragraph direction, overlay
/// policy, scrolling, directional controls, and accessibility without
/// changing retained or accessibility-tree order.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum LayoutDirection {
    /// Inline content starts at the left and advances rightward.
    #[default]
    Ltr,
    /// Inline content starts at the right and advances leftward.
    Rtl,
}

impl LayoutDirection {
    /// Returns whether inline content advances from right to left.
    #[must_use]
    pub const fn is_rtl(self) -> bool {
        matches!(self, Self::Rtl)
    }

    pub(crate) const fn inspection_summary(self) -> &'static str {
        match self {
            Self::Ltr => "ltr",
            Self::Rtl => "rtl",
        }
    }

    pub(crate) const fn to_taffy(self) -> taffy::Direction {
        match self {
            Self::Ltr => taffy::Direction::Ltr,
            Self::Rtl => taffy::Direction::Rtl,
        }
    }
}
