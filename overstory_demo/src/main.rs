// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Overstory control gallery.

use std::cell::Cell;
use std::rc::Rc;

use overstory::{
    Button, CheckBox, Divider, ListBox, ListItem, Panel, Popup, ProgressBar, ScrollBar,
    ScrollState, Slider, Spinner, Splitter, Text, TextInput, ToggleSwitch, Tooltip, Ui, layouts,
    select_list_item, set_progress_value, theme,
};
use overstory_host_winit::WindowOptions;
use peniko::{Brush, Color};

const INITIAL_SIZE: (u32, u32) = (1480, 660);

fn main() -> overstory_host_winit::HostResult<()> {
    overstory_host_winit::run(
        WindowOptions::new("overstory_demo").initial_size(INITIAL_SIZE.0, INITIAL_SIZE.1),
        build_ui,
    )
}

/// Caption-sized, muted `Text` spec used for the labels under the
/// toggle / spinner / slider.
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

fn section_label(text: &str) -> Text {
    Text::new(text)
        .font_size(12.0)
        .foreground(Color::from_rgba8(0x96, 0x9a, 0xa8, 0xff))
}

fn card(ui: &mut Ui, parent: overstory::ElementId, title: &str) -> overstory::ElementId {
    let card = ui.append(
        parent,
        Panel::column(14.0).layout(
            overstory::Layout::column(14.0)
                .logical_size(320.0, 350.0)
                .padding_all(18.0),
        ),
    );
    let background_prop = ui.properties().background;
    let foreground_prop = ui.properties().foreground;
    let border_brush_prop = ui.properties().border_brush;
    let border_width_prop = ui.properties().border_width;
    let corner_radius_prop = ui.properties().corner_radius;
    ui.set_local(card, background_prop, opt_solid(0x28, 0x28, 0x31, 0xff));
    ui.set_local(
        card,
        foreground_prop,
        Brush::Solid(Color::from_rgba8(0xf4, 0xf4, 0xf7, 0xff)),
    );
    ui.set_local(card, border_brush_prop, opt_solid(0x3b, 0x3b, 0x46, 0xff));
    ui.set_local(card, border_width_prop, 1.0_f64);
    ui.set_local(card, corner_radius_prop, 12.0_f64);
    ui.append(card, section_label(title));
    card
}

fn opt_solid(r: u8, g: u8, b: u8, a: u8) -> Option<Brush> {
    Some(Color::from_rgba8(r, g, b, a).into())
}

/// Builds a click handler that cycles the button's label each time
/// it fires. `event.current` points at the element the handler is
/// registered on — i.e. the button — so we don't need the id
/// captured separately.
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
        let content_prop = ui.properties().content;
        ui.set_local(event.current, content_prop, label);
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

/// Builds the demo tree as a small control gallery. The point is to exercise
/// normal app code: every visible piece is a built-in control, styled by the
/// default cascade, and animated through `Ui` timers.
fn build_ui() -> Ui {
    let mut ui = Ui::new();
    // Install the palette early so any spec that reads from the
    // theme sees real values rather than fallbacks.
    ui.set_theme(theme::default_theme());
    let root = ui.root();

    // Root layout: full-viewport column with centered gallery content.
    let root_layout = layouts::column_with_gap(22.0)
        .justify_center()
        .align_center()
        .percent_size(1.0, 1.0)
        .padding_all(36.0);
    ui.set_layout(root, root_layout);

    ui.append(root, heading("Overstory Control Gallery"));
    ui.append(
        root,
        caption("Templated controls, routed input, timers, focus, and styled parts"),
    );

    let gallery = ui.append(root, Panel::row(24.0).align_center().justify_center());

    let input_card = card(&mut ui, gallery, "INPUTS");
    ui.append(
        input_card,
        TextInput::new()
            .placeholder("Search command palette")
            .width(280.0),
    );
    ui.append(
        input_card,
        TextInput::new()
            .text("Read-only selection")
            .read_only(true)
            .width(280.0),
    );

    let button_row = ui.append(input_card, Panel::row(10.0).align_center());
    let deploy = ui.append(
        button_row,
        Button::new("Deploy")
            .font_size(15.0)
            .on_click(cycling_click_handler()),
    );
    ui.set_tooltip(deploy, Tooltip::new("Run the staged deployment workflow"));
    let review = ui.append(
        button_row,
        Button::new("Review")
            .font_size(15.0)
            .on_click(popup_click_handler()),
    );
    ui.set_tooltip(review, Tooltip::new("Toggle an anchored popup"));

    ui.append(input_card, Divider::horizontal().length(280.0));
    ui.append(
        input_card,
        CheckBox::new("Enable staged rollout").checked(true),
    );
    ui.append(input_card, CheckBox::new("Collect timing traces"));
    ui.append(input_card, CheckBox::new("Send diagnostics").disabled());

    let systems_card = card(&mut ui, gallery, "SYSTEMS");
    let toggle_row = ui.append(systems_card, Panel::row(12.0).align_center());
    ui.append(toggle_row, ToggleSwitch::new().checked(true));
    ui.append(toggle_row, Text::new("Live rendering").font_size(15.0));

    ui.append(systems_card, Divider::horizontal().length(280.0));

    ui.append(systems_card, caption("GPU Budget"));
    let progress = ui.append(
        systems_card,
        ProgressBar::new().value(0.62).size(280.0, 18.0),
    );
    ui.set_tooltip(
        progress,
        Tooltip::new("Animated by the retained timer queue"),
    );
    let _ = ui.schedule_timer_repeating(progress, 120_000_000);
    let mut progress_value = 0.62_f64;
    ui.on_timer(progress, move |ui, _event| {
        progress_value += 0.015;
        if progress_value > 1.0 {
            progress_value = 0.08;
        }
        set_progress_value(ui, progress, progress_value);
    });

    ui.append(systems_card, caption("Volume"));
    let volume = ui.append(systems_card, Slider::new().value(0.42));
    ui.set_tooltip(
        volume,
        Tooltip::new("Drag, arrow, PageUp/PageDown, or Shift+arrow"),
    );

    let spinner_row = ui.append(systems_card, Panel::row(12.0).align_center());
    ui.append(spinner_row, Spinner::new());
    ui.append(spinner_row, caption("Streaming updates"));

    let selection_card = card(&mut ui, gallery, "SELECTION");
    ui.append(selection_card, caption("Renderer queue"));
    let renderer_scroll = ScrollState::with_offset_y(34.0);
    let renderer_row = ui.append(selection_card, Panel::row(8.0).align_center());
    let renderer_list = ui.append(
        renderer_row,
        ListBox::new()
            .size(260.0, 146.0)
            .multiple_selection()
            .scroll_state(renderer_scroll.clone()),
    );
    ui.append(
        renderer_row,
        ScrollBar::vertical(renderer_list, renderer_scroll)
            .length(146.0)
            .thickness(10.0)
            .thumb_extent(34.0),
    );
    ui.append(renderer_list, ListItem::new("Vello scene graph"));
    ui.append(renderer_list, ListItem::new("CPU fallback raster"));
    let selected_renderer = ui.append(renderer_list, ListItem::new("Text shaping pass"));
    ui.append(renderer_list, ListItem::new("Accessibility tree"));
    ui.append(renderer_list, ListItem::new("Hit testing index"));
    ui.append(renderer_list, ListItem::new("Presentation cache"));
    ui.append(renderer_list, ListItem::new("Frame diagnostics"));
    let _ = select_list_item(&mut ui, renderer_list, selected_renderer);
    ui.set_tooltip(
        renderer_list,
        Tooltip::new("Multi-select list with viewport clipping"),
    );

    ui.append(selection_card, Divider::horizontal().length(280.0));
    ui.append(
        selection_card,
        caption("Range selection + viewport clipping"),
    );

    let layout_card = card(&mut ui, gallery, "LAYOUT");
    ui.append(layout_card, caption("Resizable panes"));
    let split_row = ui.append(layout_card, Panel::row(8.0).align_center());
    let left_pane = ui.append(
        split_row,
        Panel::column(8.0).layout(
            overstory::Layout::column(8.0)
                .logical_size(128.0, 118.0)
                .padding_all(12.0),
        ),
    );
    style_split_pane(&mut ui, left_pane, 0x31, 0x35, 0x43);
    ui.append(left_pane, Text::new("Scene").font_size(15.0));
    ui.append(left_pane, caption("Drag the grip"));

    let right_pane_slot = Rc::new(Cell::new(None));
    let right_pane_for_splitter = Rc::clone(&right_pane_slot);
    let splitter = ui.append(
        split_row,
        Splitter::vertical()
            .length(118.0)
            .on_resize(move |ui, event| {
                if let Some(right_pane) = right_pane_for_splitter.get() {
                    event.resize_pair(ui, left_pane, right_pane, 72.0, 72.0);
                }
            }),
    );
    ui.set_tooltip(splitter, Tooltip::new("Drag or focus and use arrow keys"));
    let right_pane = ui.append(
        split_row,
        Panel::column(8.0).layout(
            overstory::Layout::column(8.0)
                .logical_size(128.0, 118.0)
                .padding_all(12.0),
        ),
    );
    right_pane_slot.set(Some(right_pane));
    style_split_pane(&mut ui, right_pane, 0x2e, 0x3a, 0x35);
    ui.append(right_pane, Text::new("Inspector").font_size(15.0));
    ui.append(right_pane, caption("Arrow keys work"));

    ui.append(layout_card, Divider::horizontal().length(280.0));
    ui.append(
        layout_card,
        caption("Slider ticks come from understory_axis"),
    );
    ui.append(
        layout_card,
        Slider::new()
            .min(-24.0)
            .max(24.0)
            .value(6.0)
            .small_change(1.0)
            .large_change(8.0),
    );

    ui.set_cascade(theme::default_cascade(ui.properties()));

    ui
}

fn style_split_pane(ui: &mut Ui, pane: overstory::ElementId, r: u8, g: u8, b: u8) {
    ui.set_local(pane, ui.properties().background, opt_solid(r, g, b, 0xff));
    ui.set_local(
        pane,
        ui.properties().border_brush,
        opt_solid(0x48, 0x4c, 0x58, 0xff),
    );
    ui.set_local(pane, ui.properties().border_width, 1.0_f64);
    ui.set_local(pane, ui.properties().corner_radius, 8.0_f64);
}
