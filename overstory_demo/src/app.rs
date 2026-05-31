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
    Accordion, AccordionChildUiExt, AccordionSection, Affine, BuiltInProperties, Button, CheckBox,
    ChildUi, ChoiceOption, CursorIcon, Divider, ElementId, ElementRef, ElementRefSpecExt, Expander,
    FontFeature, Layout, LayoutAlign, LayoutAlignment, LayoutDimension, LayoutSize, ListBox,
    OutlineView, OutlineViewNode, Panel, Popup, ProgressBar, RadioGroup, Rect, SegmentedControl,
    Select, Size, Slider, Spinner, Splitter, SurfaceSpecExt, TYPE_GENERIC, TYPE_TEXT, Text,
    TextInput, TextSpecExt, TimerDuration, TimerId, ToggleSwitch, TooltipSpecExt, Ui, View,
    Viewport, layouts, set_progress_value, theme,
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

mod chrome;
mod fixtures;
mod pages;
mod style;
#[cfg(test)]
mod tests;
mod widgets;

use chrome::*;
use fixtures::*;
use style::*;
use widgets::*;

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
    Disclosure,
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
    const ALL: [Self; 11] = [
        Self::Overview,
        Self::Inputs,
        Self::Disclosure,
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
            Self::Disclosure => 2,
            Self::Systems => 3,
            Self::Selection => 4,
            Self::Outline => 5,
            Self::Layout => 6,
            Self::Svg => 7,
            Self::Typography => 8,
            Self::Motion => 9,
            Self::Devtools => 10,
        }
    }

    fn nav_label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Inputs => "Inputs",
            Self::Disclosure => "Disclosure",
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
            Self::Disclosure => "Disclosure Containers",
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
            Self::Disclosure => {
                "Independent expanders and coordinated accordion sections share typed content-slot authoring."
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

    let (nav, nav_chrome, stage) = ui.with_children(root, |root| {
        let nav = root.add(
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
        let nav_chrome = build_nav(root.ui_mut(), nav);
        let stage = root.add(
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
        (nav, nav_chrome, stage)
    });

    let nav_rows = build_nav_rows(&mut ui, nav);
    wire_nav_rows(&mut ui, &nav_rows, stage, Rc::clone(&shell));
    wire_theme_toggle(&mut ui, nav_chrome, Rc::clone(&shell));
    pages::show_demo_page(&mut ui, stage, DemoPage::Overview);

    ui.set_cascade(demo_cascade(ui.properties()));
    ui
}
