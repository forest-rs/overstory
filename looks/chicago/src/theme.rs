// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Assembling a [`Theme`] from Chicago token values.

use understory_style::{Theme, ThemeBuilder};

use crate::palette::{Seeds, TokenValues, seeds_for, token_values};
use crate::tokens::{
    ACCENT_BG, ACCENT_HOVER_BG, ACCENT_PRESSED_BG, CONTROL_BG, CONTROL_BORDER, CONTROL_HOVER_BG,
    CONTROL_HOVER_BORDER, FOCUS_RING_BORDER, MOTION_COLOR_INTERPOLATION,
    MOTION_EMPHASIZED_DURATION, MOTION_FAST_DURATION, MOTION_NORMAL_DURATION,
    MOTION_STANDARD_EASING, ON_ACCENT_FG, ON_SURFACE_FG, ON_SURFACE_MUTED_FG, PANEL_BG,
    ResolvedTokens, SLIDER_MAJOR_TICK_BG, SLIDER_MINOR_TICK_BG, SUCCESS_BG, SURFACE_BG, Scheme,
    TEXT_CARET_BG, TEXT_SELECTION_BG, THUMB_BG, TRACK_BG,
};

/// Builds the Chicago [`Theme`] for `scheme` using the default seeds.
#[must_use]
pub fn theme(scheme: Scheme, tokens: &ResolvedTokens) -> Theme {
    theme_from_seeds(scheme, &seeds_for(scheme), tokens)
}

/// Builds the Chicago [`Theme`] for `scheme` from explicit `seeds`.
#[must_use]
pub fn theme_from_seeds(scheme: Scheme, seeds: &Seeds, tokens: &ResolvedTokens) -> Theme {
    build(&token_values(scheme, seeds), tokens)
}

fn build(v: &TokenValues, tokens: &ResolvedTokens) -> Theme {
    let r = tokens.resources;
    ThemeBuilder::new()
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
        .set(ON_ACCENT_FG, v.on_accent_fg.clone())
        .set(ON_SURFACE_FG, v.on_surface_fg.clone())
        .set(ON_SURFACE_MUTED_FG, v.on_surface_muted_fg.clone())
        .set(r.bevel_light, v.bevel_light.clone())
        .set(r.bevel_dark, v.bevel_dark.clone())
        .set(MOTION_FAST_DURATION, v.motion_fast)
        .set(MOTION_NORMAL_DURATION, v.motion_normal)
        .set(MOTION_EMPHASIZED_DURATION, v.motion_emphasized)
        .set(MOTION_STANDARD_EASING, v.motion_standard_easing)
        .set(MOTION_COLOR_INTERPOLATION, v.motion_color_interpolation)
        .build()
}
