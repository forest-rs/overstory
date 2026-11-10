// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Public results from docking commands and pointer interactions.
//!
//! This module owns the adapter's result vocabulary; it explicitly does not
//! own interaction execution or Understory's semantic operations.

use understory_tiling::{HitKind, PaneId, TileError, TileOp};

/// Current high-level docking interaction kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockInteractionKind {
    /// A pending drag gesture has not crossed its threshold yet.
    PendingDrag,
    /// An active drag interaction.
    Drag,
    /// An active split resize interaction.
    Resize,
}

/// Typed result of a docking command or pointer step.
#[derive(Clone, Debug)]
pub enum DockOutcome {
    /// Nothing changed.
    None,
    /// Pointer-down did not hit a docking region.
    NoHit,
    /// A pointer interaction started.
    InteractionStarted(DockInteractionKind),
    /// A pointer interaction currently has no committable proposal.
    NoProposal(DockInteractionKind),
    /// The current pointer interaction proposal validates.
    ProposalValid(TileOp),
    /// The current pointer interaction proposal was rejected by policy or structure.
    ProposalRejected(TileError),
    /// A click selected a pane without changing the layout.
    PaneSelected(PaneId),
    /// A click did not hit an activatable pane or tab.
    ClickMiss(Option<HitKind>),
    /// A layout operation committed.
    Committed(TileOp),
    /// A layout operation was rejected.
    Rejected(TileError),
    /// The active pointer interaction was canceled.
    Canceled,
}
