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
                row.demo_responsive_card("VALUE CONTROLS", PAGE_CARD_WIDTH, |controls| {
                    let slider_value = controls.local_state(0.46_f64);
                    let value_property = controls.ui().properties().value;

                    controls.with_child(Panel::row(12.0).align_center(), |toggle_row| {
                        toggle_row.add(ToggleSwitch::new().checked(true));
                        toggle_row.add(control_label("Toggle switch"));
                    });
                    controls.add(Divider::horizontal().length(PAGE_CONTROL_WIDTH));

                    controls.add(caption("Progress bar (timer-animated)"));
                    let progress = controls.add(ProgressBar::new().value(0.62).fill_width(18.0));
                    drive_progress(controls.ui_mut(), progress, 0.62, 0.015, 120_000_000);

                    // Two sliders sharing one bound value: the default template
                    // and a replacement template with the same behavior.
                    controls.add(caption("Slider \u{2014} default template"));
                    controls.add(
                        Slider::new()
                            .fill_width()
                            .bind_property(value_property, slider_value.binding()),
                    );
                    controls.add(caption("Slider \u{2014} custom template"));
                    controls.add(
                        Slider::new()
                            .template(template_slider_template())
                            .tooltip("Same slider behavior, replacement template")
                            .bind_property(value_property, slider_value.binding()),
                    );

                    controls.with_child(Panel::row(12.0).align_center(), |spinner_row| {
                        spinner_row.add(Spinner::new());
                        spinner_row.add(caption("Spinner"));
                    });
                });

                row.demo_responsive_card("ANIMATED COUNTER", PAGE_CARD_WIDTH, |telemetry| {
                    append_large_counter_to(telemetry, "Counter", "per minute", 90_000_000, 137);
                    telemetry.add(caption(
                        "Tabular numerals advanced by the retained timer queue, so digit columns \
                         stay aligned as the value changes.",
                    ));
                });
            },
        );
    }
}
