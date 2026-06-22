// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Signature interaction motion.
//!
//! These builders produce [`TransitionRules`] that the [`cascade`](crate::cascade)
//! layers onto interactive controls. Durations, easing, and color
//! interpolation are read from theme resources, so retuning motion is a theme
//! swap rather than a cascade rebuild.
//!
//! What animates smoothly today: solid border brushes (`OKLab` color, so the
//! focus ring fades in by recoloring a *constant-width* border — never by
//! growing it, which would reflow), foreground color, and opacity.
//!
//! Two things are deliberately not animated here:
//!
//! - **Border width / ring geometry.** Border width feeds layout (it folds into
//!   the box insets), so animating it reflows neighbors. A springy ring that
//!   expands without reflow needs a paint-only outline channel — see GAPS.md G7.
//! - **Gradient backgrounds.** Gradient↔gradient doesn't tween yet (GAPS.md G3),
//!   so hover/press feedback rides the ring color and opacity instead.

use overstory::BuiltInProperties;
use overstory::motion::{
    PresentationTarget, TimingFunction, TransitionRule, TransitionRules, TransitionSpec, millis,
};

use crate::tokens::{
    MOTION_COLOR_INTERPOLATION, MOTION_FAST_DURATION, MOTION_NORMAL_DURATION,
    MOTION_STANDARD_EASING, ResolvedTokens,
};

const FALLBACK_TIMING: TimingFunction = TimingFunction::cubic_bezier(0.22, 1.0, 0.36, 1.0);

/// A color transition for a property, resolving duration/easing/interpolation
/// from the standard motion theme tokens.
fn color_spec() -> TransitionSpec {
    TransitionSpec::timed(millis(140), FALLBACK_TIMING)
        .with_duration_resource(MOTION_NORMAL_DURATION)
        .with_easing_resource(MOTION_STANDARD_EASING)
        .with_color_interpolation_resource(MOTION_COLOR_INTERPOLATION)
}

/// A plain scalar transition (opacity), standard timing.
fn scalar_spec() -> TransitionSpec {
    TransitionSpec::timed(millis(90), FALLBACK_TIMING)
        .with_duration_resource(MOTION_FAST_DURATION)
        .with_easing_resource(MOTION_STANDARD_EASING)
}

/// The Signature interaction motion bundle for a bordered control: the four
/// border brushes cross-fade in `OKLab` (the focus ring recolors in place),
/// foreground eases, and opacity eases. Border *widths* are intentionally
/// absent so focus/hover never reflow — the ring width stays constant and only
/// its color changes (see the module docs and GAPS.md G7).
#[must_use]
pub fn interaction(properties: &BuiltInProperties, _tokens: &ResolvedTokens) -> TransitionRules {
    let brushes = properties.surface.border_brushes;
    let mut rules = TransitionRules::new();
    for brush in [brushes.top, brushes.right, brushes.bottom, brushes.left] {
        rules.push(TransitionRule::interaction(
            PresentationTarget::property(brush.id()),
            color_spec(),
        ));
    }
    rules.push(TransitionRule::interaction(
        PresentationTarget::property(properties.foreground.id()),
        color_spec(),
    ));
    rules.push(TransitionRule::interaction(
        PresentationTarget::property(properties.opacity.id()),
        scalar_spec(),
    ));
    rules
}
