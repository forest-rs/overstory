// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Overstory: a WPF / `WinUI` 3-inspired UI toolkit built on `understory` and
//! `imaging`.
//!
//! # Status
//!
//! Phase 1a — bare element tree only. Subsequent phases add layout, paint,
//! input, templates, and visual states.
//!
//! # Scope of this crate
//!
//! `overstory` owns:
//!
//! - The visual tree ([`Element`]) and its identity ([`ElementId`]).
//! - The frame loop and invalidation cascade (later phases).
//! - Templates, content presenters, visual states (later phases).
//! - Layout (driven by `taffy`), paint (driven by `imaging`), input (via
//!   `ui-events`), accessibility (via `accesskit`) — all later phases.
//!
//! `overstory` does **not** own dependency property storage, style resolution,
//! property bindings, focus, event routing, hit testing, timing, or
//! virtualization. Those live in the corresponding `understory_*` crates.

extern crate alloc;

mod behavior;
mod button;
pub mod channels;
mod element;
mod generic;
mod lookup;
mod properties;
pub mod pseudos;
mod template;
mod text;
mod text_behavior;
mod ui;

pub use behavior::{
    BehaviorRegistry, ElementBehavior, KnownDimensions, MeasureConstraints, MeasureContext,
    PaintContext,
};
pub use button::{ButtonBehavior, ELEMENT_KIND_BUTTON};
pub use element::{ELEMENT_KIND_GENERIC, Element, ElementId, ElementKind};
pub use generic::GenericBehavior;
pub use lookup::UiStoreLookup;
pub use properties::BuiltInProperties;
pub use template::Template;
pub use text::TextEngine;
pub use text_behavior::{ELEMENT_KIND_TEXT, TextBehavior};
pub use ui::{ClickEvent, KeyEvent, Ui};

// Re-export the responder's `Phase` so callers can read `ClickEvent::phase`
// / `KeyEvent::phase` without depending on `understory_responder` directly.
pub use understory_responder::types::Phase;

// Re-export the navigation intent enum for callers driving focus.
pub use understory_focus::Navigation;

// Re-export `taffy` so consumers don't have to add a direct dependency for
// the `Layout` property's style type.
pub use taffy;
