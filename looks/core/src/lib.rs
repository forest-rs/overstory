// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! # Overstory Look
//!
//! The shared harness behind the `overstory_look_*` looks. It holds the pieces
//! every look reuses regardless of its visual language:
//!
//! - [`color`] — small sRGB color math (blend, lighten, darken, alpha) for
//!   deriving ramps and shades from seed colors.
//! - [`design_md`] — a reader for the Google Labs `DESIGN.md` token format
//!   (YAML frontmatter), so each look keeps its palette in a human-editable
//!   file.
//! - [`class`] — shared chrome class names every look styles, so an app can
//!   switch looks at runtime without rebuilding its tree.
//!
//! Each look layers its own token taxonomy, palette, theme, and cascade on top.

pub mod class;
pub mod color;
pub mod design_md;
