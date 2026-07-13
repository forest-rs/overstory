// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Native Windows Overstory host built directly on the raw Win32 API (via `windows-rs`)
//! and [`overstory_presenter_wgpu`].
//!
//! This crate owns window-class registration, `HWND` creation, and the Win32 message loop itself.
//! Every `WM_*` message this crate's `WNDPROC` receives is fed directly into [`ui_events_windows::WindowEventReducer`]'.
//! Frame timing is read from the Win32 `QueryPerformanceCounter` clock through [`frameclock_windows`].
//! Shared `wgpu` / Vello presentation is delegated to [`overstory_presenter_wgpu`].

extern crate alloc;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
pub use windows::{HostResult, WindowOptions, run};

#[cfg(not(target_os = "windows"))]
mod unsupported;

#[cfg(not(target_os = "windows"))]
pub use unsupported::{HostResult, UnsupportedPlatform, WindowOptions, run};
