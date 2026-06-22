// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Headless assertions that the Signature look resolves through the real
//! cascade + theme path — no renderer required.

use overstory::{Brush, Button, Panel, Rgba8, Ui};
use overstory_look_signature::{
    Scheme, SignatureTokens, design_md, theme, theme_from_design_md, tokens,
};
use understory_style::Theme;

fn solid_rgba(theme: &Theme, key: understory_style::ResourceKey) -> Rgba8 {
    match theme.get::<Brush>(key) {
        Some(Brush::Solid(color)) => color.to_rgba8(),
        other => panic!("expected a solid brush, got {other:?}"),
    }
}

#[test]
fn button_resolves_to_accent_gradient_and_solid_foreground() {
    let mut ui = Ui::new();
    let _ = overstory_look_signature::install(&mut ui, Scheme::Dark);
    let root = ui.root();
    let button = ui.append(root, Button::new("Go"));

    let background = ui.properties().surface.background;
    let foreground = ui.properties().foreground;

    assert!(
        matches!(
            ui.resolved_value(button, background).flatten(),
            Some(Brush::Gradient(_))
        ),
        "button background should be the signature accent gradient",
    );
    assert!(
        matches!(ui.resolved_value(button, foreground), Some(Brush::Solid(_))),
        "button foreground should resolve to a solid on-accent color",
    );
}

#[test]
fn card_class_resolves_to_gradient_with_translucent_hairline() {
    let mut ui = Ui::new();
    let _ = overstory_look_signature::install(&mut ui, Scheme::Dark);
    let root = ui.root();
    let card = ui.append(root, Panel::new().class(tokens::CLASS_CARD));

    let background = ui.properties().surface.background;
    let top_border = ui.properties().surface.border_brushes.top;

    assert!(
        matches!(
            ui.resolved_value(card, background).flatten(),
            Some(Brush::Gradient(_))
        ),
        "card background should be the signature card gradient",
    );
    match ui.resolved_value(card, top_border).flatten() {
        Some(Brush::Solid(color)) => assert!(
            color.to_rgba8().a < 0x40,
            "the card's top edge should be a translucent hairline highlight",
        ),
        other => panic!("expected a translucent hairline, got {other:?}"),
    }
}

#[test]
fn dark_and_light_schemes_differ() {
    let mut ui = Ui::new();
    let resolved = ui.style_tokens::<SignatureTokens>();
    let dark = theme(Scheme::Dark, &resolved);
    let light = theme(Scheme::Light, &resolved);

    assert_ne!(
        solid_rgba(&dark, tokens::ON_SURFACE_FG),
        solid_rgba(&light, tokens::ON_SURFACE_FG),
        "foreground should change between dark and light schemes",
    );
}

#[test]
fn bundled_design_md_theme_matches_default_theme() {
    let mut ui = Ui::new();
    let resolved = ui.style_tokens::<SignatureTokens>();
    let from_defaults = theme(Scheme::Dark, &resolved);
    let from_md = theme_from_design_md(design_md::DEFAULT, Scheme::Dark, &resolved)
        .expect("bundled DESIGN.md parses");

    assert_eq!(
        solid_rgba(&from_defaults, tokens::ON_SURFACE_FG),
        solid_rgba(&from_md, tokens::ON_SURFACE_FG),
        "DESIGN.md-built theme should match the Rust-default theme",
    );
}
