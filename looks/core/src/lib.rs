// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! # Overstory Look
//!
//! The shared harness behind the `overstory_look_*` looks. It holds the two
//! pieces every look reuses regardless of its visual language:
//!
//! - [`color`] — small sRGB color math (blend, lighten, darken, alpha) for
//!   deriving ramps and shades from seed colors.
//! - [`design_md`] — a minimal Markdown token-table reader and hex-color parser,
//!   so each look can keep its palette in a human-editable `DESIGN.md`.
//!
//! Each look layers its own token taxonomy, palette, theme, and cascade on top.

pub mod color;
pub mod design_md;
