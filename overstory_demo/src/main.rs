// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Overstory control gallery.

extern crate alloc;

use alloc::boxed::Box;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use overstory::motion::{
    ColorInterpolation, TimingFunction, TransitionRule, TransitionRules, TransitionSpec,
    TransitionTarget, millis,
};
use overstory::pseudos::{PSEUDO_FOCUS_VISIBLE, PSEUDO_PRESSED, PSEUDO_SELECTED};
use overstory::{
    Affine, BuiltInProperties, Button, CheckBox, CursorIcon, Divider, ElementId, FontFeature,
    Layout, LayoutAlign, LayoutAlignment, LayoutDimension, LayoutSize, ListBox, Panel, Popup,
    ProgressBar, Rect, ScrollBar, Select, Slider, Spinner, Splitter, TYPE_GENERIC, TYPE_TEXT, Text,
    TextInput, TimerDuration, TimerId, ToggleSwitch, Tooltip, Ui, Viewport, layouts,
    set_progress_value, theme,
};
use overstory_host_winit::WindowOptions;
use overstory_svg::{HitParams, SvgElementTree, SvgImportOptions};
use peniko::kurbo::{Point, Vec2};
use peniko::{Brush, Color};
use understory_style::{
    ClassId, PseudoClassId, ResourceKey, Selector, SelectorStep, StyleBuilder, StyleCascade,
    StyleOrigin,
};
use understory_view2d::{ClampMode, FitMode, Viewport2D};

const INITIAL_SIZE: (u32, u32) = (1480, 820);
const ROOT_GAP: f32 = 24.0;
const ROOT_PADDING: f32 = 28.0;
const NAV_WIDTH: f32 = 276.0;
const NAV_PADDING: f32 = 20.0;
const NAV_CONTENT_WIDTH: f32 = NAV_WIDTH - (NAV_PADDING * 2.0);
const PAGE_HEADER_HEIGHT: f32 = 92.0;
const OVERVIEW_CARD_WIDTH: f32 = 256.0;
const OVERVIEW_CONTROL_WIDTH: f32 = 206.0;
const OVERVIEW_LIST_WIDTH: f32 = 198.0;
const PAGE_CARD_WIDTH: f32 = 390.0;
const PAGE_CONTROL_WIDTH: f32 = 328.0;
const PAGE_LIST_WIDTH: f32 = 316.0;
const SPLIT_PANE_WIDTH: f32 = 148.0;
const SPLIT_PANE_HEIGHT: f32 = 142.0;
const SVG_PAGE_WIDTH: f32 = 824.0;
const SVG_INFO_HEIGHT: f32 = 132.0;
const SVG_TIGER_CARD_WIDTH: f32 = 548.0;
const SVG_TIGER_CARD_HEIGHT: f32 = 500.0;
const SVG_INSPECTOR_CARD_WIDTH: f32 = 258.0;
const SVG_VIEWPORT_WIDTH: f32 = 500.0;
const SVG_VIEWPORT_HEIGHT: f32 = 328.0;
const SVG_ZOOM_SENSITIVITY: f64 = 0.0015;
const SVG_GRID_LIMIT: usize = 140;
const SVG_SELECTION_WOBBLE_UP_MS: u64 = 135;
const SVG_SELECTION_WOBBLE_DOWN_MS: u64 = 150;
const TIGER_SVG: &[u8] = include_bytes!("../assets/Ghostscript_Tiger.svg");

const DEMO_ROOT_BG: ResourceKey = ResourceKey::new(1_000);
const DEMO_NAV_BG: ResourceKey = ResourceKey::new(1_001);
const DEMO_NAV_BORDER: ResourceKey = ResourceKey::new(1_002);
const DEMO_NAV_SELECTED_BG: ResourceKey = ResourceKey::new(1_003);
const DEMO_STAGE_BG: ResourceKey = ResourceKey::new(1_004);
const DEMO_STAGE_BORDER: ResourceKey = ResourceKey::new(1_005);
const DEMO_HEADER_BG: ResourceKey = ResourceKey::new(1_006);
const DEMO_HEADER_BORDER: ResourceKey = ResourceKey::new(1_007);
const DEMO_CARD_BG: ResourceKey = ResourceKey::new(1_008);
const DEMO_CARD_BORDER: ResourceKey = ResourceKey::new(1_009);
const DEMO_SPLIT_BORDER: ResourceKey = ResourceKey::new(1_010);
const DEMO_SPLIT_SCENE_BG: ResourceKey = ResourceKey::new(1_011);
const DEMO_SPLIT_INSPECTOR_BG: ResourceKey = ResourceKey::new(1_012);
const DEMO_MOTION_STAGE_BG: ResourceKey = ResourceKey::new(1_013);
const DEMO_ACCENT_BG: ResourceKey = ResourceKey::new(1_014);
const DEMO_TEXT_FG: ResourceKey = ResourceKey::new(1_015);
const DEMO_BODY_FG: ResourceKey = ResourceKey::new(1_016);
const DEMO_MUTED_FG: ResourceKey = ResourceKey::new(1_017);
const DEMO_TITLE_FG: ResourceKey = ResourceKey::new(1_018);
const DEMO_FOCUS_BORDER: ResourceKey = ResourceKey::new(1_019);

const CLASS_DEMO_ROOT: ClassId = ClassId(1);
const CLASS_DEMO_NAV: ClassId = ClassId(2);
const CLASS_DEMO_NAV_MARKER: ClassId = ClassId(3);
const CLASS_DEMO_STAGE: ClassId = ClassId(4);
const CLASS_DEMO_HEADER: ClassId = ClassId(5);
const CLASS_DEMO_HEADER_ACCENT: ClassId = ClassId(6);
const CLASS_DEMO_CARD: ClassId = ClassId(7);
const CLASS_DEMO_NAV_ROW: ClassId = ClassId(8);
const CLASS_DEMO_NAV_LABEL: ClassId = ClassId(9);
const CLASS_DEMO_HEADING: ClassId = ClassId(10);
const CLASS_DEMO_PAGE_HEADING: ClassId = ClassId(11);
const CLASS_DEMO_CAPTION: ClassId = ClassId(12);
const CLASS_DEMO_CARD_TITLE: ClassId = ClassId(13);
const CLASS_DEMO_CONTROL_LABEL: ClassId = ClassId(14);
const CLASS_DEMO_NUMBER: ClassId = ClassId(15);
const CLASS_DEMO_LARGE_NUMBER: ClassId = ClassId(16);
const CLASS_DEMO_BODY: ClassId = ClassId(17);
const CLASS_DEMO_SPLIT_SCENE: ClassId = ClassId(18);
const CLASS_DEMO_SPLIT_INSPECTOR: ClassId = ClassId(19);
const CLASS_DEMO_MOTION_STAGE: ClassId = ClassId(20);
const CLASS_SVG_GRID_LINE: ClassId = ClassId(21);
const CLASS_SVG_INFO_CARD: ClassId = ClassId(22);
const CLASS_SVG_TIGER_CARD: ClassId = ClassId(23);
const CLASS_SVG_INSPECTOR_CARD: ClassId = ClassId(24);

const RENDERER_QUEUE_LEN: usize = 1_000;
const INITIAL_RENDERER_SELECTION: usize = 2;
const RENDERER_STAGES: [&str; 12] = [
    "Vello scene graph",
    "CPU fallback raster",
    "Text shaping pass",
    "Accessibility tree",
    "Hit testing index",
    "Presentation cache",
    "Frame diagnostics",
    "Layer compaction",
    "Glyph atlas upload",
    "Clip stack rebuild",
    "Surface invalidation",
    "GPU submission",
];

const CLASS_OKLAB_SWATCH: ClassId = ClassId(100);
const CLASS_OPACITY_SWATCH: ClassId = ClassId(101);
const CLASS_BORDER_SWATCH: ClassId = ClassId(102);
const CLASS_RETARGET_SWATCH: ClassId = ClassId(103);
const CLASS_MOVER_SWATCH: ClassId = ClassId(104);
const CLASS_SRGB_SWATCH: ClassId = ClassId(105);

const EASE_OUT: TimingFunction = TimingFunction::cubic_bezier(0.16, 1.0, 0.3, 1.0);
const EASE_IN_OUT: TimingFunction = TimingFunction::cubic_bezier(0.65, 0.0, 0.35, 1.0);
const MID_SLOW: TimingFunction = TimingFunction::cubic_bezier(0.2, 0.48, 0.8, 0.52);

const SCENE_GRAPH_SVG: &[u8] = br##"
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="54" viewBox="0 0 100 54">
  <path id="surface" d="M8 13h31v18H8z" fill="#4f6fd6"/>
  <path id="text" d="M58 7h28v15H58z" fill="#64b37d"/>
  <path id="edge" d="M39 22 C49 22 48 14 58 14" fill="none" stroke="#d8dbea" stroke-width="3" stroke-linecap="round"/>
  <path id="output" d="M57 32h34v15H57z" fill="#d18b47"/>
  <path id="edge2" d="M39 28 C50 34 47 40 57 40" fill="none" stroke="#d8dbea" stroke-width="3" stroke-linecap="round"/>
</svg>
"##;

fn main() -> overstory_host_winit::HostResult<()> {
    overstory_host_winit::run(
        WindowOptions::new("overstory_demo").initial_size(INITIAL_SIZE.0, INITIAL_SIZE.1),
        build_ui,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DemoPage {
    Overview,
    Inputs,
    Systems,
    Selection,
    Layout,
    Svg,
    Typography,
    Motion,
}

impl DemoPage {
    const ALL: [Self; 8] = [
        Self::Overview,
        Self::Inputs,
        Self::Systems,
        Self::Selection,
        Self::Layout,
        Self::Svg,
        Self::Typography,
        Self::Motion,
    ];

    fn from_index(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }

    fn index(self) -> usize {
        match self {
            Self::Overview => 0,
            Self::Inputs => 1,
            Self::Systems => 2,
            Self::Selection => 3,
            Self::Layout => 4,
            Self::Svg => 5,
            Self::Typography => 6,
            Self::Motion => 7,
        }
    }

    fn nav_label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Inputs => "Inputs",
            Self::Systems => "Systems",
            Self::Selection => "Selection",
            Self::Layout => "Layout",
            Self::Svg => "SVG",
            Self::Typography => "Typography",
            Self::Motion => "Motion",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overstory Control Gallery",
            Self::Inputs => "Inputs and Command Surfaces",
            Self::Systems => "Controls and Values",
            Self::Selection => "Selection and Virtualization",
            Self::Layout => "Layout and Resizing",
            Self::Svg => "SVG Import",
            Self::Typography => "Typography and Text Features",
            Self::Motion => "Presentation Motion",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Self::Overview => {
                "A focused gallery for retained controls, routed input, style, timers, and composition."
            }
            Self::Inputs => {
                "Buttons, text fields, popups, checkboxes, disabled state, focus, hover, and tooltips."
            }
            Self::Systems => {
                "Timer-driven values, slider ticks, progress, toggles, and a stable tabular counter."
            }
            Self::Selection => {
                "A virtualized list backed by understory_selection with shared scroll state."
            }
            Self::Layout => {
                "Resizable panes, fixed-size content, axis-aware controls, and slider ruler marks."
            }
            Self::Svg => {
                "Static SVG paths lowered through overstory_svg into retained VectorPath elements."
            }
            Self::Typography => {
                "Font features, wrapping, editable text, and dynamic numbers shaped through Parley."
            }
            Self::Motion => {
                "Style-authored transitions for color, opacity, border, retargeting, and composition offset."
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DemoThemeMode {
    Dark,
    Light,
}

impl DemoThemeMode {
    fn toggled(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }

    fn theme(self) -> understory_style::Theme {
        match self {
            Self::Dark => theme::dark_theme_builder()
                .set(DEMO_ROOT_BG, opt_solid(0x1e, 0x1e, 0x24, 0xff))
                .set(DEMO_NAV_BG, opt_solid(0x22, 0x23, 0x2b, 0xff))
                .set(DEMO_NAV_BORDER, opt_solid(0x3a, 0x3d, 0x4b, 0xff))
                .set(DEMO_NAV_SELECTED_BG, opt_solid(0x4d, 0x6f, 0xdc, 0xff))
                .set(DEMO_STAGE_BG, opt_solid(0x1b, 0x1c, 0x23, 0xff))
                .set(DEMO_STAGE_BORDER, opt_solid(0x2f, 0x32, 0x3d, 0xff))
                .set(DEMO_HEADER_BG, opt_solid(0x24, 0x25, 0x2e, 0xff))
                .set(DEMO_HEADER_BORDER, opt_solid(0x3b, 0x3e, 0x4c, 0xff))
                .set(DEMO_CARD_BG, opt_solid(0x27, 0x28, 0x31, 0xff))
                .set(DEMO_CARD_BORDER, opt_solid(0x3b, 0x3d, 0x49, 0xff))
                .set(DEMO_SPLIT_BORDER, opt_solid(0x48, 0x4c, 0x58, 0xff))
                .set(DEMO_SPLIT_SCENE_BG, opt_solid(0x31, 0x35, 0x43, 0xff))
                .set(DEMO_SPLIT_INSPECTOR_BG, opt_solid(0x2e, 0x3a, 0x35, 0xff))
                .set(DEMO_MOTION_STAGE_BG, opt_solid(0x27, 0x28, 0x31, 0xff))
                .set(DEMO_ACCENT_BG, opt_solid(0x59, 0x75, 0xe6, 0xff))
                .set(DEMO_TEXT_FG, solid(0xf4, 0xf4, 0xf7, 0xff))
                .set(DEMO_BODY_FG, solid(0xd8, 0xd9, 0xe2, 0xff))
                .set(DEMO_MUTED_FG, solid(0xc8, 0xc8, 0xd2, 0xff))
                .set(DEMO_TITLE_FG, solid(0x96, 0x9a, 0xa8, 0xff))
                .set(DEMO_FOCUS_BORDER, opt_solid(0xee, 0xef, 0xf8, 0xff))
                .build(),
            Self::Light => theme::light_theme_builder()
                .set(DEMO_ROOT_BG, opt_solid(0xf0, 0xf3, 0xf8, 0xff))
                .set(DEMO_NAV_BG, opt_solid(0xff, 0xff, 0xff, 0xff))
                .set(DEMO_NAV_BORDER, opt_solid(0xd2, 0xd8, 0xe5, 0xff))
                .set(DEMO_NAV_SELECTED_BG, opt_solid(0x2f, 0x62, 0xd6, 0xff))
                .set(DEMO_STAGE_BG, opt_solid(0xf7, 0xf9, 0xfc, 0xff))
                .set(DEMO_STAGE_BORDER, opt_solid(0xd8, 0xde, 0xea, 0xff))
                .set(DEMO_HEADER_BG, opt_solid(0xff, 0xff, 0xff, 0xff))
                .set(DEMO_HEADER_BORDER, opt_solid(0xda, 0xe0, 0xeb, 0xff))
                .set(DEMO_CARD_BG, opt_solid(0xff, 0xff, 0xff, 0xff))
                .set(DEMO_CARD_BORDER, opt_solid(0xd8, 0xde, 0xea, 0xff))
                .set(DEMO_SPLIT_BORDER, opt_solid(0xbf, 0xc7, 0xd5, 0xff))
                .set(DEMO_SPLIT_SCENE_BG, opt_solid(0xec, 0xf1, 0xfb, 0xff))
                .set(DEMO_SPLIT_INSPECTOR_BG, opt_solid(0xea, 0xf5, 0xef, 0xff))
                .set(DEMO_MOTION_STAGE_BG, opt_solid(0xf2, 0xf4, 0xf8, 0xff))
                .set(DEMO_ACCENT_BG, opt_solid(0x2f, 0x62, 0xd6, 0xff))
                .set(DEMO_TEXT_FG, solid(0x16, 0x1a, 0x24, 0xff))
                .set(DEMO_BODY_FG, solid(0x3a, 0x40, 0x4e, 0xff))
                .set(DEMO_MUTED_FG, solid(0x61, 0x68, 0x78, 0xff))
                .set(DEMO_TITLE_FG, solid(0x6a, 0x72, 0x83, 0xff))
                .set(DEMO_FOCUS_BORDER, opt_solid(0x1e, 0x53, 0xc8, 0xff))
                .build(),
        }
    }
}

#[derive(Debug)]
struct DemoShellState {
    theme_mode: Cell<DemoThemeMode>,
    selected_page: Cell<DemoPage>,
}

impl DemoShellState {
    fn new() -> Self {
        Self {
            theme_mode: Cell::new(DemoThemeMode::Dark),
            selected_page: Cell::new(DemoPage::Overview),
        }
    }

    fn theme_mode(&self) -> DemoThemeMode {
        self.theme_mode.get()
    }

    fn toggle_theme(&self) -> DemoThemeMode {
        let next = self.theme_mode.get().toggled();
        self.theme_mode.set(next);
        next
    }

    fn set_selected_page(&self, page: DemoPage) {
        self.selected_page.set(page);
    }
}

#[derive(Clone, Copy, Debug)]
struct NavChrome {
    mode_toggle: ElementId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NavKeyAction {
    Activate(DemoPage),
    MoveTo(DemoPage),
}

#[derive(Clone, Debug)]
struct SvgViewportDemo {
    path_count: usize,
    selected_path: Option<usize>,
    viewport: Viewport2D,
    world_bounds: Rect,
}

impl SvgViewportDemo {
    fn new(svg: &SvgElementTree, view_width: f32, view_height: f32) -> Self {
        let world_bounds = Rect::new(0.0, 0.0, f64::from(svg.width()), f64::from(svg.height()));
        let mut viewport = Viewport2D::new(Rect::new(
            0.0,
            0.0,
            f64::from(view_width),
            f64::from(view_height),
        ));
        viewport.set_fit_mode(FitMode::Center);
        viewport.set_clamp_mode(ClampMode::KeepSomeVisible);
        viewport.set_zoom_limits(0.12, 18.0);
        viewport.set_world_bounds(Some(padded_svg_bounds(world_bounds)));
        viewport.fit_world();
        Self {
            path_count: svg.len(),
            selected_path: None,
            viewport,
            world_bounds,
        }
    }

    fn reset_to_fit(&mut self) {
        self.viewport.fit_rect(padded_svg_bounds(self.world_bounds));
    }

    fn zoom_about(&mut self, anchor: Point, factor: f64) {
        self.viewport.zoom_about_view_point(anchor, factor);
    }

    fn zoom_center(&mut self, factor: f64) {
        self.zoom_about(self.viewport.view_rect().center(), factor);
    }

    fn pan_by_view(&mut self, delta: Vec2) {
        self.viewport.pan_by_view(delta);
    }

    fn select_path(&mut self, path: Option<usize>) {
        self.selected_path = path;
    }

    fn status(&self) -> String {
        let zoom = self.viewport.zoom() * 100.0;
        format!("{} paths · {zoom:.0}%", self.path_count)
    }

    fn path_count_readout(&self) -> String {
        format!("{}", self.path_count)
    }

    fn zoom_readout(&self) -> String {
        format!("{:.0}%", self.viewport.zoom() * 100.0)
    }

    fn visible_rect_readout(&self) -> String {
        let visible = self.viewport.visible_world_rect();
        format!(
            "{:.0},{:.0} / {:.0}x{:.0}",
            visible.x0,
            visible.y0,
            visible.width(),
            visible.height(),
        )
    }

    fn transform_readout(&self) -> String {
        let origin = self.viewport.world_to_view_point(Point::ZERO);
        format!(
            "scale {:.2}\noffset {:.0},{:.0}",
            self.viewport.zoom(),
            origin.x,
            origin.y,
        )
    }

    fn selected_path_readout(&self, tree: &SvgElementTree) -> String {
        let Some(path_index) = self.selected_path else {
            return String::from("None");
        };
        let label = tree
            .path(path_index)
            .and_then(overstory_svg::SvgPathElement::source_id)
            .unwrap_or("path");
        format!("#{path_index} {label}")
    }

    fn selected_bounds_readout(&self, tree: &SvgElementTree) -> String {
        let Some(path) = self.selected_path.and_then(|index| tree.path(index)) else {
            return String::from("-");
        };
        let bounds = path.bounds();
        format!(
            "{:.0},{:.0} / {:.0}x{:.0}",
            bounds.x0,
            bounds.y0,
            bounds.width(),
            bounds.height(),
        )
    }
}

fn caption(text: &str) -> Text {
    Text::new(text).class(CLASS_DEMO_CAPTION)
}

fn heading(text: &str) -> Text {
    Text::new(text).class(CLASS_DEMO_HEADING)
}

fn page_heading(text: &str) -> Text {
    Text::new(text).class(CLASS_DEMO_PAGE_HEADING)
}

fn card_title(text: &str) -> Text {
    Text::new(text).class(CLASS_DEMO_CARD_TITLE)
}

fn control_label(text: &str) -> Text {
    Text::new(text).class(CLASS_DEMO_CONTROL_LABEL)
}

fn number_text(text: &str) -> Text {
    Text::new(text).class(CLASS_DEMO_NUMBER)
}

fn large_number_text(text: &str) -> Text {
    Text::new(text).class(CLASS_DEMO_LARGE_NUMBER)
}

fn body_text(text: &str) -> Text {
    Text::new(text).class(CLASS_DEMO_BODY).wrap()
}

fn opt_solid(r: u8, g: u8, b: u8, a: u8) -> Option<Brush> {
    Some(solid(r, g, b, a))
}

fn solid(r: u8, g: u8, b: u8, a: u8) -> Brush {
    Color::from_rgba8(r, g, b, a).into()
}

fn tabular_number_features() -> Box<[FontFeature]> {
    let feature = FontFeature::parse_css_list("\"tnum\" on")
        .next()
        .expect("tabular-number feature should parse")
        .expect("tabular-number feature should be valid");
    Box::from([feature])
}

fn renderer_queue_items() -> impl Iterator<Item = String> {
    (0..RENDERER_QUEUE_LEN).map(renderer_queue_label)
}

fn renderer_queue_label(index: usize) -> String {
    let stage = RENDERER_STAGES[index % RENDERER_STAGES.len()];
    let batch = (index / RENDERER_STAGES.len()) + 1;
    format!("{stage} {batch:03}")
}

fn demo_card(ui: &mut Ui, parent: ElementId, title: &str, width: f32, height: f32) -> ElementId {
    let card = ui.append(
        parent,
        Panel::column(14.0).class(CLASS_DEMO_CARD).layout(
            Layout::column(14.0)
                .logical_size(width, height)
                .padding_all(18.0),
        ),
    );
    ui.append(card, card_title(title));
    card
}

fn demo_row(ui: &mut Ui, parent: ElementId) -> ElementId {
    ui.append(
        parent,
        Panel::row(18.0).layout(
            Layout::row(18.0).wrap().size(
                LayoutSize::new()
                    .width(LayoutDimension::Percent(1.0))
                    .min_width(LayoutDimension::Logical(0.0)),
            ),
        ),
    )
}

/// Builds a click handler that cycles the button's label each time it fires.
fn cycling_click_handler() -> impl FnMut(&mut Ui, overstory::ClickEvent<'_>) + 'static {
    let activations = Cell::new(0_u32);
    move |ui: &mut Ui, event| {
        if !matches!(
            event.phase,
            overstory::Phase::Target | overstory::Phase::Bubble
        ) {
            return;
        }
        let n = activations.get() + 1;
        activations.set(n);
        let label = if n == 1 {
            "Clicked!".to_owned()
        } else {
            format!("Clicked {n}×")
        };
        ui.set_local(event.current, ui.properties().content, label);
    }
}

fn popup_click_handler() -> impl FnMut(&mut Ui, overstory::ClickEvent<'_>) + 'static {
    let open_popup = Cell::new(None);
    move |ui: &mut Ui, event| {
        if !matches!(
            event.phase,
            overstory::Phase::Target | overstory::Phase::Bubble
        ) {
            return;
        }
        if let Some(popup) = open_popup.get()
            && ui.close_popup(popup)
        {
            open_popup.set(None);
            return;
        }
        let popup = ui.open_popup(event.current, Popup::new(), |ui, popup| {
            ui.append(popup, Text::new("Review queue").font_size(13.0));
            ui.append(popup, Text::new("3 items pending").font_size(13.0));
        });
        open_popup.set(popup);
    }
}

fn demo_cascade(properties: &BuiltInProperties) -> StyleCascade {
    theme::default_cascade_builder(properties)
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_DEMO_ROOT),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_ROOT_BG)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_DEMO_NAV),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_NAV_BG)
                .set_resource(properties.border_brush, DEMO_NAV_BORDER)
                .set(properties.border_width, 1.0_f64)
                .set(properties.corner_radius, 16.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_DEMO_STAGE),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_STAGE_BG)
                .set_resource(properties.border_brush, DEMO_STAGE_BORDER)
                .set(properties.border_width, 1.0_f64)
                .set(properties.corner_radius, 18.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_DEMO_HEADER),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_HEADER_BG)
                .set_resource(properties.border_brush, DEMO_HEADER_BORDER)
                .set(properties.border_width, 1.0_f64)
                .set(properties.corner_radius, 16.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_DEMO_CARD),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_CARD_BG)
                .set_resource(properties.border_brush, DEMO_CARD_BORDER)
                .set(properties.border_width, 1.0_f64)
                .set(properties.corner_radius, 12.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_DEMO_NAV_MARKER),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_ACCENT_BG)
                .set(properties.corner_radius, 2.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_DEMO_HEADER_ACCENT),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_ACCENT_BG)
                .set(properties.corner_radius, 8.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_DEMO_NAV_ROW),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_NAV_BG)
                .set_resource(properties.border_brush, DEMO_NAV_BG)
                .set(properties.border_width, 1.0_f64)
                .set(properties.corner_radius, 8.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(CLASS_DEMO_NAV_ROW, PSEUDO_SELECTED),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_NAV_SELECTED_BG)
                .set_resource(properties.border_brush, DEMO_NAV_SELECTED_BG)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(CLASS_DEMO_NAV_ROW, PSEUDO_FOCUS_VISIBLE),
            StyleBuilder::new()
                .set_resource(properties.border_brush, DEMO_FOCUS_BORDER)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_DEMO_SPLIT_SCENE),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_SPLIT_SCENE_BG)
                .set_resource(properties.border_brush, DEMO_SPLIT_BORDER)
                .set(properties.border_width, 1.0_f64)
                .set(properties.corner_radius, 8.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_DEMO_SPLIT_INSPECTOR),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_SPLIT_INSPECTOR_BG)
                .set_resource(properties.border_brush, DEMO_SPLIT_BORDER)
                .set(properties.border_width, 1.0_f64)
                .set(properties.corner_radius, 8.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_DEMO_MOTION_STAGE),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_MOTION_STAGE_BG)
                .set_resource(properties.border_brush, DEMO_MOTION_STAGE_BG)
                .set(properties.border_width, 1.0_f64)
                .set(properties.corner_radius, 14.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(CLASS_DEMO_HEADING),
            StyleBuilder::new()
                .set_resource(properties.foreground, DEMO_TEXT_FG)
                .set(properties.font_size, 26.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(CLASS_DEMO_PAGE_HEADING),
            StyleBuilder::new()
                .set_resource(properties.foreground, DEMO_TEXT_FG)
                .set(properties.font_size, 22.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(CLASS_DEMO_CAPTION),
            StyleBuilder::new()
                .set_resource(properties.foreground, DEMO_MUTED_FG)
                .set(properties.font_size, 13.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(CLASS_DEMO_CARD_TITLE),
            StyleBuilder::new()
                .set_resource(properties.foreground, DEMO_TITLE_FG)
                .set(properties.font_size, 12.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(CLASS_DEMO_CONTROL_LABEL),
            StyleBuilder::new()
                .set_resource(properties.foreground, DEMO_TEXT_FG)
                .set(properties.font_size, 15.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(CLASS_DEMO_NAV_LABEL),
            StyleBuilder::new()
                .set_resource(properties.foreground, DEMO_TEXT_FG)
                .set(properties.font_size, 15.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            selected_nav_label_selector(),
            StyleBuilder::new()
                .set_resource(properties.foreground, theme::ON_ACCENT_FG)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(CLASS_DEMO_NUMBER),
            StyleBuilder::new()
                .set_resource(properties.foreground, DEMO_TEXT_FG)
                .set(properties.font_size, 22.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(CLASS_DEMO_LARGE_NUMBER),
            StyleBuilder::new()
                .set_resource(properties.foreground, DEMO_TEXT_FG)
                .set(properties.font_size, 34.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(CLASS_DEMO_BODY),
            StyleBuilder::new()
                .set_resource(properties.foreground, DEMO_BODY_FG)
                .set(properties.font_size, 14.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_OKLAB_SWATCH),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0xff, 0x2a, 0x78, 0xff))
                .set(properties.corner_radius, 18.0_f64)
                .set(
                    properties.transitions,
                    color_transition_rules(
                        properties,
                        millis(5_500),
                        MID_SLOW,
                        ColorInterpolation::OKLAB,
                    ),
                )
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(CLASS_OKLAB_SWATCH, PSEUDO_SELECTED),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0x26, 0xd9, 0x8e, 0xff))
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_SRGB_SWATCH),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0xff, 0x2a, 0x78, 0xff))
                .set(properties.corner_radius, 18.0_f64)
                .set(
                    properties.transitions,
                    color_transition_rules(
                        properties,
                        millis(5_500),
                        MID_SLOW,
                        ColorInterpolation::SRGB,
                    ),
                )
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(CLASS_SRGB_SWATCH, PSEUDO_SELECTED),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0x26, 0xd9, 0x8e, 0xff))
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_OPACITY_SWATCH),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0x6d, 0xf2, 0x96, 0xff))
                .set(properties.corner_radius, 18.0_f64)
                .set(properties.opacity, 1.0_f32)
                .set(
                    properties.transitions,
                    scalar_transition_rules(
                        TransitionTarget::property(properties.opacity.id()),
                        TransitionSpec::timed(millis(780), EASE_IN_OUT),
                    ),
                )
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(CLASS_OPACITY_SWATCH, PSEUDO_PRESSED),
            StyleBuilder::new()
                .set(properties.opacity, 0.16_f32)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_BORDER_SWATCH),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0x25, 0x29, 0x38, 0xff))
                .set(properties.border_brush, opt_solid(0x79, 0x86, 0xff, 0xff))
                .set(properties.border_width, 2.0_f64)
                .set(properties.corner_radius, 18.0_f64)
                .set(properties.transitions, border_transition_rules(properties))
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(CLASS_BORDER_SWATCH, PSEUDO_FOCUS_VISIBLE),
            StyleBuilder::new()
                .set(properties.border_brush, opt_solid(0xff, 0xff, 0xff, 0xff))
                .set(properties.border_width, 12.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_MOVER_SWATCH),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0xff, 0x9f, 0x1c, 0xff))
                .set(properties.corner_radius, 21.0_f64)
                .set(
                    properties.transitions,
                    scalar_transition_rules(
                        TransitionTarget::layer_offset(),
                        TransitionSpec::timed(millis(920), EASE_IN_OUT),
                    ),
                )
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_RETARGET_SWATCH),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0xd8, 0xb4, 0xfe, 0xff))
                .set(properties.corner_radius, 18.0_f64)
                .set(
                    properties.transitions,
                    color_transition_rules(
                        properties,
                        millis(780),
                        EASE_OUT,
                        ColorInterpolation::OKLAB,
                    ),
                )
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(CLASS_RETARGET_SWATCH, PSEUDO_SELECTED),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0xf9, 0xf8, 0x71, 0xff))
                .build(),
        )
        .build()
}

/// Builds the demo tree as a focused gallery. The shell owns page selection;
/// each page owns only its local controls and timers.
fn build_ui() -> Ui {
    let mut ui = Ui::new();
    let shell = Rc::new(DemoShellState::new());
    ui.set_theme(shell.theme_mode().theme());
    let root = ui.root();
    ui.add_class(root, CLASS_DEMO_ROOT);

    ui.set_layout(
        root,
        layouts::row_with_gap(ROOT_GAP)
            .percent_size(1.0, 1.0)
            .padding_all(ROOT_PADDING)
            .alignment(LayoutAlignment::new().align_items(LayoutAlign::Stretch)),
    );

    let nav = ui.append(
        root,
        Panel::column(18.0).class(CLASS_DEMO_NAV).layout(
            Layout::column(18.0)
                .size(
                    LayoutSize::new()
                        .width(LayoutDimension::Logical(NAV_WIDTH))
                        .height(LayoutDimension::Percent(1.0)),
                )
                .padding_all(NAV_PADDING)
                .flex_shrink(0.0),
        ),
    );
    let nav_chrome = build_nav(&mut ui, nav);

    let stage = ui.append(
        root,
        Panel::column(18.0).class(CLASS_DEMO_STAGE).layout(
            Layout::column(18.0)
                .size(
                    LayoutSize::new()
                        .height(LayoutDimension::Percent(1.0))
                        .min_width(LayoutDimension::Logical(0.0)),
                )
                .flex_grow(1.0)
                .flex_shrink(1.0),
        ),
    );

    let nav_rows = build_nav_rows(&mut ui, nav);
    wire_nav_rows(&mut ui, &nav_rows, stage, Rc::clone(&shell));
    wire_theme_toggle(&mut ui, nav_chrome, Rc::clone(&shell));
    show_demo_page(&mut ui, stage, DemoPage::Overview);

    ui.set_cascade(demo_cascade(ui.properties()));
    ui
}

fn build_nav(ui: &mut Ui, nav: ElementId) -> NavChrome {
    ui.append(nav, heading("Overstory"));
    ui.append(nav, caption("Focused widget gallery"));

    ui.append(
        nav,
        Panel::new()
            .class(CLASS_DEMO_NAV_MARKER)
            .layout(Layout::new().logical_size(NAV_CONTENT_WIDTH, 4.0)),
    );

    let mode_row = ui.append(
        nav,
        Panel::row(12.0).layout(
            Layout::row(12.0)
                .logical_size(NAV_CONTENT_WIDTH, 36.0)
                .align_center(),
        ),
    );
    let mode_toggle = ui.append(mode_row, ToggleSwitch::new());
    ui.append(mode_row, control_label("Light mode"));

    NavChrome { mode_toggle }
}

fn build_nav_rows(ui: &mut Ui, nav: ElementId) -> Rc<[ElementId]> {
    let stack = ui.append(
        nav,
        Panel::column(4.0).layout(Layout::column(4.0).logical_size(NAV_CONTENT_WIDTH, 348.0)),
    );
    let mut rows = Vec::with_capacity(DemoPage::ALL.len());
    for page in DemoPage::ALL {
        rows.push(append_nav_row(ui, stack, page, page == DemoPage::Overview));
    }
    Rc::from(rows.into_boxed_slice())
}

fn append_nav_row(ui: &mut Ui, stack: ElementId, page: DemoPage, selected: bool) -> ElementId {
    let row = ui.append(
        stack,
        Panel::row(0.0).class(CLASS_DEMO_NAV_ROW).layout(
            Layout::row(0.0)
                .logical_size(NAV_CONTENT_WIDTH, 40.0)
                .padding_xy(14.0, 0.0)
                .align_center(),
        ),
    );
    ui.append(row, Text::new(page.nav_label()).class(CLASS_DEMO_NAV_LABEL));
    style_nav_row(ui, row, selected);
    row
}

fn wire_nav_rows(
    ui: &mut Ui,
    nav_rows: &Rc<[ElementId]>,
    stage: ElementId,
    shell: Rc<DemoShellState>,
) {
    for (page, row) in DemoPage::ALL.iter().copied().zip(nav_rows.iter().copied()) {
        ui.set_focusable(row, true);

        let nav_rows_for_click = Rc::clone(nav_rows);
        let shell_for_click = Rc::clone(&shell);
        ui.on_click(row, move |ui, event| {
            if !matches!(
                event.phase,
                overstory::Phase::Target | overstory::Phase::Bubble
            ) {
                return;
            }
            let _ = ui.set_pointer_focus(Some(row));
            activate_nav_page(ui, &nav_rows_for_click, &shell_for_click, stage, page);
            event.mark_handled();
        });

        let nav_rows_for_key = Rc::clone(nav_rows);
        let shell_for_key = Rc::clone(&shell);
        ui.on_key(row, move |ui, event| {
            if !matches!(
                event.phase,
                overstory::Phase::Target | overstory::Phase::Bubble
            ) {
                return;
            }
            let Some(action) = nav_key_action(page, event.key) else {
                return;
            };
            match action {
                NavKeyAction::Activate(page) => {
                    activate_nav_page(ui, &nav_rows_for_key, &shell_for_key, stage, page);
                }
                NavKeyAction::MoveTo(page) => {
                    focus_nav_page(ui, &nav_rows_for_key, page);
                    activate_nav_page(ui, &nav_rows_for_key, &shell_for_key, stage, page);
                }
            }
            event.mark_handled();
        });
    }
}

fn wire_theme_toggle(ui: &mut Ui, nav_chrome: NavChrome, shell: Rc<DemoShellState>) {
    let shell_for_click = Rc::clone(&shell);
    ui.on_click(nav_chrome.mode_toggle, move |ui, event| {
        if !matches!(
            event.phase,
            overstory::Phase::Target | overstory::Phase::Bubble
        ) {
            return;
        }
        switch_demo_theme(ui, &shell_for_click);
    });

    let shell_for_key = Rc::clone(&shell);
    ui.on_key(nav_chrome.mode_toggle, move |ui, event| {
        if !matches!(event.phase, overstory::Phase::Target) || !is_activation_key(event.key) {
            return;
        }
        switch_demo_theme(ui, &shell_for_key);
    });
}

fn switch_demo_theme(ui: &mut Ui, shell: &DemoShellState) {
    ui.set_theme(shell.toggle_theme().theme());
}

fn activate_nav_page(
    ui: &mut Ui,
    nav_rows: &[ElementId],
    shell: &DemoShellState,
    stage: ElementId,
    page: DemoPage,
) {
    shell.set_selected_page(page);
    style_nav_rows(ui, nav_rows, page);
    show_demo_page(ui, stage, page);
}

fn focus_nav_page(ui: &mut Ui, nav_rows: &[ElementId], page: DemoPage) {
    if let Some(row) = nav_rows.get(page.index()).copied() {
        let _ = ui.set_focus(Some(row));
    }
}

fn nav_key_action(
    page: DemoPage,
    event: &ui_events::keyboard::KeyboardEvent,
) -> Option<NavKeyAction> {
    use ui_events::keyboard::{Key, KeyState, NamedKey};

    if event.state != KeyState::Down {
        return None;
    }
    match &event.key {
        Key::Named(NamedKey::ArrowDown | NamedKey::ArrowRight) => {
            Some(NavKeyAction::MoveTo(page_offset(page, 1)))
        }
        Key::Named(NamedKey::ArrowUp | NamedKey::ArrowLeft) => {
            Some(NavKeyAction::MoveTo(page_offset(page, -1)))
        }
        Key::Named(NamedKey::Home) => DemoPage::from_index(0).map(NavKeyAction::MoveTo),
        Key::Named(NamedKey::End) => {
            DemoPage::from_index(DemoPage::ALL.len() - 1).map(NavKeyAction::MoveTo)
        }
        Key::Named(NamedKey::Enter) => Some(NavKeyAction::Activate(page)),
        Key::Character(s) if s == " " => Some(NavKeyAction::Activate(page)),
        _ => None,
    }
}

fn is_activation_key(event: &ui_events::keyboard::KeyboardEvent) -> bool {
    use ui_events::keyboard::{Key, KeyState, NamedKey};

    if event.state != KeyState::Down {
        return false;
    }
    match &event.key {
        Key::Named(NamedKey::Enter) => true,
        Key::Character(s) if s == " " => true,
        _ => false,
    }
}

fn page_offset(page: DemoPage, delta: isize) -> DemoPage {
    let len = DemoPage::ALL.len() as isize;
    let index = (page.index() as isize + delta).rem_euclid(len) as usize;
    DemoPage::from_index(index).expect("wrapped page index should be valid")
}

fn style_nav_rows(ui: &mut Ui, nav_rows: &[ElementId], selected_page: DemoPage) {
    for (page, row) in DemoPage::ALL.iter().copied().zip(nav_rows.iter().copied()) {
        style_nav_row(ui, row, page == selected_page);
    }
}

fn style_nav_row(ui: &mut Ui, row: ElementId, selected: bool) {
    if selected {
        ui.add_pseudo(row, PSEUDO_SELECTED);
    } else {
        ui.remove_pseudo(row, PSEUDO_SELECTED);
    }
}

fn clear_children(ui: &mut Ui, parent: ElementId) {
    let children: Vec<ElementId> = ui
        .get(parent)
        .map(|element| element.children().to_vec())
        .unwrap_or_default();
    for child in children {
        if ui.get(child).is_some() {
            ui.remove(child);
        }
    }
}

fn show_demo_page(ui: &mut Ui, stage: ElementId, page: DemoPage) {
    clear_children(ui, stage);
    build_page_header(ui, stage, page);
    match page {
        DemoPage::Overview => build_overview_page(ui, stage),
        DemoPage::Inputs => build_inputs_page(ui, stage),
        DemoPage::Systems => build_systems_page(ui, stage),
        DemoPage::Selection => build_selection_page(ui, stage),
        DemoPage::Layout => build_layout_page(ui, stage),
        DemoPage::Svg => build_svg_page(ui, stage),
        DemoPage::Typography => build_typography_page(ui, stage),
        DemoPage::Motion => build_motion_page(ui, stage),
    }
}

fn build_page_header(ui: &mut Ui, stage: ElementId, page: DemoPage) {
    let header = ui.append(
        stage,
        Panel::row(18.0).class(CLASS_DEMO_HEADER).layout(
            Layout::row(18.0)
                .size(
                    LayoutSize::new()
                        .width(LayoutDimension::Percent(1.0))
                        .height(LayoutDimension::Logical(PAGE_HEADER_HEIGHT)),
                )
                .padding_all(18.0),
        ),
    );

    ui.append(
        header,
        Panel::new()
            .class(CLASS_DEMO_HEADER_ACCENT)
            .layout(Layout::new().logical_size(5.0, 56.0).flex_shrink(0.0)),
    );

    let copy = ui.append(
        header,
        Panel::column(8.0).layout(
            Layout::column(8.0)
                .size(LayoutSize::new().min_width(LayoutDimension::Logical(0.0)))
                .flex_grow(1.0)
                .flex_shrink(1.0),
        ),
    );
    ui.append(copy, page_heading(page.title()));
    ui.append(copy, caption(page.subtitle()).wrap());
}

fn build_overview_page(ui: &mut Ui, stage: ElementId) {
    let row = demo_row(ui, stage);
    let inputs = demo_card(ui, row, "INPUTS", OVERVIEW_CARD_WIDTH, 222.0);
    ui.append(
        inputs,
        TextInput::new()
            .placeholder("Search actions")
            .width(OVERVIEW_CONTROL_WIDTH),
    );
    let actions = ui.append(inputs, Panel::row(10.0).align_center());
    let deploy = ui.append(
        actions,
        Button::new("Deploy")
            .font_size(15.0)
            .on_click(cycling_click_handler()),
    );
    ui.set_tooltip(deploy, Tooltip::new("Button content is template-bound"));
    ui.append(actions, Button::new("Preview").font_size(15.0));
    ui.append(inputs, CheckBox::new("Record diagnostics").checked(true));

    let systems = demo_card(ui, row, "SYSTEMS", OVERVIEW_CARD_WIDTH, 222.0);
    let toggle_row = ui.append(systems, Panel::row(12.0).align_center());
    ui.append(toggle_row, ToggleSwitch::new().checked(true));
    ui.append(toggle_row, control_label("Rendering enabled"));
    ui.append(systems, caption("GPU Budget"));
    let progress = ui.append(
        systems,
        ProgressBar::new()
            .value(0.62)
            .size(OVERVIEW_CONTROL_WIDTH, 18.0),
    );
    drive_progress(ui, progress, 0.62, 0.015, 120_000_000);
    append_tabular_counter(ui, systems, "Frames", "fps", 120_000_000, 7);

    let selection = demo_card(ui, row, "SELECTION", OVERVIEW_CARD_WIDTH, 222.0);
    let renderer_row = ui.append(selection, Panel::row(8.0).align_center());
    let list = ui.append(
        renderer_row,
        ListBox::new()
            .items(renderer_queue_items())
            .size(OVERVIEW_LIST_WIDTH, 132.0)
            .multiple_selection()
            .selected_index(INITIAL_RENDERER_SELECTION)
            .scroll_offset_y(28.0),
    );
    ui.append(
        renderer_row,
        ScrollBar::vertical(list)
            .length(132.0)
            .thickness(10.0)
            .thumb_extent(34.0),
    );
}

fn build_inputs_page(ui: &mut Ui, stage: ElementId) {
    let row = demo_row(ui, stage);
    let forms = demo_card(ui, row, "TEXT INPUTS", PAGE_CARD_WIDTH, 224.0);
    ui.append(
        forms,
        TextInput::new()
            .placeholder("Search command palette")
            .width(PAGE_CONTROL_WIDTH),
    );
    ui.append(
        forms,
        TextInput::new()
            .text("Read-only selection")
            .read_only(true)
            .width(PAGE_CONTROL_WIDTH),
    );
    ui.append(
        forms,
        caption("Focus rings, hover borders, selection, and read-only state"),
    );

    let actions = demo_card(ui, row, "ACTIONS", PAGE_CARD_WIDTH, 224.0);
    let action_row = ui.append(actions, Panel::row(10.0).align_center());
    let deploy = ui.append(
        action_row,
        Button::new("Deploy")
            .font_size(15.0)
            .on_click(cycling_click_handler()),
    );
    ui.set_tooltip(deploy, Tooltip::new("Run the staged deployment workflow"));
    let review = ui.append(
        action_row,
        Button::new("Review")
            .font_size(15.0)
            .on_click(popup_click_handler()),
    );
    ui.set_tooltip(review, Tooltip::new("Toggle an anchored popup"));
    ui.append(actions, Divider::horizontal().length(PAGE_CONTROL_WIDTH));
    ui.append(
        actions,
        CheckBox::new("Enable staged rollout").checked(true),
    );
    ui.append(actions, CheckBox::new("Collect timing traces"));
    ui.append(actions, CheckBox::new("Send diagnostics").disabled());
}

fn build_systems_page(ui: &mut Ui, stage: ElementId) {
    let row = demo_row(ui, stage);
    let controls = demo_card(ui, row, "CONTROL SURFACE", PAGE_CARD_WIDTH, 322.0);
    let toggle_row = ui.append(controls, Panel::row(12.0).align_center());
    ui.append(toggle_row, ToggleSwitch::new().checked(true));
    ui.append(toggle_row, control_label("Rendering enabled"));
    ui.append(controls, Divider::horizontal().length(PAGE_CONTROL_WIDTH));
    ui.append(controls, caption("GPU Budget"));
    let progress = ui.append(
        controls,
        ProgressBar::new()
            .value(0.62)
            .size(PAGE_CONTROL_WIDTH, 18.0),
    );
    ui.set_tooltip(
        progress,
        Tooltip::new("Animated by the retained timer queue"),
    );
    drive_progress(ui, progress, 0.62, 0.015, 120_000_000);
    ui.append(controls, caption("Volume"));
    let volume = ui.append(controls, Slider::new().value(0.42));
    ui.set_tooltip(
        volume,
        Tooltip::new("Drag, arrow, PageUp/PageDown, or Shift+arrow"),
    );
    let spinner_row = ui.append(controls, Panel::row(12.0).align_center());
    ui.append(spinner_row, Spinner::new());
    ui.append(spinner_row, caption("Streaming updates"));

    let telemetry = demo_card(ui, row, "TELEMETRY", PAGE_CARD_WIDTH, 218.0);
    append_large_counter(ui, telemetry, "Throughput", "events / min", 90_000_000, 137);
}

fn build_selection_page(ui: &mut Ui, stage: ElementId) {
    let row = demo_row(ui, stage);
    let list_card = demo_card(ui, row, "RENDERER QUEUE", PAGE_CARD_WIDTH, 326.0);
    let renderer_row = ui.append(list_card, Panel::row(10.0).align_center());
    let initial_summary = format!(
        "Selected: {}",
        renderer_queue_label(INITIAL_RENDERER_SELECTION)
    );
    let summary = ui.append(list_card, caption(&initial_summary));
    let renderer_list = ui.append(
        renderer_row,
        ListBox::new()
            .items(renderer_queue_items())
            .size(PAGE_LIST_WIDTH, 178.0)
            .multiple_selection()
            .selected_index(INITIAL_RENDERER_SELECTION)
            .scroll_offset_y(34.0)
            .on_selection_changed(move |ui, event| {
                let label = selection_summary(&event);
                ui.set_local(summary, ui.properties().content, label);
            }),
    );
    ui.append(
        renderer_row,
        ScrollBar::vertical(renderer_list)
            .length(178.0)
            .thickness(10.0)
            .thumb_extent(40.0),
    );
    ui.set_tooltip(
        renderer_list,
        Tooltip::new("Multi-select list with viewport clipping"),
    );

    let detail = demo_card(ui, row, "DETAIL", PAGE_CARD_WIDTH, 326.0);
    ui.append(detail, page_heading("Virtualized by default"));
    ui.append(
        detail,
        body_text("Only visible rows are realized. Selection stays in item indices, and the scrollbar shares viewport metrics with the list."),
    );
    ui.append(detail, control_label("Inspection target"));
    let select = ui.append(
        detail,
        Select::new()
            .items([
                "Renderer queue",
                "Layer composition",
                "Text shaping",
                "Input routing",
            ])
            .selected_index(1)
            .width(PAGE_CONTROL_WIDTH)
            .popup_height(124.0),
    );
    ui.set_tooltip(
        select,
        Tooltip::new("Popup-backed select using the ListBox path"),
    );
}

fn build_layout_page(ui: &mut Ui, stage: ElementId) {
    let row = demo_row(ui, stage);
    let panes = demo_card(ui, row, "RESIZABLE PANES", PAGE_CARD_WIDTH, 326.0);
    let split_row = ui.append(panes, Panel::row(8.0).align_center());
    let left_pane = ui.append(
        split_row,
        Panel::column(8.0).class(CLASS_DEMO_SPLIT_SCENE).layout(
            Layout::column(8.0)
                .logical_size(SPLIT_PANE_WIDTH, SPLIT_PANE_HEIGHT)
                .padding_all(14.0),
        ),
    );
    ui.append(left_pane, control_label("Scene"));
    if let Ok(svg) = SvgElementTree::from_bytes(SCENE_GRAPH_SVG, SvgImportOptions::default()) {
        ui.append(left_pane, svg);
    }
    ui.append(left_pane, caption("Retained graph"));

    let right_pane_slot = Rc::new(Cell::new(None));
    let right_pane_for_splitter = Rc::clone(&right_pane_slot);
    let splitter = ui.append(
        split_row,
        Splitter::vertical()
            .length(SPLIT_PANE_HEIGHT)
            .on_resize(move |ui, event| {
                if let Some(right_pane) = right_pane_for_splitter.get() {
                    event.resize_pair(ui, left_pane, right_pane, 88.0, 88.0);
                }
            }),
    );
    ui.set_tooltip(splitter, Tooltip::new("Drag or focus and use arrow keys"));
    let right_pane = ui.append(
        split_row,
        Panel::column(8.0).class(CLASS_DEMO_SPLIT_INSPECTOR).layout(
            Layout::column(8.0)
                .logical_size(SPLIT_PANE_WIDTH, SPLIT_PANE_HEIGHT)
                .padding_all(14.0),
        ),
    );
    right_pane_slot.set(Some(right_pane));
    ui.append(right_pane, control_label("Inspector"));
    ui.append(right_pane, caption("Keyboard resize"));

    let ruler = demo_card(ui, row, "AXIS RULER", PAGE_CARD_WIDTH, 326.0);
    ui.append(ruler, caption("Slider ticks come from understory_axis"));
    ui.append(
        ruler,
        Slider::new()
            .min(-24.0)
            .max(24.0)
            .value(6.0)
            .small_change(1.0)
            .large_change(8.0),
    );
    ui.append(ruler, Divider::horizontal().length(PAGE_CONTROL_WIDTH));
    ui.append(
        ruler,
        body_text("Longhand layout properties keep slider ticks, labels, and thumbs independently positioned without replacing the rest of the style."),
    );
}

fn build_svg_page(ui: &mut Ui, stage: ElementId) {
    let page = ui.append(
        stage,
        Panel::column(18.0).layout(
            Layout::column(18.0).size(
                LayoutSize::new()
                    .width(LayoutDimension::Percent(1.0))
                    .min_width(LayoutDimension::Logical(0.0)),
            ),
        ),
    );

    let notes = demo_card(ui, page, "IMPORT SURFACE", SVG_PAGE_WIDTH, SVG_INFO_HEIGHT);
    ui.add_class(notes, CLASS_SVG_INFO_CARD);
    append_svg_import_summary(ui, notes);

    let main = ui.append(
        page,
        Panel::row(18.0).layout(
            Layout::row(18.0).size(
                LayoutSize::new()
                    .width(LayoutDimension::Logical(SVG_PAGE_WIDTH))
                    .height(LayoutDimension::Logical(SVG_TIGER_CARD_HEIGHT)),
            ),
        ),
    );

    let specimen = demo_card(
        ui,
        main,
        "GHOSTSCRIPT TIGER",
        SVG_TIGER_CARD_WIDTH,
        SVG_TIGER_CARD_HEIGHT,
    );
    ui.add_class(specimen, CLASS_SVG_TIGER_CARD);
    let viewer = if let Ok(viewer) = append_svg_viewer(ui, specimen, TIGER_SVG) {
        let controls = ui.append(specimen, Panel::row(8.0).align_center());
        append_svg_view_button(ui, controls, "Fit", viewer.clone(), |state| {
            state.reset_to_fit();
        });
        append_svg_view_button(ui, controls, "-", viewer.clone(), |state| {
            state.zoom_center(0.82);
        });
        append_svg_view_button(ui, controls, "+", viewer.clone(), |state| {
            state.zoom_center(1.22);
        });
        Some(viewer)
    } else {
        ui.append(
            specimen,
            body_text("The SVG stress asset did not import any supported vector paths."),
        );
        None
    };

    let inspector = demo_card(
        ui,
        main,
        "INSPECTOR",
        SVG_INSPECTOR_CARD_WIDTH,
        SVG_TIGER_CARD_HEIGHT,
    );
    ui.add_class(inspector, CLASS_SVG_INSPECTOR_CARD);
    if let Some(viewer) = viewer {
        let inspector_state = append_svg_inspector(ui, inspector, &viewer);
        viewer.runtime.inspector.set(Some(inspector_state));
        refresh_svg_viewer(ui, &viewer);
    } else {
        append_svg_static_inspector(ui, inspector);
    }
}

fn append_svg_import_summary(ui: &mut Ui, parent: ElementId) {
    let row = ui.append(
        parent,
        Panel::row(18.0).layout(
            Layout::row(18.0).size(
                LayoutSize::new()
                    .width(LayoutDimension::Percent(1.0))
                    .min_width(LayoutDimension::Logical(0.0)),
            ),
        ),
    );
    let lowering = ui.append(
        row,
        Panel::column(6.0).layout(
            Layout::column(6.0).size(
                LayoutSize::new()
                    .width(LayoutDimension::Logical(414.0))
                    .min_width(LayoutDimension::Logical(0.0)),
            ),
        ),
    );
    ui.append(lowering, page_heading("Path-first lowering"));
    ui.append(
        lowering,
        body_text("The importer keeps SVG parsing outside the UI core, then appends ordinary elements that paint through the same presentation primitive path as hand-authored vector controls."),
    );
    let support = ui.append(
        row,
        Panel::column(6.0).layout(
            Layout::column(6.0).size(
                LayoutSize::new()
                    .width(LayoutDimension::Logical(322.0))
                    .min_width(LayoutDimension::Logical(0.0)),
            ),
        ),
    );
    ui.append(support, caption("SUPPORTED NOW"));
    ui.append(
        support,
        body_text("Current support is intentionally static: solid fills, strokes, transforms, opacity, fill rules, caps, joins, dashes, and paint order."),
    );
}

fn append_svg_static_inspector(ui: &mut Ui, parent: ElementId) {
    ui.append(parent, caption("DOCUMENT"));
    append_svg_inspector_row(ui, parent, "Asset", "Ghostscript Tiger");
    append_svg_inspector_row(ui, parent, "Lowering", "VectorPath tree");
    append_svg_inspector_row(ui, parent, "Viewport", "Pan + zoom layer");
    ui.append(parent, Divider::horizontal().length(200.0));
    ui.append(parent, caption("NEXT"));
    append_svg_inspector_row(ui, parent, "Outline", "SVG element tree");
    append_svg_inspector_row(ui, parent, "Selection", "Path metadata");
    append_svg_inspector_row(ui, parent, "Properties", "Fill, stroke, transform");
}

fn append_svg_inspector(ui: &mut Ui, parent: ElementId, viewer: &SvgViewer) -> SvgInspector {
    let state = viewer.runtime.state.borrow();
    ui.append(parent, caption("DOCUMENT"));
    append_svg_inspector_row(ui, parent, "Asset", "Ghostscript Tiger");
    let paths = append_svg_inspector_row(ui, parent, "Paths", &state.path_count_readout());
    let selected = append_svg_inspector_row(
        ui,
        parent,
        "Selected",
        &state.selected_path_readout(&viewer.tree),
    );
    let bounds = append_svg_inspector_row(
        ui,
        parent,
        "Bounds",
        &state.selected_bounds_readout(&viewer.tree),
    );
    let zoom = append_svg_inspector_row(ui, parent, "Zoom", &state.zoom_readout());
    let visible = append_svg_inspector_row(ui, parent, "Visible", &state.visible_rect_readout());
    let transform =
        append_svg_inspector_multiline_row(ui, parent, "Layer", &state.transform_readout());
    ui.append(parent, Divider::horizontal().length(200.0));
    ui.append(parent, caption("PIPELINE"));
    append_svg_inspector_row(ui, parent, "Lowering", "VectorPath tree");
    append_svg_inspector_row(ui, parent, "Grid", "Viewport overlay");
    append_svg_inspector_row(ui, parent, "Next", "Outline + properties");
    SvgInspector {
        paths,
        selected,
        bounds,
        zoom,
        visible,
        transform,
    }
}

fn update_svg_inspector(
    ui: &mut Ui,
    inspector: SvgInspector,
    state: &SvgViewportDemo,
    tree: &SvgElementTree,
) {
    ui.set_local(
        inspector.paths,
        ui.properties().content,
        state.path_count_readout(),
    );
    ui.set_local(
        inspector.selected,
        ui.properties().content,
        state.selected_path_readout(tree),
    );
    ui.set_local(
        inspector.bounds,
        ui.properties().content,
        state.selected_bounds_readout(tree),
    );
    ui.set_local(
        inspector.zoom,
        ui.properties().content,
        state.zoom_readout(),
    );
    ui.set_local(
        inspector.visible,
        ui.properties().content,
        state.visible_rect_readout(),
    );
    ui.set_local(
        inspector.transform,
        ui.properties().content,
        state.transform_readout(),
    );
}

fn append_svg_inspector_row(ui: &mut Ui, parent: ElementId, label: &str, value: &str) -> ElementId {
    append_svg_inspector_row_with_height(ui, parent, label, value, 18.0)
}

fn append_svg_inspector_multiline_row(
    ui: &mut Ui,
    parent: ElementId,
    label: &str,
    value: &str,
) -> ElementId {
    append_svg_inspector_row_with_height(ui, parent, label, value, 38.0)
}

fn append_svg_inspector_row_with_height(
    ui: &mut Ui,
    parent: ElementId,
    label: &str,
    value: &str,
    height: f32,
) -> ElementId {
    let row = ui.append(
        parent,
        Panel::row(8.0).layout(
            Layout::row(8.0).size(
                LayoutSize::new()
                    .width(LayoutDimension::Percent(1.0))
                    .height(LayoutDimension::Logical(height))
                    .min_width(LayoutDimension::Logical(0.0)),
            ),
        ),
    );
    let label = ui.append(row, caption(label));
    ui.set_layout(
        label,
        Layout::new()
            .logical_size(78.0, height)
            .flex_grow(0.0)
            .flex_shrink(0.0),
    );
    let value = ui.append(row, body_text(value));
    ui.set_layout(
        value,
        Layout::new()
            .size(
                LayoutSize::new()
                    .height(LayoutDimension::Logical(height))
                    .min_width(LayoutDimension::Logical(0.0)),
            )
            .flex_grow(1.0)
            .flex_shrink(1.0),
    );
    value
}

#[derive(Clone)]
struct SvgViewer {
    tree: Rc<SvgElementTree>,
    host: ElementId,
    grid: ElementId,
    content: ElementId,
    path_elements: Rc<[ElementId]>,
    selection: ElementId,
    status: ElementId,
    runtime: Rc<SvgViewerRuntime>,
}

#[derive(Debug)]
struct SvgViewerRuntime {
    state: RefCell<SvgViewportDemo>,
    inspector: Cell<Option<SvgInspector>>,
    wobble_generation: Cell<u32>,
    wobble_timers: RefCell<Vec<TimerId>>,
}

impl SvgViewerRuntime {
    fn new(state: SvgViewportDemo) -> Self {
        Self {
            state: RefCell::new(state),
            inspector: Cell::new(None),
            wobble_generation: Cell::new(0),
            wobble_timers: RefCell::new(Vec::new()),
        }
    }

    fn next_wobble_generation(&self) -> u32 {
        let next = self.wobble_generation.get().wrapping_add(1);
        self.wobble_generation.set(next);
        next
    }
}

#[derive(Clone, Copy, Debug)]
struct SvgInspector {
    paths: ElementId,
    selected: ElementId,
    bounds: ElementId,
    zoom: ElementId,
    visible: ElementId,
    transform: ElementId,
}

fn append_svg_viewer(ui: &mut Ui, parent: ElementId, bytes: &[u8]) -> Result<SvgViewer, ()> {
    let svg = SvgElementTree::from_bytes(bytes, SvgImportOptions::default()).map_err(|_| ())?;
    if svg.is_empty() {
        return Err(());
    }
    let tree = Rc::new(svg);
    let state = SvgViewportDemo::new(&tree, SVG_VIEWPORT_WIDTH, SVG_VIEWPORT_HEIGHT);
    let host = ui.append(
        parent,
        Viewport::new().cursor_icon(CursorIcon::Grab).layout(
            Layout::new()
                .relative()
                .logical_size(SVG_VIEWPORT_WIDTH, SVG_VIEWPORT_HEIGHT),
        ),
    );
    ui.set_local(
        host,
        ui.properties().background,
        opt_solid(0x13, 0x13, 0x1b, 0xff),
    );
    ui.set_local(
        host,
        ui.properties().border_brush,
        opt_solid(0x44, 0x46, 0x54, 0xff),
    );
    ui.set_local(host, ui.properties().border_width, 1.0_f64);
    ui.set_local(host, ui.properties().corner_radius, 12.0_f64);

    let grid = ui.append(
        host,
        Panel::new().layout(
            Layout::new()
                .absolute_at(0.0, 0.0)
                .logical_size(SVG_VIEWPORT_WIDTH, SVG_VIEWPORT_HEIGHT),
        ),
    );
    let world_bounds = state.world_bounds;
    let content = ui.append(
        host,
        Panel::new().layout(Layout::new().absolute_at(0.0, 0.0).logical_size(
            f32_coord(world_bounds.width()),
            f32_coord(world_bounds.height()),
        )),
    );
    let svg_root = ui.append(content, (*tree).clone());
    let path_elements: Rc<[ElementId]> = Rc::from(
        ui.get(svg_root)
            .map(|element| element.children().to_vec())
            .unwrap_or_default()
            .into_boxed_slice(),
    );
    let selection = ui.append(
        host,
        Panel::new().layout(Layout::new().absolute_at(0.0, 0.0).logical_size(0.0, 0.0)),
    );
    ui.set_local(
        selection,
        ui.properties().border_brush,
        opt_solid(0x6f, 0x8f, 0xff, 0xff),
    );
    ui.set_local(selection, ui.properties().border_width, 1.5_f64);
    ui.set_local(selection, ui.properties().corner_radius, 4.0_f64);

    let status = ui.append(parent, caption(&state.status()));
    let viewer = SvgViewer {
        tree,
        host,
        grid,
        content,
        path_elements,
        selection,
        status,
        runtime: Rc::new(SvgViewerRuntime::new(state)),
    };
    install_svg_viewer_interaction(ui, &viewer);
    refresh_svg_viewer(ui, &viewer);
    Ok(viewer)
}

fn install_svg_viewer_interaction(ui: &mut Ui, viewer: &SvgViewer) {
    let pick_viewer = viewer.clone();
    ui.on_pointer(
        viewer.host,
        overstory::PointerEventKind::Down,
        move |ui, event| {
            if !matches!(
                event.phase,
                overstory::Phase::Capture | overstory::Phase::Target
            ) {
                return;
            }
            let Some(position) = event.position else {
                return;
            };
            let view_point = local_point(ui, pick_viewer.host, position);
            select_svg_path_at_view_point(ui, &pick_viewer, view_point);
        },
    );

    let drag_viewer = viewer.clone();
    ui.on_drag(viewer.host, move |ui, event| {
        if !matches!(
            event.phase,
            overstory::Phase::Capture | overstory::Phase::Target
        ) || event.delta.hypot() <= 0.0
        {
            return;
        }
        let _ = ui.capture_pointer(drag_viewer.host);
        drag_viewer
            .runtime
            .state
            .borrow_mut()
            .pan_by_view(event.delta);
        refresh_svg_viewer(ui, &drag_viewer);
        event.mark_handled();
        event.stop_propagation();
    });

    let scroll_viewer = viewer.clone();
    ui.on_scroll(viewer.host, move |ui, event| {
        if !matches!(
            event.phase,
            overstory::Phase::Capture | overstory::Phase::Target
        ) {
            return;
        }
        let delta = event.logical_delta(Vec2::new(48.0, 48.0), Vec2::new(260.0, 260.0));
        let amount = if delta.y.abs() >= delta.x.abs() {
            delta.y
        } else {
            delta.x
        };
        if amount.abs() <= 0.01 {
            return;
        }
        let anchor = local_point(ui, scroll_viewer.host, event.position);
        let factor = (-amount * SVG_ZOOM_SENSITIVITY).exp().clamp(0.2, 5.0);
        scroll_viewer
            .runtime
            .state
            .borrow_mut()
            .zoom_about(anchor, factor);
        refresh_svg_viewer(ui, &scroll_viewer);
        event.mark_handled();
        event.stop_propagation();
    });
}

fn select_svg_path_at_view_point(ui: &mut Ui, viewer: &SvgViewer, view_point: Point) {
    let (previous, selected) = {
        let mut state = viewer.runtime.state.borrow_mut();
        let previous = state.selected_path;
        let world_point = state.viewport.view_to_world_point(view_point);
        let params = svg_hit_params();
        let selected = viewer
            .tree
            .hit_test_path(world_point, &params)
            .map(|hit| hit.path_index());
        state.select_path(selected);
        (previous, selected)
    };
    if previous != selected {
        reset_svg_path_wobble(ui, viewer, previous);
    }
    if let Some(path_index) = selected {
        start_svg_path_wobble(ui, viewer, path_index);
    }
    refresh_svg_viewer(ui, viewer);
}

fn start_svg_path_wobble(ui: &mut Ui, viewer: &SvgViewer, path_index: usize) {
    let Some(element) = viewer.path_elements.get(path_index).copied() else {
        return;
    };
    let Some(path) = viewer.tree.path(path_index) else {
        return;
    };
    cancel_svg_wobble_timers(ui, viewer);
    let generation = viewer.runtime.next_wobble_generation();
    let path_bounds = path.bounds();
    let center = local_svg_path_center(path_bounds);
    let selection = viewer.selection;
    let selection_center = local_svg_path_center(
        viewer
            .runtime
            .state
            .borrow()
            .viewport
            .world_to_view_rect(path_bounds),
    );
    set_svg_selection_wobble(ui, element, center, selection, selection_center, 1.1);
    let first_timer = ui.schedule_timer_once(element, millis(SVG_SELECTION_WOBBLE_UP_MS));
    viewer.runtime.wobble_timers.borrow_mut().push(first_timer);

    let runtime = Rc::clone(&viewer.runtime);
    let phase = Cell::new(0_u8);
    ui.on_timer(element, move |ui, _event| {
        if runtime.wobble_generation.get() != generation {
            ui.remove_timer_handler(element);
            return;
        }
        match phase.get() {
            0 => {
                phase.set(1);
                set_svg_selection_wobble(ui, element, center, selection, selection_center, 0.985);
                runtime.wobble_timers.borrow_mut().clear();
                let second_timer =
                    ui.schedule_timer_once(element, millis(SVG_SELECTION_WOBBLE_DOWN_MS));
                runtime.wobble_timers.borrow_mut().push(second_timer);
            }
            _ => {
                runtime.wobble_timers.borrow_mut().clear();
                ui.set_visual_transform(element, Affine::IDENTITY);
                ui.set_visual_transform(selection, Affine::IDENTITY);
                ui.remove_timer_handler(element);
            }
        }
    });
}

fn reset_svg_path_wobble(ui: &mut Ui, viewer: &SvgViewer, path_index: Option<usize>) {
    cancel_svg_wobble_timers(ui, viewer);
    viewer.runtime.next_wobble_generation();
    ui.set_visual_transform(viewer.selection, Affine::IDENTITY);
    let Some(element) = path_index.and_then(|index| viewer.path_elements.get(index).copied())
    else {
        return;
    };
    ui.set_visual_transform(element, Affine::IDENTITY);
    ui.remove_timer_handler(element);
}

fn set_svg_selection_wobble(
    ui: &mut Ui,
    path: ElementId,
    path_center: Point,
    selection: ElementId,
    selection_center: Point,
    scale: f64,
) {
    ui.set_visual_transform(path, scale_about(path_center, scale));
    ui.set_visual_transform(selection, scale_about(selection_center, scale));
}

fn cancel_svg_wobble_timers(ui: &mut Ui, viewer: &SvgViewer) {
    for timer in viewer.runtime.wobble_timers.borrow_mut().drain(..) {
        let _ = ui.cancel_timer(timer);
    }
}

fn local_svg_path_center(bounds: Rect) -> Point {
    Point::new(bounds.width() * 0.5, bounds.height() * 0.5)
}

fn scale_about(center: Point, scale: f64) -> Affine {
    Affine::translate((center.x, center.y))
        * Affine::scale(scale)
        * Affine::translate((-center.x, -center.y))
}

fn svg_hit_params() -> HitParams {
    // Keep fill tolerance at zero until path tolerance is true near-edge
    // geometry rather than a bounding-box fallback.
    HitParams {
        fill_tolerance: 0.0,
        stroke_tolerance: 0.0,
        prefer_fill: true,
    }
}

fn append_svg_view_button(
    ui: &mut Ui,
    parent: ElementId,
    label: &str,
    viewer: SvgViewer,
    action: impl Fn(&mut SvgViewportDemo) + 'static,
) {
    let button = ui.append(
        parent,
        Button::new(label)
            .font_size(13.0)
            .on_click(move |ui, event| {
                if !matches!(
                    event.phase,
                    overstory::Phase::Target | overstory::Phase::Bubble
                ) {
                    return;
                }
                action(&mut viewer.runtime.state.borrow_mut());
                refresh_svg_viewer(ui, &viewer);
                event.mark_handled();
            }),
    );
    ui.set_tooltip(button, Tooltip::new("Adjust the SVG viewport"));
}

fn refresh_svg_viewer(ui: &mut Ui, viewer: &SvgViewer) {
    let state = viewer.runtime.state.borrow();
    clear_children(ui, viewer.grid);
    append_svg_grid(ui, viewer.grid, &state);
    ui.set_visual_transform(viewer.content, viewport_transform(&state.viewport));
    update_svg_selection_overlay(ui, viewer, &state);
    ui.set_local(viewer.status, ui.properties().content, state.status());
    if let Some(inspector) = viewer.runtime.inspector.get() {
        update_svg_inspector(ui, inspector, &state, &viewer.tree);
    }
}

fn update_svg_selection_overlay(ui: &mut Ui, viewer: &SvgViewer, state: &SvgViewportDemo) {
    let view_bounds = state
        .selected_path
        .and_then(|index| viewer.tree.path(index))
        .map(|path| state.viewport.world_to_view_rect(path.bounds()))
        .unwrap_or(Rect::ZERO);
    ui.set_layout(
        viewer.selection,
        Layout::new()
            .absolute_at(f32_coord(view_bounds.x0), f32_coord(view_bounds.y0))
            .logical_size(
                f32_coord(view_bounds.width().max(0.0)),
                f32_coord(view_bounds.height().max(0.0)),
            ),
    );
}

fn viewport_transform(viewport: &Viewport2D) -> Affine {
    let origin = viewport.world_to_view_point(Point::ZERO);
    let x = viewport.world_to_view_point(Point::new(1.0, 0.0));
    let y = viewport.world_to_view_point(Point::new(0.0, 1.0));
    Affine::new([
        x.x - origin.x,
        x.y - origin.y,
        y.x - origin.x,
        y.y - origin.y,
        origin.x,
        origin.y,
    ])
}

fn append_svg_grid(ui: &mut Ui, parent: ElementId, state: &SvgViewportDemo) {
    let viewport = &state.viewport;
    let visible = viewport.visible_world_rect();
    let spacing = viewport.suggest_grid_spacing(12.0).max(1.0);
    append_svg_grid_axis(ui, parent, viewport, visible.x0, visible.x1, spacing, true);
    append_svg_grid_axis(ui, parent, viewport, visible.y0, visible.y1, spacing, false);
}

fn append_svg_grid_axis(
    ui: &mut Ui,
    parent: ElementId,
    viewport: &Viewport2D,
    start: f64,
    end: f64,
    spacing: f64,
    vertical: bool,
) {
    let view = viewport.view_rect();
    let mut world = (start / spacing).floor() * spacing;
    let mut count = 0;
    while world <= end && count < SVG_GRID_LIMIT {
        let index = (world / spacing).round();
        let major = ((index / 5.0) - (index / 5.0).round()).abs() <= 0.001;
        let color = if major {
            Color::from_rgba8(0x40, 0x42, 0x50, 0x86)
        } else {
            Color::from_rgba8(0x2a, 0x2b, 0x35, 0x72)
        };
        let coordinate = if vertical {
            viewport.world_to_view_point(Point::new(world, 0.0)).x
        } else {
            viewport.world_to_view_point(Point::new(0.0, world)).y
        };
        let rect = if vertical {
            Rect::new(coordinate, view.y0, coordinate + 1.0, view.y1)
        } else {
            Rect::new(view.x0, coordinate, view.x1, coordinate + 1.0)
        };
        append_svg_grid_rect(ui, parent, rect, color);
        world += spacing;
        count += 1;
    }
}

fn append_svg_grid_rect(ui: &mut Ui, parent: ElementId, rect: Rect, color: Color) -> ElementId {
    let element = ui.append(
        parent,
        Panel::new().class(CLASS_SVG_GRID_LINE).layout(
            Layout::new()
                .absolute_at(f32_coord(rect.x0), f32_coord(rect.y0))
                .logical_size(f32_coord(rect.width()), f32_coord(rect.height())),
        ),
    );
    ui.set_local(element, ui.properties().background, Some(color.into()));
    element
}

fn padded_svg_bounds(bounds: Rect) -> Rect {
    let pad_x = (bounds.width() * 0.08).max(8.0);
    let pad_y = (bounds.height() * 0.08).max(8.0);
    bounds.inflate(pad_x, pad_y)
}

fn local_point(ui: &Ui, parent: ElementId, position: Point) -> Point {
    let bounds = ui.effective_bounds(parent);
    Point::new(position.x - bounds.x0, position.y - bounds.y0)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "demo logical coordinates fit f32 layout values"
)]
fn f32_coord(value: f64) -> f32 {
    value as f32
}

fn build_typography_page(ui: &mut Ui, stage: ElementId) {
    let row = demo_row(ui, stage);
    let counter = demo_card(ui, row, "TABULAR NUMBERS", PAGE_CARD_WIDTH, 326.0);
    append_large_counter(
        ui,
        counter,
        "Steady suffix",
        "events / min",
        85_000_000,
        257,
    );
    ui.append(
        counter,
        body_text("The number text uses the OpenType tabular-figures feature, so its measured width stays steady as digits change."),
    );

    let text = demo_card(ui, row, "TEXT SURFACES", PAGE_CARD_WIDTH, 326.0);
    ui.append(
        text,
        Text::new("Overstory keeps simple labels cheap while still resolving inherited text style through templates and generated parts.")
            .font_size(16.0)
            .class(CLASS_DEMO_BODY)
            .wrap(),
    );
    ui.append(text, Divider::horizontal().length(PAGE_CONTROL_WIDTH));
    ui.append(
        text,
        TextInput::new()
            .text("PlainEditor-backed editing")
            .width(PAGE_CONTROL_WIDTH),
    );
    ui.append(
        text,
        TextInput::new()
            .placeholder("IME, clipboard, accessibility next")
            .width(PAGE_CONTROL_WIDTH),
    );
}

fn build_motion_page(ui: &mut Ui, stage: ElementId) {
    let row = demo_row(ui, stage);

    let color = demo_card(ui, row, "COLOR MOTION", PAGE_CARD_WIDTH, 326.0);
    let color_lanes = ui.append(
        color,
        Panel::row(12.0).layout(Layout::row(12.0).logical_size(PAGE_CONTROL_WIDTH, 128.0)),
    );
    let oklab = ui.append(
        color_lanes,
        Panel::new()
            .class(CLASS_OKLAB_SWATCH)
            .layout(Layout::new().logical_size(158.0, 128.0)),
    );
    let srgb = ui.append(
        color_lanes,
        Panel::new()
            .class(CLASS_SRGB_SWATCH)
            .layout(Layout::new().logical_size(158.0, 128.0)),
    );
    let labels = ui.append(
        color,
        Panel::row(12.0).layout(Layout::row(12.0).logical_size(PAGE_CONTROL_WIDTH, 16.0)),
    );
    motion_lane_label(ui, labels, "OKLab");
    motion_lane_label(ui, labels, "sRGB");
    ui.append(
        color,
        body_text("Both lanes share endpoints. The middle of the journey exposes the interpolation policy."),
    );
    schedule_pair_toggle(ui, [oklab, srgb], PSEUDO_SELECTED, millis(5_200));

    let composition = demo_card(ui, row, "COMPOSITION MOTION", PAGE_CARD_WIDTH, 326.0);
    let strip = ui.append(
        composition,
        Panel::row(12.0).layout(Layout::row(12.0).logical_size(PAGE_CONTROL_WIDTH, 74.0)),
    );
    let opacity = motion_tile(ui, strip, CLASS_OPACITY_SWATCH);
    let border = motion_tile(ui, strip, CLASS_BORDER_SWATCH);
    let retarget = motion_tile(ui, strip, CLASS_RETARGET_SWATCH);
    ui.set_tooltip(
        retarget,
        Tooltip::new("Retargeting starts a new transition from the current sampled value"),
    );

    let motion_stage = ui.append(
        composition,
        Panel::new().class(CLASS_DEMO_MOTION_STAGE).layout(
            Layout::new()
                .relative()
                .logical_size(PAGE_CONTROL_WIDTH, 102.0)
                .padding_all(0.0),
        ),
    );
    let mover = ui.append(
        motion_stage,
        Panel::new()
            .class(CLASS_MOVER_SWATCH)
            .layout(mover_layout(false)),
    );
    ui.append(
        composition,
        caption("Opacity, border, retargeting, and layer offset all sample presentation values.")
            .wrap(),
    );

    schedule_toggle(ui, opacity, PSEUDO_PRESSED, millis(1_100));
    schedule_toggle(ui, border, PSEUDO_FOCUS_VISIBLE, millis(1_450));
    schedule_toggle(ui, retarget, PSEUDO_SELECTED, millis(180));
    schedule_movement(ui, mover, millis(1_250));
}

fn motion_tile(ui: &mut Ui, parent: ElementId, class: ClassId) -> ElementId {
    ui.append(
        parent,
        Panel::new()
            .class(class)
            .layout(Layout::new().logical_size(96.0, 74.0)),
    )
}

fn motion_lane_label(ui: &mut Ui, parent: ElementId, text: &str) -> ElementId {
    let id = ui.append(parent, Text::new(text).class(CLASS_DEMO_CARD_TITLE));
    ui.set_layout(id, Layout::new().logical_size(158.0, 16.0));
    id
}

fn drive_progress(ui: &mut Ui, progress: ElementId, initial: f64, step: f64, interval: u64) {
    let _ = ui.schedule_timer_repeating(progress, interval);
    let mut progress_value = initial;
    ui.on_timer(progress, move |ui, _event| {
        progress_value += step;
        if progress_value > 1.0 {
            progress_value = 0.08;
        }
        set_progress_value(ui, progress, progress_value);
    });
}

fn append_tabular_counter(
    ui: &mut Ui,
    parent: ElementId,
    label: &str,
    suffix: &str,
    interval: u64,
    step: u32,
) -> ElementId {
    ui.append(parent, caption(label));
    let row = ui.append(parent, Panel::row(8.0).align_center());
    let counter = ui.append(row, number_text("0000"));
    ui.set_local(
        counter,
        ui.properties().font_features,
        tabular_number_features(),
    );
    ui.append(row, caption(suffix));
    ui.set_tooltip(
        counter,
        Tooltip::new("Tabular figures keep neighboring text steady"),
    );
    drive_counter(ui, counter, interval, step);
    counter
}

fn append_large_counter(
    ui: &mut Ui,
    parent: ElementId,
    label: &str,
    suffix: &str,
    interval: u64,
    step: u32,
) -> ElementId {
    ui.append(parent, caption(label));
    let row = ui.append(parent, Panel::row(12.0).align_center());
    let counter = ui.append(row, large_number_text("0000"));
    ui.set_local(
        counter,
        ui.properties().font_features,
        tabular_number_features(),
    );
    ui.append(row, caption(suffix));
    ui.set_tooltip(
        counter,
        Tooltip::new("Tabular figures keep neighboring text steady"),
    );
    drive_counter(ui, counter, interval, step);
    counter
}

fn drive_counter(ui: &mut Ui, counter: ElementId, interval: u64, step: u32) {
    let _ = ui.schedule_timer_repeating(counter, interval);
    let mut value = 0_u32;
    ui.on_timer(counter, move |ui, _event| {
        value = (value + step) % 10_000;
        ui.set_local(counter, ui.properties().content, format!("{value:04}"));
    });
}

fn schedule_toggle(ui: &mut Ui, target: ElementId, pseudo: PseudoClassId, interval: TimerDuration) {
    let active = Cell::new(false);
    let _ = ui.schedule_timer_repeating(target, interval);
    ui.on_timer(target, move |ui, _event| {
        let next = !active.get();
        active.set(next);
        set_pseudo(ui, target, pseudo, next);
    });
}

fn schedule_pair_toggle(
    ui: &mut Ui,
    targets: [ElementId; 2],
    pseudo: PseudoClassId,
    interval: TimerDuration,
) {
    let active = Cell::new(false);
    let _ = ui.schedule_timer_repeating(targets[0], interval);
    ui.on_timer(targets[0], move |ui, _event| {
        let next = !active.get();
        active.set(next);
        for target in targets {
            set_pseudo(ui, target, pseudo, next);
        }
    });
}

fn schedule_movement(ui: &mut Ui, target: ElementId, interval: TimerDuration) {
    let active = Cell::new(false);
    let _ = ui.schedule_timer_repeating(target, interval);
    ui.on_timer(target, move |ui, _event| {
        let next = !active.get();
        active.set(next);
        let _ = ui.request_layer_offset_transition(target);
        ui.set_layout(target, mover_layout(next));
    });
}

fn mover_layout(active: bool) -> Layout {
    let x = if active { 270.0 } else { 16.0 };
    Layout::new().absolute_at(x, 30.0).logical_size(42.0, 42.0)
}

fn set_pseudo(ui: &mut Ui, target: ElementId, pseudo: PseudoClassId, active: bool) {
    if active {
        ui.add_pseudo(target, pseudo);
    } else {
        ui.remove_pseudo(target, pseudo);
    }
}

fn selection_summary(event: &overstory::ListSelectionChangedEvent) -> String {
    if event.selected_indices.is_empty() {
        return "Selected: none".to_owned();
    }
    let count = event.selected_indices.len();
    let primary = event
        .primary_index
        .map(renderer_queue_label)
        .unwrap_or_else(|| "unknown".to_owned());
    if count == 1 {
        format!("Selected: {primary}")
    } else {
        format!("Selected: {count} items, primary {primary}")
    }
}

fn class_selector(class: ClassId) -> Selector {
    Selector::single(SelectorStep::type_tag(TYPE_GENERIC).with_class(class))
}

fn text_class_selector(class: ClassId) -> Selector {
    Selector::single(SelectorStep::type_tag(TYPE_TEXT).with_class(class))
}

fn class_state_selector(class: ClassId, pseudo: PseudoClassId) -> Selector {
    Selector::single(
        SelectorStep::type_tag(TYPE_GENERIC)
            .with_class(class)
            .with_pseudo(pseudo),
    )
}

fn selected_nav_label_selector() -> Selector {
    Selector::from([
        SelectorStep::type_tag(TYPE_GENERIC)
            .with_class(CLASS_DEMO_NAV_ROW)
            .with_pseudo(PSEUDO_SELECTED),
        SelectorStep::type_tag(TYPE_TEXT).with_class(CLASS_DEMO_NAV_LABEL),
    ])
}

fn color_transition_rules(
    properties: &BuiltInProperties,
    duration: TimerDuration,
    timing: TimingFunction,
    interpolation: ColorInterpolation,
) -> TransitionRules {
    TransitionRules::single(TransitionRule::interaction(
        TransitionTarget::property(properties.background.id()),
        TransitionSpec::timed(duration, timing).with_color_interpolation(interpolation),
    ))
}

fn border_transition_rules(properties: &BuiltInProperties) -> TransitionRules {
    TransitionRules::new()
        .with_rule(TransitionRule::interaction(
            TransitionTarget::property(properties.border_brush.id()),
            TransitionSpec::timed(millis(720), EASE_OUT)
                .with_color_interpolation(ColorInterpolation::OKLAB),
        ))
        .with_rule(TransitionRule::interaction(
            TransitionTarget::property(properties.border_width.id()),
            TransitionSpec::timed(millis(520), EASE_IN_OUT),
        ))
}

fn scalar_transition_rules(target: TransitionTarget, spec: TransitionSpec) -> TransitionRules {
    TransitionRules::single(TransitionRule::interaction(target, spec))
}

#[cfg(test)]
mod tests {
    use kurbo::{Point, Size};

    use super::*;

    fn assert_close(actual: f64, expected: f64, label: &str) {
        assert!(
            (actual - expected).abs() < 0.01,
            "{label}: expected {expected}, got {actual}",
        );
    }

    #[test]
    fn gallery_shell_fills_window_content_and_nav_accent() {
        let mut ui = build_ui();
        ui.compute_layout(Size::new(
            f64::from(INITIAL_SIZE.0),
            f64::from(INITIAL_SIZE.1),
        ));

        let root = ui.root();
        let root_children = ui.get(root).expect("root should exist").children();
        let [nav, stage] = *root_children else {
            panic!("gallery root should contain nav and stage");
        };

        let nav_bounds = ui.effective_bounds(nav);
        let stage_bounds = ui.effective_bounds(stage);
        let expected_content_height = f64::from(INITIAL_SIZE.1) - (2.0 * f64::from(ROOT_PADDING));
        assert_close(nav_bounds.height(), expected_content_height, "nav height");
        assert_close(
            stage_bounds.height(),
            expected_content_height,
            "stage height",
        );

        let nav_children = ui.get(nav).expect("nav should exist").children();
        let marker = nav_children
            .get(2)
            .copied()
            .expect("nav marker should be third child");
        assert_close(
            ui.effective_bounds(marker).width(),
            f64::from(NAV_CONTENT_WIDTH),
            "nav marker width",
        );
    }

    #[test]
    fn gallery_nav_uses_shell_rows_instead_of_a_list_box() {
        let mut ui = build_ui();
        let nav_rows = nav_rows(&ui).to_vec();

        assert_eq!(nav_rows.len(), DemoPage::ALL.len());
        for row in &nav_rows {
            assert_ne!(
                ui.get(*row).and_then(|element| element.type_tag()),
                Some(overstory::TYPE_LIST_BOX),
                "navigation should not render as the listbox control demo",
            );
        }

        let systems_row = nav_rows[2];
        let point = nav_row_activation_point(&mut ui, systems_row);
        let down = ui.dispatch_pointer_event(&pointer_down(point, 1));
        assert!(down.changed(), "pointer down should focus the nav row");
        assert_eq!(
            ui.focused(),
            Some(systems_row),
            "pointer activation should move focus to the nav row",
        );
        assert!(
            !ui.get(systems_row)
                .expect("systems nav row should exist")
                .pseudos()
                .contains(&PSEUDO_FOCUS_VISIBLE),
            "pointer focus should not show keyboard-visible focus styling",
        );
        let up = ui.dispatch_pointer_event(&pointer_up(point, 2));
        assert!(up.handled, "pointer up should dispatch handled nav click");

        assert_first_page_card_height(&mut ui, 322.0, "nav row click should switch page content");
    }

    #[test]
    fn gallery_nav_supports_focus_and_keyboard_controls() {
        use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

        let mut ui = build_ui();
        let nav_rows = nav_rows(&ui).to_vec();
        for row in &nav_rows {
            assert!(ui.is_focusable(*row), "nav rows should be focusable");
        }

        assert!(ui.set_focus(Some(nav_rows[1])));
        assert!(
            ui.get(nav_rows[1])
                .expect("focused nav row should exist")
                .pseudos()
                .contains(&PSEUDO_FOCUS_VISIBLE),
            "keyboard focus should show focus-visible styling",
        );
        let enter = KeyboardEvent::key_down(Key::Named(NamedKey::Enter), Code::Enter);
        let result = ui.dispatch_key(&enter);
        assert!(result.handled, "Enter should activate the focused nav row");
        assert_first_page_card_height(&mut ui, 224.0, "Enter should switch to Inputs");

        let down = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowDown), Code::ArrowDown);
        let result = ui.dispatch_key(&down);
        assert!(result.handled, "ArrowDown should move and activate nav");
        assert_eq!(ui.focused(), Some(nav_rows[2]));
        assert_first_page_card_height(&mut ui, 322.0, "ArrowDown should switch to Systems");

        let home = KeyboardEvent::key_down(Key::Named(NamedKey::Home), Code::Home);
        let result = ui.dispatch_key(&home);
        assert!(result.handled, "Home should move to the first nav row");
        assert_eq!(ui.focused(), Some(nav_rows[0]));
        assert_first_page_card_height(&mut ui, 222.0, "Home should switch to Overview");
    }

    #[test]
    fn gallery_theme_toggle_switches_control_theme() {
        let mut ui = build_ui();
        let toggle = mode_toggle(&ui);

        let result = ui.synthesize_click(toggle, Point::ZERO);
        assert!(result.handled, "theme toggle should retain toggle behavior");
        assert!(
            ui.get(toggle)
                .expect("theme toggle should exist")
                .pseudos()
                .contains(&overstory::pseudos::PSEUDO_CHECKED),
            "theme toggle should be checked in light mode",
        );
        assert_eq!(
            ui.theme()
                .get::<Brush>(theme::ON_SURFACE_FG)
                .expect("light theme should provide foreground"),
            &Brush::from(Color::from_rgba8(0x16, 0x1a, 0x24, 0xff)),
            "theme toggle should swap the active control palette",
        );
    }

    #[test]
    fn renderer_queue_demo_uses_large_generated_data_set() {
        let items = renderer_queue_items().collect::<Vec<_>>();
        assert_eq!(items.len(), RENDERER_QUEUE_LEN);
        assert_eq!(
            items.first().map(String::as_str),
            Some("Vello scene graph 001")
        );
        assert_eq!(
            items.get(INITIAL_RENDERER_SELECTION).map(String::as_str),
            Some("Text shaping pass 001")
        );
        assert_eq!(
            items.last().map(String::as_str),
            Some("Accessibility tree 084")
        );
    }

    #[test]
    fn motion_page_mover_timer_updates_layout_target() {
        let mut ui = build_ui();
        let stage = stage_element(&ui);
        show_demo_page(&mut ui, stage, DemoPage::Motion);
        ui.compute_layout(Size::new(
            f64::from(INITIAL_SIZE.0),
            f64::from(INITIAL_SIZE.1),
        ));

        let mover = find_by_class(&ui, ui.root(), CLASS_MOVER_SWATCH)
            .expect("motion page should contain the offset specimen");
        let start = ui.effective_bounds(mover);

        ui.pump_timers(millis(1_250));
        ui.compute_layout(Size::new(
            f64::from(INITIAL_SIZE.0),
            f64::from(INITIAL_SIZE.1),
        ));

        let after = ui.effective_bounds(mover);
        assert!(
            after.x0 > start.x0 + 100.0,
            "mover target should advance after its timer fires: {start:?} -> {after:?}",
        );
    }

    #[test]
    fn layout_page_contains_imported_svg_vector_paths() {
        let mut ui = build_ui();
        let stage = stage_element(&ui);
        show_demo_page(&mut ui, stage, DemoPage::Layout);

        assert_eq!(count_type(&ui, ui.root(), overstory::TYPE_VECTOR_PATH), 5);
    }

    #[test]
    fn tiger_svg_asset_stresses_path_importer() {
        let svg = SvgElementTree::from_bytes(TIGER_SVG, SvgImportOptions::default())
            .expect("Tiger SVG asset should parse");

        assert!(
            svg.len() > 100,
            "Tiger SVG should stress the path importer, got {} paths",
            svg.len(),
        );
    }

    #[test]
    fn svg_viewer_zoom_updates_content_transform_without_rebuilding_paths() {
        let svg = br##"
            <svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
              <rect x="25" y="25" width="50" height="50" fill="#ff8a00"/>
            </svg>
        "##;
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(360.0, 280.0));
        let viewer = append_svg_viewer(&mut ui, root, svg).expect("test SVG should import");
        let inspector = append_svg_inspector(&mut ui, root, &viewer);
        viewer.runtime.inspector.set(Some(inspector));
        refresh_svg_viewer(&mut ui, &viewer);
        ui.compute_layout(Size::new(360.0, 280.0));

        let before_count = count_type(&ui, viewer.host, overstory::TYPE_VECTOR_PATH);
        let before_bounds = ui.effective_bounds(viewer.content);
        let before_scale = transform_scale(ui.effective_visual_transform(viewer.content));
        let before_grid_thickness =
            first_grid_line_thickness(&ui, viewer.grid).expect("viewer should contain grid lines");

        viewer.runtime.state.borrow_mut().zoom_center(2.0);
        refresh_svg_viewer(&mut ui, &viewer);
        ui.compute_layout(Size::new(360.0, 280.0));

        let after_count = count_type(&ui, viewer.host, overstory::TYPE_VECTOR_PATH);
        let after_bounds = ui.effective_bounds(viewer.content);
        let after_scale = transform_scale(ui.effective_visual_transform(viewer.content));
        let after_grid_thickness =
            first_grid_line_thickness(&ui, viewer.grid).expect("viewer should contain grid lines");
        assert_eq!(after_count, before_count);
        assert_eq!(after_bounds, before_bounds);
        assert_close(before_grid_thickness, 1.0, "grid starts at 1px");
        assert_close(after_grid_thickness, 1.0, "grid stays at 1px after zoom");
        assert!(
            after_scale > before_scale * 1.5,
            "zoom should enlarge the content presentation transform: {before_scale} -> {after_scale}",
        );
    }

    #[test]
    fn svg_viewer_precisely_picks_path_content() {
        let svg = br##"
            <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20">
              <path id="triangle" d="M0 0 L12 0 L0 12 Z" fill="#00ff00"/>
            </svg>
        "##;
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(360.0, 300.0));
        let viewer = append_svg_viewer(&mut ui, root, svg).expect("test SVG should import");
        let inspector = append_svg_inspector(&mut ui, root, &viewer);
        viewer.runtime.inspector.set(Some(inspector));
        refresh_svg_viewer(&mut ui, &viewer);
        ui.compute_layout(Size::new(360.0, 300.0));

        let hit = viewer
            .runtime
            .state
            .borrow()
            .viewport
            .world_to_view_point(Point::new(2.0, 2.0));
        select_svg_path_at_view_point(&mut ui, &viewer, hit);
        ui.compute_layout(Size::new(360.0, 300.0));
        assert_eq!(viewer.runtime.state.borrow().selected_path, Some(0));
        let selected_path = viewer.path_elements[0];
        assert!(
            transform_scale(ui.effective_visual_transform(selected_path)) > 1.05,
            "selecting a path should start a direct path wobble",
        );
        assert!(
            transform_scale(ui.effective_visual_transform(viewer.selection)) > 1.05,
            "selection overlay should follow the path wobble scale",
        );
        ui.pump_timers(millis(SVG_SELECTION_WOBBLE_UP_MS));
        assert!(
            transform_scale(ui.effective_visual_transform(selected_path)) < 1.0,
            "path wobble should overshoot back below the final scale",
        );
        assert!(
            transform_scale(ui.effective_visual_transform(viewer.selection)) < 1.0,
            "selection overlay should follow the wobble overshoot",
        );
        ui.pump_timers(millis(
            SVG_SELECTION_WOBBLE_UP_MS + SVG_SELECTION_WOBBLE_DOWN_MS,
        ));
        assert_close(
            transform_scale(ui.effective_visual_transform(selected_path)),
            1.0,
            "path wobble returns to identity scale",
        );
        assert_close(
            transform_scale(ui.effective_visual_transform(viewer.selection)),
            1.0,
            "selection overlay wobble returns to identity scale",
        );
        assert!(
            ui.effective_bounds(viewer.selection).width() > 0.0,
            "selection overlay should track the selected path bounds",
        );

        let miss_inside_bounds = viewer
            .runtime
            .state
            .borrow()
            .viewport
            .world_to_view_point(Point::new(10.0, 10.0));
        select_svg_path_at_view_point(&mut ui, &viewer, miss_inside_bounds);
        ui.compute_layout(Size::new(360.0, 300.0));
        assert_eq!(viewer.runtime.state.borrow().selected_path, None);
        assert_close(
            ui.effective_bounds(viewer.selection).width(),
            0.0,
            "selection overlay clears after a precise miss",
        );
    }

    #[test]
    fn svg_viewer_selection_pointer_down_still_starts_pan_drag() {
        let svg = br##"
            <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20">
              <path id="triangle" d="M0 0 L12 0 L0 12 Z" fill="#00ff00"/>
            </svg>
        "##;
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(360.0, 300.0));
        let viewer = append_svg_viewer(&mut ui, root, svg).expect("test SVG should import");
        ui.compute_layout(Size::new(360.0, 300.0));

        let local_hit = viewer
            .runtime
            .state
            .borrow()
            .viewport
            .world_to_view_point(Point::new(2.0, 2.0));
        let host_bounds = ui.effective_bounds(viewer.host);
        let hit = Point::new(host_bounds.x0 + local_hit.x, host_bounds.y0 + local_hit.y);
        let visible_before = viewer.runtime.state.borrow().viewport.visible_world_rect();

        let down = ui.dispatch_pointer_event(&pointer_down(hit, 1));
        assert!(
            down.capture_changed,
            "selection pointer down should still start pointer capture for drag"
        );
        assert_eq!(viewer.runtime.state.borrow().selected_path, Some(0));

        let moved = ui.dispatch_pointer_event(&pointer_move(
            Point::new(hit.x + 28.0, hit.y + 12.0),
            2,
            true,
        ));
        let visible_after = viewer.runtime.state.borrow().viewport.visible_world_rect();
        assert!(
            moved.handled,
            "pan drag should be handled by the SVG viewport"
        );
        assert_ne!(
            visible_after, visible_before,
            "dragging after selection should pan the SVG viewport",
        );
    }

    #[test]
    fn svg_page_puts_import_summary_above_large_tiger_and_inspector() {
        let mut ui = build_ui();
        let stage = stage_element(&ui);
        show_demo_page(&mut ui, stage, DemoPage::Svg);
        ui.compute_layout(Size::new(
            f64::from(INITIAL_SIZE.0),
            f64::from(INITIAL_SIZE.1),
        ));

        let info = find_by_class(&ui, stage, CLASS_SVG_INFO_CARD)
            .expect("SVG page should contain an import summary card");
        let tiger = find_by_class(&ui, stage, CLASS_SVG_TIGER_CARD)
            .expect("SVG page should contain the Tiger card");
        let inspector = find_by_class(&ui, stage, CLASS_SVG_INSPECTOR_CARD)
            .expect("SVG page should contain an inspector card");
        let info_bounds = ui.effective_bounds(info);
        let tiger_bounds = ui.effective_bounds(tiger);
        let inspector_bounds = ui.effective_bounds(inspector);

        assert!(
            info_bounds.y1 <= tiger_bounds.y0 + 0.01,
            "import summary should sit above Tiger: {info_bounds:?} then {tiger_bounds:?}",
        );
        assert!(
            tiger_bounds.width() > inspector_bounds.width() * 1.8,
            "Tiger should get most horizontal space: {tiger_bounds:?} vs {inspector_bounds:?}",
        );
        assert!(
            tiger_bounds.height() > info_bounds.height() * 3.0,
            "Tiger card should dominate vertical space: {tiger_bounds:?} vs {info_bounds:?}",
        );
    }

    #[test]
    fn gallery_pages_stay_inside_initial_window_width() {
        for page in DemoPage::ALL {
            let mut ui = build_ui();
            let stage = stage_element(&ui);
            show_demo_page(&mut ui, stage, page);
            ui.compute_layout(Size::new(
                f64::from(INITIAL_SIZE.0),
                f64::from(INITIAL_SIZE.1),
            ));

            assert_subtree_inside_width(&ui, ui.root(), f64::from(INITIAL_SIZE.0));
        }
    }

    #[test]
    fn gallery_pages_stay_inside_narrow_window_width() {
        const NARROW_WIDTH: f64 = 1180.0;

        for page in DemoPage::ALL {
            let mut ui = build_ui();
            let stage = stage_element(&ui);
            show_demo_page(&mut ui, stage, page);
            ui.compute_layout(Size::new(NARROW_WIDTH, f64::from(INITIAL_SIZE.1)));

            assert_subtree_inside_width(&ui, ui.root(), NARROW_WIDTH);
        }
    }

    #[test]
    fn gallery_pages_keep_primary_cards_on_one_line_at_narrow_width() {
        const NARROW_WIDTH: f64 = 1180.0;

        for page in DemoPage::ALL {
            if page == DemoPage::Svg {
                continue;
            }
            let mut ui = build_ui();
            let stage = stage_element(&ui);
            show_demo_page(&mut ui, stage, page);
            ui.compute_layout(Size::new(NARROW_WIDTH, f64::from(INITIAL_SIZE.1)));

            let row = page_row(&ui, stage);
            let row_children = ui
                .get(row)
                .expect("page row should exist")
                .children()
                .to_vec();
            let Some(first) = row_children.first().copied() else {
                continue;
            };
            let expected_y = ui.effective_bounds(first).y0;
            for child in row_children {
                assert_close(
                    ui.effective_bounds(child).y0,
                    expected_y,
                    "page card y position",
                );
            }
        }
    }

    fn assert_subtree_inside_width(ui: &Ui, element: ElementId, max_x: f64) {
        let bounds = ui.effective_bounds(element);
        assert!(
            bounds.x1 <= max_x + 0.01,
            "{element:?} overflows window width: {bounds:?} > {max_x}",
        );
        let children = ui
            .get(element)
            .expect("element should exist")
            .children()
            .to_vec();
        for child in children {
            assert_subtree_inside_width(ui, child, max_x);
        }
    }

    fn stage_element(ui: &Ui) -> ElementId {
        let root_children = ui
            .get(ui.root())
            .expect("root should exist")
            .children()
            .to_vec();
        let [_, stage] = *root_children else {
            panic!("gallery root should contain nav and stage");
        };
        stage
    }

    fn nav_element(ui: &Ui) -> ElementId {
        let root_children = ui
            .get(ui.root())
            .expect("root should exist")
            .children()
            .to_vec();
        let [nav, _] = *root_children else {
            panic!("gallery root should contain nav and stage");
        };
        nav
    }

    fn nav_rows(ui: &Ui) -> &[ElementId] {
        let nav = nav_element(ui);
        let nav_children = ui.get(nav).expect("nav should exist").children();
        let nav_stack = nav_children
            .get(4)
            .copied()
            .expect("nav should contain a compact row stack");
        ui.get(nav_stack)
            .expect("nav stack should exist")
            .children()
    }

    fn mode_toggle(ui: &Ui) -> ElementId {
        let nav = nav_element(ui);
        let nav_children = ui.get(nav).expect("nav should exist").children();
        let mode_row = nav_children
            .get(3)
            .copied()
            .expect("nav should contain a theme mode row");
        ui.get(mode_row)
            .expect("theme mode row should exist")
            .children()
            .first()
            .copied()
            .expect("theme mode row should contain a toggle")
    }

    fn find_by_class(ui: &Ui, element: ElementId, class: ClassId) -> Option<ElementId> {
        let current = ui.get(element)?;
        if current.classes().contains(&class) {
            return Some(element);
        }
        current
            .children()
            .iter()
            .find_map(|child| find_by_class(ui, *child, class))
    }

    fn count_type(ui: &Ui, element: ElementId, type_tag: understory_style::TypeTag) -> usize {
        let Some(current) = ui.get(element) else {
            return 0;
        };
        let current_count = usize::from(current.type_tag() == Some(type_tag));
        current_count
            + current
                .children()
                .iter()
                .map(|child| count_type(ui, *child, type_tag))
                .sum::<usize>()
    }

    fn transform_scale(transform: Affine) -> f64 {
        let origin = transform * Point::ZERO;
        let unit_x = transform * Point::new(1.0, 0.0);
        (unit_x - origin).hypot()
    }

    fn first_grid_line_thickness(ui: &Ui, element: ElementId) -> Option<f64> {
        let current = ui.get(element)?;
        if current.classes().contains(&CLASS_SVG_GRID_LINE) {
            let bounds = ui.effective_bounds(element);
            return Some(bounds.width().min(bounds.height()));
        }
        current
            .children()
            .iter()
            .find_map(|child| first_grid_line_thickness(ui, *child))
    }

    fn assert_first_page_card_height(ui: &mut Ui, height: f64, label: &str) {
        let stage = stage_element(ui);
        ui.compute_layout(Size::new(
            f64::from(INITIAL_SIZE.0),
            f64::from(INITIAL_SIZE.1),
        ));
        let row = page_row(ui, stage);
        let first_card = ui
            .get(row)
            .expect("page row should exist")
            .children()
            .first()
            .copied()
            .expect("page should contain a first card");
        assert_close(ui.effective_bounds(first_card).height(), height, label);
    }

    fn nav_row_activation_point(ui: &mut Ui, row: ElementId) -> Point {
        ui.compute_layout(Size::new(
            f64::from(INITIAL_SIZE.0),
            f64::from(INITIAL_SIZE.1),
        ));
        let bounds = ui.effective_bounds(row);
        Point::new(bounds.x1 - 8.0, bounds.center().y)
    }

    fn pointer_down(point: Point, time: u64) -> ui_events::pointer::PointerEvent {
        use ui_events::pointer::{PointerButton, PointerButtonEvent, PointerEvent};

        PointerEvent::Down(PointerButtonEvent {
            pointer: pointer_info(),
            button: Some(PointerButton::Primary),
            state: pointer_state(point, time, true),
        })
    }

    fn pointer_up(point: Point, time: u64) -> ui_events::pointer::PointerEvent {
        use ui_events::pointer::{PointerButton, PointerButtonEvent, PointerEvent};

        PointerEvent::Up(PointerButtonEvent {
            pointer: pointer_info(),
            button: Some(PointerButton::Primary),
            state: pointer_state(point, time, false),
        })
    }

    fn pointer_move(
        point: Point,
        time: u64,
        primary_down: bool,
    ) -> ui_events::pointer::PointerEvent {
        use ui_events::pointer::{PointerEvent, PointerUpdate};

        PointerEvent::Move(PointerUpdate {
            pointer: pointer_info(),
            current: pointer_state(point, time, primary_down),
            coalesced: Vec::new(),
            predicted: Vec::new(),
        })
    }

    fn pointer_info() -> ui_events::pointer::PointerInfo {
        use ui_events::pointer::{PointerId, PointerInfo, PointerType};

        PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        }
    }

    fn pointer_state(
        point: Point,
        time: u64,
        primary_down: bool,
    ) -> ui_events::pointer::PointerState {
        use ui_events::pointer::{PointerButton, PointerButtons, PointerState};

        let mut buttons = PointerButtons::new();
        if primary_down {
            buttons.insert(PointerButton::Primary);
        }
        let mut state = PointerState {
            time,
            buttons,
            count: 1,
            pressure: if primary_down { 0.5 } else { 0.0 },
            ..PointerState::default()
        };
        state.position.x = point.x;
        state.position.y = point.y;
        state
    }

    fn page_row(ui: &Ui, stage: ElementId) -> ElementId {
        ui.get(stage)
            .expect("stage should exist")
            .children()
            .get(1)
            .copied()
            .expect("stage should contain header and page row")
    }
}
