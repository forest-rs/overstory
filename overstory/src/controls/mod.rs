// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Built-in controls.
//!
//! Each module here defines one control: its builder + spec
//! ([`crate::AppendSpec`]), its per-element widget
//! ([`crate::Widget`]), and its default template (if any). They
//! reach for shared infrastructure (the property store, cascade,
//! responder, timer queue) through `crate::Ui` and `crate::*`
//! re-exports.
//!
//! Apps write `use overstory::Button` etc. — the modules
//! themselves are private; everything app-facing is re-exported
//! at the crate root via `lib.rs`.

pub(crate) mod button;
pub(crate) mod checkbox;
pub(crate) mod divider;
pub(crate) mod list;
pub(crate) mod panel;
pub(crate) mod popup;
pub(crate) mod progress;
pub(crate) mod scroll;
pub(crate) mod slider;
pub(crate) mod spinner;
pub(crate) mod splitter;
pub(crate) mod text;
pub(crate) mod text_input;
pub(crate) mod toggle;
pub(crate) mod tooltip;
