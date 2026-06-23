// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Standalone UIKit app entry point for the Overstory control gallery.

#[cfg(any(target_os = "ios", target_os = "tvos"))]
fn main() -> ! {
    overstory_demo_uikit::run_standalone()
}

#[cfg(not(any(target_os = "ios", target_os = "tvos")))]
fn main() {
    overstory_demo_uikit::run_standalone();
}
