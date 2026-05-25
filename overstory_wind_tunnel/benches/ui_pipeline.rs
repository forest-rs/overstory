// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
    missing_docs,
    reason = "criterion bench targets are executable experiments"
)]

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use imaging::record::Scene;
use kurbo::Size;
use overstory::{
    Button, Layout, ListBox, Panel, ProgressBar, Slider, TextInput, ToggleSwitch, Ui,
    list_scroll_offset_y, set_list_scroll_offset_y,
};
use std::hint::black_box;

const DASHBOARD_ROWS: usize = 64;
const LIST_ITEMS: usize = 10_000;
const VIEWPORT_WIDTH: f32 = 960.0;
const VIEWPORT: Size = Size::new(960.0, 720.0);

fn bench_ui_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("ui_pipeline");

    group.bench_function("build_dashboard", |b| {
        b.iter(|| black_box(build_dashboard(black_box(DASHBOARD_ROWS))));
    });

    group.bench_function("layout_dashboard", |b| {
        b.iter_batched(
            || build_dashboard(DASHBOARD_ROWS),
            |mut ui| {
                ui.compute_layout(VIEWPORT);
                black_box(ui.len());
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("paint_dashboard", |b| {
        b.iter_batched(
            || {
                let mut ui = build_dashboard(DASHBOARD_ROWS);
                ui.compute_layout(VIEWPORT);
                ui
            },
            |mut ui| {
                let mut scene = Scene::new();
                ui.paint(&mut scene);
                black_box(scene.is_empty());
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("scroll_virtual_list", |b| {
        b.iter_batched(
            || build_virtual_list(LIST_ITEMS),
            |(mut ui, listbox)| {
                for step in 0..16 {
                    let offset = f64::from(step) * 192.0;
                    let _ = set_list_scroll_offset_y(&mut ui, listbox, offset);
                    ui.compute_layout(VIEWPORT);
                }
                black_box(list_scroll_offset_y(&ui, listbox));
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn build_dashboard(rows: usize) -> Ui {
    let mut ui = Ui::new();
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

fn build_virtual_list(items: usize) -> (Ui, overstory::ElementId) {
    let mut ui = Ui::new();
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

criterion_group!(benches, bench_ui_pipeline);
criterion_main!(benches);
