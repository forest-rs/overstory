// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::Vec2;

use crate::controls::support::{CallbackSlot, EventHandler};
use crate::element::ElementId;
use crate::scroll_runtime::ScrollInputSource;

pub(super) type ScrollOffsetChangedHandler = EventHandler<ScrollOffsetChangedEvent>;
pub(super) type ScrollOffsetChangedSlot = CallbackSlot<ScrollOffsetChangedEvent>;

/// Event delivered after a scrollable viewport offset changes.
///
/// This framework-created payload is non-exhaustive. Callers migrating
/// exhaustive patterns should include `..`.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct ScrollOffsetChangedEvent {
    /// The scroll viewport whose offset changed.
    pub scroll_view: ElementId,
    /// Offset before the change.
    pub previous_offset: Vec2,
    /// Offset after the change.
    pub offset: Vec2,
    /// Where the change came from.
    pub source: ScrollInputSource,
}
