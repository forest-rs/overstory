// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Headless assertions that the System 6 look resolves through the real cascade.

use overstory::{Brush, Button, CheckBox, ElementSpecExt, Ui};

use overstory_look_system6::{Scheme, templates, theme, tokens};

fn rgba(c: peniko::Color) -> [u8; 4] {
    let c = c.to_rgba8();
    [c.r, c.g, c.b, c.a]
}

/// A foreground role, stored as a bare `Brush`.
fn fg(theme: &understory_style::Theme, key: understory_style::ResourceKey) -> [u8; 4] {
    match theme.get::<Brush>(key) {
        Some(Brush::Solid(c)) => rgba(*c),
        other => panic!("expected a solid foreground brush, got {other:?}"),
    }
}

/// A surface/border role, stored as `Option<Brush>`.
fn surface(theme: &understory_style::Theme, key: understory_style::ResourceKey) -> [u8; 4] {
    match theme.get::<Option<Brush>>(key) {
        Some(Some(Brush::Solid(c))) => rgba(*c),
        other => panic!("expected a solid surface brush, got {other:?}"),
    }
}

#[test]
fn is_one_bit_black_on_white() {
    let mut ui = Ui::new();
    let resolved = ui.style_tokens::<overstory_look_system6::System6Tokens>();
    let light = theme(Scheme::Light, &resolved);
    // Ink is black, paper is white, foreground is the ink.
    assert_eq!(fg(&light, tokens::ON_SURFACE_FG), [0, 0, 0, 0xff]);
    assert_eq!(
        surface(&light, tokens::CONTROL_BG),
        [0xff, 0xff, 0xff, 0xff]
    );
    assert_eq!(surface(&light, tokens::CONTROL_BORDER), [0, 0, 0, 0xff]);
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
        Some(Brush::Solid(c)) => assert_eq!(c.to_rgba8().r, 0xff, "button fill is paper"),
        other => panic!("expected solid paper fill, got {other:?}"),
    }
    match ui
        .resolved_value(button, ui.properties().surface.border_brushes.top)
        .flatten()
    {
        Some(Brush::Solid(c)) => assert_eq!(c.to_rgba8().r, 0x00, "button frame is ink"),
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
    assert_eq!(fg(&light, tokens::ON_SURFACE_FG), [0, 0, 0, 0xff]);
    assert_eq!(fg(&dark, tokens::ON_SURFACE_FG), [0xff, 0xff, 0xff, 0xff]);
}
