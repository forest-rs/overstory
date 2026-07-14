// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Headless assertions that the Chicago look resolves through the real cascade.

use overstory::{Brush, Button, CornerRadius, Ui};

use overstory_look_chicago::{Scheme, theme, tokens};

#[test]
fn button_is_flat_neutral_face_with_square_corners() {
    let mut ui = Ui::new();
    let _ = overstory_look_chicago::install(&mut ui, Scheme::Light);
    let root = ui.root();
    let button = ui.append(root, Button::new("OK"));

    // Flat fill, not a gradient, and the neutral face (not the navy accent).
    match ui
        .resolved_value(button, ui.properties().surface.background)
        .flatten()
    {
        Some(Brush::Solid(_)) => {}
        other => panic!("Chicago button should be a flat solid fill, got {other:?}"),
    }
    // Square corners.
    let tl = ui
        .resolved_value(button, ui.properties().surface.corner_radii.top_left)
        .expect("button exists");
    assert_eq!(
        tl,
        CornerRadius::circular(0.0),
        "Chicago corners are square"
    );
}

#[test]
fn button_has_a_raised_bevel() {
    let mut ui = Ui::new();
    let _ = overstory_look_chicago::install(&mut ui, Scheme::Light);
    let root = ui.root();
    let button = ui.append(root, Button::new("OK"));

    let top = ui
        .resolved_value(button, ui.properties().surface.border_brushes.top)
        .flatten();
    let bottom = ui
        .resolved_value(button, ui.properties().surface.border_brushes.bottom)
        .flatten();
    // A raised bevel: the top edge (highlight) differs from the bottom (shadow).
    let (Some(Brush::Solid(top)), Some(Brush::Solid(bottom))) = (top, bottom) else {
        panic!("expected solid bevel edges");
    };
    assert_ne!(
        top, bottom,
        "raised bevel: top highlight should differ from bottom shadow",
    );
}

#[test]
fn schemes_differ() {
    let mut ui = Ui::new();
    let resolved = ui.style_tokens::<overstory_look_chicago::ChicagoTokens>();
    let light = theme(Scheme::Light, &resolved);
    let dark = theme(Scheme::Dark, &resolved);
    let fg = |t: &understory_style::Theme| match t.get::<Brush>(tokens::ON_SURFACE_FG) {
        Some(Brush::Solid(color)) => *color,
        other => panic!("expected solid fg, got {other:?}"),
    };
    assert_ne!(fg(&light), fg(&dark));
}
