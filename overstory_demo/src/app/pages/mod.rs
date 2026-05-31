mod devtools;
mod motion;
pub(super) mod svg;

use super::*;

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

pub(super) fn show_demo_page(ui: &mut Ui, stage: ElementId, page: DemoPage) {
    clear_children(ui, stage);
    build_page_header(ui, stage, page);
    match page {
        DemoPage::Overview => build_overview_page(ui, stage),
        DemoPage::Inputs => build_inputs_page(ui, stage),
        DemoPage::Systems => build_systems_page(ui, stage),
        DemoPage::Selection => build_selection_page(ui, stage),
        DemoPage::Outline => build_outline_page(ui, stage),
        DemoPage::Layout => build_layout_page(ui, stage),
        DemoPage::Svg => svg::build_svg_page(ui, stage),
        DemoPage::Typography => build_typography_page(ui, stage),
        DemoPage::Motion => motion::build_motion_page(ui, stage),
        DemoPage::Devtools => devtools::build_devtools_page(ui, stage),
    }
}

fn build_page_header(ui: &mut Ui, stage: ElementId, page: DemoPage) {
    ui.children(
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
        |header| {
            header.add(
                Panel::new()
                    .class(CLASS_DEMO_HEADER_ACCENT)
                    .layout(Layout::new().logical_size(5.0, 56.0).flex_shrink(0.0)),
            );
            header.children(
                Panel::column(8.0).layout(
                    Layout::column(8.0)
                        .size(LayoutSize::new().min_width(LayoutDimension::Logical(0.0)))
                        .flex_grow(1.0)
                        .flex_shrink(1.0),
                ),
                |copy| {
                    copy.add(page_heading(page.title()));
                    copy.add(caption(page.subtitle()).wrap());
                },
            );
        },
    );
}

fn build_overview_page(ui: &mut Ui, stage: ElementId) {
    let row = demo_row(ui, stage);
    with_demo_card(ui, row, "INPUTS", OVERVIEW_CARD_WIDTH, 222.0, |inputs| {
        inputs.add(
            TextInput::new()
                .placeholder("Search actions")
                .width(OVERVIEW_CONTROL_WIDTH),
        );
        inputs.children(Panel::row(10.0).align_center(), |actions| {
            actions.add_with(
                Button::new("Deploy")
                    .font_size(15.0)
                    .on_click(cycling_click_handler()),
                |ui, deploy| {
                    ui.set_tooltip(deploy, Tooltip::new("Button content is template-bound"));
                },
            );
            actions.add(Button::new("Preview").font_size(15.0));
        });
        inputs.add(CheckBox::new("Record diagnostics").checked(true));
    });

    with_demo_card(ui, row, "SYSTEMS", OVERVIEW_CARD_WIDTH, 222.0, |systems| {
        systems.children(Panel::row(12.0).align_center(), |toggle_row| {
            toggle_row.add(ToggleSwitch::new().checked(true));
            toggle_row.add(control_label("Rendering enabled"));
        });
        systems.add(caption("GPU Budget"));
        systems.add_with(
            ProgressBar::new()
                .value(0.62)
                .size(OVERVIEW_CONTROL_WIDTH, 18.0),
            |ui, progress| {
                drive_progress(ui, progress, 0.62, 0.015, 120_000_000);
            },
        );
        let systems_id = systems.parent();
        append_tabular_counter(
            systems.ui_mut(),
            systems_id,
            "Frames",
            "fps",
            120_000_000,
            7,
        );
    });

    with_demo_card(
        ui,
        row,
        "SELECTION",
        OVERVIEW_CARD_WIDTH,
        222.0,
        |selection| {
            selection.add(
                ListBox::new()
                    .items(renderer_queue_items())
                    .size(OVERVIEW_LIST_WIDTH, 132.0)
                    .multiple_selection()
                    .selected_index(INITIAL_RENDERER_SELECTION)
                    .scroll_offset_y(28.0)
                    .with_vertical_scrollbar()
                    .scrollbar_thickness(10.0)
                    .thumb_extent(34.0),
            );
        },
    );
}

fn build_inputs_page(ui: &mut Ui, stage: ElementId) {
    let row = demo_row(ui, stage);
    with_demo_card(ui, row, "TEXT INPUTS", PAGE_CARD_WIDTH, 224.0, |forms| {
        forms.add(
            TextInput::new()
                .placeholder("Search command palette")
                .width(PAGE_CONTROL_WIDTH),
        );
        forms.add(
            TextInput::new()
                .text("Read-only selection")
                .read_only(true)
                .width(PAGE_CONTROL_WIDTH),
        );
        forms.add(caption(
            "Focus rings, hover borders, selection, and read-only state",
        ));
    });

    with_demo_card(ui, row, "ACTIONS", PAGE_CARD_WIDTH, 224.0, |actions| {
        actions.children(Panel::row(10.0).align_center(), |buttons| {
            buttons.add_with(
                Button::new("Deploy")
                    .font_size(15.0)
                    .on_click(cycling_click_handler()),
                |ui, deploy| {
                    ui.set_tooltip(deploy, Tooltip::new("Run the staged deployment workflow"));
                },
            );
            buttons.add_with(
                Button::new("Review")
                    .font_size(15.0)
                    .on_click(popup_click_handler()),
                |ui, review| {
                    ui.set_tooltip(review, Tooltip::new("Toggle an anchored popup"));
                },
            );
        });
        actions.add(Divider::horizontal().length(PAGE_CONTROL_WIDTH));
        actions.add(CheckBox::new("Enable staged rollout").checked(true));
        actions.add(CheckBox::new("Collect timing traces"));
        actions.add(CheckBox::new("Send diagnostics").disabled());
    });

    let row = demo_row(ui, stage);
    with_demo_fit_card(ui, row, "RADIO GROUP", PAGE_CARD_WIDTH, 214.0, |radios| {
        let summary = radios.add(caption("Release lane: Balanced"));
        radios.add(
            RadioGroup::new()
                .items(["Balanced", "Quality", "Low latency"])
                .selected_index(0)
                .on_selection_changed(move |ui, event| {
                    let selected_item = event.selected_item.unwrap_or_else(|| "None".to_owned());
                    ui.set_local(
                        summary,
                        ui.properties().content,
                        format!("Release lane: {selected_item}"),
                    );
                }),
        );
    });
    with_demo_fit_card(ui, row, "EXPANDER", PAGE_CARD_WIDTH, 184.0, |expanders| {
        expanders.content(
            Expander::new("Advanced routing").expanded(true),
            |advanced| {
                advanced.add(CheckBox::new("Require reviewer sign-off").checked(true));
                advanced.add(CheckBox::new("Pause on anomaly"));
            },
        );
        expanders.content(Expander::new("Audit trail"), |advanced| {
            advanced.add(CheckBox::new("Record detailed timing"));
            advanced.add(CheckBox::new("Attach render snapshots"));
        });
    });

    let row = demo_row(ui, stage);
    with_demo_fit_card(ui, row, "SEGMENTED", PAGE_CARD_WIDTH, 154.0, |segments| {
        let summary = segments.add(caption("View mode: Preview"));
        segments.add(
            SegmentedControl::new()
                .options([
                    ChoiceOption::new("Preview"),
                    ChoiceOption::new("Code"),
                    ChoiceOption::new("Metrics"),
                    ChoiceOption::new("Trace").disabled(),
                ])
                .selected_index(0)
                .on_selection_changed(move |ui, event| {
                    let selected_item = event.selected_item.unwrap_or_else(|| "None".to_owned());
                    ui.set_local(
                        summary,
                        ui.properties().content,
                        format!("View mode: {selected_item}"),
                    );
                }),
        );
    });
}

fn build_systems_page(ui: &mut Ui, stage: ElementId) {
    let row = demo_row(ui, stage);
    with_demo_card(
        ui,
        row,
        "CONTROL SURFACE",
        PAGE_CARD_WIDTH,
        322.0,
        |controls| {
            controls.children(Panel::row(12.0).align_center(), |toggle_row| {
                toggle_row.add(ToggleSwitch::new().checked(true));
                toggle_row.add(control_label("Rendering enabled"));
            });
            controls.add(Divider::horizontal().length(PAGE_CONTROL_WIDTH));
            controls.add(caption("GPU Budget"));
            controls.add_with(
                ProgressBar::new()
                    .value(0.62)
                    .size(PAGE_CONTROL_WIDTH, 18.0),
                |ui, progress| {
                    ui.set_tooltip(
                        progress,
                        Tooltip::new("Animated by the retained timer queue"),
                    );
                    drive_progress(ui, progress, 0.62, 0.015, 120_000_000);
                },
            );
            controls.add(caption("Volume"));
            controls.add_with(Slider::new().value(0.42), |ui, volume| {
                ui.set_tooltip(
                    volume,
                    Tooltip::new("Drag, arrow, PageUp/PageDown, or Shift+arrow"),
                );
            });
            controls.children(Panel::row(12.0).align_center(), |spinner_row| {
                spinner_row.add(Spinner::new());
                spinner_row.add(caption("Streaming updates"));
            });
        },
    );

    let telemetry = demo_card(ui, row, "TELEMETRY", PAGE_CARD_WIDTH, 218.0);
    append_large_counter(ui, telemetry, "Throughput", "events / min", 90_000_000, 137);
}

fn build_selection_page(ui: &mut Ui, stage: ElementId) {
    let row = demo_row(ui, stage);
    with_demo_card(
        ui,
        row,
        "RENDERER QUEUE",
        PAGE_CARD_WIDTH,
        326.0,
        |list_card| {
            let initial_summary = format!(
                "Selected: {}",
                renderer_queue_label(INITIAL_RENDERER_SELECTION)
            );
            let summary = list_card.add(caption(&initial_summary));
            list_card.add_with(
                ListBox::new()
                    .items(renderer_queue_items())
                    .size(PAGE_LIST_WIDTH, 178.0)
                    .multiple_selection()
                    .selected_index(INITIAL_RENDERER_SELECTION)
                    .scroll_offset_y(34.0)
                    .on_selection_changed(move |ui, event| {
                        let label = selection_summary(&event);
                        ui.set_local(summary, ui.properties().content, label);
                    })
                    .with_vertical_scrollbar()
                    .gap(10.0)
                    .scrollbar_thickness(10.0)
                    .thumb_extent(40.0),
                |ui, renderer_list| {
                    ui.set_tooltip(
                        renderer_list,
                        Tooltip::new("Multi-select list with viewport clipping"),
                    );
                },
            )
        },
    );

    with_demo_card(ui, row, "DETAIL", PAGE_CARD_WIDTH, 326.0, |detail| {
        detail.add(page_heading("Virtualized by default"));
        detail.add(body_text(
            "Only visible rows are realized. Selection and scrolling stay keyed to item indices.",
        ));
        detail.add(control_label("Inspection target"));
        detail.add_with(
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
            |ui, select| {
                ui.set_tooltip(
                    select,
                    Tooltip::new("Popup-backed select using the ListBox path"),
                );
            },
        )
    });
}

fn build_outline_page(ui: &mut Ui, stage: ElementId) {
    let row = demo_row(ui, stage);
    with_demo_card(
        ui,
        row,
        "PIPELINE OUTLINE",
        PAGE_CARD_WIDTH,
        374.0,
        |tree| {
            tree.add(page_heading("Expandable retained rows"));
            tree.add(body_text(
                "Click a parent row or use Left/Right, Enter, and Space to expand and collapse.",
            ));
            tree.add_with(
                OutlineView::new()
                    .nodes(renderer_outline_nodes(), Some(0))
                    .expanded(0)
                    .expanded(5)
                    .expanded(9)
                    .selected_key(10)
                    .size(PAGE_LIST_WIDTH, 194.0)
                    .item_extent(30.0)
                    .overscan(60.0)
                    .with_vertical_scrollbar()
                    .gap(10.0)
                    .scrollbar_thickness(10.0)
                    .thumb_extent(34.0),
                |ui, outline| {
                    ui.set_tooltip(
                        outline,
                        Tooltip::new("Virtualized tree projection from understory_outline"),
                    );
                },
            )
        },
    );

    with_demo_card(ui, row, "NODE DETAIL", PAGE_CARD_WIDTH, 374.0, |detail| {
        detail.add(page_heading("Motion"));
        detail.add(body_text(
            "The selected branch represents scheduled presentation work: implicit transitions, timeline cues, and compositor-ready offsets.",
        ));
        detail.add(control_label("Row behavior"));
        detail.add(body_text(
            "Expansion state is held by the outline control; scroll metrics still come from the shared scroll target registry.",
        ));
    });
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn layout_page_contains_imported_svg_vector_paths() {
        let mut ui = build_ui();
        let stage = stage_element(&ui);
        show_demo_page(&mut ui, stage, DemoPage::Layout);

        assert_eq!(count_type(&ui, ui.root(), overstory::TYPE_VECTOR_PATH), 5);
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
}
