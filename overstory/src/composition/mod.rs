// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Composition bridge between Overstory presentation and Subduction layers.
//!
//! This module owns the adapter state that turns Overstory's element tree into
//! a compositor-neutral [`subduction_core::layer::LayerStore`] plus
//! `SurfaceId`-keyed scene contents.
//!
//! Roadmap:
//! - Keep Subduction as the compositor-neutral layer substrate while Overstory
//!   owns the mapping from elements and presentation primitives to layers and
//!   surfaces.
//! - Support software-recorded scenes, external host surfaces, overlays, and
//!   future native compositor layers through the same layer topology.
//! - Make hosts consume explicit layer-tree and surface changes instead of
//!   rediscovering the UI tree every frame.
//! - Add composition diagnostics for layer count, surface updates, clips,
//!   transforms, hit regions, and host handoff cost.

mod runtime;

pub(crate) use runtime::CompositionRuntime;
pub use runtime::{CompositionPolicy, CompositionSurface};
pub use subduction_core::layer::SurfaceId;
