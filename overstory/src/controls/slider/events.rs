// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Slider value-change callback payloads.

use crate::controls::support::{CallbackSlot, EventHandler};
use crate::element::ElementId;

pub(super) type SliderValueChangedHandler = EventHandler<SliderValueChangedEvent>;
pub(super) type SliderValueChangedSlot = CallbackSlot<SliderValueChangedEvent>;

/// Commit state for a slider value-change event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SliderValueChangePhase {
    /// A live preview value while a pointer drag is active.
    Changing,
    /// A committed value from click, keyboard, accessibility, or drag release.
    Committed,
}

/// Event delivered after user interaction changes a slider value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderValueChangedEvent {
    /// The slider whose value changed.
    pub element: ElementId,
    /// The clamped value before this change.
    ///
    /// For [`SliderValueChangePhase::Changing`], this is the previous live
    /// value. For [`SliderValueChangePhase::Committed`], this is the value
    /// before the committed interaction began.
    pub previous_value: f64,
    /// The clamped value after the change.
    pub value: f64,
    /// Whether this is a live preview or committed value.
    pub phase: SliderValueChangePhase,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SliderValueChange {
    pub(super) previous_value: f64,
    pub(super) value: f64,
    pub(super) phase: SliderValueChangePhase,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SliderValueDelta {
    pub(super) previous_value: f64,
    pub(super) value: f64,
}

impl SliderValueDelta {
    pub(super) fn into_change(self, phase: SliderValueChangePhase) -> SliderValueChange {
        SliderValueChange {
            previous_value: self.previous_value,
            value: self.value,
            phase,
        }
    }
}
