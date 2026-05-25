// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Repeatedly runs one UI pipeline scenario for external profilers.

use core::{hint::black_box, time::Duration};
use overstory_wind_tunnel::scenarios::{
    DASHBOARD_ROWS, LIST_ITEMS, REPEATED_TEXT_LABELS, layout_dashboard,
    layout_repeated_text_labels, paint_dashboard, paint_repeated_text_labels, scroll_virtual_list,
};
use std::{env, time::Instant};

const DEFAULT_SECONDS: u64 = 30;

fn main() {
    let mut args = env::args().skip(1);
    let scenario = args
        .next()
        .map(|value| value.parse::<ProfileScenario>().expect("unknown profile scenario"))
        .expect("usage: profile_ui_pipeline <layout_dashboard|paint_dashboard|scroll_virtual_list|layout_repeated_text_labels|paint_repeated_text_labels> [count] [seconds]");
    let count = args
        .next()
        .map(|value| value.parse::<usize>().expect("count must be a usize"))
        .unwrap_or_else(|| scenario.default_count());
    let seconds = args
        .next()
        .map(|value| value.parse::<u64>().expect("profile seconds must be a u64"))
        .or_else(|| {
            env::var("OVERSTORY_PROFILE_SECONDS")
                .ok()
                .map(|value| value.parse::<u64>().expect("profile seconds must be a u64"))
        })
        .unwrap_or(DEFAULT_SECONDS);

    let start = Instant::now();
    let deadline = start + Duration::from_secs(seconds);
    let mut iterations = 0_u64;
    let mut last_result = 0_u64;

    while Instant::now() < deadline {
        last_result = black_box(scenario.run(black_box(count)));
        iterations += 1;
    }

    let elapsed = start.elapsed();
    let seconds = elapsed.as_secs_f64();
    eprintln!(
        "ran {scenario} {iterations} times with count {count} in {seconds:.3}s ({:.1}/s); last result {last_result}",
        iterations as f64 / seconds
    );
}

#[derive(Copy, Clone, Debug)]
enum ProfileScenario {
    LayoutDashboard,
    PaintDashboard,
    ScrollVirtualList,
    LayoutRepeatedTextLabels,
    PaintRepeatedTextLabels,
}

impl ProfileScenario {
    fn default_count(self) -> usize {
        match self {
            Self::LayoutDashboard | Self::PaintDashboard => DASHBOARD_ROWS,
            Self::ScrollVirtualList => LIST_ITEMS,
            Self::LayoutRepeatedTextLabels | Self::PaintRepeatedTextLabels => REPEATED_TEXT_LABELS,
        }
    }

    fn run(self, count: usize) -> u64 {
        match self {
            Self::LayoutDashboard => layout_dashboard(count) as u64,
            Self::PaintDashboard => u64::from(paint_dashboard(count)),
            Self::ScrollVirtualList => scroll_virtual_list(count).to_bits(),
            Self::LayoutRepeatedTextLabels => text_cache_result(layout_repeated_text_labels(count)),
            Self::PaintRepeatedTextLabels => text_cache_result(paint_repeated_text_labels(count)),
        }
    }
}

fn text_cache_result(stats: overstory::TextLayoutCacheStats) -> u64 {
    u64::try_from(stats.hits.saturating_sub(stats.misses)).unwrap_or(u64::MAX)
}

impl core::fmt::Display for ProfileScenario {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::LayoutDashboard => f.write_str("layout_dashboard"),
            Self::PaintDashboard => f.write_str("paint_dashboard"),
            Self::ScrollVirtualList => f.write_str("scroll_virtual_list"),
            Self::LayoutRepeatedTextLabels => f.write_str("layout_repeated_text_labels"),
            Self::PaintRepeatedTextLabels => f.write_str("paint_repeated_text_labels"),
        }
    }
}

impl core::str::FromStr for ProfileScenario {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "layout_dashboard" => Ok(Self::LayoutDashboard),
            "paint_dashboard" => Ok(Self::PaintDashboard),
            "scroll_virtual_list" => Ok(Self::ScrollVirtualList),
            "layout_repeated_text_labels" => Ok(Self::LayoutRepeatedTextLabels),
            "paint_repeated_text_labels" => Ok(Self::PaintRepeatedTextLabels),
            _ => Err("expected a known UI pipeline profile scenario"),
        }
    }
}
