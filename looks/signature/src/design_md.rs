// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Loading Signature [`Seeds`] from a `DESIGN.md` token table.
//!
//! `DESIGN.md` is the Markdown form of [`Seeds`]: a `## Colors` table mapping
//! each role to a per-scheme hex color, and a `## Motion` table mapping each
//! duration token to milliseconds. The parser is deliberately small and
//! forgiving — it reads the two tables, ignores everything else, and overlays
//! whatever roles it finds onto the Rust defaults, so a partial file is fine.
//!
//! ```ignore
//! let theme = overstory_look_signature::design_md::theme_from_design_md(
//!     overstory_look_signature::design_md::DEFAULT,
//!     Scheme::Dark,
//!     &tokens,
//! )?;
//! ```

use core::fmt;

use overstory_look::design_md::{parse_hex_color, table_rows};
use peniko::Color;
use understory_style::Theme;

use crate::palette::{Seeds, seeds_for};
use crate::theme::theme_from_seeds;
use crate::tokens::{ResolvedTokens, Scheme};

/// The crate's bundled `DESIGN.md`, embedded so the default theme can be built
/// from the same source designers edit.
pub const DEFAULT: &str = include_str!("../DESIGN.md");

/// A failure parsing a `DESIGN.md` token table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DesignError {
    /// A color cell was not a valid `#rrggbb` / `#rrggbbaa` hex string.
    BadColor {
        /// The role whose color failed to parse.
        role: String,
        /// The offending cell text.
        value: String,
    },
    /// A motion cell was not a valid integer millisecond count.
    BadDuration {
        /// The motion token whose value failed to parse.
        token: String,
        /// The offending cell text.
        value: String,
    },
}

impl fmt::Display for DesignError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadColor { role, value } => {
                write!(f, "invalid color for role `{role}`: {value:?}")
            }
            Self::BadDuration { token, value } => {
                write!(f, "invalid duration for token `{token}`: {value:?}")
            }
        }
    }
}

impl std::error::Error for DesignError {}

/// Parses `src` and returns the [`Seeds`] for `scheme`, starting from the
/// defaults and overlaying any roles the file defines.
///
/// # Errors
///
/// Returns [`DesignError`] if a color or duration cell is present but malformed.
pub fn seeds(src: &str, scheme: Scheme) -> Result<Seeds, DesignError> {
    let mut seeds = seeds_for(scheme);
    // Colors table columns are `dark | light`.
    let color_index = match scheme {
        Scheme::Dark => 0,
        Scheme::Light => 1,
    };
    for row in table_rows(src) {
        if row.section.starts_with("color") {
            let Some(cell) = row
                .cells
                .get(color_index)
                .copied()
                .filter(|c| !c.is_empty())
            else {
                continue;
            };
            let color = parse_hex_color(cell).ok_or_else(|| DesignError::BadColor {
                role: row.key.to_string(),
                value: cell.to_string(),
            })?;
            apply_color(&mut seeds, row.key, color);
        } else if row.section.starts_with("motion") {
            let value = row.cells.first().copied().unwrap_or("");
            let ms = value.parse::<u64>().map_err(|_| DesignError::BadDuration {
                token: row.key.to_string(),
                value: value.to_string(),
            })?;
            apply_motion(&mut seeds, row.key, ms);
        }
    }
    Ok(seeds)
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

fn apply_color(seeds: &mut Seeds, role: &str, color: Color) {
    match role {
        "accent" => seeds.accent = color,
        "surface" => seeds.surface = color,
        "panel" => seeds.panel = color,
        "card" => seeds.card = color,
        "control" => seeds.control = color,
        "control-border" => seeds.control_border = color,
        "track" => seeds.track = color,
        "thumb" => seeds.thumb = color,
        "on-accent" => seeds.on_accent = color,
        "on-surface" => seeds.on_surface = color,
        "muted" => seeds.muted = color,
        "heading" => seeds.heading = color,
        "body" => seeds.body = color,
        "focus" => seeds.focus = color,
        _ => {}
    }
}

fn apply_motion(seeds: &mut Seeds, token: &str, ms: u64) {
    match token {
        "fast" => seeds.motion_fast_ms = ms,
        "normal" => seeds.motion_normal_ms = ms,
        "emphasized" => seeds.motion_emphasized_ms = ms,
        "glow" => seeds.motion_glow_ms = ms,
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::{dark_seeds, light_seeds};

    #[test]
    fn bundled_design_md_matches_rust_dark_defaults() {
        let parsed = seeds(DEFAULT, Scheme::Dark).expect("DESIGN.md parses");
        assert_eq!(
            parsed,
            dark_seeds(),
            "DESIGN.md dark column drifted from palette::dark_seeds()"
        );
    }

    #[test]
    fn bundled_design_md_matches_rust_light_defaults() {
        let parsed = seeds(DEFAULT, Scheme::Light).expect("DESIGN.md parses");
        assert_eq!(
            parsed,
            light_seeds(),
            "DESIGN.md light column drifted from palette::light_seeds()"
        );
    }

    #[test]
    fn overrides_are_applied_and_missing_roles_fall_back() {
        let src = "## Colors\n\
                   | role | dark | light |\n\
                   | --- | --- | --- |\n\
                   | accent | #112233 | #445566 |\n\
                   ## Motion\n\
                   | token | ms |\n\
                   | --- | --- |\n\
                   | fast | 42 |\n";
        let s = seeds(src, Scheme::Dark).expect("parses");
        assert_eq!(
            s.accent.to_rgba8(),
            Color::from_rgba8(0x11, 0x22, 0x33, 0xff).to_rgba8()
        );
        assert_eq!(s.motion_fast_ms, 42);
        // Untouched roles keep their defaults.
        assert_eq!(s.surface.to_rgba8(), dark_seeds().surface.to_rgba8());
        assert_eq!(s.motion_normal_ms, dark_seeds().motion_normal_ms);
    }

    #[test]
    fn bad_color_is_an_error() {
        let src = "## Colors\n| role | dark | light |\n| accent | nope | #445566 |\n";
        let err = seeds(src, Scheme::Dark).unwrap_err();
        assert!(matches!(err, DesignError::BadColor { .. }));
    }
}
