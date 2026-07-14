// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The System 6 palette: ink, paper, and a couple of grays.
//!
//! A true 1-bit look needs only two colors — ink and paper. The two grays are
//! pragmatic concessions for things the original faked with 50% dither: the
//! desktop backdrop and disabled text. (A real stipple would need a pattern
//! brush, which the toolkit doesn't expose yet.) The text-selection wash is
//! derived from ink over paper via the canonical color interpolation API.

use overstory::motion::{ColorInterpolation, TimingFunction, millis};
use peniko::{Brush, Color, color::palette::css};
use understory_timing::TimerDuration;

use crate::tokens::Scheme;

/// The colors a System 6 theme is built from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seeds {
    /// Foreground ink.
    pub ink: Color,
    /// Surface paper.
    pub paper: Color,
    /// Desktop backdrop (a stand-in for the 50% gray stipple).
    pub desktop: Color,
    /// Muted/disabled text (a stand-in for dithered gray text).
    pub muted: Color,
}

/// The classic black-on-white desktop.
#[must_use]
pub fn classic_seeds() -> Seeds {
    Seeds {
        ink: css::BLACK,
        paper: css::WHITE,
        desktop: Color::from_rgb8(0xa0, 0xa0, 0xa0),
        muted: css::GRAY,
    }
}

/// An inverted "night" 1-bit desktop.
#[must_use]
pub fn dark_seeds() -> Seeds {
    Seeds {
        ink: css::WHITE,
        paper: Color::from_rgb8(0x10, 0x10, 0x10),
        desktop: Color::from_rgb8(0x30, 0x30, 0x30),
        muted: Color::from_rgb8(0x90, 0x90, 0x90),
    }
}

/// The default seeds for `scheme`.
#[must_use]
pub fn seeds_for(scheme: Scheme) -> Seeds {
    match scheme {
        Scheme::Light => classic_seeds(),
        Scheme::Dark => dark_seeds(),
    }
}

/// Every value a System 6 theme is built from.
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
    pub(crate) ink: Option<Brush>,
    pub(crate) paper: Option<Brush>,
    pub(crate) motion_fast: TimerDuration,
    pub(crate) motion_normal: TimerDuration,
    pub(crate) motion_emphasized: TimerDuration,
    pub(crate) motion_standard_easing: TimingFunction,
    pub(crate) motion_color_interpolation: ColorInterpolation,
}

const INSTANT: TimerDuration = millis(0);
const LINEAR: TimingFunction = TimingFunction::cubic_bezier(0.0, 0.0, 1.0, 1.0);

/// Builds the token values from `seeds`.
pub(crate) fn token_values(_scheme: Scheme, s: &Seeds) -> TokenValues {
    // A light wash so selected text stays legible without a text inversion.
    let selection = s.paper.lerp_rect(s.ink, 0.2);
    TokenValues {
        surface_bg: Some(Brush::Solid(s.desktop)),
        accent_bg: Some(Brush::Solid(s.ink)),
        accent_hover_bg: Some(Brush::Solid(s.ink)),
        accent_pressed_bg: Some(Brush::Solid(s.ink)),
        success_bg: Some(Brush::Solid(s.ink)),
        track_bg: Some(Brush::Solid(s.paper)),
        thumb_bg: Some(Brush::Solid(s.paper)),
        panel_bg: Some(Brush::Solid(s.paper)),
        control_bg: Some(Brush::Solid(s.paper)),
        control_border: Some(Brush::Solid(s.ink)),
        control_hover_border: Some(Brush::Solid(s.ink)),
        // A faint ink wash so hover reads as a row — not paper (which is near
        // black in the dark scheme, making hover look black).
        control_hover_bg: Some(Brush::Solid(s.paper.lerp_rect(s.ink, 0.14))),
        focus_ring_border: Some(Brush::Solid(s.ink)),
        text_caret_bg: Some(Brush::Solid(s.ink)),
        text_selection_bg: Some(Brush::Solid(selection)),
        slider_major_tick_bg: Some(Brush::Solid(s.ink)),
        slider_minor_tick_bg: Some(Brush::Solid(s.muted)),
        on_accent_fg: Brush::Solid(s.paper),
        on_surface_fg: Brush::Solid(s.ink),
        on_surface_muted_fg: Brush::Solid(s.muted),
        ink: Some(Brush::Solid(s.ink)),
        paper: Some(Brush::Solid(s.paper)),
        motion_fast: INSTANT,
        motion_normal: INSTANT,
        motion_emphasized: INSTANT,
        motion_standard_easing: LINEAR,
        motion_color_interpolation: ColorInterpolation::SRGB,
    }
}
