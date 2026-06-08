// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Transition planning for interaction-driven effective-value changes.
//!
//! This module owns the bridge between an element state mutation and the
//! timeline runtime. It does not decide which properties are animatable and it
//! does not sample active tracks. Animatable property adapters live in
//! `TimelinePropertyRegistry`; style-authored [`crate::TransitionRules`] decide
//! which changed targets actually transition.

use alloc::vec::Vec;

use crate::channels::ARRANGE;
use crate::element::ElementId;
use crate::motion::ResolvedTransitionSpec;
use crate::timeline_runtime::{ChangeCause, PresentationValue, TimelineTarget, VisualOutput};

use super::{Ui, collect_preorder};

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

    const fn decision_for_interaction(
        self,
        spec: Option<ResolvedTransitionSpec>,
    ) -> TransitionDecision {
        if self.interaction_transitions {
            match spec {
                Some(spec) => TransitionDecision::Animate(spec),
                None => TransitionDecision::Snap,
            }
        } else {
            TransitionDecision::Snap
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum TransitionDecision {
    Snap,
    Animate(ResolvedTransitionSpec),
}

/// Before-state capture for one interaction state transition.
#[derive(Clone, Debug)]
pub(in crate::ui) struct InteractionTransitionPlan {
    policy: MotionPolicy,
    root: ElementId,
    tracked: Vec<TrackedValue>,
    layout_outputs: Vec<TrackedLayoutOutput>,
}

impl InteractionTransitionPlan {
    /// Captures animatable effective values before an interaction state change.
    pub(in crate::ui) fn capture(ui: &Ui, id: ElementId) -> Self {
        let mut tracked = Vec::new();
        let mut layout_outputs = Vec::new();
        let properties: Vec<_> = ui.timeline_properties.properties().collect();
        for element in collect_preorder(ui, id) {
            for property in &properties {
                let Some(adapter) = ui.timeline_properties.adapter(*property) else {
                    continue;
                };
                let target = TimelineTarget::Property(*property);
                tracked.push(TrackedValue {
                    element,
                    target,
                    before: adapter.target_value(ui, element, *property),
                    before_spec: ui.transition_spec_for(element, target, ChangeCause::Interaction),
                });
            }
            let target = TimelineTarget::VisualOutput(VisualOutput::LayerOffset);
            layout_outputs.push(TrackedLayoutOutput {
                element,
                target,
                before_spec: ui.transition_spec_for(element, target, ChangeCause::Interaction),
            });
        }

        Self {
            policy: ui.motion_policy,
            root: id,
            tracked,
            layout_outputs,
        }
    }

    /// Applies transitions from captured before values to current after values.
    pub(in crate::ui) fn apply_after(self, ui: &mut Ui, _id: ElementId) {
        for tracked in self.tracked {
            tracked.apply(ui, self.policy);
        }
        for output in self.layout_outputs {
            output.request_after(ui, self.root, self.policy);
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct TrackedLayoutOutput {
    element: ElementId,
    target: TimelineTarget,
    before_spec: Option<ResolvedTransitionSpec>,
}

impl TrackedLayoutOutput {
    fn request_after(self, ui: &mut Ui, root: ElementId, policy: MotionPolicy) {
        if !has_arrange_invalidated_ancestor(ui, root, self.element) {
            return;
        }
        let spec = ui
            .transition_spec_for(self.element, self.target, ChangeCause::Interaction)
            .or(self.before_spec);
        if let TransitionDecision::Animate(spec) = policy.decision_for_interaction(spec) {
            ui.request_presentation_transition(self.element, self.target, spec);
        }
    }
}

fn has_arrange_invalidated_ancestor(ui: &Ui, root: ElementId, mut element: ElementId) -> bool {
    loop {
        if ui.invalidation().is_invalidated(element, ARRANGE) {
            return true;
        }
        if element == root {
            return false;
        }
        let Some(parent) = ui.get(element).and_then(|element| element.parent()) else {
            return false;
        };
        element = parent;
    }
}

#[derive(Clone, Copy, Debug)]
struct TrackedValue {
    element: ElementId,
    target: TimelineTarget,
    before: Option<PresentationValue>,
    before_spec: Option<ResolvedTransitionSpec>,
}

impl TrackedValue {
    fn apply(self, ui: &mut Ui, policy: MotionPolicy) {
        let after = self.target_value(ui);
        if self.before == after {
            return;
        }

        let spec = ui
            .transition_spec_for(self.element, self.target, ChangeCause::Interaction)
            .or(self.before_spec);

        match (self.before, after) {
            (Some(before), Some(after)) => match policy.decision_for_interaction(spec) {
                TransitionDecision::Animate(spec) => {
                    ui.set_presentation_target_value(
                        self.element,
                        self.target,
                        before,
                        ChangeCause::Other,
                    );
                    ui.request_presentation_transition(self.element, self.target, spec);
                    ui.set_presentation_target_value(
                        self.element,
                        self.target,
                        after,
                        ChangeCause::Interaction,
                    );
                }
                TransitionDecision::Snap => {
                    ui.clear_presentation_target(self.element, self.target);
                }
            },
            (None, Some(after)) => {
                if spec.is_some() {
                    ui.set_presentation_target_value(
                        self.element,
                        self.target,
                        after,
                        ChangeCause::Other,
                    );
                } else {
                    ui.clear_presentation_target(self.element, self.target);
                }
            }
            (Some(_), None) | (None, None) => {
                ui.clear_presentation_target(self.element, self.target);
            }
        }
    }

    fn target_value(self, ui: &Ui) -> Option<PresentationValue> {
        let TimelineTarget::Property(property) = self.target else {
            return None;
        };
        ui.timeline_properties
            .adapter(property)?
            .target_value(ui, self.element, property)
    }
}
