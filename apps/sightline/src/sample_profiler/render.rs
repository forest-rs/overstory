// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::ops::Range;

use overstory::{ElementId, Layout, LayoutDimension, LayoutSize, Ui};
use peniko::{Brush, Color};
use understory_axis::{
    AxisMajorStepLadder, AxisMapping1D, AxisRuler1D, AxisRulerOptions, AxisScale1D,
    AxisScaleOptions, AxisSubdivisionPolicy, AxisTickKind,
};
use understory_timeline_model::{LaneId, TimelineMarker, TimelineSpan};
use understory_view2d::Viewport1D;

use super::view::{
    SampleCallout, SampleProfilerView, TimelineLaneView, TimelineMarkerView, TimelineSpanView,
};
use super::{
    AXIS_HEIGHT, OVERVIEW_BAR_BASELINE_Y, OVERVIEW_BAR_MAX_HEIGHT, OVERVIEW_HEIGHT, OVERVIEW_Y,
    PLOT_HEIGHT, PLOT_WIDTH, SampleProfilerState, TIME_VIEW_WIDTH, TIMELINE_LANE_HEIGHT,
    TIMELINE_LANE_Y, TIMELINE_SPAN_DEPTH_OFFSET, TIMELINE_SPAN_HEIGHT, TRACK_LABEL_WIDTH,
    format_duration_us, format_time_us, timestamp_from_world_x,
};

const CALLOUT_WIDTH: f64 = 172.0;
const CALLOUT_HEIGHT: f64 = 86.0;
const CALLOUT_GAP: f64 = 12.0;
const CALLOUT_TOP: f64 = 38.0;
const HIDDEN_CALLOUT_X: f64 = -256.0;
const HIDDEN_CALLOUT_Y: f64 = -256.0;
const HIDDEN_SPAN_X: f64 = -512.0;
const HIDDEN_SPAN_Y: f64 = -512.0;
const OVERVIEW_WINDOW_BG: Color = Color::from_rgba8(0x60, 0xa5, 0xfa, 0x29);

pub(super) fn render(ui: &mut Ui, view: &SampleProfilerView, state: &SampleProfilerState) {
    let viewport = state.viewport.borrow();
    let visible = viewport.visible_world_range();
    let start = timestamp_from_world_x(visible.start);
    let end = timestamp_from_world_x(visible.end).max(start.saturating_add(1));
    let span = end.saturating_sub(start);
    let overview_buckets = state
        .data
        .series
        .range_envelope(
            state.data.time_range.start,
            state.data.time_range.end,
            view.bars.len(),
        )
        .collect::<Vec<_>>();
    let visible_buckets = state
        .data
        .series
        .range_envelope(start, end, view.bars.len())
        .collect::<Vec<_>>();
    let visible_samples = visible_buckets
        .iter()
        .map(|bucket| bucket.len)
        .sum::<usize>();
    let visible_bucket_count = visible_buckets
        .iter()
        .filter(|bucket| bucket.len > 0)
        .count();

    for (index, bar) in view.bars.iter().enumerate() {
        let Some(bucket) = overview_buckets.get(index) else {
            hide_bar(ui, *bar);
            continue;
        };
        let Some(max) = bucket.max else {
            hide_bar(ui, *bar);
            continue;
        };

        let x0 = overview_plot_x(state, bucket.start as f64);
        let x1 = overview_plot_x(state, bucket.end as f64).max(x0 + 1.0);
        let height =
            snap_plot_extent((max / state.data.peak_ms).clamp(0.0, 1.0) * OVERVIEW_BAR_MAX_HEIGHT)
                .max(1.0);
        let y = OVERVIEW_BAR_BASELINE_Y - height;

        ui.set_layout(
            *bar,
            Layout::new()
                .absolute_at(x0, y)
                .logical_size(x1 - x0, height),
        );
    }

    render_ticks(ui, view, &viewport);
    render_overview_window(ui, view, state, &viewport);
    render_lanes(ui, view, state);
    render_spans(ui, view, state, &viewport, start..end);
    render_markers(ui, view, state, &viewport, start..end);
    render_selection(ui, view, state, &viewport, start..end);
    ui.set_local(
        view.summary_label,
        ui.properties().text,
        format!(
            "{} samples / peak {:.1}ms",
            state.data.series.len(),
            state.data.peak_ms
        ),
    );
    ui.set_local(
        view.range_label,
        ui.properties().text,
        format!(
            "{} visible / {} buckets / {} samples",
            format_duration_us(span),
            visible_bucket_count,
            visible_samples
        ),
    );
}

fn render_ticks(ui: &mut Ui, view: &SampleProfilerView, viewport: &Viewport1D) {
    let visible = viewport.visible_world_range();
    let mapping = AxisMapping1D::linear(0.0..TIME_VIEW_WIDTH, visible);
    let scale = AxisScale1D::from_mapping(
        &mapping,
        AxisScaleOptions {
            target_major_spacing_px: 160.0,
            min_major_step: 1_000.0,
            medium_label_min_spacing_px: 260.0,
            major_step_ladder: AxisMajorStepLadder::TimeLike {
                units_per_second: 1_000_000.0,
            },
            subdivision_policy: AxisSubdivisionPolicy::Auto,
        },
    );
    let ruler = AxisRuler1D::from_mapping(
        &mapping,
        &scale,
        AxisRulerOptions {
            major_mark_extent: PLOT_HEIGHT - AXIS_HEIGHT,
            medium_mark_extent: PLOT_HEIGHT - AXIS_HEIGHT,
            minor_mark_extent: 0.0,
        },
    );
    let marks = ruler
        .marks()
        .iter()
        .filter(|mark| mark.kind != AxisTickKind::Minor)
        .take(view.ticks.len());

    let mut used = 0;
    for (tick, mark) in view.ticks.iter().zip(marks) {
        used += 1;
        let x = time_plot_x(mark.view_position);
        ui.set_layout(
            tick.line,
            Layout::new()
                .absolute_at(x, AXIS_HEIGHT)
                .logical_size(1.0, snap_plot_extent(mark.mark_extent)),
        );
        if mark.labeled {
            let label_x = snap_plot_x((x - 24.0).clamp(TRACK_LABEL_WIDTH, PLOT_WIDTH - 48.0));
            ui.set_layout(
                tick.label,
                Layout::new()
                    .absolute_at(label_x, 8.0)
                    .size(LayoutSize::new().width(LayoutDimension::Logical(64.0))),
            );
            ui.set_local(
                tick.label,
                ui.properties().text,
                format_time_us(timestamp_from_world_x(mark.value)),
            );
        } else {
            hide_label(ui, tick.label);
        }
    }

    for tick in view.ticks.iter().skip(used) {
        hide_tick(ui, tick.line, tick.label);
    }
}

fn render_lanes(ui: &mut Ui, view: &SampleProfilerView, state: &SampleProfilerState) {
    let mut lanes = view.lanes.iter();
    if let Some(sample_lane) = lanes.next() {
        render_lane_rule(ui, *sample_lane, "overview", OVERVIEW_Y, OVERVIEW_HEIGHT);
    }

    let timeline_lanes = state.timeline.lanes_with_ids();
    let mut used = 1;
    for (lane_view, (lane_id, lane)) in lanes.zip(timeline_lanes) {
        used += 1;
        let lane_y = lane_y(lane_id);
        render_lane_rule(
            ui,
            *lane_view,
            lane.label.as_str(),
            lane_y,
            TIMELINE_LANE_HEIGHT,
        );
    }

    for lane in view.lanes.iter().skip(used) {
        hide_lane(ui, *lane);
    }
}

fn render_overview_window(
    ui: &mut Ui,
    view: &SampleProfilerView,
    state: &SampleProfilerState,
    viewport: &Viewport1D,
) {
    let visible = viewport.visible_world_range();
    let x0 = overview_plot_x(state, visible.start);
    let x1 = overview_plot_x(state, visible.end).max(x0 + 1.0);
    ui.set_layout(
        view.overview_window,
        Layout::new()
            .absolute_at(x0, OVERVIEW_Y)
            .logical_size(x1 - x0, OVERVIEW_HEIGHT),
    );
    ui.set_local(
        view.overview_window,
        ui.properties().surface.background,
        Some(Brush::Solid(OVERVIEW_WINDOW_BG)),
    );
}

fn render_lane_rule(ui: &mut Ui, lane: TimelineLaneView, label: &str, y: f64, height: f64) {
    ui.set_layout(
        lane.label,
        Layout::new()
            .absolute_at(10.0, y + (height * 0.5) - 7.0)
            .size(LayoutSize::new().width(LayoutDimension::Logical(TRACK_LABEL_WIDTH - 16.0))),
    );
    ui.set_local(lane.label, ui.properties().text, String::from(label));
    ui.set_layout(
        lane.rule,
        Layout::new()
            .absolute_at(0.0, y + height)
            .logical_size(PLOT_WIDTH, 1.0),
    );
}

fn hide_lane(ui: &mut Ui, lane: TimelineLaneView) {
    hide_label(ui, lane.label);
    ui.set_layout(
        lane.rule,
        Layout::new().absolute_at(0.0, 0.0).logical_size(0.0, 0.0),
    );
}

fn render_spans(
    ui: &mut Ui,
    view: &SampleProfilerView,
    state: &SampleProfilerState,
    viewport: &Viewport1D,
    visible: Range<u64>,
) {
    let mut used = 0;
    for (span_view, (index, span)) in view
        .spans
        .iter()
        .zip(state.timeline.spans().iter().enumerate())
    {
        used += 1;
        if span.end < visible.start as f64 || span.start > visible.end as f64 {
            hide_span(ui, *span_view);
            continue;
        }

        let x0 = world_plot_x(viewport, span.start);
        let x1 = world_plot_x(viewport, span.end).max(x0 + 1.0);
        let width = x1 - x0;
        let y = span_y(span);
        ui.set_layout(
            span_view.panel,
            Layout::row(0.0)
                .absolute_at(x0, y)
                .logical_size(width, TIMELINE_SPAN_HEIGHT),
        );
        ui.set_local(
            span_view.panel,
            ui.properties().surface.background,
            Some(Brush::Solid(span_color(index))),
        );
        ui.set_local(
            span_view.label,
            ui.properties().text,
            if width >= 48.0 {
                String::from(span.label.as_str())
            } else {
                String::new()
            },
        );
    }

    for span in view.spans.iter().skip(used) {
        hide_span(ui, *span);
    }
}

fn hide_span(ui: &mut Ui, span: TimelineSpanView) {
    ui.set_layout(
        span.panel,
        Layout::row(0.0)
            .absolute_at(HIDDEN_SPAN_X, HIDDEN_SPAN_Y)
            .logical_size(0.0, 0.0),
    );
    ui.set_local(
        span.panel,
        ui.properties().surface.background,
        Option::<Brush>::None,
    );
    ui.set_local(span.label, ui.properties().text, String::new());
}

fn render_markers(
    ui: &mut Ui,
    view: &SampleProfilerView,
    state: &SampleProfilerState,
    viewport: &Viewport1D,
    visible: Range<u64>,
) {
    let mut used = 0;
    for (marker_view, (index, marker)) in view
        .markers
        .iter()
        .zip(state.timeline.markers().iter().enumerate())
    {
        used += 1;
        if marker.time < visible.start as f64 || marker.time > visible.end as f64 {
            hide_marker(ui, *marker_view);
            continue;
        }

        let x = world_plot_x(viewport, marker.time);
        let (y, height, label_y) = marker_geometry(marker);
        ui.set_layout(
            marker_view.line,
            Layout::new().absolute_at(x, y).logical_size(2.0, height),
        );
        ui.set_local(
            marker_view.line,
            ui.properties().surface.background,
            Some(Brush::Solid(marker_color(index))),
        );
        ui.set_layout(
            marker_view.label,
            Layout::new()
                .absolute_at((x + 4.0).min(PLOT_WIDTH - 72.0), label_y)
                .size(LayoutSize::new().width(LayoutDimension::Logical(72.0))),
        );
        ui.set_local(
            marker_view.label,
            ui.properties().text,
            String::from(marker.label.as_str()),
        );
    }

    for marker in view.markers.iter().skip(used) {
        hide_marker(ui, *marker);
    }
}

fn hide_marker(ui: &mut Ui, marker: TimelineMarkerView) {
    ui.set_layout(
        marker.line,
        Layout::new().absolute_at(0.0, 0.0).logical_size(0.0, 0.0),
    );
    ui.set_local(
        marker.line,
        ui.properties().surface.background,
        Option::<Brush>::None,
    );
    hide_label(ui, marker.label);
}

fn render_selection(
    ui: &mut Ui,
    view: &SampleProfilerView,
    state: &SampleProfilerState,
    viewport: &Viewport1D,
    visible: Range<u64>,
) {
    let Some(entry) = state.selected_entry() else {
        hide_selection(ui, view);
        return;
    };
    if entry.timestamp < visible.start || entry.timestamp >= visible.end {
        hide_selection(ui, view);
        return;
    }

    let x = world_plot_x(viewport, entry.timestamp as f64);
    ui.set_layout(
        view.selection_marker,
        Layout::new()
            .absolute_at(x, 0.0)
            .logical_size(2.0, PLOT_HEIGHT),
    );
    ui.set_local(
        view.selection_label,
        ui.properties().text,
        format!("{:.2}ms @ {}", entry.value, format_time_us(entry.timestamp)),
    );
    show_callout(
        ui,
        view.callout,
        x,
        entry.timestamp,
        entry.value,
        state.data.peak_ms,
    );
}

fn snap_plot_x(value: f64) -> f64 {
    snap_logical_px(value).clamp(0.0, PLOT_WIDTH)
}

fn time_plot_x(view_x: f64) -> f64 {
    snap_plot_x(TRACK_LABEL_WIDTH + view_x.clamp(0.0, TIME_VIEW_WIDTH))
}

fn world_plot_x(viewport: &Viewport1D, time: f64) -> f64 {
    time_plot_x(viewport.world_to_view_x(time))
}

fn overview_plot_x(state: &SampleProfilerState, time: f64) -> f64 {
    let range = &state.data.time_range;
    let duration = range.end.saturating_sub(range.start).max(1) as f64;
    let normalized = ((time - range.start as f64) / duration).clamp(0.0, 1.0);
    time_plot_x(normalized * TIME_VIEW_WIDTH)
}

fn snap_plot_extent(value: f64) -> f64 {
    snap_logical_px(value).clamp(0.0, PLOT_HEIGHT)
}

fn lane_y(lane: LaneId) -> f64 {
    TIMELINE_LANE_Y + lane.index() as f64 * TIMELINE_LANE_HEIGHT
}

fn span_y(span: &TimelineSpan) -> f64 {
    lane_y(span.lane) + 12.0 + span.depth as f64 * TIMELINE_SPAN_DEPTH_OFFSET
}

fn marker_geometry(marker: &TimelineMarker) -> (f64, f64, f64) {
    if let Some(lane) = marker.lane {
        let y = lane_y(lane);
        return (y, TIMELINE_LANE_HEIGHT, y + 4.0);
    }

    (AXIS_HEIGHT, PLOT_HEIGHT - AXIS_HEIGHT, 8.0)
}

fn span_color(index: usize) -> Color {
    const COLORS: [Color; 6] = [
        Color::from_rgba8(0x2f, 0x6b, 0x8f, 0xff),
        Color::from_rgba8(0x2b, 0x7a, 0x78, 0xff),
        Color::from_rgba8(0x6c, 0x4f, 0x8e, 0xff),
        Color::from_rgba8(0x8b, 0x8e, 0x3a, 0xff),
        Color::from_rgba8(0x96, 0x57, 0x4d, 0xff),
        Color::from_rgba8(0x3e, 0x7b, 0x5c, 0xff),
    ];
    COLORS[index % COLORS.len()]
}

fn marker_color(index: usize) -> Color {
    const COLORS: [Color; 5] = [
        Color::from_rgba8(0xfd, 0xe6, 0x8a, 0xff),
        Color::from_rgba8(0xfc, 0xa5, 0xa5, 0xff),
        Color::from_rgba8(0xfd, 0xba, 0x74, 0xff),
        Color::from_rgba8(0x86, 0xef, 0xac, 0xff),
        Color::from_rgba8(0x93, 0xc5, 0xfd, 0xff),
    ];
    COLORS[index % COLORS.len()]
}

fn snap_logical_px(value: f64) -> f64 {
    if value.is_finite() {
        value.round().clamp(-16_384.0, 16_384.0)
    } else {
        0.0
    }
}

fn hide_bar(ui: &mut Ui, bar: ElementId) {
    ui.set_layout(
        bar,
        Layout::new()
            .absolute_at(0.0, PLOT_HEIGHT)
            .logical_size(0.0, 0.0),
    );
}

fn hide_tick(ui: &mut Ui, line: ElementId, label: ElementId) {
    ui.set_layout(
        line,
        Layout::new().absolute_at(0.0, 0.0).logical_size(0.0, 0.0),
    );
    hide_label(ui, label);
}

fn hide_label(ui: &mut Ui, label: ElementId) {
    ui.set_layout(
        label,
        Layout::new()
            .absolute_at(0.0, 0.0)
            .size(LayoutSize::new().width(LayoutDimension::Logical(0.0))),
    );
    ui.set_local(label, ui.properties().text, String::new());
}

fn hide_selection(ui: &mut Ui, view: &SampleProfilerView) {
    ui.set_layout(
        view.selection_marker,
        Layout::new()
            .absolute_at(0.0, 0.0)
            .logical_size(0.0, PLOT_HEIGHT),
    );
    hide_callout(ui, view.callout);
    ui.set_local(
        view.selection_label,
        ui.properties().text,
        String::from("click plot to select"),
    );
}

fn show_callout(
    ui: &mut Ui,
    callout: SampleCallout,
    marker_x: f64,
    timestamp: u64,
    duration_ms: f64,
    peak_ms: f64,
) {
    let x = callout_x(marker_x);
    ui.set_layout(
        callout.panel,
        Layout::column(4.0)
            .absolute_at(x, CALLOUT_TOP)
            .logical_size(CALLOUT_WIDTH, CALLOUT_HEIGHT),
    );
    ui.set_local(
        callout.title,
        ui.properties().text,
        String::from("selected sample"),
    );
    ui.set_local(
        callout.time,
        ui.properties().text,
        format!("time {}", format_time_us(timestamp)),
    );
    ui.set_local(
        callout.duration,
        ui.properties().text,
        format!("duration {duration_ms:.2}ms"),
    );
    ui.set_local(
        callout.peak,
        ui.properties().text,
        format!("{} of peak", format_peak_ratio(duration_ms, peak_ms)),
    );
}

fn hide_callout(ui: &mut Ui, callout: SampleCallout) {
    ui.set_layout(
        callout.panel,
        Layout::column(4.0)
            .absolute_at(HIDDEN_CALLOUT_X, HIDDEN_CALLOUT_Y)
            .logical_size(0.0, 0.0),
    );
    ui.set_local(callout.title, ui.properties().text, String::new());
    ui.set_local(callout.time, ui.properties().text, String::new());
    ui.set_local(callout.duration, ui.properties().text, String::new());
    ui.set_local(callout.peak, ui.properties().text, String::new());
}

fn callout_x(marker_x: f64) -> f64 {
    let right_x = marker_x + CALLOUT_GAP;
    if right_x + CALLOUT_WIDTH <= PLOT_WIDTH {
        return right_x;
    }

    (marker_x - CALLOUT_WIDTH - CALLOUT_GAP).clamp(0.0, PLOT_WIDTH - CALLOUT_WIDTH)
}

fn format_peak_ratio(duration_ms: f64, peak_ms: f64) -> String {
    if peak_ms <= 0.0 {
        return String::from("0%");
    }
    format!("{:.0}%", (duration_ms / peak_ms).clamp(0.0, 1.0) * 100.0)
}
