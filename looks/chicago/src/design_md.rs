// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Loading Chicago [`Seeds`] from a `DESIGN.md` color table.
//!
//! Chicago uses the same lightweight Markdown token format as the other looks:
//! a `## Colors` table mapping each role to a per-scheme hex color. (Motion is
//! fixed — period UI did not animate — so there is no motion table.) The parser
//! overlays whatever roles it finds onto the Rust defaults, so a partial file
//! is fine.

use core::fmt;

use peniko::Color;
use understory_style::Theme;

use crate::palette::{Seeds, seeds_for};
use crate::theme::theme_from_seeds;
use crate::tokens::{ResolvedTokens, Scheme};

/// The crate's bundled `DESIGN.md`.
pub const DEFAULT: &str = include_str!("../DESIGN.md");

/// A failure parsing a `DESIGN.md` color table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DesignError {
    /// A color cell was not a valid `#rrggbb` / `#rrggbbaa` hex string.
    BadColor {
        /// The role whose color failed to parse.
        role: String,
        /// The offending cell text.
        value: String,
    },
}

impl fmt::Display for DesignError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self::BadColor { role, value } = self;
        write!(f, "invalid color for role `{role}`: {value:?}")
    }
}

impl std::error::Error for DesignError {}

/// Parses `src` and returns the [`Seeds`] for `scheme`, overlaying any roles the
/// file defines onto the defaults.
///
/// # Errors
///
/// Returns [`DesignError`] if a color cell is present but malformed.
pub fn seeds(src: &str, scheme: Scheme) -> Result<Seeds, DesignError> {
    let mut seeds = seeds_for(scheme);
    for (role, cells) in color_rows(src) {
        let index = match scheme {
            Scheme::Light => 0,
            Scheme::Dark => 1,
        };
        let Some(cell) = cells.get(index).copied().filter(|c| !c.is_empty()) else {
            continue;
        };
        let color = parse_color(cell).ok_or_else(|| DesignError::BadColor {
            role: role.to_string(),
            value: cell.to_string(),
        })?;
        apply_color(&mut seeds, role, color);
    }
    Ok(seeds)
}

/// Builds a Chicago [`Theme`] for `scheme` from a `DESIGN.md` source.
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

fn color_rows(src: &str) -> Vec<(&str, Vec<&str>)> {
    let mut rows = Vec::new();
    let mut in_colors = false;
    for line in src.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("##") {
            in_colors = heading.trim().to_ascii_lowercase().starts_with("color");
            continue;
        }
        if !in_colors || !trimmed.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = trimmed
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        let Some((role, rest)) = cells.split_first() else {
            continue;
        };
        if role.eq_ignore_ascii_case("role") || role.chars().all(|c| c == '-' || c == ':') {
            continue;
        }
        rows.push((*role, rest.to_vec()));
    }
    rows
}

fn parse_color(cell: &str) -> Option<Color> {
    let hex = cell.strip_prefix('#')?;
    let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    match hex.len() {
        6 => Some(Color::from_rgba8(byte(0)?, byte(2)?, byte(4)?, 0xff)),
        8 => Some(Color::from_rgba8(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
        _ => None,
    }
}

fn apply_color(seeds: &mut Seeds, role: &str, color: Color) {
    match role {
        "face" => seeds.face = color,
        "window" => seeds.window = color,
        "bevel-light" => seeds.bevel_light = color,
        "bevel-dark" => seeds.bevel_dark = color,
        "accent" => seeds.accent = color,
        "text" => seeds.text = color,
        "muted" => seeds.muted = color,
        "on-accent" => seeds.on_accent = color,
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::{classic_seeds, dark_chrome_seeds};

    #[test]
    fn bundled_design_md_matches_classic_defaults() {
        assert_eq!(
            seeds(DEFAULT, Scheme::Light).expect("DESIGN.md parses"),
            classic_seeds(),
            "DESIGN.md light column drifted from palette::classic_seeds()"
        );
    }

    #[test]
    fn bundled_design_md_matches_dark_defaults() {
        assert_eq!(
            seeds(DEFAULT, Scheme::Dark).expect("DESIGN.md parses"),
            dark_chrome_seeds(),
            "DESIGN.md dark column drifted from palette::dark_chrome_seeds()"
        );
    }

    #[test]
    fn bad_color_is_an_error() {
        let src = "## Colors\n| role | light | dark |\n| face | nope | #000000 |\n";
        assert!(matches!(
            seeds(src, Scheme::Light).unwrap_err(),
            DesignError::BadColor { .. }
        ));
    }
}
