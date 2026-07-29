// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The Signature component cascade.
//!
//! This cascade layers *after* Overstory's framework cascade (see
//! [`Ui::set_cascade`](overstory::Ui::set_cascade)). Because the Signature
//! theme already redefines the built-in palette roles, every framework control
//! re-skins with Signature colors and gradients without a rule here. This
//! cascade adds what the framework has no slot for, and corrects one framework
//! behavior:
//!
//! - **Chrome classes** — root / surface / panel / card backgrounds and the
//!   heading / body / caption type ramp.
//! - **Layout-stable focus rings.** The framework grows a control's border on
//!   focus (`0→2`px), but border width feeds layout, so focusing reflows
//!   neighbors. Signature instead *reserves* the ring width at idle (a
//!   transparent border) and only recolors it on focus — the box never changes
//!   size. The springy *expanding* ring is deferred until there's a paint-only
//!   outline channel (GAPS.md G7).
//!
//! Every rule references a token; the only literal is a structural transparent
//! brush used to reserve ring space.

use overstory::pseudos::{PSEUDO_DISABLED, PSEUDO_FOCUS_VISIBLE, PSEUDO_PRESSED};
use overstory::{
    Brush, BuiltInProperties, Color, LayoutSize, SurfaceStyleExt, TYPE_BUTTON, TYPE_GENERIC,
    TYPE_SELECT, TYPE_SLIDER, TYPE_TEXT, TYPE_TEXT_INPUT, TYPE_TOGGLE,
};
use understory_style::{
    ClassId, PseudoClassId, Selector, SelectorStep, StyleBuilder, StyleCascade,
    StyleCascadeBuilder, StyleOrigin, TypeTag,
};

use crate::motion;
use crate::tokens::{
    CONTROL_BORDER, FOCUS_RING_BORDER, ON_SURFACE_MUTED_FG, PANEL_BG, ResolvedTokens, SURFACE_BG,
    radius,
};

/// The reserved focus-ring width. Held constant across idle/focus so focusing
/// recolors the ring rather than resizing the box.
const RING_WIDTH: f64 = 2.0;

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

fn text_class_sel(class: ClassId) -> Selector {
    Selector::single(SelectorStep::type_tag(TYPE_TEXT).with_class(class))
}

/// Builds the Signature component cascade for `tokens`.
#[must_use]
pub fn cascade(properties: &BuiltInProperties, tokens: &ResolvedTokens) -> StyleCascade {
    let c = tokens.classes;
    let r = tokens.resources;

    // --- Chrome ------------------------------------------------------------
    let root = StyleBuilder::new()
        .background_resource(properties, SURFACE_BG)
        .build();
    let surface = StyleBuilder::new()
        .background_resource(properties, SURFACE_BG)
        .build();
    let panel = raised(properties, PANEL_BG, r, radius::LG);
    let card = raised(properties, r.card_gradient, r, radius::LG);

    let heading = StyleBuilder::new()
        .set_resource(properties.foreground, r.heading_fg)
        .set(properties.font_size, 26.0_f64)
        .build();
    let body = StyleBuilder::new()
        .set_resource(properties.foreground, r.body_fg)
        .set(properties.font_size, 15.0_f64)
        .build();
    let caption = StyleBuilder::new()
        .set_resource(properties.foreground, ON_SURFACE_MUTED_FG)
        .set(properties.font_size, 13.0_f64)
        .build();

    // --- Buttons -----------------------------------------------------------
    // Idle reserves the ring width with a transparent border (plus a hairline
    // top highlight); focus recolors all four edges. No size change, no reflow.
    let button_idle = StyleBuilder::new()
        .set(properties.transitions, motion::interaction(properties))
        .border_width(properties, RING_WIDTH)
        .set(properties.surface.border_brushes.top, transparent())
        .set(properties.surface.border_brushes.right, transparent())
        .set(properties.surface.border_brushes.bottom, transparent())
        .set(properties.surface.border_brushes.left, transparent())
        .build();
    let button_focus = StyleBuilder::new()
        .border_brush_resource(properties, FOCUS_RING_BORDER)
        .build();
    let button_pressed = StyleBuilder::new()
        .set(properties.opacity, 0.94_f32)
        .build();
    // Disabled: the framework mutes the fill/text; outline it so it still reads
    // as a button against the card instead of a faint blob.
    let button_disabled = StyleBuilder::new()
        .border_brush_resource(properties, CONTROL_BORDER)
        .build();

    // --- Text input: constant-width ring -----------------------------------
    // The framework resting border is 1px and grows to 2px on focus (reflow).
    // Hold it at the ring width and recolor on focus.
    let text_input_idle = StyleBuilder::new()
        .set(properties.transitions, motion::interaction(properties))
        .border_width(properties, RING_WIDTH)
        .border_brush_resource(properties, CONTROL_BORDER)
        .build();
    let text_input_focus = StyleBuilder::new()
        .border_brush_resource(properties, FOCUS_RING_BORDER)
        .build();

    // --- Slider: constant-width ring ---------------------------------------
    // The framework slider has no resting border and adds 2px on focus. Reserve
    // a transparent ring at idle and recolor on focus; keep the corner fixed so
    // focusing changes neither size nor shape.
    let slider_idle = StyleBuilder::new()
        .set(properties.transitions, motion::interaction(properties))
        .border_width(properties, RING_WIDTH)
        .set(properties.surface.border_brushes.top, transparent())
        .set(properties.surface.border_brushes.right, transparent())
        .set(properties.surface.border_brushes.bottom, transparent())
        .set(properties.surface.border_brushes.left, transparent())
        .corner_radius(properties, radius::SM)
        .build();
    let slider_focus = StyleBuilder::new()
        .border_brush_resource(properties, FOCUS_RING_BORDER)
        .corner_radius(properties, radius::SM)
        .build();

    // --- Select: constant-width ring, like the text input ------------------
    let select_idle = StyleBuilder::new()
        .set(properties.transitions, motion::interaction(properties))
        .border_width(properties, RING_WIDTH)
        .border_brush_resource(properties, CONTROL_BORDER)
        .build();
    let select_focus = StyleBuilder::new()
        .border_brush_resource(properties, FOCUS_RING_BORDER)
        .build();

    // --- Toggle: outline the off-track so it's visible against the card -----
    let toggle_idle = StyleBuilder::new()
        .set(properties.layout_size, LayoutSize::logical(74.0, 34.0))
        .border_brush_resource(properties, CONTROL_BORDER)
        .border_width(properties, 1.0_f64)
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
            type_state_sel(TYPE_BUTTON, PSEUDO_FOCUS_VISIBLE),
            button_focus,
        )
        .push_rule(
            StyleOrigin::Base,
            type_state_sel(TYPE_BUTTON, PSEUDO_PRESSED),
            button_pressed,
        )
        .push_rule(
            StyleOrigin::Base,
            type_state_sel(TYPE_BUTTON, PSEUDO_DISABLED),
            button_disabled,
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
        .push_rule(StyleOrigin::Base, type_sel(TYPE_SELECT), select_idle)
        .push_rule(
            StyleOrigin::Base,
            type_state_sel(TYPE_SELECT, PSEUDO_FOCUS_VISIBLE),
            select_focus,
        )
        .push_rule(StyleOrigin::Base, type_sel(TYPE_SLIDER), slider_idle)
        .push_rule(
            StyleOrigin::Base,
            type_state_sel(TYPE_SLIDER, PSEUDO_FOCUS_VISIBLE),
            slider_focus,
        )
        .push_rule(StyleOrigin::Base, type_sel(TYPE_TOGGLE), toggle_idle)
        .build()
}

/// A raised container: gradient fill, a hairline top highlight over a quiet
/// border on the other three sides, and a large corner radius.
fn raised(
    properties: &BuiltInProperties,
    background: understory_style::ResourceKey,
    r: crate::tokens::SignatureResources,
    corner: f64,
) -> understory_style::Style {
    StyleBuilder::new()
        .background_resource(properties, background)
        .set_resource(properties.surface.border_brushes.top, r.hairline)
        .set_resource(properties.surface.border_brushes.right, r.card_border)
        .set_resource(properties.surface.border_brushes.bottom, r.card_border)
        .set_resource(properties.surface.border_brushes.left, r.card_border)
        .border_width(properties, 1.0_f64)
        .corner_radius(properties, corner)
        .build()
}
