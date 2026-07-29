// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Signature interaction motion.
//!
//! These builders produce transition declarations that the
//! [`cascade`](crate::cascade) layers onto interactive controls. Durations, easing, and color
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
use overstory::motion::{MotionSpec, Transitions};

use crate::tokens::{
    MOTION_ALPHA_SPACE, MOTION_COLOR_SPACE, MOTION_FAST_DURATION, MOTION_HUE_DIRECTION,
    MOTION_NORMAL_DURATION, MOTION_STANDARD_EASING,
};

/// Color motion resolved from the standard theme tokens.
const fn color_motion() -> MotionSpec {
    MotionSpec::from_duration_resource(MOTION_NORMAL_DURATION)
        .with_easing_resource(MOTION_STANDARD_EASING)
        .with_color_space_resource(MOTION_COLOR_SPACE)
        .with_hue_direction_resource(MOTION_HUE_DIRECTION)
        .with_alpha_space_resource(MOTION_ALPHA_SPACE)
}

/// Scalar motion resolved from the standard theme tokens.
const fn scalar_motion() -> MotionSpec {
    MotionSpec::from_duration_resource(MOTION_FAST_DURATION)
        .with_easing_resource(MOTION_STANDARD_EASING)
}

/// The Signature interaction motion bundle for a bordered control: the four
/// border brushes cross-fade in `OKLab` (the focus ring recolors in place),
/// foreground eases, and opacity eases. Border *widths* are intentionally
/// absent so focus/hover never reflow — the ring width stays constant and only
/// its color changes (see the module docs and GAPS.md G7).
#[must_use]
pub fn interaction(properties: &BuiltInProperties) -> Transitions {
    let brushes = properties.surface.border_brushes;
    let mut transitions = Transitions::new();
    for brush in [brushes.top, brushes.right, brushes.bottom, brushes.left] {
        transitions = transitions.color(brush, color_motion());
    }
    transitions
        .color(properties.foreground, color_motion())
        .property(properties.opacity, scalar_motion())
}
