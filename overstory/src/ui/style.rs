// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Style cascade, theme, match-state, and effective-value resolution.

use alloc::vec::Vec;

use understory_property::Property;
use understory_style::{MatchState, ResolveCx, StyleCascade, Theme};

use crate::channels::{LAYOUT, STYLE};
use crate::element::{Element, ElementId};
use crate::lookup::UiResolveLookup;

use super::{Ui, collect_preorder};

impl Ui {
    /// Returns the active combined style cascade.
    #[must_use]
    #[inline]
    pub fn cascade(&self) -> &StyleCascade {
        self.style.cascade()
    }

    /// Returns the active theme.
    #[must_use]
    #[inline]
    pub fn theme(&self) -> &Theme {
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
    /// (LAYOUT → GEOMETRY → PAINT → COMPOSE) handles the rest. Match states
    /// from the previous cascade are discarded — the [`MatchState`]
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
        }
    }

    /// Swaps in a new theme. All elements are marked LAYOUT-dirty since theme
    /// resources can feed layout, text measurement, and paint values.
    pub fn set_theme(&mut self, theme: Theme) {
        self.style.set_theme(theme);
        let ids = collect_preorder(self, self.root());
        for id in ids {
            self.invalidation.mark(id, LAYOUT);
            self.retemplate_if_needed(id);
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
            UiResolveLookup::new(self),
        )
        .get_value(
            element,
            property,
            Some((self.style.cascade(), self.match_state(element.id()))),
        )
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
        let channels = self.style.style_one(id, element, &self.registry);
        if let Some(channels) = channels {
            self.mark_channels(id, channels);
        }
        if allow_retemplate {
            self.retemplate_if_needed(id);
        }
    }
}
