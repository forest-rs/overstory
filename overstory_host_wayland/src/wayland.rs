// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Linux Wayland host implementation.

use alloc::{boxed::Box, string::String};
use core::{error::Error, fmt};

use overstory::Ui;

mod input;
mod window;

const DEFAULT_INITIAL_SIZE: (u32, u32) = (1024, 768);

/// Result type used by the Wayland host.
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
            initial_size: DEFAULT_INITIAL_SIZE,
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

#[derive(Clone, Copy, Debug)]
pub(crate) enum WaylandHostError {
    MissingGlobal(&'static str),
}

impl fmt::Display for WaylandHostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingGlobal(interface) => {
                write!(f, "required Wayland global `{interface}` is unavailable")
            }
        }
    }
}

impl Error for WaylandHostError {}

/// Runs a native Wayland Overstory app.
///
/// `build_ui` is called once, after the Wayland window and GPU surface have
/// been created successfully.
pub fn run<F>(options: WindowOptions, build_ui: F) -> HostResult<()>
where
    F: FnOnce() -> Ui + 'static,
{
    window::run(options, build_ui)
}
