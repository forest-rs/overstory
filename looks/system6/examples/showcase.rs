// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A small gallery for the System 6 look.
//!
//! ```sh
//! cargo run -p overstory_look_system6 --example showcase
//! ```

use overstory::prelude::*;
use overstory_app::{HostResult, WindowOptions};
use overstory_look_system6::{Scheme, tokens};

fn main() -> HostResult<()> {
    overstory_app::run(
        WindowOptions::new("System 6 Showcase").initial_size(520, 600),
        build_ui,
    )
}

fn build_ui() -> Ui {
    let mut ui = Ui::new();
    let _ = overstory_look_system6::install(&mut ui, Scheme::Light);

    let root = ui.root();
    ui.add_class(root, tokens::CLASS_ROOT);
    ui.set_layout(root, Layout::column(16.0).percent_size(1.0, 1.0));

    ui.with_children(root, |root| {
        root.with_child(
            Panel::column(14.0)
                .class(tokens::CLASS_CARD)
                .layout(Layout::column(14.0).fill_width())
                .padding_all(20.0),
            |card| {
                card.add(Text::new("System 6").class(tokens::CLASS_HEADING));
                card.add(Text::new("A 1-bit black-and-white classic.").class(tokens::CLASS_BODY));

                card.with_child(
                    Panel::row(12.0).layout(Layout::row(12.0).align_center()),
                    |actions| {
                        actions.add(Button::new("OK"));
                        actions.add(Button::new("Cancel"));
                        actions.add(Button::new("Disabled").disabled());
                    },
                );

                card.add(ToggleSwitch::new());
                card.add(CheckBox::new("Show Clipboard").checked(true));
                card.add(CheckBox::new("Quit when done"));
                card.add(
                    RadioGroup::new()
                        .items(["By Name", "By Date", "By Size"])
                        .selected_index(0),
                );
                card.add(Slider::new());
                card.add(TextInput::new().placeholder("Untitled").fill_width());
            },
        );
    });

    ui
}
