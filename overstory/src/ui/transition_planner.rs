// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Transition planning for interaction-driven effective-value changes.
//!
//! This module owns the bridge between an element state mutation and the
//! timeline runtime. It does not decide which properties are animatable and it
//! does not sample active tracks. Animatable property adapters live in
//! `TimelinePropertyRegistry`; style-authored transition declarations decide
//! which changed targets actually transition.

use alloc::vec::Vec;

use crate::channels::ARRANGE;
use crate::element::ElementId;
use crate::motion::{PresentationSlot, PresentationTarget};
use crate::timeline_properties::{TimelineTargetAdapter, TimelineTargetValue};
use crate::timeline_runtime::{ChangeCause, ResolvedTransition};
use understory_property::PropertyId;

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
        spec: Option<ResolvedTransition>,
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
    Animate(ResolvedTransition),
}

/// Before-state capture for one interaction state transition.
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
        let target_adapters: Vec<_> = ui.timeline_properties.target_adapters().collect();
        for element in collect_preorder(ui, id) {
            // Values that have never participated in a frame are initial state,
            // not an observable before-state for an implicit transition.
            if !ui.composition.has_established_presentation(element) {
                continue;
            }
            for &(property, adapter) in &target_adapters {
                let target = PresentationTarget::Property(property);
                tracked.push(TrackedValue {
                    element,
                    property,
                    target,
                    adapter,
                    before: adapter.target_value(ui, element, property),
                    before_spec: ui.transition_spec_for(element, target, ChangeCause::Interaction),
                });
            }
            let target = PresentationTarget::Slot(PresentationSlot::LayerOffset);
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
    target: PresentationTarget,
    before_spec: Option<ResolvedTransition>,
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

struct TrackedValue {
    element: ElementId,
    property: PropertyId,
    target: PresentationTarget,
    adapter: TimelineTargetAdapter,
    before: Option<TimelineTargetValue>,
    before_spec: Option<ResolvedTransition>,
}

impl TrackedValue {
    fn apply(self, ui: &mut Ui, policy: MotionPolicy) {
        let Self {
            element,
            property,
            target,
            adapter,
            before,
            before_spec,
        } = self;

        let after = adapter.target_value(ui, element, property);
        if same_target_value(adapter, before.as_ref(), after.as_ref()) {
            return;
        }

        let spec = ui
            .transition_spec_for(element, target, ChangeCause::Interaction)
            .or(before_spec);

        match (before, after) {
            (Some(before), Some(after)) => match policy.decision_for_interaction(spec) {
                TransitionDecision::Animate(spec) => {
                    set_target(
                        ui,
                        element,
                        property,
                        target,
                        adapter,
                        before,
                        ChangeCause::Other,
                    );
                    ui.request_presentation_transition(element, target, spec);
                    set_target(
                        ui,
                        element,
                        property,
                        target,
                        adapter,
                        after,
                        ChangeCause::Interaction,
                    );
                }
                TransitionDecision::Snap => {
                    ui.clear_presentation_target(element, target);
                }
            },
            (None, Some(after)) => {
                if spec.is_some() {
                    set_target(
                        ui,
                        element,
                        property,
                        target,
                        adapter,
                        after,
                        ChangeCause::Other,
                    );
                } else {
                    ui.clear_presentation_target(element, target);
                }
            }
            (Some(_), None) | (None, None) => {
                ui.clear_presentation_target(element, target);
            }
        }
    }
}

fn same_target_value(
    adapter: TimelineTargetAdapter,
    before: Option<&TimelineTargetValue>,
    after: Option<&TimelineTargetValue>,
) -> bool {
    match (before, after) {
        (Some(before), Some(after)) => adapter.same_value(before, after),
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
    }
}

fn set_target(
    ui: &mut Ui,
    element: ElementId,
    property: PropertyId,
    target: PresentationTarget,
    adapter: TimelineTargetAdapter,
    value: TimelineTargetValue,
    cause: ChangeCause,
) -> bool {
    match (adapter, value) {
        (TimelineTargetAdapter::Presentation(_), TimelineTargetValue::Presentation(value)) => {
            ui.set_presentation_target_value(element, target, value, cause)
        }
        (TimelineTargetAdapter::Native(adapter), TimelineTargetValue::Native(value)) => ui
            .set_native_presentation_property_target_value(
                element, property, adapter, value, cause,
            ),
        _ => false,
    }
}
