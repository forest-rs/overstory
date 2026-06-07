use super::super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct LayoutPage;

impl View for LayoutPage {
    type Output = ();

    fn build(self, stage: &mut ChildUi<'_>) -> Self::Output {
        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            |row| {
                row.demo_card("RESIZABLE PANES", PAGE_CARD_WIDTH, 326.0, |panes| {
                    panes.with_child(Panel::row(8.0).align_center(), |split_row| {
                        let left_pane = split_row.add_with_children(
                            Panel::column(8.0)
                                .class(CLASS_DEMO_SPLIT_SCENE)
                                .padding_all(14.0)
                                .layout(
                                    Layout::column(8.0)
                                        .logical_size(SPLIT_PANE_WIDTH, SPLIT_PANE_HEIGHT),
                                ),
                            |left_pane| {
                                left_pane.add(control_label("Scene"));
                                if let Ok(svg) = SvgElementTree::from_bytes(
                                    SCENE_GRAPH_SVG,
                                    SvgImportOptions::default(),
                                ) {
                                    left_pane.add(svg);
                                }
                                left_pane.add(caption("Retained graph"));
                            },
                        );

                        let right_pane_ref = ElementRef::<Panel>::new();
                        let right_pane_for_splitter = right_pane_ref.clone();
                        split_row.add(
                            Splitter::vertical()
                                .length(SPLIT_PANE_HEIGHT)
                                .on_resize(move |ui, event| {
                                    if let Some(right_pane) = right_pane_for_splitter.resolve(ui) {
                                        event.resize_pair(ui, left_pane, right_pane, 88.0, 88.0);
                                    }
                                })
                                .tooltip("Drag or focus and use arrow keys"),
                        );

                        split_row.with_child(
                            Panel::column(8.0)
                                .class(CLASS_DEMO_SPLIT_INSPECTOR)
                                .padding_all(14.0)
                                .layout(
                                    Layout::column(8.0)
                                        .logical_size(SPLIT_PANE_WIDTH, SPLIT_PANE_HEIGHT),
                                )
                                .reference(right_pane_ref),
                            |right_pane| {
                                right_pane.add(control_label("Inspector"));
                                right_pane.add(caption("Keyboard resize"));
                            },
                        );
                    });
                });

                row.demo_card("AXIS RULER", PAGE_CARD_WIDTH, 326.0, |ruler| {
                    ruler.add(caption("Slider ticks come from understory_axis"));
                    ruler.add(
                        Slider::new()
                            .min(-24.0)
                            .max(24.0)
                            .value(6.0)
                            .small_change(1.0)
                            .large_change(8.0),
                    );
                    ruler.add(Divider::horizontal().length(PAGE_CONTROL_WIDTH));
                    ruler.add(
                        body_text("Longhand layout properties keep slider ticks, labels, and thumbs independently positioned without replacing the rest of the style."),
                    );
                });
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::show_demo_page;
    use super::*;

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
