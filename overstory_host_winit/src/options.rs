// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Window options for the winit host.

use alloc::string::String;

const DEFAULT_INITIAL_SIZE: (u32, u32) = (1024, 768);

/// Window options for [`crate::run`].
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
