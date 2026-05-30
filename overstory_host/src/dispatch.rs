// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Event dispatch result types shared by platform hosts.

use overstory::{KeyDispatchResult, PointerDispatchResult};

/// Result of dispatching a platform-normalized event into a [`crate::UiHost`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HostDispatchResult {
    /// Whether any UI handler consumed or reacted to the event.
    pub handled: bool,
    /// Whether focus changed while dispatching the event.
    pub focus_changed: bool,
    /// Whether the platform host should request a redraw.
    pub needs_redraw: bool,
}

impl From<KeyDispatchResult> for HostDispatchResult {
    fn from(result: KeyDispatchResult) -> Self {
        Self {
            handled: result.handled,
            focus_changed: result.focus_changed,
            needs_redraw: result.handled || result.focus_changed,
        }
    }
}

impl From<PointerDispatchResult> for HostDispatchResult {
    fn from(result: PointerDispatchResult) -> Self {
        Self {
            handled: result.handled,
            focus_changed: result.focus_changed,
            needs_redraw: result.changed(),
        }
    }
}
