// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The System 6 palette: ink, paper, and a couple of grays.
//!
//! A true 1-bit look needs only two colors — ink and paper. The two grays are
//! pragmatic concessions for things the original faked with 50% dither: the
//! desktop backdrop and disabled text. (A real stipple would need a pattern
//! brush, which the toolkit doesn't expose yet.) The text-selection wash is
//! derived from ink over paper via the shared color math.

use overstory::motion::{ColorInterpolation, TimingFunction, millis};
use overstory_look::color::{mix, rgb};
use peniko::{Brush, Color};
use understory_timing::TimerDuration;

use crate::tokens::Scheme;

fn solid(color: Color) -> Option<Brush> {
    Some(Brush::Solid(color))
}

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
        ink: rgb(0x00, 0x00, 0x00),
        paper: rgb(0xff, 0xff, 0xff),
        desktop: rgb(0xa0, 0xa0, 0xa0),
        muted: rgb(0x80, 0x80, 0x80),
    }
}

/// An inverted "night" 1-bit desktop.
#[must_use]
pub fn dark_seeds() -> Seeds {
    Seeds {
        ink: rgb(0xff, 0xff, 0xff),
        paper: rgb(0x10, 0x10, 0x10),
        desktop: rgb(0x30, 0x30, 0x30),
        muted: rgb(0x90, 0x90, 0x90),
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
    let selection = mix(s.paper, s.ink, 0.2);
    TokenValues {
        surface_bg: solid(s.desktop),
        accent_bg: solid(s.ink),
        accent_hover_bg: solid(s.ink),
        accent_pressed_bg: solid(s.ink),
        success_bg: solid(s.ink),
        track_bg: solid(s.paper),
        thumb_bg: solid(s.paper),
        panel_bg: solid(s.paper),
        control_bg: solid(s.paper),
        control_border: solid(s.ink),
        control_hover_border: solid(s.ink),
        control_hover_bg: solid(s.paper),
        focus_ring_border: solid(s.ink),
        text_caret_bg: solid(s.ink),
        text_selection_bg: solid(selection),
        slider_major_tick_bg: solid(s.ink),
        slider_minor_tick_bg: solid(s.muted),
        on_accent_fg: Brush::Solid(s.paper),
        on_surface_fg: Brush::Solid(s.ink),
        on_surface_muted_fg: Brush::Solid(s.muted),
        ink: solid(s.ink),
        paper: solid(s.paper),
        motion_fast: INSTANT,
        motion_normal: INSTANT,
        motion_emphasized: INSTANT,
        motion_standard_easing: LINEAR,
        motion_color_interpolation: ColorInterpolation::SRGB,
    }
}
