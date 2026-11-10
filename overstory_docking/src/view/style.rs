// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Docking presentation values and property application.
//!
//! This module owns the default docking appearance; it explicitly does not
//! own element realization, theme selection, or semantic state.

use overstory::{Brush, Color, ElementId, Ui};

/// Fill and border used for one kind of docking chrome.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DockPaint {
    /// Element background color.
    pub background: Color,
    /// Element border color.
    pub border: Color,
    /// Uniform border width in logical pixels.
    pub border_width: f64,
}

impl DockPaint {
    /// Creates a docking paint from background, border, and border width.
    #[must_use]
    pub const fn new(background: Color, border: Color, border_width: f64) -> Self {
        Self {
            background,
            border,
            border_width,
        }
    }
}

/// Visual styling for a docking surface.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct DockSurfaceStyle {
    /// Surface containing all docking layers.
    pub surface: DockPaint,
    /// Normal pane wrapper.
    pub pane: DockPaint,
    /// Selected pane wrapper.
    pub selected_pane: DockPaint,
    /// Tab-bar background.
    pub tab_bar: DockPaint,
    /// Inactive tab.
    pub tab: DockPaint,
    /// Active tab.
    pub active_tab: DockPaint,
    /// Tab label color.
    pub tab_foreground: Color,
    /// Split resize handle.
    pub split_handle: DockPaint,
    /// Accepted drop target.
    pub accepted_target: DockPaint,
    /// Rejected drop target.
    pub rejected_target: DockPaint,
    /// Active drop-target border color.
    pub active_target_border: Color,
    /// Valid preview ghost.
    pub preview: DockPaint,
    /// Invalid preview ghost.
    pub invalid_preview: DockPaint,
    /// Dragged-subject indicator.
    pub dragged: DockPaint,
    /// Pane content inset in logical pixels.
    pub pane_padding: f64,
    /// Tab rectangle inset in logical pixels.
    pub tab_inset: f64,
}

impl Default for DockSurfaceStyle {
    fn default() -> Self {
        Self {
            surface: paint(0x0f151dff, 0x344254ff, 1.0),
            pane: paint(0x18202bff, 0x36495fff, 1.0),
            selected_pane: paint(0x18202bff, 0xffc85aff, 2.0),
            tab_bar: paint(0x1b2531ff, 0x43556cff, 1.0),
            tab: paint(0x222d3bff, 0x3d4e64ff, 1.0),
            active_tab: paint(0x354960ff, 0xffc85aff, 2.0),
            tab_foreground: rgba(0xe9eff6ff),
            split_handle: paint(0x667f9cff, 0xb0c2d5ff, 1.0),
            accepted_target: paint(0x38a36930, 0x57d48dff, 1.0),
            rejected_target: paint(0xf0646424, 0xff8d8dff, 1.0),
            active_target_border: rgba(0xffd166ff),
            preview: paint(0x579bff32, 0x8fbaffff, 2.0),
            invalid_preview: paint(0xf0646424, 0xff8d8dff, 2.0),
            dragged: paint(0xf6c945df, 0xffee9bff, 1.0),
            pane_padding: 10.0,
            tab_inset: 3.0,
        }
    }
}

pub(super) fn apply_paint(ui: &mut Ui, id: ElementId, paint: DockPaint) {
    let props = ui.properties().surface;
    let background = Some(Brush::Solid(paint.background));
    let border = Some(Brush::Solid(paint.border));
    ui.set_local(id, props.background, background);
    ui.set_local(id, props.border_brushes.top, border.clone());
    ui.set_local(id, props.border_brushes.right, border.clone());
    ui.set_local(id, props.border_brushes.bottom, border.clone());
    ui.set_local(id, props.border_brushes.left, border);
    ui.set_local(id, props.border_widths.top, paint.border_width);
    ui.set_local(id, props.border_widths.right, paint.border_width);
    ui.set_local(id, props.border_widths.bottom, paint.border_width);
    ui.set_local(id, props.border_widths.left, paint.border_width);
}

const fn paint(background: u32, border: u32, border_width: f64) -> DockPaint {
    DockPaint::new(rgba(background), rgba(border), border_width)
}

const fn rgba(value: u32) -> Color {
    let [red, green, blue, alpha] = value.to_be_bytes();
    Color::from_rgba8(red, green, blue, alpha)
}
