// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Templates: factories that build a templated control's visual subtree.
//!
//! Phase 1f-1 establishes the substrate:
//!
//! - [`Template`] is a `'static` factory the toolkit asks for via
//!   [`crate::ElementBehavior::default_template`].
//! - The Ui automatically expands a kind's default template when an element
//!   of that kind is inserted; the template's children are tagged with a
//!   `templated_parent: Option<ElementId>` back-reference (see
//!   [`crate::Element::templated_parent`]).
//! - Application-supplied templates land later when controls grow a
//!   per-instance `Template` property.

use crate::element::ElementId;
use crate::ui::Ui;

/// Factory that builds a templated control's visual subtree.
///
/// Implementations call `ui.insert_child(owner, kind)` (and similar) for each
/// part. Any elements created while `instantiate` is on the call stack are
/// automatically tagged with `templated_parent = Some(owner)`; that's how
/// the toolkit later distinguishes template parts from logical children for
/// part-tag selectors, template-binding teardown, and resource scoping.
///
/// Templates are stored as `&'static dyn Template`, so they should be unit
/// structs or `LazyLock<...>`-style statics rather than per-instance values.
/// Per-instance template overrides come later via a `Template` property on
/// the control.
pub trait Template: Sync {
    /// Builds this template's subtree under `owner`.
    fn instantiate(&self, ui: &mut Ui, owner: ElementId);
}
