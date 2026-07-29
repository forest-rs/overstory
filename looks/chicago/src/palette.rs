// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The Chicago palette: flat system colors and two bevel edges.
//!
//! Unlike Signature, Chicago derives almost nothing — the period look is a
//! fixed set of system colors (the `#c0c0c0` face, white highlight, gray
//! shadow, navy selection) painted flat. There are no gradients, and motion is
//! effectively instant.

use overstory::motion::{TimingFunction, millis};
use peniko::color::{ColorSpaceTag, HueDirection, palette::css};
use peniko::{Brush, Color, InterpolationAlphaSpace};
use understory_timing::TimerDuration;

use crate::tokens::Scheme;

/// The flat system colors a Chicago theme is built from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seeds {
    /// Control/dialog face.
    pub face: Color,
    /// Editable surface (text fields, lists).
    pub window: Color,
    /// Raised bevel highlight (top/left).
    pub bevel_light: Color,
    /// Raised bevel shadow (bottom/right).
    pub bevel_dark: Color,
    /// Selection / progress / "on" color.
    pub accent: Color,
    /// Primary text.
    pub text: Color,
    /// Muted/disabled text.
    pub muted: Color,
    /// Text on top of the accent.
    pub on_accent: Color,
}

/// The classic light-gray chrome.
#[must_use]
pub fn classic_seeds() -> Seeds {
    Seeds {
        face: css::SILVER,
        window: css::WHITE,
        bevel_light: css::WHITE,
        bevel_dark: css::GRAY,
        accent: css::NAVY,
        text: css::BLACK,
        muted: css::GRAY,
        on_accent: css::WHITE,
    }
}

/// A darker chrome variant.
#[must_use]
pub fn dark_chrome_seeds() -> Seeds {
    Seeds {
        face: Color::from_rgb8(0x3a, 0x3a, 0x3a),
        window: Color::from_rgb8(0x1e, 0x1e, 0x1e),
        bevel_light: Color::from_rgb8(0x6a, 0x6a, 0x6a),
        bevel_dark: css::BLACK,
        // Navy is illegible on dark chrome; use a brighter selection blue.
        accent: Color::from_rgb8(0x5a, 0x7f, 0xe6),
        text: Color::from_rgb8(0xf0, 0xf0, 0xf0),
        muted: Color::from_rgb8(0xa0, 0xa0, 0xa0),
        on_accent: css::WHITE,
    }
}

/// The default seeds for `scheme`.
#[must_use]
pub fn seeds_for(scheme: Scheme) -> Seeds {
    match scheme {
        Scheme::Light => classic_seeds(),
        Scheme::Dark => dark_chrome_seeds(),
    }
}

/// Every value a Chicago theme is built from.
#[derive(Clone, Debug)]
pub(crate) struct TokenValues {
    pub(crate) surface_bg: Option<Brush>,
    pub(crate) accent_bg: Option<Brush>,
    pub(crate) accent_hover_bg: Option<Brush>,
    pub(crate) accent_pressed_bg: Option<Brush>,
    pub(crate) success_bg: Option<Brush>,
    pub(crate) track_bg: Option<Brush>,
    pub(crate) thumb_bg: Option<Brush>,
    pub(crate) panel_bg: Option<Brush>,
    pub(crate) control_bg: Option<Brush>,
    pub(crate) control_border: Option<Brush>,
    pub(crate) control_hover_border: Option<Brush>,
    pub(crate) control_hover_bg: Option<Brush>,
    pub(crate) focus_ring_border: Option<Brush>,
    pub(crate) text_caret_bg: Option<Brush>,
    pub(crate) text_selection_bg: Option<Brush>,
    pub(crate) slider_major_tick_bg: Option<Brush>,
    pub(crate) slider_minor_tick_bg: Option<Brush>,
    pub(crate) on_accent_fg: Brush,
    pub(crate) on_surface_fg: Brush,
    pub(crate) on_surface_muted_fg: Brush,
    pub(crate) bevel_light: Option<Brush>,
    pub(crate) bevel_dark: Option<Brush>,
    pub(crate) motion_fast: TimerDuration,
    pub(crate) motion_normal: TimerDuration,
    pub(crate) motion_emphasized: TimerDuration,
    pub(crate) motion_standard_easing: TimingFunction,
    pub(crate) motion_color_space: ColorSpaceTag,
    pub(crate) motion_hue_direction: HueDirection,
    pub(crate) motion_alpha_space: InterpolationAlphaSpace,
}

/// Chicago motion is effectively instant — period UI did not animate.
const INSTANT: TimerDuration = millis(0);
const LINEAR: TimingFunction = TimingFunction::cubic_bezier(0.0, 0.0, 1.0, 1.0);

/// Builds the flat token values from `seeds`.
pub(crate) fn token_values(_scheme: Scheme, s: &Seeds) -> TokenValues {
    TokenValues {
        surface_bg: Some(Brush::Solid(s.face)),
        accent_bg: Some(Brush::Solid(s.accent)),
        accent_hover_bg: Some(Brush::Solid(s.accent)),
        accent_pressed_bg: Some(Brush::Solid(s.accent)),
        success_bg: Some(Brush::Solid(s.accent)),
        track_bg: Some(Brush::Solid(s.face)),
        thumb_bg: Some(Brush::Solid(s.face)),
        panel_bg: Some(Brush::Solid(s.face)),
        control_bg: Some(Brush::Solid(s.window)),
        control_border: Some(Brush::Solid(s.bevel_dark)),
        control_hover_border: Some(Brush::Solid(s.bevel_dark)),
        // A lighter face so hover reads as a distinct row, not the panel itself.
        control_hover_bg: Some(Brush::Solid(s.face.lerp_rect(Color::WHITE, 0.12))),
        focus_ring_border: Some(Brush::Solid(s.text)),
        text_caret_bg: Some(Brush::Solid(s.text)),
        text_selection_bg: Some(Brush::Solid(s.accent)),
        slider_major_tick_bg: Some(Brush::Solid(s.text)),
        slider_minor_tick_bg: Some(Brush::Solid(s.muted)),
        on_accent_fg: Brush::Solid(s.on_accent),
        on_surface_fg: Brush::Solid(s.text),
        on_surface_muted_fg: Brush::Solid(s.muted),
        bevel_light: Some(Brush::Solid(s.bevel_light)),
        bevel_dark: Some(Brush::Solid(s.bevel_dark)),
        motion_fast: INSTANT,
        motion_normal: INSTANT,
        motion_emphasized: INSTANT,
        motion_standard_easing: LINEAR,
        motion_color_space: ColorSpaceTag::Srgb,
        motion_hue_direction: HueDirection::Shorter,
        motion_alpha_space: InterpolationAlphaSpace::Premultiplied,
    }
}
