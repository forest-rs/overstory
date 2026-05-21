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
use understory_style::{ResourceKey, StyleBuilder, StyleCascade, StyleCascadeBuilder, StyleOrigin};
use understory_style::{Theme, ThemeBuilder};

use crate::controls::button::TYPE_BUTTON;
use crate::controls::slider::{
    CLASS_SLIDER_MAJOR_TICK, CLASS_SLIDER_MINOR_TICK, PART_SLIDER_THUMB, PART_SLIDER_TICK,
    PART_SLIDER_TICK_LABEL, PART_SLIDER_TRACK, TYPE_SLIDER,
};
use crate::controls::toggle::{PART_TOGGLE_THUMB, TYPE_TOGGLE};
use crate::layouts::Layout;
use crate::properties::BuiltInProperties;
use crate::pseudos::{
    PSEUDO_CHECKED, PSEUDO_DISABLED, PSEUDO_FOCUSED, PSEUDO_HOVER, PSEUDO_PRESSED,
};
use crate::selectors;
use crate::widget::TYPE_GENERIC;

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

/// Builds Overstory's default control stylesheet for the built-in controls.
///
/// The stylesheet uses control `TypeTag`s for owners and owner-local
/// `PartTag`s for generated template parts. Apps can use it directly, layer
/// additional rules after it, or replace it entirely.
#[must_use]
pub fn default_cascade(properties: &BuiltInProperties) -> StyleCascade {
    let backdrop = StyleBuilder::new()
        .set_resource(properties.background, SURFACE_BG)
        .build();

    let button_idle = StyleBuilder::new()
        .set_resource(properties.background, ACCENT_BG)
        .set_resource(properties.foreground, ON_ACCENT_FG)
        .set(properties.corner_radius, 10.0_f64)
        .set(properties.layout, button_layout())
        .build();
    let button_hover = StyleBuilder::new()
        .set_resource(properties.background, ACCENT_HOVER_BG)
        .build();
    let button_pressed = StyleBuilder::new()
        .set_resource(properties.background, ACCENT_PRESSED_BG)
        .build();
    let button_focused = StyleBuilder::new()
        .set_resource(properties.border_brush, FOCUS_RING_BORDER)
        .set(properties.border_width, 2.0_f64)
        .build();
    let button_disabled = StyleBuilder::new()
        .set_resource(properties.background, TRACK_BG)
        .set_resource(properties.foreground, ON_SURFACE_MUTED_FG)
        .build();

    let toggle_layout = |checked: bool| {
        let layout = Layout::row(0.0)
            .align_center()
            .logical_size(64.0, 32.0)
            .padding_all(4.0);
        if checked {
            layout.justify_end()
        } else {
            layout.justify_start()
        }
    };
    let toggle_idle = StyleBuilder::new()
        .set_resource(properties.background, TRACK_BG)
        .set(properties.corner_radius, 16.0_f64)
        .set(properties.layout, toggle_layout(false))
        .build();
    let toggle_checked = StyleBuilder::new()
        .set_resource(properties.background, SUCCESS_BG)
        .set(properties.layout, toggle_layout(true))
        .build();
    let toggle_focused = StyleBuilder::new()
        .set_resource(properties.border_brush, FOCUS_RING_BORDER)
        .set(properties.border_width, 2.0_f64)
        .build();
    let thumb_style = StyleBuilder::new()
        .set_resource(properties.background, THUMB_BG)
        .set(properties.corner_radius, 12.0_f64)
        .set(properties.layout, Layout::new().logical_size(24.0, 24.0))
        .build();

    let slider_track_style = StyleBuilder::new()
        .set_resource(properties.background, TRACK_BG)
        .set(properties.corner_radius, 3.0_f64)
        .build();
    let slider_thumb_style = StyleBuilder::new()
        .set_resource(properties.background, THUMB_BG)
        .set(properties.corner_radius, 11.0_f64)
        .build();
    let slider_major_tick_style = StyleBuilder::new()
        .set(properties.background, opt_solid(0xc8, 0xc8, 0xd2, 0xff))
        .set(properties.corner_radius, 1.0_f64)
        .build();
    let slider_minor_tick_style = StyleBuilder::new()
        .set(properties.background, opt_solid(0x78, 0x78, 0x84, 0xff))
        .set(properties.corner_radius, 1.0_f64)
        .build();
    let slider_major_tick_label_style = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_MUTED_FG)
        .set(properties.font_size, 10.0_f64)
        .build();
    let slider_minor_tick_label_style = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_MUTED_FG)
        .set(properties.font_size, 8.0_f64)
        .build();
    let slider_focused_style = StyleBuilder::new()
        .set_resource(properties.border_brush, FOCUS_RING_BORDER)
        .set(properties.border_width, 2.0_f64)
        .set(properties.corner_radius, 4.0_f64)
        .build();

    StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            selectors::type_tag(TYPE_GENERIC),
            backdrop,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::type_tag(TYPE_BUTTON),
            button_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_BUTTON, PSEUDO_FOCUSED),
            button_focused,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_BUTTON, PSEUDO_HOVER),
            button_hover,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_BUTTON, PSEUDO_PRESSED),
            button_pressed,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_BUTTON, PSEUDO_DISABLED),
            button_disabled,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::type_tag(TYPE_TOGGLE),
            toggle_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_TOGGLE, PSEUDO_CHECKED),
            toggle_checked,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_TOGGLE, PSEUDO_FOCUSED),
            toggle_focused,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(TYPE_TOGGLE, PART_TOGGLE_THUMB),
            thumb_style,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(TYPE_SLIDER, PART_SLIDER_TRACK),
            slider_track_style,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_with_class(TYPE_SLIDER, PART_SLIDER_TICK, CLASS_SLIDER_MAJOR_TICK),
            slider_major_tick_style,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_with_class(TYPE_SLIDER, PART_SLIDER_TICK, CLASS_SLIDER_MINOR_TICK),
            slider_minor_tick_style,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_with_class(
                TYPE_SLIDER,
                PART_SLIDER_TICK_LABEL,
                CLASS_SLIDER_MAJOR_TICK,
            ),
            slider_major_tick_label_style,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_with_class(
                TYPE_SLIDER,
                PART_SLIDER_TICK_LABEL,
                CLASS_SLIDER_MINOR_TICK,
            ),
            slider_minor_tick_label_style,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(TYPE_SLIDER, PART_SLIDER_THUMB),
            slider_thumb_style,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_SLIDER, PSEUDO_FOCUSED),
            slider_focused_style,
        )
        .build()
}

fn button_layout() -> Layout {
    Layout::row(0.0)
        .padding_xy(32.0, 18.0)
        .justify_center()
        .align_center()
}

fn solid(r: u8, g: u8, b: u8, a: u8) -> Brush {
    Color::from_rgba8(r, g, b, a).into()
}

fn opt_solid(r: u8, g: u8, b: u8, a: u8) -> Option<Brush> {
    Some(solid(r, g, b, a))
}
