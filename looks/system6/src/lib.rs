// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! # System 6
//!
//! A 1-bit black-and-white classic Mac look for
//! [Overstory](https://github.com/forest-rs/overstory): paper surfaces inside
//! 1px ink frames, black text, rounded push buttons that invert on press,
//! square dialogs and fields, and a gray desktop. No motion.
//!
//! It's the most reductive of the looks — proof that the same token system that
//! drives Signature's gradients also expresses a two-color world.
//!
//! ## Usage
//!
//! ```ignore
//! let mut ui = overstory::Ui::new();
//! overstory_look_system6::install(&mut ui, Scheme::Light);
//! ```

pub mod cascade;
pub mod design_md;
pub mod palette;
pub mod templates;
pub mod theme;
pub mod tokens;

pub use cascade::cascade;
pub use design_md::{DesignError, theme_from_design_md};
pub use palette::{Seeds, classic_seeds, dark_seeds, seeds_for};
pub use theme::{theme, theme_from_seeds};
pub use tokens::{ResolvedTokens, Scheme, System6Tokens};

use overstory::Ui;

/// Installs the System 6 look on `ui` for the given [`Scheme`].
///
/// The theme is built from the bundled [`DESIGN.md`](design_md::DEFAULT) — the
/// source of truth — falling back to the Rust seeds in [`palette`] only if the
/// embedded file fails to parse. The cascade is layered over the framework
/// defaults. Returns the [`ResolvedTokens`].
pub fn install(ui: &mut Ui, scheme: Scheme) -> ResolvedTokens {
    let tokens = ui.style_tokens::<System6Tokens>();
    let palette = theme_from_design_md(design_md::DEFAULT, scheme, &tokens)
        .unwrap_or_else(|_| theme(scheme, &tokens));
    ui.set_theme(palette);
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
    let tokens = ui.style_tokens::<System6Tokens>();
    ui.set_theme(theme_from_design_md(design_md_src, scheme, &tokens)?);
    let sheet = cascade(ui.properties(), &tokens);
    ui.set_cascade(sheet);
    Ok(tokens)
}
