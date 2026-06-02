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
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use overstory::{
    Brush, Color, ElementId, Layout, LayoutAlign, LayoutAlignment, LayoutDimension, LayoutSize,
    OutlineNodeKey, Panel, Text, Ui, select_outline_key, theme,
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
    value: PropertyGridValue,
    origin: Option<String>,
    children: Vec<Self>,
}

impl PropertyGridRow {
    /// Creates a leaf row with a display label and value.
    #[must_use]
    pub fn new(label: impl Into<String>, value: impl Into<PropertyGridValue>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            origin: None,
            children: Vec::new(),
        }
    }

    /// Returns the display label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the semantic display value.
    #[must_use]
    pub fn value(&self) -> &PropertyGridValue {
        &self.value
    }

    /// Returns a plain-text summary of the display value.
    #[must_use]
    pub fn value_summary(&self) -> String {
        self.value.summary()
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

    fn element_link(&self) -> Option<&PropertyGridElementLink> {
        self.value.as_element_link()
    }
}

/// Semantic value shown in a [`PropertyGridRow`].
///
/// The value is intentionally not a prebuilt element subtree. It describes the
/// kind of value being inspected, and the property-grid renderer lowers it to
/// the concrete elements that fit the current devtools UI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PropertyGridValue {
    /// Plain text with no richer display semantics.
    Text(String),
    /// Missing, unset, or not-applicable value.
    Empty,
    /// Boolean value.
    Bool(bool),
    /// Count of items in a collection-like value.
    Count(usize),
    /// Link to another inspectable element.
    ElementLink(PropertyGridElementLink),
    /// Token-like value such as a class, pseudo, slot, or type tag.
    Token(PropertyGridToken),
    /// Color value with a swatch.
    Color(PropertyGridColor),
}

impl PropertyGridValue {
    /// Creates a plain text value.
    #[must_use]
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    /// Creates an empty value.
    #[must_use]
    pub const fn empty() -> Self {
        Self::Empty
    }

    /// Creates a count value.
    #[must_use]
    pub const fn count(value: usize) -> Self {
        Self::Count(value)
    }

    /// Creates a link to another inspectable element.
    #[must_use]
    pub fn element_link(element: ElementId, label: impl Into<String>) -> Self {
        Self::ElementLink(PropertyGridElementLink::new(element, label))
    }

    /// Creates a token-like value.
    #[must_use]
    pub fn token(label: impl Into<String>) -> Self {
        Self::Token(PropertyGridToken::new(label))
    }

    /// Creates a color swatch value from 8-bit RGBA components.
    #[must_use]
    pub fn color_rgba8(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self::Color(PropertyGridColor::from_rgba8(red, green, blue, alpha))
    }

    /// Returns a plain-text summary for tests, logs, and fallback displays.
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::Text(value) => value.clone(),
            Self::Empty => String::from("none"),
            Self::Bool(value) => bool_summary(*value).into(),
            Self::Count(value) => value.to_string(),
            Self::ElementLink(link) => link.label().into(),
            Self::Token(token) => token.label().into(),
            Self::Color(color) => color.label().into(),
        }
    }

    fn as_element_link(&self) -> Option<&PropertyGridElementLink> {
        match self {
            Self::ElementLink(link) => Some(link),
            _ => None,
        }
    }

    fn is_muted(&self) -> bool {
        matches!(self, Self::Empty | Self::Bool(false) | Self::Count(0))
    }
}

impl From<String> for PropertyGridValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for PropertyGridValue {
    fn from(value: &str) -> Self {
        Self::Text(value.into())
    }
}

impl From<bool> for PropertyGridValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<usize> for PropertyGridValue {
    fn from(value: usize) -> Self {
        Self::Count(value)
    }
}

/// Link from one property-grid value to another inspectable element.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PropertyGridElementLink {
    element: ElementId,
    label: String,
}

impl PropertyGridElementLink {
    /// Creates a link to `element` with a human-readable `label`.
    #[must_use]
    pub fn new(element: ElementId, label: impl Into<String>) -> Self {
        Self {
            element,
            label: label.into(),
        }
    }

    /// Returns the target element.
    #[must_use]
    pub const fn element(&self) -> ElementId {
        self.element
    }

    /// Returns the display label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Token-like property-grid value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PropertyGridToken {
    label: String,
}

impl PropertyGridToken {
    /// Creates a token with `label`.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }

    /// Returns the display label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// RGBA color value for property-grid swatches.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PropertyGridColor {
    rgba: [u8; 4],
    label: String,
}

impl PropertyGridColor {
    /// Creates a color from 8-bit RGBA components.
    #[must_use]
    pub fn from_rgba8(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            rgba: [red, green, blue, alpha],
            label: rgba_label(red, green, blue, alpha),
        }
    }

    /// Returns the 8-bit RGBA components.
    #[must_use]
    pub const fn rgba8(&self) -> [u8; 4] {
        self.rgba
    }

    /// Returns the display label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}

fn rgba_label(red: u8, green: u8, blue: u8, alpha: u8) -> String {
    if alpha == 0xff {
        format!("#{red:02x}{green:02x}{blue:02x}")
    } else {
        format!("#{red:02x}{green:02x}{blue:02x}{alpha:02x}")
    }
}

fn bool_summary(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
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

const GRID_LABEL_WIDTH: f32 = 118.0;
const GRID_ROW_INDENT: f32 = 16.0;
const GRID_ROW_PADDING_X: f32 = 8.0;
const GRID_ROW_PADDING_Y: f32 = 5.0;
const GRID_SECTION_GAP: f32 = 6.0;
const GRID_FONT_SIZE: f64 = 12.0;
const GRID_SECTION_FONT_SIZE: f64 = 11.0;
const GRID_SECTION_COUNT_FONT_SIZE: f64 = 10.0;
const GRID_ORIGIN_FONT_SIZE: f64 = 10.0;

fn append_property_grid_section(
    ui: &mut Ui,
    parent: ElementId,
    section: &PropertyGridSection,
    actions: Option<&DetailActions>,
) -> ElementId {
    let section_id = ui.append(
        parent,
        Panel::column(0.0).layout(
            Layout::column(0.0)
                .margin_xy(0.0, GRID_SECTION_GAP)
                .fill_width(),
        ),
    );
    set_grid_surface(ui, section_id, control_bg(ui), control_border(ui), 6.0);
    let header = ui.append(
        section_id,
        Panel::row(6.0).layout(
            Layout::row(6.0)
                .padding_xy(GRID_ROW_PADDING_X, 6.0)
                .alignment(LayoutAlignment::new().align_items(LayoutAlign::Center))
                .fill_width(),
        ),
    );
    set_grid_surface(ui, header, control_hover_bg(ui), None, 5.0);
    append_grid_text(
        ui,
        header,
        section.title().to_ascii_uppercase(),
        muted_text_brush(ui),
        GRID_SECTION_FONT_SIZE,
        None,
        true,
    );
    append_grid_text(
        ui,
        header,
        section_row_count(section).to_string(),
        muted_text_brush(ui),
        GRID_SECTION_COUNT_FONT_SIZE,
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
        Panel::row(8.0).layout(
            Layout::row(8.0)
                .padding_xy(
                    GRID_ROW_PADDING_X + ((depth as f32) * GRID_ROW_INDENT),
                    GRID_ROW_PADDING_Y,
                )
                .alignment(LayoutAlignment::new().align_items(LayoutAlign::Start))
                .fill_width(),
        ),
    );
    if row.element_link().is_some() {
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
    append_value_cell(ui, row_id, row, actions);
    if let Some(link) = row.element_link() {
        wire_element_link(ui, row_id, link.element(), actions);
    }
    for child in row.children() {
        append_property_grid_row(ui, parent, child, depth + 1, actions);
    }
    row_id
}

fn section_row_count(section: &PropertyGridSection) -> usize {
    section.rows().iter().map(row_and_descendant_count).sum()
}

fn row_and_descendant_count(row: &PropertyGridRow) -> usize {
    1 + row
        .children()
        .iter()
        .map(row_and_descendant_count)
        .sum::<usize>()
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

fn append_value_cell(
    ui: &mut Ui,
    parent: ElementId,
    row: &PropertyGridRow,
    actions: Option<&DetailActions>,
) -> ElementId {
    let cell = ui.append(
        parent,
        Panel::row(6.0).layout(
            Layout::row(6.0)
                .wrap()
                .size(
                    LayoutSize::new()
                        .min_width(LayoutDimension::Logical(0.0))
                        .flex_grow(1.0)
                        .flex_shrink(1.0),
                )
                .alignment(LayoutAlignment::new().align_items(LayoutAlign::Center)),
        ),
    );
    append_grid_value(ui, cell, row.value(), actions);
    if let Some(origin) = row.origin() {
        append_origin_chip(ui, cell, origin);
    }
    cell
}

fn append_grid_value(
    ui: &mut Ui,
    parent: ElementId,
    value: &PropertyGridValue,
    actions: Option<&DetailActions>,
) {
    match value {
        PropertyGridValue::Text(text) => {
            append_grid_text(
                ui,
                parent,
                text,
                value_text_brush(ui, value),
                GRID_FONT_SIZE,
                None,
                true,
            );
        }
        PropertyGridValue::Empty => {
            append_grid_text(
                ui,
                parent,
                "none",
                muted_text_brush(ui),
                GRID_FONT_SIZE,
                None,
                true,
            );
        }
        PropertyGridValue::Bool(value) => {
            append_bool_chip(ui, parent, *value);
        }
        PropertyGridValue::Count(value) => {
            append_grid_text(
                ui,
                parent,
                value.to_string(),
                value_text_brush(ui, &PropertyGridValue::Count(*value)),
                GRID_FONT_SIZE,
                None,
                true,
            );
        }
        PropertyGridValue::ElementLink(link) => {
            let chip = append_action_chip(ui, parent, link.label());
            wire_element_link(ui, chip, link.element(), actions);
        }
        PropertyGridValue::Token(token) => {
            append_token_chip(ui, parent, token.label());
        }
        PropertyGridValue::Color(color) => {
            append_color_value(ui, parent, color);
        }
    }
}

fn append_action_chip(ui: &mut Ui, parent: ElementId, content: impl Into<String>) -> ElementId {
    let chip = ui.append(
        parent,
        Panel::row(0.0).layout(
            Layout::row(0.0).padding_xy(7.0, 3.0).size(
                LayoutSize::new()
                    .min_width(LayoutDimension::Logical(0.0))
                    .flex_grow(1.0)
                    .flex_shrink(1.0),
            ),
        ),
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

fn append_bool_chip(ui: &mut Ui, parent: ElementId, value: bool) -> ElementId {
    let foreground = if value {
        primary_text_brush(ui)
    } else {
        muted_text_brush(ui)
    };
    append_chip(ui, parent, bool_summary(value), foreground)
}

fn append_token_chip(ui: &mut Ui, parent: ElementId, content: impl Into<String>) -> ElementId {
    append_chip(ui, parent, content, primary_text_brush(ui))
}

fn append_chip(
    ui: &mut Ui,
    parent: ElementId,
    content: impl Into<String>,
    foreground: Brush,
) -> ElementId {
    let chip = ui.append(
        parent,
        Panel::row(0.0).layout(Layout::row(0.0).padding_xy(6.0, 2.0)),
    );
    set_grid_surface(ui, chip, control_hover_bg(ui), control_border(ui), 4.0);
    append_grid_text(
        ui,
        chip,
        content,
        foreground,
        GRID_ORIGIN_FONT_SIZE,
        None,
        false,
    );
    chip
}

fn append_color_value(ui: &mut Ui, parent: ElementId, color: &PropertyGridColor) -> ElementId {
    let swatch = ui.append(
        parent,
        Panel::new().layout(Layout::new().logical_size(14.0, 14.0).flex_shrink(0.0)),
    );
    let [red, green, blue, alpha] = color.rgba8();
    set_grid_surface(
        ui,
        swatch,
        Some(Color::from_rgba8(red, green, blue, alpha).into()),
        control_border(ui),
        3.0,
    );
    append_grid_text(
        ui,
        parent,
        color.label(),
        primary_text_brush(ui),
        GRID_FONT_SIZE,
        None,
        true,
    );
    swatch
}

fn append_origin_chip(ui: &mut Ui, parent: ElementId, content: impl Into<String>) -> ElementId {
    let chip = ui.append(
        parent,
        Panel::row(0.0).layout(Layout::row(0.0).padding_xy(6.0, 2.0)),
    );
    set_grid_surface(ui, chip, control_hover_bg(ui), control_border(ui), 4.0);
    append_grid_text(
        ui,
        chip,
        content,
        muted_text_brush(ui),
        GRID_ORIGIN_FONT_SIZE,
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

fn value_text_brush(ui: &Ui, value: &PropertyGridValue) -> Brush {
    if value.is_muted() {
        muted_text_brush(ui)
    } else {
        primary_text_brush(ui)
    }
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

fn control_border(ui: &Ui) -> Option<Brush> {
    ui.theme()
        .get::<Option<Brush>>(theme::CONTROL_BORDER)
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

fn wire_element_link(
    ui: &mut Ui,
    source: ElementId,
    target: ElementId,
    actions: Option<&DetailActions>,
) {
    let Some(actions) = actions.cloned() else {
        return;
    };
    if actions.outline_key_for_element(target).is_none() {
        return;
    }
    ui.on_click(source, move |ui, event| {
        let key = actions.outline_key_for_element(target);
        if let Some(key) = key {
            let _ = select_outline_key(ui, actions.outline, key);
            event.mark_handled();
        }
    });
}
