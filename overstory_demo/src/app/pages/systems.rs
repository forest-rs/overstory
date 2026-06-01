use super::super::*;
use super::metrics::{append_large_counter_to, drive_progress};

#[derive(Clone, Copy, Debug)]
pub(super) struct SystemsPage;

impl View for SystemsPage {
    type Output = ();

    fn build(self, stage: &mut ChildUi<'_>) -> Self::Output {
        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            |row| {
                row.demo_card("CONTROL SURFACE", PAGE_CARD_WIDTH, 322.0, |controls| {
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
                    controls.add(
                        Slider::new()
                            .value(0.42)
                            .tooltip("Drag, arrow, PageUp/PageDown, or Shift+arrow"),
                    );
                    controls.with_child(Panel::row(12.0).align_center(), |spinner_row| {
                        spinner_row.add(Spinner::new());
                        spinner_row.add(caption("Streaming updates"));
                    });
                });

                row.demo_card("TELEMETRY", PAGE_CARD_WIDTH, 218.0, |telemetry| {
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
