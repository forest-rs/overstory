// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Loading System 6 [`Seeds`] from a `DESIGN.md` file.
//!
//! Uses the shared [Google Labs DESIGN.md][spec] reader in
//! [`overstory_look::design_md`]: YAML frontmatter `colors` tokens (each a
//! `{ light, dark }` map) map onto System 6's [`Seeds`]. Motion is fixed, so
//! there is no motion block.
//!
//! [spec]: https://github.com/google-labs-code/design.md

use overstory_look::design_md::parse;
use understory_style::Theme;

pub use overstory_look::design_md::DesignError;

use crate::palette::{Seeds, seeds_for};
use crate::theme::theme_from_seeds;
use crate::tokens::{ResolvedTokens, Scheme};

/// The crate's bundled `DESIGN.md`.
pub const DEFAULT: &str = include_str!("../DESIGN.md");

fn scheme_key(scheme: Scheme) -> &'static str {
    match scheme {
        Scheme::Light => "light",
        Scheme::Dark => "dark",
    }
}

/// Parses `src` and returns the [`Seeds`] for `scheme`, overlaying any tokens
/// the file defines onto the defaults.
///
/// # Errors
///
/// Returns [`DesignError`] if the frontmatter is missing/invalid or a color
/// value is malformed.
pub fn seeds(src: &str, scheme: Scheme) -> Result<Seeds, DesignError> {
    let doc = parse(src)?;
    let key = scheme_key(scheme);
    let mut s = seeds_for(scheme);
    if let Some(c) = doc.color("ink", key) {
        s.ink = c;
    }
    if let Some(c) = doc.color("paper", key) {
        s.paper = c;
    }
    if let Some(c) = doc.color("desktop", key) {
        s.desktop = c;
    }
    if let Some(c) = doc.color("muted", key) {
        s.muted = c;
    }
    Ok(s)
}

/// Builds a System 6 [`Theme`] for `scheme` from a `DESIGN.md` source.
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
    use crate::palette::{classic_seeds, dark_seeds};

    #[test]
    fn bundled_design_md_matches_classic_defaults() {
        assert_eq!(
            seeds(DEFAULT, Scheme::Light).expect("DESIGN.md parses"),
            classic_seeds(),
            "DESIGN.md light tokens drifted from palette::classic_seeds()"
        );
    }

    #[test]
    fn bundled_design_md_matches_dark_defaults() {
        assert_eq!(
            seeds(DEFAULT, Scheme::Dark).expect("DESIGN.md parses"),
            dark_seeds(),
            "DESIGN.md dark tokens drifted from palette::dark_seeds()"
        );
    }

    #[test]
    fn bad_color_is_an_error() {
        let src = "---\ncolors:\n  ink: \"nope\"\n---\n";
        assert!(matches!(
            seeds(src, Scheme::Light),
            Err(DesignError::BadColor { .. })
        ));
    }
}
