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
//! use overstory::SurfaceStyleExt;
//! use understory_style::StyleBuilder;
//!
//! let style = StyleBuilder::new()
//!     .background_resource(ui.properties(), theme::ACCENT_BG)
//!     .build();
//! ```
//!
//! The cascade resolves the resource through whatever
//! [`Theme`] the `Ui` currently holds, so
//! swapping `ui.set_theme(...)` changes every button on the page at
//! once.
//!
//! ## Token taxonomy
//!
//! The starter set is intentionally small. Each token has one role
//! and a single value type. Surface background and border-brush tokens carry
//! an `Option<Brush>` to match the canonical `Surface.*` brush longhands;
//! foreground tokens carry a `Brush` to match `Foreground`. Motion tokens
//! carry [`understory_timing::TimerDuration`], easing curves as
//! [`crate::motion::TimingFunction`], or
//! [`crate::motion::ColorInterpolation`]. Apps with richer palettes or motion
//! systems should layer additional tokens on top.
//!
//! Roadmap:
//! - Keep the built-in theme small: it should demonstrate token roles and
//!   control states, not become a full design system.
//! - Add tokens only when multiple controls need the same semantic color,
//!   spacing, radius, or typography role.
//! - Make light/dark/high-contrast and branded variants expressible as theme
//!   swaps rather than control rebuilds.
//! - Keep accessibility contrast and disabled/focus/selection states visible in
//!   the default theme so examples expose styling gaps early.

use peniko::{Brush, Color};
use understory_style::{
    ResourceKey, Selector, SelectorStep, StyleBuilder, StyleCascade, StyleCascadeBuilder,
    StyleOrigin,
};
use understory_style::{Theme, ThemeBuilder};

use crate::controls::accordion::{
    PART_ACCORDION_SECTION_CONTENT, PART_ACCORDION_SECTION_DISCLOSURE,
    PART_ACCORDION_SECTION_HEADER, PART_ACCORDION_SECTION_LABEL, TYPE_ACCORDION,
    TYPE_ACCORDION_SECTION, default_accordion_section_template,
};
use crate::controls::button::{TYPE_BUTTON, default_button_template};
use crate::controls::checkbox::{
    PART_CHECK_BOX_BOX, PART_CHECK_BOX_LABEL, PART_CHECK_BOX_MARK, TYPE_CHECK_BOX,
    default_check_box_template,
};
use crate::controls::divider::TYPE_DIVIDER;
use crate::controls::expander::{
    PART_EXPANDER_CONTENT, PART_EXPANDER_DISCLOSURE, PART_EXPANDER_HEADER, PART_EXPANDER_LABEL,
    TYPE_EXPANDER, default_expander_template,
};
use crate::controls::list::{
    PART_LIST_ITEM_CONTENT, TYPE_LIST_BOX, TYPE_LIST_ITEM, default_list_item_template,
};
use crate::controls::outline::{
    PART_OUTLINE_ITEM_CONTENT, PART_OUTLINE_ITEM_DISCLOSURE, PART_OUTLINE_ITEM_LABEL,
    TYPE_OUTLINE_ITEM, TYPE_OUTLINE_VIEW, default_outline_item_template,
};
use crate::controls::popup::TYPE_POPUP;
use crate::controls::progress::{
    PART_PROGRESS_BAR_FILL, PART_PROGRESS_BAR_LABEL, TYPE_PROGRESS_BAR, default_progress_template,
};
use crate::controls::radio::{
    PART_RADIO_BUTTON_DOT, PART_RADIO_BUTTON_INDICATOR, PART_RADIO_BUTTON_LABEL, TYPE_RADIO_BUTTON,
    default_radio_button_template,
};
use crate::controls::scroll::{
    PART_SCROLL_BAR_THUMB, TYPE_SCROLL_BAR, default_scroll_bar_template,
};
use crate::controls::segmented::{
    PART_SEGMENTED_SELECTION_INDICATOR, TYPE_SEGMENTED_CONTROL, TYPE_SEGMENTED_ITEM,
    default_segmented_item_template,
};
use crate::controls::select::{
    PART_SELECT_INDICATOR, PART_SELECT_LABEL, TYPE_SELECT, default_select_template,
};
use crate::controls::slider::{
    CLASS_SLIDER_MAJOR_TICK, CLASS_SLIDER_MINOR_TICK, PART_SLIDER_THUMB, PART_SLIDER_TICK,
    PART_SLIDER_TICK_LABEL, PART_SLIDER_TRACK, TYPE_SLIDER, default_slider_template,
};
use crate::controls::splitter::TYPE_SPLITTER;
use crate::controls::text_input::{
    PART_TEXT_INPUT_CARET, PART_TEXT_INPUT_SELECTION, PART_TEXT_INPUT_TEXT, TYPE_TEXT_INPUT,
    default_text_input_template,
};
use crate::controls::toggle::{PART_TOGGLE_THUMB, TYPE_TOGGLE, default_toggle_template};
use crate::controls::tooltip::{TYPE_TOOLTIP, default_tooltip_template};
use crate::layouts::{
    LayoutAlign, LayoutAlignment, LayoutContainer, LayoutDimension, LayoutJustify, LayoutSize,
    LayoutSpacing,
};
use crate::motion::{
    ColorInterpolation, TimingFunction, TransitionRule, TransitionRules, TransitionSpec,
    TransitionTarget, millis,
};
use crate::properties::BuiltInProperties;
use crate::pseudos::{
    PSEUDO_CHECKED, PSEUDO_DISABLED, PSEUDO_EXPANDED, PSEUDO_FOCUS_VISIBLE, PSEUDO_FOCUSED,
    PSEUDO_HOVER, PSEUDO_PLACEHOLDER_SHOWN, PSEUDO_PRESSED, PSEUDO_READ_ONLY, PSEUDO_SELECTED,
};
use crate::selectors;
use crate::selectors::TemplatePartSelector;
use crate::spec_ext::SurfaceStyleExt;
use understory_timing::TimerDuration;

const TOGGLE_THUMB_PART: TemplatePartSelector =
    selectors::template_part(TYPE_TOGGLE, PART_TOGGLE_THUMB);
const SLIDER_TRACK_PART: TemplatePartSelector =
    selectors::template_part(TYPE_SLIDER, PART_SLIDER_TRACK);
const SLIDER_THUMB_PART: TemplatePartSelector =
    selectors::template_part(TYPE_SLIDER, PART_SLIDER_THUMB);
const SLIDER_TICK_PART: TemplatePartSelector =
    selectors::template_part(TYPE_SLIDER, PART_SLIDER_TICK);
const SLIDER_TICK_LABEL_PART: TemplatePartSelector =
    selectors::template_part(TYPE_SLIDER, PART_SLIDER_TICK_LABEL);
const SCROLL_BAR_THUMB_PART: TemplatePartSelector =
    selectors::template_part(TYPE_SCROLL_BAR, PART_SCROLL_BAR_THUMB);
const CHECK_BOX_BOX_PART: TemplatePartSelector =
    selectors::template_part(TYPE_CHECK_BOX, PART_CHECK_BOX_BOX);
const CHECK_BOX_LABEL_PART: TemplatePartSelector =
    selectors::template_part(TYPE_CHECK_BOX, PART_CHECK_BOX_LABEL);
const RADIO_BUTTON_INDICATOR_PART: TemplatePartSelector =
    selectors::template_part(TYPE_RADIO_BUTTON, PART_RADIO_BUTTON_INDICATOR);
const RADIO_BUTTON_LABEL_PART: TemplatePartSelector =
    selectors::template_part(TYPE_RADIO_BUTTON, PART_RADIO_BUTTON_LABEL);
const EXPANDER_HEADER_PART: TemplatePartSelector =
    selectors::template_part(TYPE_EXPANDER, PART_EXPANDER_HEADER);
const EXPANDER_DISCLOSURE_PART: TemplatePartSelector =
    selectors::template_part(TYPE_EXPANDER, PART_EXPANDER_DISCLOSURE);
const EXPANDER_LABEL_PART: TemplatePartSelector =
    selectors::template_part(TYPE_EXPANDER, PART_EXPANDER_LABEL);
const EXPANDER_CONTENT_PART: TemplatePartSelector =
    selectors::template_part(TYPE_EXPANDER, PART_EXPANDER_CONTENT);
const ACCORDION_SECTION_HEADER_PART: TemplatePartSelector =
    selectors::template_part(TYPE_ACCORDION_SECTION, PART_ACCORDION_SECTION_HEADER);
const ACCORDION_SECTION_DISCLOSURE_PART: TemplatePartSelector =
    selectors::template_part(TYPE_ACCORDION_SECTION, PART_ACCORDION_SECTION_DISCLOSURE);
const ACCORDION_SECTION_LABEL_PART: TemplatePartSelector =
    selectors::template_part(TYPE_ACCORDION_SECTION, PART_ACCORDION_SECTION_LABEL);
const ACCORDION_SECTION_CONTENT_PART: TemplatePartSelector =
    selectors::template_part(TYPE_ACCORDION_SECTION, PART_ACCORDION_SECTION_CONTENT);
const PROGRESS_BAR_FILL_PART: TemplatePartSelector =
    selectors::template_part(TYPE_PROGRESS_BAR, PART_PROGRESS_BAR_FILL);
const PROGRESS_BAR_LABEL_PART: TemplatePartSelector =
    selectors::template_part(TYPE_PROGRESS_BAR, PART_PROGRESS_BAR_LABEL);
const TEXT_INPUT_TEXT_PART: TemplatePartSelector =
    selectors::template_part(TYPE_TEXT_INPUT, PART_TEXT_INPUT_TEXT);
const TEXT_INPUT_SELECTION_PART: TemplatePartSelector =
    selectors::template_part(TYPE_TEXT_INPUT, PART_TEXT_INPUT_SELECTION);
const TEXT_INPUT_CARET_PART: TemplatePartSelector =
    selectors::template_part(TYPE_TEXT_INPUT, PART_TEXT_INPUT_CARET);
const SELECT_LABEL_PART: TemplatePartSelector =
    selectors::template_part(TYPE_SELECT, PART_SELECT_LABEL);
const SELECT_INDICATOR_PART: TemplatePartSelector =
    selectors::template_part(TYPE_SELECT, PART_SELECT_INDICATOR);
const LIST_ITEM_CONTENT_PART: TemplatePartSelector =
    selectors::template_part(TYPE_LIST_ITEM, PART_LIST_ITEM_CONTENT);
const OUTLINE_ITEM_CONTENT_PART: TemplatePartSelector =
    selectors::template_part(TYPE_OUTLINE_ITEM, PART_OUTLINE_ITEM_CONTENT);
const OUTLINE_ITEM_DISCLOSURE_PART: TemplatePartSelector =
    selectors::template_part(TYPE_OUTLINE_ITEM, PART_OUTLINE_ITEM_DISCLOSURE);
const OUTLINE_ITEM_LABEL_PART: TemplatePartSelector =
    selectors::template_part(TYPE_OUTLINE_ITEM, PART_OUTLINE_ITEM_LABEL);

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
/// Hovered control surface.
pub const CONTROL_HOVER_BG: ResourceKey = ResourceKey::new(15);
/// Text caret brush.
pub const TEXT_CARET_BG: ResourceKey = ResourceKey::new(16);
/// Text selection brush.
pub const TEXT_SELECTION_BG: ResourceKey = ResourceKey::new(17);
/// Slider major tick brush.
pub const SLIDER_MAJOR_TICK_BG: ResourceKey = ResourceKey::new(18);
/// Slider minor tick brush.
pub const SLIDER_MINOR_TICK_BG: ResourceKey = ResourceKey::new(19);
/// Fast interaction motion duration.
pub const MOTION_FAST_DURATION: ResourceKey = ResourceKey::new(20);
/// Default interaction motion duration.
pub const MOTION_NORMAL_DURATION: ResourceKey = ResourceKey::new(21);
/// Emphasized motion duration for larger visual movement.
pub const MOTION_EMPHASIZED_DURATION: ResourceKey = ResourceKey::new(22);
/// Default easing curve for interaction motion.
pub const MOTION_STANDARD_EASING: ResourceKey = ResourceKey::new(23);
/// Default color interpolation policy for color transitions.
pub const MOTION_COLOR_INTERPOLATION: ResourceKey = ResourceKey::new(24);

const DEFAULT_MOTION_FAST_DURATION: TimerDuration = millis(70);
const DEFAULT_MOTION_NORMAL_DURATION: TimerDuration = millis(90);
const DEFAULT_MOTION_EMPHASIZED_DURATION: TimerDuration = millis(120);
const DEFAULT_MOTION_STANDARD_EASING: TimingFunction =
    TimingFunction::cubic_bezier(0.215, 0.61, 0.355, 1.0);

/// Builds the default Overstory theme.
///
/// This currently aliases [`dark_theme`].
#[must_use]
pub fn default_theme() -> Theme {
    dark_theme()
}

/// Builds Overstory's default dark palette.
#[must_use]
pub fn dark_theme() -> Theme {
    dark_theme_builder().build()
}

/// Builds a [`ThemeBuilder`] preloaded with Overstory's default dark palette.
///
/// Apps can use this to add app-specific resource keys while preserving the
/// built-in control tokens:
///
/// ```ignore
/// let theme = overstory::theme::dark_theme_builder()
///     .set(APP_CARD_BG, my_card_brush)
///     .build();
/// ```
#[must_use]
pub fn dark_theme_builder() -> ThemeBuilder {
    finish_motion_tokens(
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
            .set(CONTROL_HOVER_BG, opt_solid(0x36, 0x36, 0x42, 0xff))
            .set(TEXT_CARET_BG, opt_solid(0xf4, 0xf4, 0xf7, 0xff))
            .set(TEXT_SELECTION_BG, opt_solid(0x4d, 0x81, 0xf2, 0x66))
            .set(SLIDER_MAJOR_TICK_BG, opt_solid(0xc8, 0xc8, 0xd2, 0xff))
            .set(SLIDER_MINOR_TICK_BG, opt_solid(0x78, 0x78, 0x84, 0xff)),
    )
}

/// Builds Overstory's default light palette.
#[must_use]
pub fn light_theme() -> Theme {
    light_theme_builder().build()
}

/// Builds a [`ThemeBuilder`] preloaded with Overstory's default light palette.
///
/// Apps can use this to add app-specific resource keys while preserving the
/// built-in control tokens.
#[must_use]
pub fn light_theme_builder() -> ThemeBuilder {
    finish_motion_tokens(
        ThemeBuilder::new()
            .set(SURFACE_BG, opt_solid(0xf3, 0xf5, 0xfa, 0xff))
            .set(ACCENT_BG, opt_solid(0x2f, 0x62, 0xd6, 0xff))
            .set(ACCENT_HOVER_BG, opt_solid(0x46, 0x76, 0xe7, 0xff))
            .set(ACCENT_PRESSED_BG, opt_solid(0x24, 0x4a, 0xa8, 0xff))
            .set(SUCCESS_BG, opt_solid(0x2f, 0x9b, 0x6b, 0xff))
            .set(TRACK_BG, opt_solid(0xd9, 0xdd, 0xe7, 0xff))
            .set(THUMB_BG, opt_solid(0xff, 0xff, 0xff, 0xff))
            .set(ON_ACCENT_FG, solid(0xff, 0xff, 0xff, 0xff))
            .set(ON_SURFACE_MUTED_FG, solid(0x61, 0x68, 0x78, 0xff))
            .set(FOCUS_RING_BORDER, opt_solid(0x1e, 0x53, 0xc8, 0xff))
            .set(ON_SURFACE_FG, solid(0x16, 0x1a, 0x24, 0xff))
            .set(PANEL_BG, opt_solid(0xff, 0xff, 0xff, 0xff))
            .set(CONTROL_BG, opt_solid(0xf9, 0xfa, 0xfd, 0xff))
            .set(CONTROL_BORDER, opt_solid(0xb5, 0xbb, 0xc9, 0xff))
            .set(CONTROL_HOVER_BORDER, opt_solid(0x76, 0x82, 0x98, 0xff))
            .set(CONTROL_HOVER_BG, opt_solid(0xe8, 0xec, 0xf5, 0xff))
            .set(TEXT_CARET_BG, opt_solid(0x16, 0x1a, 0x24, 0xff))
            .set(TEXT_SELECTION_BG, opt_solid(0x2f, 0x62, 0xd6, 0x44))
            .set(SLIDER_MAJOR_TICK_BG, opt_solid(0x61, 0x68, 0x78, 0xff))
            .set(SLIDER_MINOR_TICK_BG, opt_solid(0xa2, 0xa9, 0xb7, 0xff)),
    )
}

fn finish_motion_tokens(builder: ThemeBuilder) -> ThemeBuilder {
    builder
        .set(MOTION_FAST_DURATION, DEFAULT_MOTION_FAST_DURATION)
        .set(MOTION_NORMAL_DURATION, DEFAULT_MOTION_NORMAL_DURATION)
        .set(
            MOTION_EMPHASIZED_DURATION,
            DEFAULT_MOTION_EMPHASIZED_DURATION,
        )
        .set(MOTION_STANDARD_EASING, DEFAULT_MOTION_STANDARD_EASING)
        .set(MOTION_COLOR_INTERPOLATION, ColorInterpolation::SRGB)
}

/// Builds Overstory's default control stylesheet for the built-in controls.
///
/// The stylesheet uses control `TypeTag`s for owners and owner-local
/// `PartTag`s for generated template parts. [`Ui`](crate::Ui) installs this as
/// its framework cascade so controls receive built-in templates through the
/// same `ControlTemplate` property path that app styles use to replace a
/// control's visual tree.
///
/// App code normally calls [`Ui::set_cascade`](crate::Ui::set_cascade) with
/// app-specific rules instead of installing this cascade manually. Use
/// [`default_cascade_builder`] when tests or framework-like code need to build
/// a custom base stylesheet from the built-in rules.
#[must_use]
pub fn default_cascade(properties: &BuiltInProperties) -> StyleCascade {
    default_cascade_builder(properties).build()
}

/// Builds a mutable default control stylesheet builder.
///
/// This is primarily for tests and framework-like code that need a custom base
/// stylesheet. App-authored rules should usually live in the app cascade passed
/// to [`Ui::set_cascade`](crate::Ui::set_cascade), where they layer after these
/// defaults automatically.
#[must_use]
pub fn default_cascade_builder(properties: &BuiltInProperties) -> StyleCascadeBuilder {
    let button_idle = StyleBuilder::new()
        .set(properties.control_template, Some(default_button_template()))
        .background_resource(properties, ACCENT_BG)
        .set_resource(properties.foreground, ON_ACCENT_FG)
        .set(properties.transitions, button_transition_rules(properties))
        .set(properties.font_size, 22.0_f64)
        .corner_radius(properties, 10.0_f64)
        .set(properties.layout_container, LayoutContainer::row())
        .padding_xy(properties, 32.0, 18.0)
        .set(properties.layout_alignment, centered_alignment())
        .build();
    let button_hover = StyleBuilder::new()
        .background_resource(properties, ACCENT_HOVER_BG)
        .build();
    let button_pressed = StyleBuilder::new()
        .background_resource(properties, ACCENT_PRESSED_BG)
        .set(properties.opacity, 0.92_f32)
        .build();
    let button_focused = StyleBuilder::new()
        .border_brush_resource(properties, FOCUS_RING_BORDER)
        .border_width(properties, 2.0_f64)
        .build();
    let button_disabled = StyleBuilder::new()
        .background_resource(properties, TRACK_BG)
        .set_resource(properties.foreground, ON_SURFACE_MUTED_FG)
        .build();

    let toggle_idle = StyleBuilder::new()
        .set(properties.control_template, Some(default_toggle_template()))
        .background_resource(properties, TRACK_BG)
        .set(properties.transitions, surface_transition_rules(properties))
        .corner_radius(properties, 16.0_f64)
        .set(properties.layout_container, LayoutContainer::row())
        .set(properties.layout_size, LayoutSize::logical(64.0, 32.0))
        .padding_all(properties, 4.0)
        .set(properties.layout_alignment, toggle_alignment(false))
        .build();
    let toggle_checked = StyleBuilder::new()
        .background_resource(properties, SUCCESS_BG)
        .set(properties.layout_alignment, toggle_alignment(true))
        .build();
    let toggle_focused = StyleBuilder::new()
        .border_brush_resource(properties, FOCUS_RING_BORDER)
        .border_width(properties, 2.0_f64)
        .build();
    let thumb_style = StyleBuilder::new()
        .background_resource(properties, THUMB_BG)
        .set(
            properties.transitions,
            layer_offset_transition_rules(
                DEFAULT_MOTION_EMPHASIZED_DURATION,
                MOTION_EMPHASIZED_DURATION,
            ),
        )
        .corner_radius(properties, 12.0_f64)
        .set(properties.layout_size, LayoutSize::logical(24.0, 24.0))
        .build();

    let slider_idle = StyleBuilder::new()
        .set(properties.control_template, Some(default_slider_template()))
        .set(
            properties.transitions,
            border_affordance_transition_rules(properties),
        )
        .build();
    let slider_track_style = StyleBuilder::new()
        .background_resource(properties, TRACK_BG)
        .corner_radius(properties, 3.0_f64)
        .build();
    let slider_thumb_style = StyleBuilder::new()
        .background_resource(properties, THUMB_BG)
        .set(
            properties.transitions,
            layer_offset_transition_rules(DEFAULT_MOTION_NORMAL_DURATION, MOTION_NORMAL_DURATION),
        )
        .corner_radius(properties, 11.0_f64)
        .build();
    let slider_major_tick_style = StyleBuilder::new()
        .background_resource(properties, SLIDER_MAJOR_TICK_BG)
        .corner_radius(properties, 1.0_f64)
        .build();
    let slider_minor_tick_style = StyleBuilder::new()
        .background_resource(properties, SLIDER_MINOR_TICK_BG)
        .corner_radius(properties, 1.0_f64)
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
        .border_brush_resource(properties, FOCUS_RING_BORDER)
        .border_width(properties, 2.0_f64)
        .corner_radius(properties, 4.0_f64)
        .build();

    let splitter_idle = StyleBuilder::new()
        .background_resource(properties, CONTROL_BORDER)
        .set(properties.transitions, surface_transition_rules(properties))
        .corner_radius(properties, 4.0_f64)
        .build();
    let splitter_hover = StyleBuilder::new()
        .background_resource(properties, CONTROL_HOVER_BORDER)
        .build();
    let splitter_pressed = StyleBuilder::new()
        .background_resource(properties, ACCENT_BG)
        .build();
    let splitter_focused = StyleBuilder::new()
        .border_brush_resource(properties, FOCUS_RING_BORDER)
        .border_width(properties, 2.0_f64)
        .build();

    let scroll_bar_idle = StyleBuilder::new()
        .set(
            properties.control_template,
            Some(default_scroll_bar_template()),
        )
        .background_resource(properties, TRACK_BG)
        .corner_radius(properties, 5.0_f64)
        .build();
    let scroll_bar_thumb_idle = StyleBuilder::new()
        .background_resource(properties, CONTROL_HOVER_BORDER)
        .corner_radius(properties, 5.0_f64)
        .build();
    let scroll_bar_thumb_hover = StyleBuilder::new()
        .background_resource(properties, THUMB_BG)
        .build();
    let scroll_bar_pressed = StyleBuilder::new()
        .background_resource(properties, CONTROL_BORDER)
        .build();

    let divider_style = StyleBuilder::new()
        .background_resource(properties, CONTROL_BORDER)
        .corner_radius(properties, 1.0_f64)
        .build();

    let checkbox_idle = StyleBuilder::new()
        .set(
            properties.control_template,
            Some(default_check_box_template()),
        )
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .set(properties.font_size, 15.0_f64)
        .set(properties.layout_container, LayoutContainer::row())
        .set(properties.layout_spacing, LayoutSpacing::gap(10.0))
        .set(
            properties.layout_alignment,
            LayoutAlignment::new().align_items(LayoutAlign::Center),
        )
        .build();
    let checkbox_disabled = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_MUTED_FG)
        .build();
    let checkbox_box_idle = StyleBuilder::new()
        .background_resource(properties, CONTROL_BG)
        .border_brush_resource(properties, CONTROL_BORDER)
        .border_width(properties, 1.0_f64)
        .corner_radius(properties, 5.0_f64)
        .set(properties.layout_container, LayoutContainer::row())
        .set(properties.layout_size, LayoutSize::logical(22.0, 22.0))
        .set(properties.layout_alignment, centered_alignment())
        .build();
    let checkbox_box_hover = StyleBuilder::new()
        .border_brush_resource(properties, CONTROL_HOVER_BORDER)
        .build();
    let checkbox_box_checked = StyleBuilder::new()
        .background_resource(properties, SUCCESS_BG)
        .border_brush_resource(properties, SUCCESS_BG)
        .build();
    let checkbox_box_focused = StyleBuilder::new()
        .border_brush_resource(properties, FOCUS_RING_BORDER)
        .border_width(properties, 2.0_f64)
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
    let radio_idle = StyleBuilder::new()
        .set(
            properties.control_template,
            Some(default_radio_button_template()),
        )
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .set(properties.font_size, 15.0_f64)
        .set(properties.layout_container, LayoutContainer::row())
        .set(properties.layout_spacing, LayoutSpacing::gap(10.0))
        .set(
            properties.layout_alignment,
            LayoutAlignment::new().align_items(LayoutAlign::Center),
        )
        .build();
    let radio_disabled = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_MUTED_FG)
        .build();
    let radio_indicator_idle = StyleBuilder::new()
        .background_resource(properties, CONTROL_BG)
        .border_brush_resource(properties, CONTROL_BORDER)
        .border_width(properties, 1.0_f64)
        .corner_radius(properties, 11.0_f64)
        .set(properties.layout_container, LayoutContainer::row())
        .set(properties.layout_size, LayoutSize::logical(22.0, 22.0))
        .set(properties.layout_alignment, centered_alignment())
        .build();
    let radio_indicator_hover = StyleBuilder::new()
        .border_brush_resource(properties, CONTROL_HOVER_BORDER)
        .build();
    let radio_indicator_checked = StyleBuilder::new()
        .border_brush_resource(properties, SUCCESS_BG)
        .build();
    let radio_indicator_focused = StyleBuilder::new()
        .border_brush_resource(properties, FOCUS_RING_BORDER)
        .border_width(properties, 2.0_f64)
        .build();
    let radio_dot_idle = StyleBuilder::new()
        .corner_radius(properties, 5.0_f64)
        .set(properties.layout_size, LayoutSize::logical(10.0, 10.0))
        .build();
    let radio_dot_checked = StyleBuilder::new()
        .background_resource(properties, SUCCESS_BG)
        .build();
    let radio_label = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .build();
    let expander_idle = StyleBuilder::new()
        .set(
            properties.control_template,
            Some(default_expander_template()),
        )
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .set(properties.font_size, 15.0_f64)
        .set(properties.layout_container, LayoutContainer::column())
        .set(properties.layout_spacing, LayoutSpacing::gap(8.0))
        .build();
    let expander_header = StyleBuilder::new()
        .background_resource(properties, CONTROL_BG)
        .border_brush_resource(properties, CONTROL_BORDER)
        .set(
            properties.transitions,
            border_affordance_transition_rules(properties),
        )
        .border_width(properties, 1.0_f64)
        .corner_radius(properties, 8.0_f64)
        .set(properties.layout_container, LayoutContainer::row())
        .padding_xy(properties, 10.0, 7.0)
        .set(
            properties.layout_alignment,
            LayoutAlignment::new().align_items(LayoutAlign::Center),
        )
        .build();
    let expander_header_expanded = StyleBuilder::new()
        .border_brush_resource(properties, CONTROL_HOVER_BORDER)
        .build();
    let expander_header_focused = StyleBuilder::new()
        .border_brush_resource(properties, FOCUS_RING_BORDER)
        .border_width(properties, 2.0_f64)
        .build();
    let expander_disclosure = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_MUTED_FG)
        .set(properties.font_size, 18.0_f64)
        .set(
            properties.layout_size,
            LayoutSize::new()
                .width(LayoutDimension::Logical(20.0))
                .flex_shrink(0.0),
        )
        .build();
    let expander_label = StyleBuilder::new()
        .set(
            properties.layout_size,
            LayoutSize::new()
                .min_width(LayoutDimension::Logical(0.0))
                .flex_grow(1.0)
                .flex_shrink(1.0),
        )
        .build();
    let expander_content = StyleBuilder::new()
        .padding_xy(properties, 30.0, 0.0)
        .set(properties.layout_spacing, LayoutSpacing::gap(8.0))
        .build();
    let accordion_idle = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .set(properties.font_size, 15.0_f64)
        .set(properties.layout_container, LayoutContainer::column())
        .set(properties.layout_spacing, LayoutSpacing::gap(8.0))
        .build();
    let accordion_section_idle = StyleBuilder::new()
        .set(
            properties.control_template,
            Some(default_accordion_section_template()),
        )
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .set(properties.layout_container, LayoutContainer::column())
        .set(properties.layout_spacing, LayoutSpacing::gap(8.0))
        .build();
    let accordion_section_header = StyleBuilder::new()
        .background_resource(properties, CONTROL_BG)
        .border_brush_resource(properties, CONTROL_BORDER)
        .set(
            properties.transitions,
            border_affordance_transition_rules(properties),
        )
        .border_width(properties, 1.0_f64)
        .corner_radius(properties, 8.0_f64)
        .set(properties.layout_container, LayoutContainer::row())
        .padding_xy(properties, 10.0, 7.0)
        .set(
            properties.layout_alignment,
            LayoutAlignment::new().align_items(LayoutAlign::Center),
        )
        .build();
    let accordion_section_header_expanded = StyleBuilder::new()
        .border_brush_resource(properties, CONTROL_HOVER_BORDER)
        .build();
    let accordion_section_header_focused = StyleBuilder::new()
        .border_brush_resource(properties, FOCUS_RING_BORDER)
        .border_width(properties, 2.0_f64)
        .build();
    let accordion_section_disclosure = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_MUTED_FG)
        .set(properties.font_size, 18.0_f64)
        .set(
            properties.layout_size,
            LayoutSize::new()
                .width(LayoutDimension::Logical(20.0))
                .flex_shrink(0.0),
        )
        .build();
    let accordion_section_label = StyleBuilder::new()
        .set(
            properties.layout_size,
            LayoutSize::new()
                .min_width(LayoutDimension::Logical(0.0))
                .flex_grow(1.0)
                .flex_shrink(1.0),
        )
        .build();
    let accordion_section_content = StyleBuilder::new()
        .padding_xy(properties, 30.0, 0.0)
        .set(properties.layout_spacing, LayoutSpacing::gap(8.0))
        .build();

    let progress_idle = StyleBuilder::new()
        .set(
            properties.control_template,
            Some(default_progress_template()),
        )
        .background_resource(properties, TRACK_BG)
        .border_brush_resource(properties, CONTROL_BORDER)
        .border_width(properties, 1.0_f64)
        .corner_radius(properties, 9.0_f64)
        .build();
    let progress_fill = StyleBuilder::new()
        .background_resource(properties, ACCENT_BG)
        .corner_radius(properties, 8.0_f64)
        .build();
    let progress_label = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .set(properties.font_size, 11.0_f64)
        .build();

    let text_input_idle = StyleBuilder::new()
        .set(
            properties.control_template,
            Some(default_text_input_template()),
        )
        .background_resource(properties, CONTROL_BG)
        .border_brush_resource(properties, CONTROL_BORDER)
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .set(
            properties.transitions,
            border_affordance_transition_rules(properties),
        )
        .set(properties.font_size, 15.0_f64)
        .border_width(properties, 1.0_f64)
        .corner_radius(properties, 8.0_f64)
        .build();
    let text_input_hover = StyleBuilder::new()
        .border_brush_resource(properties, CONTROL_HOVER_BORDER)
        .build();
    let text_input_focused = StyleBuilder::new()
        .border_brush_resource(properties, FOCUS_RING_BORDER)
        .border_width(properties, 2.0_f64)
        .build();
    let text_input_disabled = StyleBuilder::new()
        .background_resource(properties, TRACK_BG)
        .set_resource(properties.foreground, ON_SURFACE_MUTED_FG)
        .build();
    let text_input_read_only = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_MUTED_FG)
        .build();
    let text_input_placeholder = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_MUTED_FG)
        .build();
    let text_input_caret = StyleBuilder::new()
        .background_resource(properties, TEXT_CARET_BG)
        .corner_radius(properties, 0.75_f64)
        .build();
    let text_input_selection = StyleBuilder::new()
        .background_resource(properties, TEXT_SELECTION_BG)
        .corner_radius(properties, 3.0_f64)
        .build();

    let select_idle = StyleBuilder::new()
        .set(properties.control_template, Some(default_select_template()))
        .background_resource(properties, CONTROL_BG)
        .border_brush_resource(properties, CONTROL_BORDER)
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .set(
            properties.transitions,
            border_affordance_transition_rules(properties),
        )
        .set(properties.font_size, 14.0_f64)
        .border_width(properties, 1.0_f64)
        .corner_radius(properties, 8.0_f64)
        .set(properties.layout_container, LayoutContainer::row())
        .set(properties.layout_size, LayoutSize::logical(220.0, 38.0))
        .padding_xy(properties, 12.0, 8.0)
        .set(
            properties.layout_alignment,
            LayoutAlignment::new()
                .align_items(LayoutAlign::Center)
                .justify_content(LayoutJustify::Start),
        )
        .build();
    let select_hover = StyleBuilder::new()
        .border_brush_resource(properties, CONTROL_HOVER_BORDER)
        .background_resource(properties, CONTROL_HOVER_BG)
        .build();
    let select_focused = StyleBuilder::new()
        .border_brush_resource(properties, FOCUS_RING_BORDER)
        .border_width(properties, 2.0_f64)
        .build();
    let select_disabled = StyleBuilder::new()
        .background_resource(properties, TRACK_BG)
        .set_resource(properties.foreground, ON_SURFACE_MUTED_FG)
        .build();
    let select_label = StyleBuilder::new()
        .set(
            properties.layout_size,
            LayoutSize::new()
                .min_width(LayoutDimension::Logical(0.0))
                .flex_grow(1.0)
                .flex_shrink(1.0),
        )
        .build();
    let select_indicator = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_MUTED_FG)
        .set(properties.font_size, 12.0_f64)
        .build();

    let list_box_idle = StyleBuilder::new()
        .background_resource(properties, CONTROL_BG)
        .border_brush_resource(properties, CONTROL_BORDER)
        .set(
            properties.transitions,
            border_affordance_transition_rules(properties),
        )
        .border_width(properties, 1.0_f64)
        .corner_radius(properties, 8.0_f64)
        .build();
    let list_box_focused = StyleBuilder::new()
        .border_brush_resource(properties, FOCUS_RING_BORDER)
        .border_width(properties, 2.0_f64)
        .build();
    let list_item_idle = StyleBuilder::new()
        .set(
            properties.control_template,
            Some(default_list_item_template()),
        )
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .set(
            properties.transitions,
            background_transition_rules(properties),
        )
        .set(properties.font_size, 14.0_f64)
        .corner_radius(properties, 6.0_f64)
        .build();
    let list_item_content = StyleBuilder::new()
        .set(properties.layout_container, LayoutContainer::row())
        .set(properties.layout_size, LayoutSize::percent(1.0, 1.0))
        .padding_xy(properties, 10.0, 7.0)
        .set(
            properties.layout_alignment,
            LayoutAlignment::new().align_items(LayoutAlign::Center),
        )
        .build();
    let list_item_hover = StyleBuilder::new()
        .background_resource(properties, CONTROL_HOVER_BG)
        .build();
    let list_item_selected = StyleBuilder::new()
        .background_resource(properties, ACCENT_BG)
        .set_resource(properties.foreground, ON_ACCENT_FG)
        .build();
    let list_item_disabled = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_MUTED_FG)
        .build();
    let segmented_idle = StyleBuilder::new()
        .background_resource(properties, CONTROL_BG)
        .border_brush_resource(properties, CONTROL_BORDER)
        .set(
            properties.transitions,
            border_affordance_transition_rules(properties),
        )
        .border_width(properties, 1.0_f64)
        .corner_radius(properties, 9.0_f64)
        .set(properties.layout_container, LayoutContainer::row())
        .padding_all(properties, 2.0)
        .set(
            properties.layout_alignment,
            LayoutAlignment::new().align_items(LayoutAlign::Center),
        )
        .build();
    let segmented_focused = StyleBuilder::new()
        .border_brush_resource(properties, FOCUS_RING_BORDER)
        .border_width(properties, 2.0_f64)
        .build();
    let segmented_indicator_idle = StyleBuilder::new()
        .background_resource(properties, ACCENT_BG)
        .corner_radius(properties, 7.0_f64)
        .set(
            properties.transitions,
            layer_offset_transition_rules(DEFAULT_MOTION_NORMAL_DURATION, MOTION_NORMAL_DURATION),
        )
        .build();
    let segmented_item_idle = StyleBuilder::new()
        .set(
            properties.control_template,
            Some(default_segmented_item_template()),
        )
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .set(
            properties.transitions,
            background_transition_rules(properties),
        )
        .set(properties.font_size, 14.0_f64)
        .corner_radius(properties, 7.0_f64)
        .set(properties.layout_container, LayoutContainer::row())
        .padding_xy(properties, 12.0, 6.0)
        .set(properties.layout_alignment, centered_alignment())
        .build();
    let segmented_item_hover = StyleBuilder::new()
        .background_resource(properties, CONTROL_HOVER_BG)
        .build();
    let segmented_item_selected = StyleBuilder::new()
        .background(properties, None::<Brush>)
        .set_resource(properties.foreground, ON_ACCENT_FG)
        .build();
    let segmented_item_disabled = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_MUTED_FG)
        .build();
    let outline_view_idle = StyleBuilder::new()
        .background_resource(properties, CONTROL_BG)
        .border_brush_resource(properties, CONTROL_BORDER)
        .set(
            properties.transitions,
            border_affordance_transition_rules(properties),
        )
        .border_width(properties, 1.0_f64)
        .corner_radius(properties, 8.0_f64)
        .build();
    let outline_view_focused = StyleBuilder::new()
        .border_brush_resource(properties, FOCUS_RING_BORDER)
        .border_width(properties, 2.0_f64)
        .build();
    let outline_item_idle = StyleBuilder::new()
        .set(
            properties.control_template,
            Some(default_outline_item_template()),
        )
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .set(
            properties.transitions,
            background_transition_rules(properties),
        )
        .set(properties.font_size, 14.0_f64)
        .corner_radius(properties, 6.0_f64)
        .build();
    let outline_item_content = StyleBuilder::new()
        .set(properties.layout_container, LayoutContainer::row())
        .set(properties.layout_size, LayoutSize::percent(1.0, 1.0))
        .padding_xy(properties, 8.0, 5.0)
        .set(
            properties.layout_alignment,
            LayoutAlignment::new().align_items(LayoutAlign::Center),
        )
        .build();
    let outline_item_disclosure = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_MUTED_FG)
        .set(properties.font_size, 20.0_f64)
        .build();
    let outline_item_label = StyleBuilder::new()
        .set(
            properties.layout_size,
            LayoutSize::new()
                .min_width(LayoutDimension::Logical(0.0))
                .flex_grow(1.0)
                .flex_shrink(1.0),
        )
        .build();
    let outline_item_hover = StyleBuilder::new()
        .background_resource(properties, CONTROL_HOVER_BG)
        .build();
    let outline_item_selected = StyleBuilder::new()
        .background_resource(properties, ACCENT_BG)
        .set_resource(properties.foreground, ON_ACCENT_FG)
        .build();
    let tooltip_idle = StyleBuilder::new()
        .set(
            properties.control_template,
            Some(default_tooltip_template()),
        )
        .background_resource(properties, CONTROL_BG)
        .border_brush_resource(properties, CONTROL_HOVER_BORDER)
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .set(properties.font_size, 13.0_f64)
        .border_width(properties, 1.0_f64)
        .corner_radius(properties, 6.0_f64)
        .build();
    let popup_idle = StyleBuilder::new()
        .background_resource(properties, CONTROL_BG)
        .border_brush_resource(properties, CONTROL_HOVER_BORDER)
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .border_width(properties, 1.0_f64)
        .corner_radius(properties, 8.0_f64)
        .build();
    StyleCascadeBuilder::new()
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
            selectors::type_tag(TYPE_SLIDER),
            slider_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(TOGGLE_THUMB_PART),
            thumb_style,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(SLIDER_TRACK_PART),
            slider_track_style,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_with_class(SLIDER_TICK_PART, CLASS_SLIDER_MAJOR_TICK),
            slider_major_tick_style,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_with_class(SLIDER_TICK_PART, CLASS_SLIDER_MINOR_TICK),
            slider_minor_tick_style,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_with_class(SLIDER_TICK_LABEL_PART, CLASS_SLIDER_MAJOR_TICK),
            slider_major_tick_label_style,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_with_class(SLIDER_TICK_LABEL_PART, CLASS_SLIDER_MINOR_TICK),
            slider_minor_tick_label_style,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(SLIDER_THUMB_PART),
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
            selectors::type_tag(TYPE_SCROLL_BAR),
            scroll_bar_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(SCROLL_BAR_THUMB_PART),
            scroll_bar_thumb_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_when(
                SelectorStep::type_tag(TYPE_SCROLL_BAR).with_pseudo(PSEUDO_HOVER),
                SCROLL_BAR_THUMB_PART,
            ),
            scroll_bar_thumb_hover,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_SCROLL_BAR, PSEUDO_PRESSED),
            scroll_bar_pressed,
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
            selectors::part(CHECK_BOX_BOX_PART),
            checkbox_box_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_when(
                SelectorStep::type_tag(TYPE_CHECK_BOX).with_pseudo(PSEUDO_HOVER),
                CHECK_BOX_BOX_PART,
            ),
            checkbox_box_hover,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_when(
                SelectorStep::type_tag(TYPE_CHECK_BOX).with_pseudo(PSEUDO_CHECKED),
                CHECK_BOX_BOX_PART,
            ),
            checkbox_box_checked,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_when(
                SelectorStep::type_tag(TYPE_CHECK_BOX).with_pseudo(PSEUDO_FOCUS_VISIBLE),
                CHECK_BOX_BOX_PART,
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
            selectors::part(CHECK_BOX_LABEL_PART),
            checkbox_label,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::type_tag(TYPE_RADIO_BUTTON),
            radio_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_RADIO_BUTTON, PSEUDO_DISABLED),
            radio_disabled,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(RADIO_BUTTON_INDICATOR_PART),
            radio_indicator_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_when(
                SelectorStep::type_tag(TYPE_RADIO_BUTTON).with_pseudo(PSEUDO_HOVER),
                RADIO_BUTTON_INDICATOR_PART,
            ),
            radio_indicator_hover,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_when(
                SelectorStep::type_tag(TYPE_RADIO_BUTTON).with_pseudo(PSEUDO_CHECKED),
                RADIO_BUTTON_INDICATOR_PART,
            ),
            radio_indicator_checked,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_when(
                SelectorStep::type_tag(TYPE_RADIO_BUTTON).with_pseudo(PSEUDO_FOCUS_VISIBLE),
                RADIO_BUTTON_INDICATOR_PART,
            ),
            radio_indicator_focused,
        )
        .push_rule(
            StyleOrigin::Base,
            nested_radio_dot_selector(false),
            radio_dot_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            nested_radio_dot_selector(true),
            radio_dot_checked,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(RADIO_BUTTON_LABEL_PART),
            radio_label,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::type_tag(TYPE_EXPANDER),
            expander_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(EXPANDER_HEADER_PART),
            expander_header,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_when(
                SelectorStep::type_tag(TYPE_EXPANDER).with_pseudo(PSEUDO_EXPANDED),
                EXPANDER_HEADER_PART,
            ),
            expander_header_expanded,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_when(
                SelectorStep::type_tag(TYPE_EXPANDER).with_pseudo(PSEUDO_FOCUS_VISIBLE),
                EXPANDER_HEADER_PART,
            ),
            expander_header_focused,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(EXPANDER_DISCLOSURE_PART),
            expander_disclosure,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(EXPANDER_LABEL_PART),
            expander_label,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(EXPANDER_CONTENT_PART),
            expander_content,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::type_tag(TYPE_ACCORDION),
            accordion_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::type_tag(TYPE_ACCORDION_SECTION),
            accordion_section_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(ACCORDION_SECTION_HEADER_PART),
            accordion_section_header,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_when(
                SelectorStep::type_tag(TYPE_ACCORDION_SECTION).with_pseudo(PSEUDO_EXPANDED),
                ACCORDION_SECTION_HEADER_PART,
            ),
            accordion_section_header_expanded,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_when(
                SelectorStep::type_tag(TYPE_ACCORDION_SECTION).with_pseudo(PSEUDO_FOCUS_VISIBLE),
                ACCORDION_SECTION_HEADER_PART,
            ),
            accordion_section_header_focused,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(ACCORDION_SECTION_DISCLOSURE_PART),
            accordion_section_disclosure,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(ACCORDION_SECTION_LABEL_PART),
            accordion_section_label,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(ACCORDION_SECTION_CONTENT_PART),
            accordion_section_content,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::type_tag(TYPE_PROGRESS_BAR),
            progress_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(PROGRESS_BAR_FILL_PART),
            progress_fill,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(PROGRESS_BAR_LABEL_PART),
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
            selectors::part_when(
                SelectorStep::type_tag(TYPE_TEXT_INPUT).with_pseudo(PSEUDO_PLACEHOLDER_SHOWN),
                TEXT_INPUT_TEXT_PART,
            ),
            text_input_placeholder,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_when(
                SelectorStep::type_tag(TYPE_TEXT_INPUT).with_pseudo(PSEUDO_FOCUSED),
                TEXT_INPUT_SELECTION_PART,
            ),
            text_input_selection,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part_when(
                SelectorStep::type_tag(TYPE_TEXT_INPUT).with_pseudo(PSEUDO_FOCUSED),
                TEXT_INPUT_CARET_PART,
            ),
            text_input_caret,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::type_tag(TYPE_SELECT),
            select_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_SELECT, PSEUDO_HOVER),
            select_hover,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_SELECT, PSEUDO_FOCUS_VISIBLE),
            select_focused,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_SELECT, PSEUDO_DISABLED),
            select_disabled,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(SELECT_LABEL_PART),
            select_label,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(SELECT_INDICATOR_PART),
            select_indicator,
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
            selectors::part(LIST_ITEM_CONTENT_PART),
            list_item_content,
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
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_LIST_ITEM, PSEUDO_DISABLED),
            list_item_disabled,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::type_tag(TYPE_SEGMENTED_CONTROL),
            segmented_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_SEGMENTED_CONTROL, PSEUDO_FOCUS_VISIBLE),
            segmented_focused,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(selectors::template_part(
                TYPE_SEGMENTED_CONTROL,
                PART_SEGMENTED_SELECTION_INDICATOR,
            )),
            segmented_indicator_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::type_tag(TYPE_SEGMENTED_ITEM),
            segmented_item_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_SEGMENTED_ITEM, PSEUDO_HOVER),
            segmented_item_hover,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_SEGMENTED_ITEM, PSEUDO_SELECTED),
            segmented_item_selected,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_SEGMENTED_ITEM, PSEUDO_DISABLED),
            segmented_item_disabled,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::type_tag(TYPE_OUTLINE_VIEW),
            outline_view_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_OUTLINE_VIEW, PSEUDO_FOCUS_VISIBLE),
            outline_view_focused,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::type_tag(TYPE_OUTLINE_ITEM),
            outline_item_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(OUTLINE_ITEM_CONTENT_PART),
            outline_item_content,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(OUTLINE_ITEM_DISCLOSURE_PART),
            outline_item_disclosure,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(OUTLINE_ITEM_LABEL_PART),
            outline_item_label,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_OUTLINE_ITEM, PSEUDO_HOVER),
            outline_item_hover,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::state(TYPE_OUTLINE_ITEM, PSEUDO_SELECTED),
            outline_item_selected,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::type_tag(TYPE_TOOLTIP),
            tooltip_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::type_tag(TYPE_POPUP),
            popup_idle,
        )
}

fn centered_alignment() -> LayoutAlignment {
    LayoutAlignment::new()
        .justify_content(LayoutJustify::Center)
        .align_items(LayoutAlign::Center)
}

fn toggle_alignment(checked: bool) -> LayoutAlignment {
    let justify = if checked {
        LayoutJustify::End
    } else {
        LayoutJustify::Start
    };
    LayoutAlignment::new()
        .align_items(LayoutAlign::Center)
        .justify_content(justify)
}

fn button_transition_rules(properties: &BuiltInProperties) -> TransitionRules {
    surface_transition_rules(properties).with_rule(TransitionRule::interaction(
        TransitionTarget::property(properties.opacity.id()),
        scalar_transition_spec(DEFAULT_MOTION_FAST_DURATION, MOTION_FAST_DURATION),
    ))
}

fn surface_transition_rules(properties: &BuiltInProperties) -> TransitionRules {
    with_border_width_transition_rules(
        with_border_brush_transition_rules(background_transition_rules(properties), properties),
        properties,
    )
}

fn border_affordance_transition_rules(properties: &BuiltInProperties) -> TransitionRules {
    with_border_width_transition_rules(
        with_border_brush_transition_rules(TransitionRules::new(), properties),
        properties,
    )
}

fn background_transition_rules(properties: &BuiltInProperties) -> TransitionRules {
    TransitionRules::single(color_transition_rule(
        TransitionTarget::property(properties.surface.background.id()),
        DEFAULT_MOTION_NORMAL_DURATION,
        MOTION_NORMAL_DURATION,
    ))
}

fn with_border_brush_transition_rules(
    mut rules: TransitionRules,
    properties: &BuiltInProperties,
) -> TransitionRules {
    let brushes = properties.surface.border_brushes;
    for property in [brushes.top, brushes.right, brushes.bottom, brushes.left] {
        rules.push(color_transition_rule(
            TransitionTarget::property(property.id()),
            DEFAULT_MOTION_NORMAL_DURATION,
            MOTION_NORMAL_DURATION,
        ));
    }
    rules
}

fn with_border_width_transition_rules(
    mut rules: TransitionRules,
    properties: &BuiltInProperties,
) -> TransitionRules {
    let widths = properties.surface.border_widths;
    for property in [widths.top, widths.right, widths.bottom, widths.left] {
        rules.push(TransitionRule::interaction(
            TransitionTarget::property(property.id()),
            scalar_transition_spec(DEFAULT_MOTION_FAST_DURATION, MOTION_FAST_DURATION),
        ));
    }
    rules
}

fn color_transition_rule(
    target: TransitionTarget,
    duration: TimerDuration,
    duration_token: ResourceKey,
) -> TransitionRule {
    TransitionRule::interaction(target, color_transition_spec(duration, duration_token))
}

fn color_transition_spec(duration: TimerDuration, duration_token: ResourceKey) -> TransitionSpec {
    TransitionSpec::linear(duration)
        .with_duration_resource(duration_token)
        .with_color_interpolation(ColorInterpolation::SRGB)
        .with_color_interpolation_resource(MOTION_COLOR_INTERPOLATION)
}

fn scalar_transition_spec(duration: TimerDuration, duration_token: ResourceKey) -> TransitionSpec {
    TransitionSpec::linear(duration).with_duration_resource(duration_token)
}

fn layer_offset_transition_rules(
    duration: TimerDuration,
    duration_token: ResourceKey,
) -> TransitionRules {
    TransitionRules::single(TransitionRule::interaction(
        TransitionTarget::layer_offset(),
        TransitionSpec::timed(duration, DEFAULT_MOTION_STANDARD_EASING)
            .with_duration_resource(duration_token)
            .with_easing_resource(MOTION_STANDARD_EASING),
    ))
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

fn nested_radio_dot_selector(checked: bool) -> Selector {
    let owner = if checked {
        SelectorStep::type_tag(TYPE_RADIO_BUTTON).with_pseudo(PSEUDO_CHECKED)
    } else {
        SelectorStep::type_tag(TYPE_RADIO_BUTTON)
    };
    Selector::from([
        owner,
        SelectorStep::part_tag(PART_RADIO_BUTTON_INDICATOR),
        SelectorStep::part_tag(PART_RADIO_BUTTON_DOT),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_aliases_dark_palette() {
        let default = default_theme();
        let dark = dark_theme();
        assert_eq!(
            default.get::<Brush>(ON_SURFACE_FG),
            dark.get::<Brush>(ON_SURFACE_FG)
        );
        assert_eq!(
            default.get::<Option<Brush>>(TEXT_CARET_BG),
            dark.get::<Option<Brush>>(TEXT_CARET_BG)
        );
    }

    #[test]
    fn light_theme_provides_state_tokens() {
        let light = light_theme();
        assert_eq!(
            light.get::<Brush>(ON_SURFACE_FG),
            Some(&Brush::from(Color::from_rgba8(0x16, 0x1a, 0x24, 0xff))),
        );
        assert!(light.contains(CONTROL_HOVER_BG));
        assert!(light.contains(TEXT_SELECTION_BG));
        assert!(light.contains(SLIDER_MAJOR_TICK_BG));
    }

    #[test]
    fn theme_builders_can_be_extended_with_app_tokens() {
        const APP_TOKEN: ResourceKey = ResourceKey::new(9_000);

        let theme = dark_theme_builder().set(APP_TOKEN, 42_u32).build();

        assert_eq!(theme.get::<u32>(APP_TOKEN), Some(&42));
        assert_eq!(
            theme.get::<Brush>(ON_SURFACE_FG),
            dark_theme().get::<Brush>(ON_SURFACE_FG),
        );
    }
}
