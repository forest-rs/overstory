// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The Chicago token taxonomy.
//!
//! Like the other looks, Chicago reuses Overstory's built-in palette roles
//! (`overstory::theme::*`) so framework controls re-skin, and adds a small set
//! of vocabulary-allocated extended roles. Chicago's extended roles are the two
//! bevel edges — the highlight and the shadow — that the raised/sunken 3D
//! borders are built from.

use understory_style::{ClassId, ResourceKey, StyleTokenSet, StyleVocabulary};

/// Built-in palette roles Chicago redefines and reuses.
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
    /// A darker chrome variant.
    Dark,
    /// The classic light-gray chrome.
    Light,
}

impl Scheme {
    /// Toggles between the two schemes.
    #[must_use]
    pub fn toggled(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }
}

/// Bevel highlight edge (light: top + left of a raised control).
pub const RES_BEVEL_LIGHT: &str = "chicago.bevel.light";
/// Bevel shadow edge (dark: bottom + right of a raised control).
pub const RES_BEVEL_DARK: &str = "chicago.bevel.dark";

/// App root backdrop.
pub const CLASS_ROOT: &str = ".chi-root";
/// Flat surface region.
pub const CLASS_SURFACE: &str = ".chi-surface";
/// Raised panel.
pub const CLASS_PANEL: &str = ".chi-panel";
/// Card (a raised group box).
pub const CLASS_CARD: &str = ".chi-card";
/// Heading text.
pub const CLASS_HEADING: &str = ".chi-heading";
/// Body text.
pub const CLASS_BODY: &str = ".chi-body";
/// Caption text.
pub const CLASS_CAPTION: &str = ".chi-caption";

/// Resolved extended resource keys.
#[derive(Clone, Copy, Debug)]
pub struct ChicagoResources {
    /// See [`RES_BEVEL_LIGHT`].
    pub bevel_light: ResourceKey,
    /// See [`RES_BEVEL_DARK`].
    pub bevel_dark: ResourceKey,
}

/// Resolved chrome class ids.
#[derive(Clone, Copy, Debug)]
pub struct ChicagoClasses {
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

/// Everything Chicago resolves from the `Ui` vocabulary.
#[derive(Clone, Copy, Debug)]
pub struct ResolvedTokens {
    /// Extended resource keys.
    pub resources: ChicagoResources,
    /// Chrome class ids.
    pub classes: ChicagoClasses,
}

/// [`StyleTokenSet`] that interns Chicago's classes and extended resources.
#[derive(Debug)]
pub struct ChicagoTokens;

impl StyleTokenSet for ChicagoTokens {
    type Resolved = ResolvedTokens;

    fn resolve(vocabulary: &mut StyleVocabulary) -> Self::Resolved {
        ResolvedTokens {
            resources: ChicagoResources {
                bevel_light: vocabulary.resource_key(RES_BEVEL_LIGHT),
                bevel_dark: vocabulary.resource_key(RES_BEVEL_DARK),
            },
            classes: ChicagoClasses {
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
