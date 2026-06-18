// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Sightline diagnostics workspace.

extern crate alloc;

mod app;
mod shell;
mod style;

fn main() -> overstory_app::HostResult<()> {
    app::run()
}
