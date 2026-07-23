// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::string::String;
use alloc::vec::Vec;

use super::inspection::{join_summary_parts, push_labeled_summary_part};
use super::{
    LayoutAlign, LayoutAlignment, LayoutContainer, LayoutDimension, LayoutJustify, LayoutOverflow,
    LayoutPlacement, LayoutSize, LayoutSpacing, LayoutWrap,
};

/// Layout contribution for an element.
///
/// `Layout` is a shorthand builder over independent layout longhands:
/// container, size, spacing, alignment, overflow, and placement. Callers use
/// this compact type for ergonomic construction; [`crate::Ui::set_layout`]
/// splits it into the longhand properties that it explicitly carries.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    container: LayoutContainer,
    size: LayoutSize,
    spacing: LayoutSpacing,
    alignment: LayoutAlignment,
    overflow: LayoutOverflow,
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
            container: LayoutContainer::default(),
            size: LayoutSize::default(),
            spacing: LayoutSpacing::default(),
            alignment: LayoutAlignment::default(),
            overflow: LayoutOverflow::default(),
            placement: LayoutPlacement::default(),
        }
    }

    /// Creates a flex container without choosing a direction.
    #[must_use]
    pub fn flex() -> Self {
        Self::new().container(LayoutContainer::flex())
    }

    /// Creates a grid container.
    #[must_use]
    pub fn grid() -> Self {
        Self::new().container(LayoutContainer::grid())
    }

    /// Creates a horizontal flex row with a uniform child gap.
    #[must_use]
    pub fn row(gap: f64) -> Self {
        Self::new()
            .container(LayoutContainer::row())
            .spacing(LayoutSpacing::gap(gap))
    }

    /// Creates a reversed horizontal flex row with a uniform child gap.
    #[must_use]
    pub fn row_reverse(gap: f64) -> Self {
        Self::new()
            .container(LayoutContainer::row_reverse())
            .spacing(LayoutSpacing::gap(gap))
    }

    /// Creates a vertical flex column with a uniform child gap.
    #[must_use]
    pub fn column(gap: f64) -> Self {
        Self::new()
            .container(LayoutContainer::column())
            .spacing(LayoutSpacing::gap(gap))
    }

    /// Creates a bottom-to-top flex column with a uniform child gap.
    #[must_use]
    pub fn column_reverse(gap: f64) -> Self {
        Self::new()
            .container(LayoutContainer::column_reverse())
            .spacing(LayoutSpacing::gap(gap))
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
    pub fn absolute_at(mut self, left: f64, top: f64) -> Self {
        self.placement = LayoutPlacement::AbsoluteAt { left, top };
        self
    }

    /// Sets a fixed logical-pixel width and height.
    #[must_use]
    pub fn logical_size(mut self, width: f64, height: f64) -> Self {
        self.size.merge_from(LayoutSize::logical(width, height));
        self
    }

    /// Sets a width and height as fractions of the containing block.
    #[must_use]
    pub fn percent_size(mut self, width: f64, height: f64) -> Self {
        self.size.merge_from(LayoutSize::percent(width, height));
        self
    }

    /// Fills the available inline space while allowing flex layout to shrink
    /// the element below its intrinsic content width.
    ///
    /// This is the common "take the row width, but do not force overflow"
    /// constraint for wrapping rows and scrollable page bodies.
    #[must_use]
    pub fn fill_width(mut self) -> Self {
        self.size.merge_from(
            LayoutSize::new()
                .width(LayoutDimension::Percent(1.0))
                .min_width(LayoutDimension::Logical(0.0)),
        );
        self
    }

    /// Fills the available block space while allowing flex layout to shrink
    /// the element below its intrinsic content height.
    ///
    /// This is the vertical counterpart to [`Self::fill_width`] for panes,
    /// scroll areas, and controls that should stretch to their parent instead
    /// of carrying a fixed logical height.
    #[must_use]
    pub fn fill_height(mut self) -> Self {
        self.size.merge_from(
            LayoutSize::new()
                .height(LayoutDimension::Percent(1.0))
                .min_height(LayoutDimension::Logical(0.0)),
        );
        self
    }

    /// Fills both axes while allowing flex layout to shrink the element below
    /// its intrinsic content size.
    #[must_use]
    pub fn fill_size(self) -> Self {
        self.fill_width().fill_height()
    }

    /// Sets a fixed logical-pixel minimum width and height.
    #[must_use]
    pub fn min_logical_size(mut self, width: f64, height: f64) -> Self {
        self.size.merge_from(LayoutSize::min_logical(width, height));
        self
    }

    /// Sets a fixed logical-pixel maximum width and height.
    #[must_use]
    pub fn max_logical_size(mut self, width: f64, height: f64) -> Self {
        self.size.merge_from(LayoutSize::max_logical(width, height));
        self
    }

    /// Sets the flex basis contribution.
    #[must_use]
    pub fn flex_basis(mut self, basis: LayoutDimension) -> Self {
        self.size.merge_from(LayoutSize::new().flex_basis(basis));
        self
    }

    /// Sets equal margins on all four sides.
    #[must_use]
    pub fn margin_all(mut self, value: f64) -> Self {
        self.spacing.merge_from(LayoutSpacing::margin_all(value));
        self
    }

    /// Sets horizontal and vertical margins.
    #[must_use]
    pub fn margin_xy(mut self, horizontal: f64, vertical: f64) -> Self {
        self.spacing
            .merge_from(LayoutSpacing::margin_xy(horizontal, vertical));
        self
    }

    /// Centers children on the cross axis.
    #[must_use]
    pub fn align_center(mut self) -> Self {
        self.alignment
            .merge_from(LayoutAlignment::new().align_items(LayoutAlign::Center));
        self
    }

    /// Stretches this element in its parent's cross axis.
    #[must_use]
    pub fn align_self_stretch(mut self) -> Self {
        self.alignment
            .merge_from(LayoutAlignment::new().align_self(LayoutAlign::Stretch));
        self
    }

    /// Packs children at the start of the main axis.
    #[must_use]
    pub fn justify_start(mut self) -> Self {
        self.alignment
            .merge_from(LayoutAlignment::new().justify_content(LayoutJustify::Start));
        self
    }

    /// Centers children on the main axis.
    #[must_use]
    pub fn justify_center(mut self) -> Self {
        self.alignment
            .merge_from(LayoutAlignment::new().justify_content(LayoutJustify::Center));
        self
    }

    /// Packs children at the end of the main axis.
    #[must_use]
    pub fn justify_end(mut self) -> Self {
        self.alignment
            .merge_from(LayoutAlignment::new().justify_content(LayoutJustify::End));
        self
    }

    /// Sets the flex grow factor.
    #[must_use]
    pub fn flex_grow(mut self, grow: f64) -> Self {
        self.size.merge_from(LayoutSize::new().flex_grow(grow));
        self
    }

    /// Sets the flex shrink factor.
    ///
    /// Use `0.0` for fixed-size content that should overflow a smaller
    /// viewport instead of being compressed by flex layout.
    #[must_use]
    pub fn flex_shrink(mut self, shrink: f64) -> Self {
        self.size.merge_from(LayoutSize::new().flex_shrink(shrink));
        self
    }

    /// Allows flex children to wrap onto additional lines.
    #[must_use]
    pub fn wrap(mut self) -> Self {
        self.container
            .merge_from(LayoutContainer::new().wrap(LayoutWrap::Wrap));
        self
    }

    /// Allows flex children to wrap onto additional lines in reverse
    /// cross-axis order.
    #[must_use]
    pub fn wrap_reverse(mut self) -> Self {
        self.container
            .merge_from(LayoutContainer::new().wrap(LayoutWrap::WrapReverse));
        self
    }

    /// Adds or replaces a container-layout longhand contribution.
    #[must_use]
    pub fn container(mut self, container: LayoutContainer) -> Self {
        self.container.merge_from(container);
        self
    }

    /// Adds or replaces a size longhand contribution.
    #[must_use]
    pub fn size(mut self, size: LayoutSize) -> Self {
        self.size.merge_from(size);
        self
    }

    /// Adds or replaces a spacing longhand contribution.
    #[must_use]
    pub fn spacing(mut self, spacing: LayoutSpacing) -> Self {
        self.spacing.merge_from(spacing);
        self
    }

    /// Adds or replaces an alignment longhand contribution.
    #[must_use]
    pub fn alignment(mut self, alignment: LayoutAlignment) -> Self {
        self.alignment.merge_from(alignment);
        self
    }

    /// Adds or replaces an overflow longhand contribution.
    #[must_use]
    pub fn overflow(mut self, overflow: LayoutOverflow) -> Self {
        self.overflow.merge_from(overflow);
        self
    }

    /// Returns this shorthand's placement contribution.
    #[must_use]
    pub const fn placement(&self) -> LayoutPlacement {
        self.placement
    }

    pub(crate) fn inspection_summary(&self) -> String {
        let parts = self.clone().into_parts();
        let mut summaries = Vec::new();
        if let Some(container) = parts.container {
            push_labeled_summary_part(&mut summaries, "container:", container.inspection_summary());
        }
        if let Some(size) = parts.size {
            push_labeled_summary_part(&mut summaries, "size:", size.inspection_summary());
        }
        if let Some(spacing) = parts.spacing {
            push_labeled_summary_part(&mut summaries, "spacing:", spacing.inspection_summary());
        }
        if let Some(alignment) = parts.alignment {
            push_labeled_summary_part(&mut summaries, "alignment:", alignment.inspection_summary());
        }
        if let Some(overflow) = parts.overflow {
            push_labeled_summary_part(&mut summaries, "overflow:", overflow.inspection_summary());
        }
        if let Some(placement) = parts.placement {
            push_labeled_summary_part(&mut summaries, "placement:", placement.inspection_summary());
        }
        join_summary_parts(summaries, "; ")
    }

    pub(crate) fn inspection_fields(&self) -> Vec<(&'static str, String)> {
        let parts = self.clone().into_parts();
        let mut fields = Vec::new();
        if let Some(container) = parts.container {
            fields.push(("Container", container.inspection_summary()));
        }
        if let Some(size) = parts.size {
            fields.push(("Size", size.inspection_summary()));
        }
        if let Some(spacing) = parts.spacing {
            fields.push(("Spacing", spacing.inspection_summary()));
        }
        if let Some(alignment) = parts.alignment {
            fields.push(("Alignment", alignment.inspection_summary()));
        }
        if let Some(overflow) = parts.overflow {
            fields.push(("Overflow", overflow.inspection_summary()));
        }
        if let Some(placement) = parts.placement {
            fields.push(("Placement", placement.inspection_summary()));
        }
        fields
    }

    pub(crate) fn merge_container(&mut self, container: LayoutContainer) {
        self.container.merge_from(container);
    }

    pub(crate) fn merge_size(&mut self, size: LayoutSize) {
        self.size.merge_from(size);
    }

    pub(crate) fn merge_spacing(&mut self, spacing: LayoutSpacing) {
        self.spacing.merge_from(spacing);
    }

    pub(crate) fn merge_alignment(&mut self, alignment: LayoutAlignment) {
        self.alignment.merge_from(alignment);
    }

    pub(crate) fn merge_overflow(&mut self, overflow: LayoutOverflow) {
        self.overflow.merge_from(overflow);
    }

    pub(crate) fn merge_placement(&mut self, placement: LayoutPlacement) {
        if placement != LayoutPlacement::Auto {
            self.placement = placement;
        }
    }

    pub(crate) fn to_taffy(&self) -> taffy::Style {
        let mut style = taffy::Style::default();
        self.container.apply_to(&mut style);
        self.size.apply_to(&mut style);
        self.spacing.apply_to(&mut style);
        self.alignment.apply_to(&mut style);
        self.overflow.apply_to(&mut style);
        self.placement.apply_to(&mut style);
        style
    }

    pub(crate) fn into_parts(self) -> LayoutParts {
        LayoutParts {
            container: (!self.container.is_empty()).then_some(self.container),
            size: (!self.size.is_empty()).then_some(self.size),
            spacing: (!self.spacing.is_empty()).then_some(self.spacing),
            alignment: (!self.alignment.is_empty()).then_some(self.alignment),
            overflow: (!self.overflow.is_empty()).then_some(self.overflow),
            placement: match self.placement {
                LayoutPlacement::Auto => None,
                placement => Some(placement),
            },
        }
    }
}

/// Independent layout longhands carried by a [`Layout`] shorthand.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct LayoutParts {
    pub(crate) container: Option<LayoutContainer>,
    pub(crate) size: Option<LayoutSize>,
    pub(crate) spacing: Option<LayoutSpacing>,
    pub(crate) alignment: Option<LayoutAlignment>,
    pub(crate) overflow: Option<LayoutOverflow>,
    pub(crate) placement: Option<LayoutPlacement>,
}

/// Returns a horizontal flex row with a uniform child gap.
#[must_use]
pub fn row_with_gap(gap: f64) -> Layout {
    Layout::row(gap)
}

/// Returns a vertical flex column with a uniform child gap.
#[must_use]
pub fn column_with_gap(gap: f64) -> Layout {
    Layout::column(gap)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_width_sets_percent_width_and_zero_min_width() {
        let parts = Layout::new().fill_width().into_parts();

        assert_eq!(
            parts.size,
            Some(
                LayoutSize::new()
                    .width(LayoutDimension::Percent(1.0))
                    .min_width(LayoutDimension::Logical(0.0))
            )
        );
    }

    #[test]
    fn fill_height_sets_percent_height_and_zero_min_height() {
        let parts = Layout::new().fill_height().into_parts();

        assert_eq!(
            parts.size,
            Some(
                LayoutSize::new()
                    .height(LayoutDimension::Percent(1.0))
                    .min_height(LayoutDimension::Logical(0.0))
            )
        );
    }

    #[test]
    fn fill_size_sets_percent_size_and_zero_min_size() {
        let parts = Layout::new().fill_size().into_parts();

        assert_eq!(
            parts.size,
            Some(
                LayoutSize::new()
                    .width(LayoutDimension::Percent(1.0))
                    .height(LayoutDimension::Percent(1.0))
                    .min_width(LayoutDimension::Logical(0.0))
                    .min_height(LayoutDimension::Logical(0.0))
            )
        );
    }
}
