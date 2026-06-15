// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Active style cascades, theme, and per-element selector state.
//!
//! Roadmap:
//! - Keep cascade logic in `understory_style`; Overstory should provide element
//!   inputs, match-state caching, and style-aware parent lookup.
//! - Mature the selector contract for type tags, owner-local parts, classes,
//!   pseudoclasses, and eventual pseudo-elements without special-casing
//!   individual controls.
//! - Add restyle diagnostics that explain which selector/state change dirtied
//!   which invalidation channels.
//! - Keep theming resource resolution separate from control construction so
//!   themes can swap without rebuilding the element tree.

use invalidation::ChannelSet;
use smallvec::SmallVec;
use understory_property::{PropertyId, PropertyRegistry};
use understory_style::{MatchState, SelectorInputs, StyleCascade, StyleCascadeBuilder, Theme};

use crate::element::{Element, ElementId};
use crate::element_side_map::ElementSideMap;
use crate::widget::TYPE_GENERIC;

/// Invalidation produced by recomputing one element's selector state.
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct StyleInvalidation {
    /// Channels affected on the styled element itself.
    channels: ChannelSet,
    /// Channels affected on descendants by changed inherited properties.
    inherited_channels: ChannelSet,
}

impl StyleInvalidation {
    /// Returns the channels affected on the styled element itself.
    #[must_use]
    pub(crate) fn channels(self) -> ChannelSet {
        self.channels
    }

    /// Returns the channels affected on descendants by inheritance.
    #[must_use]
    pub(crate) fn inherited_channels(self) -> ChannelSet {
        self.inherited_channels
    }
}

/// Active style cascade, theme resources, and per-element match states.
pub(crate) struct StyleRuntime {
    /// Framework-supplied defaults for built-in controls.
    ///
    /// This layer stays installed when app code replaces the app cascade so
    /// controls keep their default templates, part styles, and baseline
    /// property values.
    framework_cascade: StyleCascade,
    /// App-authored stylesheet layered after framework defaults.
    app_cascade: StyleCascade,
    /// Combined selector cascade used to resolve styled property values.
    cascade: StyleCascade,
    /// Theme resource table used by style resolution.
    theme: Theme,
    /// Cached selector automaton state for each retained element.
    ///
    /// The state is derived from the parent state and this element's current
    /// type, part, classes, and pseudo-classes. It is cleared when the cascade
    /// is replaced because state ids are cascade-local.
    match_states: ElementSideMap<MatchState>,
}

impl StyleRuntime {
    pub(crate) fn new(framework_cascade: StyleCascade) -> Self {
        let app_cascade = StyleCascadeBuilder::new().build();
        let cascade = compose_cascades(&framework_cascade, &app_cascade);
        Self {
            framework_cascade,
            app_cascade,
            cascade,
            theme: Theme::default(),
            match_states: ElementSideMap::new(),
        }
    }

    pub(crate) fn cascade(&self) -> &StyleCascade {
        &self.cascade
    }

    pub(crate) fn theme(&self) -> &Theme {
        &self.theme
    }

    pub(crate) fn set_cascade(&mut self, cascade: StyleCascade) {
        self.app_cascade = cascade;
        self.cascade = compose_cascades(&self.framework_cascade, &self.app_cascade);
        self.match_states.clear();
    }

    pub(crate) fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    pub(crate) fn match_state(&self, id: ElementId) -> MatchState {
        self.match_states
            .get(id)
            .copied()
            .unwrap_or_else(|| self.cascade.root_state())
    }

    pub(crate) fn remove(&mut self, id: ElementId) {
        self.match_states.remove(id);
    }

    pub(crate) fn style_one(
        &mut self,
        id: ElementId,
        element: &Element,
        registry: &PropertyRegistry,
        surface_aware_channels: impl Fn(PropertyId, ChannelSet) -> ChannelSet,
    ) -> Option<StyleInvalidation> {
        // Anonymous containers fall back to `TYPE_GENERIC` for selector
        // matching. Widgets may opt out of a global type and rely only on an
        // owner-local PartTag.
        let type_tag = if element.has_widget_identity() {
            element.type_tag()
        } else {
            Some(TYPE_GENERIC)
        };
        let classes: SmallVec<[understory_style::ClassId; 4]> =
            SmallVec::from_slice(element.classes());
        let pseudos: SmallVec<[understory_style::PseudoClassId; 4]> =
            SmallVec::from_slice(element.pseudos());

        let parent_state = element
            .parent()
            .and_then(|parent| self.match_states.get(parent).copied())
            .unwrap_or_else(|| self.cascade.root_state());

        let inputs = SelectorInputs::with_part(type_tag, element.part_tag(), &classes, &pseudos);
        let old_state = self.match_states.get(id).copied();
        let new_state = self.cascade.enter_subject(parent_state, &inputs);
        self.match_states.insert(id, new_state);

        if let Some(old) = old_state
            && old != new_state
        {
            let restyle = self
                .cascade
                .restyle_subject(registry, old, parent_state, &inputs);
            let mut invalidation = StyleInvalidation::default();
            for property in restyle.changed_properties().property_ids() {
                let channels =
                    surface_aware_channels(*property, registry.affects_channels(*property));
                invalidation.channels |= channels;
                if registry.inherits(*property) {
                    invalidation.inherited_channels |= channels;
                }
            }
            if invalidation.channels.is_empty() && invalidation.inherited_channels.is_empty() {
                return None;
            }
            return Some(invalidation);
        }
        None
    }
}

fn compose_cascades(framework: &StyleCascade, app: &StyleCascade) -> StyleCascade {
    StyleCascadeBuilder::new()
        .push_cascade(framework)
        .push_cascade(app)
        .build()
}
