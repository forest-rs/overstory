// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Presentation timeline integration for [`Ui`].

use invalidation::ChannelSet;
use understory_timing::TimerInstant;

use crate::animation_runtime::{
    ChangeCause, PresentationProperty, PresentationTrackKey, TransitionSpec,
};
use crate::element::ElementId;

use super::Ui;

impl Ui {
    pub(crate) fn request_scalar_transition(
        &mut self,
        element: ElementId,
        property: PresentationProperty,
        spec: TransitionSpec,
    ) {
        self.animations
            .request_scalar_transition(PresentationTrackKey::new(element, property), spec);
    }

    pub(crate) fn has_pending_scalar_transition(
        &self,
        element: ElementId,
        property: PresentationProperty,
    ) -> bool {
        self.animations
            .has_pending_transition(PresentationTrackKey::new(element, property))
    }

    pub(crate) fn set_scalar_presentation_target(
        &mut self,
        element: ElementId,
        property: PresentationProperty,
        target: f32,
        cause: ChangeCause,
        affects: ChannelSet,
    ) -> bool {
        let needs_sample = self.animations.set_scalar_target(
            PresentationTrackKey::new(element, property),
            target,
            self.now(),
            cause,
            affects,
        );
        if needs_sample {
            self.mark_channels(element, affects);
        }
        needs_sample
    }

    pub(crate) fn presented_scalar(
        &self,
        element: ElementId,
        property: PresentationProperty,
        target: f32,
    ) -> f32 {
        self.animations.sample_scalar(
            PresentationTrackKey::new(element, property),
            target,
            self.now(),
        )
    }

    pub(in crate::ui) fn advance_animations(&mut self, now: TimerInstant) {
        let affected = self.animations.advance(now);
        for (element, channels) in affected {
            self.mark_channels(element, channels);
        }
    }

    /// Returns the next timer or presentation-track deadline the host should
    /// wake for.
    #[must_use]
    pub fn next_wakeup_deadline(&self) -> Option<TimerInstant> {
        min_deadline(
            self.next_timer_deadline(),
            self.animations.next_deadline(self.now()),
        )
    }
}

fn min_deadline(a: Option<TimerInstant>, b: Option<TimerInstant>) -> Option<TimerInstant> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}
