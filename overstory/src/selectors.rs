// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Selector helpers for Overstory's control and part style contract.
//!
//! Roadmap:
//! - Keep this module as Overstory-facing naming over
//!   [`understory_style::selectors`], not a second selector language.
//! - Add Overstory-specific selector helpers here only when they depend on
//!   control vocabulary from this crate.

use understory_style::{ClassId, PartTag, Selector, SelectorStep, TypeTag};

pub use understory_style::selectors::{type_state as state, type_tag};

/// Owner-local style identity for one generated template part.
///
/// A `PartTag` is only meaningful under the templated control that owns it.
/// Carrying the owner [`TypeTag`] alongside the part makes selector call sites
/// express that scoping directly instead of passing two loose identifiers.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct TemplatePartSelector {
    owner_type_tag: TypeTag,
    part_tag: PartTag,
}

impl TemplatePartSelector {
    /// Creates an owner-local template part selector identity.
    #[must_use]
    #[inline]
    pub const fn new(owner_type_tag: TypeTag, part_tag: PartTag) -> Self {
        Self {
            owner_type_tag,
            part_tag,
        }
    }

    /// Returns the global selector identity of the owning control type.
    #[must_use]
    #[inline]
    pub const fn owner_type_tag(self) -> TypeTag {
        self.owner_type_tag
    }

    /// Returns the owner-local template part tag.
    #[must_use]
    #[inline]
    pub const fn part_tag(self) -> PartTag {
        self.part_tag
    }
}

/// Creates an owner-local template part selector identity.
#[must_use]
#[inline]
pub const fn template_part(owner_type_tag: TypeTag, part_tag: PartTag) -> TemplatePartSelector {
    TemplatePartSelector::new(owner_type_tag, part_tag)
}

/// Matches an immediate owner-local part: `owner > part`.
#[must_use]
#[inline]
pub fn part(part: TemplatePartSelector) -> Selector {
    understory_style::selectors::part(part.owner_type_tag(), part.part_tag())
}

/// Matches an immediate owner-local classified part: `owner > part.class`.
#[must_use]
#[inline]
pub fn part_with_class(part: TemplatePartSelector, class: ClassId) -> Selector {
    understory_style::selectors::part_with_class(part.owner_type_tag(), part.part_tag(), class)
}

/// Matches an immediate part below a pre-built owner step: `owner_step > part`.
///
/// Use this when the owner step carries additional class or pseudoclass
/// requirements. The `part` value still carries the canonical owner type so the
/// call site keeps owner-local part scoping visible.
#[must_use]
#[inline]
pub fn part_when(owner_step: SelectorStep, part: TemplatePartSelector) -> Selector {
    debug_assert!(
        owner_step.type_tag.is_none() || owner_step.type_tag == Some(part.owner_type_tag()),
        "owner step type tag should match owner-local part selector",
    );
    understory_style::selectors::part_when(owner_step, part.part_tag())
}
