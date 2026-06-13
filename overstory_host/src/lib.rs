// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Runtime helpers for driving an [`overstory::Ui`] outside of a
//! particular platform windowing backend.
//!
//! This crate owns the reusable mechanics that every native Overstory
//! host needs: monotonic timer time, timer wakeup calculation, normalized
//! pointer/key dispatch, cursor intent tracking, layout, and preparation of
//! an `imaging` scene for a physical render target. Window creation, event
//! loop ownership, and GPU presentation are intentionally left to backend
//! crates such as `overstory_host_winit`.
//!
//! # Coordinate Spaces
//!
//! Core Overstory geometry is logical: layout bounds, hit testing,
//! presentation primitives, composition layers, and accessibility bounds all
//! use logical coordinates. Hosts project that logical scene into physical
//! device pixels at the final render-target boundary. Future device-aware
//! snapping should follow the same rule: map candidate geometry into physical
//! space to make snapping decisions, then write logical geometry back into the
//! UI.

extern crate alloc;

mod diagnostics;
mod dispatch;
mod host;
mod viewport;

pub use diagnostics::{FrameDiagnostics, FrameTimings};
pub use dispatch::HostDispatchResult;
pub use host::{HostFrameSubmission, HostFrameTiming, PreparedFrame, UiHost};
pub use viewport::{FrameViewport, PhysicalSize};
