use super::super::*;

pub(super) fn drive_progress(
    ui: &mut Ui,
    progress: ElementId,
    initial: f64,
    step: f64,
    interval: u64,
) {
    let _ = ui.schedule_timer_repeating(progress, interval);
    let mut progress_value = initial;
    ui.on_timer(progress, move |ui, _event| {
        progress_value += step;
        if progress_value > 1.0 {
            progress_value = 0.08;
        }
        set_progress_value(ui, progress, progress_value);
    });
}

pub(super) fn append_tabular_counter_to(
    parent: &mut ChildUi<'_>,
    label: &str,
    suffix: &str,
    interval: u64,
    step: u32,
) -> ElementId {
    parent.add(caption(label));
    let value = parent.local_state(0_u32);
    let counter = parent.with_child(Panel::row(8.0).align_center(), |row| {
        let counter = row.add(
            number_text("")
                .font_features(tabular_number_features())
                .bind_text(value.map(|value| format!("{value:04}")))
                .tooltip("Tabular figures keep neighboring text steady"),
        );
        row.add(caption(suffix));
        counter
    });
    drive_counter(parent.ui_mut(), counter, value, interval, step);
    counter
}

pub(super) fn append_large_counter_to(
    parent: &mut ChildUi<'_>,
    label: &str,
    suffix: &str,
    interval: u64,
    step: u32,
) -> ElementId {
    parent.add(caption(label));
    let value = parent.local_state(0_u32);
    let counter = parent.with_child(Panel::row(12.0).align_center(), |row| {
        let counter = row.add(
            large_number_text("")
                .font_features(tabular_number_features())
                .bind_text(value.map(|value| format!("{value:04}")))
                .tooltip("Tabular figures keep neighboring text steady"),
        );
        row.add(caption(suffix));
        counter
    });
    drive_counter(parent.ui_mut(), counter, value, interval, step);
    counter
}

fn drive_counter(
    ui: &mut Ui,
    counter: ElementId,
    state: LocalState<u32>,
    interval: u64,
    step: u32,
) {
    let _ = ui.schedule_timer_repeating(counter, interval);
    let mut value = 0_u32;
    ui.on_timer(counter, move |ui, _event| {
        value = (value + step) % 10_000;
        state.set(ui, value);
    });
}
