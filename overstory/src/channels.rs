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
//! These are wired as a channel cascade on
//! [`invalidation::InvalidationTracker::add_cascade`] in
//! [`crate::Ui::new`]: marking any upstream channel auto-marks every
//! downstream channel for the same key, so call sites don't need to
//! enumerate every channel a change touches. Per-property
//! [`affects_channels`](understory_property::PropertyMetadataBuilder)
//! contributes the initial mark; the tracker's cascade does the rest.
//!
//! Roadmap:
//! - Keep the frame pipeline explicit: property metadata marks the earliest
//!   affected channel and the channel cascade propagates downstream work. See
//!   ticket `ove-jeb7`.
//! - Add instrumentation per channel so examples and wind-tunnel scenarios can
//!   explain where a frame spent time and what caused invalidation.
//! - Refine granularity only when measurement shows it matters; avoid hidden
//!   scheduling or magic background work.
//! - Keep binding invalidation separate from frame invalidation until there is
//!   a clear reason to merge those lifecycles.

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

/// Binding-internal channel used by `understory_property_binding::BindingSet`
/// to track dirty bindings. Not part of the regular cascade; Overstory
/// reserves channel id 5 so the binding set has a unique slot.
pub const BINDING: Channel = Channel::new(5);
