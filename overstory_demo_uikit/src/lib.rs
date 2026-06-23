// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! UIKit app wrapper for the Overstory control gallery.
//!
//! This crate owns app-bundle entry points for running the gallery under UIKit.
//! It intentionally does not own Overstory UI construction or reusable host
//! behavior; those stay in `overstory_demo` and `overstory_host_uikit`.

#![cfg_attr(
    any(target_os = "ios", target_os = "tvos"),
    expect(
        unsafe_code,
        reason = "UIKit app delegates and C ABI exports require Objective-C FFI"
    )
)]

#[cfg(any(target_os = "ios", target_os = "tvos"))]
mod uikit;

#[cfg(any(target_os = "ios", target_os = "tvos"))]
pub use uikit::{
    create_demo_root_view_controller, overstory_demo_uikit_create_root_view_controller,
    run_standalone,
};

#[cfg(not(any(target_os = "ios", target_os = "tvos")))]
mod unsupported;

#[cfg(not(any(target_os = "ios", target_os = "tvos")))]
pub use unsupported::run_standalone;
