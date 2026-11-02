// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Conversion between native Apple host time and Overstory host-relative time.

use frameclock::time::Timebase;
use frameclock::{Duration, FrameTick, HostTime};

#[derive(Clone, Copy, Debug)]
pub(crate) struct AppleTimeMap {
    start: HostTime,
    timebase: Timebase,
}

impl AppleTimeMap {
    pub(crate) fn new() -> Self {
        Self {
            start: frameclock_apple::now(),
            timebase: frameclock_apple::timebase(),
        }
    }

    pub(crate) fn map_host_time(self, time: HostTime) -> HostTime {
        let delta = time.saturating_duration_since(self.start);
        HostTime(delta.to_nanos(self.timebase))
    }

    pub(crate) fn map_apple_event_time_ns(self, time_ns: u64) -> u64 {
        time_ns.saturating_sub(self.start.to_nanos(self.timebase))
    }

    fn map_refresh_interval(self, interval: Duration) -> Duration {
        Duration(interval.to_nanos(self.timebase))
    }

    pub(crate) fn map_tick(self, tick: FrameTick) -> FrameTick {
        FrameTick {
            now: self.map_host_time(tick.now),
            predicted_present: tick
                .predicted_present
                .map(|predicted_present| self.map_host_time(predicted_present)),
            refresh_interval: tick
                .refresh_interval
                .map(|interval| self.map_refresh_interval(interval)),
            output: tick.output,
            prev_actual_present: tick
                .prev_actual_present
                .map(|actual_present| self.map_host_time(actual_present)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_tick_maps_timestamps_and_refresh_duration_to_nanos() {
        let map = AppleTimeMap {
            start: HostTime(2_000),
            timebase: Timebase::new(125, 3),
        };
        let tick = map.map_tick(FrameTick {
            now: HostTime(2_024),
            predicted_present: Some(HostTime(2_048)),
            refresh_interval: Some(Duration(24)),
            output: frameclock::OutputId(7),
            prev_actual_present: Some(HostTime(2_012)),
        });

        assert_eq!(tick.now, HostTime(1_000));
        assert_eq!(tick.predicted_present, Some(HostTime(2_000)));
        assert_eq!(tick.refresh_interval, Some(Duration(1_000)));
        assert_eq!(tick.prev_actual_present, Some(HostTime(500)));
        assert_eq!(tick.output, frameclock::OutputId(7));
    }

    #[test]
    fn apple_event_time_maps_to_host_relative_nanos() {
        let map = AppleTimeMap {
            start: HostTime(2_000),
            timebase: Timebase::new(1, 2),
        };

        assert_eq!(map.map_apple_event_time_ns(1_250), 250);
    }
}
