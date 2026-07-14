// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Headless assertions that the System 6 look resolves through the real cascade.

use overstory::color::palette::css;
use overstory::{Brush, Button, CheckBox, Color, ElementSpecExt, Ui};

use overstory_look_system6::{Scheme, templates, theme, tokens};

/// A foreground role, stored as a bare `Brush`.
fn fg(theme: &understory_style::Theme, key: understory_style::ResourceKey) -> Color {
    match theme.get::<Brush>(key) {
        Some(Brush::Solid(color)) => *color,
        other => panic!("expected a solid foreground brush, got {other:?}"),
    }
}

/// A surface/border role, stored as `Option<Brush>`.
fn surface(theme: &understory_style::Theme, key: understory_style::ResourceKey) -> Color {
    match theme.get::<Option<Brush>>(key) {
        Some(Some(Brush::Solid(color))) => *color,
        other => panic!("expected a solid surface brush, got {other:?}"),
    }
}

#[test]
fn is_one_bit_black_on_white() {
    let mut ui = Ui::new();
    let resolved = ui.style_tokens::<overstory_look_system6::System6Tokens>();
    let light = theme(Scheme::Light, &resolved);
    // Ink is black, paper is white, foreground is the ink.
    assert_eq!(fg(&light, tokens::ON_SURFACE_FG), css::BLACK);
    assert_eq!(surface(&light, tokens::CONTROL_BG), css::WHITE);
    assert_eq!(surface(&light, tokens::CONTROL_BORDER), css::BLACK);
}

#[test]
fn button_is_paper_with_solid_ink_frame() {
    let mut ui = Ui::new();
    let _ = overstory_look_system6::install(&mut ui, Scheme::Light);
    let root = ui.root();
    let button = ui.append(root, Button::new("OK"));

    match ui
        .resolved_value(button, ui.properties().surface.background)
        .flatten()
    {
        Some(Brush::Solid(color)) => assert_eq!(color, css::WHITE, "button fill is paper"),
        other => panic!("expected solid paper fill, got {other:?}"),
    }
    match ui
        .resolved_value(button, ui.properties().surface.border_brushes.top)
        .flatten()
    {
        Some(Brush::Solid(color)) => assert_eq!(color, css::BLACK, "button frame is ink"),
        other => panic!("expected solid ink frame, got {other:?}"),
    }
}

#[test]
fn checkbox_uses_the_look_authored_template() {
    // Proof a look can own a control's structure: the cascade swaps the
    // checkbox's control_template to the look's own, overriding the framework
    // default, and the checkbox still expands without error.
    let mut ui = Ui::new();
    let _ = overstory_look_system6::install(&mut ui, Scheme::Light);
    let root = ui.root();
    let checkbox = ui.append(root, CheckBox::new("Show Clipboard").checked(true));

    let resolved = ui
        .resolved_value(checkbox, ui.properties().control_template)
        .flatten();
    assert_eq!(
        resolved,
        Some(templates::check_box()),
        "the System 6 cascade should install its own checkbox template",
    );
}

#[test]
fn primary_button_uses_the_double_border_template() {
    // A class-driven template swap: a `.primary` button gets System 6's
    // double-frame default-button template (which styling alone can't produce).
    let mut ui = Ui::new();
    let _ = overstory_look_system6::install(&mut ui, Scheme::Light);
    let root = ui.root();
    let button = ui.append(root, Button::new("OK").class(tokens::CLASS_PRIMARY));

    let resolved = ui
        .resolved_value(button, ui.properties().control_template)
        .flatten();
    assert_eq!(
        resolved,
        Some(templates::default_button()),
        "a primary button should get the System 6 double-border template",
    );
}

#[test]
fn schemes_invert() {
    let mut ui = Ui::new();
    let resolved = ui.style_tokens::<overstory_look_system6::System6Tokens>();
    let light = theme(Scheme::Light, &resolved);
    let dark = theme(Scheme::Dark, &resolved);
    // Foreground ink flips from black to white between the schemes.
    assert_eq!(fg(&light, tokens::ON_SURFACE_FG), css::BLACK);
    assert_eq!(fg(&dark, tokens::ON_SURFACE_FG), css::WHITE);
}
