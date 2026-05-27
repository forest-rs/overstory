// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::LayoutDimension;

/// Child layout strategy for an element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutDisplay {
    /// Flexbox layout.
    Flex,
    /// The element and its descendants do not generate layout boxes.
    None,
}

impl LayoutDisplay {
    fn to_taffy(self) -> taffy::Display {
        match self {
            Self::Flex => taffy::Display::Flex,
            Self::None => taffy::Display::None,
        }
    }
}

/// Main-axis direction for flex layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutDirection {
    /// Children flow horizontally.
    Row,
    /// Children flow vertically.
    Column,
}

impl LayoutDirection {
    fn to_taffy(self) -> taffy::FlexDirection {
        match self {
            Self::Row => taffy::FlexDirection::Row,
            Self::Column => taffy::FlexDirection::Column,
        }
    }
}

/// Cross-axis alignment mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutAlign {
    /// Pack toward the start of the axis.
    Start,
    /// Pack at the center of the axis.
    Center,
    /// Pack toward the end of the axis.
    End,
    /// Stretch to fill the available cross-axis space.
    Stretch,
}

impl LayoutAlign {
    fn to_taffy(self) -> taffy::AlignItems {
        match self {
            Self::Start => taffy::AlignItems::Start,
            Self::Center => taffy::AlignItems::Center,
            Self::End => taffy::AlignItems::End,
            Self::Stretch => taffy::AlignItems::Stretch,
        }
    }
}

/// Main-axis content distribution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutJustify {
    /// Pack children at the start of the main axis.
    Start,
    /// Center children on the main axis.
    Center,
    /// Pack children at the end of the main axis.
    End,
}

impl LayoutJustify {
    fn to_taffy(self) -> taffy::JustifyContent {
        match self {
            Self::Start => taffy::JustifyContent::Start,
            Self::Center => taffy::JustifyContent::Center,
            Self::End => taffy::JustifyContent::End,
        }
    }
}

/// Overflow contribution for one axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutOverflowMode {
    /// Overflow contributes normally.
    Visible,
    /// Clip overflow without becoming a scroll container.
    Clip,
    /// Hide overflow and use zero automatic minimum size.
    Hidden,
    /// Reserve scrollbar layout space and use zero automatic minimum size.
    Scroll,
}

impl LayoutOverflowMode {
    fn to_taffy(self) -> taffy::Overflow {
        match self {
            Self::Visible => taffy::Overflow::Visible,
            Self::Clip => taffy::Overflow::Clip,
            Self::Hidden => taffy::Overflow::Hidden,
            Self::Scroll => taffy::Overflow::Scroll,
        }
    }
}

/// Container-layout contribution for an element.
///
/// This covers the layout algorithm and its main-axis direction. Size, spacing,
/// placement, alignment, and overflow are separate longhands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayoutContainer {
    display: Option<LayoutDisplay>,
    direction: Option<LayoutDirection>,
}

impl LayoutContainer {
    /// Creates an empty container contribution.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            display: None,
            direction: None,
        }
    }

    /// Creates a flex container contribution.
    #[must_use]
    pub const fn flex() -> Self {
        Self::new().display(LayoutDisplay::Flex)
    }

    /// Creates a horizontal flex-row contribution.
    #[must_use]
    pub const fn row() -> Self {
        Self::flex().direction(LayoutDirection::Row)
    }

    /// Creates a vertical flex-column contribution.
    #[must_use]
    pub const fn column() -> Self {
        Self::flex().direction(LayoutDirection::Column)
    }

    /// Sets the child layout strategy.
    #[must_use]
    pub const fn display(mut self, display: LayoutDisplay) -> Self {
        self.display = Some(display);
        self
    }

    /// Sets the flex main-axis direction.
    #[must_use]
    pub const fn direction(mut self, direction: LayoutDirection) -> Self {
        self.direction = Some(direction);
        self
    }

    pub(super) fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    pub(super) fn merge_from(&mut self, other: Self) {
        if other.display.is_some() {
            self.display = other.display;
        }
        if other.direction.is_some() {
            self.direction = other.direction;
        }
    }

    pub(super) fn apply_to(&self, style: &mut taffy::Style) {
        if let Some(display) = self.display {
            style.display = display.to_taffy();
        }
        if let Some(direction) = self.direction {
            style.flex_direction = direction.to_taffy();
        }
    }
}

/// Size contribution for an element.
///
/// This covers explicit dimensions, min/max constraints, flex basis, and flex
/// grow/shrink factors.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayoutSize {
    width: Option<LayoutDimension>,
    height: Option<LayoutDimension>,
    min_width: Option<LayoutDimension>,
    min_height: Option<LayoutDimension>,
    max_width: Option<LayoutDimension>,
    max_height: Option<LayoutDimension>,
    flex_basis: Option<LayoutDimension>,
    flex_grow: Option<f32>,
    flex_shrink: Option<f32>,
}

impl LayoutSize {
    /// Creates an empty size contribution.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            width: None,
            height: None,
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
            flex_basis: None,
            flex_grow: None,
            flex_shrink: None,
        }
    }

    /// Creates a fixed logical-pixel size contribution.
    #[must_use]
    pub const fn logical(width: f32, height: f32) -> Self {
        Self::new()
            .width(LayoutDimension::Logical(width))
            .height(LayoutDimension::Logical(height))
    }

    /// Creates a percent-of-containing-block size contribution.
    #[must_use]
    pub const fn percent(width: f32, height: f32) -> Self {
        Self::new()
            .width(LayoutDimension::Percent(width))
            .height(LayoutDimension::Percent(height))
    }

    /// Creates a fixed logical-pixel minimum-size contribution.
    #[must_use]
    pub const fn min_logical(width: f32, height: f32) -> Self {
        Self::new()
            .min_width(LayoutDimension::Logical(width))
            .min_height(LayoutDimension::Logical(height))
    }

    /// Creates a fixed logical-pixel maximum-size contribution.
    #[must_use]
    pub const fn max_logical(width: f32, height: f32) -> Self {
        Self::new()
            .max_width(LayoutDimension::Logical(width))
            .max_height(LayoutDimension::Logical(height))
    }

    /// Sets the width contribution.
    #[must_use]
    pub const fn width(mut self, width: LayoutDimension) -> Self {
        self.width = Some(width);
        self
    }

    /// Sets the height contribution.
    #[must_use]
    pub const fn height(mut self, height: LayoutDimension) -> Self {
        self.height = Some(height);
        self
    }

    /// Sets the minimum width contribution.
    #[must_use]
    pub const fn min_width(mut self, min_width: LayoutDimension) -> Self {
        self.min_width = Some(min_width);
        self
    }

    /// Sets the minimum height contribution.
    #[must_use]
    pub const fn min_height(mut self, min_height: LayoutDimension) -> Self {
        self.min_height = Some(min_height);
        self
    }

    /// Sets the maximum width contribution.
    #[must_use]
    pub const fn max_width(mut self, max_width: LayoutDimension) -> Self {
        self.max_width = Some(max_width);
        self
    }

    /// Sets the maximum height contribution.
    #[must_use]
    pub const fn max_height(mut self, max_height: LayoutDimension) -> Self {
        self.max_height = Some(max_height);
        self
    }

    /// Sets the flex basis contribution.
    #[must_use]
    pub const fn flex_basis(mut self, basis: LayoutDimension) -> Self {
        self.flex_basis = Some(basis);
        self
    }

    /// Sets the flex grow factor contribution.
    #[must_use]
    pub const fn flex_grow(mut self, grow: f32) -> Self {
        self.flex_grow = Some(grow);
        self
    }

    /// Sets the flex shrink factor contribution.
    #[must_use]
    pub const fn flex_shrink(mut self, shrink: f32) -> Self {
        self.flex_shrink = Some(shrink);
        self
    }

    pub(super) fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    pub(super) fn merge_from(&mut self, other: Self) {
        if other.width.is_some() {
            self.width = other.width;
        }
        if other.height.is_some() {
            self.height = other.height;
        }
        if other.min_width.is_some() {
            self.min_width = other.min_width;
        }
        if other.min_height.is_some() {
            self.min_height = other.min_height;
        }
        if other.max_width.is_some() {
            self.max_width = other.max_width;
        }
        if other.max_height.is_some() {
            self.max_height = other.max_height;
        }
        if other.flex_basis.is_some() {
            self.flex_basis = other.flex_basis;
        }
        if other.flex_grow.is_some() {
            self.flex_grow = other.flex_grow;
        }
        if other.flex_shrink.is_some() {
            self.flex_shrink = other.flex_shrink;
        }
    }

    pub(super) fn apply_to(&self, style: &mut taffy::Style) {
        if let Some(width) = self.width {
            style.size.width = width.to_taffy();
        }
        if let Some(height) = self.height {
            style.size.height = height.to_taffy();
        }
        if let Some(min_width) = self.min_width {
            style.min_size.width = min_width.to_taffy();
        }
        if let Some(min_height) = self.min_height {
            style.min_size.height = min_height.to_taffy();
        }
        if let Some(max_width) = self.max_width {
            style.max_size.width = max_width.to_taffy();
        }
        if let Some(max_height) = self.max_height {
            style.max_size.height = max_height.to_taffy();
        }
        if let Some(flex_basis) = self.flex_basis {
            style.flex_basis = flex_basis.to_taffy();
        }
        if let Some(flex_grow) = self.flex_grow {
            style.flex_grow = flex_grow;
        }
        if let Some(flex_shrink) = self.flex_shrink {
            style.flex_shrink = flex_shrink;
        }
    }
}

/// Logical rectangle edge values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutEdges {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

impl LayoutEdges {
    /// Creates edge values with every side set to `value`.
    #[must_use]
    pub const fn all(value: f32) -> Self {
        Self {
            left: value,
            right: value,
            top: value,
            bottom: value,
        }
    }

    /// Creates edge values from horizontal and vertical pairs.
    #[must_use]
    pub const fn xy(horizontal: f32, vertical: f32) -> Self {
        Self {
            left: horizontal,
            right: horizontal,
            top: vertical,
            bottom: vertical,
        }
    }

    fn to_taffy_length_percentage(self) -> taffy::Rect<taffy::LengthPercentage> {
        taffy::Rect {
            left: taffy::LengthPercentage::length(self.left),
            right: taffy::LengthPercentage::length(self.right),
            top: taffy::LengthPercentage::length(self.top),
            bottom: taffy::LengthPercentage::length(self.bottom),
        }
    }

    fn to_taffy_length_percentage_auto(self) -> taffy::Rect<taffy::LengthPercentageAuto> {
        taffy::Rect {
            left: taffy::LengthPercentageAuto::length(self.left),
            right: taffy::LengthPercentageAuto::length(self.right),
            top: taffy::LengthPercentageAuto::length(self.top),
            bottom: taffy::LengthPercentageAuto::length(self.bottom),
        }
    }
}

/// Row and column gap contribution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutGap {
    horizontal: f32,
    vertical: f32,
}

impl LayoutGap {
    /// Creates equal horizontal and vertical gaps.
    #[must_use]
    pub const fn all(value: f32) -> Self {
        Self {
            horizontal: value,
            vertical: value,
        }
    }

    fn to_taffy(self) -> taffy::Size<taffy::LengthPercentage> {
        taffy::Size {
            width: taffy::LengthPercentage::length(self.horizontal),
            height: taffy::LengthPercentage::length(self.vertical),
        }
    }
}

/// Spacing contribution for an element.
///
/// This covers margins, padding, and child gaps.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayoutSpacing {
    margin: Option<LayoutEdges>,
    padding: Option<LayoutEdges>,
    gap: Option<LayoutGap>,
}

impl LayoutSpacing {
    /// Creates an empty spacing contribution.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            margin: None,
            padding: None,
            gap: None,
        }
    }

    /// Creates a uniform margin contribution.
    #[must_use]
    pub const fn margin_all(value: f32) -> Self {
        Self::new().margin(LayoutEdges::all(value))
    }

    /// Creates a horizontal/vertical margin contribution.
    #[must_use]
    pub const fn margin_xy(horizontal: f32, vertical: f32) -> Self {
        Self::new().margin(LayoutEdges::xy(horizontal, vertical))
    }

    /// Creates a uniform padding contribution.
    #[must_use]
    pub const fn padding_all(value: f32) -> Self {
        Self::new().padding(LayoutEdges::all(value))
    }

    /// Creates a horizontal/vertical padding contribution.
    #[must_use]
    pub const fn padding_xy(horizontal: f32, vertical: f32) -> Self {
        Self::new().padding(LayoutEdges::xy(horizontal, vertical))
    }

    /// Creates a uniform child-gap contribution.
    #[must_use]
    pub const fn gap(value: f32) -> Self {
        Self::new().with_gap(LayoutGap::all(value))
    }

    /// Sets the margin contribution.
    #[must_use]
    pub const fn margin(mut self, margin: LayoutEdges) -> Self {
        self.margin = Some(margin);
        self
    }

    /// Sets the padding contribution.
    #[must_use]
    pub const fn padding(mut self, padding: LayoutEdges) -> Self {
        self.padding = Some(padding);
        self
    }

    /// Sets the child-gap contribution.
    #[must_use]
    pub const fn with_gap(mut self, gap: LayoutGap) -> Self {
        self.gap = Some(gap);
        self
    }

    pub(super) fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    pub(super) fn merge_from(&mut self, other: Self) {
        if other.margin.is_some() {
            self.margin = other.margin;
        }
        if other.padding.is_some() {
            self.padding = other.padding;
        }
        if other.gap.is_some() {
            self.gap = other.gap;
        }
    }

    pub(super) fn apply_to(&self, style: &mut taffy::Style) {
        if let Some(margin) = self.margin {
            style.margin = margin.to_taffy_length_percentage_auto();
        }
        if let Some(padding) = self.padding {
            style.padding = padding.to_taffy_length_percentage();
        }
        if let Some(gap) = self.gap {
            style.gap = gap.to_taffy();
        }
    }
}

/// Alignment contribution for an element.
///
/// This covers child alignment and this element's cross-axis alignment within
/// its parent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayoutAlignment {
    align_items: Option<LayoutAlign>,
    align_self: Option<LayoutAlign>,
    justify_content: Option<LayoutJustify>,
}

impl LayoutAlignment {
    /// Creates an empty alignment contribution.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            align_items: None,
            align_self: None,
            justify_content: None,
        }
    }

    /// Sets child cross-axis alignment.
    #[must_use]
    pub const fn align_items(mut self, align: LayoutAlign) -> Self {
        self.align_items = Some(align);
        self
    }

    /// Sets this element's cross-axis alignment in its parent.
    #[must_use]
    pub const fn align_self(mut self, align: LayoutAlign) -> Self {
        self.align_self = Some(align);
        self
    }

    /// Sets child main-axis distribution.
    #[must_use]
    pub const fn justify_content(mut self, justify: LayoutJustify) -> Self {
        self.justify_content = Some(justify);
        self
    }

    pub(super) fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    pub(super) fn merge_from(&mut self, other: Self) {
        if other.align_items.is_some() {
            self.align_items = other.align_items;
        }
        if other.align_self.is_some() {
            self.align_self = other.align_self;
        }
        if other.justify_content.is_some() {
            self.justify_content = other.justify_content;
        }
    }

    pub(super) fn apply_to(&self, style: &mut taffy::Style) {
        if let Some(align_items) = self.align_items {
            style.align_items = Some(align_items.to_taffy());
        }
        if let Some(align_self) = self.align_self {
            style.align_self = Some(align_self.to_taffy());
        }
        if let Some(justify_content) = self.justify_content {
            style.justify_content = Some(justify_content.to_taffy());
        }
    }
}

/// Overflow contribution for an element.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayoutOverflow {
    x: Option<LayoutOverflowMode>,
    y: Option<LayoutOverflowMode>,
}

impl LayoutOverflow {
    /// Creates an empty overflow contribution.
    #[must_use]
    pub const fn new() -> Self {
        Self { x: None, y: None }
    }

    /// Creates an overflow contribution that applies to both axes.
    #[must_use]
    pub const fn all(mode: LayoutOverflowMode) -> Self {
        Self::new().x(mode).y(mode)
    }

    /// Sets horizontal overflow.
    #[must_use]
    pub const fn x(mut self, mode: LayoutOverflowMode) -> Self {
        self.x = Some(mode);
        self
    }

    /// Sets vertical overflow.
    #[must_use]
    pub const fn y(mut self, mode: LayoutOverflowMode) -> Self {
        self.y = Some(mode);
        self
    }

    pub(super) fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    pub(super) fn merge_from(&mut self, other: Self) {
        if other.x.is_some() {
            self.x = other.x;
        }
        if other.y.is_some() {
            self.y = other.y;
        }
    }

    pub(super) fn apply_to(&self, style: &mut taffy::Style) {
        if let Some(x) = self.x {
            style.overflow.x = x.to_taffy();
        }
        if let Some(y) = self.y {
            style.overflow.y = y.to_taffy();
        }
    }
}
