// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Native Linux Overstory host built on Wayland, `xdg-shell`, and
//! `overstory_presenter_wgpu`.
//!
//! This crate owns the Wayland connection and event loop, the
//! `wl_surface`/`xdg_toplevel` window, `wl_surface.frame` callback ticks, and
//! native `wgpu` surface creation. Shared `wgpu` / Vello presentation is
//! delegated to `overstory_presenter_wgpu`. It explicitly does not own
//! Overstory UI semantics, input normalization, or platform-independent frame
//! scheduling policy.

extern crate alloc;

#[cfg(target_os = "linux")]
mod wayland;

#[cfg(target_os = "linux")]
pub use wayland::{HostResult, WindowOptions, run};

#[cfg(not(target_os = "linux"))]
mod unsupported;

#[cfg(not(target_os = "linux"))]
pub use unsupported::{HostResult, UnsupportedPlatform, WindowOptions, run};
