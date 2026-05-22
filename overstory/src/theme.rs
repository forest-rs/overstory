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
use understory_style::{
    ResourceKey, Selector, SelectorStep, StyleBuilder, StyleCascade, StyleCascadeBuilder,
    StyleOrigin,
};
use understory_style::{Theme, ThemeBuilder};

use crate::controls::button::TYPE_BUTTON;
use crate::controls::checkbox::{
    PART_CHECK_BOX_BOX, PART_CHECK_BOX_LABEL, PART_CHECK_BOX_MARK, TYPE_CHECK_BOX,
};
use crate::controls::divider::TYPE_DIVIDER;
use crate::controls::list::{TYPE_LIST_BOX, TYPE_LIST_ITEM};
use crate::controls::progress::{
    PART_PROGRESS_BAR_FILL, PART_PROGRESS_BAR_LABEL, TYPE_PROGRESS_BAR,
};
use crate::controls::slider::{
    CLASS_SLIDER_MAJOR_TICK, CLASS_SLIDER_MINOR_TICK, PART_SLIDER_THUMB, PART_SLIDER_TICK,
    PART_SLIDER_TICK_LABEL, PART_SLIDER_TRACK, TYPE_SLIDER,
};
use crate::controls::splitter::TYPE_SPLITTER;
use crate::controls::text_input::{
    CLASS_TEXT_INPUT_PLACEHOLDER, PART_TEXT_INPUT_CARET, PART_TEXT_INPUT_SELECTION,
    PART_TEXT_INPUT_TEXT, TYPE_TEXT_INPUT,
};
use crate::controls::toggle::{PART_TOGGLE_THUMB, TYPE_TOGGLE};
use crate::layouts::Layout;
use crate::properties::BuiltInProperties;
use crate::pseudos::{
    PSEUDO_CHECKED, PSEUDO_DISABLED, PSEUDO_FOCUS_VISIBLE, PSEUDO_FOCUSED, PSEUDO_HOVER,
    PSEUDO_PRESSED, PSEUDO_READ_ONLY, PSEUDO_SELECTED,
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
/// Primary foreground used on ordinary dark surfaces.
pub const ON_SURFACE_FG: ResourceKey = ResourceKey::new(10);
/// Raised panel surface.
pub const PANEL_BG: ResourceKey = ResourceKey::new(11);
/// Input/control surface.
pub const CONTROL_BG: ResourceKey = ResourceKey::new(12);
/// Default control border.
pub const CONTROL_BORDER: ResourceKey = ResourceKey::new(13);
/// Hovered control border.
pub const CONTROL_HOVER_BORDER: ResourceKey = ResourceKey::new(14);

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
        .set(ON_SURFACE_FG, solid(0xf4, 0xf4, 0xf7, 0xff))
        .set(PANEL_BG, opt_solid(0x28, 0x28, 0x31, 0xff))
        .set(CONTROL_BG, opt_solid(0x2c, 0x2c, 0x35, 0xff))
        .set(CONTROL_BORDER, opt_solid(0x62, 0x62, 0x70, 0xff))
        .set(CONTROL_HOVER_BORDER, opt_solid(0x86, 0x91, 0xaa, 0xff))
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

    let splitter_idle = StyleBuilder::new()
        .set_resource(properties.background, CONTROL_BORDER)
        .set(properties.corner_radius, 4.0_f64)
        .build();
    let splitter_hover = StyleBuilder::new()
        .set_resource(properties.background, CONTROL_HOVER_BORDER)
        .build();
    let splitter_pressed = StyleBuilder::new()
        .set_resource(properties.background, ACCENT_BG)
        .build();
    let splitter_focused = StyleBuilder::new()
        .set_resource(properties.border_brush, FOCUS_RING_BORDER)
        .set(properties.border_width, 2.0_f64)
        .build();

    let divider_style = StyleBuilder::new()
        .set_resource(properties.background, CONTROL_BORDER)
        .set(properties.corner_radius, 1.0_f64)
        .build();

    let checkbox_idle = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .set(properties.layout, Layout::row(10.0).align_center())
        .build();
    let checkbox_disabled = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_MUTED_FG)
        .build();
    let checkbox_box_idle = StyleBuilder::new()
        .set_resource(properties.background, CONTROL_BG)
        .set_resource(properties.border_brush, CONTROL_BORDER)
        .set(properties.border_width, 1.0_f64)
        .set(properties.corner_radius, 5.0_f64)
        .set(
            properties.layout,
            Layout::row(0.0)
                .justify_center()
                .align_center()
                .logical_size(22.0, 22.0),
        )
        .build();
    let checkbox_box_hover = StyleBuilder::new()
        .set_resource(properties.border_brush, CONTROL_HOVER_BORDER)
        .build();
    let checkbox_box_checked = StyleBuilder::new()
        .set_resource(properties.background, SUCCESS_BG)
        .set_resource(properties.border_brush, SUCCESS_BG)
        .build();
    let checkbox_box_focused = StyleBuilder::new()
        .set_resource(properties.border_brush, FOCUS_RING_BORDER)
        .set(properties.border_width, 2.0_f64)
        .build();
    let checkbox_mark_idle = StyleBuilder::new()
        .set(properties.foreground, solid(0, 0, 0, 0))
        .set(properties.font_size, 15.0_f64)
        .build();
    let checkbox_mark_checked = StyleBuilder::new()
        .set_resource(properties.foreground, ON_ACCENT_FG)
        .build();
    let checkbox_label = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .build();

    let progress_idle = StyleBuilder::new()
        .set_resource(properties.background, TRACK_BG)
        .set_resource(properties.border_brush, CONTROL_BORDER)
        .set(properties.border_width, 1.0_f64)
        .set(properties.corner_radius, 9.0_f64)
        .build();
    let progress_fill = StyleBuilder::new()
        .set_resource(properties.background, ACCENT_BG)
        .set(properties.corner_radius, 8.0_f64)
        .build();
    let progress_label = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .set(properties.font_size, 11.0_f64)
        .build();

    let text_input_idle = StyleBuilder::new()
        .set_resource(properties.background, CONTROL_BG)
        .set_resource(properties.border_brush, CONTROL_BORDER)
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .set(properties.border_width, 1.0_f64)
        .set(properties.corner_radius, 8.0_f64)
        .build();
    let text_input_hover = StyleBuilder::new()
        .set_resource(properties.border_brush, CONTROL_HOVER_BORDER)
        .build();
    let text_input_focused = StyleBuilder::new()
        .set_resource(properties.border_brush, FOCUS_RING_BORDER)
        .set(properties.border_width, 2.0_f64)
        .build();
    let text_input_disabled = StyleBuilder::new()
        .set_resource(properties.background, TRACK_BG)
        .set_resource(properties.foreground, ON_SURFACE_MUTED_FG)
        .build();
    let text_input_read_only = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_MUTED_FG)
        .build();
    let text_input_placeholder = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_MUTED_FG)
        .build();
    let text_input_caret = StyleBuilder::new()
        .set(properties.background, opt_solid(0xf4, 0xf4, 0xf7, 0xff))
        .set(properties.corner_radius, 0.75_f64)
        .build();
    let text_input_selection = StyleBuilder::new()
        .set(properties.background, opt_solid(0x4d, 0x81, 0xf2, 0x66))
        .set(properties.corner_radius, 3.0_f64)
        .build();

    let list_box_idle = StyleBuilder::new()
        .set_resource(properties.background, CONTROL_BG)
        .set_resource(properties.border_brush, CONTROL_BORDER)
        .set(properties.border_width, 1.0_f64)
        .set(properties.corner_radius, 8.0_f64)
        .build();
    let list_box_focused = StyleBuilder::new()
        .set_resource(properties.border_brush, FOCUS_RING_BORDER)
        .set(properties.border_width, 2.0_f64)
        .build();
    let list_item_idle = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .set(
            properties.layout,
            Layout::row(0.0)
                .align_center()
                .align_self_stretch()
                .padding_xy(10.0, 7.0),
        )
        .set(properties.corner_radius, 6.0_f64)
        .build();
    let list_item_hover = StyleBuilder::new()
        .set(properties.background, opt_solid(0x36, 0x36, 0x42, 0xff))
        .build();
    let list_item_selected = StyleBuilder::new()
        .set_resource(properties.background, ACCENT_BG)
        .set_resource(properties.foreground, ON_ACCENT_FG)
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
            selectors::state(TYPE_BUTTON, PSEUDO_FOCUS_VISIBLE),
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
            selectors::state(TYPE_TOGGLE, PSEUDO_FOCUS_VISIBLE),
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
            selectors::state(TYPE_SLIDER, PSEUDO_FOCUS_VISIBLE),
            slider_focused_style,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::type_tag(TYPE_SPLITTER),
            splitter_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_SPLITTER, PSEUDO_HOVER),
            splitter_hover,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_SPLITTER, PSEUDO_PRESSED),
            splitter_pressed,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_SPLITTER, PSEUDO_FOCUS_VISIBLE),
            splitter_focused,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::type_tag(TYPE_DIVIDER),
            divider_style,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::type_tag(TYPE_CHECK_BOX),
            checkbox_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_CHECK_BOX, PSEUDO_DISABLED),
            checkbox_disabled,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(TYPE_CHECK_BOX, PART_CHECK_BOX_BOX),
            checkbox_box_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_when(
                SelectorStep::type_tag(TYPE_CHECK_BOX).with_pseudo(PSEUDO_HOVER),
                PART_CHECK_BOX_BOX,
            ),
            checkbox_box_hover,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_when(
                SelectorStep::type_tag(TYPE_CHECK_BOX).with_pseudo(PSEUDO_CHECKED),
                PART_CHECK_BOX_BOX,
            ),
            checkbox_box_checked,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_when(
                SelectorStep::type_tag(TYPE_CHECK_BOX).with_pseudo(PSEUDO_FOCUS_VISIBLE),
                PART_CHECK_BOX_BOX,
            ),
            checkbox_box_focused,
        )
        .push_rule(
            StyleOrigin::Base,
            nested_check_mark_selector(false),
            checkbox_mark_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            nested_check_mark_selector(true),
            checkbox_mark_checked,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(TYPE_CHECK_BOX, PART_CHECK_BOX_LABEL),
            checkbox_label,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::type_tag(TYPE_PROGRESS_BAR),
            progress_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(TYPE_PROGRESS_BAR, PART_PROGRESS_BAR_FILL),
            progress_fill,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(TYPE_PROGRESS_BAR, PART_PROGRESS_BAR_LABEL),
            progress_label,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::type_tag(TYPE_TEXT_INPUT),
            text_input_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_TEXT_INPUT, PSEUDO_HOVER),
            text_input_hover,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_TEXT_INPUT, PSEUDO_FOCUS_VISIBLE),
            text_input_focused,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_TEXT_INPUT, PSEUDO_DISABLED),
            text_input_disabled,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_TEXT_INPUT, PSEUDO_READ_ONLY),
            text_input_read_only,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_with_class(
                TYPE_TEXT_INPUT,
                PART_TEXT_INPUT_TEXT,
                CLASS_TEXT_INPUT_PLACEHOLDER,
            ),
            text_input_placeholder,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_when(
                SelectorStep::type_tag(TYPE_TEXT_INPUT).with_pseudo(PSEUDO_FOCUSED),
                PART_TEXT_INPUT_SELECTION,
            ),
            text_input_selection,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_when(
                SelectorStep::type_tag(TYPE_TEXT_INPUT).with_pseudo(PSEUDO_FOCUSED),
                PART_TEXT_INPUT_CARET,
            ),
            text_input_caret,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::type_tag(TYPE_LIST_BOX),
            list_box_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_LIST_BOX, PSEUDO_FOCUS_VISIBLE),
            list_box_focused,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::type_tag(TYPE_LIST_ITEM),
            list_item_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_LIST_ITEM, PSEUDO_HOVER),
            list_item_hover,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_LIST_ITEM, PSEUDO_SELECTED),
            list_item_selected,
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

fn nested_check_mark_selector(checked: bool) -> Selector {
    let owner = if checked {
        SelectorStep::type_tag(TYPE_CHECK_BOX).with_pseudo(PSEUDO_CHECKED)
    } else {
        SelectorStep::type_tag(TYPE_CHECK_BOX)
    };
    Selector::from([
        owner,
        SelectorStep::part_tag(PART_CHECK_BOX_BOX),
        SelectorStep::part_tag(PART_CHECK_BOX_MARK),
    ])
}
