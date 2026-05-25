// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Internal presentation timeline runtime.
//!
//! This is intentionally tiny: one implicit track per element/property and one
//! scalar transition clip kind. The shape leaves room for a future shared
//! timeline scheduler without making built-in controls own local timers.
//!
//! Vocabulary for this slice:
//!
//! - A *target value* is the resolved semantic value produced by control state,
//!   local properties, templates, and style.
//! - A *presented value* is the sampled value used for the current frame.
//! - A transition is a temporary presentation opinion over a target value; it
//!   never replaces the target itself.
//!
//! Deliberate non-goals for the first slice: public animation APIs, springs,
//! keyframes, color interpolation, layout animation, reduced-motion policy, and
//! compositor delegation. Layout may snap to the target while composition or
//! paint samples a presented value, which keeps ordinary interaction motion
//! cheap and avoids making animated layout the default.

use alloc::vec::Vec;

use hashbrown::HashMap;
use invalidation::ChannelSet;
use kurbo::Vec2;
use peniko::Color;
use understory_timing::{TimerDuration, TimerInstant};

use crate::channels::{COMPOSE, PAINT};
use crate::element::ElementId;

/// Nominal frame interval hosts can use while presentation tracks are active.
pub(crate) const PRESENTATION_FRAME_INTERVAL: TimerDuration = 16_666_667;

/// Presentation property whose sampled value may differ from its target value.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum PresentationProperty {
    /// Solid background color sampled by paint while style keeps the resolved
    /// target brush.
    BackgroundColor,
    /// Layer opacity sampled by composition while semantic visibility and hit
    /// testing use the target state.
    LayerOpacity,
    /// Layer-local offset sampled by composition while layout keeps the
    /// resolved target geometry.
    LayerOffset,
}

impl PresentationProperty {
    /// Returns static metadata for this presentation property.
    #[must_use]
    pub(crate) const fn metadata(self) -> PresentationPropertyMetadata {
        match self {
            Self::BackgroundColor => PresentationPropertyMetadata {
                value_kind: PresentationValueKind::Color,
                evaluation_domain: AnimationEvaluationDomain::Paint,
                affects: PAINT.into_set(),
            },
            Self::LayerOpacity => PresentationPropertyMetadata {
                value_kind: PresentationValueKind::Scalar,
                evaluation_domain: AnimationEvaluationDomain::Composition,
                affects: COMPOSE.into_set(),
            },
            Self::LayerOffset => PresentationPropertyMetadata {
                value_kind: PresentationValueKind::Offset2d,
                evaluation_domain: AnimationEvaluationDomain::Composition,
                affects: COMPOSE.into_set(),
            },
        }
    }
}

/// Storage/interpolation kind for a presentation property.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PresentationValueKind {
    /// A single `f32` value.
    Scalar,
    /// A straight-alpha sRGB color.
    Color,
    /// A two-dimensional logical offset.
    Offset2d,
}

/// Frame-pipeline domain that samples the presentation value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AnimationEvaluationDomain {
    /// Sampled during layout. Avoid unless animated layout is intentional.
    Layout,
    /// Sampled during paint/presentation recording.
    Paint,
    /// Sampled during layer composition.
    Composition,
}

/// Static metadata for a presentation property.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PresentationPropertyMetadata {
    value_kind: PresentationValueKind,
    evaluation_domain: AnimationEvaluationDomain,
    affects: ChannelSet,
}

impl PresentationPropertyMetadata {
    /// Returns the value kind this property stores.
    #[must_use]
    pub(crate) const fn value_kind(self) -> PresentationValueKind {
        self.value_kind
    }

    /// Returns where this property is sampled in the frame pipeline.
    #[must_use]
    pub(crate) const fn evaluation_domain(self) -> AnimationEvaluationDomain {
        self.evaluation_domain
    }

    /// Returns the earliest invalidation channels this property affects.
    #[must_use]
    pub(crate) const fn affects(self) -> ChannelSet {
        self.affects
    }
}

/// Typed target or sampled value carried by a presentation track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PresentationValue {
    /// A scalar value.
    Scalar(f32),
    /// A straight-alpha sRGB color.
    Color(Color),
    /// A two-dimensional logical offset.
    Offset2d(Vec2),
}

impl PresentationValue {
    fn kind(self) -> PresentationValueKind {
        match self {
            Self::Scalar(_) => PresentationValueKind::Scalar,
            Self::Color(_) => PresentationValueKind::Color,
            Self::Offset2d(_) => PresentationValueKind::Offset2d,
        }
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

/// Which target changes a transition policy applies to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TransitionTrigger {
    /// Animate only interaction-driven changes.
    Interaction,
}

impl TransitionTrigger {
    fn allows(self, cause: ChangeCause) -> bool {
        match self {
            Self::Interaction => cause == ChangeCause::Interaction,
        }
    }
}

/// Timing function for a scalar transition.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Easing {
    /// Linear interpolation.
    #[default]
    Linear,
}

impl Easing {
    fn sample(self, t: f32) -> f32 {
        match self {
            Self::Linear => t,
        }
    }
}

/// Transition policy for an implicit scalar presentation track.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TransitionSpec {
    duration: TimerDuration,
    easing: Easing,
}

impl TransitionSpec {
    /// Creates a linear transition over `duration` nanoseconds.
    #[must_use]
    pub(crate) const fn linear(duration: TimerDuration) -> Self {
        Self {
            duration,
            easing: Easing::Linear,
        }
    }
}

/// Policy for deciding whether a target change creates a presentation track.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TransitionPolicy {
    trigger: TransitionTrigger,
    spec: TransitionSpec,
}

impl TransitionPolicy {
    /// Animates interaction-driven changes with `spec`.
    #[must_use]
    pub(crate) const fn interaction(spec: TransitionSpec) -> Self {
        Self {
            trigger: TransitionTrigger::Interaction,
            spec,
        }
    }

    fn spec_for(self, cause: ChangeCause) -> Option<TransitionSpec> {
        self.trigger.allows(cause).then_some(self.spec)
    }
}

/// Key for one implicit presentation track.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PresentationTrackKey {
    element: ElementId,
    property: PresentationProperty,
}

impl PresentationTrackKey {
    /// Creates a presentation track key.
    #[must_use]
    pub(crate) const fn new(element: ElementId, property: PresentationProperty) -> Self {
        Self { element, property }
    }

    /// Returns the element this track targets.
    #[must_use]
    pub(crate) const fn element(self) -> ElementId {
        self.element
    }

    /// Returns the presentation property this track targets.
    #[must_use]
    pub(crate) const fn property(self) -> PresentationProperty {
        self.property
    }
}

/// Active clip carried by one presentation track.
#[derive(Clone, Copy, Debug, PartialEq)]
enum MotionClip {
    Scalar(ScalarTransition),
    Color(ColorTransition),
    Offset(OffsetTransition),
}

impl MotionClip {
    fn new(
        from: PresentationValue,
        to: PresentationValue,
        started_at: TimerInstant,
        spec: TransitionSpec,
    ) -> Self {
        match (from, to) {
            (PresentationValue::Scalar(from), PresentationValue::Scalar(to)) => {
                Self::Scalar(ScalarTransition::new(from, to, started_at, spec))
            }
            (PresentationValue::Color(from), PresentationValue::Color(to)) => {
                Self::Color(ColorTransition::new(from, to, started_at, spec))
            }
            (PresentationValue::Offset2d(from), PresentationValue::Offset2d(to)) => {
                Self::Offset(OffsetTransition::new(from, to, started_at, spec))
            }
            _ => unreachable!("presentation target kinds should match property metadata"),
        }
    }

    fn sample(self, now: TimerInstant) -> PresentationValue {
        match self {
            Self::Scalar(transition) => PresentationValue::Scalar(transition.sample(now)),
            Self::Color(transition) => PresentationValue::Color(transition.sample(now)),
            Self::Offset(transition) => PresentationValue::Offset2d(transition.sample(now)),
        }
    }

    fn is_complete(self, now: TimerInstant) -> bool {
        match self {
            Self::Scalar(transition) => transition.is_complete(now),
            Self::Color(transition) => transition.is_complete(now),
            Self::Offset(transition) => transition.is_complete(now),
        }
    }

    fn end_time(self) -> TimerInstant {
        match self {
            Self::Scalar(transition) => transition.end_time(),
            Self::Color(transition) => transition.end_time(),
            Self::Offset(transition) => transition.end_time(),
        }
    }
}

/// One scalar transition clip.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ScalarTransition {
    from: f32,
    to: f32,
    started_at: TimerInstant,
    duration: TimerDuration,
    easing: Easing,
}

impl ScalarTransition {
    fn new(from: f32, to: f32, started_at: TimerInstant, spec: TransitionSpec) -> Self {
        Self {
            from,
            to,
            started_at,
            duration: spec.duration,
            easing: spec.easing,
        }
    }

    fn sample(self, now: TimerInstant) -> f32 {
        let t = self
            .easing
            .sample(transition_progress(now, self.started_at, self.duration));
        self.from + ((self.to - self.from) * t)
    }

    fn is_complete(self, now: TimerInstant) -> bool {
        now.saturating_sub(self.started_at) >= self.duration
    }

    fn end_time(self) -> TimerInstant {
        self.started_at.saturating_add(self.duration)
    }
}

/// One color transition clip.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ColorTransition {
    from: Color,
    to: Color,
    started_at: TimerInstant,
    duration: TimerDuration,
    easing: Easing,
}

impl ColorTransition {
    fn new(from: Color, to: Color, started_at: TimerInstant, spec: TransitionSpec) -> Self {
        Self {
            from,
            to,
            started_at,
            duration: spec.duration,
            easing: spec.easing,
        }
    }

    fn sample(self, now: TimerInstant) -> Color {
        let t = self
            .easing
            .sample(transition_progress(now, self.started_at, self.duration));
        self.from.lerp_rect(self.to, t)
    }

    fn is_complete(self, now: TimerInstant) -> bool {
        now.saturating_sub(self.started_at) >= self.duration
    }

    fn end_time(self) -> TimerInstant {
        self.started_at.saturating_add(self.duration)
    }
}

/// One two-dimensional offset transition clip.
#[derive(Clone, Copy, Debug, PartialEq)]
struct OffsetTransition {
    from: Vec2,
    to: Vec2,
    started_at: TimerInstant,
    duration: TimerDuration,
    easing: Easing,
}

impl OffsetTransition {
    fn new(from: Vec2, to: Vec2, started_at: TimerInstant, spec: TransitionSpec) -> Self {
        Self {
            from,
            to,
            started_at,
            duration: spec.duration,
            easing: spec.easing,
        }
    }

    fn sample(self, now: TimerInstant) -> Vec2 {
        let t = f64::from(self.easing.sample(transition_progress(
            now,
            self.started_at,
            self.duration,
        )));
        self.from + ((self.to - self.from) * t)
    }

    fn is_complete(self, now: TimerInstant) -> bool {
        now.saturating_sub(self.started_at) >= self.duration
    }

    fn end_time(self) -> TimerInstant {
        self.started_at.saturating_add(self.duration)
    }
}

fn transition_progress(
    now: TimerInstant,
    started_at: TimerInstant,
    duration: TimerDuration,
) -> f32 {
    if duration == 0 {
        return 1.0;
    }
    let elapsed = now.saturating_sub(started_at);
    if elapsed >= duration {
        return 1.0;
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "transition progress is a bounded f32 sample"
    )]
    {
        elapsed as f32 / duration as f32
    }
}

/// Active implicit presentation track.
#[derive(Clone, Copy, Debug, PartialEq)]
struct PresentationTrack {
    clip: MotionClip,
}

/// Internal animation runtime for sampled presentation values.
///
/// This is the degenerate form of a future global timeline: each active entry
/// is an anonymous presentation track keyed by element/property. Retargeting
/// starts from the current sampled value, not the previous target value.
#[derive(Debug, Default)]
pub(crate) struct AnimationRuntime {
    targets: HashMap<PresentationTrackKey, PresentationValue>,
    tracks: HashMap<PresentationTrackKey, PresentationTrack>,
    pending: HashMap<PresentationTrackKey, TransitionPolicy>,
}

impl AnimationRuntime {
    /// Creates an empty runtime.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Requests that the next target change for `key` animate when it is caused
    /// by interaction.
    pub(crate) fn request_transition(
        &mut self,
        key: PresentationTrackKey,
        policy: TransitionPolicy,
    ) {
        self.pending.insert(key, policy);
    }

    /// Returns whether a transition spec is waiting for `key`'s next target.
    #[must_use]
    pub(crate) fn has_pending_transition(&self, key: PresentationTrackKey) -> bool {
        self.pending.contains_key(&key)
    }

    /// Records the resolved target value for `key`, planning or retargeting its
    /// presentation track when policy allows.
    pub(crate) fn set_target(
        &mut self,
        key: PresentationTrackKey,
        target: PresentationValue,
        now: TimerInstant,
        cause: ChangeCause,
    ) -> bool {
        let metadata = key.property().metadata();
        debug_assert_eq!(
            metadata.value_kind(),
            target.kind(),
            "presentation target kind must match property metadata"
        );
        debug_assert_ne!(
            metadata.evaluation_domain(),
            AnimationEvaluationDomain::Layout,
            "layout-domain animation must not be sampled by the generic target setter"
        );
        let previous_target = self.targets.get(&key).copied();
        let current_presented = self.sample(key, previous_target.unwrap_or(target), now);
        self.targets.insert(key, target);
        let target_changed = previous_target.is_none_or(|previous| !same_value(previous, target));
        if !target_changed {
            if cause == ChangeCause::Interaction {
                self.pending.remove(&key);
            }
            return false;
        }

        let had_track = self.tracks.contains_key(&key);
        let spec = self
            .pending
            .remove(&key)
            .and_then(|policy| policy.spec_for(cause));
        let Some(spec) = spec else {
            self.tracks.remove(&key);
            return had_track;
        };

        let from = current_presented;
        if same_value(from, target) || spec.duration == 0 {
            self.tracks.remove(&key);
            return had_track;
        }

        self.tracks.insert(
            key,
            PresentationTrack {
                clip: MotionClip::new(from, target, now, spec),
            },
        );
        true
    }

    /// Records a scalar target value.
    pub(crate) fn set_scalar_target(
        &mut self,
        key: PresentationTrackKey,
        target: f32,
        now: TimerInstant,
        cause: ChangeCause,
    ) -> bool {
        self.set_target(key, PresentationValue::Scalar(target), now, cause)
    }

    /// Records a color target value.
    pub(crate) fn set_color_target(
        &mut self,
        key: PresentationTrackKey,
        target: Color,
        now: TimerInstant,
        cause: ChangeCause,
    ) -> bool {
        self.set_target(key, PresentationValue::Color(target), now, cause)
    }

    /// Records a two-dimensional offset target value.
    pub(crate) fn set_offset_target(
        &mut self,
        key: PresentationTrackKey,
        target: Vec2,
        now: TimerInstant,
        cause: ChangeCause,
    ) -> bool {
        self.set_target(key, PresentationValue::Offset2d(target), now, cause)
    }

    /// Returns the sampled scalar presentation value for `key`.
    #[must_use]
    pub(crate) fn sample_scalar(
        &self,
        key: PresentationTrackKey,
        target: f32,
        now: TimerInstant,
    ) -> f32 {
        let value = self.sample(key, PresentationValue::Scalar(target), now);
        match value {
            PresentationValue::Scalar(value) => value,
            PresentationValue::Color(_) | PresentationValue::Offset2d(_) => target,
        }
    }

    /// Returns the sampled color presentation value for `key`.
    #[must_use]
    pub(crate) fn sample_color(
        &self,
        key: PresentationTrackKey,
        target: Color,
        now: TimerInstant,
    ) -> Color {
        let value = self.sample(key, PresentationValue::Color(target), now);
        match value {
            PresentationValue::Color(value) => value,
            PresentationValue::Scalar(_) | PresentationValue::Offset2d(_) => target,
        }
    }

    /// Returns the sampled two-dimensional offset presentation value for `key`.
    #[must_use]
    pub(crate) fn sample_offset(
        &self,
        key: PresentationTrackKey,
        target: Vec2,
        now: TimerInstant,
    ) -> Vec2 {
        let value = self.sample(key, PresentationValue::Offset2d(target), now);
        match value {
            PresentationValue::Offset2d(value) => value,
            PresentationValue::Scalar(_) | PresentationValue::Color(_) => target,
        }
    }

    /// Returns the sampled presentation value for `key`.
    #[must_use]
    pub(crate) fn sample(
        &self,
        key: PresentationTrackKey,
        target: PresentationValue,
        now: TimerInstant,
    ) -> PresentationValue {
        self.tracks.get(&key).map_or_else(
            || self.targets.get(&key).copied().unwrap_or(target),
            |track| track.clip.sample(now),
        )
    }

    /// Advances active tracks, removing completed clips and returning affected
    /// elements/channels that should be invalidated for this sample.
    pub(crate) fn advance(&mut self, now: TimerInstant) -> Vec<(ElementId, ChannelSet)> {
        let mut affected = Vec::new();
        self.tracks.retain(|key, track| {
            affected.push((key.element(), key.property().metadata().affects()));
            !track.clip.is_complete(now)
        });
        affected
    }

    /// Returns the next deadline a host should wake for active presentation
    /// tracks.
    #[must_use]
    pub(crate) fn next_deadline(&self, now: TimerInstant) -> Option<TimerInstant> {
        if self.tracks.is_empty() {
            return None;
        }
        let next_frame = now.saturating_add(PRESENTATION_FRAME_INTERVAL);
        let next_end = self
            .tracks
            .values()
            .map(|track| track.clip.end_time())
            .min();
        Some(next_end.map_or(next_frame, |end| next_frame.min(end).max(now)))
    }

    /// Removes all runtime state for `element`.
    pub(crate) fn remove_element(&mut self, element: ElementId) {
        self.targets.retain(|key, _| key.element() != element);
        self.tracks.retain(|key, _| key.element() != element);
        self.pending.retain(|key, _| key.element() != element);
    }

    /// Returns whether `key` currently has an active presentation track.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn has_active_track(&self, key: PresentationTrackKey) -> bool {
        self.tracks.contains_key(&key)
    }
}

fn same_value(a: PresentationValue, b: PresentationValue) -> bool {
    match (a, b) {
        (PresentationValue::Scalar(a), PresentationValue::Scalar(b)) => same_scalar(a, b),
        (PresentationValue::Color(a), PresentationValue::Color(b)) => same_color(a, b),
        (PresentationValue::Offset2d(a), PresentationValue::Offset2d(b)) => same_offset(a, b),
        _ => false,
    }
}

fn same_scalar(a: f32, b: f32) -> bool {
    (a - b).abs() <= f32::EPSILON
}

fn same_color(a: Color, b: Color) -> bool {
    a.components
        .into_iter()
        .zip(b.components)
        .all(|(a, b)| same_scalar(a, b))
}

fn same_offset(a: Vec2, b: Vec2) -> bool {
    (a.x - b.x).abs() <= f64::EPSILON && (a.y - b.y).abs() <= f64::EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::{COMPOSE, PAINT};

    const SPEC: TransitionSpec = TransitionSpec::linear(100);
    const POLICY: TransitionPolicy = TransitionPolicy::interaction(SPEC);

    fn scalar_key() -> PresentationTrackKey {
        PresentationTrackKey::new(ElementId::new(7, 0), PresentationProperty::LayerOpacity)
    }

    fn color_key() -> PresentationTrackKey {
        PresentationTrackKey::new(ElementId::new(7, 0), PresentationProperty::BackgroundColor)
    }

    fn offset_key() -> PresentationTrackKey {
        PresentationTrackKey::new(ElementId::new(7, 0), PresentationProperty::LayerOffset)
    }

    #[test]
    fn scalar_transition_samples_linearly() {
        let key = scalar_key();
        let mut runtime = AnimationRuntime::new();
        runtime.set_scalar_target(key, 0.0, 0, ChangeCause::Other);
        runtime.request_transition(key, POLICY);
        runtime.set_scalar_target(key, 10.0, 10, ChangeCause::Interaction);

        assert_eq!(runtime.sample_scalar(key, 10.0, 10), 0.0);
        assert_eq!(runtime.sample_scalar(key, 10.0, 60), 5.0);
        assert_eq!(runtime.sample_scalar(key, 10.0, 110), 10.0);
    }

    #[test]
    fn scalar_transition_retargets_from_sampled_value() {
        let key = scalar_key();
        let mut runtime = AnimationRuntime::new();
        runtime.set_scalar_target(key, 0.0, 0, ChangeCause::Other);
        runtime.request_transition(key, POLICY);
        runtime.set_scalar_target(key, 10.0, 0, ChangeCause::Interaction);
        runtime.request_transition(key, POLICY);
        runtime.set_scalar_target(key, 2.0, 50, ChangeCause::Interaction);

        assert_eq!(runtime.sample_scalar(key, 2.0, 50), 5.0);
        assert_eq!(runtime.sample_scalar(key, 2.0, 100), 3.5);
        assert_eq!(runtime.sample_scalar(key, 2.0, 150), 2.0);
    }

    #[test]
    fn color_transition_samples_paint_domain() {
        let key = color_key();
        let mut runtime = AnimationRuntime::new();
        let from = Color::from_rgba8(0x00, 0x00, 0x00, 0xff);
        let to = Color::from_rgba8(0xff, 0xff, 0xff, 0xff);
        runtime.set_color_target(key, from, 0, ChangeCause::Other);
        runtime.request_transition(key, POLICY);
        runtime.set_color_target(key, to, 0, ChangeCause::Interaction);

        assert_eq!(key.property().metadata().affects(), PAINT.into_set());
        assert_eq!(
            runtime.sample_color(key, to, 50),
            Color::new([0.5, 0.5, 0.5, 1.0])
        );
    }

    #[test]
    fn offset_transition_samples_both_axes() {
        let key = offset_key();
        let mut runtime = AnimationRuntime::new();
        let from = Vec2::new(2.0, 4.0);
        let to = Vec2::new(12.0, 24.0);
        runtime.set_offset_target(key, from, 0, ChangeCause::Other);
        runtime.request_transition(key, POLICY);
        runtime.set_offset_target(key, to, 0, ChangeCause::Interaction);

        assert_eq!(runtime.sample_offset(key, to, 50), Vec2::new(7.0, 14.0));
    }

    #[test]
    fn completed_transition_is_removed_by_advance() {
        let key = scalar_key();
        let mut runtime = AnimationRuntime::new();
        runtime.set_scalar_target(key, 0.0, 0, ChangeCause::Other);
        runtime.request_transition(key, POLICY);
        runtime.set_scalar_target(key, 10.0, 0, ChangeCause::Interaction);

        assert!(runtime.has_active_track(key));
        let affected = runtime.advance(100);

        assert_eq!(affected, alloc::vec![(key.element(), COMPOSE.into_set())]);
        assert!(!runtime.has_active_track(key));
        assert_eq!(runtime.sample_scalar(key, 10.0, 150), 10.0);
    }

    #[test]
    fn other_changes_snap_to_target() {
        let key = scalar_key();
        let mut runtime = AnimationRuntime::new();
        runtime.set_scalar_target(key, 0.0, 0, ChangeCause::Other);
        runtime.request_transition(key, POLICY);
        runtime.set_scalar_target(key, 10.0, 0, ChangeCause::Other);

        assert!(!runtime.has_active_track(key));
        assert_eq!(runtime.sample_scalar(key, 10.0, 50), 10.0);
    }

    #[test]
    fn next_deadline_tracks_frame_and_completion() {
        let key = scalar_key();
        let mut runtime = AnimationRuntime::new();
        let long_spec = TransitionSpec::linear(PRESENTATION_FRAME_INTERVAL * 4);
        runtime.set_scalar_target(key, 0.0, 0, ChangeCause::Other);
        runtime.request_transition(key, TransitionPolicy::interaction(long_spec));
        runtime.set_scalar_target(key, 10.0, 0, ChangeCause::Interaction);

        assert_eq!(runtime.next_deadline(0), Some(PRESENTATION_FRAME_INTERVAL));
        assert_eq!(
            runtime.next_deadline(PRESENTATION_FRAME_INTERVAL * 4 - 10),
            Some(PRESENTATION_FRAME_INTERVAL * 4)
        );
    }

    #[test]
    fn remove_element_drops_tracks_targets_and_pending_specs() {
        let key = scalar_key();
        let mut runtime = AnimationRuntime::new();
        runtime.set_scalar_target(key, 0.0, 0, ChangeCause::Other);
        runtime.request_transition(key, POLICY);
        runtime.set_scalar_target(key, 10.0, 0, ChangeCause::Interaction);

        runtime.remove_element(key.element());

        assert!(!runtime.has_active_track(key));
        assert_eq!(runtime.next_deadline(0), None);
        assert_eq!(runtime.sample_scalar(key, 25.0, 50), 25.0);
    }
}
