// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::Vec2;
use peniko::Color;
use understory_animation::{AnimationPlayState, CompositeOperation, TimingSample};
use understory_timing::TimerDuration;

use crate::element::ElementId;
use crate::motion::{
    Animatable, PresentationSlot, PresentationTarget, PresentationValue, PresentationValueKind,
};

use super::instance::AnimationId;
use super::timeline::AnimationTimelineId;

/// Frame-pipeline domain that samples the timeline value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TimelineEvaluationDomain {
    /// Sampled during layout. Avoid unless animated layout is intentional.
    Layout,
    /// Sampled during layer composition.
    Composition,
}

pub(crate) use crate::motion::PresentationAnimationValue as PresentationRuntimeValue;

/// Kind of retained animation layer inside one target stack.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TimelineAnimationLayerKind {
    /// Implicit transition layer generated from a target-value change.
    Transition,
    /// Explicit or style-triggered effect layer.
    Effect,
}

/// Cold-path retained animation row exposed to inspection snapshot assembly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TimelineAnimationRuntimeRow {
    id: AnimationId,
    key: TimelineTrackKey,
    layer: TimelineAnimationLayerKind,
    state: AnimationPlayState,
    current_time: Option<TimerDuration>,
    timing_sample: Option<TimingSample>,
    playback_rate: f64,
    composite: CompositeOperation,
    value_kind: Option<PresentationValueKind>,
    timeline: AnimationTimelineId,
}

impl TimelineAnimationRuntimeRow {
    /// Creates a retained animation runtime row.
    #[must_use]
    pub(crate) const fn new(
        id: AnimationId,
        key: TimelineTrackKey,
        layer: TimelineAnimationLayerKind,
        state: AnimationPlayState,
        current_time: Option<TimerDuration>,
        timing_sample: Option<TimingSample>,
        playback_rate: f64,
        composite: CompositeOperation,
        value_kind: Option<PresentationValueKind>,
        timeline: AnimationTimelineId,
    ) -> Self {
        Self {
            id,
            key,
            layer,
            state,
            current_time,
            timing_sample,
            playback_rate,
            composite,
            value_kind,
            timeline,
        }
    }

    /// Returns the retained animation id.
    #[must_use]
    pub(crate) const fn id(self) -> AnimationId {
        self.id
    }

    /// Returns the target key this animation affects.
    #[must_use]
    pub(crate) const fn key(self) -> TimelineTrackKey {
        self.key
    }

    /// Returns whether this row describes a transition or an effect.
    #[must_use]
    pub(crate) const fn layer(self) -> TimelineAnimationLayerKind {
        self.layer
    }

    /// Returns the playback state.
    #[must_use]
    pub(crate) const fn state(self) -> AnimationPlayState {
        self.state
    }

    /// Returns the current local animation time.
    #[must_use]
    pub(crate) const fn current_time(self) -> Option<TimerDuration> {
        self.current_time
    }

    /// Returns the current timing sample.
    #[must_use]
    pub(crate) const fn timing_sample(self) -> Option<TimingSample> {
        self.timing_sample
    }

    /// Returns the playback rate.
    #[must_use]
    pub(crate) const fn playback_rate(self) -> f64 {
        self.playback_rate
    }

    /// Returns how this animation layer composites with the value beneath it.
    #[must_use]
    pub(crate) const fn composite(self) -> CompositeOperation {
        self.composite
    }

    /// Returns the sampled presentation value kind.
    #[must_use]
    pub(crate) const fn value_kind(self) -> Option<PresentationValueKind> {
        self.value_kind
    }

    /// Returns the timeline source driving this animation.
    #[must_use]
    pub(crate) const fn timeline(self) -> AnimationTimelineId {
        self.timeline
    }
}

/// Why a target value changed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChangeCause {
    /// User interaction changed semantic control state.
    Interaction,
    /// Any change that should snap for the first animation slice.
    Other,
}

/// Key for one implicit timeline track.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct TimelineTrackKey {
    element: ElementId,
    target: PresentationTarget,
}

impl TimelineTrackKey {
    /// Creates a timeline track key.
    #[must_use]
    pub(crate) const fn new(element: ElementId, target: PresentationTarget) -> Self {
        Self { element, target }
    }

    /// Returns the element this track targets.
    #[must_use]
    pub(crate) const fn element(self) -> ElementId {
        self.element
    }

    /// Returns the timeline target this track targets.
    #[must_use]
    pub(crate) const fn target(self) -> PresentationTarget {
        self.target
    }
}

/// Sample emitted while advancing a timeline track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TimelineSample {
    key: TimelineTrackKey,
    value: PresentationValue,
    complete: bool,
}

impl TimelineSample {
    /// Creates a timeline sample.
    #[must_use]
    pub(super) const fn new(
        key: TimelineTrackKey,
        value: PresentationValue,
        complete: bool,
    ) -> Self {
        Self {
            key,
            value,
            complete,
        }
    }

    /// Returns the track key this sample targets.
    #[must_use]
    pub(crate) const fn key(self) -> TimelineTrackKey {
        self.key
    }

    /// Returns the sampled value.
    #[must_use]
    pub(crate) const fn value(self) -> PresentationValue {
        self.value
    }

    /// Returns whether this is the final sample for the track.
    #[must_use]
    pub(crate) const fn is_complete(self) -> bool {
        self.complete
    }
}

/// Result from controlling a retained animation instance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TimelineControlResult {
    key: TimelineTrackKey,
    value: Option<PresentationValue>,
    complete: bool,
}

impl TimelineControlResult {
    /// Creates a timeline control result.
    #[must_use]
    pub(super) const fn new(
        key: TimelineTrackKey,
        value: Option<PresentationValue>,
        complete: bool,
    ) -> Self {
        Self {
            key,
            value,
            complete,
        }
    }

    /// Returns the track key this control operation affected.
    #[must_use]
    pub(crate) const fn key(self) -> TimelineTrackKey {
        self.key
    }

    /// Returns whether the affected track has no remaining active motion.
    #[must_use]
    pub(crate) const fn is_complete(self) -> bool {
        self.complete
    }

    /// Converts this result into a timeline sample, when it has a value.
    #[must_use]
    pub(crate) const fn sample(self) -> Option<TimelineSample> {
        match self.value {
            Some(value) => Some(TimelineSample::new(self.key, value, self.complete)),
            None => None,
        }
    }
}

pub(super) const fn expected_value_kind(
    target: PresentationTarget,
) -> Option<PresentationValueKind> {
    match target {
        PresentationTarget::Property(_) => None,
        PresentationTarget::Slot(
            PresentationSlot::LayerOffset | PresentationSlot::VisualTranslation,
        ) => Some(PresentationValueKind::Offset2d),
    }
}

pub(super) const fn evaluation_domain(
    target: PresentationTarget,
) -> Option<TimelineEvaluationDomain> {
    match target {
        PresentationTarget::Property(_) => None,
        PresentationTarget::Slot(
            PresentationSlot::LayerOffset | PresentationSlot::VisualTranslation,
        ) => Some(TimelineEvaluationDomain::Composition),
    }
}

pub(super) const fn default_underlying_value(
    target: PresentationTarget,
) -> Option<PresentationValue> {
    match target {
        PresentationTarget::Property(_) => None,
        PresentationTarget::Slot(
            PresentationSlot::LayerOffset | PresentationSlot::VisualTranslation,
        ) => Some(PresentationValue::Offset2d(Vec2::ZERO)),
    }
}

pub(super) fn same_value(a: PresentationValue, b: PresentationValue) -> bool {
    match (a, b) {
        (PresentationValue::Scalar32(a), PresentationValue::Scalar32(b)) => a.is_close(&b),
        (PresentationValue::Scalar64(a), PresentationValue::Scalar64(b)) => a.is_close(&b),
        (PresentationValue::Color(a), PresentationValue::Color(b)) => same_color(a, b),
        (PresentationValue::Offset2d(a), PresentationValue::Offset2d(b)) => a.is_close(&b),
        _ => false,
    }
}

fn same_color(a: Color, b: Color) -> bool {
    a.components
        .into_iter()
        .zip(b.components)
        .all(|(a, b)| a.is_close(&b))
}
