// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Template child insertion descriptions.

use alloc::boxed::Box;

use crate::controls::text::ContentPresenterWidget;
use crate::widget::{SurfacePartWidget, Widget};
use understory_style::PartTag;

use super::{TemplateItemKey, TemplateSlot};

/// Description of one child element a template wants to insert.
pub struct TemplateChild {
    pub(super) widget: Box<dyn Widget>,
    pub(super) slot: Option<TemplateSlot>,
    pub(super) part_tag: Option<PartTag>,
    pub(super) key: Option<TemplateItemKey>,
}

impl core::fmt::Debug for TemplateChild {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TemplateChild")
            .field("slot", &self.slot)
            .field("part_tag", &self.part_tag)
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

impl TemplateChild {
    /// Creates an unnamed child with no retained slot or part record.
    #[must_use]
    #[inline]
    pub fn widget(widget: Box<dyn Widget>) -> Self {
        Self {
            widget,
            slot: None,
            part_tag: None,
            key: None,
        }
    }

    /// Creates an unnamed surface-only child.
    #[must_use]
    #[inline]
    pub fn surface() -> Self {
        Self::widget(Box::new(SurfacePartWidget))
    }

    /// Creates a child that fills a semantic template slot.
    #[must_use]
    #[inline]
    pub fn slot(slot: TemplateSlot, widget: Box<dyn Widget>) -> Self {
        Self::widget(widget).with_slot(slot)
    }

    /// Creates a child with an owner-local part tag.
    #[must_use]
    #[inline]
    pub fn part(part_tag: PartTag, widget: Box<dyn Widget>) -> Self {
        Self::widget(widget).with_part(part_tag)
    }

    /// Creates a surface-only child that fills a semantic template slot.
    #[must_use]
    #[inline]
    pub fn surface_slot(slot: TemplateSlot) -> Self {
        Self::surface().with_slot(slot)
    }

    /// Creates a content presenter child for a content-style semantic slot.
    ///
    /// The generated widget presents the templated owner's `Content` value,
    /// while style and foreground continue to resolve on the generated element.
    #[must_use]
    #[inline]
    pub fn content_presenter(slot: TemplateSlot) -> Self {
        Self::slot(slot, Box::new(ContentPresenterWidget))
    }

    /// Creates a surface-only child with an owner-local part tag.
    #[must_use]
    #[inline]
    pub fn surface_part(part_tag: PartTag) -> Self {
        Self::surface().with_part(part_tag)
    }

    /// Assigns this child to a semantic template slot.
    #[must_use]
    #[inline]
    pub fn with_slot(mut self, slot: TemplateSlot) -> Self {
        self.slot = Some(slot);
        self
    }

    /// Assigns an owner-local part tag to this child.
    #[must_use]
    #[inline]
    pub fn with_part(mut self, part_tag: PartTag) -> Self {
        self.part_tag = Some(part_tag);
        self
    }

    /// Assigns a stable repeated-item key to this child.
    #[must_use]
    #[inline]
    pub fn with_key(mut self, key: TemplateItemKey) -> Self {
        self.key = Some(key);
        self
    }
}
