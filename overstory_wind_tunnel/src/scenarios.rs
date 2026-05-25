// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared benchmark and profiling scenarios.

use kurbo::Size;
use overstory::{
    Button, Layout, ListBox, Panel, ProgressBar, Slider, TextInput, TextResources, ToggleSwitch, Ui,
};
use std::sync::OnceLock;

/// Default number of rows in the rich dashboard scenario.
pub const DASHBOARD_ROWS: usize = 64;

/// Default item count in the virtual list scenario.
pub const LIST_ITEMS: usize = 10_000;

/// Default viewport width used by dashboard scenarios.
pub const VIEWPORT_WIDTH: f32 = 960.0;

/// Default viewport size used by UI pipeline scenarios.
pub const VIEWPORT: Size = Size::new(960.0, 720.0);

fn shared_text_resources() -> &'static TextResources {
    static RESOURCES: OnceLock<TextResources> = OnceLock::new();
    RESOURCES.get_or_init(TextResources::system_fonts)
}

/// Builds a cold dashboard tree with several templated controls per row.
///
/// This intentionally includes control initialization and default template
/// expansion so the benchmark reflects the cost of constructing a rich UI
/// surface from application code.
pub fn build_dashboard(rows: usize) -> Ui {
    let mut ui = Ui::with_text_resources(shared_text_resources());
    let root = ui.root();
    let rows = u16::try_from(rows).expect("dashboard row count fits in u16");
    ui.set_layout(
        root,
        Layout::column(8.0)
            .padding_all(12.0)
            .logical_size(VIEWPORT_WIDTH, (f32::from(rows) * 56.0) + 24.0),
    );

    for row in 0..usize::from(rows) {
        let row_panel = ui.append(root, Panel::row(8.0).align_center());
        let value = row as f64 / rows.max(1) as f64;
        ui.append(row_panel, Button::new(format!("Action {row:02}")));
        ui.append(row_panel, ToggleSwitch::new().checked(row % 2 == 0));
        ui.append(row_panel, Slider::new().value(value));
        ui.append(
            row_panel,
            ProgressBar::new()
                .value(value)
                .size(180.0, 18.0)
                .label(row % 3 == 0),
        );
        ui.append(
            row_panel,
            TextInput::new()
                .text(format!("Row {row:02} editable text"))
                .width(220.0),
        );
    }

    ui
}

/// Builds a virtualized list scenario with many logical rows.
pub fn build_virtual_list(items: usize) -> (Ui, overstory::ElementId) {
    let mut ui = Ui::with_text_resources(shared_text_resources());
    let root = ui.root();
    ui.set_layout(root, Layout::column(0.0).logical_size(360.0, 360.0));
    let listbox = ui.append(
        root,
        ListBox::new()
            .items((0..items).map(|index| format!("Item {index:05}")))
            .size(320.0, 320.0)
            .item_extent(28.0)
            .overscan(84.0),
    );
    (ui, listbox)
}
