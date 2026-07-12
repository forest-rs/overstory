// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `ScrollView`: reusable clipped viewport state and behavior.
//!
//! Roadmap:
//! - Make `ScrollView` the reusable basis for lists, text fields, graph
//!   viewports, and any control that needs clipped, offset content.
//! - Support both axes consistently: horizontal and vertical bars, overlay vs
//!   reserved gutters, keyboard paging, wheel / touchpad deltas, and
//!   bring-into-view requests.
//! - Keep scroll metrics, clamping, thumb math, and shared scroll state here so
//!   controls do not each invent their own scrolling model.
//! - Integrate with virtualization and compositor-backed scrolling as separate
//!   policy layers instead of baking renderer decisions into the control.
//! - Add accessibility range metadata and platform scroll actions through the
//!   host layer.

mod area;
mod bar;
mod events;
mod ids;
pub(crate) mod runtime;
mod view;

#[cfg(test)]
mod tests;

pub use area::{
    ScrollArea, scroll_area_horizontal_bar, scroll_area_vertical_bar, scroll_area_viewport,
};
pub use bar::ScrollBar;
pub use events::{ScrollOffsetChangeSource, ScrollOffsetChangedEvent};
pub use ids::{
    PART_SCROLL_BAR_THUMB, SLOT_SCROLL_BAR_THUMB, TYPE_SCROLL_AREA, TYPE_SCROLL_BAR,
    TYPE_SCROLL_VIEW,
};
pub use runtime::{
    scroll_offset_y, scroll_view_offset_y, set_scroll_offset_y, set_scroll_view_offset_y,
};
pub use view::ScrollView;

pub(crate) use bar::default_scroll_bar_template;
pub(crate) use runtime::{normalize_scroll_offset_y, state_scroll_offset_y, wheel_scroll_delta};
