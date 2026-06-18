// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::ops::Range;

use chronostore::{Chronology, Direction, Entry, StatsSummary};

use super::{FRAME_STEP_US, INITIAL_VISIBLE_US, SAMPLE_COUNT};

#[derive(Debug)]
pub(super) struct SampleProfilerData {
    pub(super) series: Chronology<f64, StatsSummary<f64>>,
    pub(super) time_range: Range<u64>,
    pub(super) peak_ms: f64,
}

impl SampleProfilerData {
    pub(super) fn seed() -> Self {
        let mut series = Chronology::<f64, StatsSummary<f64>>::with_chunk_capacity(64);
        let mut entries = Vec::with_capacity(SAMPLE_COUNT);
        let mut peak_ms = 0.0_f64;

        for index in 0..SAMPLE_COUNT {
            let timestamp = index as u64 * FRAME_STEP_US;
            let value = sampled_frame_duration_ms(index);
            peak_ms = peak_ms.max(value);
            entries.push(Entry::new(timestamp, value));
        }

        series
            .insert_values(&entries)
            .expect("seeded profiler samples are monotonic");

        let end = (SAMPLE_COUNT.saturating_sub(1) as u64 * FRAME_STEP_US) + FRAME_STEP_US;
        Self {
            series,
            time_range: 0..end,
            peak_ms,
        }
    }

    pub(super) fn duration_us(&self) -> u64 {
        self.time_range.end.saturating_sub(self.time_range.start)
    }

    pub(super) fn time_range_f64(&self) -> Range<f64> {
        self.time_range.start as f64..self.time_range.end as f64
    }

    pub(super) fn initial_range_f64(&self) -> Range<f64> {
        let end = self.time_range.end;
        let start = end.saturating_sub(INITIAL_VISIBLE_US);
        start as f64..end as f64
    }

    pub(super) fn nearest(&self, timestamp: u64) -> Option<Entry<f64>> {
        self.series
            .find_nearest_value(timestamp, Direction::Backward)
            .or_else(|| {
                self.series
                    .find_nearest_value(timestamp, Direction::Forward)
            })
    }
}

fn sampled_frame_duration_ms(index: usize) -> f64 {
    let wave = ((index % 47) as f64 * 0.31).sin().abs() * 2.0;
    let jitter = ((index.wrapping_mul(17) % 11) as f64) * 0.18;
    let spike = if index.is_multiple_of(61) {
        6.5
    } else if index.is_multiple_of(37) {
        3.0
    } else {
        0.0
    };
    4.6 + wave + jitter + spike
}
