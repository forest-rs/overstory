// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use kurbo::Vec2;
use peniko::Color;
use understory_animation::{
    AnimationPlayState, AnimationTiming, CompositeOperation, FillMode, PlaybackDirection,
    TimingPhase,
};
use understory_animation_timeline::{AnimationTimeline, TimelineTime};
use understory_property::PropertyId;

use crate::element::ElementId;
use crate::motion::{
    Animatable, EffectClip, EffectKeyframe, Interpolate, PresentationSlot, PresentationTarget,
    PresentationValue, TimingFunction,
};
use understory_motion::ColorInterpolation;

use super::timeline::DocumentTimeline;
use super::*;

const SPEC: ResolvedTransition = ResolvedTransition::linear(100);
const SCALAR_PROPERTY: PropertyId = PropertyId::new(1);
const COLOR_PROPERTY: PropertyId = PropertyId::new(2);

#[derive(Clone, Debug, PartialEq)]
struct Gauge(f64);

impl Interpolate for Gauge {
    fn interpolate(&self, to: &Self, progress: f64) -> Self {
        Self(self.0.interpolate(&to.0, progress))
    }
}

impl Animatable for Gauge {
    type Velocity = f64;

    fn is_close(&self, other: &Self) -> bool {
        self.0.is_close(&other.0)
    }

    fn add(&self, other: &Self) -> Option<Self> {
        self.0.add(&other.0).map(Self)
    }

    fn accumulate(&self, other: &Self, count: u64) -> Option<Self> {
        self.0.accumulate(&other.0, count).map(Self)
    }
}

fn scalar_key() -> TimelineTrackKey {
    TimelineTrackKey::new(
        ElementId::new(7, 0),
        PresentationTarget::Property(SCALAR_PROPERTY),
    )
}

fn color_key() -> TimelineTrackKey {
    TimelineTrackKey::new(
        ElementId::new(7, 0),
        PresentationTarget::Property(COLOR_PROPERTY),
    )
}

fn offset_key() -> TimelineTrackKey {
    TimelineTrackKey::new(
        ElementId::new(7, 0),
        PresentationTarget::Slot(PresentationSlot::LayerOffset),
    )
}

fn native_key() -> TimelineTrackKey {
    TimelineTrackKey::new(
        ElementId::new(8, 0),
        PresentationTarget::Property(PropertyId::new(88)),
    )
}

fn visual_translation_key() -> TimelineTrackKey {
    TimelineTrackKey::new(
        ElementId::new(7, 0),
        PresentationTarget::Slot(PresentationSlot::VisualTranslation),
    )
}

fn presentation_control_result(result: TimelineTargetControlResult) -> TimelineControlResult {
    match result {
        TimelineTargetControlResult::Presentation(result) => result,
        TimelineTargetControlResult::Native(_) => {
            panic!("expected presentation control result")
        }
    }
}

fn presentation_samples(samples: Vec<TimelineTargetSample>) -> Vec<TimelineSample> {
    samples
        .into_iter()
        .map(|sample| match sample {
            TimelineTargetSample::Presentation(sample) => sample,
            TimelineTargetSample::Native(_) => panic!("expected presentation sample"),
        })
        .collect()
}

fn native_samples(samples: Vec<TimelineTargetSample>) -> Vec<NativeTimelineSample> {
    samples
        .into_iter()
        .map(|sample| match sample {
            TimelineTargetSample::Presentation(_) => panic!("expected native sample"),
            TimelineTargetSample::Native(sample) => sample,
        })
        .collect()
}

#[test]
fn document_timeline_maps_host_instants_to_timeline_time() {
    let timeline = DocumentTimeline;

    assert_eq!(
        timeline.current_time(&42).map(TimelineTime::duration),
        Some(42)
    );
    assert_eq!(timeline.time_at(99).duration(), 99);
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
fn color_transition_uses_resolved_interpolation_policy() {
    let key = color_key();
    let mut runtime = TimelineRuntime::new();
    let from = Color::from_rgba8(0xff, 0x00, 0x00, 0xff);
    let to = Color::from_rgba8(0x00, 0x00, 0xff, 0xff);
    let spec = ResolvedTransition::new(100, 0, TimingFunction::LINEAR, ColorInterpolation::OKLAB);
    runtime.set_target(key, from, 0, ChangeCause::Other);
    runtime.request_transition(key, spec);
    runtime.set_target(key, to, 0, ChangeCause::Interaction);

    assert_eq!(
        runtime.sample(key, to, 50),
        ColorInterpolation::OKLAB.interpolate(from, to, 0.5)
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
fn native_transition_samples_custom_target_value() {
    let key = native_key();
    let mut runtime = TimelineRuntime::new();
    assert_eq!(
        runtime.set_native_target_value(key, Gauge(0.0), 0, ChangeCause::Other, None),
        Some(false)
    );
    runtime.request_transition(key, SPEC);

    assert_eq!(
        runtime.set_native_target_value(key, Gauge(10.0), 0, ChangeCause::Interaction, None),
        Some(true)
    );
    assert_eq!(runtime.target_count(), 1);
    assert_eq!(runtime.active_transition_count(), 1);
    assert_eq!(runtime.active_effect_count(), 0);
    assert_eq!(
        runtime
            .active_sample(key, 50)
            .and_then(TimelineTargetSample::into_native::<Gauge>),
        Some(Gauge(5.0))
    );

    assert!(runtime.animation_rows(50).is_empty());

    let samples = native_samples(runtime.advance(100));
    assert_eq!(samples.len(), 1);
    assert!(samples[0].is_complete());
    assert_eq!(samples[0].value().downcast_ref(), Some(&Gauge(10.0)));
    assert_eq!(runtime.active_transition_count(), 0);
    assert_eq!(runtime.target_count(), 1);
}

#[test]
fn completed_transition_is_removed_by_advance() {
    let key = scalar_key();
    let mut runtime = TimelineRuntime::new();
    runtime.set_target(key, 0.0_f32, 0, ChangeCause::Other);
    runtime.request_transition(key, SPEC);
    runtime.set_target(key, 10.0_f32, 0, ChangeCause::Interaction);

    assert!(runtime.has_active_track(key));
    let samples = presentation_samples(runtime.advance(100));

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
fn same_value_interaction_consumes_pending_transition() {
    let key = scalar_key();
    let mut runtime = TimelineRuntime::new();
    runtime.set_target(key, 1.0_f32, 0, ChangeCause::Other);
    runtime.request_transition(key, SPEC);

    assert!(runtime.has_pending_transition(key));
    assert!(!runtime.set_target(key, 1.0_f32, 0, ChangeCause::Interaction));

    assert!(!runtime.has_pending_transition(key));
    assert!(!runtime.has_active_track(key));
}

#[test]
fn zero_duration_transition_does_not_create_active_track() {
    let key = scalar_key();
    let mut runtime = TimelineRuntime::new();
    runtime.set_target(key, 0.0_f32, 0, ChangeCause::Other);
    runtime.request_transition(key, ResolvedTransition::linear(0));

    assert!(!runtime.set_target(key, 10.0_f32, 0, ChangeCause::Interaction));
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
    assert!(runtime.clear_target(key, 25));
    assert!(!runtime.has_active_track(key));
    assert!(!runtime.has_pending_transition(key));
    assert_eq!(runtime.sample(key, 2.0_f32, 50), 2.0);
    assert!(!runtime.clear_target(key, 50));
}

#[test]
fn active_motion_does_not_arm_timer_deadline() {
    let key = scalar_key();
    let mut runtime = TimelineRuntime::new();
    runtime.set_target(key, 0.0_f32, 0, ChangeCause::Other);
    runtime.request_transition(key, ResolvedTransition::linear(100));
    runtime.set_target(key, 10.0_f32, 0, ChangeCause::Interaction);

    assert!(runtime.has_active_motion());
    assert_eq!(runtime.next_cue_deadline(0), None);
}

#[test]
fn transition_sampling_depends_on_timeline_time_not_frame_cadence() {
    fn sample_after_advancing(frames: &[u64]) -> f32 {
        let key = scalar_key();
        let mut runtime = TimelineRuntime::new();
        runtime.set_target(key, 0.0_f32, 0, ChangeCause::Other);
        runtime.request_transition(key, ResolvedTransition::linear(120));
        runtime.set_target(key, 120.0_f32, 0, ChangeCause::Interaction);

        let mut sample = PresentationValue::Scalar32(0.0);
        for frame in frames {
            sample = presentation_samples(runtime.advance(*frame))[0].value();
        }
        match sample {
            PresentationValue::Scalar32(value) => value,
            _ => panic!("expected scalar sample"),
        }
    }

    assert_eq!(
        sample_after_advancing(&[8, 16, 24, 32, 40, 48, 56, 64, 72]),
        72.0
    );
    assert_eq!(sample_after_advancing(&[16, 32, 48, 64, 72]), 72.0);
    assert_eq!(sample_after_advancing(&[33, 66, 72]), 72.0);
}

#[test]
fn scheduled_cues_drain_in_due_then_id_order() {
    let mut runtime = TimelineRuntime::new();
    let second = runtime.schedule_cue(None, 20, 2);
    let first = runtime.schedule_cue(None, 10, 1);
    let third = runtime.schedule_cue(None, 20, 3);

    assert_eq!(runtime.next_cue_deadline(0), Some(10));
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
    assert_eq!(runtime.next_cue_deadline(20), None);
}

#[test]
fn cancel_and_remove_element_drop_pending_cues() {
    let mut runtime = TimelineRuntime::new();
    let element = ElementId::new(7, 0);
    let cancelled = runtime.schedule_cue(None, 10, 1);
    let scoped = runtime.schedule_cue(Some(element), 10, 2);
    let kept = runtime.schedule_cue(None, 10, 3);

    assert!(runtime.cancel_cue(cancelled));
    runtime.remove_element(element, 10);

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
    let id = runtime.start_effect(
        key,
        EffectClip::from_keyframes([
            EffectKeyframe::new(0, PresentationValue::Offset2d(Vec2::ZERO)),
            EffectKeyframe::new(100, PresentationValue::Offset2d(Vec2::new(10.0, 0.0))),
            EffectKeyframe::new(200, PresentationValue::Offset2d(Vec2::ZERO)),
        ]),
        10,
    );
    assert_eq!(
        runtime.animation_state(id),
        Some(AnimationPlayState::Running)
    );
    assert_eq!(runtime.drain_animation_events()[0].id(), id);

    assert_eq!(runtime.sample(key, Vec2::ZERO, 60), Vec2::new(5.0, 0.0));
    assert_eq!(runtime.target_count(), 0);

    let samples = presentation_samples(runtime.advance(210));
    assert_eq!(samples.len(), 1);
    assert!(samples[0].is_complete());
    assert_eq!(samples[0].value(), PresentationValue::Offset2d(Vec2::ZERO));
    assert!(!runtime.has_active_track(key));
}

#[test]
fn effect_clip_timing_retimes_keyframe_curve() {
    let key = visual_translation_key();
    let mut runtime = TimelineRuntime::new();
    runtime.start_effect(
        key,
        EffectClip::from_keyframes([
            EffectKeyframe::new(0, PresentationValue::Offset2d(Vec2::ZERO)),
            EffectKeyframe::new(100, PresentationValue::Offset2d(Vec2::new(10.0, 0.0))),
        ])
        .with_timing(AnimationTiming {
            duration: 200,
            fill: FillMode::Forwards,
            ..AnimationTiming::new(200)
        }),
        0,
    );

    assert_eq!(runtime.sample(key, Vec2::ZERO, 50), Vec2::new(2.5, 0.0));

    let pending = presentation_samples(runtime.advance(100));
    assert_eq!(pending.len(), 1);
    assert!(!pending[0].is_complete());
    assert_eq!(
        pending[0].value(),
        PresentationValue::Offset2d(Vec2::new(5.0, 0.0))
    );

    let complete = presentation_samples(runtime.advance(200));
    assert_eq!(complete.len(), 1);
    assert!(complete[0].is_complete());
    assert_eq!(
        complete[0].value(),
        PresentationValue::Offset2d(Vec2::new(10.0, 0.0))
    );
}

#[test]
fn delayed_effect_uses_backwards_fill_before_active_interval() {
    let key = visual_translation_key();
    let mut runtime = TimelineRuntime::new();
    runtime.start_effect(
        key,
        EffectClip::from_keyframes([
            EffectKeyframe::new(0, PresentationValue::Offset2d(Vec2::new(10.0, 0.0))),
            EffectKeyframe::new(100, PresentationValue::Offset2d(Vec2::new(20.0, 0.0))),
        ])
        .with_timing(AnimationTiming {
            delay: 50,
            duration: 100,
            fill: FillMode::Backwards,
            ..AnimationTiming::new(100)
        }),
        0,
    );

    assert_eq!(
        runtime.sample(key, Vec2::ZERO, 25),
        Vec2::new(10.0, 0.0),
        "backwards fill should sample the first keyframe before the active interval",
    );
    assert_eq!(runtime.sample(key, Vec2::ZERO, 75), Vec2::new(12.5, 0.0));
}

#[test]
fn end_delay_retains_effect_until_full_timing_duration() {
    let key = visual_translation_key();
    let mut runtime = TimelineRuntime::new();
    runtime.start_effect(
        key,
        EffectClip::from_keyframes([
            EffectKeyframe::new(0, PresentationValue::Offset2d(Vec2::ZERO)),
            EffectKeyframe::new(100, PresentationValue::Offset2d(Vec2::new(20.0, 0.0))),
        ])
        .with_timing(AnimationTiming {
            duration: 100,
            end_delay: 50,
            fill: FillMode::Forwards,
            ..AnimationTiming::new(100)
        }),
        0,
    );

    let pending = presentation_samples(runtime.advance(125));
    assert_eq!(pending.len(), 1);
    assert!(!pending[0].is_complete());
    assert_eq!(
        pending[0].value(),
        PresentationValue::Offset2d(Vec2::new(20.0, 0.0))
    );

    let complete = presentation_samples(runtime.advance(150));
    assert_eq!(complete.len(), 1);
    assert!(complete[0].is_complete());
    assert_eq!(
        complete[0].value(),
        PresentationValue::Offset2d(Vec2::new(20.0, 0.0))
    );
}

#[test]
fn manual_timeline_effect_samples_when_timeline_is_sought() {
    let key = scalar_key();
    let mut runtime = TimelineRuntime::new();
    runtime.set_target(key, 0.0_f32, 0, ChangeCause::Other);
    let timeline = runtime.create_manual_timeline();
    assert_eq!(runtime.manual_timeline_time(timeline), Some(0));

    let animation = runtime.start_effect_on_timeline(
        key,
        EffectClip::from_keyframes([
            EffectKeyframe::new(0, PresentationValue::Scalar32(0.0)),
            EffectKeyframe::new(100, PresentationValue::Scalar32(10.0)),
        ]),
        timeline,
        1000,
    );

    assert_eq!(
        runtime.animation_state(animation),
        Some(AnimationPlayState::Running)
    );
    assert!(
        !runtime.has_active_motion(),
        "manual timelines should not request document-frame animation demand"
    );
    assert_eq!(runtime.sample(key, 0.0_f32, 2000), 0.0);

    let samples = presentation_samples(
        runtime
            .seek_manual_timeline(timeline, 50, 2000)
            .expect("manual timeline should exist"),
    );
    assert_eq!(samples.len(), 1);
    assert!(!samples[0].is_complete());
    assert_eq!(samples[0].value(), PresentationValue::Scalar32(5.0));
    assert_eq!(runtime.animation_current_time(animation, 9000), Some(50));
    assert_eq!(runtime.sample(key, 0.0_f32, 9000), 5.0);

    let samples = presentation_samples(
        runtime
            .seek_manual_timeline(timeline, 100, 3000)
            .expect("manual timeline should exist"),
    );
    assert_eq!(samples.len(), 1);
    assert!(
        !samples[0].is_complete(),
        "manual timeline endpoint seeks should keep retained tracks scrub-capable",
    );
    assert_eq!(samples[0].value(), PresentationValue::Scalar32(10.0));
    assert!(runtime.has_active_track(key));

    let samples = presentation_samples(
        runtime
            .seek_manual_timeline(timeline, 25, 4000)
            .expect("manual timeline should exist"),
    );
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].value(), PresentationValue::Scalar32(2.5));
    assert_eq!(runtime.animation_current_time(animation, 9000), Some(25));
}

#[test]
fn effect_clip_can_add_over_underlying_target() {
    let key = scalar_key();
    let mut runtime = TimelineRuntime::new();
    runtime.set_target(key, 10.0_f32, 0, ChangeCause::Other);
    runtime.start_effect(
        key,
        EffectClip::from_keyframes([
            EffectKeyframe::new(0, PresentationValue::Scalar32(2.0)),
            EffectKeyframe::new(100, PresentationValue::Scalar32(4.0)),
        ])
        .with_composite(CompositeOperation::Add),
        0,
    );

    assert_eq!(runtime.sample(key, 10.0_f32, 50), 13.0);
}

#[test]
fn effect_clip_can_accumulate_over_underlying_target() {
    let key = scalar_key();
    let mut runtime = TimelineRuntime::new();
    runtime.set_target(key, 10.0_f32, 0, ChangeCause::Other);
    runtime.start_effect(
        key,
        EffectClip::from_keyframes([
            EffectKeyframe::new(0, PresentationValue::Scalar32(2.0)),
            EffectKeyframe::new(100, PresentationValue::Scalar32(4.0)),
        ])
        .with_composite(CompositeOperation::Accumulate),
        0,
    );

    assert_eq!(runtime.sample(key, 10.0_f32, 50), 13.0);
}

#[test]
fn multiple_effect_clips_stack_and_cancel_by_id() {
    let key = scalar_key();
    let mut runtime = TimelineRuntime::new();
    runtime.set_target(key, 10.0_f32, 0, ChangeCause::Other);
    let first = runtime.start_effect(
        key,
        EffectClip::from_keyframes([
            EffectKeyframe::new(0, PresentationValue::Scalar32(2.0)),
            EffectKeyframe::new(100, PresentationValue::Scalar32(2.0)),
        ])
        .with_composite(CompositeOperation::Add),
        0,
    );
    let second = runtime.start_effect(
        key,
        EffectClip::from_keyframes([
            EffectKeyframe::new(0, PresentationValue::Scalar32(3.0)),
            EffectKeyframe::new(100, PresentationValue::Scalar32(3.0)),
        ])
        .with_composite(CompositeOperation::Add),
        0,
    );

    let started = runtime.drain_animation_events();
    assert_eq!(started.len(), 2);
    assert_eq!(started[0].id(), first);
    assert_eq!(started[1].id(), second);
    let rows = runtime.animation_rows(50);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].composite(), CompositeOperation::Add);
    assert_eq!(rows[1].composite(), CompositeOperation::Add);
    assert_eq!(runtime.active_effect_count(), 2);
    assert_eq!(runtime.sample(key, 10.0_f32, 50), 15.0);

    let canceled = runtime.cancel_animation(first, 50).unwrap();
    let canceled = presentation_control_result(canceled);
    assert_eq!(
        canceled.sample().unwrap().value(),
        PresentationValue::Scalar32(13.0)
    );

    let events = runtime.drain_animation_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id(), first);
    assert_eq!(events[0].kind(), AnimationEventKind::Canceled);
    assert_eq!(runtime.active_effect_count(), 1);
    assert_eq!(runtime.sample(key, 10.0_f32, 75), 13.0);
}

#[test]
fn additive_slot_effect_uses_default_underlying_and_completes() {
    let key = visual_translation_key();
    let mut runtime = TimelineRuntime::new();
    runtime.start_effect(
        key,
        EffectClip::from_keyframes([
            EffectKeyframe::new(0, PresentationValue::Offset2d(Vec2::new(2.0, 0.0))),
            EffectKeyframe::new(100, PresentationValue::Offset2d(Vec2::ZERO)),
        ])
        .with_composite(CompositeOperation::Add),
        0,
    );

    assert_eq!(
        runtime.active_sample_value(key, 50),
        Some(PresentationValue::Offset2d(Vec2::new(1.0, 0.0)))
    );

    let samples = presentation_samples(runtime.advance(100));
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].value(), PresentationValue::Offset2d(Vec2::ZERO));
    assert!(samples[0].is_complete());
    assert!(!runtime.has_active_track(key));
}

#[test]
fn color_effect_clip_uses_timing_direction() {
    let key = color_key();
    let mut runtime = TimelineRuntime::new();
    let black = Color::from_rgba8(0x00, 0x00, 0x00, 0xff);
    let white = Color::from_rgba8(0xff, 0xff, 0xff, 0xff);
    runtime.start_effect(
        key,
        EffectClip::from_keyframes([
            EffectKeyframe::new(0, PresentationValue::Color(black)),
            EffectKeyframe::new(100, PresentationValue::Color(white)),
        ])
        .with_timing(AnimationTiming {
            duration: 100,
            direction: PlaybackDirection::Reverse,
            fill: FillMode::Forwards,
            ..AnimationTiming::new(100)
        }),
        0,
    );

    assert_eq!(
        runtime.sample(key, black, 25),
        Color::new([0.75, 0.75, 0.75, 1.0])
    );
}

#[test]
fn effect_replaces_transition_while_active_then_transition_resumes() {
    let key = scalar_key();
    let mut runtime = TimelineRuntime::new();
    runtime.set_target(key, 0.0_f32, 0, ChangeCause::Other);
    runtime.request_transition(key, SPEC);
    runtime.set_target(key, 10.0_f32, 0, ChangeCause::Interaction);
    runtime.start_effect(
        key,
        EffectClip::from_keyframes([
            EffectKeyframe::new(0, PresentationValue::Scalar32(100.0)),
            EffectKeyframe::new(20, PresentationValue::Scalar32(200.0)),
        ]),
        0,
    );

    assert_eq!(runtime.sample(key, 10.0_f32, 10), 150.0);

    let samples = presentation_samples(runtime.advance(20));
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].key(), key);
    assert_eq!(samples[0].value(), PresentationValue::Scalar32(200.0));
    assert!(!samples[0].is_complete());

    assert!(runtime.has_active_track(key));
    assert_eq!(runtime.sample(key, 10.0_f32, 50), 5.0);
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

    runtime.remove_element(key.element(), 50);

    assert!(!runtime.has_active_track(key));
    assert!(!runtime.has_active_track(effect_key));
    assert_eq!(runtime.next_cue_deadline(0), None);
    assert_eq!(runtime.sample(key, 25.0_f32, 50), 25.0);
    assert_eq!(runtime.sample(effect_key, Vec2::ZERO, 50), Vec2::ZERO);
}

#[test]
fn transitions_do_not_create_controllable_animation_instances() {
    let key = scalar_key();
    let mut runtime = TimelineRuntime::new();
    runtime.set_target(key, 0.0_f32, 0, ChangeCause::Other);
    runtime.request_transition(key, SPEC);
    runtime.set_target(key, 10.0_f32, 0, ChangeCause::Interaction);

    assert!(runtime.drain_animation_events().is_empty());
    assert_eq!(runtime.active_transition_count(), 1);
    assert!(runtime.animation_rows(0).is_empty());

    runtime.advance(100);

    assert!(runtime.drain_animation_events().is_empty());
    assert_eq!(runtime.active_transition_count(), 0);
}

#[test]
fn repeated_effect_reports_iteration_events() {
    let key = visual_translation_key();
    let mut runtime = TimelineRuntime::new();
    let id = runtime.start_effect(
        key,
        EffectClip::from_keyframes([
            EffectKeyframe::new(0, PresentationValue::Offset2d(Vec2::ZERO)),
            EffectKeyframe::new(100, PresentationValue::Offset2d(Vec2::new(10.0, 0.0))),
        ])
        .with_timing(AnimationTiming {
            duration: 100,
            iterations: 3.0,
            fill: FillMode::Forwards,
            ..AnimationTiming::new(100)
        }),
        0,
    );
    runtime.drain_animation_events();

    runtime.advance(99);
    assert!(runtime.drain_animation_events().is_empty());

    runtime.advance(100);
    let first_iteration = runtime.drain_animation_events();
    assert_eq!(first_iteration.len(), 1);
    assert_eq!(first_iteration[0].id(), id);
    assert_eq!(first_iteration[0].kind(), AnimationEventKind::Iteration);
    assert_eq!(first_iteration[0].iteration(), Some(1));
    assert_eq!(first_iteration[0].at(), 100);

    runtime.advance(300);
    let finished = runtime.drain_animation_events();
    assert_eq!(finished.len(), 2);
    assert_eq!(finished[0].id(), id);
    assert_eq!(finished[0].kind(), AnimationEventKind::Iteration);
    assert_eq!(finished[0].iteration(), Some(2));
    assert_eq!(finished[0].at(), 300);
    assert_eq!(finished[1].id(), id);
    assert_eq!(finished[1].kind(), AnimationEventKind::Finished);
    assert_eq!(finished[1].iteration(), None);
    assert_eq!(finished[1].at(), 300);
}

#[test]
fn repeated_effect_reports_coalesced_iteration_events_before_finish() {
    let key = visual_translation_key();
    let mut runtime = TimelineRuntime::new();
    let id = runtime.start_effect(
        key,
        EffectClip::from_keyframes([
            EffectKeyframe::new(0, PresentationValue::Offset2d(Vec2::ZERO)),
            EffectKeyframe::new(100, PresentationValue::Offset2d(Vec2::new(10.0, 0.0))),
        ])
        .with_timing(AnimationTiming {
            duration: 100,
            iterations: 3.0,
            fill: FillMode::Forwards,
            ..AnimationTiming::new(100)
        }),
        0,
    );
    runtime.drain_animation_events();

    runtime.advance(300);
    let events = runtime.drain_animation_events();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].id(), id);
    assert_eq!(events[0].kind(), AnimationEventKind::Iteration);
    assert_eq!(events[0].iteration(), Some(1));
    assert_eq!(events[1].id(), id);
    assert_eq!(events[1].kind(), AnimationEventKind::Iteration);
    assert_eq!(events[1].iteration(), Some(2));
    assert_eq!(events[2].id(), id);
    assert_eq!(events[2].kind(), AnimationEventKind::Finished);
}

#[test]
fn retargeting_transition_replaces_the_active_transition() {
    let key = scalar_key();
    let mut runtime = TimelineRuntime::new();
    runtime.set_target(key, 0.0_f32, 0, ChangeCause::Other);
    runtime.request_transition(key, SPEC);
    runtime.set_target(key, 10.0_f32, 0, ChangeCause::Interaction);
    assert_eq!(runtime.sample(key, 10.0_f32, 50), 5.0);

    runtime.request_transition(key, SPEC);
    runtime.set_target(key, 2.0_f32, 50, ChangeCause::Interaction);

    assert!(runtime.drain_animation_events().is_empty());
    assert_eq!(runtime.active_transition_count(), 1);
    assert_eq!(runtime.sample(key, 2.0_f32, 50), 5.0);
    assert_eq!(runtime.sample(key, 2.0_f32, 100), 3.5);
}

#[test]
fn retargeting_under_additive_effect_does_not_double_apply_effect() {
    let key = scalar_key();
    let mut runtime = TimelineRuntime::new();
    runtime.set_target(key, 0.0_f32, 0, ChangeCause::Other);
    runtime.request_transition(key, SPEC);
    runtime.set_target(key, 10.0_f32, 0, ChangeCause::Interaction);
    runtime.start_effect(
        key,
        EffectClip::from_keyframes([
            EffectKeyframe::new(0, PresentationValue::Scalar32(5.0)),
            EffectKeyframe::new(200, PresentationValue::Scalar32(5.0)),
        ])
        .with_composite(CompositeOperation::Add),
        0,
    );
    assert_eq!(runtime.sample(key, 10.0_f32, 50), 10.0);

    runtime.request_transition(key, SPEC);
    runtime.set_target(key, 20.0_f32, 50, ChangeCause::Interaction);

    assert_eq!(
        runtime.sample(key, 20.0_f32, 50),
        10.0,
        "retargeting should preserve the final composited value at the retarget instant",
    );
    assert_eq!(runtime.sample(key, 20.0_f32, 100), 17.5);
}

#[test]
fn native_retargeting_under_additive_effect_does_not_double_apply_effect() {
    let key = native_key();
    let mut runtime = TimelineRuntime::new();
    runtime.set_native_target_value(key, Gauge(0.0), 0, ChangeCause::Other, None);
    runtime.request_transition(key, SPEC);
    runtime.set_native_target_value(key, Gauge(10.0), 0, ChangeCause::Interaction, None);
    runtime.start_native_effect_on_timeline(
        key,
        EffectClip::from_keyframes([
            EffectKeyframe::new(0, Gauge(5.0)),
            EffectKeyframe::new(200, Gauge(5.0)),
        ])
        .with_composite(CompositeOperation::Add),
        AnimationTimelineId::DOCUMENT,
        0,
        None,
    );
    assert_eq!(
        runtime
            .active_sample(key, 50)
            .and_then(TimelineTargetSample::into_native::<Gauge>),
        Some(Gauge(10.0))
    );

    runtime.request_transition(key, SPEC);
    runtime.set_native_target_value(key, Gauge(20.0), 50, ChangeCause::Interaction, None);

    assert_eq!(
        runtime
            .active_sample(key, 50)
            .and_then(TimelineTargetSample::into_native::<Gauge>),
        Some(Gauge(10.0)),
        "native retargeting should preserve the final composited value at the retarget instant",
    );
    assert_eq!(
        runtime
            .active_sample(key, 100)
            .and_then(TimelineTargetSample::into_native::<Gauge>),
        Some(Gauge(17.5))
    );
}

#[test]
fn clearing_target_cancels_the_active_transition() {
    let key = scalar_key();
    let mut runtime = TimelineRuntime::new();
    runtime.set_target(key, 0.0_f32, 0, ChangeCause::Other);
    runtime.request_transition(key, SPEC);
    runtime.set_target(key, 10.0_f32, 0, ChangeCause::Interaction);
    assert!(runtime.clear_target(key, 20));
    assert_eq!(runtime.active_transition_count(), 0);
    assert!(runtime.drain_animation_events().is_empty());
}

#[test]
fn playback_controls_pause_seek_resume_and_reverse_animation() {
    let key = scalar_key();
    let mut runtime = TimelineRuntime::new();
    runtime.set_target(key, 0.0_f32, 0, ChangeCause::Other);
    let id = runtime.start_effect(
        key,
        EffectClip::from_keyframes([
            EffectKeyframe::new(0, PresentationValue::Scalar32(0.0)),
            EffectKeyframe::new(100, PresentationValue::Scalar32(10.0)),
        ]),
        0,
    );
    runtime.drain_animation_events();

    assert_eq!(
        runtime.animation_state(id),
        Some(AnimationPlayState::Running)
    );
    assert_eq!(runtime.animation_current_time(id, 50), Some(50));
    let timing_sample = runtime
        .animation_timing_sample(id, 50)
        .expect("running transition should expose timing");
    assert_eq!(timing_sample.phase, TimingPhase::Active);
    assert_eq!(timing_sample.iteration, Some(0));
    assert_eq!(timing_sample.progress, Some(0.5));
    assert_eq!(timing_sample.eased_progress, Some(0.5));
    assert!(runtime.has_active_motion());

    let paused = runtime.pause_animation(id, 50).unwrap();
    let paused = presentation_control_result(paused);
    let paused_sample = paused.sample().unwrap();
    assert_eq!(paused_sample.value(), PresentationValue::Scalar32(5.0));
    assert!(!paused_sample.is_complete());
    assert_eq!(
        runtime.animation_state(id),
        Some(AnimationPlayState::Paused)
    );
    assert_eq!(runtime.animation_current_time(id, 100), Some(50));
    assert_eq!(
        runtime.animation_timing_sample(id, 100),
        Some(timing_sample)
    );
    assert_eq!(runtime.sample(key, 10.0_f32, 100), 5.0);
    assert!(!runtime.has_active_motion());
    assert_eq!(runtime.active_transition_count(), 0);

    let sought = runtime.seek_animation(id, 25, 100).unwrap();
    let sought = presentation_control_result(sought);
    assert_eq!(
        sought.sample().unwrap().value(),
        PresentationValue::Scalar32(2.5)
    );
    assert_eq!(runtime.animation_current_time(id, 125), Some(25));

    assert!(runtime.set_animation_playback_rate(id, 2.0, 125).is_some());
    assert_eq!(runtime.animation_playback_rate(id), Some(2.0));
    assert!(runtime.play_animation(id, 125).is_some());
    assert!(runtime.has_active_motion());
    assert_eq!(runtime.sample(key, 10.0_f32, 150), 7.5);

    assert!(runtime.reverse_animation(id, 150).is_some());
    assert_eq!(runtime.animation_playback_rate(id), Some(-2.0));
    assert_eq!(runtime.sample(key, 10.0_f32, 175), 2.5);
}

#[test]
fn cancel_animation_queues_event_and_resamples_target() {
    let key = scalar_key();
    let mut runtime = TimelineRuntime::new();
    runtime.set_target(key, 10.0_f32, 0, ChangeCause::Other);
    let id = runtime.start_effect(
        key,
        EffectClip::from_keyframes([
            EffectKeyframe::new(0, PresentationValue::Scalar32(0.0)),
            EffectKeyframe::new(100, PresentationValue::Scalar32(10.0)),
        ]),
        0,
    );
    runtime.drain_animation_events();

    let canceled = runtime.cancel_animation(id, 50).unwrap();
    let canceled = presentation_control_result(canceled);
    let canceled_sample = canceled.sample().unwrap();
    assert_eq!(canceled_sample.value(), PresentationValue::Scalar32(10.0));
    assert!(canceled_sample.is_complete());
    assert_eq!(runtime.animation_state(id), None);
    assert!(!runtime.has_active_motion());

    let events = runtime.drain_animation_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id(), id);
    assert_eq!(events[0].kind(), AnimationEventKind::Canceled);
    assert_eq!(events[0].at(), 50);
}

#[test]
fn finish_animation_queues_event_and_removes_instance() {
    let key = scalar_key();
    let mut runtime = TimelineRuntime::new();
    runtime.set_target(key, 0.0_f32, 0, ChangeCause::Other);
    let id = runtime.start_effect(
        key,
        EffectClip::from_keyframes([
            EffectKeyframe::new(0, PresentationValue::Scalar32(0.0)),
            EffectKeyframe::new(100, PresentationValue::Scalar32(10.0)),
        ]),
        0,
    );
    runtime.drain_animation_events();

    let finished = runtime.finish_animation(id, 20).unwrap();
    let finished = presentation_control_result(finished);
    let finished_sample = finished.sample().unwrap();
    assert_eq!(finished_sample.value(), PresentationValue::Scalar32(10.0));
    assert!(finished_sample.is_complete());
    assert_eq!(runtime.animation_state(id), None);
    assert!(!runtime.has_active_motion());

    let events = runtime.drain_animation_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id(), id);
    assert_eq!(events[0].kind(), AnimationEventKind::Finished);
    assert_eq!(events[0].at(), 20);
}
