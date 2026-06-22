// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use understory_timing::TimerInstant;

use crate::element::ElementId;

/// Stable id for a scheduled timeline cue.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TimelineCueId(pub(super) u64);

impl TimelineCueId {
    /// Returns this cue id as a compact integer token.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A deterministic cue point emitted by the UI timeline.
///
/// Cues are for scheduled side effects that are not themselves animations:
/// a host may map `payload` to a sound, an inspector marker, a scripted action,
/// or future control-service work. The timeline only stores and emits the cue;
/// it does not execute arbitrary callbacks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimelineCue {
    /// Stable id returned when the cue was scheduled.
    pub id: TimelineCueId,
    /// Optional element the cue is scoped to.
    ///
    /// Element-scoped cues are removed automatically when that element is
    /// removed from the UI tree.
    pub target: Option<ElementId>,
    /// Host/application-defined payload token.
    pub payload: u64,
    /// Monotonic instant when the cue becomes due.
    pub due_at: TimerInstant,
}
