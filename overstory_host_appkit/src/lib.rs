// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Native macOS Overstory host built on `AppKit`, `CAMetalLayer`, and
//! `overstory_presenter_wgpu`.
//!
//! This crate owns the AppKit event loop, `NSWindow`/`CAMetalLayer` surface,
//! native display-link ticks, and native `wgpu` surface creation. Shared
//! `wgpu` / Vello presentation is delegated to `overstory_presenter_wgpu`.
//! It explicitly does not own Overstory UI semantics, input normalization, or
//! platform-independent frame scheduling policy.

extern crate alloc;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::{HostResult, WindowOptions, run};

#[cfg(not(target_os = "macos"))]
mod unsupported;

#[cfg(not(target_os = "macos"))]
pub use unsupported::{HostResult, UnsupportedPlatform, WindowOptions, run};
