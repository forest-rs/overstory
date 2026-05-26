// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Presentation timeline integration for [`Ui`].
//!
//! `Ui` advances the timeline from host-supplied monotonic time, converts
//! sampled tracks into ordinary invalidation, and exposes wakeup deadlines to
//! host crates. Controls can request transitions, but the runtime remains the
//! owner of active tracks.

use understory_property::DependencyObject;
use understory_timing::TimerInstant;

use crate::channels::COMPOSE;
use crate::element::ElementId;
use crate::timeline_runtime::{
    AnimatableValue, ChangeCause, PresentationValue, TimelineSample, TimelineTarget,
    TimelineTrackKey, TransitionPolicy, VisualOutput,
};

use super::Ui;

impl Ui {
    pub(crate) fn request_presentation_transition(
        &mut self,
        element: ElementId,
        target: TimelineTarget,
        policy: TransitionPolicy,
    ) {
        self.timeline
            .request_transition(TimelineTrackKey::new(element, target), policy);
    }

    pub(crate) fn has_pending_presentation_transition(
        &self,
        element: ElementId,
        target: TimelineTarget,
    ) -> bool {
        self.timeline
            .has_pending_transition(TimelineTrackKey::new(element, target))
    }

    pub(crate) fn set_presentation_target<T: AnimatableValue>(
        &mut self,
        element: ElementId,
        timeline_target: TimelineTarget,
        value: T,
        cause: ChangeCause,
    ) -> bool {
        let key = TimelineTrackKey::new(element, timeline_target);
        let needs_sample = self.timeline.set_target(key, value, self.now(), cause);
        self.sync_timeline_property_sample(key);
        if needs_sample {
            self.mark_timeline_target_channels(element, key.target());
        }
        needs_sample
    }

    pub(crate) fn presented_layer_opacity(&self, element: ElementId) -> f32 {
        let property = self.properties.opacity;
        let target = self
            .get(element)
            .map(|el| self.effective_value_for(el, property))
            .unwrap_or(1.0);
        let key = TimelineTrackKey::new(element, TimelineTarget::Property(property.id()));
        self.timeline.sample(key, target, self.now())
    }

    pub(crate) fn presented_value<T: AnimatableValue>(
        &self,
        element: ElementId,
        property: TimelineTarget,
        target: T,
    ) -> T {
        self.timeline
            .sample(TimelineTrackKey::new(element, property), target, self.now())
    }

    pub(in crate::ui) fn advance_timeline(&mut self, now: TimerInstant) {
        let samples = self.timeline.advance(now);
        for sample in samples {
            self.apply_timeline_sample(sample);
        }
    }

    fn apply_timeline_sample(&mut self, sample: TimelineSample) {
        let key = sample.key();
        if sample.is_complete() {
            self.clear_timeline_property_sample(key);
        } else {
            self.set_timeline_property_sample(key, sample.value());
        }
        self.mark_timeline_target_channels(key.element(), key.target());
    }

    fn sync_timeline_property_sample(&mut self, key: TimelineTrackKey) {
        if let Some(value) = self.timeline.active_sample_value(key, self.now()) {
            self.set_timeline_property_sample(key, value);
        } else {
            self.clear_timeline_property_sample(key);
        }
    }

    fn set_timeline_property_sample(&mut self, key: TimelineTrackKey, value: PresentationValue) {
        let TimelineTarget::Property(property) = key.target() else {
            return;
        };
        let props = self.properties;
        let Some(element) = self.get_mut(key.element()) else {
            return;
        };
        let store = element.property_store_mut();
        if property == props.background.id() {
            if let PresentationValue::Color(color) = value {
                store.set_animation(props.background, Some(peniko::Brush::Solid(color)));
            }
        } else if property == props.opacity.id() {
            if let PresentationValue::Scalar32(value) = value {
                store.set_animation(props.opacity, value);
            }
        } else if property == props.border_width.id() {
            if let PresentationValue::Scalar64(value) = value {
                store.set_animation(props.border_width, value);
            }
        } else if property == props.corner_radius.id()
            && let PresentationValue::Scalar64(value) = value
        {
            store.set_animation(props.corner_radius, value);
        }
    }

    pub(in crate::ui) fn clear_timeline_property_sample(&mut self, key: TimelineTrackKey) {
        let TimelineTarget::Property(property) = key.target() else {
            return;
        };
        let props = self.properties;
        let Some(element) = self.get_mut(key.element()) else {
            return;
        };
        let store = element.property_store_mut();
        if property == props.background.id() {
            store.clear_animation(props.background);
        } else if property == props.opacity.id() {
            store.clear_animation(props.opacity);
        } else if property == props.border_width.id() {
            store.clear_animation(props.border_width);
        } else if property == props.corner_radius.id() {
            store.clear_animation(props.corner_radius);
        }
    }

    fn mark_timeline_target_channels(&mut self, element: ElementId, target: TimelineTarget) {
        let channels = match target {
            TimelineTarget::Property(property) => self.registry.affects_channels(property),
            TimelineTarget::VisualOutput(VisualOutput::LayerOffset) => COMPOSE.into_set(),
        };
        self.mark_channels(element, channels);
    }

    /// Returns the next timer or presentation-track deadline the host should
    /// wake for.
    #[must_use]
    pub fn next_wakeup_deadline(&self) -> Option<TimerInstant> {
        min_deadline(
            self.next_timer_deadline(),
            self.timeline.next_deadline(self.now()),
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
