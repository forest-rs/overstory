// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Assembling a [`Theme`] from Signature token values.
//!
//! Built-in palette roles (`overstory::theme::*`) are set so the framework
//! control cascade re-skins automatically; extended roles are set on the
//! vocabulary-allocated keys carried in [`ResolvedTokens`].

use understory_style::{Theme, ThemeBuilder};

use crate::palette::{Seeds, TokenValues, seeds_for, token_values};
use crate::tokens::{
    ACCENT_BG, ACCENT_HOVER_BG, ACCENT_PRESSED_BG, CONTROL_BG, CONTROL_BORDER, CONTROL_HOVER_BG,
    CONTROL_HOVER_BORDER, FOCUS_RING_BORDER, MOTION_ALPHA_SPACE, MOTION_COLOR_SPACE,
    MOTION_EMPHASIZED_DURATION, MOTION_FAST_DURATION, MOTION_HUE_DIRECTION, MOTION_NORMAL_DURATION,
    MOTION_STANDARD_EASING, ON_ACCENT_FG, ON_SURFACE_FG, ON_SURFACE_MUTED_FG, PANEL_BG,
    ResolvedTokens, SLIDER_MAJOR_TICK_BG, SLIDER_MINOR_TICK_BG, SUCCESS_BG, SURFACE_BG, Scheme,
    TEXT_CARET_BG, TEXT_SELECTION_BG, THUMB_BG, TRACK_BG,
};

/// Builds the Signature [`Theme`] for `scheme` using the default seeds.
///
/// `tokens` must come from `ui.style_tokens::<SignatureTokens>()` on the same
/// `Ui` the theme will be installed on, so the extended resource keys line up.
#[must_use]
pub fn theme(scheme: Scheme, tokens: &ResolvedTokens) -> Theme {
    theme_from_seeds(scheme, &seeds_for(scheme), tokens)
}

/// Builds the Signature [`Theme`] for `scheme` from explicit `seeds`.
#[must_use]
pub fn theme_from_seeds(scheme: Scheme, seeds: &Seeds, tokens: &ResolvedTokens) -> Theme {
    build(&token_values(scheme, seeds), tokens)
}

fn build(v: &TokenValues, tokens: &ResolvedTokens) -> Theme {
    let r = tokens.resources;
    ThemeBuilder::new()
        // Built-in surface/border roles (Option<Brush>).
        .set(SURFACE_BG, v.surface_bg.clone())
        .set(ACCENT_BG, v.accent_bg.clone())
        .set(ACCENT_HOVER_BG, v.accent_hover_bg.clone())
        .set(ACCENT_PRESSED_BG, v.accent_pressed_bg.clone())
        .set(SUCCESS_BG, v.success_bg.clone())
        .set(TRACK_BG, v.track_bg.clone())
        .set(THUMB_BG, v.thumb_bg.clone())
        .set(PANEL_BG, v.panel_bg.clone())
        .set(CONTROL_BG, v.control_bg.clone())
        .set(CONTROL_BORDER, v.control_border.clone())
        .set(CONTROL_HOVER_BORDER, v.control_hover_border.clone())
        .set(CONTROL_HOVER_BG, v.control_hover_bg.clone())
        .set(FOCUS_RING_BORDER, v.focus_ring_border.clone())
        .set(TEXT_CARET_BG, v.text_caret_bg.clone())
        .set(TEXT_SELECTION_BG, v.text_selection_bg.clone())
        .set(SLIDER_MAJOR_TICK_BG, v.slider_major_tick_bg.clone())
        .set(SLIDER_MINOR_TICK_BG, v.slider_minor_tick_bg.clone())
        // Built-in foreground roles (Brush).
        .set(ON_ACCENT_FG, v.on_accent_fg.clone())
        .set(ON_SURFACE_FG, v.on_surface_fg.clone())
        .set(ON_SURFACE_MUTED_FG, v.on_surface_muted_fg.clone())
        // Extended roles.
        .set(r.card_gradient, v.card_gradient.clone())
        .set(r.card_border, v.card_border.clone())
        .set(r.hairline, v.hairline.clone())
        .set(r.focus_glow, v.focus_glow.clone())
        .set(r.heading_fg, v.heading_fg.clone())
        .set(r.body_fg, v.body_fg.clone())
        // Motion roles.
        .set(MOTION_FAST_DURATION, v.motion_fast)
        .set(MOTION_NORMAL_DURATION, v.motion_normal)
        .set(MOTION_EMPHASIZED_DURATION, v.motion_emphasized)
        .set(r.motion_glow_duration, v.motion_glow)
        .set(MOTION_STANDARD_EASING, v.motion_standard_easing)
        .set(r.motion_overshoot_easing, v.motion_overshoot_easing)
        .set(MOTION_COLOR_SPACE, v.motion_color_space)
        .set(MOTION_HUE_DIRECTION, v.motion_hue_direction)
        .set(MOTION_ALPHA_SPACE, v.motion_alpha_space)
        .build()
}
