// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Style cascade, theme, match-state, and effective-value resolution.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use understory_property::Property;
use understory_style::{
    MatchState, ResolveCx, Resolved, ResolvedSource, ResourceKey, StyleCascade, StyleVocabulary,
    ThemeResolveOptions, ThemeVariant, VariantTheme,
};

use crate::channels::{LAYOUT, STYLE};
use crate::element::{Element, ElementId};
use crate::lookup::UiResolveLookup;

use super::property_changes::{PropertyChange, StateProjectionPolicy, TemplateRefreshPolicy};
use super::{Ui, collect_preorder};

impl Ui {
    /// Returns the active combined style cascade.
    #[must_use]
    #[inline]
    pub fn cascade(&self) -> &StyleCascade {
        self.style.cascade()
    }

    /// Returns the active theme lookup, including variant and scope fallback.
    #[must_use]
    #[inline]
    pub fn theme(&self) -> ThemeResolveOptions<'_> {
        self.style.theme()
    }

    /// Swaps in a new app style cascade and restyles the entire tree.
    ///
    /// The supplied cascade is layered after Overstory's built-in framework
    /// cascade. App styles can override default control templates and part
    /// styles, but replacing the app cascade does not remove built-in control
    /// behavior.
    ///
    /// Every element is marked LAYOUT-dirty since styled values may
    /// now resolve differently; the channel cascade
    /// (LAYOUT → ARRANGE → GEOMETRY → PAINT → COMPOSE) handles the rest.
    /// Match states from the previous cascade are discarded — the [`MatchState`]
    /// type is bound to its originating cascade, so they can't be
    /// reused after a swap. The LAYOUT mark catches cascades that
    /// contribute a `Layout` for an element whose previous state
    /// didn't have one — `style_one`'s diff path only triggers on
    /// state *transitions*, not first-time styling.
    pub fn set_cascade(&mut self, cascade: StyleCascade) {
        self.style.set_cascade(cascade);
        let ids = collect_preorder(self, self.root());
        for id in ids {
            self.style_one(id);
            self.invalidation.mark(id, LAYOUT);
            self.record_element_tree_update(id);
        }
    }

    /// Swaps in a new theme. All elements are marked LAYOUT-dirty since theme
    /// resources can feed layout, text measurement, and paint values.
    pub fn set_theme(&mut self, theme: impl Into<VariantTheme>) {
        self.style.set_theme(theme);
        self.invalidate_theme_dependents();
    }

    /// Selects a theme variant without rebuilding resource tables.
    pub fn set_theme_variant(&mut self, variant: Option<ThemeVariant>) {
        self.style.set_theme_variant(variant);
        self.invalidate_theme_dependents();
    }

    fn invalidate_theme_dependents(&mut self) {
        let ids = collect_preorder(self, self.root());
        for id in ids {
            self.invalidation.mark(id, LAYOUT);
            self.retemplate_if_needed(id);
            self.record_element_tree_update(id);
        }
    }

    /// Returns the match state for `id`, falling back to the cascade root
    /// state if the element has not yet been styled.
    #[must_use]
    pub fn match_state(&self, id: ElementId) -> MatchState {
        self.style.match_state(id)
    }

    /// Resolves `property` for `element` through the active property stores,
    /// style cascade, theme resources, and inherited ancestor values.
    pub(crate) fn resolve_value_for<T: Clone + 'static>(
        &self,
        element: &Element,
        property: Property<T>,
    ) -> T {
        ResolveCx::new(
            &self.registry,
            self.style.theme(),
            self.style.expressions(),
            UiResolveLookup::new(self),
        )
        .get_value(
            element,
            property,
            Some((self.style.cascade(), self.match_state(element.id()))),
        )
    }

    /// Resolves `property` for `element` and reports which source supplied the
    /// value. Element-based sibling of [`Self::explain_property`].
    pub(crate) fn explain_value_for<T: Clone + 'static>(
        &self,
        element: &Element,
        property: Property<T>,
    ) -> Resolved<T> {
        ResolveCx::new(
            &self.registry,
            self.style.theme(),
            self.style.expressions(),
            UiResolveLookup::new(self),
        )
        .explain_value(
            element,
            property,
            Some((self.style.cascade(), self.match_state(element.id()))),
        )
    }

    /// Reads the effective resolved value of `property` on `id`.
    ///
    /// Resolves through the full precedence chain — animation, local layers,
    /// style cascade, theme resources, inheritance, and the registry default —
    /// the same path styling and layout use. Returns `None` if `id` does not
    /// exist.
    #[must_use]
    pub fn resolved_value<T: Clone + 'static>(
        &self,
        id: ElementId,
        property: Property<T>,
    ) -> Option<T> {
        let element = self.get(id)?;
        Some(self.resolve_value_for(element, property))
    }

    /// Resolves `property` on `id` and explains which precedence stage and
    /// source supplied the value.
    ///
    /// This is an owned, cold-path inspection API for devtools and tooling; it
    /// does not affect the borrowed hot path. The returned
    /// [`Resolved::source`](understory_style::Resolved) carries ids and
    /// selector structures — mapping those to author-facing names is the
    /// caller's job through [`Ui::style_vocabulary`].
    #[must_use]
    pub fn explain_property<T: Clone + 'static>(
        &self,
        id: ElementId,
        property: Property<T>,
    ) -> Option<Resolved<T>> {
        let element = self.get(id)?;
        Some(
            ResolveCx::new(
                &self.registry,
                self.style.theme(),
                self.style.expressions(),
                UiResolveLookup::new(self),
            )
            .explain_value(
                element,
                property,
                Some((self.style.cascade(), self.match_state(element.id()))),
            ),
        )
    }

    /// Resolves `property` on `id` and returns a short human-readable
    /// description of the source that supplied the value, mapping resource
    /// keys to their registered names. Returns `None` if `id` does not exist.
    #[must_use]
    pub fn describe_property_source<T: Clone + 'static>(
        &self,
        id: ElementId,
        property: Property<T>,
    ) -> Option<String> {
        let resolved = self.explain_property(id, property)?;
        Some(describe_resolved_source(
            &resolved.source,
            self.style_vocabulary(),
        ))
    }

    /// Drains the STYLE channel, recomputing the match state of any dirty
    /// element subtree. Channels affected by changed styled properties are
    /// unioned into the invalidation tracker.
    pub fn flush_style(&mut self) {
        let dirty: Vec<ElementId> = self.invalidation.drain_sorted(STYLE).collect();
        self.frame_stats.style_roots += dirty.len();
        for id in dirty {
            self.restyle_subtree(id);
        }
    }

    /// Restyles `id` and its descendants in preorder. Channels affected by
    /// each element's styled-property changes are marked in the tracker.
    pub(in crate::ui) fn restyle_subtree(&mut self, id: ElementId) {
        let order: Vec<ElementId> = collect_preorder(self, id);
        self.frame_stats.styled_elements += order.len();
        for id in order {
            self.style_one(id);
        }
    }

    /// Recomputes the match state for one element. If a previous state exists
    /// for `id`, the cascade's `restyle_subject` is used to surface changed
    /// channels into the tracker; otherwise the new state is just stored.
    ///
    /// Elements are entered from their parent match state. The style matcher
    /// starts selectors at every subject, so single-step rules still apply to
    /// nested controls while child/descendant selectors can carry owner state
    /// into template parts.
    pub(in crate::ui) fn style_one(&mut self, id: ElementId) {
        self.style_one_inner(id, true);
    }

    /// Computes initial match state for a newly inserted element without
    /// materializing templates.
    ///
    /// Append paths expand templates after construction-time properties have
    /// been applied. That ordering matters for deferred controls whose default
    /// templates read owner properties during expansion.
    pub(in crate::ui) fn style_one_for_insert(&mut self, id: ElementId) {
        self.style_one_inner(id, false);
    }

    fn style_one_inner(&mut self, id: ElementId, allow_retemplate: bool) {
        let Some(element) = self.elements.get(id) else {
            self.style.remove(id);
            return;
        };
        let invalidation = self.style.style_one(id, element, &self.registry);
        let mut applied_property_change = false;
        if let Some(invalidation) = invalidation {
            for property in invalidation.changed_properties() {
                applied_property_change = true;
                self.apply_property_change(PropertyChange {
                    element: id,
                    property: *property,
                    channels: self.registry.affects_channels(*property),
                    template_refresh: if allow_retemplate {
                        TemplateRefreshPolicy::RefreshRuntimeTemplate
                    } else {
                        TemplateRefreshPolicy::DeferInitialExpansion
                    },
                    state_projection: StateProjectionPolicy::DoNotProjectState,
                });
            }
        }
        if allow_retemplate && !applied_property_change {
            self.retemplate_if_needed(id);
        }
    }
}

/// Formats a [`ResolvedSource`] into a short, name-mapped description for
/// inspectors. Resource keys are mapped to their registered names; selector
/// structure is summarized by stage and origin rather than fully printed.
fn describe_resolved_source(source: &ResolvedSource, vocabulary: &StyleVocabulary) -> String {
    fn resource(vocabulary: &StyleVocabulary, key: ResourceKey) -> String {
        vocabulary
            .resource_name(key)
            .map_or_else(|| String::from("resource"), String::from)
    }
    match source {
        ResolvedSource::Animation => String::from("Animation"),
        ResolvedSource::LocalOverride { source } => format!("Local override ({source:?})"),
        ResolvedSource::CascadeRule {
            origin,
            resource: res,
            ..
        } => match res {
            Some(key) => format!("{origin:?} rule \u{2192} {}", resource(vocabulary, *key)),
            None => format!("{origin:?} rule"),
        },
        ResolvedSource::CascadeDirect {
            origin,
            resource: res,
            ..
        } => match res {
            Some(key) => format!("{origin:?} style \u{2192} {}", resource(vocabulary, *key)),
            None => format!("{origin:?} style"),
        },
        ResolvedSource::ThemeResource { key } => {
            format!("Theme resource {}", resource(vocabulary, *key))
        }
        ResolvedSource::Inherited { ancestor_depth, .. } => {
            format!("Inherited (depth {ancestor_depth})")
        }
        ResolvedSource::Default | ResolvedSource::DefaultExpression { .. } => {
            String::from("Framework default")
        }
    }
}

#[cfg(test)]
mod provenance_tests {
    use crate::{Button, ResolvedSource, Ui};

    #[test]
    fn resolved_value_and_explain_property_report_source() {
        let mut ui = Ui::new();
        let root = ui.root();
        let button = ui.append(root, Button::new("Hi"));
        let font_size = ui.properties().font_size;

        // Unset locally: resolves from the cascade/default, not a local override.
        let explained = ui
            .explain_property(button, font_size)
            .expect("button should exist");
        assert!(!matches!(
            explained.source,
            ResolvedSource::LocalOverride { .. }
        ));
        assert_eq!(ui.resolved_value(button, font_size), Some(explained.value));

        // Set locally: the value reflects it and provenance reports the override.
        ui.set_local(button, font_size, 22.0);
        assert_eq!(ui.resolved_value(button, font_size), Some(22.0));
        let explained = ui
            .explain_property(button, font_size)
            .expect("button should exist");
        assert_eq!(explained.value, 22.0);
        assert!(matches!(
            explained.source,
            ResolvedSource::LocalOverride { .. }
        ));

        let described = ui
            .describe_property_source(button, font_size)
            .expect("button should exist");
        assert!(
            described.contains("Local override"),
            "described source was {described:?}",
        );
    }

    #[test]
    fn inspection_snapshot_reports_property_origin() {
        use crate::inspection::ElementPropertyInspectionOrigin;

        let mut ui = Ui::new();
        let root = ui.root();
        let button = ui.append(root, Button::new("Hi"));
        let font_size = ui.properties().font_size;
        ui.set_local(button, font_size, 22.0);

        let snapshot = ui.inspection_snapshot();
        let row = snapshot
            .elements
            .iter()
            .find(|row| row.id == button)
            .expect("button should appear in the snapshot");
        let origins: alloc::vec::Vec<_> = row.properties.iter().map(|p| p.origin).collect();
        assert!(
            origins.contains(&ElementPropertyInspectionOrigin::Local),
            "locally set font size should report a Local origin, got {origins:?}",
        );
    }
}
