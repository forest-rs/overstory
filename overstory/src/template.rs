// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Templates: factories that build a templated control's visual subtree.
//!
//! A [`Template`] is a `'static` factory returned by
//! [`crate::Widget::default_template`]. The [`crate::Ui`] expands it when a
//! widget element is inserted, and every element created during expansion is
//! tagged with a `templated_parent: Option<ElementId>` back-reference (see
//! [`crate::Element::templated_parent`]).
//!
//! Default templates can also label generated nodes with owner-local part
//! identities (see [`crate::Element::part_tag`]) so style rules can target
//! `Button > label` or `Slider > thumb` without treating parts as globally
//! unique widget types.

use crate::element::ElementId;
use crate::ui::Ui;

/// Factory that builds a templated control's visual subtree.
///
/// Implementations call `ui.append(owner, ...)`, `ui.append_widget(owner, ...)`,
/// or their part-tagging counterparts for each generated node. Any elements
/// created while `instantiate` is on the call stack are automatically tagged
/// with `templated_parent = Some(owner)`; Overstory uses that today to route
/// owner-state selectors through template descendants and to clean binding graph
/// edges when parts are removed.
///
/// Templates are stored as `&'static dyn Template`, so they should be unit
/// structs or `LazyLock<...>`-style statics rather than per-instance values.
/// Per-instance template overrides come later via a `Template` property on
/// the control.
pub trait Template: Sync {
    /// Builds this template's subtree under `owner`.
    fn instantiate(&self, ui: &mut Ui, owner: ElementId);
}
