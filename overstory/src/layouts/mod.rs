// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Overstory-owned layout intent.
//!
//! The public API deliberately exposes a compact [`Layout`] builder instead of
//! the full underlying layout engine. Add named methods here when Overstory
//! needs to support a new layout concept; do not mirror every engine type.
//! Padding is not a layout authoring concept: use
//! [`SurfaceSpecExt::padding_all`](crate::SurfaceSpecExt::padding_all) or
//! [`Ui::set_box_padding`](crate::Ui::set_box_padding) so layout and box
//! decoration geometry share one source of truth.
//!
//! ```ignore
//! use overstory::{Layout, Panel};
//!
//! ui.append(
//!     parent,
//!     Panel::new()
//!         .padding_all(12.0)
//!         .layout(Layout::row(24.0).align_center().justify_center()),
//! );
//! ```
//!
//! Roadmap:
//! - Keep `Layout` as Overstory intent, with Taffy as an implementation detail
//!   hidden behind conversion during layout. See ticket `ove-9tcf`.
//! - Keep box padding canonical in `Surface.Padding*`; layout consumes the
//!   resolved inset but does not expose padding as an authoring shorthand.
//! - Add builder methods only for concepts the toolkit wants to support long
//!   term: min/max constraints, margins, flex basis, grid, overflow, and
//!   alignment should be deliberate API choices.
//! - Preserve escape-hatch pressure by improving Overstory types instead of
//!   exposing raw layout-engine types in public control APIs.
//! - Coordinate layout vocabulary with `kurbo::Axis`, scrolling, splitters, and
//!   viewport controls so axis-specific code stays small.

mod dimension;
mod direction;
mod inspection;
mod longhands;
mod placement;
mod shorthand;

#[expect(
    clippy::cast_possible_truncation,
    reason = "layout backend lowering is the f64-to-f32 boundary"
)]
pub(crate) fn taffy_scalar(value: f64) -> f32 {
    value as f32
}

pub use dimension::LayoutDimension;
pub use direction::LayoutDirection;
pub use longhands::{
    FlexDirection, LayoutAlign, LayoutAlignment, LayoutContainer, LayoutDisplay, LayoutEdges,
    LayoutGap, LayoutJustify, LayoutOverflow, LayoutOverflowMode, LayoutSize, LayoutSpacing,
    LayoutWrap,
};
pub use placement::LayoutPlacement;
pub use shorthand::{Layout, column_with_gap, row_with_gap};
