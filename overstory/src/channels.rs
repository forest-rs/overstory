// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Invalidation channels Overstory cascades through each frame.
//!
//! The cascade order is the same one validated by the previous prototype:
//!
//! ```text
//! STYLE -> TEMPLATE -> LAYOUT -> PAINT -> COMPOSE
//! ```
//!
//! Earlier phases dirty downstream channels: a style change can dirty layout
//! and paint, a layout change can dirty paint, etc. The toolkit unions
//! `affects_channels` from each property write into its
//! [`invalidation::InvalidationTracker`].
//!
//! In Phase 1b only [`PAINT`] and [`COMPOSE`] are wired through `Ui::paint`;
//! [`STYLE`], [`TEMPLATE`], and [`LAYOUT`] are reserved channel ids that later
//! phases attach work to.

use invalidation::Channel;

/// Style cascade re-resolution (selectors changed, pseudoclasses changed,
/// theme swapped).
pub const STYLE: Channel = Channel::new(0);

/// Template needs (re-)expansion. Reserved for Phase 1c.
pub const TEMPLATE: Channel = Channel::new(1);

/// Layout pass (measure + arrange) needs to run. Reserved for Phase 1e.
pub const LAYOUT: Channel = Channel::new(2);

/// Per-element paint recording is stale. Behaviors must re-record into the
/// scene cache.
pub const PAINT: Channel = Channel::new(3);

/// Frame composition needs to re-assemble cached scenes into the output.
/// Marked whenever the tree structure or per-element transforms change even
/// when no element's paint recording is stale.
pub const COMPOSE: Channel = Channel::new(4);
