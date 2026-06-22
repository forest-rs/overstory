// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! # Chicago
//!
//! A flat, beveled, square-cornered retro look for
//! [Overstory](https://github.com/forest-rs/overstory) — a deliberate homage to
//! the mid-90s desktop, and the opposite end of the design space from the
//! gradient-forward `overstory_look_signature`.
//!
//! Chicago exists to prove the token system isn't secretly gradient-only: it
//! uses flat fills, square corners, and 3D bevels built from per-side border
//! brushes (light top/left, dark bottom/right; inverted when pressed). Motion is
//! effectively instant.
//!
//! ## Usage
//!
//! ```ignore
//! let mut ui = overstory::Ui::new();
//! overstory_look_chicago::install(&mut ui, Scheme::Light);
//! ```

pub mod cascade;
pub mod design_md;
pub mod palette;
pub mod theme;
pub mod tokens;

pub use cascade::cascade;
pub use design_md::{DesignError, theme_from_design_md};
pub use palette::{Seeds, classic_seeds, dark_chrome_seeds, seeds_for};
pub use theme::{theme, theme_from_seeds};
pub use tokens::{ChicagoTokens, ResolvedTokens, Scheme};

use overstory::Ui;

/// Installs the Chicago look on `ui` for the given [`Scheme`]: resolves the
/// extended tokens, builds and applies the theme, and layers the Chicago
/// cascade over the framework defaults. Returns the [`ResolvedTokens`].
pub fn install(ui: &mut Ui, scheme: Scheme) -> ResolvedTokens {
    let tokens = ui.style_tokens::<ChicagoTokens>();
    ui.set_theme(theme(scheme, &tokens));
    let sheet = cascade(ui.properties(), &tokens);
    ui.set_cascade(sheet);
    tokens
}

/// Like [`install`], but builds the theme from a `DESIGN.md` token source.
///
/// # Errors
///
/// Propagates any [`DesignError`] from parsing `design_md_src`.
pub fn install_from_design_md(
    ui: &mut Ui,
    design_md_src: &str,
    scheme: Scheme,
) -> Result<ResolvedTokens, DesignError> {
    let tokens = ui.style_tokens::<ChicagoTokens>();
    ui.set_theme(theme_from_design_md(design_md_src, scheme, &tokens)?);
    let sheet = cascade(ui.properties(), &tokens);
    ui.set_cascade(sheet);
    Ok(tokens)
}
