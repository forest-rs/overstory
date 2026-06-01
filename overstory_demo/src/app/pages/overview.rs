use super::super::*;
use super::metrics::{append_tabular_counter_to, drive_progress};

#[derive(Clone, Copy, Debug)]
pub(super) struct OverviewPage;

impl View for OverviewPage {
    type Output = ();

    fn build(self, stage: &mut ChildUi<'_>) -> Self::Output {
        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            |row| {
                row.demo_card("INPUTS", OVERVIEW_CARD_WIDTH, 222.0, |inputs| {
                    inputs.add(
                        TextInput::new()
                            .placeholder("Search actions")
                            .width(OVERVIEW_CONTROL_WIDTH),
                    );
                    inputs.with_child(Panel::row(10.0).align_center(), |actions| {
                        let deploy_clicks = actions.ui_mut().state(0_u32);
                        actions.add(
                            Button::new("")
                                .font_size(15.0)
                                .on_click(cycling_click_handler(deploy_clicks))
                                .bind_content(
                                    deploy_clicks
                                        .map(|count| cycling_click_label("Deploy", *count)),
                                )
                                .tooltip("Button content is template-bound"),
                        );
                        actions.add(Button::new("Preview").font_size(15.0));
                    });
                    inputs.add(CheckBox::new("Record diagnostics").checked(true));
                });

                row.demo_card("SYSTEMS", OVERVIEW_CARD_WIDTH, 222.0, |systems| {
                    systems.with_child(Panel::row(12.0).align_center(), |toggle_row| {
                        toggle_row.add(ToggleSwitch::new().checked(true));
                        toggle_row.add(control_label("Rendering enabled"));
                    });
                    systems.add(caption("GPU Budget"));
                    let progress = systems.add(
                        ProgressBar::new()
                            .value(0.62)
                            .size(OVERVIEW_CONTROL_WIDTH, 18.0),
                    );
                    drive_progress(systems.ui_mut(), progress, 0.62, 0.015, 120_000_000);
                    append_tabular_counter_to(systems, "Frames", "fps", 120_000_000, 7);
                });

                row.demo_card("SELECTION", OVERVIEW_CARD_WIDTH, 222.0, |selection| {
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
                });
            },
        );
    }
}
