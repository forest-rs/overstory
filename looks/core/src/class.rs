// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared chrome class names every look styles.
//!
//! All looks target the *same* class names for app chrome, so an application
//! can switch looks at runtime by swapping theme + cascade without rebuilding
//! its element tree: a `.look-card` element simply restyles under whichever
//! look is active.

/// App root backdrop (the desktop / window background).
pub const ROOT: &str = ".look-root";
/// A flat surface region.
pub const SURFACE: &str = ".look-surface";
/// A raised panel.
pub const PANEL: &str = ".look-panel";
/// A card / grouped-content container.
pub const CARD: &str = ".look-card";
/// Display / heading text.
pub const HEADING: &str = ".look-heading";
/// Body text.
pub const BODY: &str = ".look-body";
/// Muted / caption text.
pub const CAPTION: &str = ".look-caption";

/// Marks a primary / default action (e.g. the default button). Looks may give
/// it special emphasis — System 6, for instance, draws a double border.
pub const PRIMARY: &str = ".look-primary";
