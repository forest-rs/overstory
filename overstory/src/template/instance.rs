// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained result of one template expansion.

use alloc::vec::Vec;

use crate::element::ElementId;
use understory_property::PropertyId;
use understory_property_binding::BindingId;
use understory_style::PartTag;

use super::{TemplateDiagnostic, TemplateId, TemplateItemKey, TemplatePart, TemplateSlot};

/// Retained result of expanding one element's resolved template.
///
/// The instance records the generated part elements and binding ids created
/// while the template expanded. It is intentionally owner-local: callers ask
/// for `owner + PartTag`, not for global widget-type identities on parts.
#[derive(Debug)]
pub struct TemplateInstance {
    owner: ElementId,
    template_id: TemplateId,
    parts: Vec<TemplatePart>,
    owner_property_dependents: Vec<TemplateOwnerPropertyDependent>,
    bindings: Vec<BindingId>,
    diagnostics: Vec<TemplateDiagnostic>,
}

/// One generated element that depends on a property of the template owner.
///
/// The owner is implicit in the surrounding [`TemplateInstance`]. This records
/// only the property being observed and the generated element that must be
/// invalidated when that owner property changes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct TemplateOwnerPropertyDependent {
    /// Owner property read by the generated element.
    property: PropertyId,
    /// Generated element whose measure/presentation/layout depends on it.
    element: ElementId,
}

impl TemplateInstance {
    pub(super) fn new(owner: ElementId, template_id: TemplateId) -> Self {
        Self {
            owner,
            template_id,
            parts: Vec::new(),
            owner_property_dependents: Vec::new(),
            bindings: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Returns the element whose template produced this instance.
    #[must_use]
    #[inline]
    pub fn owner(&self) -> ElementId {
        self.owner
    }

    /// Returns the identity of the template value that produced this instance.
    #[must_use]
    #[inline]
    pub fn template_id(&self) -> TemplateId {
        self.template_id
    }

    /// Returns all registered parts in template creation order.
    #[must_use]
    #[inline]
    pub fn part_records(&self) -> &[TemplatePart] {
        &self.parts
    }

    /// Returns the first element registered for `tag`, if any.
    ///
    /// Use [`Self::parts`] when the part tag is repeated.
    #[must_use]
    pub fn part(&self, tag: PartTag) -> Option<ElementId> {
        self.parts
            .iter()
            .find(|part| part.part_tag == Some(tag))
            .map(|part| part.element)
    }

    /// Returns the element registered for `tag` and `key`, if any.
    #[must_use]
    pub fn keyed_part(&self, tag: PartTag, key: TemplateItemKey) -> Option<ElementId> {
        self.parts
            .iter()
            .find(|part| part.part_tag == Some(tag) && part.key == Some(key))
            .map(|part| part.element)
    }

    /// Returns the first element registered for `slot`, if any.
    ///
    /// Use [`Self::slots`] when the slot is repeated.
    #[must_use]
    pub fn slot(&self, slot: TemplateSlot) -> Option<ElementId> {
        self.parts
            .iter()
            .find(|part| part.slot == Some(slot))
            .map(|part| part.element)
    }

    /// Returns the element registered for `slot` and `key`, if any.
    #[must_use]
    pub fn keyed_slot(&self, slot: TemplateSlot, key: TemplateItemKey) -> Option<ElementId> {
        self.parts
            .iter()
            .find(|part| part.slot == Some(slot) && part.key == Some(key))
            .map(|part| part.element)
    }

    /// Returns all elements registered for `tag`, in template creation order.
    pub fn parts(&self, tag: PartTag) -> impl Iterator<Item = ElementId> + '_ {
        self.parts
            .iter()
            .filter(move |part| part.part_tag == Some(tag))
            .map(|part| part.element)
    }

    /// Returns all elements registered for `slot`, in template creation order.
    pub fn slots(&self, slot: TemplateSlot) -> impl Iterator<Item = ElementId> + '_ {
        self.parts
            .iter()
            .filter(move |part| part.slot == Some(slot))
            .map(|part| part.element)
    }

    /// Returns generated elements that depend on the owner's `property`.
    ///
    /// The owner is the instance owner. These edges are declared by the
    /// template during expansion and let the property write path invalidate
    /// derived template parts without knowing why the part depends on the
    /// owner property.
    pub(crate) fn owner_property_dependents(
        &self,
        property: PropertyId,
    ) -> impl Iterator<Item = ElementId> + '_ {
        self.owner_property_dependents
            .iter()
            .filter(move |dependency| dependency.property == property)
            .map(|dependency| dependency.element)
    }

    /// Returns binding ids created while expanding the template.
    #[must_use]
    #[inline]
    pub fn binding_ids(&self) -> &[BindingId] {
        &self.bindings
    }

    /// Returns non-fatal diagnostics produced during template expansion.
    #[must_use]
    #[inline]
    pub fn diagnostics(&self) -> &[TemplateDiagnostic] {
        &self.diagnostics
    }

    pub(crate) fn remove_element(&mut self, element: ElementId) {
        self.parts.retain(|part| part.element != element);
        self.owner_property_dependents
            .retain(|dependency| dependency.element != element);
        self.diagnostics
            .retain(|diagnostic| !diagnostic.references(element));
    }

    pub(super) fn push_part(
        &mut self,
        slot: Option<TemplateSlot>,
        part_tag: Option<PartTag>,
        key: Option<TemplateItemKey>,
        element: ElementId,
    ) {
        if let Some(slot) = slot
            && let Some(first) = self
                .parts
                .iter()
                .find(|part| part.slot == Some(slot) && part.key == key)
                .map(|part| part.element)
        {
            self.diagnostics.push(TemplateDiagnostic::DuplicateSlot {
                slot,
                key,
                first,
                duplicate: element,
            });
        }
        if let Some(part_tag) = part_tag
            && let Some(first) = self
                .parts
                .iter()
                .find(|part| part.part_tag == Some(part_tag) && part.key == key)
                .map(|part| part.element)
        {
            self.diagnostics.push(TemplateDiagnostic::DuplicatePart {
                part_tag,
                key,
                first,
                duplicate: element,
            });
        }
        self.parts.push(TemplatePart {
            slot,
            part_tag,
            key,
            element,
        });
    }

    /// Records that `element` depends on `property` of this instance's owner.
    pub(super) fn push_owner_property_dependent(
        &mut self,
        property: PropertyId,
        element: ElementId,
    ) {
        self.owner_property_dependents
            .push(TemplateOwnerPropertyDependent { property, element });
    }

    pub(super) fn push_binding(&mut self, id: BindingId) {
        self.bindings.push(id);
    }
}
