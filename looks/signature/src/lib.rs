// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! # Signature
//!
//! A token-driven, gradient-forward design system for
//! [Overstory](https://github.com/forest-rs/overstory).
//!
//! Signature is a *separate* look layered on top of Overstory's built-in
//! controls. It exists for two reasons:
//!
//! 1. To produce a deliberate, cohesive theme — subtle multi-stop gradient
//!    surfaces, gradient-filled accents, glow focus rings, and lively
//!    interaction motion.
//! 2. To exercise the toolkit hard enough to surface missing capabilities.
//!    Gaps found while building Signature are tracked in `GAPS.md` next to this
//!    crate, separated into "themeable today" and "needs an `understory_*`
//!    change".
//!
//! ## Architecture
//!
//! Everything is a **token**. No color, duration, or radius literal lives in a
//! style rule — rules only reference [`tokens`] by [`ResourceKey`] or
//! [`ClassId`]. The literal values live in exactly two places: the [`palette`]
//! module (where tokens are *defined* in Rust) and `DESIGN.md` (the same token
//! table authored as Markdown, loaded by [`design_md`]).
//!
//! Signature reuses Overstory's built-in palette roles (`overstory::theme::*`,
//! e.g. [`ACCENT_BG`](tokens::ACCENT_BG)) so the framework's default control
//! cascade re-skins automatically, and adds extended roles (gradients, glow,
//! elevation) that its own [`cascade`] consumes.
//!
//! ## Usage
//!
//! ```ignore
//! let mut ui = overstory::Ui::new();
//! overstory_look_signature::install(&mut ui, Scheme::Dark);
//! // ... build your tree using `tokens::CLASS_*` for chrome ...
//! ```

pub mod cascade;
pub mod design_md;
pub mod motion;
pub mod palette;
pub mod theme;
pub mod tokens;

pub use cascade::cascade;
pub use design_md::{DesignError, theme_from_design_md};
pub use palette::{Seeds, dark_seeds, light_seeds, seeds_for};
pub use theme::{theme, theme_from_seeds};
pub use tokens::{ResolvedTokens, Scheme, SignatureTokens};

use overstory::Ui;

/// Installs the Signature look on `ui` for the given [`Scheme`].
///
/// The theme is built from the bundled [`DESIGN.md`](design_md::DEFAULT) — the
/// real source of truth — so editing that file changes the rendered look. The
/// Rust seeds in [`palette`] are only the fallback if the embedded file somehow
/// fails to parse. The cascade is then layered over the framework defaults.
///
/// Returns the [`ResolvedTokens`] so callers can keep styling app chrome with
/// the Signature classes and extended resources.
pub fn install(ui: &mut Ui, scheme: Scheme) -> ResolvedTokens {
    let tokens = ui.style_tokens::<SignatureTokens>();
    let palette = theme_from_design_md(design_md::DEFAULT, scheme, &tokens)
        .unwrap_or_else(|_| theme(scheme, &tokens));
    ui.set_theme(palette);
    let sheet = cascade(ui.properties(), &tokens);
    ui.set_cascade(sheet);
    tokens
}

/// Like [`install`], but builds the theme from a `DESIGN.md` token source
/// instead of the Rust defaults. Pass [`design_md::DEFAULT`] for the bundled
/// table, or your own edited copy.
///
/// # Errors
///
/// Propagates any [`DesignError`] from parsing `design_md_src`.
pub fn install_from_design_md(
    ui: &mut Ui,
    design_md_src: &str,
    scheme: Scheme,
) -> Result<ResolvedTokens, DesignError> {
    let tokens = ui.style_tokens::<SignatureTokens>();
    ui.set_theme(theme_from_design_md(design_md_src, scheme, &tokens)?);
    let sheet = cascade(ui.properties(), &tokens);
    ui.set_cascade(sheet);
    Ok(tokens)
}
