// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `wgpu` and Vello presentation helpers for Overstory host crates.
//!
//! This crate owns the backend work that starts after a native host has created
//! a [`wgpu::Surface`]: adapter/device selection, surface configuration,
//! rendering a prepared Overstory scene through `imaging_vello`, blitting that
//! offscreen texture into the presentation surface, and reporting renderer
//! timings.
//!
//! It explicitly does not own native windows, event loops, input translation,
//! accessibility, timers, frame scheduling, or platform-specific surface
//! creation.

extern crate alloc;

mod blit;
mod descriptor;
mod error;
mod presenter;

pub use descriptor::WgpuPresenterDescriptor;
pub use error::WgpuPresenterError;
pub use presenter::{WgpuPresentResult, WgpuPresenter};
pub use wgpu;
