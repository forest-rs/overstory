// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A reader for the [Google Labs DESIGN.md format][spec] (the `alpha` spec):
//! YAML frontmatter carrying machine-readable design tokens, followed by a
//! Markdown body of human-readable rationale.
//!
//! This reads the token frontmatter — `name`, `description`, `colors`,
//! `rounded`, `spacing`, and (as a documented extension) `motion` — into a
//! [`DesignDoc`]. The Markdown body is not interpreted.
//!
//! Two deviations from the base spec, both deliberate:
//!
//! - **Per-scheme colors.** The spec's `Color` is a single value (one identity
//!   per file). Overstory looks carry light *and* dark, so a color value may
//!   also be a `{ light: <Color>, dark: <Color> }` map. A plain scalar applies
//!   to every scheme.
//! - **Motion.** The spec has no motion tokens; looks may add a `motion:`
//!   block mapping a token name to a millisecond count.
//!
//! Color values accept any CSS color the `color` crate parses (hex, `rgb()`,
//! `oklch()`, named); hex takes an exact fast path.
//!
//! [spec]: https://github.com/google-labs-code/design.md

use std::collections::BTreeMap;
use std::fmt;

use peniko::Color;
use peniko::color::{Srgb, parse_color};
use yaml_rust2::{Yaml, YamlLoader};

/// A failure reading a DESIGN.md file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DesignError {
    /// The file has no `---`-fenced YAML frontmatter.
    MissingFrontmatter,
    /// The frontmatter is not valid YAML.
    Yaml(String),
    /// A color value was not a parseable CSS color.
    BadColor {
        /// The color role.
        role: String,
        /// The offending value text.
        value: String,
    },
    /// A motion value was not a non-negative integer millisecond count.
    BadDuration {
        /// The motion token.
        token: String,
        /// The offending value text.
        value: String,
    },
}

impl fmt::Display for DesignError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingFrontmatter => write!(f, "DESIGN.md has no `---` YAML frontmatter"),
            Self::Yaml(msg) => write!(f, "invalid DESIGN.md frontmatter: {msg}"),
            Self::BadColor { role, value } => {
                write!(f, "invalid color for `{role}`: {value:?}")
            }
            Self::BadDuration { token, value } => {
                write!(f, "invalid duration for `{token}`: {value:?}")
            }
        }
    }
}

impl std::error::Error for DesignError {}

/// A color token: one value, or one per named scheme.
#[derive(Clone, Debug)]
pub enum ColorValue {
    /// One color for every scheme.
    Single(Color),
    /// A color per scheme name (e.g. `light`, `dark`).
    PerScheme(BTreeMap<String, Color>),
}

/// The parsed token frontmatter of a DESIGN.md file.
#[derive(Clone, Debug, Default)]
pub struct DesignDoc {
    /// The `name` token.
    pub name: Option<String>,
    /// The `description` token.
    pub description: Option<String>,
    /// `colors` token map (role → value).
    pub colors: BTreeMap<String, ColorValue>,
    /// `rounded` scale (level → dimension string).
    pub rounded: BTreeMap<String, String>,
    /// `spacing` scale (level → dimension string).
    pub spacing: BTreeMap<String, String>,
    /// `motion` extension (token → milliseconds).
    pub motion: BTreeMap<String, u64>,
}

impl DesignDoc {
    /// Resolves color `role` for `scheme`, falling back to a single value.
    #[must_use]
    pub fn color(&self, role: &str, scheme: &str) -> Option<Color> {
        match self.colors.get(role)? {
            ColorValue::Single(c) => Some(*c),
            ColorValue::PerScheme(by_scheme) => by_scheme.get(scheme).copied(),
        }
    }

    /// Returns the millisecond count for motion `token`, if present.
    #[must_use]
    pub fn motion_ms(&self, token: &str) -> Option<u64> {
        self.motion.get(token).copied()
    }
}

/// Parses the YAML frontmatter of a DESIGN.md `src`.
///
/// # Errors
///
/// Returns [`DesignError`] if the frontmatter is missing, not valid YAML, or
/// contains a malformed color or duration value.
pub fn parse(src: &str) -> Result<DesignDoc, DesignError> {
    let frontmatter = frontmatter(src).ok_or(DesignError::MissingFrontmatter)?;
    let docs =
        YamlLoader::load_from_str(frontmatter).map_err(|e| DesignError::Yaml(e.to_string()))?;
    let root = docs
        .into_iter()
        .next()
        .ok_or(DesignError::MissingFrontmatter)?;

    let mut doc = DesignDoc {
        name: root["name"].as_str().map(str::to_owned),
        description: root["description"].as_str().map(str::to_owned),
        ..DesignDoc::default()
    };

    if let Yaml::Hash(colors) = &root["colors"] {
        for (key, value) in colors {
            let Some(role) = key.as_str() else { continue };
            doc.colors
                .insert(role.to_owned(), parse_color_value(role, value)?);
        }
    }

    doc.rounded = string_map(&root["rounded"]);
    doc.spacing = string_map(&root["spacing"]);

    if let Yaml::Hash(motion) = &root["motion"] {
        for (key, value) in motion {
            let Some(token) = key.as_str() else { continue };
            let ms = yaml_u64(value).ok_or_else(|| DesignError::BadDuration {
                token: token.to_owned(),
                value: yaml_scalar(value),
            })?;
            doc.motion.insert(token.to_owned(), ms);
        }
    }

    Ok(doc)
}

/// Returns the text between the leading `---` fence and the next `---` line.
fn frontmatter(src: &str) -> Option<&str> {
    let rest = src.strip_prefix("---")?;
    let rest = rest
        .strip_prefix('\n')
        .or_else(|| rest.strip_prefix("\r\n"))?;
    let end = rest.find("\n---")?;
    Some(&rest[..end])
}

fn parse_color_value(role: &str, value: &Yaml) -> Result<ColorValue, DesignError> {
    match value {
        Yaml::String(s) => Ok(ColorValue::Single(parse_css_color(s).ok_or_else(|| {
            DesignError::BadColor {
                role: role.to_owned(),
                value: s.clone(),
            }
        })?)),
        Yaml::Hash(by_scheme) => {
            let mut map = BTreeMap::new();
            for (key, color) in by_scheme {
                let (Some(scheme), Some(text)) = (key.as_str(), color.as_str()) else {
                    continue;
                };
                let parsed = parse_css_color(text).ok_or_else(|| DesignError::BadColor {
                    role: format!("{role}.{scheme}"),
                    value: text.to_owned(),
                })?;
                map.insert(scheme.to_owned(), parsed);
            }
            Ok(ColorValue::PerScheme(map))
        }
        other => Err(DesignError::BadColor {
            role: role.to_owned(),
            value: yaml_scalar(other),
        }),
    }
}

/// Parses a CSS color string. Hex takes an exact `from_rgba8` fast path so hex
/// tokens round-trip bit-for-bit; everything else goes through the `color`
/// crate (`rgb()`, `oklch()`, named, …) and is converted to sRGB.
#[must_use]
pub fn parse_css_color(text: &str) -> Option<Color> {
    let text = text.trim();
    if let Some(hex) = parse_hex(text) {
        return Some(hex);
    }
    parse_color(text).ok().map(|c| c.to_alpha_color::<Srgb>())
}

fn parse_hex(text: &str) -> Option<Color> {
    let hex = text.strip_prefix('#')?;
    let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    match hex.len() {
        6 => Some(Color::from_rgba8(byte(0)?, byte(2)?, byte(4)?, 0xff)),
        8 => Some(Color::from_rgba8(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
        _ => None,
    }
}

fn string_map(value: &Yaml) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    if let Yaml::Hash(entries) = value {
        for (key, val) in entries {
            if let Some(name) = key.as_str() {
                map.insert(name.to_owned(), yaml_scalar(val));
            }
        }
    }
    map
}

fn yaml_u64(value: &Yaml) -> Option<u64> {
    match value {
        Yaml::Integer(i) => u64::try_from(*i).ok(),
        Yaml::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn yaml_scalar(value: &Yaml) -> String {
    match value {
        Yaml::String(s) => s.clone(),
        Yaml::Integer(i) => i.to_string(),
        Yaml::Real(r) => r.clone(),
        Yaml::Boolean(b) => b.to_string(),
        _ => String::from("<non-scalar>"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "---\n\
        name: Sample\n\
        colors:\n\
        \x20 accent: { light: \"#112233\", dark: \"#445566\" }\n\
        \x20 paper: \"#ffffff\"\n\
        rounded:\n\
        \x20 md: 12px\n\
        motion:\n\
        \x20 fast: 42\n\
        ---\n\
        # Body\n";

    #[test]
    fn parses_frontmatter_tokens() {
        let doc = parse(SAMPLE).expect("parses");
        assert_eq!(doc.name.as_deref(), Some("Sample"));
        assert_eq!(
            doc.color("accent", "light").map(Color::to_rgba8),
            Some(Color::from_rgba8(0x11, 0x22, 0x33, 0xff).to_rgba8())
        );
        assert_eq!(
            doc.color("accent", "dark").map(Color::to_rgba8),
            Some(Color::from_rgba8(0x44, 0x55, 0x66, 0xff).to_rgba8())
        );
        // A single scalar applies to every scheme.
        assert_eq!(
            doc.color("paper", "dark").map(Color::to_rgba8),
            Some(Color::from_rgba8(0xff, 0xff, 0xff, 0xff).to_rgba8())
        );
        assert_eq!(doc.rounded.get("md").map(String::as_str), Some("12px"));
        assert_eq!(doc.motion_ms("fast"), Some(42));
    }

    #[test]
    fn missing_frontmatter_is_an_error() {
        assert!(matches!(
            parse("# no frontmatter\n"),
            Err(DesignError::MissingFrontmatter)
        ));
    }

    #[test]
    fn bad_color_is_an_error() {
        let src = "---\ncolors:\n  accent: \"not-a-color\"\n---\n";
        assert!(matches!(parse(src), Err(DesignError::BadColor { .. })));
    }

    #[test]
    fn non_hex_css_colors_parse() {
        assert!(parse_css_color("oklch(62% 0.18 250)").is_some());
        assert!(parse_css_color("rebeccapurple").is_some());
    }
}
