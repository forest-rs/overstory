// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Native Overstory host built on `winit`, `wgpu`, and `imaging_vello`.
//!
//! This crate owns the platform event loop, window creation, normalized
//! input translation, GPU surface setup, and presentation path. Application
//! crates only need to provide a function that builds an [`overstory::Ui`].

extern crate alloc;

use alloc::boxed::Box;
use core::error::Error;

use overstory::Ui;
use winit::event_loop::EventLoop;

mod app;
mod blit;
mod error;
mod options;
mod state;

use app::{App, HostEvent};
pub use options::WindowOptions;

/// Result type used by the winit host.
pub type HostResult<T> = Result<T, Box<dyn Error>>;

/// Runs a native winit Overstory app.
///
/// `build_ui` is called once, after the platform window and GPU surface have
/// been created successfully.
pub fn run<F>(options: WindowOptions, build_ui: F) -> HostResult<()>
where
    F: FnOnce() -> Ui + 'static,
{
    let event_loop = EventLoop::<HostEvent>::with_user_event().build()?;
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
    let accesskit_proxy = event_loop.create_proxy();

    let mut app = App::new(options, build_ui, accesskit_proxy);
    event_loop.run_app(&mut app)?;
    Ok(())
}
