// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Unsupported-platform fallback for the UIKit demo wrapper.

/// Report that the standalone UIKit app wrapper cannot run on this target.
pub fn run_standalone() {
    eprintln!("overstory_demo_uikit only runs as a UIKit app bundle on iOS, tvOS, or Mac Catalyst");
}
