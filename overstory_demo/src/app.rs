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
    Affine, BuiltInProperties, Button, CheckBox, ChildUi, CursorIcon, Divider, ElementId,
    FontFeature, Layout, LayoutAlign, LayoutAlignment, LayoutDimension, LayoutSize, ListBox,
    OutlineView, OutlineViewNode, Panel, Popup, ProgressBar, Rect, Select, Size, Slider, Spinner,
    Splitter, TYPE_GENERIC, TYPE_TEXT, Text, TextInput, TimerDuration, TimerId, ToggleSwitch,
    Tooltip, Ui, Viewport, layouts, set_progress_value, theme,
};
use overstory_devtools::{
    DevtoolsSession, DevtoolsSessionHandle, DevtoolsSnapshot, append_session_view,
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
const DEVTOOLS_INSPECTOR_WIDTH: f32 = 520.0;
const DEVTOOLS_SUMMARY_WIDTH: f32 = 250.0;
const DEVTOOLS_CARD_HEIGHT: f32 = 520.0;
const DEVTOOLS_TARGET_WIDTH: f32 = 640.0;
const DEVTOOLS_TARGET_HEIGHT: f32 = 420.0;
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
const CLASS_DEVTOOLS_INSPECTOR_CARD: ClassId = ClassId(25);
const CLASS_DEVTOOLS_SUMMARY_CARD: ClassId = ClassId(26);

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

mod pages;
#[cfg(test)]
mod tests;

pub(crate) fn run() -> overstory_host_winit::HostResult<()> {
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
    Outline,
    Layout,
    Svg,
    Typography,
    Motion,
    Devtools,
}

impl DemoPage {
    const ALL: [Self; 10] = [
        Self::Overview,
        Self::Inputs,
        Self::Systems,
        Self::Selection,
        Self::Outline,
        Self::Layout,
        Self::Svg,
        Self::Typography,
        Self::Motion,
        Self::Devtools,
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
            Self::Outline => 4,
            Self::Layout => 5,
            Self::Svg => 6,
            Self::Typography => 7,
            Self::Motion => 8,
            Self::Devtools => 9,
        }
    }

    fn nav_label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Inputs => "Inputs",
            Self::Systems => "Systems",
            Self::Selection => "Selection",
            Self::Outline => "Outline",
            Self::Layout => "Layout",
            Self::Svg => "SVG",
            Self::Typography => "Typography",
            Self::Motion => "Motion",
            Self::Devtools => "Devtools",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overstory Control Gallery",
            Self::Inputs => "Inputs and Command Surfaces",
            Self::Systems => "Controls and Values",
            Self::Selection => "Selection and Virtualization",
            Self::Outline => "Expandable Outline",
            Self::Layout => "Layout and Resizing",
            Self::Svg => "SVG Import",
            Self::Typography => "Typography and Text Features",
            Self::Motion => "Presentation Motion",
            Self::Devtools => "Overstory Devtools",
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
            Self::Outline => {
                "A virtualized tree projection backed by understory_outline with expandable rows."
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
            Self::Devtools => {
                "An inspector page built from owned UI snapshots instead of live privileged handles."
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

fn renderer_outline_nodes() -> Vec<OutlineViewNode> {
    vec![
        OutlineViewNode::new(String::from("Renderer"))
            .with_first_child(Some(1))
            .with_next_sibling(Some(5)),
        OutlineViewNode::new(String::from("Scene graph lowering")).with_next_sibling(Some(2)),
        OutlineViewNode::new(String::from("Text shaping")).with_next_sibling(Some(3)),
        OutlineViewNode::new(String::from("Layer composition")).with_next_sibling(Some(4)),
        OutlineViewNode::new(String::from("AccessKit tree")),
        OutlineViewNode::new(String::from("Interaction"))
            .with_first_child(Some(6))
            .with_next_sibling(Some(9)),
        OutlineViewNode::new(String::from("Focus routing")).with_next_sibling(Some(7)),
        OutlineViewNode::new(String::from("Selection model")).with_next_sibling(Some(8)),
        OutlineViewNode::new(String::from("Scroll target registry")),
        OutlineViewNode::new(String::from("Motion")).with_first_child(Some(10)),
        OutlineViewNode::new(String::from("Implicit transitions")).with_next_sibling(Some(11)),
        OutlineViewNode::new(String::from("Timeline cues")),
    ]
}

fn demo_card(ui: &mut Ui, parent: ElementId, title: &str, width: f32, height: f32) -> ElementId {
    with_demo_card(ui, parent, title, width, height, |_| ()).0
}

fn with_demo_card<R>(
    ui: &mut Ui,
    parent: ElementId,
    title: &str,
    width: f32,
    height: f32,
    f: impl FnOnce(&mut ChildUi<'_>) -> R,
) -> (ElementId, R) {
    ui.children(
        parent,
        Panel::column(14.0).class(CLASS_DEMO_CARD).layout(
            Layout::column(14.0)
                .logical_size(width, height)
                .padding_all(18.0),
        ),
        |card| {
            card.add(card_title(title));
            f(card)
        },
    )
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
    pages::show_demo_page(&mut ui, stage, DemoPage::Overview);

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
    let row_count = DemoPage::ALL.len() as f32;
    let stack_height = (row_count * 40.0) + ((row_count - 1.0) * 4.0);
    let stack = ui.append(
        nav,
        Panel::column(4.0)
            .layout(Layout::column(4.0).logical_size(NAV_CONTENT_WIDTH, stack_height)),
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
    pages::show_demo_page(ui, stage, page);
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
