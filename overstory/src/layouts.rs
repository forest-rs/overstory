// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Overstory-owned layout intent.
//!
//! The public API deliberately exposes a compact [`Layout`] builder instead of
//! the full underlying layout engine. Add named methods here when Overstory
//! needs to support a new layout concept; do not mirror every engine type.
//!
//! ```ignore
//! use overstory::{Layout, Panel};
//!
//! ui.append(
//!     parent,
//!     Panel::new().layout(
//!         Layout::row(24.0)
//!             .align_center()
//!             .justify_center()
//!             .padding_all(12.0),
//!     ),
//! );
//! ```
//!
//! Roadmap:
//! - Keep `Layout` as Overstory intent, with Taffy as an implementation detail
//!   hidden behind conversion during layout. See ticket `ove-9tcf`.
//! - Add builder methods only for concepts the toolkit wants to support long
//!   term: min/max constraints, margins, flex basis, grid, overflow, and
//!   alignment should be deliberate API choices.
//! - Preserve escape-hatch pressure by improving Overstory types instead of
//!   exposing raw layout-engine types in public control APIs.
//! - Coordinate layout vocabulary with `kurbo::Axis`, scrolling, splitters, and
//!   viewport controls so axis-specific code stays small.

/// Placement contribution for an element.
///
/// `LayoutPlacement` covers how the element itself is positioned in its
/// parent. It is deliberately separate from [`Layout`], which describes sizing
/// and child layout. That lets virtualized controls own scroll-relative row
/// placement without overwriting themed padding or alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum LayoutPlacement {
    /// No placement opinion. Lowering falls back to placement carried by
    /// [`Layout`], then to [`Self::Relative`]. [`crate::Ui::set_layout`] does
    /// not write this variant when the layout carries no explicit placement,
    /// so styled placement can still contribute independently.
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

    fn apply_to(self, style: &mut taffy::Style) {
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

/// Layout contribution for an element.
///
/// `Layout` is the boundary between Overstory controls and the layout engine:
/// callers describe intent with Overstory methods, and `Ui` lowers that intent
/// to its current engine during [`crate::Ui::compute_layout`].
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    style: taffy::Style,
    placement: LayoutPlacement,
}

impl Default for Layout {
    fn default() -> Self {
        Self::new()
    }
}

impl Layout {
    /// Creates a layout with no explicit constraints.
    #[must_use]
    pub fn new() -> Self {
        Self {
            style: taffy::Style::default(),
            placement: LayoutPlacement::default(),
        }
    }

    /// Creates a flex container without choosing a direction.
    #[must_use]
    pub fn flex() -> Self {
        let mut layout = Self::new();
        layout.style.display = taffy::Display::Flex;
        layout
    }

    /// Creates a horizontal flex row with a uniform child gap.
    #[must_use]
    pub fn row(gap: f32) -> Self {
        Self::flex()
            .flex_direction(taffy::FlexDirection::Row)
            .gap(gap)
    }

    /// Creates a vertical flex column with a uniform child gap.
    #[must_use]
    pub fn column(gap: f32) -> Self {
        Self::flex()
            .flex_direction(taffy::FlexDirection::Column)
            .gap(gap)
    }

    /// Uses normal in-flow positioning and anchors absolute descendants.
    #[must_use]
    pub fn relative(mut self) -> Self {
        self.placement = LayoutPlacement::Relative;
        self
    }

    /// Removes this element from normal flow.
    #[must_use]
    pub fn absolute(mut self) -> Self {
        self.placement = LayoutPlacement::Absolute;
        self
    }

    /// Removes this element from normal flow and places its top-left corner.
    #[must_use]
    pub fn absolute_at(mut self, left: f32, top: f32) -> Self {
        self.placement = LayoutPlacement::AbsoluteAt { left, top };
        self
    }

    /// Sets a fixed logical-pixel width and height.
    #[must_use]
    pub fn logical_size(mut self, width: f32, height: f32) -> Self {
        self.style.size = taffy::Size {
            width: taffy::Dimension::length(width),
            height: taffy::Dimension::length(height),
        };
        self
    }

    /// Sets a width and height as fractions of the containing block.
    #[must_use]
    pub fn percent_size(mut self, width: f32, height: f32) -> Self {
        self.style.size = taffy::Size {
            width: taffy::Dimension::percent(width),
            height: taffy::Dimension::percent(height),
        };
        self
    }

    /// Sets equal padding on all four sides.
    #[must_use]
    pub fn padding_all(mut self, value: f32) -> Self {
        self.style.padding = rect_lp(value, value, value, value);
        self
    }

    /// Sets horizontal and vertical padding.
    #[must_use]
    pub fn padding_xy(mut self, horizontal: f32, vertical: f32) -> Self {
        self.style.padding = rect_lp(horizontal, horizontal, vertical, vertical);
        self
    }

    /// Centers children on the cross axis.
    #[must_use]
    pub fn align_center(mut self) -> Self {
        self.style.align_items = Some(taffy::AlignItems::Center);
        self
    }

    /// Stretches this element in its parent's cross axis.
    #[must_use]
    pub fn align_self_stretch(mut self) -> Self {
        self.style.align_self = Some(taffy::AlignSelf::Stretch);
        self
    }

    /// Packs children at the start of the main axis.
    #[must_use]
    pub fn justify_start(mut self) -> Self {
        self.style.justify_content = Some(taffy::JustifyContent::Start);
        self
    }

    /// Centers children on the main axis.
    #[must_use]
    pub fn justify_center(mut self) -> Self {
        self.style.justify_content = Some(taffy::JustifyContent::Center);
        self
    }

    /// Packs children at the end of the main axis.
    #[must_use]
    pub fn justify_end(mut self) -> Self {
        self.style.justify_content = Some(taffy::JustifyContent::End);
        self
    }

    /// Sets the flex grow factor.
    #[must_use]
    pub fn flex_grow(mut self, grow: f32) -> Self {
        self.style.flex_grow = grow;
        self
    }

    /// Sets the flex shrink factor.
    ///
    /// Use `0.0` for fixed-size content that should overflow a smaller
    /// viewport instead of being compressed by flex layout.
    #[must_use]
    pub fn flex_shrink(mut self, shrink: f32) -> Self {
        self.style.flex_shrink = shrink;
        self
    }

    pub(crate) fn to_taffy(&self, placement: LayoutPlacement) -> taffy::Style {
        let mut style = self.style.clone();
        match placement {
            LayoutPlacement::Auto => self.placement.apply_to(&mut style),
            placement => placement.apply_to(&mut style),
        }
        style
    }

    pub(crate) fn into_parts(mut self) -> (Self, Option<LayoutPlacement>) {
        let placement = self.placement;
        self.placement = LayoutPlacement::default();
        let placement = match placement {
            LayoutPlacement::Auto => None,
            placement => Some(placement),
        };
        (self, placement)
    }

    fn flex_direction(mut self, direction: taffy::FlexDirection) -> Self {
        self.style.flex_direction = direction;
        self
    }

    fn gap(mut self, gap: f32) -> Self {
        let gap = taffy::LengthPercentage::length(gap);
        self.style.gap = taffy::Size {
            width: gap,
            height: gap,
        };
        self
    }
}

/// Returns a horizontal flex row with a uniform child gap.
#[must_use]
pub fn row_with_gap(gap: f32) -> Layout {
    Layout::row(gap)
}

/// Returns a vertical flex column with a uniform child gap.
#[must_use]
pub fn column_with_gap(gap: f32) -> Layout {
    Layout::column(gap)
}

fn rect_lp(left: f32, right: f32, top: f32, bottom: f32) -> taffy::Rect<taffy::LengthPercentage> {
    taffy::Rect {
        left: taffy::LengthPercentage::length(left),
        right: taffy::LengthPercentage::length(right),
        top: taffy::LengthPercentage::length(top),
        bottom: taffy::LengthPercentage::length(bottom),
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
