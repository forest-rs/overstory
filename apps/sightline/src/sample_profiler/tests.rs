// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::rc::Rc;

use overstory::{Brush, Layout, Size, Ui};

use super::*;
use crate::app::AppState;
use crate::style::{SightlineStyleTokens, sightline_cascade, sightline_theme};

#[test]
fn viewport_selection_uses_chronostore_nearest_sample() {
    let state = SampleProfilerState::new();
    state.select_at_view_x(f64::from(PLOT_WIDTH) * 0.5);
    let selected = state
        .selected_entry()
        .expect("selection should map to a retained sample");

    let visible = state.viewport.borrow().visible_world_range();
    assert!(selected.timestamp >= timestamp_from_world_x(visible.start));
    assert!(selected.timestamp <= timestamp_from_world_x(visible.end));
}

#[test]
fn plot_click_selects_a_sample_and_updates_label() {
    let (mut ui, view, app_state) = build_wired_test_profiler();

    let click = ui.effective_bounds(view.plot).center();
    ui.synthesize_click(view.plot, click);

    assert!(
        app_state.sample_profiler().selected_entry().is_some(),
        "plot click should select a chronostore sample"
    );
    let label = ui
        .resolved_value(view.selection_label, ui.properties().text)
        .expect("selection label should receive selected text");
    assert!(label.contains("ms @"));
}

#[test]
fn plot_click_shows_sample_callout() {
    let (mut ui, view, _app_state) = build_wired_test_profiler();

    let click = ui.effective_bounds(view.plot).center();
    ui.synthesize_click(view.plot, click);
    ui.compute_layout(Size::new(1_100.0, 520.0));

    let callout = ui.effective_bounds(view.callout.panel);
    assert!(callout.width() > 0.0, "selection should show the callout");
    assert!(callout.height() > 0.0, "selection should show the callout");

    let title = ui
        .resolved_value::<String>(view.callout.title, ui.properties().text)
        .expect("callout title should be populated");
    assert_eq!(title, "selected sample");

    let duration = ui
        .resolved_value::<String>(view.callout.duration, ui.properties().text)
        .expect("callout duration should be populated");
    assert!(duration.starts_with("duration "));

    let title_bounds = ui.effective_bounds(view.callout.title);
    let time_bounds = ui.effective_bounds(view.callout.time);
    let duration_bounds = ui.effective_bounds(view.callout.duration);
    let peak_bounds = ui.effective_bounds(view.callout.peak);
    assert!(title_bounds.y0 < time_bounds.y0);
    assert!(time_bounds.y0 < duration_bounds.y0);
    assert!(duration_bounds.y0 < peak_bounds.y0);
}

#[test]
fn sample_callout_is_hidden_before_selection() {
    let (ui, view) = build_test_profiler();

    let plot = ui.effective_bounds(view.plot);
    let callout = ui.effective_bounds(view.callout.panel);
    assert!(
        callout.x1 <= plot.x0 || callout.y1 <= plot.y0,
        "hidden callout should not paint inside the plot"
    );

    let title = ui
        .resolved_value::<String>(view.callout.title, ui.properties().text)
        .expect("callout title should have a text value");
    assert!(title.is_empty());
}

#[test]
fn sample_callout_stays_inside_plot() {
    let (mut ui, view, app_state) = build_wired_test_profiler();

    app_state
        .sample_profiler()
        .select_at_view_x(f64::from(PLOT_WIDTH) - 1.0);
    render::render(&mut ui, &view, app_state.sample_profiler());
    ui.compute_layout(Size::new(1_100.0, 520.0));

    let plot = ui.effective_bounds(view.plot);
    let callout = ui.effective_bounds(view.callout.panel);
    assert!(callout.x0 >= plot.x0 - 0.001);
    assert!(callout.x1 <= plot.x1 + 0.001);
}

#[test]
fn visible_chronostore_samples_render_as_bars() {
    let (ui, view) = build_test_profiler();

    let visible_bars = view
        .bars
        .iter()
        .copied()
        .filter(|bar| {
            let bounds = ui.effective_bounds(*bar);
            bounds.width() > 0.0 && bounds.height() > 0.0
        })
        .collect::<Vec<_>>();
    assert!(
        !visible_bars.is_empty(),
        "seeded Chronostore samples should produce visible bars"
    );
    let bar_background = ui
        .resolved_value::<Option<Brush>>(visible_bars[0], ui.properties().surface.background)
        .flatten();
    assert!(
        bar_background.is_some(),
        "visible sample bars should resolve a paintable background"
    );

    let summary = ui
        .resolved_value(view.summary_label, ui.properties().text)
        .expect("sample summary should be rendered");
    assert!(summary.contains("420 samples"));

    let range = ui
        .resolved_value(view.range_label, ui.properties().text)
        .expect("visible range summary should be rendered");
    assert!(range.contains("buckets"));
    assert!(!range.contains("0 buckets"));
}

#[test]
fn timeline_model_lanes_render_below_sample_lane() {
    let (ui, view) = build_test_profiler();

    let labels = view
        .lanes
        .iter()
        .filter_map(|lane| ui.resolved_value::<String>(lane.label, ui.properties().text))
        .collect::<Vec<_>>();
    assert!(labels.iter().any(|label| label == "overview"));
    assert!(labels.iter().any(|label| label == "ui thread"));
    assert!(labels.iter().any(|label| label == "renderer"));
}

#[test]
fn visible_timeline_model_spans_render_in_time_region() {
    let (ui, view) = build_test_profiler();
    let plot = ui.effective_bounds(view.plot);
    let time_region_x0 = plot.x0 + f64::from(TRACK_LABEL_WIDTH);

    let visible_spans = view
        .spans
        .iter()
        .copied()
        .filter(|span| {
            let bounds = ui.effective_bounds(span.panel);
            bounds.width() > 0.0
                && bounds.height() > 0.0
                && bounds.x1 > plot.x0
                && bounds.x0 < plot.x1
                && bounds.y1 > plot.y0
                && bounds.y0 < plot.y1
        })
        .collect::<Vec<_>>();
    assert!(
        !visible_spans.is_empty(),
        "seeded timeline model should produce visible spans"
    );

    for span in &visible_spans {
        let bounds = ui.effective_bounds(span.panel);
        assert!(
            bounds.x0 >= time_region_x0 - 0.001,
            "timeline spans should not render in the lane-label rail"
        );
        assert!(bounds.x1 <= plot.x1 + 0.001);
    }

    let span_background = ui
        .resolved_value::<Option<Brush>>(visible_spans[0].panel, ui.properties().surface.background)
        .flatten();
    assert!(
        span_background.is_some(),
        "visible timeline spans should resolve a paintable background"
    );
}

#[test]
fn visible_sample_bars_share_one_pixel_aligned_baseline() {
    let (ui, view) = build_test_profiler();
    let plot = ui.effective_bounds(view.plot);
    let expected_baseline = plot.y0 + f64::from(OVERVIEW_BAR_BASELINE_Y);

    let mut checked = 0;
    for bar in &view.bars {
        let bounds = ui.effective_bounds(*bar);
        if bounds.width() == 0.0 || bounds.height() == 0.0 {
            continue;
        }

        checked += 1;
        assert_pixel_aligned(bounds.x0);
        assert_pixel_aligned(bounds.x1);
        assert_pixel_aligned(bounds.y0);
        assert_pixel_aligned(bounds.y1);
        assert_close(bounds.y1, expected_baseline);
    }
    assert!(checked > 0, "test should inspect visible bars");
}

#[test]
fn time_axis_renders_labeled_ticks() {
    let (ui, view) = build_test_profiler();

    let labels = view
        .ticks
        .iter()
        .filter_map(|tick| ui.resolved_value::<String>(tick.label, ui.properties().text))
        .filter(|label| !label.is_empty())
        .collect::<Vec<_>>();
    assert!(
        labels.iter().any(|label| label.ends_with('s')),
        "axis-backed time ticks should render second-based labels"
    );
}

fn assert_pixel_aligned(value: f64) {
    assert_close(value, value.round());
}

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta <= 0.001,
        "expected {actual} to be within 0.001 of {expected}, delta {delta}"
    );
}

fn build_test_profiler() -> (Ui, SampleProfilerView) {
    let (ui, view, _app_state) = build_unwired_test_profiler();
    (ui, view)
}

fn build_wired_test_profiler() -> (Ui, SampleProfilerView, Rc<AppState>) {
    let (mut ui, view, app_state) = build_unwired_test_profiler();
    wire(&mut ui, view.clone(), Rc::clone(&app_state));
    (ui, view, app_state)
}

fn build_unwired_test_profiler() -> (Ui, SampleProfilerView, Rc<AppState>) {
    let mut ui = Ui::new();
    let tokens = ui.style_tokens::<SightlineStyleTokens>();
    let app_state = Rc::new(AppState::new());
    ui.set_theme(sightline_theme(app_state.theme_mode(), tokens));
    ui.set_cascade(sightline_cascade(ui.properties(), tokens));
    ui.set_layout(ui.root(), Layout::column(0.0).logical_size(1_100.0, 520.0));

    let view = ui.with_children(ui.root(), |root| {
        build(root, tokens, app_state.sample_profiler())
    });
    ui.compute_layout(Size::new(1_100.0, 520.0));
    (ui, view, app_state)
}
