// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Repeatedly builds the rich dashboard scenario for external profilers.

use core::{hint::black_box, time::Duration};
use overstory_wind_tunnel::scenarios::{DASHBOARD_ROWS, build_dashboard};
use std::{env, time::Instant};

const DEFAULT_SECONDS: u64 = 30;

fn main() {
    let mut args = env::args().skip(1);
    let rows = args
        .next()
        .map(|value| value.parse::<usize>().expect("row count must be a usize"))
        .unwrap_or(DASHBOARD_ROWS);
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
    let mut elements = 0_usize;

    while Instant::now() < deadline {
        let ui = build_dashboard(black_box(rows));
        elements = black_box(ui.len());
        black_box(&ui);
        iterations += 1;
    }

    let elapsed = start.elapsed();
    let seconds = elapsed.as_secs_f64();
    eprintln!(
        "built {iterations} dashboards with {elements} elements each in {seconds:.3}s ({:.1}/s)",
        iterations as f64 / seconds
    );
}
