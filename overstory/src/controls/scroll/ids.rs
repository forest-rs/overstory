// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use understory_style::{PartTag, TypeTag};

use crate::template::TemplateSlot;
/// Selector identity for `ScrollView`.
pub const TYPE_SCROLL_VIEW: TypeTag = TypeTag(13);
/// Selector identity for `ScrollBar`.
pub const TYPE_SCROLL_BAR: TypeTag = TypeTag(14);
/// Selector identity for `ScrollArea`.
pub const TYPE_SCROLL_AREA: TypeTag = TypeTag(20);
/// Owner-local part identity for the draggable scrollbar thumb.
pub const PART_SCROLL_BAR_THUMB: PartTag = PartTag(1);
/// Semantic slot for the draggable scrollbar thumb.
pub const SLOT_SCROLL_BAR_THUMB: TemplateSlot =
    TemplateSlot::new("scrollbar-thumb").with_part_tag(PART_SCROLL_BAR_THUMB);

pub(super) const DEFAULT_SCROLL_LINE_Y: f64 = 36.0;
pub(super) const DEFAULT_SCROLL_AREA_WIDTH: f64 = 220.0;
pub(super) const DEFAULT_SCROLL_AREA_HEIGHT: f64 = 160.0;
pub(super) const DEFAULT_SCROLL_AREA_GAP: f64 = 8.0;
pub(super) const DEFAULT_SCROLL_BAR_LENGTH: f64 = 146.0;
pub(super) const DEFAULT_SCROLL_BAR_THICKNESS: f64 = 10.0;
pub(super) const DEFAULT_SCROLL_BAR_THUMB_EXTENT: f64 = 30.0;
