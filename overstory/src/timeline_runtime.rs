// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Internal presentation timeline runtime.
//!
//! This is intentionally tiny: one implicit track per element/property and one
//! value-transition clip kind. The shape leaves room for a future shared
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
//! keyframes, layout animation, reduced-motion policy, and compositor
//! delegation. Layout may snap to the target while composition or paint samples
//! a presented value, which keeps ordinary interaction motion cheap and avoids
//! making animated layout the default.

use alloc::vec::Vec;

use hashbrown::HashMap;
use kurbo::Vec2;
use peniko::Color;
use understory_property::PropertyId;
use understory_timing::{TimerDuration, TimerInstant};

use crate::element::ElementId;
use crate::motion::{
    Animatable, ColorInterpolation, ColorTransition, Interpolate, ResolvedTransitionSpec,
    TimingFunction, Transition, TransitionTarget,
};

/// Nominal frame interval hosts can use while timeline tracks are active.
pub(crate) const PRESENTATION_FRAME_INTERVAL: TimerDuration = 16_666_667;

/// Stable id for a scheduled timeline cue.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TimelineCueId(u64);

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

/// Destination whose sampled value may differ from its target value.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum TimelineTarget {
    /// Dependency property whose animation value participates in normal
    /// property resolution.
    Property(PropertyId),
    /// Pipeline-derived visual output that is not an authored dependency
    /// property.
    VisualOutput(VisualOutput),
}

impl TimelineTarget {
    /// Returns the style-facing transition target for this runtime target.
    #[must_use]
    pub(crate) const fn transition_target(self) -> TransitionTarget {
        match self {
            Self::Property(property) => TransitionTarget::Property(property),
            Self::VisualOutput(VisualOutput::LayerOffset) => TransitionTarget::LayerOffset,
            Self::VisualOutput(VisualOutput::VisualTranslation) => {
                TransitionTarget::VisualTranslation
            }
        }
    }

    /// Returns the expected value kind when this target knows it statically.
    #[must_use]
    pub(crate) const fn expected_value_kind(self) -> Option<PresentationValueKind> {
        match self {
            Self::Property(_) => None,
            Self::VisualOutput(output) => Some(output.value_kind()),
        }
    }

    /// Returns where this target is sampled when it is not a dependency
    /// property.
    #[must_use]
    pub(crate) const fn visual_domain(self) -> Option<TimelineEvaluationDomain> {
        match self {
            Self::Property(_) => None,
            Self::VisualOutput(output) => Some(output.evaluation_domain()),
        }
    }
}

/// Pipeline output animated by the timeline but not represented as a property.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum VisualOutput {
    /// Layout-derived layer placement sampled by composition while layout keeps
    /// the resolved target geometry.
    ///
    /// Use this for moving real parts between layout positions. Transient
    /// feedback should use [`Self::VisualTranslation`] so it does not rewrite
    /// the part's target placement.
    LayerOffset,
    /// Additive transient visual translation sampled by composition after
    /// layout.
    VisualTranslation,
}

impl VisualOutput {
    const fn value_kind(self) -> PresentationValueKind {
        match self {
            Self::LayerOffset | Self::VisualTranslation => PresentationValueKind::Offset2d,
        }
    }

    const fn evaluation_domain(self) -> TimelineEvaluationDomain {
        match self {
            Self::LayerOffset | Self::VisualTranslation => TimelineEvaluationDomain::Composition,
        }
    }
}

/// Storage/interpolation kind for a timeline target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PresentationValueKind {
    /// A single `f32` value.
    Scalar32,
    /// A single `f64` value.
    Scalar64,
    /// A straight-alpha sRGB color.
    Color,
    /// A two-dimensional logical offset.
    Offset2d,
}

/// Frame-pipeline domain that samples the timeline value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TimelineEvaluationDomain {
    /// Sampled during layout. Avoid unless animated layout is intentional.
    Layout,
    /// Sampled during layer composition.
    Composition,
}

/// Typed target or sampled value carried by a timeline track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PresentationValue {
    /// A scalar value.
    Scalar32(f32),
    /// A scalar value.
    Scalar64(f64),
    /// A straight-alpha sRGB color.
    Color(Color),
    /// A two-dimensional logical offset.
    Offset2d(Vec2),
}

impl PresentationValue {
    const fn kind(self) -> PresentationValueKind {
        match self {
            Self::Scalar32(_) => PresentationValueKind::Scalar32,
            Self::Scalar64(_) => PresentationValueKind::Scalar64,
            Self::Color(_) => PresentationValueKind::Color,
            Self::Offset2d(_) => PresentationValueKind::Offset2d,
        }
    }
}

/// Value type that can be stored in erased timeline sample storage.
pub(crate) trait AnimatableValue: Copy + PartialEq {
    /// Runtime storage kind for this value.
    const KIND: PresentationValueKind;

    /// Converts this typed value into erased presentation storage.
    fn into_value(self) -> PresentationValue;

    /// Converts erased presentation storage back into this typed value.
    fn from_value(value: PresentationValue) -> Option<Self>;
}

impl AnimatableValue for f32 {
    const KIND: PresentationValueKind = PresentationValueKind::Scalar32;

    fn into_value(self) -> PresentationValue {
        PresentationValue::Scalar32(self)
    }

    fn from_value(value: PresentationValue) -> Option<Self> {
        match value {
            PresentationValue::Scalar32(value) => Some(value),
            PresentationValue::Scalar64(_)
            | PresentationValue::Color(_)
            | PresentationValue::Offset2d(_) => None,
        }
    }
}

impl AnimatableValue for f64 {
    const KIND: PresentationValueKind = PresentationValueKind::Scalar64;

    fn into_value(self) -> PresentationValue {
        PresentationValue::Scalar64(self)
    }

    fn from_value(value: PresentationValue) -> Option<Self> {
        match value {
            PresentationValue::Scalar64(value) => Some(value),
            PresentationValue::Scalar32(_)
            | PresentationValue::Color(_)
            | PresentationValue::Offset2d(_) => None,
        }
    }
}

impl AnimatableValue for Color {
    const KIND: PresentationValueKind = PresentationValueKind::Color;

    fn into_value(self) -> PresentationValue {
        PresentationValue::Color(self)
    }

    fn from_value(value: PresentationValue) -> Option<Self> {
        match value {
            PresentationValue::Color(value) => Some(value),
            PresentationValue::Scalar32(_)
            | PresentationValue::Scalar64(_)
            | PresentationValue::Offset2d(_) => None,
        }
    }
}

impl AnimatableValue for Vec2 {
    const KIND: PresentationValueKind = PresentationValueKind::Offset2d;

    fn into_value(self) -> PresentationValue {
        PresentationValue::Offset2d(self)
    }

    fn from_value(value: PresentationValue) -> Option<Self> {
        match value {
            PresentationValue::Offset2d(value) => Some(value),
            PresentationValue::Scalar32(_)
            | PresentationValue::Scalar64(_)
            | PresentationValue::Color(_) => None,
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

/// Key for one implicit timeline track.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct TimelineTrackKey {
    element: ElementId,
    target: TimelineTarget,
}

impl TimelineTrackKey {
    /// Creates a timeline track key.
    #[must_use]
    pub(crate) const fn new(element: ElementId, target: TimelineTarget) -> Self {
        Self { element, target }
    }

    /// Returns the element this track targets.
    #[must_use]
    pub(crate) const fn element(self) -> ElementId {
        self.element
    }

    /// Returns the timeline target this track targets.
    #[must_use]
    pub(crate) const fn target(self) -> TimelineTarget {
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

/// Active clip carried by one timeline track.
#[derive(Clone, Copy, Debug, PartialEq)]
enum MotionClip {
    Scalar32(Transition<f32>),
    Scalar64(Transition<f64>),
    Color(ColorTransition),
    Offset(Transition<Vec2>),
}

impl MotionClip {
    fn new(
        from: PresentationValue,
        to: PresentationValue,
        started_at: TimerInstant,
        spec: ResolvedTransitionSpec,
    ) -> Self {
        match (from, to) {
            (PresentationValue::Scalar32(from), PresentationValue::Scalar32(to)) => Self::Scalar32(
                Transition::new(from, to, started_at, spec.duration(), spec.timing()),
            ),
            (PresentationValue::Scalar64(from), PresentationValue::Scalar64(to)) => Self::Scalar64(
                Transition::new(from, to, started_at, spec.duration(), spec.timing()),
            ),
            (PresentationValue::Color(from), PresentationValue::Color(to)) => {
                Self::Color(ColorTransition::new(
                    from,
                    to,
                    started_at,
                    spec.duration(),
                    spec.timing(),
                    spec.color_interpolation(),
                ))
            }
            (PresentationValue::Offset2d(from), PresentationValue::Offset2d(to)) => Self::Offset(
                Transition::new(from, to, started_at, spec.duration(), spec.timing()),
            ),
            _ => unreachable!("timeline target kinds should match property metadata"),
        }
    }

    fn sample(self, now: TimerInstant) -> PresentationValue {
        match self {
            Self::Scalar32(transition) => transition.sample(now).into_value(),
            Self::Scalar64(transition) => transition.sample(now).into_value(),
            Self::Color(transition) => transition.sample(now).into_value(),
            Self::Offset(transition) => transition.sample(now).into_value(),
        }
    }

    fn is_complete(self, now: TimerInstant) -> bool {
        match self {
            Self::Scalar32(transition) => transition.is_complete(now),
            Self::Scalar64(transition) => transition.is_complete(now),
            Self::Color(transition) => transition.is_complete(now),
            Self::Offset(transition) => transition.is_complete(now),
        }
    }

    fn end_time(self) -> TimerInstant {
        match self {
            Self::Scalar32(transition) => transition.end_time(),
            Self::Scalar64(transition) => transition.end_time(),
            Self::Color(transition) => transition.end_time(),
            Self::Offset(transition) => transition.end_time(),
        }
    }
}

/// Active implicit timeline track.
#[derive(Clone, Copy, Debug, PartialEq)]
struct TimelineTrack {
    clip: MotionClip,
}

/// One keyframe in a finite presentation effect clip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EffectKeyframe {
    offset: TimerDuration,
    value: PresentationValue,
}

impl EffectKeyframe {
    /// Creates a keyframe at `offset` relative to clip start.
    #[must_use]
    pub(crate) const fn new(offset: TimerDuration, value: PresentationValue) -> Self {
        Self { offset, value }
    }
}

/// Finite presentation effect clip applied to one visual output.
///
/// This is for semantic feedback where there is no lasting target value to
/// retain, such as a short denied-action nudge. It deliberately lives beside
/// normal target transitions rather than on individual widgets.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EffectClip {
    keyframes: Vec<EffectKeyframe>,
    kind: PresentationValueKind,
}

impl EffectClip {
    /// Builds a clip from keyframes.
    ///
    /// The first keyframe supplies the sample before its timestamp. All
    /// keyframes must carry the same presentation value kind.
    #[must_use]
    pub(crate) fn from_keyframes(keyframes: impl IntoIterator<Item = EffectKeyframe>) -> Self {
        let mut keyframes: Vec<_> = keyframes.into_iter().collect();
        keyframes.sort_by_key(|keyframe| keyframe.offset);
        assert!(
            !keyframes.is_empty(),
            "effect clips require at least one keyframe"
        );
        let kind = keyframes[0].value.kind();
        debug_assert!(
            keyframes
                .iter()
                .all(|keyframe| keyframe.value.kind() == kind),
            "effect clip keyframes must use one value kind"
        );
        Self { keyframes, kind }
    }

    fn value_kind(&self) -> PresentationValueKind {
        self.kind
    }

    fn sample(&self, elapsed: TimerDuration) -> PresentationValue {
        let first = self.keyframes[0];
        if elapsed <= first.offset {
            return first.value;
        }
        for pair in self.keyframes.windows(2) {
            let from = pair[0];
            let to = pair[1];
            if elapsed <= to.offset {
                let span = to.offset.saturating_sub(from.offset);
                let progress = if span == 0 {
                    1.0
                } else {
                    (elapsed.saturating_sub(from.offset) as f32 / span as f32).clamp(0.0, 1.0)
                };
                return interpolate_presentation_value(from.value, to.value, progress);
            }
        }
        self.keyframes
            .last()
            .map(|keyframe| keyframe.value)
            .unwrap_or(first.value)
    }

    fn duration(&self) -> TimerDuration {
        self.keyframes
            .last()
            .map(|keyframe| keyframe.offset)
            .unwrap_or(0)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct EffectTrack {
    started_at: TimerInstant,
    clip: EffectClip,
}

impl EffectTrack {
    fn sample(&self, now: TimerInstant) -> PresentationValue {
        self.clip.sample(now.saturating_sub(self.started_at))
    }

    fn is_complete(&self, now: TimerInstant) -> bool {
        now.saturating_sub(self.started_at) >= self.clip.duration()
    }

    fn end_time(&self) -> TimerInstant {
        self.started_at.saturating_add(self.clip.duration())
    }
}

/// Internal runtime for sampled timeline values.
///
/// This is the degenerate form of a future global timeline: each active entry
/// is an anonymous timeline track keyed by element and target. Retargeting
/// starts from the current sampled value, not the previous target value.
#[derive(Debug, Default)]
pub(crate) struct TimelineRuntime {
    targets: HashMap<TimelineTrackKey, PresentationValue>,
    tracks: HashMap<TimelineTrackKey, TimelineTrack>,
    effects: HashMap<TimelineTrackKey, EffectTrack>,
    pending: HashMap<TimelineTrackKey, ResolvedTransitionSpec>,
    cues: Vec<TimelineCue>,
    next_cue_id: u64,
}

impl TimelineRuntime {
    /// Creates an empty runtime.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Schedules a cue for `due_at`.
    pub(crate) fn schedule_cue(
        &mut self,
        target: Option<ElementId>,
        due_at: TimerInstant,
        payload: u64,
    ) -> TimelineCueId {
        let id = TimelineCueId(self.next_cue_id);
        self.next_cue_id = self.next_cue_id.wrapping_add(1);
        self.cues.push(TimelineCue {
            id,
            target,
            payload,
            due_at,
        });
        id
    }

    /// Cancels a pending cue by id.
    pub(crate) fn cancel_cue(&mut self, id: TimelineCueId) -> bool {
        let before = self.cues.len();
        self.cues.retain(|cue| cue.id != id);
        self.cues.len() != before
    }

    /// Drains all app-visible cues due at or before `now`.
    pub(crate) fn drain_cues(&mut self, now: TimerInstant) -> Vec<TimelineCue> {
        let mut due = Vec::new();
        let mut pending = Vec::new();
        for cue in self.cues.drain(..) {
            if cue.due_at <= now {
                due.push(cue);
            } else {
                pending.push(cue);
            }
        }
        self.cues = pending;
        due.sort_by_key(|cue| (cue.due_at, cue.id.get()));
        due
    }

    /// Requests that the next target change for `key` animate when it is caused
    /// by interaction.
    pub(crate) fn request_transition(
        &mut self,
        key: TimelineTrackKey,
        spec: ResolvedTransitionSpec,
    ) {
        self.pending.insert(key, spec);
    }

    /// Applies a finite visual effect clip at `now`, replacing any existing
    /// effect clip on the same key.
    pub(crate) fn start_effect(
        &mut self,
        key: TimelineTrackKey,
        clip: EffectClip,
        now: TimerInstant,
    ) {
        if let Some(kind) = key.target().expected_value_kind() {
            debug_assert_eq!(
                kind,
                clip.value_kind(),
                "effect clip value kind must match target metadata"
            );
        }
        debug_assert_ne!(
            key.target().visual_domain(),
            Some(TimelineEvaluationDomain::Layout),
            "layout-domain effects need explicit layout-animation policy"
        );
        self.effects.insert(
            key,
            EffectTrack {
                started_at: now,
                clip,
            },
        );
    }

    /// Returns whether a transition spec is waiting for `key`'s next target.
    #[must_use]
    pub(crate) fn has_pending_transition(&self, key: TimelineTrackKey) -> bool {
        self.pending.contains_key(&key)
    }

    /// Returns whether `key` has a resolved target value.
    #[must_use]
    pub(crate) fn has_target(&self, key: TimelineTrackKey) -> bool {
        self.targets.contains_key(&key)
    }

    /// Removes the target, active track, and pending transition for `key`.
    pub(crate) fn clear_target(&mut self, key: TimelineTrackKey) -> bool {
        let had_target = self.targets.remove(&key).is_some();
        let had_track = self.tracks.remove(&key).is_some();
        let had_effect = self.effects.remove(&key).is_some();
        let had_pending = self.pending.remove(&key).is_some();
        had_target || had_track || had_effect || had_pending
    }

    /// Records the resolved target value for `key`, planning or retargeting
    /// its timeline track when policy allows.
    pub(crate) fn set_target<T: AnimatableValue>(
        &mut self,
        key: TimelineTrackKey,
        target: T,
        now: TimerInstant,
        cause: ChangeCause,
    ) -> bool {
        let target = target.into_value();
        if let Some(kind) = key.target().expected_value_kind() {
            debug_assert_eq!(
                kind,
                T::KIND,
                "timeline target kind must match target metadata"
            );
        }
        debug_assert_ne!(
            key.target().visual_domain(),
            Some(TimelineEvaluationDomain::Layout),
            "layout-domain animation must not be sampled by the generic target setter"
        );
        let previous_target = self.targets.get(&key).copied();
        let current_presented = self.sample_value(key, previous_target.unwrap_or(target), now);
        self.targets.insert(key, target);
        let target_changed = previous_target.is_none_or(|previous| !same_value(previous, target));
        if !target_changed {
            if cause == ChangeCause::Interaction {
                self.pending.remove(&key);
            }
            return false;
        }

        let had_track = self.tracks.contains_key(&key);
        let spec = if cause == ChangeCause::Interaction {
            self.pending.remove(&key)
        } else {
            self.pending.remove(&key);
            None
        };
        let Some(spec) = spec else {
            self.tracks.remove(&key);
            return had_track;
        };

        let from = current_presented;
        if same_value(from, target) || spec.duration() == 0 {
            self.tracks.remove(&key);
            return had_track;
        }

        self.tracks.insert(
            key,
            TimelineTrack {
                clip: MotionClip::new(from, target, now, spec),
            },
        );
        true
    }

    /// Returns the sampled timeline value for `key`.
    #[must_use]
    pub(crate) fn sample<T: AnimatableValue>(
        &self,
        key: TimelineTrackKey,
        target: T,
        now: TimerInstant,
    ) -> T {
        let value = self.sample_value(key, target.into_value(), now);
        T::from_value(value).unwrap_or(target)
    }

    /// Returns the sampled timeline value for `key`.
    #[must_use]
    pub(crate) fn sample_value(
        &self,
        key: TimelineTrackKey,
        target: PresentationValue,
        now: TimerInstant,
    ) -> PresentationValue {
        if let Some(effect) = self.effects.get(&key) {
            return effect.sample(now);
        }
        self.tracks
            .get(&key)
            .map_or(target, |track| track.clip.sample(now))
    }

    /// Returns the sampled value for an active track, if one exists.
    #[must_use]
    pub(crate) fn active_sample_value(
        &self,
        key: TimelineTrackKey,
        now: TimerInstant,
    ) -> Option<PresentationValue> {
        self.effects
            .get(&key)
            .map(|effect| effect.sample(now))
            .or_else(|| self.tracks.get(&key).map(|track| track.clip.sample(now)))
    }

    /// Advances active tracks, removing completed clips and returning affected
    /// samples that should be applied to their targets.
    pub(crate) fn advance(&mut self, now: TimerInstant) -> Vec<TimelineSample> {
        let mut samples = Vec::new();
        let mut completed = Vec::new();
        for (&key, track) in &self.tracks {
            let complete = track.clip.is_complete(now);
            samples.push(TimelineSample {
                key,
                value: track.clip.sample(now),
                complete,
            });
            if complete {
                completed.push(key);
            }
        }
        for key in completed {
            self.tracks.remove(&key);
        }
        let mut completed = Vec::new();
        for (&key, effect) in &self.effects {
            let complete = effect.is_complete(now);
            samples.push(TimelineSample {
                key,
                value: effect.sample(now),
                complete,
            });
            if complete {
                completed.push(key);
            }
        }
        for key in completed {
            self.effects.remove(&key);
        }
        samples
    }

    /// Returns the next deadline a host should wake for active presentation
    /// tracks.
    #[must_use]
    pub(crate) fn next_deadline(&self, now: TimerInstant) -> Option<TimerInstant> {
        let track_deadline = if self.tracks.is_empty() && self.effects.is_empty() {
            None
        } else {
            let next_frame = now.saturating_add(PRESENTATION_FRAME_INTERVAL);
            let next_end = self
                .tracks
                .values()
                .map(|track| track.clip.end_time())
                .chain(self.effects.values().map(EffectTrack::end_time))
                .min();
            Some(next_end.map_or(next_frame, |end| next_frame.min(end).max(now)))
        };
        let cue_deadline = self.cues.iter().map(|cue| cue.due_at.max(now)).min();
        min_deadline(track_deadline, cue_deadline)
    }

    /// Returns the number of retained timeline targets.
    #[must_use]
    pub(crate) fn target_count(&self) -> usize {
        self.targets.len()
    }

    /// Returns the number of active timeline tracks.
    #[must_use]
    pub(crate) fn active_track_count(&self) -> usize {
        self.tracks.len()
    }

    /// Returns the number of pending transition specs.
    #[must_use]
    pub(crate) fn pending_transition_count(&self) -> usize {
        self.pending.len()
    }

    /// Returns the number of pending timeline cues.
    #[must_use]
    pub(crate) fn pending_cue_count(&self) -> usize {
        self.cues.len()
    }

    /// Removes all runtime state for `element`.
    pub(crate) fn remove_element(&mut self, element: ElementId) {
        self.targets.retain(|key, _| key.element() != element);
        self.tracks.retain(|key, _| key.element() != element);
        self.effects.retain(|key, _| key.element() != element);
        self.pending.retain(|key, _| key.element() != element);
        self.cues.retain(|cue| cue.target != Some(element));
    }

    /// Returns whether `key` currently has an active timeline track.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn has_active_track(&self, key: TimelineTrackKey) -> bool {
        self.tracks.contains_key(&key) || self.effects.contains_key(&key)
    }
}

fn interpolate_presentation_value(
    from: PresentationValue,
    to: PresentationValue,
    progress: f32,
) -> PresentationValue {
    match (from, to) {
        (PresentationValue::Scalar32(from), PresentationValue::Scalar32(to)) => {
            PresentationValue::Scalar32(from.interpolate(to, progress))
        }
        (PresentationValue::Scalar64(from), PresentationValue::Scalar64(to)) => {
            PresentationValue::Scalar64(from.interpolate(to, progress))
        }
        (PresentationValue::Color(from), PresentationValue::Color(to)) => PresentationValue::Color(
            ColorTransition::new(
                from,
                to,
                0,
                1_000_000,
                TimingFunction::LINEAR,
                ColorInterpolation::SRGB,
            )
            .sample(progress_timer_instant(progress)),
        ),
        (PresentationValue::Offset2d(from), PresentationValue::Offset2d(to)) => {
            PresentationValue::Offset2d(from.interpolate(to, progress))
        }
        _ => to,
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "effect progress is clamped to 0..=1 before conversion to the fixed sample span"
)]
fn progress_timer_instant(progress: f32) -> TimerInstant {
    (f64::from(progress.clamp(0.0, 1.0)) * 1_000_000.0).round() as TimerInstant
}

fn same_value(a: PresentationValue, b: PresentationValue) -> bool {
    match (a, b) {
        (PresentationValue::Scalar32(a), PresentationValue::Scalar32(b)) => a.is_close(b),
        (PresentationValue::Scalar64(a), PresentationValue::Scalar64(b)) => a.is_close(b),
        (PresentationValue::Color(a), PresentationValue::Color(b)) => same_color(a, b),
        (PresentationValue::Offset2d(a), PresentationValue::Offset2d(b)) => a.is_close(b),
        _ => false,
    }
}

fn same_color(a: Color, b: Color) -> bool {
    a.components
        .into_iter()
        .zip(b.components)
        .all(|(a, b)| a.is_close(b))
}

fn min_deadline(a: Option<TimerInstant>, b: Option<TimerInstant>) -> Option<TimerInstant> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: ResolvedTransitionSpec = ResolvedTransitionSpec::linear(100);
    const SCALAR_PROPERTY: PropertyId = PropertyId::new(1);
    const COLOR_PROPERTY: PropertyId = PropertyId::new(2);

    fn scalar_key() -> TimelineTrackKey {
        TimelineTrackKey::new(
            ElementId::new(7, 0),
            TimelineTarget::Property(SCALAR_PROPERTY),
        )
    }

    fn color_key() -> TimelineTrackKey {
        TimelineTrackKey::new(
            ElementId::new(7, 0),
            TimelineTarget::Property(COLOR_PROPERTY),
        )
    }

    fn offset_key() -> TimelineTrackKey {
        TimelineTrackKey::new(
            ElementId::new(7, 0),
            TimelineTarget::VisualOutput(VisualOutput::LayerOffset),
        )
    }

    fn visual_translation_key() -> TimelineTrackKey {
        TimelineTrackKey::new(
            ElementId::new(7, 0),
            TimelineTarget::VisualOutput(VisualOutput::VisualTranslation),
        )
    }

    #[test]
    fn scalar_transition_samples_linearly() {
        let key = scalar_key();
        let mut runtime = TimelineRuntime::new();
        runtime.set_target(key, 0.0_f32, 0, ChangeCause::Other);
        runtime.request_transition(key, SPEC);
        runtime.set_target(key, 10.0_f32, 10, ChangeCause::Interaction);

        assert_eq!(runtime.sample(key, 10.0_f32, 10), 0.0);
        assert_eq!(runtime.sample(key, 10.0_f32, 60), 5.0);
        assert_eq!(runtime.sample(key, 10.0_f32, 110), 10.0);
    }

    #[test]
    fn scalar_transition_retargets_from_sampled_value() {
        let key = scalar_key();
        let mut runtime = TimelineRuntime::new();
        runtime.set_target(key, 0.0_f32, 0, ChangeCause::Other);
        runtime.request_transition(key, SPEC);
        runtime.set_target(key, 10.0_f32, 0, ChangeCause::Interaction);
        runtime.request_transition(key, SPEC);
        runtime.set_target(key, 2.0_f32, 50, ChangeCause::Interaction);

        assert_eq!(runtime.sample(key, 2.0_f32, 50), 5.0);
        assert_eq!(runtime.sample(key, 2.0_f32, 100), 3.5);
        assert_eq!(runtime.sample(key, 2.0_f32, 150), 2.0);
    }

    #[test]
    fn color_transition_samples_linearly() {
        let key = color_key();
        let mut runtime = TimelineRuntime::new();
        let from = Color::from_rgba8(0x00, 0x00, 0x00, 0xff);
        let to = Color::from_rgba8(0xff, 0xff, 0xff, 0xff);
        runtime.set_target(key, from, 0, ChangeCause::Other);
        runtime.request_transition(key, SPEC);
        runtime.set_target(key, to, 0, ChangeCause::Interaction);

        assert_eq!(
            runtime.sample(key, to, 50),
            Color::new([0.5, 0.5, 0.5, 1.0])
        );
    }

    #[test]
    fn offset_transition_samples_both_axes() {
        let key = offset_key();
        let mut runtime = TimelineRuntime::new();
        let from = Vec2::new(2.0, 4.0);
        let to = Vec2::new(12.0, 24.0);
        runtime.set_target(key, from, 0, ChangeCause::Other);
        runtime.request_transition(key, SPEC);
        runtime.set_target(key, to, 0, ChangeCause::Interaction);

        assert_eq!(runtime.sample(key, to, 50), Vec2::new(7.0, 14.0));
    }

    #[test]
    fn completed_transition_is_removed_by_advance() {
        let key = scalar_key();
        let mut runtime = TimelineRuntime::new();
        runtime.set_target(key, 0.0_f32, 0, ChangeCause::Other);
        runtime.request_transition(key, SPEC);
        runtime.set_target(key, 10.0_f32, 0, ChangeCause::Interaction);

        assert!(runtime.has_active_track(key));
        let samples = runtime.advance(100);

        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].key(), key);
        assert_eq!(samples[0].value(), PresentationValue::Scalar32(10.0));
        assert!(samples[0].is_complete());
        assert!(!runtime.has_active_track(key));
        assert_eq!(runtime.sample(key, 10.0_f32, 150), 10.0);
    }

    #[test]
    fn idle_sample_uses_supplied_target_instead_of_stored_target() {
        let key = color_key();
        let mut runtime = TimelineRuntime::new();
        let old = Color::from_rgba8(0x00, 0x00, 0x00, 0xff);
        let new = Color::from_rgba8(0xff, 0x00, 0x00, 0xff);

        runtime.set_target(key, old, 0, ChangeCause::Other);

        assert_eq!(runtime.sample(key, new, 50), new);
    }

    #[test]
    fn other_changes_snap_to_target() {
        let key = scalar_key();
        let mut runtime = TimelineRuntime::new();
        runtime.set_target(key, 0.0_f32, 0, ChangeCause::Other);
        runtime.request_transition(key, SPEC);
        runtime.set_target(key, 10.0_f32, 0, ChangeCause::Other);

        assert!(!runtime.has_active_track(key));
        assert_eq!(runtime.sample(key, 10.0_f32, 50), 10.0);
    }

    #[test]
    fn clear_target_drops_target_track_and_pending_transition() {
        let key = scalar_key();
        let mut runtime = TimelineRuntime::new();
        runtime.set_target(key, 0.0_f32, 0, ChangeCause::Other);
        runtime.request_transition(key, SPEC);
        runtime.set_target(key, 10.0_f32, 0, ChangeCause::Interaction);

        assert!(runtime.has_active_track(key));
        assert!(runtime.clear_target(key));
        assert!(!runtime.has_active_track(key));
        assert!(!runtime.has_pending_transition(key));
        assert_eq!(runtime.sample(key, 2.0_f32, 50), 2.0);
        assert!(!runtime.clear_target(key));
    }

    #[test]
    fn next_deadline_tracks_frame_and_completion() {
        let key = scalar_key();
        let mut runtime = TimelineRuntime::new();
        let long_spec = ResolvedTransitionSpec::linear(PRESENTATION_FRAME_INTERVAL * 4);
        runtime.set_target(key, 0.0_f32, 0, ChangeCause::Other);
        runtime.request_transition(key, long_spec);
        runtime.set_target(key, 10.0_f32, 0, ChangeCause::Interaction);

        assert_eq!(runtime.next_deadline(0), Some(PRESENTATION_FRAME_INTERVAL));
        assert_eq!(
            runtime.next_deadline(PRESENTATION_FRAME_INTERVAL * 4 - 10),
            Some(PRESENTATION_FRAME_INTERVAL * 4)
        );
    }

    #[test]
    fn scheduled_cues_drain_in_due_then_id_order() {
        let mut runtime = TimelineRuntime::new();
        let second = runtime.schedule_cue(None, 20, 2);
        let first = runtime.schedule_cue(None, 10, 1);
        let third = runtime.schedule_cue(None, 20, 3);

        assert_eq!(runtime.next_deadline(0), Some(10));
        assert_eq!(
            runtime.drain_cues(20),
            alloc::vec![
                TimelineCue {
                    id: first,
                    target: None,
                    payload: 1,
                    due_at: 10,
                },
                TimelineCue {
                    id: second,
                    target: None,
                    payload: 2,
                    due_at: 20,
                },
                TimelineCue {
                    id: third,
                    target: None,
                    payload: 3,
                    due_at: 20,
                },
            ]
        );
        assert_eq!(runtime.next_deadline(20), None);
    }

    #[test]
    fn cancel_and_remove_element_drop_pending_cues() {
        let mut runtime = TimelineRuntime::new();
        let element = ElementId::new(7, 0);
        let cancelled = runtime.schedule_cue(None, 10, 1);
        let scoped = runtime.schedule_cue(Some(element), 10, 2);
        let kept = runtime.schedule_cue(None, 10, 3);

        assert!(runtime.cancel_cue(cancelled));
        runtime.remove_element(element);

        assert_eq!(
            runtime.drain_cues(10),
            alloc::vec![TimelineCue {
                id: kept,
                target: None,
                payload: 3,
                due_at: 10,
            }]
        );
        assert!(!runtime.cancel_cue(scoped));
    }

    #[test]
    fn effect_clip_samples_and_completes_without_retained_target() {
        let key = visual_translation_key();
        let mut runtime = TimelineRuntime::new();
        runtime.start_effect(
            key,
            EffectClip::from_keyframes([
                EffectKeyframe::new(0, PresentationValue::Offset2d(Vec2::ZERO)),
                EffectKeyframe::new(100, PresentationValue::Offset2d(Vec2::new(10.0, 0.0))),
                EffectKeyframe::new(200, PresentationValue::Offset2d(Vec2::ZERO)),
            ]),
            10,
        );

        assert_eq!(runtime.sample(key, Vec2::ZERO, 60), Vec2::new(5.0, 0.0));
        assert_eq!(runtime.target_count(), 0);

        let samples = runtime.advance(210);
        assert_eq!(samples.len(), 1);
        assert!(samples[0].is_complete());
        assert_eq!(samples[0].value(), PresentationValue::Offset2d(Vec2::ZERO));
        assert!(!runtime.has_active_track(key));
    }

    #[test]
    fn remove_element_drops_tracks_targets_and_pending_specs() {
        let key = scalar_key();
        let effect_key = visual_translation_key();
        let mut runtime = TimelineRuntime::new();
        runtime.set_target(key, 0.0_f32, 0, ChangeCause::Other);
        runtime.request_transition(key, SPEC);
        runtime.set_target(key, 10.0_f32, 0, ChangeCause::Interaction);
        runtime.start_effect(
            effect_key,
            EffectClip::from_keyframes([
                EffectKeyframe::new(0, PresentationValue::Offset2d(Vec2::ZERO)),
                EffectKeyframe::new(100, PresentationValue::Offset2d(Vec2::new(10.0, 0.0))),
            ]),
            0,
        );

        runtime.remove_element(key.element());

        assert!(!runtime.has_active_track(key));
        assert!(!runtime.has_active_track(effect_key));
        assert_eq!(runtime.next_deadline(0), None);
        assert_eq!(runtime.sample(key, 25.0_f32, 50), 25.0);
        assert_eq!(runtime.sample(effect_key, Vec2::ZERO, 50), Vec2::ZERO);
    }
}
