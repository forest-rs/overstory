use super::super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct InputsPage;

impl View for InputsPage {
    type Output = ();

    fn build(self, stage: &mut ChildUi<'_>) -> Self::Output {
        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            |row| {
                row.demo_card("TEXT INPUTS", PAGE_CARD_WIDTH, 224.0, |forms| {
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

                row.demo_card("ACTIONS", PAGE_CARD_WIDTH, 224.0, |actions| {
                    actions.with_child(Panel::row(10.0).align_center(), |buttons| {
                        let deploy_clicks = buttons.ui_mut().state(0_u32);
                        buttons.add(
                            Button::new("")
                                .font_size(15.0)
                                .on_click(cycling_click_handler(deploy_clicks))
                                .bind_content(
                                    deploy_clicks
                                        .map(|count| cycling_click_label("Deploy", *count)),
                                )
                                .tooltip("Run the staged deployment workflow"),
                        );
                        buttons.add(
                            Button::new("Review")
                                .font_size(15.0)
                                .on_click(popup_click_handler())
                                .tooltip("Toggle an anchored popup"),
                        );
                    });
                    actions.add(Divider::horizontal().length(PAGE_CONTROL_WIDTH));
                    actions.add(CheckBox::new("Enable staged rollout").checked(true));
                    actions.add(CheckBox::new("Collect timing traces"));
                    actions.add(CheckBox::new("Send diagnostics").disabled());
                });
            },
        );

        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            |row| {
                row.demo_fit_card("RADIO GROUP", PAGE_CARD_WIDTH, 214.0, |radios| {
                    let summary = radios
                        .ui_mut()
                        .state(String::from("Release lane: Balanced"));
                    radios.add(caption("").bind_content(summary.binding()));
                    radios.add(
                        RadioGroup::new()
                            .items(["Balanced", "Quality", "Low latency"])
                            .selected_index(0)
                            .on_selection_changed(move |ui, event| {
                                let selected_item =
                                    event.selected_item.unwrap_or_else(|| "None".to_owned());
                                summary.set(ui, format!("Release lane: {selected_item}"));
                            }),
                    );
                });
                row.demo_fit_card("SEGMENTED", PAGE_CARD_WIDTH, 154.0, |segments| {
                    let summary = segments.ui_mut().state(String::from("View mode: Preview"));
                    segments.add(caption("").bind_content(summary.binding()));
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
                                let selected_item =
                                    event.selected_item.unwrap_or_else(|| "None".to_owned());
                                summary.set(ui, format!("View mode: {selected_item}"));
                            }),
                    );
                });
            },
        );
    }
}
