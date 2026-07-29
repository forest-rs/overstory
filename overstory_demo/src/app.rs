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

use overstory::color::ColorSpaceTag;
use overstory::motion::{MotionSpec, TimingFunction, Transitions, millis};
use overstory::pseudos::{PSEUDO_FOCUS_VISIBLE, PSEUDO_PRESSED, PSEUDO_SELECTED};
use overstory::{
    Accordion, AccordionChildUiExt, AccordionSection, Affine, Badge, BindingSpecExt, BorderStyle,
    BuiltInProperties, Button, CheckBox, ChildUi, ChoiceOption, ContextMenu,
    ControlTemplateSpecExt, CornerRadius, CornerShape, Corners, CursorIcon, Divider, Edges,
    ElementId, ElementRef, ElementRefSpecExt, ElementSpecExt, Expander, FontFeature, Layout,
    LayoutAlign, LayoutAlignment, LayoutContainer, LayoutDimension, LayoutJustify, LayoutPlacement,
    LayoutSize, ListBox, LocalState, Menu, MenuButton, MenuItem, OutlineView, OutlineViewNode,
    Panel, Placement, Popup, ProgressBar, RadioButton, RadioGroup, Rect, SLOT_BUTTON_CONTENT,
    SLOT_CHECK_BOX_BOX, SLOT_CHECK_BOX_LABEL, SLOT_CHECK_BOX_MARK, SLOT_SLIDER_THUMB,
    SLOT_SLIDER_TRACK, ScrollView, SegmentedControl, Select, Slider, Spinner, Splitter,
    StackAlignment, SurfaceSpecExt, SurfaceStyleExt, TYPE_BUTTON, TYPE_GENERIC, TYPE_TEXT, TabKey,
    TabPage, TabView, TabViewChildUiExt, TemplateNode, TemplateSpec, Text, TextInput,
    TextInputSubmitAction, TextSpecExt, TimerDuration, TimerId, ToggleSwitch, Tooltip,
    TooltipSpecExt, Ui, View, Viewport, ZStack, layouts, list_box_scroll_area_list,
    realized_list_item_index, remove_tab_view_page, selected_list_index, set_progress_value,
    set_text_input_text, text_input_text, theme,
};
use overstory_svg::{HitParams, SvgElementTree, SvgImportOptions};
use peniko::kurbo::{Point, Vec2};
use peniko::{Brush, Color};
use understory_style::{
    ClassId, PseudoClassId, ResourceKey, Selector, SelectorStep, StyleBuilder, StyleCascade,
    StyleCascadeBuilder, StyleOrigin, expr,
};
use understory_view2d::{ClampMode, FitMode, Viewport2D};

const INITIAL_WINDOW_SIZE: (u32, u32) = (1480, 820);
#[cfg(test)]
const INITIAL_VIEWPORT_SIZE: kurbo::Size = kurbo::Size {
    width: 1480.0,
    height: 820.0,
};
const ROOT_GAP: f64 = 24.0;
const ROOT_PADDING: f64 = 28.0;
const NAV_WIDTH: f64 = 276.0;
const NAV_PADDING: f64 = 20.0;
const NAV_CONTENT_WIDTH: f64 = NAV_WIDTH - (NAV_PADDING * 2.0);
const PAGE_HEADER_HEIGHT: f64 = 92.0;
const OVERVIEW_CARD_WIDTH: f64 = 256.0;
const PAGE_CARD_WIDTH: f64 = 390.0;
const PAGE_CONTROL_WIDTH: f64 = 328.0;
const PAGE_LIST_WIDTH: f64 = 316.0;
const SPLIT_PANE_WIDTH: f64 = 148.0;
const SPLIT_PANE_HEIGHT: f64 = 142.0;
const DEVTOOLS_INSPECTOR_WIDTH: f64 = 600.0;
const DEVTOOLS_REFRESH_INTERVAL_MS: u64 = 500;
const SVG_PAGE_WIDTH: f64 = 824.0;
const SVG_INFO_HEIGHT: f64 = 132.0;
const SVG_TIGER_CARD_WIDTH: f64 = 548.0;
const SVG_TIGER_CARD_HEIGHT: f64 = 500.0;
const SVG_INSPECTOR_CARD_WIDTH: f64 = 258.0;
const SVG_VIEWPORT_WIDTH: f64 = 500.0;
const SVG_VIEWPORT_HEIGHT: f64 = 328.0;
const SVG_ZOOM_SENSITIVITY: f64 = 0.0015;
const SVG_GRID_LIMIT: usize = 140;
const SVG_SELECTION_WOBBLE_UP_MS: u64 = 135;
const SVG_SELECTION_WOBBLE_DOWN_MS: u64 = 150;

mod chrome;
mod devtools;
mod fixtures;
mod pages;
mod style;
#[cfg(test)]
mod tests;
mod widgets;

use chrome::*;
use devtools::*;
use fixtures::*;
#[cfg(feature = "host-runner")]
use overstory_app::WindowOptions;
use style::*;
use widgets::*;

/// Return the demo's preferred initial window size in logical pixels.
pub fn initial_window_size() -> (u32, u32) {
    INITIAL_WINDOW_SIZE
}

/// Run the demo through the selected desktop host.
#[cfg(feature = "host-runner")]
pub fn run() -> overstory_app::HostResult<()> {
    let (width, height) = initial_window_size();
    overstory_app::run(
        WindowOptions::new("overstory_demo").initial_size(width, height),
        build_ui,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DemoPage {
    Overview,
    Workbench,
    Inputs,
    Disclosure,
    Systems,
    Selection,
    Outline,
    Layout,
    Anchoring,
    BoxModel,
    Transcript,
    Svg,
    Typography,
    Motion,
}

impl DemoPage {
    const ALL: [Self; 14] = [
        Self::Overview,
        Self::Workbench,
        Self::Inputs,
        Self::Disclosure,
        Self::Systems,
        Self::Selection,
        Self::Outline,
        Self::Layout,
        Self::Anchoring,
        Self::BoxModel,
        Self::Transcript,
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
            Self::Workbench => 1,
            Self::Inputs => 2,
            Self::Disclosure => 3,
            Self::Systems => 4,
            Self::Selection => 5,
            Self::Outline => 6,
            Self::Layout => 7,
            Self::Anchoring => 8,
            Self::BoxModel => 9,
            Self::Transcript => 10,
            Self::Svg => 11,
            Self::Typography => 12,
            Self::Motion => 13,
        }
    }

    fn nav_label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Workbench => "Workbench",
            Self::Inputs => "Inputs",
            Self::Disclosure => "Disclosure",
            Self::Systems => "Systems",
            Self::Selection => "Selection",
            Self::Outline => "Outline",
            Self::Layout => "Layout",
            Self::Anchoring => "Anchoring",
            Self::BoxModel => "Box Model",
            Self::Transcript => "Transcript",
            Self::Svg => "SVG",
            Self::Typography => "Typography",
            Self::Motion => "Motion",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overstory Control Gallery",
            Self::Workbench => "Styling & Theming Workbench",
            Self::Inputs => "Inputs and Command Surfaces",
            Self::Disclosure => "Disclosure Containers",
            Self::Systems => "Controls and Values",
            Self::Selection => "Selection and Virtualization",
            Self::Outline => "Expandable Outline",
            Self::Layout => "Layout and Resizing",
            Self::Anchoring => "Anchored Overlays",
            Self::BoxModel => "Box Decoration and Contours",
            Self::Transcript => "Agent Transcript",
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
            Self::Workbench => {
                "Drive a live control's properties, swap its template, and re-skin the accent token \u{2014} every edit flows through the real binding, style, and theme paths."
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
            Self::Anchoring => {
                "Popup and tooltip targets exercising preferred sides, measured overlay placement, and viewport fallback."
            }
            Self::BoxModel => {
                "Border sides, padding contours, elliptical radii, and shaped corners resolved through Understory box decoration geometry."
            }
            Self::Transcript => {
                "A rich interaction log backed by understory_transcript and realized as Overstory elements."
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

#[derive(Debug)]
struct DemoShellState {
    resources: DemoResources,
    theme_mode: Cell<DemoThemeMode>,
    accent: Cell<DemoAccent>,
    selected_page: Cell<DemoPage>,
}

impl DemoShellState {
    fn new(resources: DemoResources) -> Self {
        Self {
            resources,
            theme_mode: Cell::new(DemoThemeMode::Dark),
            accent: Cell::new(DemoAccent::Indigo),
            selected_page: Cell::new(DemoPage::Overview),
        }
    }

    fn accent(&self) -> DemoAccent {
        self.accent.get()
    }

    /// The theme for the current mode + accent. Both the mode toggle and the
    /// styling workbench rebuild and re-apply through this single path.
    fn current_theme(&self) -> understory_style::Theme {
        self.theme_mode
            .get()
            .theme(self.resources, self.accent.get())
    }

    fn toggle_theme(&self) -> DemoThemeMode {
        let next = self.theme_mode.get().toggled();
        self.theme_mode.set(next);
        next
    }

    fn set_accent(&self, accent: DemoAccent) {
        self.accent.set(accent);
    }

    fn set_selected_page(&self, page: DemoPage) {
        self.selected_page.set(page);
    }
}

/// Applies the shell's current mode + accent theme. Visible to the chrome and
/// page submodules as a private ancestor item.
fn apply_demo_theme(ui: &mut Ui, shell: &DemoShellState) {
    ui.set_theme(shell.current_theme());
}

/// Build the demo tree as a focused gallery.
///
/// The shell owns page selection; each page owns only its local controls and
/// timers.
pub fn build_ui() -> Ui {
    let mut ui = Ui::new();
    let tokens = ui.style_tokens::<DemoStyleTokens>();
    let shell = Rc::new(DemoShellState::new(tokens.resources));
    apply_demo_theme(&mut ui, &shell);
    let root = ui.root();
    ui.add_class(root, CLASS_DEMO_ROOT);

    ui.set_layout(
        root,
        layouts::row_with_gap(ROOT_GAP)
            .percent_size(1.0, 1.0)
            .alignment(LayoutAlignment::new().align_items(LayoutAlign::Stretch)),
    );
    ui.set_box_padding(root, Edges::all(ROOT_PADDING));

    let (nav_chrome, nav_rows, stage) = ui.with_children(root, |root| {
        let (nav_chrome, nav_rows) = root.with_child(
            Panel::column(18.0)
                .class(CLASS_DEMO_NAV)
                .layout(
                    Layout::column(18.0)
                        .size(
                            LayoutSize::new()
                                .width(LayoutDimension::Logical(NAV_WIDTH))
                                .height(LayoutDimension::Percent(1.0)),
                        )
                        .flex_shrink(0.0),
                )
                .padding_all(NAV_PADDING),
            |nav| {
                let nav_chrome = build_nav(nav);
                let nav_rows = build_nav_rows(nav);
                (nav_chrome, nav_rows)
            },
        );
        let stage = root.add(
            ScrollView::new().class(CLASS_DEMO_STAGE).layout(
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
        (nav_chrome, nav_rows, stage)
    });

    wire_nav_rows(&mut ui, &nav_rows, stage, Rc::clone(&shell));
    wire_theme_toggle(&mut ui, nav_chrome, Rc::clone(&shell));
    wire_devtools_button(&mut ui, nav_chrome.devtools_button());
    pages::show_demo_page_themed(&mut ui, stage, DemoPage::Overview, &shell);

    ui.set_cascade(demo_cascade(ui.properties(), tokens));
    ui
}
