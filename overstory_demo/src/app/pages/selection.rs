use super::super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct SelectionPage;

impl View for SelectionPage {
    type Output = ();

    fn build(self, stage: &mut ChildUi<'_>) -> Self::Output {
        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            |row| {
                row.demo_card("RENDERER QUEUE", PAGE_CARD_WIDTH, 326.0, |list_card| {
                    let initial_summary = format!(
                        "Selected: {}",
                        renderer_queue_label(INITIAL_RENDERER_SELECTION)
                    );
                    let summary = list_card.local_state(initial_summary);
                    list_card.add(caption("").bind_content(summary.binding()));
                    list_card.add(
                        ListBox::new()
                            .items(renderer_queue_items())
                            .size(PAGE_LIST_WIDTH, 178.0)
                            .multiple_selection()
                            .selected_index(INITIAL_RENDERER_SELECTION)
                            .scroll_offset_y(34.0)
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

                row.demo_card("DETAIL", PAGE_CARD_WIDTH, 326.0, |detail| {
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
                            .selected_index(1)
                            .width(PAGE_CONTROL_WIDTH)
                            .popup_height(124.0)
                            .tooltip("Popup-backed select using the ListBox path"),
                    )
                });
            },
        );
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
