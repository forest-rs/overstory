// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The Chicago component cascade.
//!
//! Chicago's signature is the 3D bevel built from per-side border brushes: a
//! *raised* control is light on top/left and dark on bottom/right; a *sunken*
//! one is the inverse. Pressing a button swaps the two so it visibly sinks.
//! Everything is square-cornered and flat-filled.
//!
//! This is also a useful counter-example to the framework defaults: the
//! framework button is accent-filled, but a period button is neutral face gray,
//! so Chicago overrides the button background to the panel face.

use overstory::pseudos::{PSEUDO_FOCUS_VISIBLE, PSEUDO_HOVER, PSEUDO_PRESSED};
use overstory::{
    Brush, BuiltInProperties, Color, PART_SLIDER_THUMB, PART_SLIDER_TRACK, SurfaceStyleExt,
    TYPE_BUTTON, TYPE_GENERIC, TYPE_SLIDER, TYPE_TEXT, TYPE_TEXT_INPUT, selectors,
};
use understory_style::{
    ClassId, PseudoClassId, ResourceKey, Selector, SelectorStep, Style, StyleBuilder, StyleCascade,
    StyleCascadeBuilder, StyleOrigin, TypeTag,
};

use crate::tokens::{
    CONTROL_BG, ChicagoResources, ON_SURFACE_FG, ON_SURFACE_MUTED_FG, PANEL_BG, ResolvedTokens,
    SURFACE_BG, THUMB_BG, TRACK_BG,
};

/// Bevel thickness, held constant across states so focus/press never reflow.
const BEVEL: f64 = 2.0;

fn transparent() -> Option<Brush> {
    Some(Brush::Solid(Color::from_rgba8(0, 0, 0, 0)))
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

fn text_class_sel(class: ClassId) -> Selector {
    Selector::single(SelectorStep::type_tag(TYPE_TEXT).with_class(class))
}

/// A raised 3D box: light top/left, dark bottom/right, square corners.
fn raised(properties: &BuiltInProperties, background: ResourceKey, r: ChicagoResources) -> Style {
    bevel(properties, background, r.bevel_light, r.bevel_dark)
}

/// A sunken 3D box: dark top/left, light bottom/right, square corners.
fn sunken(properties: &BuiltInProperties, background: ResourceKey, r: ChicagoResources) -> Style {
    bevel(properties, background, r.bevel_dark, r.bevel_light)
}

fn bevel(
    properties: &BuiltInProperties,
    background: ResourceKey,
    top_left: ResourceKey,
    bottom_right: ResourceKey,
) -> Style {
    StyleBuilder::new()
        .background_resource(properties, background)
        .set_resource(properties.surface.border_brushes.top, top_left)
        .set_resource(properties.surface.border_brushes.left, top_left)
        .set_resource(properties.surface.border_brushes.right, bottom_right)
        .set_resource(properties.surface.border_brushes.bottom, bottom_right)
        .border_width(properties, BEVEL)
        .corner_radius(properties, 0.0_f64)
        .build()
}

/// Builds the Chicago component cascade for `tokens`.
#[must_use]
pub fn cascade(properties: &BuiltInProperties, tokens: &ResolvedTokens) -> StyleCascade {
    let c = tokens.classes;
    let r = tokens.resources;

    // Chrome.
    let root = StyleBuilder::new()
        .background_resource(properties, SURFACE_BG)
        .build();
    let surface = StyleBuilder::new()
        .background_resource(properties, SURFACE_BG)
        .build();
    let panel = raised(properties, PANEL_BG, r);
    let card = raised(properties, PANEL_BG, r);

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

    // Buttons: neutral face, raised; pressed inverts the bevel to sink in.
    let button_idle = StyleBuilder::new()
        .background_resource(properties, PANEL_BG)
        .set_resource(properties.surface.border_brushes.top, r.bevel_light)
        .set_resource(properties.surface.border_brushes.left, r.bevel_light)
        .set_resource(properties.surface.border_brushes.right, r.bevel_dark)
        .set_resource(properties.surface.border_brushes.bottom, r.bevel_dark)
        .border_width(properties, BEVEL)
        .corner_radius(properties, 0.0_f64)
        .set_resource(properties.foreground, ON_SURFACE_FG)
        .build();
    // Period buttons don't highlight on hover — neutralize the framework's
    // accent-on-hover so the face (and its black label) stay readable.
    let button_hover = StyleBuilder::new()
        .background_resource(properties, PANEL_BG)
        .build();
    let button_pressed = sunken(properties, PANEL_BG, r);

    // Text input: white, sunken; re-assert the bevel on focus so the framework's
    // focus border doesn't flatten it (and width stays constant — no reflow).
    let text_input_idle = sunken(properties, CONTROL_BG, r);
    let text_input_focus = sunken(properties, CONTROL_BG, r);

    // Slider: reserve a constant-width ring at idle, recolor on focus.
    let slider_idle = StyleBuilder::new()
        .border_width(properties, BEVEL)
        .set(properties.surface.border_brushes.top, transparent())
        .set(properties.surface.border_brushes.right, transparent())
        .set(properties.surface.border_brushes.bottom, transparent())
        .set(properties.surface.border_brushes.left, transparent())
        .corner_radius(properties, 0.0_f64)
        .build();
    let slider_focus = StyleBuilder::new()
        .border_brush_resource(properties, ON_SURFACE_FG)
        .corner_radius(properties, 0.0_f64)
        .build();

    // Slider parts: a sunken groove and a raised square thumb, so they read as
    // 3D against the face panel instead of blending face-on-face.
    let slider_track = sunken(properties, TRACK_BG, r);
    let slider_thumb = raised(properties, THUMB_BG, r);

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
        .build()
}
