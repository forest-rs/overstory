// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
    missing_docs,
    reason = "criterion bench targets are executable experiments"
)]

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use imaging::record::Scene;
use overstory::{list_scroll_offset_y, set_list_scroll_offset_y};
use overstory_wind_tunnel::scenarios::{
    DASHBOARD_ROWS, LIST_ITEMS, REPEATED_TEXT_LABELS, VIEWPORT, build_dashboard,
    build_virtual_list, layout_repeated_text_labels, paint_repeated_text_labels,
};
use std::hint::black_box;

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

    group.bench_function("scroll_virtual_list_small_deltas", |b| {
        b.iter_batched(
            || {
                let (mut ui, listbox) = build_virtual_list(LIST_ITEMS);
                ui.compute_layout(VIEWPORT);
                (ui, listbox)
            },
            |(mut ui, listbox)| {
                for _ in 0..32 {
                    let offset = list_scroll_offset_y(&ui, listbox).unwrap_or(0.0) + 4.0;
                    let _ = set_list_scroll_offset_y(&mut ui, listbox, offset);
                    ui.compute_layout(VIEWPORT);
                }
                black_box(list_scroll_offset_y(&ui, listbox));
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("layout_repeated_text_labels", |b| {
        b.iter(|| black_box(layout_repeated_text_labels(black_box(REPEATED_TEXT_LABELS))));
    });

    group.bench_function("paint_repeated_text_labels", |b| {
        b.iter(|| black_box(paint_repeated_text_labels(black_box(REPEATED_TEXT_LABELS))));
    });

    group.finish();
}

criterion_group!(benches, bench_ui_pipeline);
criterion_main!(benches);
