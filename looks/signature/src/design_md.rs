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
    for row in table_rows(src) {
        match row.section {
            Section::Colors => {
                let Some(cell) = row.color_cell(scheme) else {
                    continue;
                };
                let color = parse_color(cell).ok_or_else(|| DesignError::BadColor {
                    role: row.key.to_string(),
                    value: cell.to_string(),
                })?;
                apply_color(&mut seeds, row.key, color);
            }
            Section::Motion => {
                let value = row.cells.first().copied().unwrap_or("");
                let ms = value.parse::<u64>().map_err(|_| DesignError::BadDuration {
                    token: row.key.to_string(),
                    value: value.to_string(),
                })?;
                apply_motion(&mut seeds, row.key, ms);
            }
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

// --- Parsing ----------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Colors,
    Motion,
}

struct Row<'a> {
    section: Section,
    key: &'a str,
    cells: Vec<&'a str>,
}

impl Row<'_> {
    /// The hex cell for `scheme` in a colors row (`| role | dark | light |`).
    fn color_cell(&self, scheme: Scheme) -> Option<&str> {
        let index = match scheme {
            Scheme::Dark => 0,
            Scheme::Light => 1,
        };
        self.cells.get(index).copied().filter(|c| !c.is_empty())
    }
}

/// Yields the data rows of the `## Colors` and `## Motion` tables.
fn table_rows(src: &str) -> Vec<Row<'_>> {
    let mut rows = Vec::new();
    let mut section: Option<Section> = None;
    for line in src.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("##") {
            let name = heading.trim().to_ascii_lowercase();
            section = if name.starts_with("color") {
                Some(Section::Colors)
            } else if name.starts_with("motion") {
                Some(Section::Motion)
            } else {
                None
            };
            continue;
        }
        let Some(section) = section else { continue };
        if !trimmed.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = trimmed
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        let Some((key, rest)) = cells.split_first() else {
            continue;
        };
        // Skip header and separator rows.
        if key.eq_ignore_ascii_case("role") || key.eq_ignore_ascii_case("token") {
            continue;
        }
        if key.chars().all(|c| c == '-' || c == ':') {
            continue;
        }
        rows.push(Row {
            section,
            key,
            cells: rest.to_vec(),
        });
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
