use super::super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct OutlinePage;

impl View for OutlinePage {
    type Output = ();

    fn build(self, stage: &mut ChildUi<'_>) -> Self::Output {
        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            |row| {
                row.demo_responsive_card("PIPELINE OUTLINE", PAGE_CARD_WIDTH, |tree| {
                    tree.add(page_heading("Expandable retained rows"));
                    tree.add(body_text(
                        "Click a parent row or use Left/Right, Enter, and Space to expand and collapse.",
                    ));
                    tree.add(
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
                            .thumb_extent(34.0)
                            .tooltip("Virtualized tree projection from understory_outline"),
                    )
                });

                row.demo_responsive_card("ABOUT THE OUTLINE", PAGE_CARD_WIDTH, |detail| {
                    detail.add(page_heading("Virtualized tree"));
                    detail.add(body_text(
                        "Only the visible rows are realized. The control owns keyed expansion state, while scroll metrics come from the shared scroll-target registry.",
                    ));
                    detail.add(control_label("Keyboard"));
                    detail.add(body_text(
                        "Left and Right collapse and expand, Up and Down move the focused row, and Enter or Space toggles it.",
                    ));
                });
            },
        );
    }
}
