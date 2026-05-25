// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Small selector helpers for Overstory's control and part style contract.
//!
//! Roadmap:
//! - Keep helpers small and readable; selectors should reveal control, state,
//!   part, and class intent without hiding the underlying style model.
//! - Add helpers for pseudo-elements, descendant depth, or stateful owner/part
//!   combinations only when themes repeat those patterns.
//! - Keep this module as convenience over `understory_style`, not a second
//!   selector language.

use understory_style::{ClassId, PartTag, PseudoClassId, Selector, SelectorStep, TypeTag};

/// Matches any subject with `type_tag`.
#[must_use]
pub fn type_tag(type_tag: TypeTag) -> Selector {
    Selector::single(SelectorStep::type_tag(type_tag))
}

/// Matches a typed owner with `pseudo`.
#[must_use]
pub fn state(type_tag: TypeTag, pseudo: PseudoClassId) -> Selector {
    Selector::single(SelectorStep::type_tag(type_tag).with_pseudo(pseudo))
}

/// Matches `owner > part`.
#[must_use]
pub fn part(owner: TypeTag, part: PartTag) -> Selector {
    Selector::child(SelectorStep::type_tag(owner), SelectorStep::part_tag(part))
}

/// Matches `owner > part.class`.
#[must_use]
pub fn part_with_class(owner: TypeTag, part: PartTag, class: ClassId) -> Selector {
    Selector::child(
        SelectorStep::type_tag(owner),
        SelectorStep::part_tag(part).with_class(class),
    )
}

/// Matches `owner_step > part`.
///
/// Use this when the owner step carries classes or pseudoclasses.
#[must_use]
pub fn part_when(owner_step: SelectorStep, part: PartTag) -> Selector {
    Selector::child(owner_step, SelectorStep::part_tag(part))
}
