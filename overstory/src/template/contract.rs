// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Control-declared template contracts.

use smallvec::SmallVec;

use super::TemplateSlot;

/// Importance of a slot in a control's visual template contract.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TemplateSlotImportance {
    /// The control's behavior or semantic content depends on this slot.
    Required,
    /// The control knows about this extension point and will use it when
    /// present, but absence is expected for simpler custom templates.
    Optional,
    /// The default look expects this slot, but the control can still function
    /// when a custom template omits it.
    Recommended,
}

/// One slot declaration in a control's visual template contract.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TemplateSlotContract {
    slot: TemplateSlot,
    importance: TemplateSlotImportance,
}

impl TemplateSlotContract {
    /// Creates a required slot declaration.
    #[must_use]
    #[inline]
    pub const fn required(slot: TemplateSlot) -> Self {
        Self {
            slot,
            importance: TemplateSlotImportance::Required,
        }
    }

    /// Creates an optional slot declaration.
    #[must_use]
    #[inline]
    pub const fn optional(slot: TemplateSlot) -> Self {
        Self {
            slot,
            importance: TemplateSlotImportance::Optional,
        }
    }

    /// Creates a recommended slot declaration.
    #[must_use]
    #[inline]
    pub const fn recommended(slot: TemplateSlot) -> Self {
        Self {
            slot,
            importance: TemplateSlotImportance::Recommended,
        }
    }

    /// Returns the semantic slot.
    #[must_use]
    #[inline]
    pub const fn slot(self) -> TemplateSlot {
        self.slot
    }

    /// Returns how strongly the owning control expects this slot.
    #[must_use]
    #[inline]
    pub const fn importance(self) -> TemplateSlotImportance {
        self.importance
    }
}

/// Control-declared expectations for a visual template.
///
/// The contract belongs to the control, not the template instance: the control
/// knows which semantic slots its behavior can use, while each concrete
/// template chooses which generated elements fill those slots. Inspectors use
/// this to report missing custom-template parts without hardcoding per-control
/// knowledge.
///
/// Required slots are behavior-critical. Optional slots are known extension
/// points that custom templates may omit without being considered degraded.
/// Recommended slots are not required for behavior, but omitting them usually
/// means the template loses some default visual quality or diagnostic detail.
///
/// ```rust,ignore
/// TemplateContract::new()
///     .with_required_slot(SLOT_SLIDER_THUMB)
///     .with_recommended_slot(SLOT_SLIDER_TRACK)
///     .with_optional_slot(SLOT_SLIDER_AXIS);
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TemplateContract {
    slots: SmallVec<[TemplateSlotContract; 8]>,
}

impl TemplateContract {
    /// Creates an empty contract.
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a required slot and returns the updated contract.
    #[must_use]
    #[inline]
    pub fn with_required_slot(self, slot: TemplateSlot) -> Self {
        self.with_slot(slot, TemplateSlotImportance::Required)
    }

    /// Adds an optional slot and returns the updated contract.
    #[must_use]
    #[inline]
    pub fn with_optional_slot(self, slot: TemplateSlot) -> Self {
        self.with_slot(slot, TemplateSlotImportance::Optional)
    }

    /// Adds a recommended slot and returns the updated contract.
    #[must_use]
    #[inline]
    pub fn with_recommended_slot(self, slot: TemplateSlot) -> Self {
        self.with_slot(slot, TemplateSlotImportance::Recommended)
    }

    /// Adds a slot declaration and returns the updated contract.
    #[must_use]
    #[inline]
    pub fn with_slot(mut self, slot: TemplateSlot, importance: TemplateSlotImportance) -> Self {
        self.slots.push(TemplateSlotContract { slot, importance });
        self
    }

    /// Returns all slot declarations in insertion order.
    #[must_use]
    #[inline]
    pub fn slots(&self) -> &[TemplateSlotContract] {
        &self.slots
    }

    /// Returns slots that should be present for full control behavior.
    #[inline]
    pub fn required_slots(&self) -> impl Iterator<Item = TemplateSlot> + '_ {
        self.slots_with_importance(TemplateSlotImportance::Required)
    }

    /// Returns known extension-point slots that may be omitted intentionally.
    #[inline]
    pub fn optional_slots(&self) -> impl Iterator<Item = TemplateSlot> + '_ {
        self.slots_with_importance(TemplateSlotImportance::Optional)
    }

    /// Returns slots that improve styling, diagnostics, or default visuals but
    /// are not required for core behavior.
    #[inline]
    pub fn recommended_slots(&self) -> impl Iterator<Item = TemplateSlot> + '_ {
        self.slots_with_importance(TemplateSlotImportance::Recommended)
    }

    /// Returns whether this contract declares no slots.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    fn slots_with_importance(
        &self,
        importance: TemplateSlotImportance,
    ) -> impl Iterator<Item = TemplateSlot> + '_ {
        self.slots
            .iter()
            .filter(move |slot| slot.importance == importance)
            .map(|slot| slot.slot)
    }
}
