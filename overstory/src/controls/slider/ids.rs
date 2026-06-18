// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Selector, slot, and fixed geometry identities for `Slider`.

use understory_style::{ClassId, PartTag, TypeTag};

use crate::template::TemplateSlot;

/// Selector identity for `Slider`.
pub const TYPE_SLIDER: TypeTag = TypeTag(6);
/// Owner-local part identity for the slider track.
pub const PART_SLIDER_TRACK: PartTag = PartTag(1);
/// Owner-local part identity for the slider thumb.
pub const PART_SLIDER_THUMB: PartTag = PartTag(2);
/// Owner-local part identity for the slider axis surface.
pub const PART_SLIDER_AXIS: PartTag = PartTag(3);
/// Owner-local repeated part identity for slider tick marks.
pub const PART_SLIDER_TICK: PartTag = PartTag(4);
/// Owner-local repeated part identity for slider tick labels.
pub const PART_SLIDER_TICK_LABEL: PartTag = PartTag(5);

/// Semantic slot for the slider track.
pub const SLOT_SLIDER_TRACK: TemplateSlot =
    TemplateSlot::new("slider-track").with_part_tag(PART_SLIDER_TRACK);
/// Semantic slot for the slider thumb.
pub const SLOT_SLIDER_THUMB: TemplateSlot =
    TemplateSlot::new("slider-thumb").with_part_tag(PART_SLIDER_THUMB);
/// Semantic slot for the slider axis surface.
pub const SLOT_SLIDER_AXIS: TemplateSlot =
    TemplateSlot::new("slider-axis").with_part_tag(PART_SLIDER_AXIS);
/// Semantic repeated slot for slider tick marks.
pub const SLOT_SLIDER_TICK: TemplateSlot =
    TemplateSlot::new("slider-tick").with_part_tag(PART_SLIDER_TICK);
/// Semantic repeated slot for slider tick labels.
pub const SLOT_SLIDER_TICK_LABEL: TemplateSlot =
    TemplateSlot::new("slider-tick-label").with_part_tag(PART_SLIDER_TICK_LABEL);

/// Class applied to major slider tick marks and labels.
pub const CLASS_SLIDER_MAJOR_TICK: ClassId = ClassId(1);
/// Class applied to minor slider tick marks and labels.
pub const CLASS_SLIDER_MINOR_TICK: ClassId = ClassId(2);

pub(super) const SLIDER_WIDTH: f32 = 220.0;
pub(super) const SLIDER_HEIGHT: f32 = 52.0;
pub(super) const TRACK_HEIGHT: f32 = 6.0;
pub(super) const THUMB_DIAMETER: f32 = 22.0;
pub(super) const TICK_WIDTH: f32 = 2.0;
pub(super) const MAJOR_TICK_HEIGHT: f32 = 10.0;
pub(super) const MINOR_TICK_HEIGHT: f32 = 6.0;
pub(super) const TICK_TOP: f32 = 28.0;
pub(super) const LABEL_TOP: f32 = 38.0;
pub(super) const LABEL_CHAR_WIDTH: f32 = 6.0;
pub(super) const LABEL_MIN_WIDTH: f32 = 6.0;
pub(super) const DEFAULT_SMALL_CHANGE_FRACTION: f64 = 0.05;
pub(super) const DEFAULT_LARGE_CHANGE_FRACTION: f64 = 0.20;
