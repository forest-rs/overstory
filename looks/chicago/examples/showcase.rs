// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A small gallery for the Chicago look.
//!
//! ```sh
//! cargo run -p overstory_look_chicago --example showcase
//! ```

use overstory::prelude::*;
use overstory_app::{HostResult, WindowOptions};
use overstory_look_chicago::{Scheme, tokens};

fn main() -> HostResult<()> {
    overstory_app::run(
        WindowOptions::new("Chicago Showcase").initial_size(520, 600),
        build_ui,
    )
}

fn build_ui() -> Ui {
    let mut ui = Ui::new();
    let _ = overstory_look_chicago::install(&mut ui, Scheme::Light);

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
                card.add(Text::new("Chicago").class(tokens::CLASS_HEADING));
                card.add(
                    Text::new("A beveled, square-cornered retro look.").class(tokens::CLASS_BODY),
                );

                card.with_child(
                    Panel::row(12.0).layout(Layout::row(12.0).align_center()),
                    |actions| {
                        actions.add(Button::new("OK"));
                        actions.add(Button::new("Cancel"));
                        actions.add(Button::new("Disabled").disabled());
                    },
                );

                card.add(ToggleSwitch::new());
                card.add(CheckBox::new("Show hidden files").checked(true));
                card.add(CheckBox::new("Read only"));
                card.add(
                    RadioGroup::new()
                        .items(["Tiles", "Icons", "Details"])
                        .selected_index(0),
                );
                card.add(Slider::new());
                card.add(TextInput::new().placeholder("C:\\").fill_width());
            },
        );
    });

    ui
}
