// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Named design tokens + a default Overstory palette.
//!
//! Apps don't have to use this module — Overstory works fine with
//! literal brushes in every style rule — but the indirection a
//! [`Theme`] gives you (define the palette in one place, swap it
//! out wholesale via [`crate::Ui::set_theme`]) is the cleanest way
//! to support light / dark / branded variants without rewriting
//! every cascade rule.
//!
//! The constants here are [`ResourceKey`] handles into the theme's
//! resource table. A style rule references one with
//! [`understory_style::StyleBuilder::set_resource`]:
//!
//! ```ignore
//! use overstory::theme;
//! use understory_style::StyleBuilder;
//!
//! let style = StyleBuilder::new()
//!     .set_resource(ui.properties().background, theme::ACCENT_BG)
//!     .build();
//! ```
//!
//! The cascade resolves the resource through whatever
//! [`Theme`](understory_style::Theme) the `Ui` currently holds, so
//! swapping `ui.set_theme(...)` changes every button on the page at
//! once.
//!
//! ## Token taxonomy
//!
//! The starter set is intentionally small. Each token has one role
//! and a single value type. Background-flavored tokens carry an
//! `Option<Brush>` to match the `Background` / `BorderBrush`
//! properties; foreground tokens carry a `Brush` to match
//! `Foreground`. Apps with richer palettes should layer additional
//! tokens on top.

use peniko::{Brush, Color};
use understory_style::{ResourceKey, Theme, ThemeBuilder};

/// Window-wide background surface.
pub const SURFACE_BG: ResourceKey = ResourceKey::new(0);
/// Primary action color (e.g. Button idle background).
pub const ACCENT_BG: ResourceKey = ResourceKey::new(1);
/// Primary action, hovered.
pub const ACCENT_HOVER_BG: ResourceKey = ResourceKey::new(2);
/// Primary action, pressed.
pub const ACCENT_PRESSED_BG: ResourceKey = ResourceKey::new(3);
/// "On" / success state (e.g. `ToggleSwitch` when `:checked`).
pub const SUCCESS_BG: ResourceKey = ResourceKey::new(4);
/// Neutral track (e.g. `ToggleSwitch` when unchecked).
pub const TRACK_BG: ResourceKey = ResourceKey::new(5);
/// Inner thumb / handle (e.g. `ToggleSwitch` thumb, `Slider` thumb).
pub const THUMB_BG: ResourceKey = ResourceKey::new(6);
/// Foreground color used on top of [`ACCENT_BG`] (`Button` labels).
pub const ON_ACCENT_FG: ResourceKey = ResourceKey::new(7);
/// Muted foreground for secondary text (captions, helper labels).
pub const ON_SURFACE_MUTED_FG: ResourceKey = ResourceKey::new(8);
/// Focus ring border brush.
pub const FOCUS_RING_BORDER: ResourceKey = ResourceKey::new(9);

/// Builds the default Overstory theme — a single dark palette that
/// matches the demo's pre-token look. Apps can call
/// [`crate::Ui::set_theme`] with this directly, or use it as a
/// starting point for their own [`ThemeBuilder`] chains.
#[must_use]
pub fn default_theme() -> Theme {
    ThemeBuilder::new()
        .set(SURFACE_BG, opt_solid(0x1e, 0x1e, 0x24, 0xff))
        .set(ACCENT_BG, opt_solid(0x3a, 0x69, 0xd6, 0xff))
        .set(ACCENT_HOVER_BG, opt_solid(0x4d, 0x81, 0xf2, 0xff))
        .set(ACCENT_PRESSED_BG, opt_solid(0x29, 0x4d, 0xa3, 0xff))
        .set(SUCCESS_BG, opt_solid(0x3a, 0xa3, 0x6d, 0xff))
        .set(TRACK_BG, opt_solid(0x44, 0x44, 0x4a, 0xff))
        .set(THUMB_BG, opt_solid(0xff, 0xff, 0xff, 0xff))
        .set(ON_ACCENT_FG, solid(0xff, 0xff, 0xff, 0xff))
        .set(ON_SURFACE_MUTED_FG, solid(0xc8, 0xc8, 0xd2, 0xff))
        .set(FOCUS_RING_BORDER, opt_solid(0xff, 0xff, 0xff, 0xff))
        .build()
}

fn solid(r: u8, g: u8, b: u8, a: u8) -> Brush {
    Brush::Solid(Color::from_rgba8(r, g, b, a))
}

fn opt_solid(r: u8, g: u8, b: u8, a: u8) -> Option<Brush> {
    Some(solid(r, g, b, a))
}
