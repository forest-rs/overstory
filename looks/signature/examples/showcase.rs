// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A small gallery that installs the Signature look and lays out the
//! first-slice controls so the gradients, glow focus ring, and interaction
//! motion can be seen.
//!
//! Run it with:
//!
//! ```sh
//! cargo run -p overstory_look_signature --example showcase
//! ```
//!
//! Tab between controls to see the focus ring spring out; hover and press the
//! buttons to feel the motion.

use overstory::prelude::*;
use overstory_app::{HostResult, WindowOptions};
use overstory_look_signature::{Scheme, tokens};

fn main() -> HostResult<()> {
    overstory_app::run(
        WindowOptions::new("Signature Showcase").initial_size(520, 600),
        build_ui,
    )
}

fn build_ui() -> Ui {
    let mut ui = Ui::new();
    let _ = overstory_look_signature::install(&mut ui, Scheme::Dark);

    let root = ui.root();
    ui.add_class(root, tokens::CLASS_ROOT);
    ui.set_layout(root, Layout::column(20.0).percent_size(1.0, 1.0));

    ui.with_children(root, |root| {
        root.with_child(
            Panel::column(16.0)
                .class(tokens::CLASS_CARD)
                .layout(Layout::column(16.0).fill_width())
                .padding_all(24.0),
            |card| {
                card.add(Text::new("Signature").class(tokens::CLASS_HEADING));
                card.add(
                    Text::new("A gradient-forward look for Overstory.").class(tokens::CLASS_BODY),
                );

                card.with_child(
                    Panel::row(12.0).layout(Layout::row(12.0).align_center()),
                    |actions| {
                        actions.add(Button::new("Primary"));
                        actions.add(Button::new("Disabled").disabled());
                    },
                );

                card.add(ToggleSwitch::new());
                card.add(CheckBox::new("Subtle gradients").checked(true));
                card.add(CheckBox::new("Glow focus"));
                card.add(
                    RadioGroup::new()
                        .items(["Balanced", "Quality", "Low latency"])
                        .selected_index(0),
                );
                card.add(Slider::new());
                card.add(TextInput::new().placeholder("Type here").fill_width());
            },
        );
    });

    ui
}
