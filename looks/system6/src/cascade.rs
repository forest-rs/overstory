// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The System 6 component cascade.
//!
//! Everything is paper with a 1px ink frame. Push buttons are rounded and
//! **invert on press** (ink fill, paper label) — the classic Mac click. Dialogs
//! and fields are square. There is no hover highlight and no motion.
//!
//! Focus is deliberately not drawn: in System 6 the cue was the default
//! button's bold *outline* ring, which sits outside the box and so needs a
//! paint-only outline channel the toolkit doesn't have yet (tracked as G7).
//! Rather than reflow by growing a border, focus is left visually unchanged.

use overstory::pseudos::{PSEUDO_FOCUS_VISIBLE, PSEUDO_HOVER, PSEUDO_PRESSED};
use overstory::{
    Brush, BuiltInProperties, Color, LayoutAlign, LayoutAlignment, LayoutContainer, LayoutJustify,
    PART_CHECK_BOX_BOX, PART_SLIDER_THUMB, PART_SLIDER_TRACK, PART_TOGGLE_THUMB, SurfaceStyleExt,
    TYPE_BUTTON, TYPE_CHECK_BOX, TYPE_GENERIC, TYPE_POPUP, TYPE_SELECT, TYPE_SLIDER, TYPE_TEXT,
    TYPE_TEXT_INPUT, TYPE_TOGGLE, selectors,
};
use understory_style::{
    ClassId, PseudoClassId, ResourceKey, Selector, SelectorStep, Style, StyleBuilder, StyleCascade,
    StyleCascadeBuilder, StyleOrigin, TypeTag,
};

use crate::tokens::{
    ACCENT_BG, CONTROL_BG, CONTROL_BORDER, ON_ACCENT_FG, ON_SURFACE_FG, ON_SURFACE_MUTED_FG,
    PANEL_BG, ResolvedTokens, SURFACE_BG, THUMB_BG, TRACK_BG,
};

/// Rounded push-button radius; everything else is square.
const RADIUS_BUTTON: f64 = 9.0;
/// System 6 button content padding (overrides the framework's larger default).
const BUTTON_PAD_X: f64 = 18.0;
const BUTTON_PAD_Y: f64 = 8.0;
/// Gap between the default button's outer ring and inner face.
const BUTTON_RING_PAD: f64 = 3.0;
/// Inner-face padding for the double-frame default button, sized so its outer
/// box matches a regular button: a regular button insets content by
/// `pad + 1` (border) per side; the double frame insets by
/// `inner + 1 + ring + 1`, so `inner = pad - ring - 1`.
const BUTTON_INNER_PAD_X: f64 = BUTTON_PAD_X - BUTTON_RING_PAD - 1.0;
const BUTTON_INNER_PAD_Y: f64 = BUTTON_PAD_Y - BUTTON_RING_PAD - 1.0;

fn transparent() -> Option<Brush> {
    Some(Brush::Solid(Color::TRANSPARENT))
}

fn type_sel(type_tag: TypeTag) -> Selector {
    Selector::single(SelectorStep::type_tag(type_tag))
}

fn type_state_sel(type_tag: TypeTag, pseudo: PseudoClassId) -> Selector {
    Selector::single(SelectorStep::type_tag(type_tag).with_pseudo(pseudo))
}

fn class_sel(class: ClassId) -> Selector {
    Selector::single(SelectorStep::type_tag(TYPE_GENERIC).with_class(class))
}

fn untyped_class_sel(class: ClassId) -> Selector {
    Selector::single(SelectorStep::class(class))
}

fn type_class_sel(type_tag: TypeTag, class: ClassId) -> Selector {
    Selector::single(SelectorStep::type_tag(type_tag).with_class(class))
}

fn centered() -> LayoutAlignment {
    LayoutAlignment::new()
        .align_items(LayoutAlign::Center)
        .justify_content(LayoutJustify::Center)
}

fn text_class_sel(class: ClassId) -> Selector {
    Selector::single(SelectorStep::type_tag(TYPE_TEXT).with_class(class))
}

/// A paper fill inside a 1px ink frame.
fn framed(properties: &BuiltInProperties, background: ResourceKey, corner: f64) -> Style {
    StyleBuilder::new()
        .background_resource(properties, background)
        .border_brush_resource(properties, CONTROL_BORDER)
        .border_width(properties, 1.0_f64)
        .corner_radius(properties, corner)
        .build()
}

/// Builds the System 6 component cascade for `tokens`.
#[must_use]
pub fn cascade(properties: &BuiltInProperties, tokens: &ResolvedTokens) -> StyleCascade {
    let c = tokens.classes;

    let root = StyleBuilder::new()
        .background_resource(properties, SURFACE_BG)
        .build();
    let surface = StyleBuilder::new()
        .background_resource(properties, PANEL_BG)
        .build();
    let panel = framed(properties, PANEL_BG, 0.0);
    let card = framed(properties, PANEL_BG, 0.0);

    let heading = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .set(properties.font_size, 20.0_f64)
        .build();
    let body = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .set(properties.font_size, 13.0_f64)
        .build();
    let caption = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_MUTED_FG)
        .set(properties.font_size, 12.0_f64)
        .build();

    // Buttons: paper with a rounded ink frame; press inverts to ink/paper.
    // Override the framework's large default padding so System 6 buttons are
    // modest, and so the regular and primary (double-frame) buttons match in
    // size (the primary's inner padding is sized to land at the same outer box;
    // see `BUTTON_PAD_*`).
    let button_idle = StyleBuilder::new()
        .background_resource(properties, PANEL_BG)
        .border_brush_resource(properties, CONTROL_BORDER)
        .border_width(properties, 1.0_f64)
        .corner_radius(properties, RADIUS_BUTTON)
        .padding_xy(properties, BUTTON_PAD_X, BUTTON_PAD_Y)
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .build();
    // No hover highlight in System 6 — neutralize the framework accent-on-hover.
    let button_hover = StyleBuilder::new()
        .background_resource(properties, PANEL_BG)
        .build();
    let button_pressed = StyleBuilder::new()
        .background_resource(properties, ACCENT_BG)
        .set_resource(properties.foreground, ON_ACCENT_FG)
        .build();

    // Text fields: square, paper, ink frame; hold the frame on focus (no reflow).
    let text_input_idle = framed(properties, CONTROL_BG, 0.0);
    let text_input_focus = StyleBuilder::new()
        .border_brush_resource(properties, CONTROL_BORDER)
        .border_width(properties, 1.0_f64)
        .build();

    // Slider: the outer frame is reserved transparent (so it doesn't box the
    // whole slider including the ticks) and only inked on focus. The visible
    // parts are the framed track and thumb.
    let slider_idle = StyleBuilder::new()
        .set(properties.surface.border_brushes.top, transparent())
        .set(properties.surface.border_brushes.right, transparent())
        .set(properties.surface.border_brushes.bottom, transparent())
        .set(properties.surface.border_brushes.left, transparent())
        .border_width(properties, 1.0_f64)
        .corner_radius(properties, 0.0_f64)
        .build();
    let slider_focus = StyleBuilder::new()
        .border_brush_resource(properties, CONTROL_BORDER)
        .corner_radius(properties, 0.0_f64)
        .build();
    let slider_track = framed(properties, TRACK_BG, 0.0);
    let slider_thumb = framed(properties, THUMB_BG, 0.0);

    // Dropdown: a paper field and popup, both in 1px ink frames.
    let select_idle = framed(properties, CONTROL_BG, 0.0);
    let popup = framed(properties, PANEL_BG, 0.0);

    // Toggle: a framed track with a framed thumb so the thumb reads instead of
    // blending paper-on-paper.
    let toggle_idle = framed(properties, TRACK_BG, 0.0);
    let toggle_thumb = framed(properties, THUMB_BG, 0.0);

    // Checkbox: a look owning structure, not just style — swap in the look's own
    // template via the cascade (see `crate::templates`), and square the box.
    let check_box_idle = StyleBuilder::new()
        .set(
            properties.control_template,
            Some(crate::templates::check_box()),
        )
        .build();
    let check_box_box = framed(properties, CONTROL_BG, 0.0);

    // Default (primary) button: the classic Mac double border. A template swap
    // (an outer ring around an inner face) — styling can't draw two concentric
    // outlines with a gap. The button's own surface goes transparent so the
    // outer/inner frames are the visible structure.
    let button_primary = StyleBuilder::new()
        .set(
            properties.control_template,
            Some(crate::templates::default_button()),
        )
        .background(properties, None::<Brush>)
        .border_width(properties, 0.0_f64)
        .padding_all(properties, 0.0)
        .build();
    let button_outer = StyleBuilder::new()
        .border_brush_resource(properties, CONTROL_BORDER)
        .border_width(properties, 1.0_f64)
        .corner_radius(properties, RADIUS_BUTTON + BUTTON_RING_PAD)
        .padding_all(properties, BUTTON_RING_PAD)
        .set(properties.layout_container, LayoutContainer::row())
        .set(properties.layout_alignment, centered())
        .build();
    let button_inner = StyleBuilder::new()
        .background_resource(properties, CONTROL_BG)
        .border_brush_resource(properties, CONTROL_BORDER)
        .border_width(properties, 1.0_f64)
        .corner_radius(properties, RADIUS_BUTTON)
        .padding_xy(properties, BUTTON_INNER_PAD_X, BUTTON_INNER_PAD_Y)
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .set(properties.layout_container, LayoutContainer::row())
        .set(properties.layout_alignment, centered())
        .build();

    StyleCascadeBuilder::new()
        .push_rule(StyleOrigin::Base, class_sel(c.root), root)
        .push_rule(StyleOrigin::Base, class_sel(c.surface), surface)
        .push_rule(StyleOrigin::Base, class_sel(c.panel), panel)
        .push_rule(StyleOrigin::Base, class_sel(c.card), card)
        .push_rule(StyleOrigin::Base, text_class_sel(c.heading), heading)
        .push_rule(StyleOrigin::Base, text_class_sel(c.body), body)
        .push_rule(StyleOrigin::Base, text_class_sel(c.caption), caption)
        .push_rule(StyleOrigin::Base, type_sel(TYPE_BUTTON), button_idle)
        .push_rule(
            StyleOrigin::Base,
            type_state_sel(TYPE_BUTTON, PSEUDO_HOVER),
            button_hover,
        )
        .push_rule(
            StyleOrigin::Base,
            type_state_sel(TYPE_BUTTON, PSEUDO_PRESSED),
            button_pressed,
        )
        .push_rule(
            StyleOrigin::Base,
            type_sel(TYPE_TEXT_INPUT),
            text_input_idle,
        )
        .push_rule(
            StyleOrigin::Base,
            type_state_sel(TYPE_TEXT_INPUT, PSEUDO_FOCUS_VISIBLE),
            text_input_focus,
        )
        .push_rule(StyleOrigin::Base, type_sel(TYPE_SLIDER), slider_idle)
        .push_rule(
            StyleOrigin::Base,
            type_state_sel(TYPE_SLIDER, PSEUDO_FOCUS_VISIBLE),
            slider_focus,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(selectors::template_part(TYPE_SLIDER, PART_SLIDER_TRACK)),
            slider_track,
        )
        .push_rule(
            StyleOrigin::Base,
            selectors::part(selectors::template_part(TYPE_SLIDER, PART_SLIDER_THUMB)),
            slider_thumb,
        )
        .push_rule(StyleOrigin::Base, type_sel(TYPE_SELECT), select_idle)
        .push_rule(StyleOrigin::Base, type_sel(TYPE_POPUP), popup)
        .push_rule(StyleOrigin::Base, type_sel(TYPE_TOGGLE), toggle_idle)
        .push_rule(
            StyleOrigin::Base,
            selectors::part(selectors::template_part(TYPE_TOGGLE, PART_TOGGLE_THUMB)),
            toggle_thumb,
        )
        .push_rule(StyleOrigin::Base, type_sel(TYPE_CHECK_BOX), check_box_idle)
        .push_rule(
            StyleOrigin::Base,
            selectors::part(selectors::template_part(TYPE_CHECK_BOX, PART_CHECK_BOX_BOX)),
            check_box_box,
        )
        .push_rule(
            StyleOrigin::Base,
            type_class_sel(TYPE_BUTTON, c.primary),
            button_primary,
        )
        .push_rule(
            StyleOrigin::Base,
            untyped_class_sel(c.default_button_outer),
            button_outer,
        )
        .push_rule(
            StyleOrigin::Base,
            untyped_class_sel(c.default_button_inner),
            button_inner,
        )
        .build()
}
