// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The System 6 token taxonomy.
//!
//! A 1-bit world: everything is ink or paper. The extended roles are just those
//! two, so the cascade can name pure black/white directly (for the press
//! inversion and bordered frames) regardless of scheme.

use understory_style::{ClassId, ResourceKey, StyleTokenSet, StyleVocabulary};

/// Built-in palette roles System 6 redefines and reuses.
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
    /// Inverted "night" 1-bit: white ink on black paper.
    Dark,
    /// The classic black-ink-on-white-paper desktop.
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

/// Pure foreground ink (black on the classic scheme).
pub const RES_INK: &str = "system6.ink";
/// Pure surface paper (white on the classic scheme).
pub const RES_PAPER: &str = "system6.paper";

// Chrome classes are shared across all looks (see `overstory_look::class`) so an
// app can switch looks at runtime without rebuilding its tree.
pub use overstory_look::class::{
    BODY as CLASS_BODY, CAPTION as CLASS_CAPTION, CARD as CLASS_CARD, HEADING as CLASS_HEADING,
    PANEL as CLASS_PANEL, ROOT as CLASS_ROOT, SURFACE as CLASS_SURFACE,
};

/// Resolved extended resource keys.
#[derive(Clone, Copy, Debug)]
pub struct System6Resources {
    /// See [`RES_INK`].
    pub ink: ResourceKey,
    /// See [`RES_PAPER`].
    pub paper: ResourceKey,
}

/// Resolved chrome class ids.
#[derive(Clone, Copy, Debug)]
pub struct System6Classes {
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

/// Everything System 6 resolves from the `Ui` vocabulary.
#[derive(Clone, Copy, Debug)]
pub struct ResolvedTokens {
    /// Extended resource keys.
    pub resources: System6Resources,
    /// Chrome class ids.
    pub classes: System6Classes,
}

/// [`StyleTokenSet`] that interns System 6's classes and extended resources.
#[derive(Debug)]
pub struct System6Tokens;

impl StyleTokenSet for System6Tokens {
    type Resolved = ResolvedTokens;

    fn resolve(vocabulary: &mut StyleVocabulary) -> Self::Resolved {
        ResolvedTokens {
            resources: System6Resources {
                ink: vocabulary.resource_key(RES_INK),
                paper: vocabulary.resource_key(RES_PAPER),
            },
            classes: System6Classes {
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
