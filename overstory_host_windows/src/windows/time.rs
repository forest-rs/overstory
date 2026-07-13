// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Conversion between native Win32 (`QueryPerformanceCounter`) host time and Overstory host-relative time.

use frameclock::{HostTime, time::Timebase};

#[derive(Clone, Copy, Debug)]
pub(crate) struct TimeMap {
    start: HostTime,
    timebase: Timebase,
}

impl TimeMap {
    pub(crate) fn new() -> Self {
        Self {
            start: frameclock_windows::now(),
            timebase: frameclock_windows::timebase(),
        }
    }

    pub(crate) fn map_host_time(self, time: HostTime) -> HostTime {
        let delta = time.saturating_duration_since(self.start);
        HostTime(delta.to_nanos(self.timebase))
    }

    pub(crate) fn now(self) -> HostTime {
        self.map_host_time(frameclock_windows::now())
    }
}

/// Converts a millihertz display refresh rate into a nanosecond frame duration.
#[allow(dead_code, reason = "see the TODO in state.rs::frame_opportunity")]
#[must_use]
pub(crate) fn refresh_interval_from_millihertz(millihertz: u32) -> Option<frameclock::Duration> {
    if millihertz == 0 {
        return None;
    }

    let millihertz = u64::from(millihertz);
    Some(frameclock::Duration(
        (1_000_000_000_000_u64 + millihertz / 2) / millihertz,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_interval_from_millihertz_rounds_to_nanosecond_ticks() {
        assert_eq!(
            refresh_interval_from_millihertz(120_000),
            Some(frameclock::Duration(8_333_333))
        );
        assert_eq!(
            refresh_interval_from_millihertz(60_000),
            Some(frameclock::Duration(16_666_667))
        );
        assert_eq!(refresh_interval_from_millihertz(0), None);
    }
}
