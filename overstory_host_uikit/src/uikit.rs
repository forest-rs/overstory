// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! UIKit host implementation.

use alloc::boxed::Box;
use core::{error::Error, fmt};

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_ui_kit::UIViewController;
use overstory::Ui;

mod input;
mod state;
mod time;
mod view;

use state::{init_view_controller, view_state_installed};

const DEFAULT_INITIAL_SIZE: (u32, u32) = (1024, 768);

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
            initial_size: DEFAULT_INITIAL_SIZE,
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

#[derive(Clone, Copy, Debug)]
pub(crate) enum UiKitHostError {
    MainThreadRequired,
    ViewAlreadyRunning,
}

impl fmt::Display for UiKitHostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MainThreadRequired => {
                f.write_str("overstory_host_uikit must run on the main thread")
            }
            Self::ViewAlreadyRunning => {
                f.write_str("overstory_host_uikit already has a running root view")
            }
        }
    }
}

impl Error for UiKitHostError {}

/// Creates a UIKit root view controller for an Overstory UI.
///
/// UIKit application and window ownership stays with the app bundle. A typical
/// iOS or Mac Catalyst shell creates a `UIWindow`, sets this returned controller
/// as `rootViewController`, and makes the window key and visible.
///
/// `build_ui` is called once, after the UIKit content view, `CAMetalLayer`, and
/// GPU surface have been created successfully.
pub fn create_root_view_controller<F>(
    options: ViewOptions,
    build_ui: F,
) -> HostResult<Retained<UIViewController>>
where
    F: FnOnce() -> Ui + 'static,
{
    let mtm = MainThreadMarker::new().ok_or(UiKitHostError::MainThreadRequired)?;
    if view_state_installed() {
        return Err(Box::new(UiKitHostError::ViewAlreadyRunning));
    }

    init_view_controller(mtm, options, build_ui)
}

fn frame_from_size(width: u32, height: u32) -> CGRect {
    CGRect::new(
        CGPoint::new(0.0, 0.0),
        CGSize::new(f64::from(width), f64::from(height)),
    )
}
