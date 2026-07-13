// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Win32 backend entry point.
//!
//! Ties together window creation ([`window`]), retained per-window state ([`state`]),
//! AccessKit ([`accessibility`]), cursor mapping ([`cursor`]), and frame-pacing helpers ([`time`])
//! into the [`run`] function application crates call.

#![expect(unsafe_code, reason = "Win32 FFI")]

use alloc::{boxed::Box, string::String};

use overstory::Ui;

mod accessibility;
mod cursor;
mod state;
mod time;
mod window;

use state::WindowState;

/// Result type used by the Win32 host.
pub type HostResult<T> = Result<T, Box<dyn core::error::Error>>;

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

/// Runs a native Win32 Overstory app.
///
/// `build_ui` is called once, after the platform window and GPU surface have been created successfully.
pub fn run<F>(options: WindowOptions, build_ui: F) -> HostResult<()>
where
    F: FnOnce() -> Ui + 'static,
{
    window::enable_per_monitor_dpi_awareness();

    // SAFETY: called on the thread that will run the message loop below.
    let hwnd = unsafe { window::create_window(options) }?;

    let mut state = WindowState::new(hwnd, build_ui)?;
    state.request_initial_frame();
    state.attach();

    // SAFETY: `hwnd` was just created above, on this thread.
    unsafe { window::show_window(hwnd) };

    // SAFETY: `hwnd` was just created above.
    let wait = unsafe { WindowState::fetch(hwnd) }
        .as_deref()
        .and_then(WindowState::duration_until_next_wakeup);
    window::arm_next_wakeup(hwnd, wait);

    // SAFETY: this thread created `hwnd` and owns its message queue. every dispatched message
    //          (including this crate's own `WM_TIMER` wakeups) is routed through `window::wndproc`,
    //          which pumps a frame and re-arms the wakeup timer after each one.
    unsafe { window::run_message_loop() };

    Ok(())
}
