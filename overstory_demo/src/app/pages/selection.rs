use super::super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct SelectionPage;

impl View for SelectionPage {
    type Output = ();

    fn build(self, stage: &mut ChildUi<'_>) -> Self::Output {
        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            |row| {
                row.demo_responsive_card("RENDERER QUEUE", PAGE_CARD_WIDTH, |list_card| {
                    let initial_summary = format!(
                        "Selected: {}",
                        renderer_queue_label(INITIAL_RENDERER_SELECTION)
                    );
                    let summary = list_card.local_state(initial_summary);
                    list_card.add(caption("").bind_text(summary.binding()));
                    list_card.add(
                        ListBox::new()
                            .items(renderer_queue_items())
                            .item_template(|row, item| {
                                row.with_child(Panel::row(9.0).align_center(), |row| {
                                    row.add(
                                        Panel::new()
                                            .background(Some(renderer_queue_stage_brush(
                                                item.index(),
                                            )))
                                            .corner_radius(4.0)
                                            .layout(Layout::new().logical_size(8.0, 8.0)),
                                    );
                                    row.with_child(Panel::new().layout(Layout::column(1.0).fill_width()), |text| {
                                        text.add(Text::new(item.label()).class(CLASS_DEMO_BODY));
                                        text.add(
                                            Text::new(renderer_queue_meta(item.index()))
                                                .class(CLASS_DEMO_CAPTION),
                                        );
                                    });
                                });
                            })
                            .fill_width(190.0)
                            .item_extent(46.0)
                            .multiple_selection()
                            .selected_index(INITIAL_RENDERER_SELECTION)
                            .scroll_offset_y(46.0)
                            .on_selection_changed(move |ui, event| {
                                let label = selection_summary(&event);
                                summary.set(ui, label);
                            })
                            .with_vertical_scrollbar()
                            .gap(10.0)
                            .scrollbar_thickness(10.0)
                            .thumb_extent(40.0)
                            .tooltip("Multi-select list with viewport clipping"),
                    )
                });

                row.demo_responsive_card("DETAIL", PAGE_CARD_WIDTH, |detail| {
                    detail.add(page_heading("Virtualized by default"));
                    detail.add(body_text(
                        "Only visible rows are realized. Selection and scrolling stay keyed to item indices.",
                    ));
                    detail.add(control_label("Inspection target"));
                    detail.add(
                        Select::new()
                            .items([
                                "Renderer queue",
                                "Layer composition",
                                "Text shaping",
                                "Input routing",
                            ])
                            .item_template(|row, item| {
                                row.with_child(Panel::row(9.0).align_center(), |row| {
                                    row.add(
                                        Panel::new()
                                            .background(Some(inspector_target_brush(item.index())))
                                            .corner_radius(5.0)
                                            .layout(Layout::new().logical_size(10.0, 10.0)),
                                    );
                                    row.with_child(
                                        Panel::new().layout(Layout::column(1.0).fill_width()),
                                        |text| {
                                            text.add(
                                                Text::new(item.label()).class(CLASS_DEMO_BODY),
                                            );
                                            text.add(
                                                Text::new(inspector_target_subtitle(item.index()))
                                                    .class(CLASS_DEMO_CAPTION),
                                            );
                                        },
                                    );
                                });
                            })
                            .selected_index(1)
                            .width(PAGE_CONTROL_WIDTH)
                            .popup_height(176.0)
                            .item_extent(44.0)
                            .tooltip("Popup-backed select using the ListBox path"),
                    )
                });

            },
        );

        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            |row| {
                row.demo_responsive_card("KEYED TABS", PAGE_CARD_WIDTH, |card| {
                    card.add(body_text(
                        "Pages stay retained while keyed selection switches their visibility.",
                    ));
                    card.with_child(
                        TabView::new()
                            .selected_key(2)
                            .item_template(|header, item| {
                                if item.option().key() == TabKey::new(2) {
                                    header.with_child(Panel::row(7.0).align_center(), |header| {
                                        header.add(Text::new(item.option().label()));
                                        header.add(Badge::count(3));
                                    });
                                } else {
                                    header.add(Text::new(item.option().label()));
                                }
                            }),
                        |tabs| {
                            tabs.tab(1, "Layers", |page| {
                                page.add(body_text("Retained composition layers and surfaces."));
                            });
                            tabs.tab(2, "Diagnostics", |page| {
                                page.with_child(
                                    ZStack::new().alignment(StackAlignment::TopLeft),
                                    |stack| {
                                        stack.add(
                                            Panel::new()
                                                .background(Some(solid(0x34, 0x3a, 0x46, 0xff)))
                                                .corner_radius(8.0)
                                                .layout(Layout::new().logical_size(260.0, 72.0)),
                                        );
                                        stack.add(
                                            Text::new("3 diagnostics ready")
                                                .layout(Layout::new().margin_all(16.0)),
                                        );
                                    },
                                );
                            });
                            tabs.tab(3, "Accessibility", |page| {
                                page.add(body_text("Inactive pages leave the accessibility tree."));
                            });
                        },
                    );
                });
            },
        );
    }
}

fn renderer_queue_stage_brush(index: usize) -> Brush {
    match index % 4 {
        0 => solid(0x64, 0xb3, 0x7d, 0xff),
        1 => solid(0x4f, 0x6f, 0xd6, 0xff),
        2 => solid(0xd1, 0x8b, 0x47, 0xff),
        _ => solid(0xb4, 0x86, 0xd8, 0xff),
    }
}

fn renderer_queue_meta(index: usize) -> String {
    let stage = (index % RENDERER_STAGES.len()) + 1;
    let batch = (index / RENDERER_STAGES.len()) + 1;
    format!(
        "stage {stage:02} / {} - batch {batch:03}",
        RENDERER_STAGES.len()
    )
}

fn inspector_target_brush(index: usize) -> Brush {
    match index {
        0 => solid(0x64, 0xb3, 0x7d, 0xff),
        1 => solid(0x6d, 0x8f, 0xff, 0xff),
        2 => solid(0xd1, 0x8b, 0x47, 0xff),
        _ => solid(0xb4, 0x86, 0xd8, 0xff),
    }
}

fn inspector_target_subtitle(index: usize) -> &'static str {
    match index {
        0 => "Virtual rows, selection, and scroll state",
        1 => "Layer tree, surfaces, and compositor handoff",
        2 => "Font features, cache keys, and wrap inputs",
        _ => "Routed pointer, key, and focus events",
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
}
