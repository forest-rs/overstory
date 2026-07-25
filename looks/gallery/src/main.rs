// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A live gallery of the Overstory looks.
//!
//! One window, one control set. A dropdown switches the look and a segmented
//! control switches the scheme; both re-install the chosen theme + cascade on
//! the live `Ui` without rebuilding the tree (the looks share chrome class
//! names via `overstory_look::class`).
//!
//! ```sh
//! cargo run -p overstory_look_gallery
//! ```

use std::cell::Cell;
use std::rc::Rc;

use overstory::ChoiceOption;
use overstory::prelude::*;
use overstory_app::{HostResult, WindowOptions};
use overstory_look::class;

#[derive(Clone, Copy, PartialEq, Eq)]
enum LookId {
    Signature,
    Chicago,
    System6,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SchemeId {
    Light,
    Dark,
}

/// Re-installs `look` + `scheme` on the live `Ui`.
fn apply(ui: &mut Ui, look: LookId, scheme: SchemeId) {
    match look {
        LookId::Signature => {
            use overstory_look_signature::{Scheme, install};
            let s = match scheme {
                SchemeId::Light => Scheme::Light,
                SchemeId::Dark => Scheme::Dark,
            };
            install(ui, s);
        }
        LookId::Chicago => {
            use overstory_look_chicago::{Scheme, install};
            let s = match scheme {
                SchemeId::Light => Scheme::Light,
                SchemeId::Dark => Scheme::Dark,
            };
            install(ui, s);
        }
        LookId::System6 => {
            use overstory_look_system6::{Scheme, install};
            let s = match scheme {
                SchemeId::Light => Scheme::Light,
                SchemeId::Dark => Scheme::Dark,
            };
            install(ui, s);
        }
    }
}

fn main() -> HostResult<()> {
    overstory_app::run(
        WindowOptions::new("Overstory Looks").initial_size(560, 680),
        build_ui,
    )
}

fn build_ui() -> Ui {
    let mut ui = Ui::new();
    // Shared selection, captured by both switchers.
    let state = Rc::new(Cell::new((LookId::Signature, SchemeId::Dark)));
    let (look, scheme) = state.get();
    apply(&mut ui, look, scheme);

    let root = ui.root();
    ui.add_class(root, class::ROOT);
    ui.set_layout(root, Layout::column(16.0).percent_size(1.0, 1.0));

    ui.with_children(root, |root| {
        // --- Switch bar ---------------------------------------------------
        root.with_child(
            Panel::row(12.0)
                .class(class::SURFACE)
                .layout(Layout::row(12.0).fill_width().align_center())
                .padding_all(12.0),
            |bar| {
                bar.add(Text::new("Look").class(class::CAPTION));
                let look_state = state.clone();
                bar.add(
                    Select::new()
                        .items(["Signature", "Chicago", "System 6"])
                        .selected_index(0)
                        .on_selection_changed(move |ui, event| {
                            if let Some(index) = event.selected_index {
                                let look = match index {
                                    0 => LookId::Signature,
                                    1 => LookId::Chicago,
                                    _ => LookId::System6,
                                };
                                let (_, scheme) = look_state.get();
                                look_state.set((look, scheme));
                                apply(ui, look, scheme);
                            }
                        }),
                );
                bar.add(Text::new("Scheme").class(class::CAPTION));
                let scheme_state = state.clone();
                bar.add(
                    SegmentedControl::new()
                        .options([ChoiceOption::new("Light"), ChoiceOption::new("Dark")])
                        .selected_index(1)
                        .on_selection_changed(move |ui, event| {
                            if let Some(index) = event.selected_index {
                                let scheme = if index == 0 {
                                    SchemeId::Light
                                } else {
                                    SchemeId::Dark
                                };
                                let (look, _) = scheme_state.get();
                                scheme_state.set((look, scheme));
                                apply(ui, look, scheme);
                            }
                        }),
                );
            },
        );

        // --- Showcase card ------------------------------------------------
        root.with_child(
            Panel::column(14.0)
                .class(class::CARD)
                .layout(Layout::column(14.0).fill_width())
                .padding_all(20.0),
            |card| {
                card.add(Text::new("Overstory Looks").class(class::HEADING));
                card.add(
                    Text::new("One control set, switchable look and scheme.").class(class::BODY),
                );

                card.with_child(
                    Panel::row(12.0).layout(Layout::row(12.0).align_center()),
                    |actions| {
                        actions.add(Button::new("Primary").class(class::PRIMARY));
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
