// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Presentation timeline integration for [`Ui`].
//!
//! `Ui` advances the timeline from host-supplied monotonic time, converts
//! sampled tracks into ordinary invalidation, and exposes wakeup deadlines to
//! host crates. Controls can request transitions, but the runtime remains the
//! owner of active tracks.

use kurbo::Vec2;
use peniko::Color;
use understory_timing::TimerInstant;

use crate::animation_runtime::{
    ChangeCause, PresentationProperty, PresentationTrackKey, TransitionPolicy,
};
use crate::element::ElementId;

use super::Ui;

impl Ui {
    pub(crate) fn request_presentation_transition(
        &mut self,
        element: ElementId,
        property: PresentationProperty,
        policy: TransitionPolicy,
    ) {
        self.animations
            .request_transition(PresentationTrackKey::new(element, property), policy);
    }

    pub(crate) fn has_pending_presentation_transition(
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
    ) -> bool {
        let needs_sample = self.animations.set_scalar_target(
            PresentationTrackKey::new(element, property),
            target,
            self.now(),
            cause,
        );
        if needs_sample {
            self.mark_channels(element, property.metadata().affects());
        }
        needs_sample
    }

    pub(crate) fn set_color_presentation_target(
        &mut self,
        element: ElementId,
        property: PresentationProperty,
        target: Color,
        cause: ChangeCause,
    ) -> bool {
        let needs_sample = self.animations.set_color_target(
            PresentationTrackKey::new(element, property),
            target,
            self.now(),
            cause,
        );
        if needs_sample {
            self.mark_channels(element, property.metadata().affects());
        }
        needs_sample
    }

    pub(crate) fn set_offset_presentation_target(
        &mut self,
        element: ElementId,
        property: PresentationProperty,
        target: Vec2,
        cause: ChangeCause,
    ) -> bool {
        let needs_sample = self.animations.set_offset_target(
            PresentationTrackKey::new(element, property),
            target,
            self.now(),
            cause,
        );
        if needs_sample {
            self.mark_channels(element, property.metadata().affects());
        }
        needs_sample
    }

    pub(crate) fn presented_layer_opacity(&self, element: ElementId) -> f32 {
        let key = PresentationTrackKey::new(element, PresentationProperty::LayerOpacity);
        let target = self.animations.scalar_target(key).unwrap_or(1.0);
        self.animations.sample_scalar(key, target, self.now())
    }

    pub(crate) fn presented_offset(
        &self,
        element: ElementId,
        property: PresentationProperty,
        target: Vec2,
    ) -> Vec2 {
        self.animations.sample_offset(
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
