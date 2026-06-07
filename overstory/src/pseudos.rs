// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Built-in pseudoclass identifiers.
//!
//! Pseudoclasses are dynamic boolean states the input pipeline toggles on
//! elements (the cursor is over me; I'm pressed; I'm focused; …). Style
//! rules can require them via `SelectorStep::with_pseudo`.
//!
//! Application code should resolve its own pseudoclasses through
//! `Ui::style_vocabulary_mut().pseudo_class_id(...)` instead of picking raw
//! numeric ids.
//!
//! Roadmap:
//! - Keep pseudoclasses for dynamic state that affects selector matching:
//!   hover, pressed, focused, disabled, checked, expanded, selected,
//!   read-only, and placeholder-shown.
//! - Revisit placeholder and part styling once pseudo-elements exist; not every
//!   visual subpart should become a pseudoclass.
//! - Add states only with a clear owner responsible for keeping the state in
//!   sync with semantic properties.
//! - Keep Overstory's built-in ids stable while application and downstream
//!   control states resolve through the shared style vocabulary.

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

/// `:expanded` — this element is currently showing expandable content.
pub const PSEUDO_EXPANDED: PseudoClassId = PseudoClassId(10);
