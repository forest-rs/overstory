// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! macOS `AppKit` host implementation.

use alloc::{boxed::Box, string::String};
use core::{cell::RefCell, error::Error, fmt};

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSWindow};
use overstory::Ui;

mod accessibility;
mod blit;
mod cursor;
mod input;
mod state;
mod time;
mod window;

use state::{
    init_window, install_window_state, request_initial_frame_and_retain_window,
    shutdown_window_state, window_state_installed,
};
use window::AppDelegate;

const DEFAULT_INITIAL_SIZE: (u32, u32) = (1024, 768);

thread_local! {
    static APP_DELEGATE: RefCell<Option<Retained<AppDelegate>>> = const { RefCell::new(None) };
}

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
pub(crate) enum AppKitHostError {
    MainThreadRequired,
    WindowAlreadyRunning,
    MissingContentLayer,
    SurfaceIncompatible,
    SurfaceValidation,
}

impl fmt::Display for AppKitHostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MainThreadRequired => {
                f.write_str("overstory_host_appkit must run on the main thread")
            }
            Self::WindowAlreadyRunning => {
                f.write_str("overstory_host_appkit already has a running window")
            }
            Self::MissingContentLayer => f.write_str("AppKit content view has no backing layer"),
            Self::SurfaceIncompatible => {
                f.write_str("CAMetalLayer surface is not compatible with the selected adapter")
            }
            Self::SurfaceValidation => {
                f.write_str("wgpu surface acquisition reported a validation error")
            }
        }
    }
}

impl Error for AppKitHostError {}

/// Runs a native AppKit Overstory app.
///
/// `build_ui` is called once, after the platform window, `CAMetalLayer`, and
/// GPU surface have been created successfully.
pub fn run<F>(options: WindowOptions, build_ui: F) -> HostResult<()>
where
    F: FnOnce() -> Ui + 'static,
{
    let mtm = MainThreadMarker::new().ok_or(AppKitHostError::MainThreadRequired)?;
    if window_state_installed() {
        return Err(Box::new(AppKitHostError::WindowAlreadyRunning));
    }

    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

    let delegate = AppDelegate::new(mtm);
    let delegate_proto = ProtocolObject::from_ref(&*delegate);
    app.setDelegate(Some(delegate_proto));
    APP_DELEGATE.with(|cell| {
        *cell.borrow_mut() = Some(delegate);
    });

    let state = init_window(mtm, &options, build_ui)?;
    install_window_state(state);
    let window = request_initial_frame_and_retain_window();
    show_window(&window);

    #[expect(deprecated, reason = "explicit foreground activation for app startup")]
    app.activateIgnoringOtherApps(true);
    app.run();

    shutdown_window_state();
    APP_DELEGATE.with(|cell| {
        *cell.borrow_mut() = None;
    });
    Ok(())
}

fn show_window(window: &NSWindow) {
    window.center();
    window.makeKeyAndOrderFront(None);
}
