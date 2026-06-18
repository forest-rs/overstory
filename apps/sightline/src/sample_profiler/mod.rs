// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Chronostore-backed sampled profiler instrument.
//!
//! This module owns the sampled-data instrument view: deterministic sample
//! storage, `Viewport1D` navigation state, and the retained Overstory elements
//! that present the current visible range. It explicitly does not own live
//! transport, persistence, or a generic charting API.

use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use core::cell::{Cell, RefCell};

use chronostore::{Direction, Entry};
use overstory::{ChildUi, Ui};
use peniko::kurbo::{Point, Vec2};
use understory_timeline_model::TimelineDoc;
use understory_view2d::{ClampMode, FitMode, Viewport1D};

use crate::app::AppState;
use crate::style::SightlineTokens;

mod data;
mod render;
#[cfg(test)]
mod tests;
mod timeline;
mod view;

pub(crate) use view::SampleProfilerView;

pub(super) const PLOT_WIDTH: f32 = 920.0;
pub(super) const PLOT_HEIGHT: f32 = 410.0;
pub(super) const TRACK_LABEL_WIDTH: f32 = 106.0;
pub(super) const TIME_VIEW_WIDTH: f32 = PLOT_WIDTH - TRACK_LABEL_WIDTH;
pub(super) const AXIS_HEIGHT: f32 = 34.0;
pub(super) const OVERVIEW_Y: f32 = AXIS_HEIGHT;
pub(super) const OVERVIEW_HEIGHT: f32 = 78.0;
pub(super) const OVERVIEW_BAR_BASELINE_Y: f32 = OVERVIEW_Y + OVERVIEW_HEIGHT - 12.0;
pub(super) const OVERVIEW_BAR_MAX_HEIGHT: f32 = OVERVIEW_HEIGHT - 22.0;
pub(super) const TIMELINE_LANE_Y: f32 = OVERVIEW_Y + OVERVIEW_HEIGHT + 18.0;
pub(super) const TIMELINE_LANE_HEIGHT: f32 = 48.0;
pub(super) const TIMELINE_SPAN_HEIGHT: f32 = 22.0;
pub(super) const TIMELINE_SPAN_DEPTH_OFFSET: f32 = 12.0;
pub(super) const BAR_COUNT: usize = 184;
pub(super) const TICK_COUNT: usize = 24;
pub(super) const TIMELINE_LANE_COUNT: usize = 4;
pub(super) const TIMELINE_SPAN_COUNT: usize = 24;
pub(super) const TIMELINE_MARKER_COUNT: usize = 12;
pub(super) const SAMPLE_COUNT: usize = 420;
pub(super) const FRAME_STEP_US: u64 = 8_333;
pub(super) const INITIAL_VISIBLE_US: u64 = 1_200_000;
pub(super) const MIN_VISIBLE_US: u64 = 80_000;

#[derive(Debug)]
pub(crate) struct SampleProfilerState {
    data: data::SampleProfilerData,
    timeline: TimelineDoc,
    viewport: RefCell<Viewport1D>,
    selected_timestamp: Cell<Option<u64>>,
}

impl SampleProfilerState {
    pub(crate) fn new() -> Self {
        let data = data::SampleProfilerData::seed();
        let timeline = timeline::seed_timeline_doc();
        let mut viewport = Viewport1D::new(0.0..f64::from(TIME_VIEW_WIDTH));
        viewport.set_world_bounds(Some(data.time_range_f64()));
        viewport.set_clamp_mode(ClampMode::KeepSomeVisible);
        viewport.set_fit_mode(FitMode::AlignMin);
        viewport.set_zoom_limits(
            f64::from(TIME_VIEW_WIDTH) / data.duration_us().max(1) as f64,
            f64::from(TIME_VIEW_WIDTH) / MIN_VISIBLE_US as f64,
        );
        viewport.set_visible_world_range(data.initial_range_f64());

        Self {
            data,
            timeline,
            viewport: RefCell::new(viewport),
            selected_timestamp: Cell::new(None),
        }
    }

    fn zoom_about_center(&self, factor: f64) {
        self.viewport
            .borrow_mut()
            .zoom_about_view_point(f64::from(TIME_VIEW_WIDTH) * 0.5, factor);
    }

    fn fit(&self) {
        self.viewport
            .borrow_mut()
            .set_visible_world_range(self.data.initial_range_f64());
    }

    fn select_at_view_x(&self, view_x: f64) {
        let timestamp = {
            let viewport = self.viewport.borrow();
            timestamp_from_world_x(viewport.view_to_world_x(view_x))
        };
        self.selected_timestamp
            .set(self.data.nearest(timestamp).map(|entry| entry.timestamp));
    }

    fn selected_entry(&self) -> Option<Entry<f64>> {
        let timestamp = self.selected_timestamp.get()?;
        self.data
            .series
            .find_nearest_value(timestamp, Direction::Backward)
            .or_else(|| {
                self.data
                    .series
                    .find_nearest_value(timestamp, Direction::Forward)
            })
    }
}

pub(crate) fn build(
    parent: &mut ChildUi<'_>,
    tokens: SightlineTokens,
    state: &SampleProfilerState,
) -> SampleProfilerView {
    let view = view::build(parent, tokens);
    render::render(parent.ui_mut(), &view, state);
    view
}

pub(crate) fn wire(ui: &mut Ui, view: SampleProfilerView, state: Rc<AppState>) {
    let zoom_in_view = view.clone();
    let zoom_in_state = Rc::clone(&state);
    ui.on_click(view.zoom_in, move |ui, event| {
        event.mark_handled();
        zoom_in_state.sample_profiler().zoom_about_center(1.45);
        render::render(ui, &zoom_in_view, zoom_in_state.sample_profiler());
    });

    let zoom_out_view = view.clone();
    let zoom_out_state = Rc::clone(&state);
    ui.on_click(view.zoom_out, move |ui, event| {
        event.mark_handled();
        zoom_out_state
            .sample_profiler()
            .zoom_about_center(1.0 / 1.45);
        render::render(ui, &zoom_out_view, zoom_out_state.sample_profiler());
    });

    let fit_view = view.clone();
    let fit_state = Rc::clone(&state);
    ui.on_click(view.fit, move |ui, event| {
        event.mark_handled();
        fit_state.sample_profiler().fit();
        render::render(ui, &fit_view, fit_state.sample_profiler());
    });

    let select_view = view.clone();
    let select_state = Rc::clone(&state);
    ui.on_click(view.plot, move |ui, event| {
        event.mark_handled();
        let local_x = local_plot_x(ui, select_view.plot, event.position);
        select_state.sample_profiler().select_at_view_x(local_x);
        render::render(ui, &select_view, select_state.sample_profiler());
    });

    let scroll_view = view;
    ui.on_scroll(scroll_view.plot, move |ui, event| {
        let delta = event.logical_delta(
            Vec2::new(24.0, 24.0),
            Vec2::new(f64::from(TIME_VIEW_WIDTH), f64::from(PLOT_HEIGHT)),
        );
        if delta.y == 0.0 {
            return;
        }
        event.mark_handled();

        let factor = if delta.y > 0.0 { 1.18 } else { 1.0 / 1.18 };
        let local_x = local_plot_x(ui, scroll_view.plot, event.position);
        state
            .sample_profiler()
            .viewport
            .borrow_mut()
            .zoom_about_view_point(local_x, factor);
        render::render(ui, &scroll_view, state.sample_profiler());
    });
}

fn local_plot_x(ui: &Ui, plot: overstory::ElementId, position: Point) -> f64 {
    let bounds = ui.effective_bounds(plot);
    (position.x - bounds.x0 - f64::from(TRACK_LABEL_WIDTH)).clamp(0.0, f64::from(TIME_VIEW_WIDTH))
}

fn timestamp_from_world_x(world_x: f64) -> u64 {
    if !world_x.is_finite() || world_x <= 0.0 {
        return 0;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "profiler timestamps are clamped to finite non-negative microseconds"
    )]
    {
        world_x.round().min(u64::MAX as f64) as u64
    }
}

fn format_time_us(timestamp: u64) -> String {
    if timestamp >= 1_000_000 {
        format!("{:.2}s", timestamp as f64 / 1_000_000.0)
    } else {
        format!("{:.0}ms", timestamp as f64 / 1_000.0)
    }
}

fn format_duration_us(duration: u64) -> String {
    if duration >= 1_000_000 {
        format!("{:.2}s", duration as f64 / 1_000_000.0)
    } else {
        format!("{:.0}ms", duration as f64 / 1_000.0)
    }
}
