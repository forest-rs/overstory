// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Prints per-stage frame timings for latency-oriented profile scenarios.

use core::time::Duration;
use overstory_wind_tunnel::scenarios::{
    DASHBOARD_ROWS, FrameRunSummary, LIST_ITEMS, PROFILE_FRAMES, prepare_dashboard_frames,
    scroll_virtual_list_frames, toggle_retarget_frames,
};
use std::env;

fn main() {
    let mut args = env::args().skip(1);
    let scenario = args
        .next()
        .map(|value| value.parse::<LatencyScenario>().expect("unknown latency scenario"))
        .expect(
            "usage: profile_frame_latency <dashboard|scroll_virtual_list|toggle_retarget> [count] [frames]",
        );
    let count = args
        .next()
        .map(|value| value.parse::<usize>().expect("count must be a usize"))
        .unwrap_or_else(|| scenario.default_count());
    let frames = args
        .next()
        .map(|value| value.parse::<usize>().expect("frames must be a usize"))
        .unwrap_or(PROFILE_FRAMES);

    let summary = scenario.run(count, frames);
    print_summary(scenario, count, &summary);
}

#[derive(Copy, Clone, Debug)]
enum LatencyScenario {
    Dashboard,
    ScrollVirtualList,
    ToggleRetarget,
}

impl LatencyScenario {
    fn default_count(self) -> usize {
        match self {
            Self::Dashboard => DASHBOARD_ROWS,
            Self::ScrollVirtualList => LIST_ITEMS,
            Self::ToggleRetarget => 1,
        }
    }

    fn run(self, count: usize, frames: usize) -> FrameRunSummary {
        match self {
            Self::Dashboard => prepare_dashboard_frames(count, frames),
            Self::ScrollVirtualList => scroll_virtual_list_frames(count, frames),
            Self::ToggleRetarget => toggle_retarget_frames(frames),
        }
    }
}

impl core::fmt::Display for LatencyScenario {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Dashboard => f.write_str("dashboard"),
            Self::ScrollVirtualList => f.write_str("scroll_virtual_list"),
            Self::ToggleRetarget => f.write_str("toggle_retarget"),
        }
    }
}

impl core::str::FromStr for LatencyScenario {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "dashboard" => Ok(Self::Dashboard),
            "scroll_virtual_list" => Ok(Self::ScrollVirtualList),
            "toggle_retarget" => Ok(Self::ToggleRetarget),
            _ => Err("expected dashboard, scroll_virtual_list, or toggle_retarget"),
        }
    }
}

fn print_summary(scenario: LatencyScenario, count: usize, summary: &FrameRunSummary) {
    let frames = summary.frames.max(1);
    let timings = summary.timings;
    eprintln!("scenario: {scenario}");
    eprintln!("count: {count}");
    eprintln!("frames: {}", summary.frames);
    eprintln!("empty outputs: {}", summary.empty_outputs);
    eprintln!("checksum: {}", summary.checksum);
    eprintln!(
        "avg input-to-frame: {:.3} ms",
        average_ms(summary.input_to_frame, frames)
    );
    eprintln!(
        "max input-to-frame: {:.3} ms",
        duration_ms(summary.max_input_to_frame)
    );
    eprintln!(
        "avg frame total: {:.3} ms",
        average_ms(timings.total, frames)
    );
    eprintln!(
        "max frame total: {:.3} ms",
        duration_ms(summary.max_timings.total)
    );
    eprintln!("  set_now: {:.3} ms", average_ms(timings.set_now, frames));
    eprintln!("  layout: {:.3} ms", average_ms(timings.layout, frames));
    eprintln!(
        "  surface_refresh: {:.3} ms",
        average_ms(timings.surface_refresh, frames)
    );
    eprintln!(
        "  sync_surfaces: {:.3} ms",
        average_ms(timings.sync_surfaces, frames)
    );
    eprintln!("  compose: {:.3} ms", average_ms(timings.compose, frames));
    eprintln!("  scale: {:.3} ms", average_ms(timings.scale, frames));
    eprintln!("last frame counters:");
    eprintln!("  elements: {}", summary.last_ui.elements);
    eprintln!("  layout passes: {}", summary.last_ui.layout_passes);
    eprintln!(
        "  layout style updates: {}",
        summary.last_ui.layout_style_updates
    );
    eprintln!("  measured widgets: {}", summary.last_ui.measured_widgets);
    eprintln!("  bounds written: {}", summary.last_ui.bounds_written);
    eprintln!("  realize widgets: {}", summary.last_ui.realize_widgets);
    eprintln!("  painted elements: {}", summary.last_ui.painted_elements);
    eprintln!("  rebuilt surfaces: {}", summary.last_ui.rebuilt_surfaces);
    eprintln!(
        "  composition surface updates: {}",
        summary.last_ui.composition_surface_updates
    );
    eprintln!(
        "  channel marks s/t/l/p/c/b: {}/{}/{}/{}/{}/{}",
        summary.last_ui.channel_marks.style,
        summary.last_ui.channel_marks.template,
        summary.last_ui.channel_marks.layout,
        summary.last_ui.channel_marks.paint,
        summary.last_ui.channel_marks.compose,
        summary.last_ui.channel_marks.binding
    );
    eprintln!(
        "  text cache hits/misses/evictions: {}/{}/{}",
        summary.last_ui.text_layout_cache.hits,
        summary.last_ui.text_layout_cache.misses,
        summary.last_ui.text_layout_cache.evictions
    );
}

fn average_ms(duration: Duration, frames: usize) -> f64 {
    duration_ms(duration) / frames as f64
}

fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}
