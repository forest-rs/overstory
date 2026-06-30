// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The smallest end-to-end Overstory app.
//!
//! It shows the whole adopter-facing path in one screen of code:
//!
//! 1. Build a [`Ui`] tree from control specs ([`Panel`], [`Text`], [`Button`]).
//! 2. Hand the builder to [`overstory_app::run`], which selects a native host
//!    (winit by default) and drives the frame loop.
//! 3. React to input through a routed control handler ([`Button::on_activate`])
//!    and push new state back into the tree with [`Ui::set_local`].
//!
//! Run it with:
//!
//! ```sh
//! cargo run --bin hello
//! ```

use std::cell::Cell;
use std::rc::Rc;

use overstory::prelude::*;
use overstory::theme;
use overstory_app::WindowOptions;
use peniko::Brush;

const WINDOW_BG: Color = Color::from_rgba8(0x14, 0x16, 0x1c, 0xff);
const CARD_BG: Color = Color::from_rgba8(0x1d, 0x20, 0x2a, 0xff);
const CARD_BORDER: Color = Color::from_rgba8(0x3a, 0x3f, 0x52, 0xff);
const HEADING_FG: Color = Color::from_rgba8(0xf5, 0xf3, 0xff, 0xff);
const BODY_FG: Color = Color::from_rgba8(0xc2, 0xc6, 0xd6, 0xff);

fn main() -> overstory_app::HostResult<()> {
    overstory_app::run(
        WindowOptions::new("Overstory — Hello").initial_size(420, 320),
        build_ui,
    )
}

/// Builds the retained UI tree. Called once, after the window and GPU surface
/// exist.
fn build_ui() -> Ui {
    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());

    // The root fills the window; center its single child on both axes.
    let root = ui.root();
    ui.set_layout(
        root,
        Layout::column(0.0)
            .percent_size(1.0, 1.0)
            .align_center()
            .justify_center(),
    );
    let surface = ui.properties().surface;
    ui.set_local(root, surface.background, Some(Brush::from(WINDOW_BG)));

    // A content-sized card stacking the heading, the live count, and a button.
    let card = ui.append(
        root,
        Panel::column(16.0)
            .align_center()
            .padding_all(28.0)
            .background(Brush::from(CARD_BG))
            .border_brush(Brush::from(CARD_BORDER))
            .border_width(1.0)
            .corner_radius(16.0),
    );

    ui.append(
        card,
        Text::new("Hello, Overstory")
            .font_size(24.0)
            .foreground(HEADING_FG),
    );

    let count_label = ui.append(
        card,
        Text::new("Clicks: 0").font_size(15.0).foreground(BODY_FG),
    );

    // The button owns its own pressed/focus/activation behavior; we keep the
    // app-level count in a shared cell and rewrite the label on each activation.
    let count = Rc::new(Cell::new(0_i32));
    ui.append(
        card,
        Button::new("Click me")
            .font_size(15.0)
            .layout(Layout::new().logical_size(160.0, 36.0))
            .on_activate(move |ui, _event| {
                let next = count.get() + 1;
                count.set(next);
                let text = ui.properties().text;
                ui.set_local(count_label, text, format!("Clicks: {next}"));
            }),
    );

    ui
}
