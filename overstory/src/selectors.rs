// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Selector helpers for Overstory's control and part style contract.
//!
//! Roadmap:
//! - Keep this module as Overstory-facing naming over
//!   [`understory_style::selectors`], not a second selector language.
//! - Add Overstory-specific selector helpers here only when they depend on
//!   control vocabulary from this crate.

pub use understory_style::selectors::{
    part, part_when, part_with_class, type_state as state, type_tag,
};
