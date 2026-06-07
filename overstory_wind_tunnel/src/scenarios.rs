// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared benchmark and profiling scenarios.

use imaging::record::Scene;
use kurbo::Size;
use overstory::{
    Button, Edges, Layout, ListBox, Panel, ProgressBar, Slider, Text, TextInput,
    TextLayoutCacheStats, TextResources, ToggleSwitch, Ui, UiFrameStats, theme, toggle_checked,
};
use overstory::{list_scroll_offset_y, set_list_scroll_offset_y};
use overstory_host::{FrameDiagnostics, FrameTimings, FrameViewport, UiHost};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Default number of rows in the rich dashboard scenario.
pub const DASHBOARD_ROWS: usize = 64;

/// Default item count in the virtual list scenario.
pub const LIST_ITEMS: usize = 10_000;

/// Default label count in the repeated text scenario.
pub const REPEATED_TEXT_LABELS: usize = 512;

/// Default viewport width used by dashboard scenarios.
pub const VIEWPORT_WIDTH: f32 = 960.0;

/// Default viewport size used by UI pipeline scenarios.
pub const VIEWPORT: Size = Size::new(960.0, 720.0);

/// Default frame count for latency-oriented profile scenarios.
pub const PROFILE_FRAMES: usize = 240;

fn default_frame_viewport() -> FrameViewport {
    FrameViewport::from_logical_size(VIEWPORT, 1.0)
}

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
        Layout::column(8.0).logical_size(VIEWPORT_WIDTH, (f32::from(rows) * 56.0) + 24.0),
    );
    ui.set_box_padding(root, Edges::all(12.0));

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

/// Builds a small toggle scene for animation and retargeting profiles.
pub fn build_toggle_scene() -> (Ui, overstory::ElementId) {
    let mut ui = Ui::with_text_resources(shared_text_resources());
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(
        root,
        Layout::row(12.0).align_center().logical_size(320.0, 96.0),
    );
    ui.set_box_padding(root, Edges::all(16.0));
    let toggle = ui.append(root, ToggleSwitch::new());
    ui.append(root, Text::new("Live rendering").font_size(18.0));
    (ui, toggle)
}

/// Builds a text-heavy tree where many widgets share identical text/style.
pub fn build_repeated_text_labels(labels: usize) -> Ui {
    let mut ui = Ui::with_text_resources(shared_text_resources());
    let root = ui.root();
    let labels = u16::try_from(labels).expect("label count fits in u16");
    ui.set_layout(
        root,
        Layout::column(2.0).logical_size(260.0, (f32::from(labels) * 20.0) + 16.0),
    );
    ui.set_box_padding(root, Edges::all(8.0));

    for _ in 0..usize::from(labels) {
        ui.append(root, Text::new("Shared label").font_size(14.0));
    }

    ui
}

/// Builds the rich dashboard, computes layout, and returns the live element count.
pub fn layout_dashboard(rows: usize) -> usize {
    let mut ui = build_dashboard(rows);
    ui.compute_layout(VIEWPORT);
    ui.len()
}

/// Builds and lays out the rich dashboard, paints it, and returns whether the
/// resulting scene is empty.
pub fn paint_dashboard(rows: usize) -> bool {
    let mut ui = build_dashboard(rows);
    ui.compute_layout(VIEWPORT);
    let mut scene = Scene::new();
    ui.paint(&mut scene);
    scene.is_empty()
}

/// Builds the virtual list, performs the same scroll loop as the Criterion
/// benchmark, and returns the final vertical scroll offset.
pub fn scroll_virtual_list(items: usize) -> f64 {
    let (mut ui, listbox) = build_virtual_list(items);
    for step in 0..16 {
        let offset = f64::from(step) * 192.0;
        let _ = set_list_scroll_offset_y(&mut ui, listbox, offset);
        ui.compute_layout(VIEWPORT);
    }
    list_scroll_offset_y(&ui, listbox).expect("listbox scroll offset should be present")
}

/// Builds and lays out the repeated text scenario, returning text cache stats.
pub fn layout_repeated_text_labels(labels: usize) -> TextLayoutCacheStats {
    let mut ui = build_repeated_text_labels(labels);
    ui.compute_layout(VIEWPORT);
    ui.text_layout_cache_stats()
}

/// Builds, lays out, and paints the repeated text scenario, returning text cache stats.
pub fn paint_repeated_text_labels(labels: usize) -> TextLayoutCacheStats {
    let mut ui = build_repeated_text_labels(labels);
    ui.compute_layout(VIEWPORT);
    let mut scene = Scene::new();
    ui.paint(&mut scene);
    ui.text_layout_cache_stats()
}

/// Aggregated frame timings and counters from a profile run.
#[derive(Clone, Debug, Default)]
pub struct FrameRunSummary {
    /// Number of frames included in the summary.
    pub frames: usize,
    /// Sum of synthetic mutation/input-to-prepared-frame durations.
    pub input_to_frame: Duration,
    /// Slowest synthetic mutation/input-to-prepared-frame duration.
    pub max_input_to_frame: Duration,
    /// Sum of host frame preparation timings by stage.
    pub timings: FrameTimings,
    /// Slowest host frame preparation timings by stage.
    pub max_timings: FrameTimings,
    /// UI counters from the final frame.
    pub last_ui: UiFrameStats,
    /// Number of frames whose output scene was empty.
    pub empty_outputs: usize,
    /// Small checksum used to keep profile loops observable to the optimizer.
    pub checksum: u64,
}

impl FrameRunSummary {
    fn push(
        &mut self,
        diagnostics: FrameDiagnostics,
        input_to_frame: Duration,
        output_empty: bool,
    ) {
        self.frames += 1;
        self.input_to_frame += input_to_frame;
        self.max_input_to_frame = self.max_input_to_frame.max(input_to_frame);
        self.timings.set_now += diagnostics.timings.set_now;
        self.timings.layout += diagnostics.timings.layout;
        self.timings.surface_refresh += diagnostics.timings.surface_refresh;
        self.timings.sync_surfaces += diagnostics.timings.sync_surfaces;
        self.timings.compose += diagnostics.timings.compose;
        self.timings.scale += diagnostics.timings.scale;
        self.timings.total += diagnostics.timings.total;
        self.max_timings.set_now = self.max_timings.set_now.max(diagnostics.timings.set_now);
        self.max_timings.layout = self.max_timings.layout.max(diagnostics.timings.layout);
        self.max_timings.surface_refresh = self
            .max_timings
            .surface_refresh
            .max(diagnostics.timings.surface_refresh);
        self.max_timings.sync_surfaces = self
            .max_timings
            .sync_surfaces
            .max(diagnostics.timings.sync_surfaces);
        self.max_timings.compose = self.max_timings.compose.max(diagnostics.timings.compose);
        self.max_timings.scale = self.max_timings.scale.max(diagnostics.timings.scale);
        self.max_timings.total = self.max_timings.total.max(diagnostics.timings.total);
        self.last_ui = diagnostics.ui;
        self.empty_outputs += usize::from(output_empty);
        self.checksum = self.checksum.wrapping_add(frame_checksum(diagnostics));
    }
}

/// Prepares repeated frames for the rich dashboard scenario.
pub fn prepare_dashboard_frames(rows: usize, frames: usize) -> FrameRunSummary {
    let ui = build_dashboard(rows);
    let mut host = UiHost::new(ui);
    let mut summary = FrameRunSummary::default();
    for _ in 0..frames {
        let input_start = Instant::now();
        let output_empty = host.prepare_frame(default_frame_viewport()).is_empty();
        let input_to_frame = input_start.elapsed();
        summary.push(host.last_frame_diagnostics(), input_to_frame, output_empty);
    }
    summary
}

/// Scrolls a virtual list and prepares a frame after every mutation.
pub fn scroll_virtual_list_frames(items: usize, frames: usize) -> FrameRunSummary {
    let (ui, listbox) = build_virtual_list(items);
    let mut host = UiHost::new(ui);
    let mut summary = FrameRunSummary::default();
    let _ = host.prepare_frame(default_frame_viewport()).is_empty();
    for step in 0..frames {
        host.ui_mut().reset_frame_stats();
        let input_start = Instant::now();
        let offset = step as f64 * 96.0;
        let _ = set_list_scroll_offset_y(host.ui_mut(), listbox, offset);
        let output_empty = host
            .prepare_frame_preserving_stats(default_frame_viewport())
            .is_empty();
        let input_to_frame = input_start.elapsed();
        summary.push(host.last_frame_diagnostics(), input_to_frame, output_empty);
    }
    summary
}

/// Repeatedly retargets a toggle transition and prepares a frame after every
/// mutation.
pub fn toggle_retarget_frames(frames: usize) -> FrameRunSummary {
    let (ui, toggle) = build_toggle_scene();
    let mut host = UiHost::new(ui);
    let mut summary = FrameRunSummary::default();
    let viewport = FrameViewport::from_logical_size(Size::new(320.0, 96.0), 1.0);
    let _ = host.prepare_frame(viewport).is_empty();
    for _ in 0..frames {
        host.ui_mut().reset_frame_stats();
        let input_start = Instant::now();
        toggle_checked(host.ui_mut(), toggle);
        let output_empty = host.prepare_frame_preserving_stats(viewport).is_empty();
        let input_to_frame = input_start.elapsed();
        summary.push(host.last_frame_diagnostics(), input_to_frame, output_empty);
    }
    summary
}

fn frame_checksum(diagnostics: FrameDiagnostics) -> u64 {
    let stats = diagnostics.ui;
    (stats.elements as u64)
        ^ ((stats.layout_passes as u64) << 8)
        ^ ((stats.painted_elements as u64) << 16)
        ^ ((stats.rebuilt_surfaces as u64) << 24)
        ^ ((diagnostics.output_empty as u64) << 63)
}
