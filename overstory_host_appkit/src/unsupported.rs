// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Non-macOS fallback for the AppKit host crate.

use alloc::{boxed::Box, string::String};
use core::{error::Error, fmt};

use overstory::Ui;

/// Result type used by the AppKit host.
pub type HostResult<T> = Result<T, Box<dyn Error>>;

/// Window options for [`run`].
#[derive(Clone, Debug)]
pub struct WindowOptions {
    title: String,
    initial_size: (u32, u32),
}

impl WindowOptions {
    /// Creates options with `title` and a default logical window size.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            initial_size: (1024, 768),
        }
    }

    /// Sets the initial logical window size.
    #[must_use]
    pub const fn initial_size(mut self, width: u32, height: u32) -> Self {
        self.initial_size = (width, height);
        self
    }

    /// Returns the platform window title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the initial logical window size.
    #[must_use]
    pub const fn size(&self) -> (u32, u32) {
        self.initial_size
    }
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self::new("Overstory")
    }
}

/// Error returned when the AppKit host is run on a non-macOS target.
#[derive(Clone, Copy, Debug)]
pub struct UnsupportedPlatform;

impl fmt::Display for UnsupportedPlatform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("overstory_host_appkit is only supported on macOS")
    }
}

impl Error for UnsupportedPlatform {}

/// Returns an error on non-macOS targets.
pub fn run<F>(_options: WindowOptions, _build_ui: F) -> HostResult<()>
where
    F: FnOnce() -> Ui + 'static,
{
    Err(Box::new(UnsupportedPlatform))
}
