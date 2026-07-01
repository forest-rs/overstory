// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! One source value, fanned out to several bound views.
//!
//! Where `hello` pushed state into the tree by hand from an event handler,
//! this shows the declarative path: a single [`Ui::local_state`] is the source
//! of truth, and every readout below is a one-way binding off it. The slider is
//! the only input adapter — its [`Slider::on_value_changed`] commits the new
//! value into the state. The percentage label, the progress bar, and the
//! qualitative label are all `.map()`-transformed bindings, so nothing needs
//! manual syncing when the value changes.
//!
//! Installing the default theme colors the window and text (see the `hello`
//! example), so this file has no literal colors.
//!
//! Run it with:
//!
//! ```sh
//! cargo run --bin binding
//! ```

use overstory::prelude::*;
use overstory::theme;
use overstory_app::WindowOptions;

fn main() -> overstory_app::HostResult<()> {
    overstory_app::run(
        WindowOptions::new("Overstory — Binding").initial_size(380, 360),
        build_ui,
    )
}

fn build_ui() -> Ui {
    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());

    let root = ui.root();
    ui.set_layout(
        root,
        Layout::column(0.0)
            .percent_size(1.0, 1.0)
            .align_center()
            .justify_center(),
    );

    // The single source of truth. The slider writes here; the views below read
    // from here. `LocalState` is a cheap `Copy` handle, so the closures and
    // bindings can each capture it.
    let amount = ui.local_state(40.0_f64);
    let value_prop = ui.properties().value;

    let stack = ui.append(root, Panel::column(20.0).align_center().padding_all(24.0));

    ui.append(stack, Text::new("Reactive value").font_size(22.0));

    // The one input adapter: commit the slider's value into the source state.
    // The slider self-sizes (track + ticks + labels) from its default template,
    // so there's no layout to hand-tune here.
    ui.append(
        stack,
        Slider::new()
            .range(0.0, 100.0)
            .value(40.0)
            .on_value_changed(move |ui, event| {
                ui.set_local_state(amount, event.value);
            }),
    );

    // Bound readouts — no handlers. Each is a `.map()` off the same source.
    ui.append(
        stack,
        Text::new("")
            .font_size(15.0)
            .bind_text(amount.map(|value| format!("{value:.0} / 100"))),
    );

    // Like the slider, the progress bar self-sizes from its default template —
    // no `.size(...)`. `min`/`max` only set the value range for the fill.
    ui.append(
        stack,
        ProgressBar::new()
            .min(0.0)
            .max(100.0)
            .bind_property(value_prop, amount.binding()),
    );

    ui.append(
        stack,
        Text::new("")
            .font_size(13.0)
            .bind_text(amount.map(|value| level_label(*value).to_string())),
    );

    ui
}

/// Buckets the 0–100 value into a coarse label, purely to show a binding whose
/// mapped output is a different shape than its source.
fn level_label(value: f64) -> &'static str {
    match value {
        v if v < 33.0 => "Low",
        v if v < 66.0 => "Medium",
        _ => "High",
    }
}
