// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use hashbrown::HashMap;
use understory_animation_timeline::{AnimationTimeline, ManualTimeline, TimelineTime};
use understory_timing::TimerInstant;

/// Identifier for an animation timeline retained by `Ui`.
///
/// The document timeline is always available as [`Self::DOCUMENT`]. Manual
/// timelines are created by `Ui` and can then drive retained animations through
/// explicit seeks without creating a separate animation engine.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AnimationTimelineId(u64);

impl AnimationTimelineId {
    /// Timeline driven by the host-supplied document sample time.
    pub const DOCUMENT: Self = Self(0);

    pub(super) const FIRST_MANUAL: u64 = 1;

    /// Creates an animation timeline id from a stable integer.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the compact integer value for this timeline id.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Document timeline backed by the host-supplied UI monotonic clock.
///
/// Host runtimes still decide when frames are prepared and which sample time
/// they pass to [`crate::Ui`]. This adapter is the explicit boundary between
/// those host instants and the animation-runtime timeline time used by
/// retained animation instances.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DocumentTimeline;

impl DocumentTimeline {
    /// Converts a host monotonic instant into document timeline time.
    #[must_use]
    pub(crate) fn time_at(self, now: TimerInstant) -> TimelineTime {
        self.current_time(&now)
            .expect("document timeline is always active")
    }
}

impl AnimationTimeline<TimerInstant> for DocumentTimeline {
    fn current_time(&self, context: &TimerInstant) -> Option<TimelineTime> {
        Some(TimelineTime::from_duration(*context))
    }
}

/// Runtime view of active timeline sources for one sampling pass.
#[derive(Clone, Copy, Debug)]
pub(super) struct TimelineClock<'a> {
    document_timeline: DocumentTimeline,
    document_context: TimerInstant,
    manual_timelines: &'a HashMap<AnimationTimelineId, ManualTimeline>,
}

impl<'a> TimelineClock<'a> {
    /// Creates a clock view over document and manual timeline sources.
    #[must_use]
    pub(super) const fn new(
        document_timeline: DocumentTimeline,
        document_context: TimerInstant,
        manual_timelines: &'a HashMap<AnimationTimelineId, ManualTimeline>,
    ) -> Self {
        Self {
            document_timeline,
            document_context,
            manual_timelines,
        }
    }

    /// Returns the current time for `timeline`, or `None` when inactive.
    #[must_use]
    pub(super) fn time(self, timeline: AnimationTimelineId) -> Option<TimelineTime> {
        if timeline == AnimationTimelineId::DOCUMENT {
            return self.document_timeline.current_time(&self.document_context);
        }

        self.manual_timelines
            .get(&timeline)
            .and_then(|timeline| timeline.current_time(&()))
    }
}
