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
//!
//! Roadmap:
//! - Add `RadioButton` / `RadioGroup` soon to prove shared checked-state,
//!   selection, focus, and form semantics across mutually exclusive choices.
//! - Add `Select` / `ComboBox` as a composition of `Button`, `Popup`,
//!   `ListBox`, `ScrollView`, and `understory_selection`.
//! - Add menu-family controls (`Menu`, `MenuItem`, `ContextMenu`, `Flyout`) to
//!   exercise popup placement, focus scopes, dismiss routing, keyboard
//!   navigation, and command-style activation.
//! - Add `TabView` / `TabControl` to cover keyed selection, header templates,
//!   selected content panels, and overflow behavior.
//! - Add multiline text editing (`TextArea`) once `TextInput`, `ScrollView`,
//!   and prepared editable-text rendering have a stable shared shape.
//! - Add `TreeView` or hierarchical `ListView` after list virtualization and
//!   item templates can handle keyed, expandable data without one-off glue.
//! - Keep specialist controls behind reusable primitives: new controls should
//!   strengthen templates, routed events, focus, selection, scrolling,
//!   accessibility, or composition instead of bypassing them.
//!
//! Detailed control-family ticket: `ove-pgfg`.

pub(crate) mod button;
pub(crate) mod checkbox;
pub(crate) mod divider;
pub(crate) mod list;
pub(crate) mod panel;
pub(crate) mod popup;
pub(crate) mod progress;
pub(crate) mod scroll;
pub(crate) mod select;
pub(crate) mod slider;
pub(crate) mod spinner;
pub(crate) mod splitter;
pub(crate) mod support;
pub(crate) mod text;
pub(crate) mod text_input;
pub(crate) mod toggle;
pub(crate) mod tooltip;
pub(crate) mod viewport;
