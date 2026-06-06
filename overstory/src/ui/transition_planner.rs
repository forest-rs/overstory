// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Transition planning for interaction-driven presentation values.
//!
//! This module owns deciding which values are eligible for interaction-state
//! transitions. It does not own style resolution, timeline sampling, or control
//! semantic state.

use peniko::{Brush, Color};
use understory_presentation::Edges;
use understory_property::Property;
use understory_style::PseudoClassId;

use crate::element::ElementId;
use crate::pseudos::{
    PSEUDO_CHECKED, PSEUDO_FOCUS_VISIBLE, PSEUDO_FOCUSED, PSEUDO_HOVER, PSEUDO_PRESSED,
    PSEUDO_SELECTED,
};
use crate::timeline_runtime::{AnimatableValue, ChangeCause, TimelineTarget, TimelineTrackKey};

use super::Ui;

/// Internal policy hook for interaction-driven motion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::ui) struct MotionPolicy {
    interaction_transitions: bool,
}

impl MotionPolicy {
    /// Creates the default motion policy.
    #[must_use]
    pub(in crate::ui) const fn new() -> Self {
        Self {
            interaction_transitions: true,
        }
    }

    const fn decision_for_interaction(self, _target: TimelineTarget) -> TransitionDecision {
        if self.interaction_transitions {
            TransitionDecision::Animate
        } else {
            TransitionDecision::Snap
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransitionDecision {
    Snap,
    Animate,
}

/// Before-state capture for one pseudo-class transition.
#[derive(Clone, Copy, Debug)]
pub(in crate::ui) struct InteractionTransitionPlan {
    policy: MotionPolicy,
    background: Option<TrackedValue<Color>>,
    border_brushes: Option<Edges<TrackedValue<Color>>>,
    border_widths: Option<Edges<TrackedValue<f64>>>,
    opacity: Option<TrackedValue<f32>>,
}

impl InteractionTransitionPlan {
    /// Captures values before `pseudo` changes on `id`.
    pub(in crate::ui) fn capture(ui: &Ui, id: ElementId, pseudo: PseudoClassId) -> Self {
        let properties = ui.properties;
        Self {
            policy: ui.motion_policy,
            background: interaction_pseudo_animates_background(pseudo).then(|| {
                TrackedValue::new(
                    TimelineTarget::Property(properties.surface.background.id()),
                    solid_optional_brush_color(ui, id, properties.surface.background),
                )
            }),
            border_brushes: interaction_pseudo_animates_border_brush(pseudo).then(|| {
                Edges::new(
                    tracked_brush(ui, id, properties.surface.border_brushes.top),
                    tracked_brush(ui, id, properties.surface.border_brushes.right),
                    tracked_brush(ui, id, properties.surface.border_brushes.bottom),
                    tracked_brush(ui, id, properties.surface.border_brushes.left),
                )
            }),
            border_widths: interaction_pseudo_animates_border_width(pseudo).then(|| {
                Edges::new(
                    tracked_width(ui, id, properties.surface.border_widths.top),
                    tracked_width(ui, id, properties.surface.border_widths.right),
                    tracked_width(ui, id, properties.surface.border_widths.bottom),
                    tracked_width(ui, id, properties.surface.border_widths.left),
                )
            }),
            opacity: (pseudo == PSEUDO_PRESSED).then(|| {
                TrackedValue::new(
                    TimelineTarget::Property(properties.opacity.id()),
                    opacity_target(ui, id),
                )
            }),
        }
    }

    /// Applies the after-state values produced by the completed pseudo change.
    pub(in crate::ui) fn apply_after(self, ui: &mut Ui, id: ElementId) {
        self.clear_samples(ui, id);

        let properties = ui.properties;
        let background_after = solid_optional_brush_color(ui, id, properties.surface.background);
        let border_brush_after = border_brush_targets(ui, id);
        let border_width_after = border_width_targets(ui, id);
        let opacity_after = opacity_target(ui, id);

        if let Some(tracked) = self.background {
            tracked.apply(ui, id, self.policy, background_after);
        }
        if let Some(tracked) = self.border_brushes {
            apply_tracked_edges(ui, id, self.policy, tracked, border_brush_after);
        }
        if let Some(tracked) = self.border_widths {
            apply_tracked_edges(ui, id, self.policy, tracked, border_width_after);
        }
        if let Some(tracked) = self.opacity {
            tracked.apply(ui, id, self.policy, opacity_after);
        }
    }

    fn clear_samples(self, ui: &mut Ui, id: ElementId) {
        if let Some(tracked) = self.background {
            tracked.clear_sample(ui, id);
        }
        if let Some(tracked) = self.border_brushes {
            clear_tracked_edges(ui, id, tracked);
        }
        if let Some(tracked) = self.border_widths {
            clear_tracked_edges(ui, id, tracked);
        }
        if let Some(tracked) = self.opacity {
            tracked.clear_sample(ui, id);
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct TrackedValue<T> {
    target: TimelineTarget,
    before: Option<T>,
}

impl<T> TrackedValue<T>
where
    T: AnimatableValue,
{
    const fn new(target: TimelineTarget, before: Option<T>) -> Self {
        Self { target, before }
    }

    fn clear_sample(self, ui: &mut Ui, id: ElementId) {
        ui.clear_timeline_property_sample(TimelineTrackKey::new(id, self.target));
    }

    fn apply(self, ui: &mut Ui, id: ElementId, policy: MotionPolicy, after: Option<T>) {
        match (self.before, after) {
            (Some(before), Some(after)) => {
                ui.set_presentation_target(id, self.target, before, ChangeCause::Other);
                match policy.decision_for_interaction(self.target) {
                    TransitionDecision::Animate => {
                        ui.request_styled_presentation_transition(id, self.target);
                        ui.set_presentation_target(
                            id,
                            self.target,
                            after,
                            ChangeCause::Interaction,
                        );
                    }
                    TransitionDecision::Snap => {
                        ui.set_presentation_target(id, self.target, after, ChangeCause::Other);
                    }
                }
            }
            (None, Some(after)) => {
                ui.set_presentation_target(id, self.target, after, ChangeCause::Other);
            }
            (Some(_), None) | (None, None) => {
                ui.clear_presentation_target(id, self.target);
            }
        }
    }
}

fn tracked_brush(ui: &Ui, id: ElementId, property: Property<Option<Brush>>) -> TrackedValue<Color> {
    TrackedValue::new(
        TimelineTarget::Property(property.id()),
        solid_optional_brush_color(ui, id, property),
    )
}

fn tracked_width(ui: &Ui, id: ElementId, property: Property<f64>) -> TrackedValue<f64> {
    TrackedValue::new(
        TimelineTarget::Property(property.id()),
        numeric_property_target(ui, id, property),
    )
}

fn clear_tracked_edges<T>(ui: &mut Ui, id: ElementId, edges: Edges<TrackedValue<T>>)
where
    T: AnimatableValue,
{
    edges.top.clear_sample(ui, id);
    edges.right.clear_sample(ui, id);
    edges.bottom.clear_sample(ui, id);
    edges.left.clear_sample(ui, id);
}

fn apply_tracked_edges<T>(
    ui: &mut Ui,
    id: ElementId,
    policy: MotionPolicy,
    tracked: Edges<TrackedValue<T>>,
    after: Edges<Option<T>>,
) where
    T: AnimatableValue,
{
    tracked.top.apply(ui, id, policy, after.top);
    tracked.right.apply(ui, id, policy, after.right);
    tracked.bottom.apply(ui, id, policy, after.bottom);
    tracked.left.apply(ui, id, policy, after.left);
}

fn solid_optional_brush_color(
    ui: &Ui,
    id: ElementId,
    property: Property<Option<Brush>>,
) -> Option<Color> {
    let element = ui.get(id)?;
    match ui.resolve_value_for(element, property) {
        Some(Brush::Solid(color)) => Some(color),
        _ => None,
    }
}

fn numeric_property_target(ui: &Ui, id: ElementId, property: Property<f64>) -> Option<f64> {
    let element = ui.get(id)?;
    Some(ui.resolve_value_for(element, property))
}

fn border_brush_targets(ui: &Ui, id: ElementId) -> Edges<Option<Color>> {
    let properties = ui.properties.surface.border_brushes;
    Edges::new(
        solid_optional_brush_color(ui, id, properties.top),
        solid_optional_brush_color(ui, id, properties.right),
        solid_optional_brush_color(ui, id, properties.bottom),
        solid_optional_brush_color(ui, id, properties.left),
    )
}

fn border_width_targets(ui: &Ui, id: ElementId) -> Edges<Option<f64>> {
    let properties = ui.properties.surface.border_widths;
    Edges::new(
        numeric_property_target(ui, id, properties.top),
        numeric_property_target(ui, id, properties.right),
        numeric_property_target(ui, id, properties.bottom),
        numeric_property_target(ui, id, properties.left),
    )
}

fn opacity_target(ui: &Ui, id: ElementId) -> Option<f32> {
    let element = ui.get(id)?;
    Some(ui.resolve_value_for(element, ui.properties.opacity))
}

fn interaction_pseudo_animates_background(pseudo: PseudoClassId) -> bool {
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

fn interaction_pseudo_animates_border_brush(pseudo: PseudoClassId) -> bool {
    matches!(
        pseudo,
        PSEUDO_HOVER | PSEUDO_PRESSED | PSEUDO_FOCUS_VISIBLE | PSEUDO_CHECKED | PSEUDO_SELECTED
    )
}

fn interaction_pseudo_animates_border_width(pseudo: PseudoClassId) -> bool {
    pseudo == PSEUDO_FOCUS_VISIBLE
}
