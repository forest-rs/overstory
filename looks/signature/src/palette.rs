// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The Signature palette: a small set of seed colors, plus derived gradients
//! and shades.
//!
//! The design philosophy is "few seeds, derived ramps". A [`Seeds`] value names
//! a dozen base colors and four motion durations; everything else — the subtle
//! surface gradients, the accent sheen, hover/pressed shades, the focus glow,
//! the hairline highlight — is *computed* from those seeds. That keeps the
//! palette coherent and avoids hand-tuning forty separate hex literals.
//!
//! All gradients are authored in the unit square `[0, 1]²`; the surface painter
//! scales them to each element's bounds (see GAPS.md G1).

use overstory::motion::{ColorInterpolation, TimingFunction, millis};
use overstory_look::color::{darken, lighten, rgb, with_alpha};
use peniko::{Brush, Color, Gradient};
use understory_timing::TimerDuration;

use crate::tokens::Scheme;

// --- Gradient builders (unit space) -----------------------------------------

/// A top-to-bottom gradient (the Signature default sheen direction).
fn vertical(top: Color, bottom: Color) -> Brush {
    Brush::Gradient(
        Gradient::new_linear((0.0, 0.0), (0.0, 1.0))
            .with_stops([(0.0_f32, top), (1.0_f32, bottom)]),
    )
}

/// A top-left-to-bottom-right gradient, used for accent surfaces so the sheen
/// reads as lit from the upper left.
fn diagonal(start: Color, end: Color) -> Brush {
    Brush::Gradient(
        Gradient::new_linear((0.0, 0.0), (1.0, 1.0)).with_stops([(0.0_f32, start), (1.0_f32, end)]),
    )
}

// --- Seeds ------------------------------------------------------------------

/// The seed colors and motion durations a Signature theme is derived from.
///
/// Override a field and call [`token_values`] (via the crate's theme builder) to
/// re-derive the whole palette. `DESIGN.md` is the Markdown form of this struct.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seeds {
    /// Primary action color; the accent gradient and focus glow derive from it.
    pub accent: Color,
    /// Window backdrop base.
    pub surface: Color,
    /// Raised panel base.
    pub panel: Color,
    /// Card base (a gentler raise than a panel).
    pub card: Color,
    /// Input/control surface base.
    pub control: Color,
    /// Default control border.
    pub control_border: Color,
    /// Recessed track base (slider/toggle/progress backing).
    pub track: Color,
    /// Thumb/handle base.
    pub thumb: Color,
    /// Foreground on top of the accent.
    pub on_accent: Color,
    /// Primary foreground on ordinary surfaces.
    pub on_surface: Color,
    /// Muted/secondary foreground.
    pub muted: Color,
    /// Heading foreground.
    pub heading: Color,
    /// Body-copy foreground.
    pub body: Color,
    /// Focus color; the focus ring and glow derive from it.
    pub focus: Color,
    /// Fast interaction duration, in milliseconds.
    pub motion_fast_ms: u64,
    /// Default interaction duration, in milliseconds.
    pub motion_normal_ms: u64,
    /// Emphasized motion duration, in milliseconds.
    pub motion_emphasized_ms: u64,
    /// Slow glow/elevation duration, in milliseconds.
    pub motion_glow_ms: u64,
}

/// The default dark-scheme seeds.
#[must_use]
pub fn dark_seeds() -> Seeds {
    Seeds {
        accent: rgb(0x63, 0x66, 0xf1),
        surface: rgb(0x18, 0x1a, 0x22),
        panel: rgb(0x1c, 0x1e, 0x27),
        card: rgb(0x22, 0x25, 0x30),
        control: rgb(0x1f, 0x22, 0x30),
        control_border: rgb(0x3a, 0x3e, 0x4d),
        track: rgb(0x2a, 0x2d, 0x3a),
        thumb: rgb(0xf6, 0xf7, 0xfb),
        on_accent: rgb(0xff, 0xff, 0xff),
        on_surface: rgb(0xec, 0xee, 0xf6),
        muted: rgb(0xa3, 0xa8, 0xbb),
        heading: rgb(0xf5, 0xf6, 0xfc),
        body: rgb(0xcd, 0xd0, 0xdc),
        focus: rgb(0xaa, 0xb2, 0xff),
        motion_fast_ms: 90,
        motion_normal_ms: 140,
        motion_emphasized_ms: 220,
        motion_glow_ms: 420,
    }
}

/// The default light-scheme seeds.
#[must_use]
pub fn light_seeds() -> Seeds {
    Seeds {
        accent: rgb(0x5b, 0x5e, 0xf0),
        surface: rgb(0xee, 0xf0, 0xf7),
        panel: rgb(0xff, 0xff, 0xff),
        card: rgb(0xff, 0xff, 0xff),
        control: rgb(0xf6, 0xf7, 0xfb),
        control_border: rgb(0xc9, 0xce, 0xdd),
        track: rgb(0xdf, 0xe3, 0xee),
        thumb: rgb(0xff, 0xff, 0xff),
        on_accent: rgb(0xff, 0xff, 0xff),
        on_surface: rgb(0x1b, 0x1d, 0x27),
        muted: rgb(0x5d, 0x63, 0x7a),
        heading: rgb(0x14, 0x16, 0x1f),
        body: rgb(0x3a, 0x3f, 0x51),
        focus: rgb(0x4b, 0x51, 0xe6),
        motion_fast_ms: 90,
        motion_normal_ms: 140,
        motion_emphasized_ms: 220,
        motion_glow_ms: 420,
    }
}

/// The default seeds for `scheme`.
#[must_use]
pub fn seeds_for(scheme: Scheme) -> Seeds {
    match scheme {
        Scheme::Dark => dark_seeds(),
        Scheme::Light => light_seeds(),
    }
}

// --- Derived token values ---------------------------------------------------

/// Every value a Signature [`Theme`](understory_style::Theme) is built from,
/// derived from a [`Seeds`]. Built-in palette roles carry gradients so the
/// framework controls re-skin; extended roles cover cards, the hairline, the
/// glow, and chrome typography.
#[derive(Clone, Debug)]
pub(crate) struct TokenValues {
    // Built-in roles (Option<Brush>): surfaces and borders.
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
    // Built-in roles (Brush): foregrounds.
    pub(crate) on_accent_fg: Brush,
    pub(crate) on_surface_fg: Brush,
    pub(crate) on_surface_muted_fg: Brush,
    // Extended roles.
    pub(crate) card_gradient: Option<Brush>,
    pub(crate) card_border: Option<Brush>,
    pub(crate) hairline: Option<Brush>,
    pub(crate) focus_glow: Option<Brush>,
    pub(crate) heading_fg: Brush,
    pub(crate) body_fg: Brush,
    // Motion.
    pub(crate) motion_fast: TimerDuration,
    pub(crate) motion_normal: TimerDuration,
    pub(crate) motion_emphasized: TimerDuration,
    pub(crate) motion_glow: TimerDuration,
    pub(crate) motion_standard_easing: TimingFunction,
    pub(crate) motion_overshoot_easing: TimingFunction,
    pub(crate) motion_color_interpolation: ColorInterpolation,
}

/// Snappy ease-out (fast start settle) for most interaction motion.
const STANDARD_EASING: TimingFunction = TimingFunction::cubic_bezier(0.22, 1.0, 0.36, 1.0);
/// Back-out overshoot for lively press/hover feedback.
const OVERSHOOT_EASING: TimingFunction = TimingFunction::cubic_bezier(0.34, 1.56, 0.64, 1.0);

/// Derives the full token value set from `seeds`.
pub(crate) fn token_values(_scheme: Scheme, s: &Seeds) -> TokenValues {
    TokenValues {
        surface_bg: Some(vertical(lighten(s.surface, 0.03), darken(s.surface, 0.02))),
        accent_bg: Some(diagonal(lighten(s.accent, 0.12), darken(s.accent, 0.04))),
        accent_hover_bg: Some(diagonal(lighten(s.accent, 0.20), lighten(s.accent, 0.02))),
        accent_pressed_bg: Some(diagonal(darken(s.accent, 0.02), darken(s.accent, 0.14))),
        success_bg: Some(diagonal(lighten(s.accent, 0.12), darken(s.accent, 0.04))),
        track_bg: Some(vertical(darken(s.track, 0.05), lighten(s.track, 0.03))),
        thumb_bg: Some(vertical(lighten(s.thumb, 0.0), darken(s.thumb, 0.06))),
        panel_bg: Some(vertical(lighten(s.panel, 0.04), s.panel)),
        control_bg: Some(vertical(lighten(s.control, 0.05), darken(s.control, 0.02))),
        control_border: Some(Brush::Solid(s.control_border)),
        control_hover_border: Some(Brush::Solid(lighten(s.control_border, 0.18))),
        control_hover_bg: Some(vertical(lighten(s.control, 0.10), lighten(s.control, 0.02))),
        focus_ring_border: Some(Brush::Solid(s.focus)),
        text_caret_bg: Some(Brush::Solid(s.heading)),
        text_selection_bg: Some(Brush::Solid(with_alpha(s.accent, 0x59))),
        slider_major_tick_bg: Some(Brush::Solid(s.muted)),
        slider_minor_tick_bg: Some(Brush::Solid(with_alpha(s.muted, 0x88))),
        on_accent_fg: Brush::Solid(s.on_accent),
        on_surface_fg: Brush::Solid(s.on_surface),
        on_surface_muted_fg: Brush::Solid(s.muted),
        card_gradient: Some(vertical(lighten(s.card, 0.05), s.card)),
        card_border: Some(Brush::Solid(lighten(s.control_border, 0.04))),
        hairline: Some(Brush::Solid(with_alpha(Color::WHITE, 0x14))),
        focus_glow: Some(Brush::Solid(with_alpha(lighten(s.focus, 0.10), 0x66))),
        heading_fg: Brush::Solid(s.heading),
        body_fg: Brush::Solid(s.body),
        motion_fast: millis(s.motion_fast_ms),
        motion_normal: millis(s.motion_normal_ms),
        motion_emphasized: millis(s.motion_emphasized_ms),
        motion_glow: millis(s.motion_glow_ms),
        motion_standard_easing: STANDARD_EASING,
        motion_overshoot_easing: OVERSHOOT_EASING,
        motion_color_interpolation: ColorInterpolation::OKLAB,
    }
}
