// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared scroll state and Overstory's retained scroll-target registry.
//!
//! The state owns backend-independent metrics, offset constraint once metrics
//! are known, and delta accounting. The registry owns retained element
//! identity and invalidation policy. Event routing, virtualization,
//! accessibility projection, scrollbar rendering, and motion policy remain
//! outside this module.

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

/// Source of a scroll command or offset-changed event.
///
/// This enum is non-exhaustive so new input mechanisms do not require
/// downstream exhaustive matches. This replaces the earlier exhaustive
/// `ScrollOffsetChangeSource`; migrating matches should include a wildcard
/// arm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ScrollInputSource {
    /// Command issued through pointer wheel or touchpad scrolling.
    Wheel,
    /// Command issued through a scrollbar click or drag.
    ScrollBar,
    /// Command issued through an accessibility action.
    Accessibility,
    /// Command issued while revealing keyboard navigation.
    Keyboard,
    /// Command issued through a direct programmatic request.
    Programmatic,
}

/// Result of one constrained scroll command.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct ScrollCommandResult {
    /// Input mechanism that issued the command.
    pub source: ScrollInputSource,
    /// Offset before the command was applied.
    pub previous_offset: Vec2,
    /// Requested movement from the previous offset.
    pub requested_delta: Vec2,
    /// Constrained offset after the command was applied.
    pub offset: Vec2,
    /// Movement applied after constraining the request to the scroll range.
    pub consumed_delta: Vec2,
    /// Movement left for an ancestor viewport or other boundary policy.
    pub unconsumed_delta: Vec2,
    /// Whether a non-finite or negative requested coordinate was normalized.
    pub request_normalized: bool,
}

impl ScrollCommandResult {
    /// Returns whether the command changed the constrained offset.
    #[must_use]
    pub fn changed(self) -> bool {
        self.previous_offset != self.offset
    }
}

/// Inspectable snapshot of one shared scroll state.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct ScrollStateSnapshot {
    /// Current normalized offset in logical pixels.
    ///
    /// Once metrics have been supplied, this is constrained to `max_offset`.
    pub offset: Vec2,
    /// Latest viewport size in logical pixels.
    pub viewport: Size,
    /// Latest content size in logical pixels.
    pub content: Size,
    /// Maximum constrained offset derived from the current metrics.
    pub max_offset: Vec2,
    /// Revision of the offset or metrics.
    pub revision: u64,
    /// Number of constrained commands applied, including boundary hits.
    pub command_count: u64,
    /// Most recent constrained command result, including a boundary hit.
    pub last_command: Option<ScrollCommandResult>,
}

/// Shared scroll offset state for viewport-like controls.
///
/// A scrollable widget owns one `ScrollState` and registers it against its
/// viewport element with [`Ui::register_scroll_target`](crate::Ui::register_scroll_target).
/// Scroll bars, wheel handlers, accessibility actions, and virtualized controls
/// then share one offset and one viewport/content extent instead of
/// maintaining side tables keyed by element id.
///
/// The raw setters are synchronization hooks for adapters whose backing model
/// remains authoritative. They do not issue a command or invalidate a retained
/// target. Ordinary app requests should use target-level helpers such as
/// [`crate::set_scroll_offset_y`] so mutation and invalidation stay together.
#[derive(Clone)]
pub struct ScrollState {
    inner: Rc<ScrollStateInner>,
}

struct ScrollStateInner {
    model: Cell<ScrollModel>,
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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct ScrollModel {
    offset: Vec2,
    metrics: ScrollMetrics,
    revision: u64,
    command_count: u64,
    last_command: Option<ScrollCommandResult>,
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
                model: Cell::new(ScrollModel::default()),
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

    /// Returns the current normalized scroll offset in logical pixels.
    ///
    /// Once metrics have been supplied, this is constrained to [`Self::max_offset`].
    #[must_use]
    pub fn offset(&self) -> Vec2 {
        self.inner.model.get().offset
    }

    /// Returns the current normalized vertical scroll offset in logical pixels.
    #[must_use]
    pub fn offset_y(&self) -> f64 {
        self.inner.model.get().offset.y
    }

    /// Returns the current maximum scroll offset in logical pixels.
    #[must_use]
    pub fn max_offset(&self) -> Vec2 {
        self.inner.model.get().metrics.max_offset()
    }

    /// Returns the latest viewport size in logical pixels.
    #[must_use]
    pub fn viewport_size(&self) -> Size {
        self.inner.model.get().metrics.viewport
    }

    /// Returns current metrics, revision, and the last command result.
    #[must_use]
    pub fn snapshot(&self) -> ScrollStateSnapshot {
        let model = self.inner.model.get();
        ScrollStateSnapshot {
            offset: model.offset,
            viewport: model.metrics.viewport,
            content: model.metrics.content,
            max_offset: model.metrics.max_offset(),
            revision: model.revision,
            command_count: model.command_count,
            last_command: model.last_command,
        }
    }

    /// Sets the current logical scroll offset without clamping to metrics.
    ///
    /// This is primarily for viewport widgets that already used their own
    /// virtualization model to clamp the offset. General callers should prefer
    /// higher-level helpers such as [`crate::set_scroll_offset_y`].
    ///
    /// This now treats every distinct normalized offset as a change rather
    /// than suppressing sub-epsilon differences. Callers that need an
    /// approximate synchronization policy should apply it before this call.
    /// A changed raw offset also clears the most recent command because the new
    /// position did not come from that command.
    ///
    /// Returns `true` when the stored offset changed.
    pub fn set_offset_raw(&self, offset: Vec2) -> bool {
        let (offset, _) = normalize_scroll_offset(offset);
        let mut model = self.inner.model.get();
        if model.offset == offset {
            return false;
        }
        model.offset = offset;
        model.revision = model.revision.wrapping_add(1);
        model.last_command = None;
        self.inner.model.set(model);
        true
    }

    /// Sets the current vertical scroll offset without clamping to metrics.
    ///
    /// This has the same synchronization and exact-change semantics as
    /// [`Self::set_offset_raw`].
    ///
    /// Returns `true` when the stored offset changed.
    pub fn set_offset_y_raw(&self, offset_y: f64) -> bool {
        self.set_offset_raw(Vec2::new(self.offset().x, offset_y))
    }

    /// Replaces the latest viewport/content metrics in logical pixels.
    ///
    /// Virtualized controls should set this after recomputing their backing
    /// extent so scrollbars and accessibility can report the same range as the
    /// widget's own realization model. This now constrains an existing offset
    /// to the new range; callers that previously clamped and wrote the offset
    /// back separately can remove that write-back path. This state-only update
    /// does not invalidate a retained target.
    pub fn set_metrics(&self, viewport: Size, content: Size) {
        let mut model = self.inner.model.get();
        let metrics = ScrollMetrics::new(viewport, content);
        let offset = constrain_scroll_offset(model.offset, metrics.max_offset());
        if model.metrics == metrics && model.offset == offset {
            return;
        }
        let offset_changed = model.offset != offset;
        model.metrics = metrics;
        model.offset = offset;
        model.revision = model.revision.wrapping_add(1);
        if offset_changed {
            model.last_command = None;
        }
        self.inner.model.set(model);
    }

    pub(crate) fn apply_offset(
        &self,
        offset: Vec2,
        source: ScrollInputSource,
    ) -> ScrollCommandResult {
        let mut model = self.inner.model.get();
        let previous = model.offset;
        let (requested_offset, normalized) = normalize_scroll_offset(offset);
        let requested_delta = Vec2::new(
            finite_delta(offset.x, requested_offset.x, previous.x),
            finite_delta(offset.y, requested_offset.y, previous.y),
        );
        let offset = constrain_scroll_offset(requested_offset, model.metrics.max_offset());
        let consumed_delta = offset - previous;
        let unconsumed_delta = requested_delta - consumed_delta;
        model.command_count = model.command_count.wrapping_add(1);
        let change = ScrollCommandResult {
            source,
            previous_offset: previous,
            requested_delta,
            offset,
            consumed_delta,
            unconsumed_delta,
            request_normalized: normalized,
        };
        model.last_command = Some(change);
        if offset != previous {
            model.offset = offset;
            model.revision = model.revision.wrapping_add(1);
        }
        self.inner.model.set(model);
        change
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

fn normalize_scroll_offset(offset: Vec2) -> (Vec2, bool) {
    let normalized = Vec2::new(
        normalize_scroll_axis(offset.x),
        normalize_scroll_axis(offset.y),
    );
    (normalized, normalized != offset)
}

fn constrain_scroll_offset(offset: Vec2, max: Vec2) -> Vec2 {
    Vec2::new(offset.x.min(max.x), offset.y.min(max.y))
}

fn finite_delta(requested: f64, normalized: f64, previous: f64) -> f64 {
    if requested.is_finite() {
        requested - previous
    } else {
        normalized - previous
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
        assert!(
            self.targets.get(target).is_none(),
            "scroll target {target:?} is already registered"
        );
        self.targets.insert(target, scroll_target);
    }

    pub(crate) fn target(&self, target: ElementId) -> Option<ScrollTarget> {
        self.targets.get(target).cloned()
    }

    pub(crate) fn remove_target(&mut self, target: ElementId) {
        self.targets.remove(target);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constrained_commands_report_consumed_and_unconsumed_delta() {
        let state = ScrollState::new();
        state.set_metrics(Size::new(100.0, 40.0), Size::new(100.0, 100.0));

        let change = state.apply_offset(Vec2::new(0.0, 75.0), ScrollInputSource::Programmatic);

        assert_eq!(change.consumed_delta, Vec2::new(0.0, 60.0));
        assert_eq!(
            state.snapshot().last_command,
            Some(ScrollCommandResult {
                source: ScrollInputSource::Programmatic,
                previous_offset: Vec2::ZERO,
                requested_delta: Vec2::new(0.0, 75.0),
                offset: Vec2::new(0.0, 60.0),
                consumed_delta: Vec2::new(0.0, 60.0),
                unconsumed_delta: Vec2::new(0.0, 15.0),
                request_normalized: false,
            })
        );
    }

    #[test]
    fn metrics_revision_clamps_offset_and_invalid_commands_remain_observable() {
        let state = ScrollState::new();
        state.set_metrics(Size::new(50.0, 50.0), Size::new(50.0, 150.0));
        let _ = state.apply_offset(Vec2::new(0.0, 80.0), ScrollInputSource::Programmatic);
        let before = state.snapshot();

        state.set_metrics(Size::new(50.0, 50.0), Size::new(50.0, 40.0));
        let after_metrics = state.snapshot();
        assert_eq!(after_metrics.offset, Vec2::ZERO);
        assert!(after_metrics.revision > before.revision);

        let change =
            state.apply_offset(Vec2::new(f64::NAN, -20.0), ScrollInputSource::Programmatic);
        let snapshot = state.snapshot();
        assert!(!change.changed());
        assert_eq!(snapshot.command_count, 2);
        let last = snapshot.last_command.expect("command should be recorded");
        assert!(last.request_normalized);
        assert_eq!(last.unconsumed_delta.y, -20.0);
    }
}
