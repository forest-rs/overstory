// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The Signature token taxonomy.
//!
//! Tokens come in two families:
//!
//! - **Built-in roles** — the palette keys Overstory's framework cascade
//!   already references (`overstory::theme::*`). Signature redefines their
//!   *values* (often as gradients) so built-in controls re-skin for free, and
//!   re-exports the keys here so the loader and cascade can name the whole
//!   palette from one module.
//! - **Extended roles** — surfaces and effects the built-in palette has no slot
//!   for: cards, the hairline highlight, the focus glow, chrome typography, and
//!   a couple of extra motion curves. These are *vocabulary-allocated*
//!   ([`ResourceKey`]s handed out by the `Ui`'s [`StyleVocabulary`]) rather than
//!   hardcoded indices, because a fresh vocabulary numbers from `1` and would
//!   otherwise collide with the built-in keys. Resolve them once via
//!   [`Ui::style_tokens`](overstory::Ui::style_tokens).
//!
//! Class tokens name the chrome surfaces (root, panel, card, …) so app layout
//! can opt into Signature treatment without control-specific code.

use understory_style::{ClassId, ResourceKey, StyleTokenSet, StyleVocabulary};

/// Built-in palette roles Signature redefines and reuses. Re-exported so the
/// rest of the crate (and `DESIGN.md`) can name the full palette from one path.
pub use overstory::theme::{
    ACCENT_BG, ACCENT_HOVER_BG, ACCENT_PRESSED_BG, CONTROL_BG, CONTROL_BORDER, CONTROL_HOVER_BG,
    CONTROL_HOVER_BORDER, FOCUS_RING_BORDER, MOTION_COLOR_INTERPOLATION,
    MOTION_EMPHASIZED_DURATION, MOTION_FAST_DURATION, MOTION_NORMAL_DURATION,
    MOTION_STANDARD_EASING, ON_ACCENT_FG, ON_SURFACE_FG, ON_SURFACE_MUTED_FG, PANEL_BG,
    SLIDER_MAJOR_TICK_BG, SLIDER_MINOR_TICK_BG, SUCCESS_BG, SURFACE_BG, TEXT_CARET_BG,
    TEXT_SELECTION_BG, THUMB_BG, TRACK_BG,
};

/// Which color scheme a theme is built for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Scheme {
    /// Dark surfaces, light foreground.
    Dark,
    /// Light surfaces, dark foreground.
    Light,
}

impl Scheme {
    /// Toggles between [`Scheme::Dark`] and [`Scheme::Light`].
    #[must_use]
    pub fn toggled(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }
}

// --- Extended resource role names ------------------------------------------
//
// Authored as stable strings so `DESIGN.md` can address the same roles. The
// resolved `ResourceKey`s come from the vocabulary at install time.

/// Card gradient (a subtler raise than a panel; cards group content).
pub const RES_CARD_GRADIENT: &str = "signature.card.gradient";
/// Card border brush.
pub const RES_CARD_BORDER: &str = "signature.card.border";
/// Hairline top-highlight border brush for raised surfaces.
pub const RES_HAIRLINE: &str = "signature.hairline";
/// Focus glow brush (translucent accent halo).
pub const RES_FOCUS_GLOW: &str = "signature.focus.glow";
/// Strong heading foreground (chrome typography).
pub const RES_HEADING_FG: &str = "signature.heading.fg";
/// Body-copy foreground (chrome typography).
pub const RES_BODY_FG: &str = "signature.body.fg";
/// Overshoot easing curve for lively press/hover motion.
pub const RES_MOTION_OVERSHOOT_EASING: &str = "signature.motion.overshoot.easing";
/// Slower duration for emphasized glow/elevation motion.
pub const RES_MOTION_GLOW_DURATION: &str = "signature.motion.glow.duration";

// --- Class role names ------------------------------------------------------

/// App root backdrop.
pub const CLASS_ROOT: &str = ".sig-root";
/// Flat surface region.
pub const CLASS_SURFACE: &str = ".sig-surface";
/// Raised panel.
pub const CLASS_PANEL: &str = ".sig-panel";
/// Card (subtle raise, used for grouped content).
pub const CLASS_CARD: &str = ".sig-card";
/// Display/heading text.
pub const CLASS_HEADING: &str = ".sig-heading";
/// Body text.
pub const CLASS_BODY: &str = ".sig-body";
/// Muted/caption text.
pub const CLASS_CAPTION: &str = ".sig-caption";

/// Resolved extended resource keys, handed out by the `Ui` vocabulary.
#[derive(Clone, Copy, Debug)]
pub struct SignatureResources {
    /// See [`RES_CARD_GRADIENT`].
    pub card_gradient: ResourceKey,
    /// See [`RES_CARD_BORDER`].
    pub card_border: ResourceKey,
    /// See [`RES_HAIRLINE`].
    pub hairline: ResourceKey,
    /// See [`RES_FOCUS_GLOW`].
    pub focus_glow: ResourceKey,
    /// See [`RES_HEADING_FG`].
    pub heading_fg: ResourceKey,
    /// See [`RES_BODY_FG`].
    pub body_fg: ResourceKey,
    /// See [`RES_MOTION_OVERSHOOT_EASING`].
    pub motion_overshoot_easing: ResourceKey,
    /// See [`RES_MOTION_GLOW_DURATION`].
    pub motion_glow_duration: ResourceKey,
}

/// Resolved chrome class ids.
#[derive(Clone, Copy, Debug)]
pub struct SignatureClasses {
    /// See [`CLASS_ROOT`].
    pub root: ClassId,
    /// See [`CLASS_SURFACE`].
    pub surface: ClassId,
    /// See [`CLASS_PANEL`].
    pub panel: ClassId,
    /// See [`CLASS_CARD`].
    pub card: ClassId,
    /// See [`CLASS_HEADING`].
    pub heading: ClassId,
    /// See [`CLASS_BODY`].
    pub body: ClassId,
    /// See [`CLASS_CAPTION`].
    pub caption: ClassId,
}

/// Everything Signature resolves from the `Ui` vocabulary in one bundle.
#[derive(Clone, Copy, Debug)]
pub struct ResolvedTokens {
    /// Extended resource keys.
    pub resources: SignatureResources,
    /// Chrome class ids.
    pub classes: SignatureClasses,
}

/// [`StyleTokenSet`] that interns every Signature class and extended resource
/// name. Resolve with `ui.style_tokens::<SignatureTokens>()`.
#[derive(Debug)]
pub struct SignatureTokens;

impl StyleTokenSet for SignatureTokens {
    type Resolved = ResolvedTokens;

    fn resolve(vocabulary: &mut StyleVocabulary) -> Self::Resolved {
        ResolvedTokens {
            resources: SignatureResources {
                card_gradient: vocabulary.resource_key(RES_CARD_GRADIENT),
                card_border: vocabulary.resource_key(RES_CARD_BORDER),
                hairline: vocabulary.resource_key(RES_HAIRLINE),
                focus_glow: vocabulary.resource_key(RES_FOCUS_GLOW),
                heading_fg: vocabulary.resource_key(RES_HEADING_FG),
                body_fg: vocabulary.resource_key(RES_BODY_FG),
                motion_overshoot_easing: vocabulary.resource_key(RES_MOTION_OVERSHOOT_EASING),
                motion_glow_duration: vocabulary.resource_key(RES_MOTION_GLOW_DURATION),
            },
            classes: SignatureClasses {
                root: vocabulary.class_id(CLASS_ROOT),
                surface: vocabulary.class_id(CLASS_SURFACE),
                panel: vocabulary.class_id(CLASS_PANEL),
                card: vocabulary.class_id(CLASS_CARD),
                heading: vocabulary.class_id(CLASS_HEADING),
                body: vocabulary.class_id(CLASS_BODY),
                caption: vocabulary.class_id(CLASS_CAPTION),
            },
        }
    }
}

/// The Signature geometry scale (corner radii, in logical px).
///
/// Radii are plain values rather than theme resources because the surface
/// style helpers take `f64` directly (see GAPS.md G6); keeping them in one
/// module still makes the scale a single source of truth.
pub mod radius {
    /// Tight radius — chips, ticks, inner marks.
    pub const SM: f64 = 6.0;
    /// Default control radius — buttons, inputs, list rows.
    pub const MD: f64 = 12.0;
    /// Raised container radius — cards, panels.
    pub const LG: f64 = 18.0;
    /// Fully rounded — toggles, pills, thumbs.
    pub const PILL: f64 = 999.0;
}
