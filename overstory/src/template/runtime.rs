// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained template expansion metadata for controls.
//!
//! Roadmap:
//! - Keep template children as normal elements while retaining enough slot,
//!   part, and key metadata for controls to find and synchronize their parts.
//! - Add keyed incremental template updates before controls with many repeated
//!   parts need to rebuild whole subtrees.
//! - Make missing required slots and stale parts diagnosable; template failures
//!   should point at the owning control contract.
//! - Keep binding cleanup tied to template lifetime so generated parts do not
//!   leave property graph edges behind.

use crate::element::ElementId;
use crate::element_side_map::ElementSideMap;
use understory_property::PropertyId;
use understory_style::PartTag;

use super::{TemplateInstance, TemplateSlot};

/// Retained template expansion metadata for controls.
pub(crate) struct TemplateRuntime {
    /// Template instance per owner element.
    ///
    /// The instance records the realized part/slot element ids created when a
    /// resolved template expands. Template children remain normal elements.
    instances: ElementSideMap<TemplateInstance>,
}

impl TemplateRuntime {
    pub(crate) fn new() -> Self {
        Self {
            instances: ElementSideMap::new(),
        }
    }

    pub(crate) fn insert(&mut self, owner: ElementId, instance: TemplateInstance) {
        self.instances.insert(owner, instance);
    }

    pub(crate) fn remove_instance(&mut self, owner: ElementId) -> Option<TemplateInstance> {
        self.instances.remove(owner)
    }

    pub(crate) fn get(&self, owner: ElementId) -> Option<&TemplateInstance> {
        self.instances.get(owner)
    }

    pub(crate) fn part(&self, owner: ElementId, tag: PartTag) -> Option<ElementId> {
        self.get(owner).and_then(|instance| instance.part(tag))
    }

    pub(crate) fn slot(&self, owner: ElementId, slot: TemplateSlot) -> Option<ElementId> {
        self.get(owner).and_then(|instance| instance.slot(slot))
    }

    pub(crate) fn parts(
        &self,
        owner: ElementId,
        tag: PartTag,
    ) -> impl Iterator<Item = ElementId> + '_ {
        self.instances
            .get(owner)
            .into_iter()
            .flat_map(move |instance| instance.parts(tag))
    }

    pub(crate) fn slots(
        &self,
        owner: ElementId,
        slot: TemplateSlot,
    ) -> impl Iterator<Item = ElementId> + '_ {
        self.instances
            .get(owner)
            .into_iter()
            .flat_map(move |instance| instance.slots(slot))
    }

    /// Returns generated parts that depend on `property` of `owner`.
    ///
    /// Template expansion registers these edges for generated elements that
    /// derive measure, presentation, or layout from owner properties without
    /// copying those values into local template part state.
    pub(crate) fn owner_property_dependents(
        &self,
        owner: ElementId,
        property: PropertyId,
    ) -> impl Iterator<Item = ElementId> + '_ {
        self.instances
            .get(owner)
            .into_iter()
            .flat_map(move |instance| instance.owner_property_dependents(property))
    }

    pub(crate) fn remove_element(&mut self, id: ElementId) {
        self.instances.remove(id);
        // A removed element may be a realized part inside some other owner's
        // template instance, so prune it from every remaining instance too.
        for instance in self.instances.values_mut() {
            instance.remove_element(id);
        }
    }
}
