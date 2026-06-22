// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The Chicago palette: flat system colors and two bevel edges.
//!
//! Unlike Signature, Chicago derives almost nothing — the period look is a
//! fixed set of system colors (the `#c0c0c0` face, white highlight, gray
//! shadow, navy selection) painted flat. There are no gradients, and motion is
//! effectively instant.

use overstory::motion::{ColorInterpolation, TimingFunction, millis};
use overstory_look::color::{lighten, rgb};
use peniko::{Brush, Color};
use understory_timing::TimerDuration;

use crate::tokens::Scheme;

fn solid(color: Color) -> Option<Brush> {
    Some(Brush::Solid(color))
}

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
        face: rgb(0xc0, 0xc0, 0xc0),
        window: rgb(0xff, 0xff, 0xff),
        bevel_light: rgb(0xff, 0xff, 0xff),
        bevel_dark: rgb(0x80, 0x80, 0x80),
        accent: rgb(0x00, 0x00, 0x80),
        text: rgb(0x00, 0x00, 0x00),
        muted: rgb(0x80, 0x80, 0x80),
        on_accent: rgb(0xff, 0xff, 0xff),
    }
}

/// A darker chrome variant.
#[must_use]
pub fn dark_chrome_seeds() -> Seeds {
    Seeds {
        face: rgb(0x3a, 0x3a, 0x3a),
        window: rgb(0x1e, 0x1e, 0x1e),
        bevel_light: rgb(0x6a, 0x6a, 0x6a),
        bevel_dark: rgb(0x00, 0x00, 0x00),
        // Navy is illegible on dark chrome; use a brighter selection blue.
        accent: rgb(0x5a, 0x7f, 0xe6),
        text: rgb(0xf0, 0xf0, 0xf0),
        muted: rgb(0xa0, 0xa0, 0xa0),
        on_accent: rgb(0xff, 0xff, 0xff),
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
    pub(crate) motion_color_interpolation: ColorInterpolation,
}

/// Chicago motion is effectively instant — period UI did not animate.
const INSTANT: TimerDuration = millis(0);
const LINEAR: TimingFunction = TimingFunction::cubic_bezier(0.0, 0.0, 1.0, 1.0);

/// Builds the flat token values from `seeds`.
pub(crate) fn token_values(_scheme: Scheme, s: &Seeds) -> TokenValues {
    TokenValues {
        surface_bg: solid(s.face),
        accent_bg: solid(s.accent),
        accent_hover_bg: solid(s.accent),
        accent_pressed_bg: solid(s.accent),
        success_bg: solid(s.accent),
        track_bg: solid(s.face),
        thumb_bg: solid(s.face),
        panel_bg: solid(s.face),
        control_bg: solid(s.window),
        control_border: solid(s.bevel_dark),
        control_hover_border: solid(s.bevel_dark),
        // A lighter face so hover reads as a distinct row, not the panel itself.
        control_hover_bg: solid(lighten(s.face, 0.12)),
        focus_ring_border: solid(s.text),
        text_caret_bg: solid(s.text),
        text_selection_bg: solid(s.accent),
        slider_major_tick_bg: solid(s.text),
        slider_minor_tick_bg: solid(s.muted),
        on_accent_fg: Brush::Solid(s.on_accent),
        on_surface_fg: Brush::Solid(s.text),
        on_surface_muted_fg: Brush::Solid(s.muted),
        bevel_light: solid(s.bevel_light),
        bevel_dark: solid(s.bevel_dark),
        motion_fast: INSTANT,
        motion_normal: INSTANT,
        motion_emphasized: INSTANT,
        motion_standard_easing: LINEAR,
        motion_color_interpolation: ColorInterpolation::SRGB,
    }
}
