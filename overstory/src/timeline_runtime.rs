// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Internal presentation timeline runtime.
//!
//! This module owns UI target identity, retained presentation-state
//! bookkeeping, cues, and sampled writes. It explicitly does not own pure
//! playback math, interpolation kernels, host frame scheduling, style
//! resolution, or renderer command emission.
//!
//! Vocabulary for this slice:
//!
//! - A *target value* is the resolved semantic value produced by control state,
//!   local properties, templates, and style.
//! - A *presented value* is the sampled value used for the current frame.
//! - A transition is a temporary presentation opinion over a target value; it
//!   never replaces the target itself.
//!
//! The pure timing/effect pieces live in the shared Understory animation
//! crates. This module adapts those primitives to Overstory element IDs,
//! presentation targets, the document timeline, invalidation, and UI-side cue
//! delivery.

mod cue;
mod effect;
mod instance;
mod runtime;
mod target;
mod timeline;
mod types;
mod value;

#[cfg(test)]
mod tests;

pub use cue::{TimelineCue, TimelineCueId};
pub use instance::{AnimationEvent, AnimationEventKind, AnimationId};
pub(crate) use runtime::TimelineRuntime;
pub(crate) use target::{
    NativeTimelineControlResult, NativeTimelineSample, TimelineTargetControlResult,
    TimelineTargetSample,
};
pub use timeline::AnimationTimelineId;
pub(crate) use types::{
    ChangeCause, PresentationRuntimeValue, TimelineAnimationLayerKind, TimelineControlResult,
    TimelineSample, TimelineTrackKey,
};
