// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Interactive element state: classes, pseudos, focus, hover, press, and capture.
//!
//! This module owns reconciliation between retained element lifecycle and
//! interactive state; it explicitly does not own tree mutation or routed-event
//! storage.

use super::events::PointerRouteData;
use alloc::vec::Vec;

use cursor_icon::CursorIcon;
use kurbo::Point;
use understory_box_tree::NodeFlags;
use understory_event_state::hover::HoverEvent;
use understory_focus::Navigation;
use understory_property::PropertyId;
use understory_style::PseudoClassId;

use crate::element::ElementId;
use crate::event_runtime::{FocusChangeReason, PointerDispatchResult, RoutedEventKind};
use crate::pseudos::{
    PSEUDO_CHECKED, PSEUDO_DISABLED, PSEUDO_FOCUS_VISIBLE, PSEUDO_FOCUSED, PSEUDO_HOVER,
    PSEUDO_PRESSED, PSEUDO_READ_ONLY,
};
use crate::style_class::StyleClass;

use super::transition_planner::InteractionTransitionPlan;
use super::{ControlState, Ui};

impl Ui {
    /// Adds `class` to `id` and re-styles the affected subtree.
    ///
    /// Passing a class name interns it through this UI's style vocabulary.
    pub fn add_class(&mut self, id: ElementId, class: impl Into<StyleClass>) {
        let class = self.resolve_style_class(class);
        let changed = self
            .get_mut(id)
            .map(|el| el.add_class(class))
            .unwrap_or(false);
        if changed {
            self.restyle_subtree(id);
            self.record_element_tree_update(id);
        }
    }

    pub(crate) fn add_initial_class(&mut self, id: ElementId, class: understory_style::ClassId) {
        let changed = self
            .get_mut(id)
            .map(|el| el.add_class(class))
            .unwrap_or(false);
        if changed {
            self.style_one_for_insert(id);
            self.record_element_tree_update(id);
        }
    }

    /// Removes `class` from `id` and re-styles the affected subtree.
    ///
    /// Passing an unknown class name is a no-op; removal does not intern new
    /// names into the style vocabulary.
    pub fn remove_class(&mut self, id: ElementId, class: impl Into<StyleClass>) {
        let Some(class) = class.into().resolve_existing(self.style_vocabulary()) else {
            return;
        };
        let changed = self
            .get_mut(id)
            .map(|el| el.remove_class(class))
            .unwrap_or(false);
        if changed {
            self.restyle_subtree(id);
            self.record_element_tree_update(id);
        }
    }

    /// Adds `pseudo` to `id` and re-styles the affected subtree.
    pub fn add_pseudo(&mut self, id: ElementId, pseudo: PseudoClassId) {
        let transition_plan = InteractionTransitionPlan::capture(self, id);
        let changed = self
            .get_mut(id)
            .map(|el| el.add_pseudo(pseudo))
            .unwrap_or(false);
        if changed {
            self.restyle_subtree(id);
            transition_plan.apply_after(self, id);
            self.record_element_tree_update(id);
        }
    }

    /// Removes `pseudo` from `id` and re-styles the affected subtree.
    pub fn remove_pseudo(&mut self, id: ElementId, pseudo: PseudoClassId) {
        let transition_plan = InteractionTransitionPlan::capture(self, id);
        let changed = self
            .get_mut(id)
            .map(|el| el.remove_pseudo(pseudo))
            .unwrap_or(false);
        if changed {
            self.restyle_subtree(id);
            transition_plan.apply_after(self, id);
            self.record_element_tree_update(id);
        }
    }

    /// Returns the deepest element currently in the `:hover` state, if
    /// any. Higher-up ancestors along the hover path are also
    /// `:hover`; this is just the leaf for
    /// convenience.
    #[must_use]
    #[inline]
    pub fn hovered(&self) -> Option<ElementId> {
        self.events.hovered()
    }

    /// Returns the element currently receiving primary-pointer drag events
    /// after a press, if any.
    ///
    /// Pointer capture preserves the original hit element as the routed drag
    /// source, even when a templated owner carries the `:pressed` visual state.
    #[must_use]
    #[inline]
    pub fn pointer_capture(&self) -> Option<ElementId> {
        self.events.pointer_capture()
    }

    /// Captures primary-pointer movement and release events to `id`.
    ///
    /// Capture is explicit UI state: while present, pointer move/up/cancel
    /// events route from the captured element even when hit testing points
    /// somewhere else. Stale, disabled, or effectively hidden targets are
    /// rejected.
    pub fn capture_pointer(&mut self, id: ElementId) -> bool {
        if self.get(id).is_none() || !self.is_enabled(id) || !self.is_effectively_visible(id) {
            return false;
        }
        self.set_pointer_capture(Some(id))
    }

    /// Releases primary-pointer capture if `id` currently owns it.
    pub fn release_pointer_capture(&mut self, id: ElementId) -> bool {
        if self.pointer_capture() != Some(id) {
            return false;
        }
        self.set_pointer_capture(None)
    }

    /// Returns the cursor icon preferred by the active press target
    /// or deepest hovered widget.
    ///
    /// Hosts can call this after pointer dispatch and apply the
    /// result to their platform window. `None` means the host should
    /// use its platform default.
    #[must_use]
    pub fn cursor_icon(&self) -> Option<CursorIcon> {
        self.events
            .press_target()
            .or_else(|| self.hovered())
            .and_then(|target| self.cursor_icon_for(target))
    }

    /// Applies shared interactive control state to `id`.
    ///
    /// This is the canonical setup path for built-in controls. It stores the
    /// desired focusability, writes `IsEnabled`, updates `:disabled`, and keeps
    /// disabled controls out of focus traversal.
    pub fn set_control_state(&mut self, id: ElementId, state: ControlState) {
        self.set_focusable(id, state.focusable);
        self.set_enabled(id, state.enabled);
    }

    pub(crate) fn set_initial_control_state(&mut self, id: ElementId, state: ControlState) {
        if self.get(id).is_none() {
            return;
        }
        self.focus.set_focusable_preference(id, state.focusable);
        self.apply_focusable_preference(id);
        self.set_local(id, self.properties.is_enabled, state.enabled);
        if state.enabled {
            self.remove_initial_pseudo(id, PSEUDO_DISABLED);
            self.apply_focusable_preference(id);
        } else {
            self.add_initial_pseudo(id, PSEUDO_DISABLED);
            self.set_focusable_flag(id, false);
        }
    }

    fn add_initial_pseudo(&mut self, id: ElementId, pseudo: PseudoClassId) {
        let changed = self
            .get_mut(id)
            .map(|el| el.add_pseudo(pseudo))
            .unwrap_or(false);
        if changed {
            self.style_one_for_insert(id);
            self.record_element_tree_update(id);
        }
    }

    fn remove_initial_pseudo(&mut self, id: ElementId, pseudo: PseudoClassId) {
        let changed = self
            .get_mut(id)
            .map(|el| el.remove_pseudo(pseudo))
            .unwrap_or(false);
        if changed {
            self.style_one_for_insert(id);
            self.record_element_tree_update(id);
        }
    }

    /// Toggles the desired `Focusable` flag for `id`.
    ///
    /// Focusable, enabled elements participate in [`Self::focus_navigate`] and
    /// can receive keyboard events via [`Self::on_key`]. If an element is
    /// disabled, the preference is retained but its box-tree focus flag stays
    /// cleared until it is re-enabled. No-op for stale ids.
    pub fn set_focusable(&mut self, id: ElementId, focusable: bool) {
        if self.get(id).is_none() {
            return;
        }
        self.focus.set_focusable_preference(id, focusable);
        self.apply_focusable_preference(id);
        if !focusable && self.focus.focused() == Some(id) {
            self.set_focus(None);
        }
    }

    /// Returns whether `id` is currently eligible for focus traversal.
    #[must_use]
    pub fn is_focusable(&self, id: ElementId) -> bool {
        let Some(primary) = self.box_for(id) else {
            return false;
        };
        self.geometry
            .flags(primary)
            .map(|flags| flags.contains(NodeFlags::FOCUSABLE))
            .unwrap_or(false)
    }

    pub(in crate::ui) fn apply_focusable_preference(&mut self, id: ElementId) {
        let focusable = self.focus.focusable_preference(id)
            && self.is_enabled(id)
            && self.is_effectively_visible(id);
        self.set_focusable_flag(id, focusable);
    }

    fn set_focusable_flag(&mut self, id: ElementId, focusable: bool) {
        let Some(primary) = self.box_for(id) else {
            return;
        };
        let mut flags = self.geometry.flags(primary).unwrap_or_default();
        let changed = flags.contains(NodeFlags::FOCUSABLE) != focusable;
        if focusable {
            flags.insert(NodeFlags::FOCUSABLE);
        } else {
            flags.remove(NodeFlags::FOCUSABLE);
        }
        self.geometry.set_flags(primary, flags);
        if changed {
            self.record_element_tree_update(id);
        }
    }

    /// Returns whether `id` is currently enabled.
    #[must_use]
    pub fn is_enabled(&self, id: ElementId) -> bool {
        self.get(id)
            .map(|element| self.resolve_value_for(element, self.properties.is_enabled))
            .unwrap_or(false)
    }

    /// Enables or disables a control element.
    ///
    /// Disabling dismisses popups anchored to `id`, repairs or clears focus,
    /// clears pressed state and pointer capture, removes it from focus
    /// traversal, and applies `:disabled` for style matching.
    pub fn set_enabled(&mut self, id: ElementId, enabled: bool) {
        if self.get(id).is_none() {
            return;
        }
        self.set_local(id, self.properties.is_enabled, enabled);
    }

    /// Projects semantic state properties into their runtime side effects.
    pub(in crate::ui) fn sync_state_property_projection(
        &mut self,
        id: ElementId,
        property: PropertyId,
    ) {
        if property == self.properties.is_checked.id() {
            let checked = self
                .get(id)
                .map(|element| self.resolve_value_for(element, self.properties.is_checked))
                .unwrap_or(false);
            self.set_pseudo_state(id, PSEUDO_CHECKED, checked);
        } else if property == self.properties.is_read_only.id() {
            let read_only = self
                .get(id)
                .map(|element| self.resolve_value_for(element, self.properties.is_read_only))
                .unwrap_or(false);
            self.set_pseudo_state(id, PSEUDO_READ_ONLY, read_only);
        } else if property == self.properties.is_enabled.id() {
            self.sync_enabled_projection(id, self.is_enabled(id));
        }
    }

    fn set_pseudo_state(&mut self, id: ElementId, pseudo: PseudoClassId, active: bool) {
        if active {
            self.add_pseudo(id, pseudo);
        } else {
            self.remove_pseudo(id, pseudo);
        }
    }

    fn sync_enabled_projection(&mut self, id: ElementId, enabled: bool) {
        if self.get(id).is_none() {
            return;
        }
        if enabled {
            self.remove_pseudo(id, PSEUDO_DISABLED);
            self.apply_focusable_preference(id);
        } else {
            let _ = self.dismiss_overlay_anchor(id);
            self.add_pseudo(id, PSEUDO_DISABLED);
            self.set_focusable_flag(id, false);
            if self.focus.focused() == Some(id) {
                let contained = self
                    .overlays
                    .topmost_containing_focus_scope()
                    .is_some_and(|scope| self.is_descendant_or_self(id, scope.element));
                if contained {
                    let _ = self.focus_navigate(Navigation::Next);
                } else {
                    self.set_focus(None);
                }
            }
            if self
                .hovered()
                .is_some_and(|hovered| self.press_visual_target(hovered) == id)
            {
                self.clear_hover();
            }
            if self.events.press_target() == Some(id)
                || self
                    .events
                    .pointer_capture()
                    .is_some_and(|capture| capture == id || self.press_visual_target(capture) == id)
            {
                self.clear_pointer_session();
            }
        }
    }

    /// Returns the element that currently has focus, if any.
    #[must_use]
    #[inline]
    pub fn focused(&self) -> Option<ElementId> {
        self.focus.focused()
    }

    /// Sets keyboard-visible focus. Passes `None` to clear focus. Disabled or
    /// stale, disabled, or effectively hidden target ids are treated as
    /// `None`. Applies / removes `:focused` and `:focus-visible` on the affected
    /// elements. Returns `true` if the focus state changed.
    pub fn set_focus(&mut self, new: Option<ElementId>) -> bool {
        self.set_focus_with_visibility(new, true, FocusChangeReason::Programmatic)
    }

    /// Sets pointer-originated focus. Passes `None` to clear focus.
    ///
    /// This applies `:focused` without `:focus-visible`, matching the focus
    /// modality used by the built-in pointer dispatch path. Composed controls
    /// can call this from pointer/click handlers when focus should move without
    /// showing keyboard-visible focus chrome.
    pub fn set_pointer_focus(&mut self, new: Option<ElementId>) -> bool {
        self.set_focus_with_visibility(new, false, FocusChangeReason::Pointer)
    }

    pub(in crate::ui) fn set_keyboard_focus(&mut self, new: Option<ElementId>) -> bool {
        self.set_focus_with_visibility(new, true, FocusChangeReason::Keyboard)
    }

    fn set_focus_with_visibility(
        &mut self,
        new: Option<ElementId>,
        focus_visible: bool,
        reason: FocusChangeReason,
    ) -> bool {
        let previous = self.focus.focused();
        let accepted = new.filter(|id| {
            self.is_enabled(*id)
                && self.is_effectively_visible(*id)
                && !self
                    .interaction_exit_roots
                    .iter()
                    .any(|root| self.get(*root).is_some() && self.is_descendant_or_self(*id, *root))
        });
        let mut changed = false;
        if self.focus.focused() == accepted {
            if let Some(id) = accepted {
                changed |= self.set_focus_visible_pseudo(id, focus_visible);
            }
            return changed;
        }
        if let Some(old_id) = self.focus.take_focused() {
            self.remove_pseudo(old_id, PSEUDO_FOCUSED);
            self.remove_pseudo(old_id, PSEUDO_FOCUS_VISIBLE);
        }
        if let Some(new_id) = accepted {
            self.add_pseudo(new_id, PSEUDO_FOCUSED);
            self.set_focus_visible_pseudo(new_id, focus_visible);
            self.focus.set_focused(Some(new_id));
            changed = true;
        } else {
            self.focus.set_focused(None);
            changed = true;
        }
        if changed {
            if let Some(previous) = previous {
                self.record_element_tree_update(previous);
            }
            if let Some(accepted) = accepted {
                self.record_element_tree_update(accepted);
            }
            self.dispatch_focus_change(previous, accepted, focus_visible, reason);
            self.refresh_tooltip_from_focus();
            self.apply_overlay_focus_change(self.focused());
        }
        changed
    }

    fn set_focus_visible_pseudo(&mut self, id: ElementId, focus_visible: bool) -> bool {
        let had_visible = self
            .get(id)
            .map(|element| element.pseudos().contains(&PSEUDO_FOCUS_VISIBLE))
            .unwrap_or(false);
        if focus_visible {
            self.add_pseudo(id, PSEUDO_FOCUS_VISIBLE);
        } else {
            self.remove_pseudo(id, PSEUDO_FOCUS_VISIBLE);
        }
        had_visible != focus_visible
    }

    /// Updates the hover state to reflect a pointer at `pos`. Computes
    /// the element path from the box tree's hit-test result and feeds
    /// it to the event runtime, applying its `Enter`/`Leave` events to
    /// pseudoclass state.
    pub(in crate::ui) fn update_hover_at(&mut self, pos: Point) -> bool {
        let path: Vec<ElementId> = self.responder().hit_path(pos);
        let events = self.events.update_hover_path(&path);
        self.apply_hover_events(events)
    }

    pub(in crate::ui) fn update_hover_at_with_pointer(
        &mut self,
        pos: Point,
        data: PointerRouteData<'_>,
        result: &mut PointerDispatchResult,
    ) {
        let previous = self.hovered();
        result.hover_changed |= self.update_hover_at(pos);
        let current = self.hovered();
        self.dispatch_hover_transition(previous, current, data, result);
    }

    /// Clears the hover state (e.g. on pointer Leave). Returns `true`
    /// if anything changed.
    fn clear_hover(&mut self) -> bool {
        let events = self.events.clear_hover();
        self.apply_hover_events(events)
    }

    /// Retires every interaction whose responder or visual target is inside a
    /// subtree that is still live but about to be removed.
    ///
    /// Tree mutation owns when this is called. `EventRuntime` remains only the
    /// storage and recognizer owner; it cannot route `FocusLost` or clear a
    /// pressed visual that differs from the captured source on its own.
    pub(in crate::ui) fn begin_interaction_subtree_exit(&mut self, root: ElementId) {
        self.interaction_exit_roots.push(root);

        if self
            .focused()
            .is_some_and(|focused| self.is_descendant_or_self(focused, root))
        {
            self.set_focus(None);
        }
        if self
            .hovered()
            .is_some_and(|hovered| self.is_descendant_or_self(hovered, root))
        {
            self.clear_hover();
        }
        if self
            .events
            .press_target()
            .is_some_and(|pressed| self.is_descendant_or_self(pressed, root))
            || self
                .events
                .pointer_capture()
                .is_some_and(|capture| self.is_descendant_or_self(capture, root))
        {
            self.clear_pointer_session();
        }
    }

    pub(in crate::ui) fn end_interaction_subtree_exit(&mut self, root: ElementId) {
        let index = self
            .interaction_exit_roots
            .iter()
            .rposition(|candidate| *candidate == root)
            .expect("interaction subtree exits must remain balanced");
        self.interaction_exit_roots.remove(index);
    }

    /// Clears interactions which became invalid because an element or one of
    /// its ancestors is no longer visible.
    pub(in crate::ui) fn reconcile_interaction_visibility(&mut self) {
        if self
            .hovered()
            .is_some_and(|hovered| !self.is_effectively_visible(hovered))
        {
            self.clear_hover();
        }
        if self
            .events
            .press_target()
            .is_some_and(|pressed| !self.is_effectively_visible(pressed))
            || self
                .events
                .pointer_capture()
                .is_some_and(|capture| !self.is_effectively_visible(capture))
        {
            self.clear_pointer_session();
        }
    }

    fn clear_pointer_session(&mut self) {
        self.set_press_target(None);
        self.set_pointer_capture(None);
        self.events.clear_press();
        self.events.cancel_click();
    }

    pub(in crate::ui) fn clear_hover_with_pointer(
        &mut self,
        data: PointerRouteData<'_>,
        result: &mut PointerDispatchResult,
    ) {
        let previous = self.hovered();
        result.hover_changed |= self.clear_hover();
        self.dispatch_hover_transition(previous, None, data, result);
    }

    fn dispatch_hover_transition(
        &mut self,
        previous: Option<ElementId>,
        current: Option<ElementId>,
        data: PointerRouteData<'_>,
        result: &mut PointerDispatchResult,
    ) {
        if previous == current {
            return;
        }
        if let Some(previous) = previous
            && self.get(previous).is_some()
        {
            result.include_routed(self.dispatch_routed_pointer_event(
                RoutedEventKind::PointerLeave,
                previous,
                data,
            ));
        }
        if let Some(current) = current
            && self.get(current).is_some()
        {
            result.include_routed(self.dispatch_routed_pointer_event(
                RoutedEventKind::PointerEnter,
                current,
                data,
            ));
        }
    }

    fn apply_hover_events(&mut self, events: Vec<HoverEvent<ElementId>>) -> bool {
        let mut changed = false;
        for event in events {
            match event {
                HoverEvent::Enter(id) => {
                    self.add_pseudo(id, PSEUDO_HOVER);
                    changed = true;
                }
                HoverEvent::Leave(id) => {
                    self.remove_pseudo(id, PSEUDO_HOVER);
                    changed = true;
                }
            }
        }
        changed
    }

    /// Resolves a hit element to the element that should carry the
    /// `:pressed` pseudoclass.
    ///
    /// Hit-testing returns the deepest box under the pointer, which for a
    /// templated control (Button → label `Text`) is the template part rather
    /// than the control itself. Cascade rules keyed on the control's `TypeTag`
    /// (`TYPE_BUTTON:pressed`, …) would never match that part. To match WPF,
    /// climb the `templated_parent` chain to the first ancestor with a retained
    /// template instance — that ancestor is the templated control the part
    /// belongs to. Retained descendants hosted by a template content presenter
    /// use the presenter's template owner for the same reason. Non-template
    /// hits return themselves.
    pub(in crate::ui) fn press_visual_target(&self, hit: ElementId) -> ElementId {
        let mut current = hit;
        loop {
            let Some(element) = self.get(current) else {
                return hit;
            };
            if self.template_instance(current).is_some() {
                return current;
            }
            match element.templated_parent() {
                Some(parent) => current = parent,
                None => {
                    return self
                        .template_owner_for_presented_descendant(hit)
                        .unwrap_or(current);
                }
            }
        }
    }

    fn cursor_icon_for(&self, target: ElementId) -> Option<CursorIcon> {
        let mut current = Some(target);
        while let Some(id) = current {
            let element = self.get(id)?;
            if let Some(icon) = element
                .widget()
                .and_then(|widget| widget.cursor_icon(element))
            {
                return Some(icon);
            }
            current = element.templated_parent().or_else(|| element.parent());
        }
        None
    }

    pub(in crate::ui) fn set_press_target(&mut self, new: Option<ElementId>) -> bool {
        if self.events.press_target() == new {
            return false;
        }
        if let Some(old_id) = self.events.take_press_target() {
            self.remove_pseudo(old_id, PSEUDO_PRESSED);
        }
        if let Some(new_id) = new {
            self.add_pseudo(new_id, PSEUDO_PRESSED);
        }
        self.events.set_press_target(new);
        true
    }

    pub(in crate::ui) fn set_pointer_capture(&mut self, new: Option<ElementId>) -> bool {
        let previous = self.events.pointer_capture();
        if previous == new {
            return false;
        }
        self.events.set_pointer_capture(new);
        if let Some(previous) = previous {
            self.record_element_tree_update(previous);
        }
        if let Some(new) = new {
            self.record_element_tree_update(new);
        }
        true
    }
}
