// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Application runner facade for Overstory native hosts.
//!
//! This crate owns app-facing host selection and "run this UI" plumbing. It
//! explicitly does not own native event loops, rendering surfaces, input
//! normalization, or Overstory UI semantics.

#[cfg(not(any(feature = "appkit-host", feature = "winit-host")))]
compile_error!("select an Overstory app host feature: `winit-host` or `appkit-host`");

#[cfg(all(feature = "appkit-host", target_os = "macos"))]
mod selected {
    pub use overstory_host_appkit::{HostResult, WindowOptions, run};
}

#[cfg(all(
    feature = "appkit-host",
    not(target_os = "macos"),
    not(feature = "winit-host")
))]
mod selected {
    pub use overstory_host_appkit::{HostResult, WindowOptions, run};
}

#[cfg(all(
    feature = "winit-host",
    any(not(feature = "appkit-host"), not(target_os = "macos"))
))]
mod selected {
    pub use overstory_host_winit::{HostResult, WindowOptions, run};
}

pub use selected::{HostResult, WindowOptions, run};
