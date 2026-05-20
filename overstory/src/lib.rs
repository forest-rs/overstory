// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Overstory: a WPF / `WinUI` 3-inspired UI toolkit built on `understory` and
//! `imaging`.
//!
//! # Status
//!
//! Phase 1a — bare element tree only. Subsequent phases add layout, paint,
//! input, templates, and visual states.
//!
//! # Scope of this crate
//!
//! `overstory` owns:
//!
//! - The visual tree ([`Element`]) and its identity ([`ElementId`]).
//! - The frame loop and invalidation cascade (later phases).
//! - Templates, content presenters, visual states (later phases).
//! - Layout (driven by `taffy`), paint (driven by `imaging`), input (via
//!   `ui-events`), accessibility (via `accesskit`) — all later phases.
//!
//! `overstory` does **not** own dependency property storage, style resolution,
//! property bindings, focus, event routing, hit testing, timing, or
//! virtualization. Those live in the corresponding `understory_*` crates.

extern crate alloc;

mod element;
mod ui;

pub use element::{Element, ElementId};
pub use ui::Ui;
