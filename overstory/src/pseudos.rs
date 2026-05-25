// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Built-in pseudoclass identifiers.
//!
//! Pseudoclasses are dynamic boolean states the input pipeline toggles on
//! elements (the cursor is over me; I'm pressed; I'm focused; …). Style
//! rules can require them via `SelectorStep::with_pseudo`.
//!
//! Built-in ids are reserved in the range `0..=31`; application code should
//! pick values above that for its own pseudoclasses.
//!
//! Roadmap:
//! - Keep pseudoclasses for dynamic state that affects selector matching:
//!   hover, pressed, focused, disabled, checked, selected, read-only, and
//!   placeholder-shown.
//! - Revisit placeholder and part styling once pseudo-elements exist; not every
//!   visual subpart should become a pseudoclass.
//! - Add states only with a clear owner responsible for keeping the state in
//!   sync with semantic properties.
//! - Reserve enough id space for Overstory while leaving applications and
//!   downstream control libraries room for their own states.

use understory_style::PseudoClassId;

/// `:hover` — the primary pointer is currently over this element.
pub const PSEUDO_HOVER: PseudoClassId = PseudoClassId(1);

/// `:pressed` — a primary pointer button is pressed on this element.
pub const PSEUDO_PRESSED: PseudoClassId = PseudoClassId(2);

/// `:focused` — this element currently has keyboard focus.
pub const PSEUDO_FOCUSED: PseudoClassId = PseudoClassId(3);

/// `:disabled` — this element is non-interactive.
pub const PSEUDO_DISABLED: PseudoClassId = PseudoClassId(4);

/// `:checked` — this element is in a "checked" toggle state.
pub const PSEUDO_CHECKED: PseudoClassId = PseudoClassId(5);

/// `:read-only` — this element can be focused/selected but not edited.
pub const PSEUDO_READ_ONLY: PseudoClassId = PseudoClassId(6);

/// `:focus-visible` — this element has keyboard focus from a keyboard-like
/// interaction and should show an explicit focus indicator.
pub const PSEUDO_FOCUS_VISIBLE: PseudoClassId = PseudoClassId(7);

/// `:selected` — this item is selected by its owning selection control.
pub const PSEUDO_SELECTED: PseudoClassId = PseudoClassId(8);

/// `:placeholder-shown` — this text input is currently showing placeholder text.
pub const PSEUDO_PLACEHOLDER_SHOWN: PseudoClassId = PseudoClassId(9);
