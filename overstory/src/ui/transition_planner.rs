// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Transition planning for causally grouped effective-value changes.
//!
//! This module owns before/after capture across one retained mutation or scoped
//! motion transaction and bridges changed targets to the timeline runtime. It
//! does not decide which properties are animatable or sample active tracks.

use alloc::vec::Vec;

use kurbo::Vec2;
use understory_property::PropertyId;

use crate::channels::ARRANGE;
use crate::element::ElementId;
use crate::motion::{PresentationSlot, PresentationTarget};
use crate::timeline_properties::{TimelineTargetAdapter, TimelineTargetValue};
use crate::timeline_runtime::{ChangeCause, ResolvedTransition};

use super::{Ui, collect_preorder};

/// Internal policy for implicit and scoped programmatic motion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::ui) struct MotionPolicy {
    transaction: Option<ResolvedTransition>,
}

impl MotionPolicy {
    /// Creates the default motion policy.
    #[must_use]
    pub(in crate::ui) const fn new() -> Self {
        Self { transaction: None }
    }

    pub(in crate::ui) fn replace_transaction(
        &mut self,
        transaction: Option<ResolvedTransition>,
    ) -> Option<ResolvedTransition> {
        core::mem::replace(&mut self.transaction, transaction)
    }

    pub(in crate::ui) const fn has_transaction(self) -> bool {
        self.transaction.is_some()
    }

    fn decision(self, styled: Option<ResolvedTransition>) -> TransitionDecision {
        match self.transaction.or(styled) {
            Some(spec) => TransitionDecision::Animate(spec),
            None => TransitionDecision::Snap,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum TransitionDecision {
    Snap,
    Animate(ResolvedTransition),
}

/// Before-state journal for one causally grouped presentation change.
pub(in crate::ui) struct PresentationTransitionPlan {
    policy: MotionPolicy,
    roots: Vec<ElementId>,
    tracked: Vec<TrackedValue>,
    layout_outputs: Vec<TrackedLayoutOutput>,
}

impl PresentationTransitionPlan {
    /// Creates an empty journal governed by `policy`.
    pub(in crate::ui) const fn new(policy: MotionPolicy) -> Self {
        Self {
            policy,
            roots: Vec::new(),
            tracked: Vec::new(),
            layout_outputs: Vec::new(),
        }
    }

    /// Captures animatable effective values before a retained state change.
    pub(in crate::ui) fn capture(ui: &Ui, id: ElementId) -> Self {
        let mut plan = Self::new(ui.motion_policy);
        plan.capture_root(ui, id);
        plan
    }

    /// Adds the before-state of `id` and its existing descendants to the journal.
    pub(in crate::ui) fn capture_root(&mut self, ui: &Ui, id: ElementId) {
        if ui.get(id).is_none() {
            return;
        }
        if !self.roots.contains(&id) {
            self.roots.push(id);
        }

        let target_adapters: Vec<_> = ui.timeline_properties.target_adapters().collect();
        for element in collect_preorder(ui, id) {
            // Values that have never participated in a frame are initial state,
            // not an observable before-state for an implicit transition.
            if !ui.composition.has_established_presentation(element) {
                continue;
            }
            // The layout entry doubles as the per-element capture marker. All
            // registered property adapters are captured in the same pass.
            if self
                .layout_outputs
                .iter()
                .any(|output| output.element == element)
            {
                continue;
            }
            for &(property, adapter) in &target_adapters {
                let target = PresentationTarget::Property(property);
                self.tracked.push(TrackedValue {
                    element,
                    property,
                    target,
                    adapter,
                    before_presented: adapter.presented_value(ui, element, property),
                    before_base: adapter.base_value(ui, element, property),
                    before_spec: ui.transition_spec_for(element, target, ChangeCause::Interaction),
                });
            }
            let target = PresentationTarget::Slot(PresentationSlot::LayerOffset);
            self.layout_outputs.push(TrackedLayoutOutput {
                element,
                target,
                before_presented: ui
                    .composition
                    .layer_offset_for(element)
                    .unwrap_or(Vec2::ZERO),
                before_spec: ui.transition_spec_for(element, target, ChangeCause::Interaction),
            });
        }
    }

    /// Applies transitions from captured before values to current after values.
    pub(in crate::ui) fn apply_after(self, ui: &mut Ui) {
        let Self {
            policy,
            roots,
            tracked,
            layout_outputs,
        } = self;

        validate_transition_declarations(ui, &roots);
        for tracked in tracked {
            tracked.apply(ui, policy);
        }
        for output in layout_outputs {
            output.request_after(ui, &roots, policy);
        }
    }
}

impl Ui {
    /// Records the before-state for a write in the active scoped transaction.
    pub(in crate::ui) fn capture_motion_transaction_change(&mut self, root: ElementId) {
        let Some(mut transaction) = self.motion_transaction.take() else {
            return;
        };
        transaction.capture_root(self, root);
        self.motion_transaction = Some(transaction);
    }

    /// Captures an ordinary state change, or journals it when a scope is active.
    pub(in crate::ui) fn plan_presentation_change(
        &mut self,
        root: ElementId,
    ) -> Option<PresentationTransitionPlan> {
        if self.motion_transaction.is_some() {
            self.capture_motion_transaction_change(root);
            None
        } else {
            Some(PresentationTransitionPlan::capture(self, root))
        }
    }

    /// Commits the current scoped journal, if any.
    pub(in crate::ui) fn finish_motion_transaction(&mut self) {
        if let Some(transaction) = self.motion_transaction.take() {
            transaction.apply_after(self);
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct TrackedLayoutOutput {
    element: ElementId,
    target: PresentationTarget,
    before_presented: Vec2,
    before_spec: Option<ResolvedTransition>,
}

impl TrackedLayoutOutput {
    fn request_after(self, ui: &mut Ui, roots: &[ElementId], policy: MotionPolicy) {
        if !roots
            .iter()
            .copied()
            .any(|root| has_arrange_invalidated_ancestor(ui, root, self.element))
        {
            return;
        }
        let spec = ui
            .transition_spec_for(self.element, self.target, ChangeCause::Interaction)
            .or(self.before_spec);
        if let TransitionDecision::Animate(spec) = policy.decision(spec) {
            if !ui.has_presentation_target(self.element, self.target) {
                ui.set_presentation_target(
                    self.element,
                    self.target,
                    self.before_presented,
                    ChangeCause::Other,
                );
            }
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

fn validate_transition_declarations(ui: &Ui, roots: &[ElementId]) {
    for root in roots {
        for element in collect_preorder(ui, *root) {
            let Some(element_ref) = ui.get(element) else {
                continue;
            };
            let declarations = ui.resolve_value_for(element_ref, ui.properties.transitions);
            for declaration in declarations.declarations() {
                let PresentationTarget::Property(property) = declaration.target() else {
                    continue;
                };
                assert!(
                    ui.timeline_properties.target_adapter(property).is_some(),
                    "motion target property {property:?} is not registered; register custom \
                     properties with `Ui::register_animation_target_property` or \
                     `Ui::register_color_animation_target_property`"
                );
            }
        }
    }
}

struct TrackedValue {
    element: ElementId,
    property: PropertyId,
    target: PresentationTarget,
    adapter: TimelineTargetAdapter,
    before_presented: Option<TimelineTargetValue>,
    before_base: Option<TimelineTargetValue>,
    before_spec: Option<ResolvedTransition>,
}

impl TrackedValue {
    fn apply(self, ui: &mut Ui, policy: MotionPolicy) {
        let Self {
            element,
            property,
            target,
            adapter,
            before_presented,
            before_base,
            before_spec,
        } = self;

        let after = adapter.base_value(ui, element, property);
        if same_target_value(adapter, before_base.as_ref(), after.as_ref()) {
            return;
        }

        let spec = ui
            .transition_spec_for(element, target, ChangeCause::Interaction)
            .or(before_spec);

        match (before_presented, after) {
            (Some(before), Some(after)) => match policy.decision(spec) {
                TransitionDecision::Animate(spec) => {
                    if !ui.has_presentation_target(element, target) {
                        set_target(
                            ui,
                            element,
                            property,
                            target,
                            adapter,
                            before,
                            ChangeCause::Other,
                        );
                    }
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
