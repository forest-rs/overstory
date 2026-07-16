// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Built-in controls.
//!
//! Each module here defines one control: its builder + spec
//! ([`crate::AppendSpec`]), its per-element widget
//! ([`crate::Widget`]), and its default template recipe (if any). They
//! reach for shared infrastructure (the property store, cascade,
//! responder, timer queue) through `crate::Ui` and `crate::*`
//! re-exports.
//!
//! Apps write `use overstory::Button` etc. — the modules
//! themselves are private; everything app-facing is re-exported
//! at the crate root via `lib.rs`.
//!
//! Roadmap:
//! - Keep `RadioButton` / `RadioGroup`, `ListBox`, `Select`, and the eventual
//!   segmented control on a shared selection path. String/index APIs are fine
//!   for today's demos, but typed option keys, per-option metadata, validation,
//!   and item templates should arrive as common item-control infrastructure.
//! - Add `Select` / `ComboBox` as a composition of `Button`, `Popup`,
//!   `ListBox`, `ScrollView`, and `understory_selection`, then fold any shared
//!   keyed-option behavior back into list/radio controls instead of letting
//!   each control invent its own option model.
//! - Use `MenuButton` and `ContextMenu` as the proving consumers for shared
//!   menu realization before adding menu bars, flyouts, check/radio items, or
//!   other menu-family policy.
//! - Add `TabView` / `TabControl` to cover keyed selection, header templates,
//!   selected content panels, and overflow behavior.
//! - Add multiline text editing (`TextArea`) once `TextInput`, `ScrollView`,
//!   and prepared editable-text rendering have a stable shared shape.
//! - Grow `OutlineView` toward `TreeView` / hierarchical `ListView` once item
//!   templates can handle keyed, expandable data without one-off glue.
//! - Keep specialist controls behind reusable primitives: new controls should
//!   strengthen templates, routed events, focus, selection, scrolling,
//!   accessibility, or composition instead of bypassing them.
//!
//! Detailed control-family ticket: `ove-pgfg`.

pub(crate) mod accordion;
pub(crate) mod badge;
pub(crate) mod badged;
pub(crate) mod button;
pub(crate) mod checkbox;
pub(crate) mod choice;
pub(crate) mod disclosure;
pub(crate) mod divider;
pub(crate) mod expander;
pub(crate) mod indexed_stack;
pub(crate) mod list;
pub(crate) mod menu;
pub(crate) mod outline;
pub(crate) mod panel;
pub(crate) mod popup;
pub(crate) mod progress;
pub(crate) mod radio;
pub(crate) mod range;
pub(crate) mod scroll;
pub(crate) mod segmented;
pub(crate) mod select;
pub(crate) mod slider;
pub(crate) mod spinner;
pub(crate) mod splitter;
pub(crate) mod support;
pub(crate) mod tab_bar;
pub(crate) mod tab_view;
pub(crate) mod text;
pub(crate) mod text_input;
pub(crate) mod toggle;
pub(crate) mod tooltip;
pub(crate) mod vector_path;
pub(crate) mod viewport;
pub(crate) mod zstack;
