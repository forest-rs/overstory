// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `TextInput`: single-line editable text field.
//!
//! Roadmap:
//! - Keep Parley's [`PlainEditor`] as the editing, cursor, selection, and IME
//!   layout core for plain text.
//! - Make the control feel complete through Overstory-owned behavior: platform
//!   keybindings, single-line policy, caret reveal / horizontal scrolling,
//!   selection gestures, focus states, and read-only / disabled semantics.
//! - Integrate host-backed capabilities in `overstory_host` / platform hosts:
//!   clipboard, IME event bridging, accessibility, cursor blink, and platform
//!   modifier conventions.
//! - Revisit rendering so editable text can use the editor's cached layout via
//!   the presentation model instead of reshaping through a `TextWidget` part.

mod api;
mod builder;
mod editor;
mod events;
mod ids;
mod template;
mod widget;

#[cfg(test)]
mod tests;

pub use api::{set_text_input_read_only, set_text_input_text, text_input_text};
pub use builder::TextInput;
pub use events::{
    TextInputEditCommitReason, TextInputEditCommittedEvent, TextInputEditRejectReason,
    TextInputEditRejectedEvent, TextInputSubmitAction, TextInputSubmitEvent,
    TextInputTextChangeSource, TextInputTextChangedEvent,
};
pub use ids::{
    PART_TEXT_INPUT_CARET, PART_TEXT_INPUT_SELECTION, PART_TEXT_INPUT_TEXT, SLOT_TEXT_INPUT_CARET,
    SLOT_TEXT_INPUT_SELECTION, SLOT_TEXT_INPUT_TEXT, TYPE_TEXT_INPUT,
};

pub(crate) use template::default_text_input_template;
