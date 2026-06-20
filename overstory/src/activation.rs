// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Semantic activation event types shared by controls.
//!
//! This module owns control-level activation vocabulary: how an activation was
//! requested and why an attempted activation was denied. It explicitly does not
//! own raw pointer, keyboard, focus, or scroll routing; those remain in the
//! routed input event runtime.

use crate::element::ElementId;

/// Input path that requested a semantic control activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationSource {
    /// Activation came from a routed primary pointer click.
    Pointer,
    /// Activation came from keyboard activation, such as Space or Enter.
    Keyboard,
    /// Activation came from an accessibility action request.
    Accessibility,
}

/// Reason an attempted semantic activation was denied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationDeniedReason {
    /// The target visual or its semantic owner was disabled.
    Disabled,
}

/// Event delivered to widgets when a semantic activation attempt is rejected.
///
/// This is a toolkit-side affordance hook, not an app callback. Widgets use it
/// for local feedback such as shaking a disabled segmented-control item. It
/// does not re-enable the target and does not participate in raw routed input
/// dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationDeniedEvent {
    /// Element whose widget receives the denial hook.
    pub target: ElementId,
    /// Disabled element that caused the denial.
    pub disabled_target: ElementId,
    /// Input path that requested activation.
    pub source: ActivationSource,
    /// Why activation was denied.
    pub reason: ActivationDeniedReason,
}
