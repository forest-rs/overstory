// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Diagnostics emitted while expanding templates.

use crate::element::ElementId;
use understory_style::PartTag;

use super::{TemplateItemKey, TemplateSlot};

/// Diagnostic emitted while expanding a retained template instance.
///
/// Diagnostics are non-fatal: templates may still expand enough for an app to
/// run, while inspectors and tests can point at ambiguous contracts. Duplicate
/// records are reported for the concrete lookup identity Overstory would use:
/// `slot + key` or `part + key`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TemplateDiagnostic {
    /// Two generated elements registered the same semantic slot/key.
    DuplicateSlot {
        /// Semantic slot that was registered more than once.
        slot: TemplateSlot,
        /// Optional repeated-item key. `None` means the unkeyed slot was
        /// duplicated.
        key: Option<TemplateItemKey>,
        /// First generated element that registered this identity.
        first: ElementId,
        /// Later generated element that registered the same identity.
        duplicate: ElementId,
    },
    /// Two generated elements registered the same owner-local part/key.
    DuplicatePart {
        /// Owner-local part that was registered more than once.
        part_tag: PartTag,
        /// Optional repeated-item key. `None` means the unkeyed part was
        /// duplicated.
        key: Option<TemplateItemKey>,
        /// First generated element that registered this identity.
        first: ElementId,
        /// Later generated element that registered the same identity.
        duplicate: ElementId,
    },
}

impl TemplateDiagnostic {
    /// Returns whether this diagnostic references `element`.
    #[must_use]
    pub fn references(self, element: ElementId) -> bool {
        match self {
            Self::DuplicateSlot {
                first, duplicate, ..
            }
            | Self::DuplicatePart {
                first, duplicate, ..
            } => first == element || duplicate == element,
        }
    }
}
