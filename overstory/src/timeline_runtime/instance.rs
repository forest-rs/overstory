// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use understory_timing::TimerInstant;

use crate::element::ElementId;
use crate::motion::PresentationTarget;

use super::types::TimelineTrackKey;

/// Stable id for a retained presentation animation instance.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AnimationId(u64);

impl AnimationId {
    /// Returns this animation id as a compact integer token.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Runtime event kind produced by a retained presentation animation instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnimationEventKind {
    /// The animation entered the active set.
    Started,
    /// The animation crossed into another iteration.
    Iteration,
    /// The animation reached its natural endpoint.
    Finished,
    /// Playback was canceled and the instance returned to the inactive set.
    Canceled,
}

/// Runtime event produced by a retained presentation animation instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnimationEvent {
    id: AnimationId,
    key: TimelineTrackKey,
    kind: AnimationEventKind,
    at: TimerInstant,
    iteration: Option<u64>,
}

impl AnimationEvent {
    /// Creates an animation event.
    #[must_use]
    pub(crate) const fn new(
        id: AnimationId,
        key: TimelineTrackKey,
        kind: AnimationEventKind,
        at: TimerInstant,
    ) -> Self {
        Self {
            id,
            key,
            kind,
            at,
            iteration: None,
        }
    }

    /// Creates an animation iteration event.
    #[must_use]
    pub(crate) const fn new_iteration(
        id: AnimationId,
        key: TimelineTrackKey,
        at: TimerInstant,
        iteration: u64,
    ) -> Self {
        Self {
            id,
            key,
            kind: AnimationEventKind::Iteration,
            at,
            iteration: Some(iteration),
        }
    }

    /// Returns the animation instance id.
    #[must_use]
    pub const fn id(self) -> AnimationId {
        self.id
    }

    /// Returns the element this animation instance affected.
    #[must_use]
    pub const fn element(self) -> ElementId {
        self.key.element()
    }

    /// Returns the presentation target this animation instance affected.
    #[must_use]
    pub const fn target(self) -> PresentationTarget {
        self.key.target()
    }

    /// Returns the event kind.
    #[must_use]
    pub const fn kind(self) -> AnimationEventKind {
        self.kind
    }

    /// Returns the monotonic time associated with the event.
    #[must_use]
    pub const fn at(self) -> TimerInstant {
        self.at
    }

    /// Returns the iteration index for [`AnimationEventKind::Iteration`].
    ///
    /// Other event kinds return `None`.
    #[must_use]
    pub const fn iteration(self) -> Option<u64> {
        self.iteration
    }
}

pub(super) fn next_animation_id(next: &mut u64) -> AnimationId {
    let id = AnimationId(*next);
    *next = next.wrapping_add(1);
    id
}
