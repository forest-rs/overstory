// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Overstory control gallery.

extern crate alloc;

use alloc::boxed::Box;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::Cell;

use overstory::{
    Button, CheckBox, Divider, ElementId, FontFeature, Layout, LayoutAlign, LayoutAlignment,
    LayoutDimension, LayoutSize, ListBox, Panel, Popup, ProgressBar, ScrollBar, ScrollState,
    Slider, Spinner, Splitter, Text, TextInput, ToggleSwitch, Tooltip, Ui, layouts,
    set_progress_value, theme,
};
use overstory_host_winit::WindowOptions;
use peniko::{Brush, Color};

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
    Typography,
}

impl DemoPage {
    const ALL: [Self; 6] = [
        Self::Overview,
        Self::Inputs,
        Self::Systems,
        Self::Selection,
        Self::Layout,
        Self::Typography,
    ];

    fn from_index(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }

    fn nav_label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Inputs => "Inputs",
            Self::Systems => "Systems",
            Self::Selection => "Selection",
            Self::Layout => "Layout",
            Self::Typography => "Typography",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overstory Control Gallery",
            Self::Inputs => "Inputs and Command Surfaces",
            Self::Systems => "Controls and Values",
            Self::Selection => "Selection and Virtualization",
            Self::Layout => "Layout and Resizing",
            Self::Typography => "Typography and Text Features",
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
            Self::Typography => {
                "Font features, wrapping, editable text, and dynamic numbers shaped through Parley."
            }
        }
    }
}

fn caption(text: &str) -> Text {
    Text::new(text)
        .font_size(13.0)
        .foreground(Color::from_rgba8(0xc8, 0xc8, 0xd2, 0xff))
}

fn heading(text: &str) -> Text {
    Text::new(text)
        .font_size(26.0)
        .foreground(Color::from_rgba8(0xf4, 0xf4, 0xf7, 0xff))
}

fn page_heading(text: &str) -> Text {
    Text::new(text)
        .font_size(22.0)
        .foreground(Color::from_rgba8(0xf4, 0xf4, 0xf7, 0xff))
}

fn card_title(text: &str) -> Text {
    Text::new(text)
        .font_size(12.0)
        .foreground(Color::from_rgba8(0x96, 0x9a, 0xa8, 0xff))
}

fn control_label(text: &str) -> Text {
    Text::new(text)
        .font_size(15.0)
        .foreground(Color::from_rgba8(0xf4, 0xf4, 0xf7, 0xff))
}

fn number_text(text: &str) -> Text {
    Text::new(text)
        .font_size(22.0)
        .foreground(Color::from_rgba8(0xf4, 0xf4, 0xf7, 0xff))
}

fn large_number_text(text: &str) -> Text {
    Text::new(text)
        .font_size(34.0)
        .foreground(Color::from_rgba8(0xf4, 0xf4, 0xf7, 0xff))
}

fn opt_solid(r: u8, g: u8, b: u8, a: u8) -> Option<Brush> {
    Some(Color::from_rgba8(r, g, b, a).into())
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

fn style_surface(
    ui: &mut Ui,
    element: ElementId,
    background: (u8, u8, u8),
    border: (u8, u8, u8),
    radius: f64,
) {
    ui.set_local(
        element,
        ui.properties().background,
        opt_solid(background.0, background.1, background.2, 0xff),
    );
    ui.set_local(
        element,
        ui.properties().border_brush,
        opt_solid(border.0, border.1, border.2, 0xff),
    );
    ui.set_local(element, ui.properties().border_width, 1.0_f64);
    ui.set_local(element, ui.properties().corner_radius, radius);
}

fn style_fill(ui: &mut Ui, element: ElementId, color: (u8, u8, u8)) {
    ui.set_local(
        element,
        ui.properties().background,
        opt_solid(color.0, color.1, color.2, 0xff),
    );
}

fn demo_card(ui: &mut Ui, parent: ElementId, title: &str, width: f32, height: f32) -> ElementId {
    let card = ui.append(
        parent,
        Panel::column(14.0).layout(
            Layout::column(14.0)
                .logical_size(width, height)
                .padding_all(18.0),
        ),
    );
    style_surface(ui, card, (0x27, 0x28, 0x31), (0x3b, 0x3d, 0x49), 12.0);
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
            ui.append(popup, caption("3 items pending"));
        });
        open_popup.set(popup);
    }
}

/// Builds the demo tree as a focused gallery. The shell owns page selection;
/// each page owns only its local controls and timers.
fn build_ui() -> Ui {
    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    let root = ui.root();

    ui.set_layout(
        root,
        layouts::row_with_gap(ROOT_GAP)
            .percent_size(1.0, 1.0)
            .padding_all(ROOT_PADDING)
            .alignment(LayoutAlignment::new().align_items(LayoutAlign::Stretch)),
    );

    let nav = ui.append(
        root,
        Panel::column(18.0).layout(
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
    style_surface(&mut ui, nav, (0x22, 0x23, 0x2b), (0x3a, 0x3d, 0x4b), 16.0);
    build_nav(&mut ui, nav);

    let stage = ui.append(
        root,
        Panel::column(18.0).layout(
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
    style_surface(&mut ui, stage, (0x1b, 0x1c, 0x23), (0x2f, 0x32, 0x3d), 18.0);

    let nav_list = ui.append(
        nav,
        ListBox::new()
            .items(DemoPage::ALL.iter().map(|page| page.nav_label()))
            .size(NAV_CONTENT_WIDTH, 244.0)
            .item_extent(36.0)
            .selected_index(0)
            .on_selection_changed(move |ui, event| {
                let Some(index) = event.primary_index else {
                    return;
                };
                let Some(page) = DemoPage::from_index(index) else {
                    return;
                };
                show_demo_page(ui, stage, page);
            }),
    );
    ui.set_tooltip(nav_list, Tooltip::new("Choose a focused demo page"));

    show_demo_page(&mut ui, stage, DemoPage::Overview);

    ui.set_cascade(theme::default_cascade(ui.properties()));
    ui
}

fn build_nav(ui: &mut Ui, nav: ElementId) {
    ui.append(nav, heading("Overstory"));
    ui.append(nav, caption("Focused widget gallery"));

    let marker = ui.append(
        nav,
        Panel::new().layout(Layout::new().logical_size(NAV_CONTENT_WIDTH, 4.0)),
    );
    style_fill(ui, marker, (0x59, 0x75, 0xe6));
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
        DemoPage::Typography => build_typography_page(ui, stage),
    }
}

fn build_page_header(ui: &mut Ui, stage: ElementId, page: DemoPage) {
    let header = ui.append(
        stage,
        Panel::row(18.0).layout(
            Layout::row(18.0)
                .size(
                    LayoutSize::new()
                        .width(LayoutDimension::Percent(1.0))
                        .height(LayoutDimension::Logical(PAGE_HEADER_HEIGHT)),
                )
                .padding_all(18.0),
        ),
    );
    style_surface(ui, header, (0x24, 0x25, 0x2e), (0x3b, 0x3e, 0x4c), 16.0);

    let accent = ui.append(
        header,
        Panel::new().layout(Layout::new().logical_size(5.0, 56.0).flex_shrink(0.0)),
    );
    style_surface(ui, accent, (0x59, 0x75, 0xe6), (0x59, 0x75, 0xe6), 8.0);

    let copy = ui.append(
        header,
        Panel::column(8.0).layout(Layout::column(8.0).flex_grow(1.0)),
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
    let renderer_scroll = ScrollState::with_offset_y(28.0);
    let renderer_row = ui.append(selection, Panel::row(8.0).align_center());
    let list = ui.append(
        renderer_row,
        ListBox::new()
            .items(renderer_queue_items())
            .size(OVERVIEW_LIST_WIDTH, 132.0)
            .multiple_selection()
            .selected_index(INITIAL_RENDERER_SELECTION)
            .scroll_state(renderer_scroll.clone()),
    );
    ui.append(
        renderer_row,
        ScrollBar::vertical(list, renderer_scroll)
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
    let renderer_scroll = ScrollState::with_offset_y(34.0);
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
            .scroll_state(renderer_scroll.clone())
            .on_selection_changed(move |ui, event| {
                let label = selection_summary(&event);
                ui.set_local(summary, ui.properties().content, label);
            }),
    );
    ui.append(
        renderer_row,
        ScrollBar::vertical(renderer_list, renderer_scroll)
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
        Text::new(
            "Only visible rows are realized. Selection stays in item indices, and the scrollbar shares viewport metrics with the list.",
        )
        .font_size(14.0)
        .foreground(Color::from_rgba8(0xd8, 0xd9, 0xe2, 0xff))
        .wrap(),
    );
}

fn build_layout_page(ui: &mut Ui, stage: ElementId) {
    let row = demo_row(ui, stage);
    let panes = demo_card(ui, row, "RESIZABLE PANES", PAGE_CARD_WIDTH, 326.0);
    let split_row = ui.append(panes, Panel::row(8.0).align_center());
    let left_pane = ui.append(
        split_row,
        Panel::column(8.0).layout(
            Layout::column(8.0)
                .logical_size(SPLIT_PANE_WIDTH, SPLIT_PANE_HEIGHT)
                .padding_all(14.0),
        ),
    );
    style_split_pane(ui, left_pane, 0x31, 0x35, 0x43);
    ui.append(left_pane, Text::new("Scene").font_size(15.0));
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
        Panel::column(8.0).layout(
            Layout::column(8.0)
                .logical_size(SPLIT_PANE_WIDTH, SPLIT_PANE_HEIGHT)
                .padding_all(14.0),
        ),
    );
    right_pane_slot.set(Some(right_pane));
    style_split_pane(ui, right_pane, 0x2e, 0x3a, 0x35);
    ui.append(right_pane, Text::new("Inspector").font_size(15.0));
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
        Text::new(
            "Longhand layout properties keep slider ticks, labels, and thumbs independently positioned without replacing the rest of the style.",
        )
        .font_size(14.0)
        .foreground(Color::from_rgba8(0xd8, 0xd9, 0xe2, 0xff))
        .wrap(),
    );
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
        Text::new(
            "The number text uses the OpenType tabular-figures feature, so its measured width stays steady as digits change.",
        )
        .font_size(14.0)
        .foreground(Color::from_rgba8(0xd8, 0xd9, 0xe2, 0xff))
        .wrap(),
    );

    let text = demo_card(ui, row, "TEXT SURFACES", PAGE_CARD_WIDTH, 326.0);
    ui.append(
        text,
        Text::new("Overstory keeps simple labels cheap while still resolving inherited text style through templates and generated parts.")
            .font_size(16.0)
            .foreground(Color::from_rgba8(0xf4, 0xf4, 0xf7, 0xff))
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

fn style_split_pane(ui: &mut Ui, pane: ElementId, r: u8, g: u8, b: u8) {
    ui.set_local(pane, ui.properties().background, opt_solid(r, g, b, 0xff));
    ui.set_local(
        pane,
        ui.properties().border_brush,
        opt_solid(0x48, 0x4c, 0x58, 0xff),
    );
    ui.set_local(pane, ui.properties().border_width, 1.0_f64);
    ui.set_local(pane, ui.properties().corner_radius, 8.0_f64);
}

#[cfg(test)]
mod tests {
    use kurbo::Size;

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

    fn page_row(ui: &Ui, stage: ElementId) -> ElementId {
        ui.get(stage)
            .expect("stage should exist")
            .children()
            .get(1)
            .copied()
            .expect("stage should contain header and page row")
    }
}
