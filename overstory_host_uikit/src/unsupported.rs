// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Non-UIKit fallback for the UIKit host crate.

use alloc::boxed::Box;
use core::{error::Error, fmt};

use overstory::Ui;

/// Result type used by the UIKit host.
pub type HostResult<T> = Result<T, Box<dyn Error>>;

/// View options for [`create_root_view_controller`].
#[derive(Clone, Copy, Debug)]
pub struct ViewOptions {
    initial_size: (u32, u32),
}

impl ViewOptions {
    /// Creates options with a default logical view size.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            initial_size: (1024, 768),
        }
    }

    /// Sets the initial logical view size.
    #[must_use]
    pub const fn initial_size(mut self, width: u32, height: u32) -> Self {
        self.initial_size = (width, height);
        self
    }

    /// Returns the initial logical view size.
    #[must_use]
    pub const fn size(&self) -> (u32, u32) {
        self.initial_size
    }
}

impl Default for ViewOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// Error returned when the UIKit host is used on a non-UIKit target.
#[derive(Clone, Copy, Debug)]
pub struct UnsupportedPlatform;

impl fmt::Display for UnsupportedPlatform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("overstory_host_uikit is only supported on iOS and tvOS")
    }
}

impl Error for UnsupportedPlatform {}

/// Returns an error on non-UIKit targets.
pub fn create_root_view_controller<F>(_options: ViewOptions, _build_ui: F) -> HostResult<()>
where
    F: FnOnce() -> Ui + 'static,
{
    Err(Box::new(UnsupportedPlatform))
}
