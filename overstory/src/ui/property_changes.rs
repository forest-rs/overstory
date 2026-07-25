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
        let mut channels = self.surface_aware_channels(change.property, change.channels);
        self.apply_effective_property_channels(change.element, change.property, channels);
        channels |= self.mark_expression_property_dependents(
            change.element,
            change.property,
            change.template_refresh,
        );
        self.sync_element_lifecycle_projection(change.element, change.property);
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

    fn apply_effective_property_channels(
        &mut self,
        element: ElementId,
        property: PropertyId,
        channels: ChannelSet,
    ) {
        if channels.is_empty() {
            return;
        }
        self.mark_channels(element, channels);
        self.mark_inherited_property_descendants(element, property, channels);
        self.mark_template_owner_property_dependents(element, property, channels);
    }

    fn mark_expression_property_dependents(
        &mut self,
        element: ElementId,
        property: PropertyId,
        template_refresh: TemplateRefreshPolicy,
    ) -> ChannelSet {
        let mut affected = ChannelSet::empty();
        let mut pending = SmallVec::<[PropertyId; 4]>::new();
        let mut seen = SmallVec::<[PropertyId; 8]>::new();

        push_property_once(&mut seen, property);
        pending.push(property);

        while let Some(dependency) = pending.pop() {
            let state = self.style.match_state(element);
            let targets = self
                .style
                .expression_targets_depending_on(state, dependency);
            for target in targets {
                if !push_property_once(&mut seen, target) {
                    continue;
                }

                let channels =
                    self.surface_aware_channels(target, self.registry.affects_channels(target));
                affected |= channels;
                self.apply_effective_property_channels(element, target, channels);
                self.sync_element_lifecycle_projection(element, target);
                if template_refresh == TemplateRefreshPolicy::RefreshRuntimeTemplate
                    && target == self.properties.control_template.id()
                {
                    self.retemplate_if_needed(element);
                }
                pending.push(target);
            }
        }

        affected
    }

    fn sync_element_lifecycle_projection(&mut self, element: ElementId, property: PropertyId) {
        if property == self.properties.is_enabled.id() {
            self.reconcile_enabled_subtree(element);
            return;
        }
        if property != self.properties.visibility.id() {
            return;
        }

        let mut pending = SmallVec::<[ElementId; 16]>::from_slice(&[element]);
        while let Some(candidate) = pending.pop() {
            let Some(children) = self
                .get(candidate)
                .map(|candidate| candidate.children().to_vec())
            else {
                continue;
            };
            self.apply_focusable_preference(candidate);
            pending.extend(children);
        }

        self.dismiss_ineligible_overlay_anchors();
        if self
            .focused()
            .is_some_and(|focused| !self.is_effectively_visible(focused))
        {
            self.set_focus(None);
        }
        self.reconcile_interaction_visibility();
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

fn push_property_once(properties: &mut SmallVec<[PropertyId; 8]>, property: PropertyId) -> bool {
    match properties.binary_search(&property) {
        Ok(_) => false,
        Err(index) => {
            properties.insert(index, property);
            true
        }
    }
}
