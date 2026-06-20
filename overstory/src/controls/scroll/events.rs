// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::Vec2;

use crate::controls::support::{CallbackSlot, EventHandler};
use crate::element::ElementId;

pub(super) type ScrollOffsetChangedHandler = EventHandler<ScrollOffsetChangedEvent>;
pub(super) type ScrollOffsetChangedSlot = CallbackSlot<ScrollOffsetChangedEvent>;

/// Source of a scroll offset change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScrollOffsetChangeSource {
    /// Offset changed through pointer wheel or touchpad scrolling.
    Wheel,
    /// Offset changed through a scrollbar click or drag.
    ScrollBar,
    /// Offset changed through an accessibility action.
    Accessibility,
    /// Offset changed through [`set_scroll_view_offset_y`] or [`set_scroll_offset_y`].
    Programmatic,
}

/// Event delivered after a scrollable viewport offset changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollOffsetChangedEvent {
    /// The scroll viewport whose offset changed.
    pub scroll_view: ElementId,
    /// Offset before the change.
    pub previous_offset: Vec2,
    /// Offset after the change.
    pub offset: Vec2,
    /// Where the change came from.
    pub source: ScrollOffsetChangeSource,
}
