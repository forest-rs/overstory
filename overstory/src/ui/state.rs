// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Interactive element state: classes, pseudos, focus, hover, press, and capture.

use super::events::PointerRouteData;
use alloc::vec::Vec;

use cursor_icon::CursorIcon;
use kurbo::Point;
use peniko::{Brush, Color};
use understory_box_tree::NodeFlags;
use understory_event_state::hover::HoverEvent;

use crate::animation_runtime::{
    ChangeCause, PresentationProperty, TransitionPolicy, TransitionSpec,
};
use crate::element::ElementId;
use crate::event_runtime::{FocusChangeReason, PointerDispatchResult, RoutedEventKind};
use crate::pseudos::{
    PSEUDO_CHECKED, PSEUDO_DISABLED, PSEUDO_FOCUS_VISIBLE, PSEUDO_FOCUSED, PSEUDO_HOVER,
    PSEUDO_PRESSED, PSEUDO_SELECTED,
};

use super::{ControlState, Ui};

const INTERACTION_BACKGROUND_TRANSITION: TransitionSpec = TransitionSpec::linear(90_000_000);
const INTERACTION_OPACITY_TRANSITION: TransitionSpec = TransitionSpec::linear(70_000_000);
const PRESSED_OPACITY: f32 = 0.92;

impl Ui {
    /// Adds `class` to `id` and re-styles the affected subtree.
    pub fn add_class(&mut self, id: ElementId, class: understory_style::ClassId) {
        let changed = self
            .get_mut(id)
            .map(|el| el.add_class(class))
            .unwrap_or(false);
        if changed {
            self.restyle_subtree(id);
        }
    }

    /// Removes `class` from `id` and re-styles the affected subtree.
    pub fn remove_class(&mut self, id: ElementId, class: understory_style::ClassId) {
        let changed = self
            .get_mut(id)
            .map(|el| el.remove_class(class))
            .unwrap_or(false);
        if changed {
            self.restyle_subtree(id);
        }
    }

    /// Adds `pseudo` to `id` and re-styles the affected subtree.
    pub fn add_pseudo(&mut self, id: ElementId, pseudo: understory_style::PseudoClassId) {
        let background_before = self.interaction_background_before(id, pseudo);
        let pressed_before = self.pressed_opacity_target(id);
        let changed = self
            .get_mut(id)
            .map(|el| el.add_pseudo(pseudo))
            .unwrap_or(false);
        if changed {
            self.restyle_subtree(id);
            self.sync_interaction_visual_transition(id, pseudo, background_before, pressed_before);
        }
    }

    /// Removes `pseudo` from `id` and re-styles the affected subtree.
    pub fn remove_pseudo(&mut self, id: ElementId, pseudo: understory_style::PseudoClassId) {
        let background_before = self.interaction_background_before(id, pseudo);
        let pressed_before = self.pressed_opacity_target(id);
        let changed = self
            .get_mut(id)
            .map(|el| el.remove_pseudo(pseudo))
            .unwrap_or(false);
        if changed {
            self.restyle_subtree(id);
            self.sync_interaction_visual_transition(id, pseudo, background_before, pressed_before);
        }
    }

    fn interaction_background_before(
        &self,
        id: ElementId,
        pseudo: understory_style::PseudoClassId,
    ) -> Option<Color> {
        interaction_pseudo_animates_background(pseudo)
            .then(|| self.solid_background_color(id))
            .flatten()
    }

    fn solid_background_color(&self, id: ElementId) -> Option<Color> {
        let element = self.get(id)?;
        match self.effective_value_for(element, self.properties.background) {
            Some(Brush::Solid(color)) => Some(color),
            _ => None,
        }
    }

    fn pressed_opacity_target(&self, id: ElementId) -> Option<f32> {
        self.get(id).map(|element| {
            if element.pseudos().contains(&PSEUDO_PRESSED) {
                PRESSED_OPACITY
            } else {
                1.0
            }
        })
    }

    fn sync_interaction_visual_transition(
        &mut self,
        id: ElementId,
        pseudo: understory_style::PseudoClassId,
        background_before: Option<Color>,
        pressed_before: Option<f32>,
    ) {
        if let Some(before) = background_before {
            self.set_presentation_target(
                id,
                PresentationProperty::BackgroundColor,
                before,
                ChangeCause::Other,
            );
        }
        if interaction_pseudo_animates_background(pseudo)
            && let Some(after) = self.solid_background_color(id)
        {
            self.request_presentation_transition(
                id,
                PresentationProperty::BackgroundColor,
                TransitionPolicy::interaction(INTERACTION_BACKGROUND_TRANSITION),
            );
            self.set_presentation_target(
                id,
                PresentationProperty::BackgroundColor,
                after,
                ChangeCause::Interaction,
            );
        }
        if pseudo == PSEUDO_PRESSED
            && let Some(before) = pressed_before
            && let Some(after) = self.pressed_opacity_target(id)
        {
            self.set_presentation_target(
                id,
                PresentationProperty::LayerOpacity,
                before,
                ChangeCause::Other,
            );
            self.request_presentation_transition(
                id,
                PresentationProperty::LayerOpacity,
                TransitionPolicy::interaction(INTERACTION_OPACITY_TRANSITION),
            );
            self.set_presentation_target(
                id,
                PresentationProperty::LayerOpacity,
                after,
                ChangeCause::Interaction,
            );
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
    /// somewhere else. Stale or disabled targets are rejected.
    pub fn capture_pointer(&mut self, id: ElementId) -> bool {
        if self.get(id).is_none() || !self.is_enabled(id) {
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

    fn apply_focusable_preference(&mut self, id: ElementId) {
        let focusable = self.focus.focusable_preference(id) && self.is_enabled(id);
        self.set_focusable_flag(id, focusable);
    }

    fn set_focusable_flag(&mut self, id: ElementId, focusable: bool) {
        let Some(primary) = self.box_for(id) else {
            return;
        };
        let mut flags = self.geometry.flags(primary).unwrap_or_default();
        if focusable {
            flags.insert(NodeFlags::FOCUSABLE);
        } else {
            flags.remove(NodeFlags::FOCUSABLE);
        }
        self.geometry.set_flags(primary, flags);
    }

    /// Returns whether `id` is currently enabled.
    #[must_use]
    pub fn is_enabled(&self, id: ElementId) -> bool {
        self.get(id)
            .map(|element| self.effective_value_for(element, self.properties.is_enabled))
            .unwrap_or(false)
    }

    /// Enables or disables a control element.
    ///
    /// Disabling clears focus, pressed state, and pointer capture for `id`,
    /// removes it from focus traversal, and applies `:disabled` for style
    /// matching.
    pub fn set_enabled(&mut self, id: ElementId, enabled: bool) {
        if self.get(id).is_none() {
            return;
        }
        self.set_local(id, self.properties.is_enabled, enabled);
        if enabled {
            self.remove_pseudo(id, PSEUDO_DISABLED);
            self.apply_focusable_preference(id);
        } else {
            self.add_pseudo(id, PSEUDO_DISABLED);
            self.set_focusable_flag(id, false);
            if self.focus.focused() == Some(id) {
                self.set_focus(None);
            }
            let clear_pointer =
                self.events.press_target() == Some(id) || self.events.pointer_capture() == Some(id);
            if self.events.press_target() == Some(id) {
                self.set_press_target(None);
            }
            if clear_pointer {
                self.set_pointer_capture(None);
                self.events.clear_press();
                self.events.cancel_click();
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
    /// stale target ids are treated as `None`. Applies / removes `:focused`
    /// and `:focus-visible` on the affected elements. Returns `true` if the
    /// focus state changed.
    pub fn set_focus(&mut self, new: Option<ElementId>) -> bool {
        self.set_focus_with_visibility(new, true, FocusChangeReason::Programmatic)
    }

    pub(crate) fn set_pointer_focus(&mut self, new: Option<ElementId>) -> bool {
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
        let accepted = new.filter(|id| self.is_enabled(*id));
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
            self.dispatch_focus_change(previous, accepted, focus_visible, reason);
            self.refresh_tooltip_from_focus();
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
        let path: Vec<ElementId> = self
            .geometry
            .hit_test(pos)
            .map(|hit| {
                hit.path
                    .into_iter()
                    .filter_map(|n| self.node_to_element(n))
                    .collect()
            })
            .unwrap_or_default();
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
    /// Hit-testing returns the deepest box under the pointer, which
    /// for a templated control (Button → label `Text`) is the
    /// template part rather than the control itself. Cascade rules
    /// keyed on the control's `TypeTag` (`TYPE_BUTTON:pressed`, …)
    /// would never match that part. To match WPF, climb the
    /// `templated_parent` chain to the first ancestor whose widget
    /// owns a default template — that ancestor is the templated
    /// control the part belongs to. Non-template hits return
    /// themselves.
    pub(in crate::ui) fn press_visual_target(&self, hit: ElementId) -> ElementId {
        let mut current = hit;
        loop {
            let Some(element) = self.get(current) else {
                return hit;
            };
            if element
                .widget()
                .and_then(|w| w.default_template())
                .is_some()
            {
                return current;
            }
            match element.templated_parent() {
                Some(parent) => current = parent,
                None => return current,
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
        if self.events.pointer_capture() == new {
            return false;
        }
        self.events.set_pointer_capture(new);
        true
    }
}

fn interaction_pseudo_animates_background(pseudo: understory_style::PseudoClassId) -> bool {
    matches!(
        pseudo,
        PSEUDO_HOVER
            | PSEUDO_PRESSED
            | PSEUDO_FOCUSED
            | PSEUDO_FOCUS_VISIBLE
            | PSEUDO_CHECKED
            | PSEUDO_SELECTED
    )
}
