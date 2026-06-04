// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Contract validation for expanded template instances.

use alloc::vec::Vec;

use crate::element::ElementId;

use super::{TemplateDiagnostic, TemplateInstance, TemplateSlot};

/// Status of one declared template slot after expansion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TemplateSlotStatus {
    slot: TemplateSlot,
    element: Option<ElementId>,
}

impl TemplateSlotStatus {
    /// Creates a status row for `slot`.
    #[must_use]
    #[inline]
    pub const fn new(slot: TemplateSlot, element: Option<ElementId>) -> Self {
        Self { slot, element }
    }

    /// Returns the declared slot.
    #[must_use]
    #[inline]
    pub const fn slot(&self) -> TemplateSlot {
        self.slot
    }

    /// Returns the element that filled this slot, if the expanded template
    /// provided one.
    #[must_use]
    #[inline]
    pub const fn element(&self) -> Option<ElementId> {
        self.element
    }

    /// Returns whether the slot has a realized element.
    #[must_use]
    #[inline]
    pub const fn is_present(&self) -> bool {
        self.element.is_some()
    }
}

/// Contract and diagnostic report for one expanded template instance.
///
/// This report is the shared interpretation of a control's
/// [`super::TemplateContract`] against its retained [`TemplateInstance`].
/// Runtime, inspection, and devtools should consume this shape instead of each
/// re-walking required, optional, recommended, and diagnostic data.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TemplateExpansionReport {
    required_slots: Vec<TemplateSlotStatus>,
    optional_slots: Vec<TemplateSlotStatus>,
    recommended_slots: Vec<TemplateSlotStatus>,
    diagnostics: Vec<TemplateDiagnostic>,
}

impl TemplateExpansionReport {
    /// Creates an expansion report from already-classified slot statuses.
    #[must_use]
    pub fn new(
        required_slots: Vec<TemplateSlotStatus>,
        optional_slots: Vec<TemplateSlotStatus>,
        recommended_slots: Vec<TemplateSlotStatus>,
        diagnostics: Vec<TemplateDiagnostic>,
    ) -> Self {
        Self {
            required_slots,
            optional_slots,
            recommended_slots,
            diagnostics,
        }
    }

    /// Returns required slot statuses in contract declaration order.
    #[must_use]
    #[inline]
    pub fn required_slots(&self) -> &[TemplateSlotStatus] {
        &self.required_slots
    }

    /// Returns optional slot statuses in contract declaration order.
    #[must_use]
    #[inline]
    pub fn optional_slots(&self) -> &[TemplateSlotStatus] {
        &self.optional_slots
    }

    /// Returns recommended slot statuses in contract declaration order.
    #[must_use]
    #[inline]
    pub fn recommended_slots(&self) -> &[TemplateSlotStatus] {
        &self.recommended_slots
    }

    /// Returns diagnostics emitted while the template expanded.
    #[must_use]
    #[inline]
    pub fn diagnostics(&self) -> &[TemplateDiagnostic] {
        &self.diagnostics
    }

    /// Returns whether any required slot is missing.
    #[must_use]
    pub fn has_missing_required_slots(&self) -> bool {
        self.required_slots.iter().any(|slot| !slot.is_present())
    }

    /// Returns whether any recommended slot is missing.
    ///
    /// Missing recommended slots do not make the contract invalid, but they are
    /// useful quality/degradation hints for custom templates.
    #[must_use]
    pub fn has_missing_recommended_slots(&self) -> bool {
        self.recommended_slots.iter().any(|slot| !slot.is_present())
    }

    /// Returns whether this report has missing required slots or expansion
    /// diagnostics.
    #[must_use]
    pub fn has_issues(&self) -> bool {
        self.has_missing_required_slots() || !self.diagnostics.is_empty()
    }
}

pub(super) fn status_for_slot(
    instance: Option<&TemplateInstance>,
    slot: TemplateSlot,
) -> TemplateSlotStatus {
    TemplateSlotStatus::new(slot, instance.and_then(|instance| instance.slot(slot)))
}
