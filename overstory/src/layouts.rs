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
    fn to_taffy(self) -> taffy::Dimension {
        match self {
            Self::Auto => taffy::Dimension::auto(),
            Self::Logical(value) => taffy::Dimension::length(value),
            Self::Percent(value) => taffy::Dimension::percent(value),
        }
    }
}

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

    fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    fn merge_from(&mut self, other: Self) {
        if other.display.is_some() {
            self.display = other.display;
        }
        if other.direction.is_some() {
            self.direction = other.direction;
        }
    }

    fn apply_to(&self, style: &mut taffy::Style) {
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
/// This covers explicit dimensions and flex item grow/shrink factors. Min/max
/// constraints and basis are intentionally left for a later API pass.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayoutSize {
    width: Option<LayoutDimension>,
    height: Option<LayoutDimension>,
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

    fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    fn merge_from(&mut self, other: Self) {
        if other.width.is_some() {
            self.width = other.width;
        }
        if other.height.is_some() {
            self.height = other.height;
        }
        if other.flex_grow.is_some() {
            self.flex_grow = other.flex_grow;
        }
        if other.flex_shrink.is_some() {
            self.flex_shrink = other.flex_shrink;
        }
    }

    fn apply_to(&self, style: &mut taffy::Style) {
        if let Some(width) = self.width {
            style.size.width = width.to_taffy();
        }
        if let Some(height) = self.height {
            style.size.height = height.to_taffy();
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
/// This currently covers padding and child gaps. Margins should be added here
/// when Overstory has a clear margin API.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayoutSpacing {
    padding: Option<LayoutEdges>,
    gap: Option<LayoutGap>,
}

impl LayoutSpacing {
    /// Creates an empty spacing contribution.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            padding: None,
            gap: None,
        }
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

    fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    fn merge_from(&mut self, other: Self) {
        if other.padding.is_some() {
            self.padding = other.padding;
        }
        if other.gap.is_some() {
            self.gap = other.gap;
        }
    }

    fn apply_to(&self, style: &mut taffy::Style) {
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

    fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    fn merge_from(&mut self, other: Self) {
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

    fn apply_to(&self, style: &mut taffy::Style) {
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

    fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    fn merge_from(&mut self, other: Self) {
        if other.x.is_some() {
            self.x = other.x;
        }
        if other.y.is_some() {
            self.y = other.y;
        }
    }

    fn apply_to(&self, style: &mut taffy::Style) {
        if let Some(x) = self.x {
            style.overflow.x = x.to_taffy();
        }
        if let Some(y) = self.y {
            style.overflow.y = y.to_taffy();
        }
    }
}

/// Layout contribution for an element.
///
/// `Layout` is now a shorthand builder over independent layout longhands:
/// container, size, spacing, alignment, overflow, and placement. Callers use
/// this compact type for ergonomic construction; `Ui::set_layout` splits it
/// into the longhand properties that it explicitly carries.
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

    /// Creates a horizontal flex row with a uniform child gap.
    #[must_use]
    pub fn row(gap: f32) -> Self {
        Self::new()
            .container(LayoutContainer::row())
            .spacing(LayoutSpacing::gap(gap))
    }

    /// Creates a vertical flex column with a uniform child gap.
    #[must_use]
    pub fn column(gap: f32) -> Self {
        Self::new()
            .container(LayoutContainer::column())
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
    pub fn absolute_at(mut self, left: f32, top: f32) -> Self {
        self.placement = LayoutPlacement::AbsoluteAt { left, top };
        self
    }

    /// Sets a fixed logical-pixel width and height.
    #[must_use]
    pub fn logical_size(mut self, width: f32, height: f32) -> Self {
        self.size.merge_from(LayoutSize::logical(width, height));
        self
    }

    /// Sets a width and height as fractions of the containing block.
    #[must_use]
    pub fn percent_size(mut self, width: f32, height: f32) -> Self {
        self.size.merge_from(LayoutSize::percent(width, height));
        self
    }

    /// Sets equal padding on all four sides.
    #[must_use]
    pub fn padding_all(mut self, value: f32) -> Self {
        self.spacing.merge_from(LayoutSpacing::padding_all(value));
        self
    }

    /// Sets horizontal and vertical padding.
    #[must_use]
    pub fn padding_xy(mut self, horizontal: f32, vertical: f32) -> Self {
        self.spacing
            .merge_from(LayoutSpacing::padding_xy(horizontal, vertical));
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
    pub fn flex_grow(mut self, grow: f32) -> Self {
        self.size.merge_from(LayoutSize::new().flex_grow(grow));
        self
    }

    /// Sets the flex shrink factor.
    ///
    /// Use `0.0` for fixed-size content that should overflow a smaller
    /// viewport instead of being compressed by flex layout.
    #[must_use]
    pub fn flex_shrink(mut self, shrink: f32) -> Self {
        self.size.merge_from(LayoutSize::new().flex_shrink(shrink));
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
pub fn row_with_gap(gap: f32) -> Layout {
    Layout::row(gap)
}

/// Returns a vertical flex column with a uniform child gap.
#[must_use]
pub fn column_with_gap(gap: f32) -> Layout {
    Layout::column(gap)
}

fn auto_insets() -> taffy::Rect<taffy::LengthPercentageAuto> {
    taffy::Rect {
        left: taffy::LengthPercentageAuto::auto(),
        right: taffy::LengthPercentageAuto::auto(),
        top: taffy::LengthPercentageAuto::auto(),
        bottom: taffy::LengthPercentageAuto::auto(),
    }
}
