// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Consequences of effective element-property changes.
//!
//! Property values can change through several sources: local writes, bindings,
//! style diffs, theme changes, and timeline animation samples. This module owns
//! the shared consequences of those changes while keeping source-specific work
//! at the source boundary.

use invalidation::ChannelSet;
use smallvec::SmallVec;
use understory_property::PropertyId;

use crate::element::ElementId;

use super::Ui;

/// Controls whether a `ControlTemplate` change refreshes generated parts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::ui) enum TemplateRefreshPolicy {
    /// Refresh expanded template parts if the changed property is `ControlTemplate`.
    RefreshRuntimeTemplate,
    /// Leave first expansion to the append/template construction lifecycle.
    DeferInitialExpansion,
    /// Do not refresh templates for this property change.
    DoNotRefreshTemplate,
}

/// Controls whether a property change projects into widget semantic state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::ui) enum StateProjectionPolicy {
    /// Project property values into semantic state such as `:checked`.
    ProjectSemanticState,
    /// Leave semantic state untouched.
    DoNotProjectState,
}

/// An element property change whose common consequences should be applied.
#[derive(Clone, Copy, Debug)]
pub(in crate::ui) struct PropertyChange {
    /// Element whose effective property changed.
    pub element: ElementId,
    /// Property that changed.
    pub property: PropertyId,
    /// Channels affected by the property before surface-aware expansion.
    pub channels: ChannelSet,
    /// Template refresh behavior for this source.
    pub template_refresh: TemplateRefreshPolicy,
    /// Semantic state projection behavior for this source.
    pub state_projection: StateProjectionPolicy,
}

impl Ui {
    /// Applies the common consequences of one effective property change.
    ///
    /// This is the single chokepoint for dirty channels, inherited-property
    /// propagation, template owner-property dependents, optional semantic state
    /// projection, and optional runtime `ControlTemplate` refresh. The caller
    /// remains responsible for source-specific work such as property storage,
    /// selector matching, timeline sampling, and binding endpoint notification.
    pub(in crate::ui) fn apply_property_change(&mut self, change: PropertyChange) -> ChannelSet {
        let channels = self.surface_aware_channels(change.property, change.channels);
        if !channels.is_empty() {
            self.mark_channels(change.element, channels);
            self.mark_inherited_property_descendants(change.element, change.property, channels);
            self.mark_template_owner_property_dependents(change.element, change.property, channels);
        }
        if change.state_projection == StateProjectionPolicy::ProjectSemanticState {
            self.sync_state_property_projection(change.element, change.property);
        }
        if change.template_refresh == TemplateRefreshPolicy::RefreshRuntimeTemplate
            && change.property == self.properties.control_template.id()
        {
            self.retemplate_if_needed(change.element);
        }
        channels
    }

    /// Marks template-generated elements that depend on an owner property.
    ///
    /// Template expansion owns the meaning of these edges. Property change
    /// consequences only apply invalidation channels to registered generated
    /// dependents for this owner/property pair.
    fn mark_template_owner_property_dependents(
        &mut self,
        owner: ElementId,
        property: PropertyId,
        channels: ChannelSet,
    ) {
        if channels.is_empty() {
            return;
        }
        // Snapshot the generated dependents before marking channels. The
        // iterator borrows `self.templates`, while `mark_channels` mutates
        // multiple `Ui` fields and records inspection changes.
        let dependents: SmallVec<[ElementId; 4]> = self
            .templates
            .owner_property_dependents(owner, property)
            .collect();
        for dependent in dependents {
            self.mark_channels(dependent, channels);
        }
    }
}
