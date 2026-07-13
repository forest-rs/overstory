// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Application runner facade for Overstory native hosts.
//!
//! This crate owns app-facing host selection and "run this UI" plumbing. It
//! explicitly does not own native event loops, rendering surfaces, input
//! normalization, or Overstory UI semantics.

#[cfg(not(any(
    feature = "appkit-host",
    feature = "winit-host",
    feature = "wayland-host"
)))]
compile_error!(
    "select an Overstory app host feature: `winit-host`, `wayland-host`, or `appkit-host`"
);

#[cfg(all(
    feature = "wayland-host",
    not(target_os = "linux"),
    not(feature = "winit-host"),
    not(feature = "appkit-host")
))]
compile_error!("`wayland-host` is only supported on Linux; enable `winit-host` for this target");

#[cfg(all(feature = "appkit-host", target_os = "macos"))]
mod selected {
    pub use overstory_host_appkit::{HostResult, WindowOptions, run};
}

#[cfg(all(
    feature = "appkit-host",
    not(target_os = "macos"),
    not(feature = "winit-host"),
    not(all(feature = "wayland-host", target_os = "linux"))
))]
mod selected {
    pub use overstory_host_appkit::{HostResult, WindowOptions, run};
}

#[cfg(all(feature = "wayland-host", target_os = "linux"))]
mod selected {
    pub use overstory_host_wayland::{HostResult, WindowOptions, run};
}

#[cfg(all(feature = "windows-host", target_os = "windows"))]
mod selected {
    pub use overstory_host_windows::{HostResult, WindowOptions, run};
}

#[cfg(all(
    feature = "winit-host",
    any(not(feature = "appkit-host"), not(target_os = "macos")),
    not(all(feature = "wayland-host", target_os = "linux")),
    not(all(feature = "windows-host", target_os = "windows")),
))]
mod selected {
    pub use overstory_host_winit::{HostResult, WindowOptions, run};
}

pub use selected::{HostResult, WindowOptions, run};
