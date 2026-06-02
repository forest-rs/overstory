// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Frame diagnostics captured by the reusable host runtime.

use core::time::Duration;

use overstory::UiFrameStats;

/// Wall-clock timings for one host frame preparation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FrameTimings {
    /// Time spent advancing the UI clock and presentation timeline.
    pub set_now: Duration,
    /// Time spent in layout, including bindings, post-layout hooks, and
    /// composition-layer geometry synchronization.
    pub layout: Duration,
    /// Time spent refreshing presentation and Overstory-owned surface scenes.
    pub surface_refresh: Duration,
    /// Time spent copying updated surface scenes into the host cache.
    pub sync_surfaces: Duration,
    /// Time spent composing cached surfaces into the logical output scene.
    pub compose: Duration,
    /// Time spent scaling the logical scene for the physical render target.
    pub scale: Duration,
    /// Total time spent preparing the frame.
    pub total: Duration,
}

/// Diagnostics captured for one host frame preparation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FrameDiagnostics {
    /// Host-side wall-clock timings by stage.
    pub timings: FrameTimings,
    /// Core UI counters captured after the frame was prepared.
    pub ui: UiFrameStats,
    /// Number of Overstory-owned surface scenes retained by the host cache.
    pub surface_cache_len: usize,
    /// Whether the scaled output scene is empty.
    pub output_empty: bool,
}
