// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A minimal Markdown token-table reader shared by every look's `DESIGN.md`.
//!
//! The format is intentionally tiny: `##`-headed sections, each containing a
//! pipe table whose first column is a role/token name and whose remaining
//! columns are per-scheme (or per-unit) values. Header and separator rows are
//! skipped. Each look maps the rows it cares about onto its own seeds.
//!
//! This is a convention of this workspace, not an external standard.

use peniko::Color;

/// One data row of a token table: its section, key, and value cells.
#[derive(Clone, Debug)]
pub struct TableRow<'a> {
    /// The `##` section heading this row sits under, lowercased and trimmed.
    pub section: String,
    /// The first column — the role or token name.
    pub key: &'a str,
    /// The remaining columns, trimmed, in order.
    pub cells: Vec<&'a str>,
}

/// Reads every data row of every `##` section's pipe table in `src`.
///
/// Header rows (`role` / `token`) and separator rows (`---`) are dropped. Cells
/// are trimmed. Non-table lines are ignored, so prose between tables is fine.
#[must_use]
pub fn table_rows(src: &str) -> Vec<TableRow<'_>> {
    let mut rows = Vec::new();
    let mut section: Option<String> = None;
    for line in src.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("##") {
            section = Some(heading.trim().to_ascii_lowercase());
            continue;
        }
        let Some(section) = section.as_deref() else {
            continue;
        };
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
        if key.eq_ignore_ascii_case("role")
            || key.eq_ignore_ascii_case("token")
            || (!key.is_empty() && key.chars().all(|c| c == '-' || c == ':'))
        {
            continue;
        }
        rows.push(TableRow {
            section: section.to_owned(),
            key,
            cells: rest.to_vec(),
        });
    }
    rows
}

/// Parses a `#rrggbb` or `#rrggbbaa` hex string into an sRGB color.
#[must_use]
pub fn parse_hex_color(cell: &str) -> Option<Color> {
    let hex = cell.strip_prefix('#')?;
    let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    match hex.len() {
        6 => Some(Color::from_rgba8(byte(0)?, byte(2)?, byte(4)?, 0xff)),
        8 => Some(Color::from_rgba8(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_rows_by_section_and_skips_headers() {
        let src = "## Colors\n\
                   | role | dark | light |\n\
                   | --- | --- | --- |\n\
                   | accent | #112233 | #445566 |\n\
                   ## Motion\n\
                   | token | ms |\n\
                   | fast | 42 |\n";
        let rows = table_rows(src);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].section, "colors");
        assert_eq!(rows[0].key, "accent");
        assert_eq!(rows[0].cells, ["#112233", "#445566"]);
        assert_eq!(rows[1].section, "motion");
        assert_eq!(rows[1].key, "fast");
        assert_eq!(rows[1].cells, ["42"]);
    }

    #[test]
    fn parses_hex() {
        assert_eq!(
            parse_hex_color("#112233").map(Color::to_rgba8),
            Some(Color::from_rgba8(0x11, 0x22, 0x33, 0xff).to_rgba8())
        );
        assert!(parse_hex_color("nope").is_none());
    }
}
