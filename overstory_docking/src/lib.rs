// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Overstory docking state, interaction, and retained presentation.
//!
//! This crate owns the state machine and keyed Overstory elements that present
//! `understory_tiling` frames. Applications retain ownership of pane contents,
//! command policy, document lifecycle, and native windows.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

mod outcome;
mod state;
mod view;

pub use outcome::{DockInteractionKind, DockOutcome};
pub use state::DockState;
pub use view::{DockGeometryError, DockPaint, DockSurface, DockSurfaceStyle};

/// Re-export of the underlying headless tiling crate.
///
/// Use this when callers need frame, operation, or id types that are intentionally
/// owned by `understory_tiling`.
pub use understory_tiling as tiling;
