// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Loading Signature [`Seeds`] from a `DESIGN.md` file.
//!
//! Uses the shared [Google Labs DESIGN.md][spec] reader in
//! [`overstory_look::design_md`]: YAML frontmatter carries the `colors` tokens
//! (each as a `{ light, dark }` map, the per-scheme extension) plus a `motion`
//! block. This maps those tokens onto Signature's [`Seeds`]; roles the file
//! omits keep their Rust defaults.
//!
//! [spec]: https://github.com/google-labs-code/design.md

use overstory_look::design_md::parse;
use understory_style::Theme;

pub use overstory_look::design_md::DesignError;

use crate::palette::{Seeds, seeds_for};
use crate::theme::theme_from_seeds;
use crate::tokens::{ResolvedTokens, Scheme};

/// The crate's bundled `DESIGN.md`, embedded so the default theme can be built
/// from the same source designers edit.
pub const DEFAULT: &str = include_str!("../DESIGN.md");

fn scheme_key(scheme: Scheme) -> &'static str {
    match scheme {
        Scheme::Dark => "dark",
        Scheme::Light => "light",
    }
}

/// Parses `src` and returns the [`Seeds`] for `scheme`, overlaying any tokens
/// the file defines onto the defaults.
///
/// # Errors
///
/// Returns [`DesignError`] if the frontmatter is missing/invalid or a color or
/// motion value is malformed.
pub fn seeds(src: &str, scheme: Scheme) -> Result<Seeds, DesignError> {
    let doc = parse(src)?;
    let key = scheme_key(scheme);
    let mut s = seeds_for(scheme);
    if let Some(c) = doc.color("accent", key) {
        s.accent = c;
    }
    if let Some(c) = doc.color("surface", key) {
        s.surface = c;
    }
    if let Some(c) = doc.color("panel", key) {
        s.panel = c;
    }
    if let Some(c) = doc.color("card", key) {
        s.card = c;
    }
    if let Some(c) = doc.color("control", key) {
        s.control = c;
    }
    if let Some(c) = doc.color("control-border", key) {
        s.control_border = c;
    }
    if let Some(c) = doc.color("track", key) {
        s.track = c;
    }
    if let Some(c) = doc.color("thumb", key) {
        s.thumb = c;
    }
    if let Some(c) = doc.color("on-accent", key) {
        s.on_accent = c;
    }
    if let Some(c) = doc.color("on-surface", key) {
        s.on_surface = c;
    }
    if let Some(c) = doc.color("muted", key) {
        s.muted = c;
    }
    if let Some(c) = doc.color("heading", key) {
        s.heading = c;
    }
    if let Some(c) = doc.color("body", key) {
        s.body = c;
    }
    if let Some(c) = doc.color("focus", key) {
        s.focus = c;
    }
    if let Some(ms) = doc.motion_ms("fast") {
        s.motion_fast_ms = ms;
    }
    if let Some(ms) = doc.motion_ms("normal") {
        s.motion_normal_ms = ms;
    }
    if let Some(ms) = doc.motion_ms("emphasized") {
        s.motion_emphasized_ms = ms;
    }
    if let Some(ms) = doc.motion_ms("glow") {
        s.motion_glow_ms = ms;
    }
    Ok(s)
}

/// Builds a Signature [`Theme`] for `scheme` from a `DESIGN.md` source.
///
/// # Errors
///
/// Propagates any [`DesignError`] from [`seeds`].
pub fn theme_from_design_md(
    src: &str,
    scheme: Scheme,
    tokens: &ResolvedTokens,
) -> Result<Theme, DesignError> {
    Ok(theme_from_seeds(scheme, &seeds(src, scheme)?, tokens))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::{dark_seeds, light_seeds};

    #[test]
    fn bundled_design_md_matches_rust_dark_defaults() {
        assert_eq!(
            seeds(DEFAULT, Scheme::Dark).expect("DESIGN.md parses"),
            dark_seeds(),
            "DESIGN.md dark tokens drifted from palette::dark_seeds()"
        );
    }

    #[test]
    fn bundled_design_md_matches_rust_light_defaults() {
        assert_eq!(
            seeds(DEFAULT, Scheme::Light).expect("DESIGN.md parses"),
            light_seeds(),
            "DESIGN.md light tokens drifted from palette::light_seeds()"
        );
    }

    #[test]
    fn missing_roles_fall_back_to_defaults() {
        let src = "---\ncolors:\n  accent: { dark: \"#112233\" }\nmotion:\n  fast: 42\n---\n";
        let s = seeds(src, Scheme::Dark).expect("parses");
        assert_eq!(s.accent, Color::from_rgb8(0x11, 0x22, 0x33));
        assert_eq!(s.motion_fast_ms, 42);
        // Untouched tokens keep their defaults.
        assert_eq!(s.surface, dark_seeds().surface);
        assert_eq!(s.motion_normal_ms, dark_seeds().motion_normal_ms);
    }

    #[test]
    fn bad_color_is_an_error() {
        let src = "---\ncolors:\n  accent: \"nope\"\n---\n";
        assert!(matches!(
            seeds(src, Scheme::Dark),
            Err(DesignError::BadColor { .. })
        ));
    }

    use peniko::Color;
}
