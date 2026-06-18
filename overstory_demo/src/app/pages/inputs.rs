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
                row.demo_responsive_card("TEXT FIELDS", PAGE_CARD_WIDTH, |forms| {
                    forms.add(
                        TextInput::new()
                            .placeholder("Placeholder text")
                            .fill_width(),
                    );
                    forms.add(TextInput::new().text("Editable value").fill_width());
                    forms.add(
                        TextInput::new()
                            .text("Read-only field")
                            .read_only(true)
                            .fill_width(),
                    );
                    forms.add(
                        TextInput::new()
                            .text("Disabled field")
                            .disabled()
                            .fill_width(),
                    );
                    forms.add(caption(
                        "Placeholder, editable, read-only, and disabled states. Focus and hover \
                         draw from the active theme.",
                    ));
                });

                row.demo_responsive_card("BUTTONS & CHECKS", PAGE_CARD_WIDTH, |actions| {
                    actions.with_child(
                        Panel::new().layout(Layout::row(10.0).wrap().align_center()),
                        |buttons| {
                            let activations = buttons.local_state(0_u32);
                            // A button wearing a custom two-surface template; behavior and
                            // bound content are unchanged.
                            buttons.content(
                                Button::new("")
                                    .template(double_frame_button_template())
                                    .font_size(15.0)
                                    .layout(Layout::row(0.0).align_center())
                                    .background(None::<Brush>)
                                    .border_brush(None::<Brush>)
                                    .border_width(0.0)
                                    .corner_radius(0.0)
                                    .on_click(cycling_click_handler(activations))
                                    .tooltip("Same button behavior, replacement template"),
                                |content| {
                                    content.with_child(Panel::row(8.0).align_center(), |label| {
                                        label.add(
                                            Panel::new()
                                                .layout(Layout::new().logical_size(8.0, 8.0))
                                                .background(solid(0x9d, 0xf8, 0xb0, 0xff))
                                                .corner_radius(4.0),
                                        );
                                        label.add(
                                            Text::new("").font_size(15.0).bind_text(
                                                activations.map(|count| {
                                                    cycling_click_label("Deploy", *count)
                                                }),
                                            ),
                                        );
                                    });
                                },
                            );
                            buttons.add(Button::new("Default").font_size(15.0));
                            buttons.add(Button::new("Disabled").font_size(15.0).disabled());
                            buttons.add(
                                Button::new("Popup")
                                    .font_size(15.0)
                                    .on_click(popup_click_handler())
                                    .tooltip("Anchored popup"),
                            );
                        },
                    );
                    actions.add(Divider::horizontal().length(PAGE_CONTROL_WIDTH));
                    // A check box wearing a custom box template; checked behavior is
                    // unchanged.
                    actions.content(
                        CheckBox::new("")
                            .checked(true)
                            .template(round_check_box_template()),
                        |label| {
                            label.with_child(Panel::row(8.0).align_center(), |row| {
                                row.add(Text::new("Custom box template").font_size(15.0));
                                row.add(caption("checked"));
                            });
                        },
                    );
                    actions.add(CheckBox::new("Unchecked"));
                    actions.add(CheckBox::new("Disabled").disabled());
                });
            },
        );

        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            |row| {
                row.demo_responsive_card("RADIO GROUP", PAGE_CARD_WIDTH, |radios| {
                    let summary = radios.local_state(String::from("Selected: Balanced"));
                    radios.add(caption("").bind_text(summary.binding()));
                    radios.add(
                        RadioGroup::new()
                            .items(["Balanced", "Quality", "Low latency"])
                            .selected_index(0)
                            .on_selection_changed(move |ui, event| {
                                let selected_item =
                                    event.selected_item.unwrap_or_else(|| "None".to_owned());
                                summary.set(ui, format!("Selected: {selected_item}"));
                            }),
                    );
                    radios.content(RadioButton::new("").disabled(), |label| {
                        label.with_child(Panel::row(8.0).align_center(), |row| {
                            row.add(Text::new("Disabled option").font_size(15.0));
                        });
                    });
                });
                row.demo_responsive_card("SEGMENTED", PAGE_CARD_WIDTH, |segments| {
                    let summary = segments.local_state(String::from("Selected: Preview"));
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
                                summary.set(ui, format!("Selected: {selected_item}"));
                            }),
                    );
                    segments.add(caption("One segment disabled; selection skips it."));
                });
            },
        );
    }
}
