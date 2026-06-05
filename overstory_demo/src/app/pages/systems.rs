use super::super::*;
use super::metrics::{append_large_counter_to, drive_progress};

#[derive(Clone, Copy, Debug)]
pub(super) struct SystemsPage;

fn template_slider_template() -> TemplateSpec {
    TemplateSpec::from_nodes([
        TemplateNode::surface_slot(SLOT_SLIDER_TRACK).with_class(CLASS_DEMO_TEMPLATE_SLIDER_TRACK),
        TemplateNode::surface_slot(SLOT_SLIDER_THUMB).with_class(CLASS_DEMO_TEMPLATE_SLIDER_THUMB),
    ])
}

impl View for SystemsPage {
    type Output = ();

    fn build(self, stage: &mut ChildUi<'_>) -> Self::Output {
        stage.with_child(
            Panel::new().layout(
                Layout::row(18.0)
                    .wrap()
                    .fill_width()
                    .alignment(LayoutAlignment::new().align_items(LayoutAlign::Start)),
            ),
            |row| {
                row.demo_auto_card("CONTROL SURFACE", PAGE_CARD_WIDTH, |controls| {
                    let slider_value = controls.local_state(0.46_f64);
                    let value_property = controls.ui().properties().value;
                    controls.with_child(Panel::row(12.0).align_center(), |toggle_row| {
                        toggle_row.add(ToggleSwitch::new().checked(true));
                        toggle_row.add(control_label("Rendering enabled"));
                    });
                    controls.add(Divider::horizontal().length(PAGE_CONTROL_WIDTH));
                    controls.add(caption("GPU Budget"));
                    let progress = controls.add(
                        ProgressBar::new()
                            .value(0.62)
                            .size(PAGE_CONTROL_WIDTH, 18.0)
                            .tooltip("Animated by the retained timer queue"),
                    );
                    drive_progress(controls.ui_mut(), progress, 0.62, 0.015, 120_000_000);
                    controls.add(caption("Volume"));
                    controls.with_child(Panel::column(6.0), |column| {
                        column.add(caption("Default template"));
                        column.add(
                            Slider::new()
                                .tooltip("Default slider template")
                                .bind_property(value_property, slider_value.binding()),
                        );
                    });
                    controls.with_child(Panel::column(6.0), |column| {
                        column.add(caption("Custom template"));
                        column.add(
                            Slider::new()
                                .template(template_slider_template())
                                .tooltip("Same slider behavior, replacement template")
                                .bind_property(value_property, slider_value.binding()),
                        );
                    });
                    controls.with_child(Panel::row(12.0).align_center(), |spinner_row| {
                        spinner_row.add(Spinner::new());
                        spinner_row.add(caption("Streaming updates"));
                    });
                });

                row.demo_auto_card("TELEMETRY", PAGE_CARD_WIDTH, |telemetry| {
                    append_large_counter_to(
                        telemetry,
                        "Throughput",
                        "events / min",
                        90_000_000,
                        137,
                    );
                });
            },
        );
    }
}
