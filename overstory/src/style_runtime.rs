// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use hashbrown::HashMap;
use invalidation::ChannelSet;
use smallvec::SmallVec;
use understory_property::PropertyRegistry;
use understory_style::{MatchState, SelectorInputs, StyleCascade, StyleCascadeBuilder, Theme};

use crate::element::{Element, ElementId};
use crate::widget::TYPE_GENERIC;

/// Active style cascade, theme resources, and per-element match states.
pub(crate) struct StyleRuntime {
    cascade: StyleCascade,
    theme: Theme,
    match_states: HashMap<ElementId, MatchState>,
}

impl StyleRuntime {
    pub(crate) fn new() -> Self {
        Self {
            cascade: StyleCascadeBuilder::new().build(),
            theme: Theme::default(),
            match_states: HashMap::new(),
        }
    }

    pub(crate) fn cascade(&self) -> &StyleCascade {
        &self.cascade
    }

    pub(crate) fn theme(&self) -> &Theme {
        &self.theme
    }

    pub(crate) fn set_cascade(&mut self, cascade: StyleCascade) {
        self.cascade = cascade;
        self.match_states.clear();
    }

    pub(crate) fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    pub(crate) fn match_state(&self, id: ElementId) -> MatchState {
        self.match_states
            .get(&id)
            .copied()
            .unwrap_or_else(|| self.cascade.root_state())
    }

    pub(crate) fn remove(&mut self, id: ElementId) {
        self.match_states.remove(&id);
    }

    pub(crate) fn style_one(
        &mut self,
        id: ElementId,
        element: &Element,
        registry: &PropertyRegistry,
    ) -> Option<ChannelSet> {
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
            .and_then(|parent| self.match_states.get(&parent).copied())
            .unwrap_or_else(|| self.cascade.root_state());

        let inputs = SelectorInputs::with_part(type_tag, element.part_tag(), &classes, &pseudos);
        let old_state = self.match_states.get(&id).copied();
        let new_state = self.cascade.enter_subject(parent_state, &inputs);
        self.match_states.insert(id, new_state);

        if let Some(old) = old_state
            && old != new_state
        {
            let restyle = self
                .cascade
                .restyle_subject(registry, old, parent_state, &inputs);
            return Some(restyle.changed_channels());
        }
        None
    }
}
