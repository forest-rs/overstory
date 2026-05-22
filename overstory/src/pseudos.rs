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
