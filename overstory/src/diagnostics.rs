// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Lightweight diagnostics snapshots for the UI frame pipeline.
//!
//! The counters here are intentionally cheap and always available in the
//! `no_std` core crate. Host crates own wall-clock timing because they already
//! depend on a monotonic platform clock.

use core::time::Duration;

use crate::channels::{ARRANGE, BINDING, COMPOSE, GEOMETRY, LAYOUT, PAINT, STYLE, TEMPLATE};
use crate::element::ElementId;
use crate::text::TextLayoutCacheStats;
use frameclock::{FrameDemand, FrameTimingSummary};

/// Outcome of one bounded layout-stabilization attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutOutcome {
    /// Layout reached a fixed point within the pass budget.
    Stable {
        /// Number of layout passes used by this attempt.
        passes: usize,
    },
    /// Layout still requested another pass after consuming the pass budget.
    ///
    /// The pending work remains invalidated so a host can schedule another
    /// frame. [`UiFrameStats::layout_stabilization_causes`] reports how many
    /// widgets requested the final additional pass.
    Deferred {
        /// Number of layout passes used by this attempt.
        passes: usize,
    },
}

impl LayoutOutcome {
    /// Returns the number of layout passes used by this attempt.
    #[must_use]
    pub const fn passes(self) -> usize {
        match self {
            Self::Stable { passes } | Self::Deferred { passes } => passes,
        }
    }

    /// Returns whether another frame is required to continue stabilization.
    #[must_use]
    pub const fn needs_another_frame(self) -> bool {
        matches!(self, Self::Deferred { .. })
    }
}

impl Default for LayoutOutcome {
    fn default() -> Self {
        Self::Stable { passes: 0 }
    }
}

/// Layout stage that requested another stabilization pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutStabilizationPhase {
    /// A widget's arrange hook changed layout-affecting state.
    Arrange,
    /// A widget's element-realization hook changed retained layout structure.
    ElementRealization,
    /// Layout-affecting state remained invalidated after widget hooks.
    LayoutInvalidation,
}

/// Widget responsible for requesting another layout-stabilization pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LayoutStabilizationCause {
    /// Widget element that requested another pass.
    pub element: ElementId,
    /// Stage that requested another pass.
    pub phase: LayoutStabilizationPhase,
}

/// Count of invalidation-channel marks observed during a frame.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ChannelMarkStats {
    /// Number of `STYLE` channel marks.
    pub style: usize,
    /// Number of `TEMPLATE` channel marks.
    pub template: usize,
    /// Number of `LAYOUT` channel marks.
    pub layout: usize,
    /// Number of `ARRANGE` channel marks.
    pub arrange: usize,
    /// Number of `GEOMETRY` channel marks.
    pub geometry: usize,
    /// Number of `PAINT` channel marks.
    pub paint: usize,
    /// Number of `COMPOSE` channel marks.
    pub compose: usize,
    /// Number of binding-runtime channel marks.
    pub binding: usize,
}

impl ChannelMarkStats {
    pub(crate) fn record(&mut self, channel: invalidation::Channel) {
        if channel == STYLE {
            self.style += 1;
        } else if channel == TEMPLATE {
            self.template += 1;
        } else if channel == LAYOUT {
            self.layout += 1;
        } else if channel == ARRANGE {
            self.arrange += 1;
        } else if channel == GEOMETRY {
            self.geometry += 1;
        } else if channel == PAINT {
            self.paint += 1;
        } else if channel == COMPOSE {
            self.compose += 1;
        } else if channel == BINDING {
            self.binding += 1;
        }
    }
}

/// Counters collected while driving one UI frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UiFrameStats {
    /// Host-supplied wall-clock timings for the most recently prepared frame.
    ///
    /// Core Overstory does not read the platform clock; host crates measure
    /// these phases and write them back so devtools snapshots can display the
    /// same frame data as host-side diagnostics.
    pub timings: UiFrameTimings,
    /// Host-supplied backend renderer timings for the most recently submitted
    /// frame.
    ///
    /// These timings are zero until a platform host records renderer work.
    /// They are CPU-side submission timings, not GPU completion timestamps.
    pub renderer_timings: UiRendererTimings,
    /// Host-supplied frame scheduling facts for the most recently submitted
    /// frame.
    ///
    /// These facts come from `frameclock`: demand, frame-start time,
    /// sample-time, commit deadline, presentation feedback, and scheduler
    /// adaptation state. Core Overstory stores the summary so devtools can read
    /// it from the same frame snapshot as other UI counters.
    pub frame_timing: Option<FrameTimingSummary>,
    /// Demand that selected the currently prepared frame.
    ///
    /// This is filled by host crates when a frame is prepared through
    /// `frameclock`. It is `FrameDemand::NONE` for ad hoc preparation paths
    /// that are not backed by a retained frame plan.
    pub prepared_demand: FrameDemand,
    /// Live element count observed when the snapshot is read.
    pub elements: usize,
    /// Invalidation-channel marks observed since the last reset.
    pub channel_marks: ChannelMarkStats,
    /// Roots drained from the `STYLE` channel.
    pub style_roots: usize,
    /// Elements whose selector match state was recomputed.
    pub styled_elements: usize,
    /// Whether `compute_layout` found layout work to perform.
    pub layout_requested: bool,
    /// Number of layout passes run. Element realization can request more than
    /// one pass.
    pub layout_passes: usize,
    /// Outcome of the most recent layout-stabilization attempt.
    pub layout_outcome: LayoutOutcome,
    /// Number of widgets that requested the final additional stabilization
    /// pass.
    pub layout_stabilization_causes: usize,
    /// Layout style records pushed into the layout engine.
    pub layout_style_updates: usize,
    /// Widget measure callbacks invoked by the layout engine.
    pub measured_widgets: usize,
    /// Element bounds written back after layout.
    pub bounds_written: usize,
    /// Elements visited for element-realization hooks after arrange.
    pub element_realize_visits: usize,
    /// Widgets whose element-realization hook reported a layout-affecting
    /// change.
    pub element_realize_changes: usize,
    /// Elements drained from the `PAINT` channel.
    pub painted_elements: usize,
    /// Elements drained from the `GEOMETRY` channel.
    pub geometry_elements: usize,
    /// Presentation nodes drained from the presentation dirty set.
    pub presentation_dirty_nodes: usize,
    /// Composition surfaces rebuilt from presentation primitives.
    pub rebuilt_surfaces: usize,
    /// Composition surfaces rewritten during the most recent paint call.
    pub composition_surface_updates: usize,
    /// Live composition surfaces observed when the snapshot is read.
    pub composition_surfaces: usize,
    /// Text layout cache counters observed when the snapshot is read.
    pub text_layout_cache: TextLayoutCacheStats,
}

/// Wall-clock timings for one prepared UI frame.
///
/// The fields are zero when no host has supplied timing data yet.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UiFrameTimings {
    /// Time spent advancing the UI clock and presentation timeline.
    pub set_now: Duration,
    /// Time spent in layout, including bindings, realization hooks, and
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

/// CPU-side backend renderer timings for one submitted UI frame.
///
/// These timings are filled by platform host crates. Core Overstory does not
/// own a renderer, GPU queue, or swapchain, so the default value means "not
/// measured by this host" rather than "free".
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UiRendererTimings {
    /// Time spent rendering the prepared scene into a backend texture.
    pub scene_to_texture: Duration,
    /// Time spent acquiring the presentation surface texture.
    pub surface_acquire: Duration,
    /// Time spent blitting or copying the rendered texture to the presentation
    /// surface.
    pub blit: Duration,
    /// Time spent presenting the frame to the platform surface.
    pub present: Duration,
    /// Total CPU-side backend render submission time.
    pub total: Duration,
}
