// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Small sRGB color math for deriving palettes from seed colors.

use peniko::Color;

/// An opaque sRGB color from 8-bit channels.
#[must_use]
pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgba8(r, g, b, 0xff)
}

/// Straight-alpha linear blend between two sRGB colors (`t` = 0 → `a`, 1 → `b`).
#[must_use]
pub fn mix(a: Color, b: Color, t: f64) -> Color {
    let a = a.to_rgba8();
    let b = b.to_rgba8();
    Color::from_rgba8(
        blend(a.r, b.r, t),
        blend(a.g, b.g, t),
        blend(a.b, b.b, t),
        blend(a.a, b.a, t),
    )
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "blended channel is rounded and clamped to 0..=255 before truncation"
)]
fn blend(a: u8, b: u8, t: f64) -> u8 {
    let v = f64::from(a) + (f64::from(b) - f64::from(a)) * t;
    v.round().clamp(0.0, 255.0) as u8
}

/// Mixes `color` toward white by `t` (0 = unchanged, 1 = white).
#[must_use]
pub fn lighten(color: Color, t: f64) -> Color {
    mix(color, Color::WHITE, t)
}

/// Mixes `color` toward black by `t` (0 = unchanged, 1 = black).
#[must_use]
pub fn darken(color: Color, t: f64) -> Color {
    mix(color, Color::BLACK, t)
}

/// Returns `color` with a new straight-alpha byte.
#[must_use]
pub fn with_alpha(color: Color, alpha: u8) -> Color {
    let c = color.to_rgba8();
    Color::from_rgba8(c.r, c.g, c.b, alpha)
}
