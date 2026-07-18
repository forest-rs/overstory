// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained scroll views, scrollbars, and scrollbar composition.
//!
//! Backend-independent metrics and command accounting live in the shared
//! scroll state. These controls own retained identity, layout integration,
//! accessibility projection, input policy, and presentation.

mod area;
mod bar;
mod events;
mod fixed_virtual;
mod ids;
pub(crate) mod runtime;
mod view;

#[cfg(test)]
mod tests;

pub use area::{
    ScrollArea, scroll_area_horizontal_bar, scroll_area_vertical_bar, scroll_area_viewport,
};
pub use bar::ScrollBar;
pub use events::ScrollOffsetChangedEvent;
pub use ids::{
    PART_SCROLL_BAR_THUMB, SLOT_SCROLL_BAR_THUMB, TYPE_SCROLL_AREA, TYPE_SCROLL_BAR,
    TYPE_SCROLL_VIEW,
};
pub use runtime::{
    scroll_offset_y, scroll_view_offset_y, set_scroll_offset_y, set_scroll_view_offset_y,
};
pub use view::ScrollView;

pub(crate) use bar::default_scroll_bar_template;
pub(crate) use fixed_virtual::FixedVirtualViewport;
pub(crate) use runtime::{normalize_scroll_offset_y, wheel_scroll_delta};
