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

use smallvec::SmallVec;
use understory_property::{PropertyId, PropertyRegistry};
use understory_style::{
    ExprDeps, ExpressionLayer, MatchState, SelectorInputs, StyleCascade, StyleCascadeBuilder,
    ThemeResolveOptions, ThemeVariant, VariantTheme,
};

use crate::element::{Element, ElementId};
use crate::element_side_map::ElementSideMap;
use crate::widget::TYPE_GENERIC;

/// Invalidation produced by recomputing one element's selector state.
#[derive(Clone, Debug, Default)]
pub(crate) struct StyleInvalidation {
    /// Styled properties whose effective values may have changed.
    ///
    /// `StyleRuntime` owns selector-state comparison, but it does not own
    /// Overstory's template dependency graph. Returning property ids lets `Ui`
    /// apply owner-property dependency invalidation without teaching the style
    /// runtime about templates.
    changed_properties: SmallVec<[PropertyId; 4]>,
}

impl StyleInvalidation {
    /// Returns styled properties whose effective values may have changed.
    #[must_use]
    pub(crate) fn changed_properties(&self) -> &[PropertyId] {
        &self.changed_properties
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
    /// Variant-aware theme used by style resolution.
    theme: VariantTheme,
    /// Active theme variant.
    theme_variant: Option<ThemeVariant>,
    /// Property default expressions and expression functions used by style
    /// resolution.
    expressions: ExpressionLayer,
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
            theme: VariantTheme::default(),
            theme_variant: None,
            expressions: ExpressionLayer::new(),
            match_states: ElementSideMap::new(),
        }
    }

    pub(crate) fn cascade(&self) -> &StyleCascade {
        &self.cascade
    }

    pub(crate) fn theme(&self) -> ThemeResolveOptions<'_> {
        let options = ThemeResolveOptions::from(&self.theme);
        match self.theme_variant {
            Some(variant) => options.with_variant(variant),
            None => options,
        }
    }

    pub(crate) fn expressions(&self) -> &ExpressionLayer {
        &self.expressions
    }

    pub(crate) fn set_cascade(&mut self, cascade: StyleCascade) {
        self.app_cascade = cascade;
        self.cascade = compose_cascades(&self.framework_cascade, &self.app_cascade);
        self.match_states.clear();
    }

    pub(crate) fn set_theme(&mut self, theme: impl Into<VariantTheme>) {
        self.theme = theme.into();
    }

    pub(crate) fn set_theme_variant(&mut self, variant: Option<ThemeVariant>) {
        self.theme_variant = variant;
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

    /// Returns expression targets that may depend on `property` for `state`.
    ///
    /// The set is intentionally conservative: every relevant cascade style
    /// expression and every default expression is considered, even if a
    /// higher-precedence source currently wins the target property.
    pub(crate) fn expression_targets_depending_on(
        &self,
        state: MatchState,
        property: PropertyId,
    ) -> SmallVec<[PropertyId; 4]> {
        let mut targets = SmallVec::new();
        for entry in self.expressions.defaults().expression_entries() {
            push_expression_target(&mut targets, entry.property(), entry.deps(), property);
        }
        for entry in self.cascade.expression_entries(state) {
            push_expression_target(&mut targets, entry.property(), entry.deps(), property);
        }
        targets
    }

    pub(crate) fn style_one(
        &mut self,
        id: ElementId,
        element: &Element,
        registry: &PropertyRegistry,
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
                invalidation.changed_properties.push(*property);
            }
            if invalidation.changed_properties.is_empty() {
                return None;
            }
            return Some(invalidation);
        }
        None
    }
}

fn push_expression_target(
    targets: &mut SmallVec<[PropertyId; 4]>,
    target: PropertyId,
    deps: &ExprDeps,
    property: PropertyId,
) {
    if deps.properties.binary_search(&property).is_ok() {
        push_sorted_unique(targets, target);
    }
}

fn push_sorted_unique(targets: &mut SmallVec<[PropertyId; 4]>, target: PropertyId) {
    match targets.binary_search(&target) {
        Ok(_) => {}
        Err(index) => targets.insert(index, target),
    }
}

fn compose_cascades(framework: &StyleCascade, app: &StyleCascade) -> StyleCascade {
    StyleCascadeBuilder::new()
        .push_cascade(framework)
        .push_cascade(app)
        .build()
}
