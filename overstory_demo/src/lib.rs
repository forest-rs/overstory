// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared Overstory control gallery construction.

extern crate alloc;

mod app;

pub use app::{build_ui, initial_window_size};

#[cfg(feature = "host-runner")]
pub use app::run;
