// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Read-only property-grid presentation helpers.
//!
//! This is the small Overstory-specific presentation layer that
//! `understory_inspector` does not own yet: sectioned property rows, nested
//! value rows, and devtools actions. If the generic inspector controller grows
//! column and property-value semantics later, this model is the part to
//! reconsider.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use overstory::{
    Brush, Color, ElementId, Layout, LayoutDimension, LayoutSize, OutlineNodeKey, Panel, Text, Ui,
    select_outline_key, theme,
};

#[cfg(not(feature = "spoor-session"))]
use crate::DevtoolsSessionHandle;
#[cfg(feature = "spoor-session")]
use crate::InspectionClient;

/// Read-only property-grid content grouped into sections.
///
/// This is intentionally only a presentation model. It does not resolve
/// properties, mutate live UI state, or define editor widgets.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PropertyGrid {
    sections: Vec<PropertyGridSection>,
}

impl PropertyGrid {
    /// Creates an empty property grid.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sections: Vec::new(),
        }
    }

    /// Returns the grid's sections.
    #[must_use]
    pub fn sections(&self) -> &[PropertyGridSection] {
        &self.sections
    }

    /// Returns whether this grid contains no sections.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    /// Appends `section` and returns the updated grid.
    #[must_use]
    pub fn section(mut self, section: PropertyGridSection) -> Self {
        self.push_section(section);
        self
    }

    /// Appends `section`.
    pub fn push_section(&mut self, section: PropertyGridSection) {
        self.sections.push(section);
    }
}

/// A named property-grid section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PropertyGridSection {
    title: String,
    rows: Vec<PropertyGridRow>,
}

impl PropertyGridSection {
    /// Creates an empty section with `title`.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            rows: Vec::new(),
        }
    }

    /// Returns the section title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the section's top-level rows.
    #[must_use]
    pub fn rows(&self) -> &[PropertyGridRow] {
        &self.rows
    }

    /// Appends `row` and returns the updated section.
    #[must_use]
    pub fn row(mut self, row: PropertyGridRow) -> Self {
        self.push_row(row);
        self
    }

    /// Appends `row`.
    pub fn push_row(&mut self, row: PropertyGridRow) {
        self.rows.push(row);
    }
}

/// A read-only row in a [`PropertyGridSection`].
///
/// Rows may have child rows so structured values can be shown as small trees
/// without changing the parent section model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PropertyGridRow {
    label: String,
    value: String,
    origin: Option<String>,
    children: Vec<Self>,
    action: Option<PropertyGridAction>,
}

impl PropertyGridRow {
    /// Creates a leaf row with a display label and value.
    #[must_use]
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            origin: None,
            children: Vec::new(),
            action: None,
        }
    }

    /// Returns the display label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the display value.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the row origin, if the adapter supplied one.
    #[must_use]
    pub fn origin(&self) -> Option<&str> {
        self.origin.as_deref()
    }

    /// Returns child rows.
    #[must_use]
    pub fn children(&self) -> &[Self] {
        &self.children
    }

    /// Sets an origin label and returns the updated row.
    ///
    /// Origins are intended for future source display such as `theme`,
    /// `local`, `inherited`, `template`, or `animation`.
    #[must_use]
    pub fn with_origin(mut self, origin: impl Into<String>) -> Self {
        self.origin = Some(origin.into());
        self
    }

    /// Appends `child` and returns the updated row.
    #[must_use]
    pub fn child(mut self, child: Self) -> Self {
        self.push_child(child);
        self
    }

    /// Appends `child`.
    pub fn push_child(&mut self, child: Self) {
        self.children.push(child);
    }

    pub(crate) fn select_element(mut self, element: ElementId) -> Self {
        self.action = Some(PropertyGridAction::SelectElement(element));
        self
    }

    fn action(&self) -> Option<PropertyGridAction> {
        self.action
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PropertyGridAction {
    SelectElement(ElementId),
}

#[derive(Clone, Debug)]
pub(crate) struct DetailActions {
    outline: ElementId,
    resolver: DetailActionResolver,
}

impl DetailActions {
    #[cfg(not(feature = "spoor-session"))]
    pub(crate) fn from_session(session: DevtoolsSessionHandle, outline: ElementId) -> Self {
        Self {
            outline,
            resolver: DetailActionResolver::Session(session),
        }
    }

    #[cfg(feature = "spoor-session")]
    pub(crate) fn from_client(client: InspectionClient, outline: ElementId) -> Self {
        Self {
            outline,
            resolver: DetailActionResolver::Client(client),
        }
    }

    fn outline_key_for_element(&self, element: ElementId) -> Option<OutlineNodeKey> {
        match &self.resolver {
            #[cfg(not(feature = "spoor-session"))]
            DetailActionResolver::Session(session) => {
                session.with(|session| session.snapshot().outline_key_for_element(element))
            }
            #[cfg(feature = "spoor-session")]
            DetailActionResolver::Client(client) => client
                .element_tree()
                .ok()
                .and_then(|tree| tree.outline_key_for_element(element)),
        }
    }
}

#[derive(Clone, Debug)]
enum DetailActionResolver {
    #[cfg(not(feature = "spoor-session"))]
    Session(DevtoolsSessionHandle),
    #[cfg(feature = "spoor-session")]
    Client(InspectionClient),
}

pub(crate) fn append_property_grid_sections(
    ui: &mut Ui,
    parent: ElementId,
    grid: &PropertyGrid,
    actions: Option<&DetailActions>,
) {
    for section in grid.sections() {
        append_property_grid_section(ui, parent, section, actions);
    }
}

const GRID_LABEL_WIDTH: f32 = 156.0;
const GRID_ROW_INDENT: f32 = 16.0;
const GRID_ROW_PADDING_X: f32 = 8.0;
const GRID_ROW_PADDING_Y: f32 = 4.0;
const GRID_SECTION_GAP: f32 = 4.0;
const GRID_FONT_SIZE: f64 = 12.0;
const GRID_SECTION_FONT_SIZE: f64 = 11.0;

fn append_property_grid_section(
    ui: &mut Ui,
    parent: ElementId,
    section: &PropertyGridSection,
    actions: Option<&DetailActions>,
) -> ElementId {
    let section_id = ui.append(
        parent,
        Panel::column(0.0).layout(Layout::column(0.0).margin_xy(0.0, GRID_SECTION_GAP)),
    );
    let header = ui.append(
        section_id,
        Panel::row(0.0).layout(Layout::row(0.0).padding_xy(GRID_ROW_PADDING_X, 5.0)),
    );
    set_grid_surface(ui, header, control_bg(ui), None, 5.0);
    append_grid_text(
        ui,
        header,
        section.title(),
        muted_text_brush(ui),
        GRID_SECTION_FONT_SIZE,
        None,
        false,
    );
    for row in section.rows() {
        append_property_grid_row(ui, section_id, row, 0, actions);
    }
    section_id
}

fn append_property_grid_row(
    ui: &mut Ui,
    parent: ElementId,
    row: &PropertyGridRow,
    depth: usize,
    actions: Option<&DetailActions>,
) -> ElementId {
    let row_id = ui.append(
        parent,
        Panel::row(8.0).layout(Layout::row(8.0).align_center().padding_xy(
            GRID_ROW_PADDING_X + ((depth as f32) * GRID_ROW_INDENT),
            GRID_ROW_PADDING_Y,
        )),
    );
    if row.action().is_some() {
        set_grid_surface(ui, row_id, control_hover_bg(ui), None, 4.0);
    }
    append_grid_text(
        ui,
        row_id,
        row.label(),
        muted_text_brush(ui),
        GRID_FONT_SIZE,
        Some(label_layout_width(depth)),
        false,
    );
    if row.action().is_some() {
        append_action_chip(ui, row_id, row_value_text(row));
    } else {
        append_grid_text(
            ui,
            row_id,
            row_value_text(row),
            primary_text_brush(ui),
            GRID_FONT_SIZE,
            None,
            true,
        );
    }
    wire_row_action(ui, row_id, row.action(), actions);
    for child in row.children() {
        append_property_grid_row(ui, parent, child, depth + 1, actions);
    }
    row_id
}

fn row_value_text(row: &PropertyGridRow) -> String {
    match row.origin() {
        Some(origin) => format!("{} [{origin}]", row.value()),
        None => row.value().into(),
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "property-grid nesting is UI depth and maps directly to logical pixels"
)]
fn label_layout_width(depth: usize) -> f32 {
    (GRID_LABEL_WIDTH - ((depth as f32) * GRID_ROW_INDENT)).max(72.0)
}

fn append_grid_text(
    ui: &mut Ui,
    parent: ElementId,
    content: impl Into<String>,
    brush: Brush,
    font_size: f64,
    width: Option<f32>,
    grow: bool,
) -> ElementId {
    let text = ui.append(
        parent,
        Text::new(content.into())
            .font_size(font_size)
            .foreground(brush)
            .wrap(),
    );
    let mut size = LayoutSize::new().min_width(LayoutDimension::Logical(0.0));
    if let Some(width) = width {
        size = size.width(LayoutDimension::Logical(width)).flex_shrink(0.0);
    }
    if grow {
        size = size.flex_grow(1.0).flex_shrink(1.0);
    }
    ui.set_layout(text, Layout::new().size(size));
    text
}

fn append_action_chip(ui: &mut Ui, parent: ElementId, content: impl Into<String>) -> ElementId {
    let chip = ui.append(
        parent,
        Panel::row(0.0).layout(Layout::row(0.0).padding_xy(7.0, 3.0)),
    );
    set_grid_surface(ui, chip, accent_bg(ui), None, 5.0);
    append_grid_text(
        ui,
        chip,
        content,
        on_accent_text_brush(ui),
        GRID_FONT_SIZE,
        None,
        false,
    );
    chip
}

fn set_grid_surface(
    ui: &mut Ui,
    element: ElementId,
    background: Option<Brush>,
    border: Option<Brush>,
    radius: f64,
) {
    ui.set_local(element, ui.properties().background, background);
    if border.is_some() {
        ui.set_local(element, ui.properties().border_brush, border);
        ui.set_local(element, ui.properties().border_width, 1.0_f64);
    }
    ui.set_local(element, ui.properties().corner_radius, radius);
}

fn muted_text_brush(ui: &Ui) -> Brush {
    ui.theme()
        .get::<Brush>(theme::ON_SURFACE_MUTED_FG)
        .cloned()
        .unwrap_or_else(fallback_text_brush)
}

fn primary_text_brush(ui: &Ui) -> Brush {
    ui.theme()
        .get::<Brush>(theme::ON_SURFACE_FG)
        .cloned()
        .unwrap_or_else(fallback_text_brush)
}

fn on_accent_text_brush(ui: &Ui) -> Brush {
    ui.theme()
        .get::<Brush>(theme::ON_ACCENT_FG)
        .cloned()
        .unwrap_or_else(|| Color::from_rgba8(0xff, 0xff, 0xff, 0xff).into())
}

fn accent_bg(ui: &Ui) -> Option<Brush> {
    ui.theme()
        .get::<Option<Brush>>(theme::ACCENT_BG)
        .cloned()
        .flatten()
}

fn control_bg(ui: &Ui) -> Option<Brush> {
    ui.theme()
        .get::<Option<Brush>>(theme::CONTROL_BG)
        .cloned()
        .flatten()
}

fn control_hover_bg(ui: &Ui) -> Option<Brush> {
    ui.theme()
        .get::<Option<Brush>>(theme::CONTROL_HOVER_BG)
        .cloned()
        .flatten()
}

fn fallback_text_brush() -> Brush {
    Color::from_rgba8(0xf4, 0xf4, 0xf7, 0xff).into()
}

fn wire_row_action(
    ui: &mut Ui,
    row: ElementId,
    action: Option<PropertyGridAction>,
    actions: Option<&DetailActions>,
) {
    let Some(PropertyGridAction::SelectElement(target)) = action else {
        return;
    };
    let Some(actions) = actions.cloned() else {
        return;
    };
    if actions.outline_key_for_element(target).is_none() {
        return;
    }
    ui.on_click(row, move |ui, event| {
        let key = actions.outline_key_for_element(target);
        if let Some(key) = key {
            let _ = select_outline_key(ui, actions.outline, key);
            event.mark_handled();
        }
    });
}
