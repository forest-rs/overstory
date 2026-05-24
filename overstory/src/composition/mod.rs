// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Composition bridge between Overstory presentation and Subduction layers.
//!
//! This module owns the adapter state that turns Overstory's element tree into
//! a compositor-neutral [`subduction_core::layer::LayerStore`] plus
//! `SurfaceId`-keyed software scene contents.

mod runtime;

pub(crate) use runtime::CompositionRuntime;
pub use runtime::CompositionSurface;
