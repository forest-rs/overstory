use super::super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct InputsPage;

fn double_frame_button_template() -> TemplateSpec {
    TemplateSpec::from_nodes([TemplateNode::surface()
        .with_class(CLASS_DEMO_TEMPLATE_BUTTON_OUTER)
        .with_child(
            TemplateNode::surface()
                .with_class(CLASS_DEMO_TEMPLATE_BUTTON_INNER)
                .with_child(TemplateNode::content_presenter(SLOT_BUTTON_CONTENT)),
        )])
}

fn round_check_box_template() -> TemplateSpec {
    TemplateSpec::from_nodes([
        TemplateNode::surface_slot(SLOT_CHECK_BOX_BOX)
            .with_class(CLASS_DEMO_TEMPLATE_CHECK_BOX_BOX)
            .with_child(TemplateNode::text_slot(SLOT_CHECK_BOX_MARK)),
        TemplateNode::content_presenter(SLOT_CHECK_BOX_LABEL),
    ])
}

impl View for InputsPage {
    type Output = ();

    fn build(self, stage: &mut ChildUi<'_>) -> Self::Output {
        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            |row| {
                row.demo_fit_card("TEXT INPUTS", PAGE_CARD_WIDTH, 224.0, |forms| {
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

                row.demo_fit_card("ACTIONS", PAGE_CARD_WIDTH, 224.0, |actions| {
                    actions.with_child(Panel::row(10.0).align_center(), |buttons| {
                        let deploy_clicks = buttons.local_state(0_u32);
                        buttons.content(
                            Button::new("")
                                .template(double_frame_button_template())
                                .font_size(15.0)
                                .layout(Layout::row(0.0).align_center())
                                .background(None::<Brush>)
                                .border_brush(None::<Brush>)
                                .border_width(0.0)
                                .corner_radius(0.0)
                                .on_click(cycling_click_handler(deploy_clicks))
                                .tooltip("Run the staged deployment workflow"),
                            |content| {
                                content.with_child(Panel::row(8.0).align_center(), |label| {
                                    label.add(
                                        Panel::new()
                                            .layout(Layout::new().logical_size(8.0, 8.0))
                                            .background(solid(0x9d, 0xf8, 0xb0, 0xff))
                                            .corner_radius(4.0),
                                    );
                                    label.add(
                                        Text::new("")
                                            .font_size(15.0)
                                            .bind_text(deploy_clicks.map(|count| {
                                                cycling_click_label("Deploy", *count)
                                            })),
                                    );
                                });
                            },
                        );
                        buttons.add(
                            Button::new("Review")
                                .font_size(15.0)
                                .on_click(popup_click_handler())
                                .tooltip("Toggle an anchored popup"),
                        );
                    });
                    actions.add(Divider::horizontal().length(PAGE_CONTROL_WIDTH));
                    actions.content(
                        CheckBox::new("")
                            .checked(true)
                            .template(round_check_box_template()),
                        |label| {
                            label.with_child(Panel::row(8.0).align_center(), |row| {
                                row.add(Text::new("Enable staged rollout").font_size(15.0));
                                row.add(caption("template slot"));
                            });
                        },
                    );
                    actions.add(CheckBox::new("Collect timing traces"));
                    actions.add(CheckBox::new("Send diagnostics").disabled());
                });
            },
        );

        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            |row| {
                row.demo_fit_card("RADIO GROUP", PAGE_CARD_WIDTH, 214.0, |radios| {
                    let summary = radios.local_state(String::from("Release lane: Balanced"));
                    radios.add(caption("").bind_text(summary.binding()));
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
                    radios.content(RadioButton::new("").disabled(), |label| {
                        label.with_child(Panel::row(8.0).align_center(), |row| {
                            row.add(Text::new("Canary").font_size(15.0));
                            row.add(caption("retained label"));
                        });
                    });
                });
                row.demo_fit_card("SEGMENTED", PAGE_CARD_WIDTH, 154.0, |segments| {
                    let summary = segments.local_state(String::from("View mode: Preview"));
                    segments.add(caption("").bind_text(summary.binding()));
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
