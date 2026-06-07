// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Internal scroll target registry and shared scroll state.
//!
//! Roadmap:
//! - Keep scroll offset and extent state target-owned while allowing controls,
//!   scrollbars, accessibility, and future virtualized widgets to bind by
//!   element id.
//! - Grow this into the common capability for scroll containers, list-like
//!   virtualization, text viewport scrolling, and compositor-backed scroll
//!   surfaces.
//! - Keep app code out of scroll-state plumbing: builders should expose initial
//!   intent, while target widgets and runtimes own live scrolling mechanics.

use alloc::rc::Rc;
use core::cell::Cell;

use invalidation::ChannelSet;
use kurbo::{Size, Vec2};

use crate::channels::ARRANGE;
use crate::element::ElementId;
use crate::element_side_map::ElementSideMap;

/// Registered scrollable target capability.
#[derive(Clone, Debug)]
pub(crate) struct ScrollTarget {
    state: ScrollState,
    offset_channels: ChannelSet,
}

impl ScrollTarget {
    pub(crate) fn new(state: ScrollState, offset_channels: ChannelSet) -> Self {
        Self {
            state,
            offset_channels,
        }
    }

    pub(crate) fn state(&self) -> ScrollState {
        self.state.clone()
    }

    pub(crate) fn offset_channels(&self) -> ChannelSet {
        self.offset_channels
    }

    pub(crate) fn offset_y(&self) -> f64 {
        self.state.offset_y()
    }
}

impl Default for ScrollTarget {
    fn default() -> Self {
        Self::new(ScrollState::new(), ARRANGE.into())
    }
}

/// Shared scroll offset state for viewport-like controls.
///
/// A scrollable widget owns one `ScrollState` and registers it against its
/// viewport element with [`Ui::register_scroll_target`](crate::Ui::register_scroll_target).
/// Scroll bars, wheel handlers, accessibility actions, and virtualized controls
/// then share one offset and one viewport/content extent instead of
/// maintaining side tables keyed by element id.
///
/// The setters are intentionally low-level: they update the shared state only.
/// Widgets that change the offset should also mark the target's registered
/// offset-invalidation channels so ordinary viewports can reuse layout results
/// while virtualized controls can rematerialize rows.
#[derive(Clone)]
pub struct ScrollState {
    inner: Rc<ScrollStateInner>,
}

struct ScrollStateInner {
    offset: Cell<Vec2>,
    metrics: Cell<ScrollMetrics>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct ScrollMetrics {
    viewport: Size,
    content: Size,
}

impl ScrollMetrics {
    fn new(viewport: Size, content: Size) -> Self {
        Self {
            viewport: Size::new(
                normalize_scroll_axis(viewport.width),
                normalize_scroll_axis(viewport.height),
            ),
            content: Size::new(
                normalize_scroll_axis(content.width),
                normalize_scroll_axis(content.height),
            ),
        }
    }

    fn max_offset(self) -> Vec2 {
        Vec2::new(
            (self.content.width - self.viewport.width).max(0.0),
            (self.content.height - self.viewport.height).max(0.0),
        )
    }
}

impl core::fmt::Debug for ScrollState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ScrollState")
            .field("offset", &self.offset())
            .finish()
    }
}

impl Default for ScrollState {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollState {
    /// Creates shared scroll state with a zero offset.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Rc::new(ScrollStateInner {
                offset: Cell::new(Vec2::ZERO),
                metrics: Cell::new(ScrollMetrics::default()),
            }),
        }
    }

    /// Creates shared scroll state with an initial vertical offset.
    #[must_use]
    pub fn with_offset_y(offset_y: f64) -> Self {
        let state = Self::new();
        state.set_offset_y_raw(offset_y);
        state
    }

    /// Returns the current scroll offset in logical pixels.
    #[must_use]
    pub fn offset(&self) -> Vec2 {
        self.inner.offset.get()
    }

    /// Returns the current vertical scroll offset in logical pixels.
    #[must_use]
    pub fn offset_y(&self) -> f64 {
        self.inner.offset.get().y
    }

    /// Returns the current maximum scroll offset in logical pixels.
    #[must_use]
    pub fn max_offset(&self) -> Vec2 {
        self.inner.metrics.get().max_offset()
    }

    /// Returns the current maximum vertical scroll offset in logical pixels.
    #[must_use]
    #[cfg(test)]
    pub(crate) fn max_offset_y(&self) -> f64 {
        self.max_offset().y
    }

    /// Returns the latest viewport size in logical pixels.
    #[must_use]
    pub fn viewport_size(&self) -> Size {
        self.inner.metrics.get().viewport
    }

    /// Returns the latest scrollable content size in logical pixels.
    #[must_use]
    #[cfg(test)]
    pub(crate) fn content_size(&self) -> Size {
        self.inner.metrics.get().content
    }

    /// Sets the current logical scroll offset without clamping to metrics.
    ///
    /// This is primarily for viewport widgets that already used their own
    /// virtualization model to clamp the offset. General callers should prefer
    /// higher-level helpers such as [`crate::set_scroll_offset_y`].
    ///
    /// Returns `true` when the stored offset changed.
    pub fn set_offset_raw(&self, offset: Vec2) -> bool {
        let offset = Vec2::new(
            normalize_scroll_axis(offset.x),
            normalize_scroll_axis(offset.y),
        );
        let current = self.inner.offset.get();
        if (current.x - offset.x).abs() <= f64::EPSILON
            && (current.y - offset.y).abs() <= f64::EPSILON
        {
            return false;
        }
        self.inner.offset.set(offset);
        true
    }

    /// Sets the current vertical scroll offset without clamping to metrics.
    ///
    /// Returns `true` when the stored offset changed.
    pub fn set_offset_y_raw(&self, offset_y: f64) -> bool {
        self.set_offset_raw(Vec2::new(self.inner.offset.get().x, offset_y))
    }

    /// Replaces the latest viewport/content metrics in logical pixels.
    ///
    /// Virtualized controls should set this after recomputing their backing
    /// extent so scrollbars and accessibility can report the same range as the
    /// widget's own realization model.
    pub fn set_metrics(&self, viewport: Size, content: Size) {
        self.inner
            .metrics
            .set(ScrollMetrics::new(viewport, content));
    }
}

/// Normalizes a logical scroll coordinate to a finite non-negative value.
#[must_use]
pub(crate) fn normalize_scroll_axis(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

/// Registry of elements that expose scroll state to other systems.
#[derive(Debug, Default)]
pub(crate) struct ScrollRuntime {
    targets: ElementSideMap<ScrollTarget>,
}

impl ScrollRuntime {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn register_target(&mut self, target: ElementId, scroll_target: ScrollTarget) {
        self.targets.insert(target, scroll_target);
    }

    pub(crate) fn target(&self, target: ElementId) -> Option<ScrollTarget> {
        self.targets.get(target).cloned()
    }

    pub(crate) fn remove_target(&mut self, target: ElementId) {
        self.targets.remove(target);
    }
}
