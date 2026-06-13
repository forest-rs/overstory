// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Template child insertion descriptions.

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::controls::text::ContentPresenterWidget;
use crate::properties::BuiltInProperties;
use crate::widget::{SurfacePartWidget, Widget};
use understory_property::{Property, PropertyId};
use understory_style::PartTag;

use super::{TemplateItemKey, TemplateSlot};

/// Description of one child element a template wants to insert.
pub struct TemplateChild {
    pub(super) widget: Box<dyn Widget>,
    pub(super) slot: Option<TemplateSlot>,
    pub(super) part_tag: Option<PartTag>,
    pub(super) key: Option<TemplateItemKey>,
    pub(super) owner_property_dependencies: Vec<OwnerPropertyRead>,
}

/// Owner property read declared by a generated template child.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum OwnerPropertyRead {
    /// The built-in semantic owner content property.
    Content,
    /// A concrete dependency-property id.
    Property(PropertyId),
}

impl OwnerPropertyRead {
    /// Resolves this read to a concrete property id for `properties`.
    pub(crate) fn property_id(self, properties: &BuiltInProperties) -> PropertyId {
        match self {
            Self::Content => properties.content.id(),
            Self::Property(property) => property,
        }
    }
}

impl core::fmt::Debug for TemplateChild {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TemplateChild")
            .field("slot", &self.slot)
            .field("part_tag", &self.part_tag)
            .field("key", &self.key)
            .field(
                "owner_property_dependencies",
                &self.owner_property_dependencies,
            )
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
            owner_property_dependencies: Vec::new(),
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
    /// The generated widget presents the templated owner's `Content` string
    /// when it has no authored children. If app code adds retained children to
    /// the slot, the presenter becomes a transparent host for that subtree.
    #[must_use]
    #[inline]
    pub fn content_presenter(slot: TemplateSlot) -> Self {
        Self::slot(slot, Box::new(ContentPresenterWidget)).reads_owner_content()
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

    /// Declares that this generated child reads an owner property.
    ///
    /// When `property` changes on the template owner, Overstory invalidates
    /// this generated element with the same invalidation channels. Use this
    /// for template parts that read owner state directly during measure,
    /// presentation, or layout instead of receiving a copied template binding.
    #[must_use]
    pub fn reads_owner_property<T>(mut self, property: Property<T>) -> Self
    where
        T: 'static,
    {
        self.owner_property_dependencies
            .push(OwnerPropertyRead::Property(property.id()));
        self
    }

    /// Declares that this generated child reads the owner's `Content`.
    ///
    /// Content presenters use this automatically. It remains symbolic so
    /// default template declarations do not need a concrete built-in property
    /// registry at construction time.
    #[must_use]
    pub fn reads_owner_content(mut self) -> Self {
        self.owner_property_dependencies
            .push(OwnerPropertyRead::Content);
        self
    }

    /// Adds an already-normalized owner-property read edge.
    ///
    /// `TemplateSpec` uses this while lowering data-driven nodes to
    /// `TemplateChild` values. Handwritten templates should prefer
    /// [`Self::reads_owner_property`].
    pub(crate) fn reads_owner_property_id(mut self, property: PropertyId) -> Self {
        self.owner_property_dependencies
            .push(OwnerPropertyRead::Property(property));
        self
    }
}
