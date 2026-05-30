use super::super::*;

pub(super) fn build_motion_page(ui: &mut Ui, stage: ElementId) {
    let row = demo_row(ui, stage);

    let color = demo_card(ui, row, "COLOR MOTION", PAGE_CARD_WIDTH, 326.0);
    let color_lanes = ui.append(
        color,
        Panel::row(12.0).layout(Layout::row(12.0).logical_size(PAGE_CONTROL_WIDTH, 128.0)),
    );
    let oklab = ui.append(
        color_lanes,
        Panel::new()
            .class(CLASS_OKLAB_SWATCH)
            .layout(Layout::new().logical_size(158.0, 128.0)),
    );
    let srgb = ui.append(
        color_lanes,
        Panel::new()
            .class(CLASS_SRGB_SWATCH)
            .layout(Layout::new().logical_size(158.0, 128.0)),
    );
    let labels = ui.append(
        color,
        Panel::row(12.0).layout(Layout::row(12.0).logical_size(PAGE_CONTROL_WIDTH, 16.0)),
    );
    motion_lane_label(ui, labels, "OKLab");
    motion_lane_label(ui, labels, "sRGB");
    ui.append(
        color,
        body_text("Both lanes share endpoints. The middle of the journey exposes the interpolation policy."),
    );
    schedule_pair_toggle(ui, [oklab, srgb], PSEUDO_SELECTED, millis(5_200));

    let composition = demo_card(ui, row, "COMPOSITION MOTION", PAGE_CARD_WIDTH, 326.0);
    let strip = ui.append(
        composition,
        Panel::row(12.0).layout(Layout::row(12.0).logical_size(PAGE_CONTROL_WIDTH, 74.0)),
    );
    let opacity = motion_tile(ui, strip, CLASS_OPACITY_SWATCH);
    let border = motion_tile(ui, strip, CLASS_BORDER_SWATCH);
    let retarget = motion_tile(ui, strip, CLASS_RETARGET_SWATCH);
    ui.set_tooltip(
        retarget,
        Tooltip::new("Retargeting starts a new transition from the current sampled value"),
    );

    let motion_stage = ui.append(
        composition,
        Panel::new().class(CLASS_DEMO_MOTION_STAGE).layout(
            Layout::new()
                .relative()
                .logical_size(PAGE_CONTROL_WIDTH, 102.0)
                .padding_all(0.0),
        ),
    );
    let mover = ui.append(
        motion_stage,
        Panel::new()
            .class(CLASS_MOVER_SWATCH)
            .layout(mover_layout(false)),
    );
    ui.append(
        composition,
        caption("Opacity, border, retargeting, and layer offset all sample presentation values.")
            .wrap(),
    );

    schedule_toggle(ui, opacity, PSEUDO_PRESSED, millis(1_100));
    schedule_toggle(ui, border, PSEUDO_FOCUS_VISIBLE, millis(1_450));
    schedule_toggle(ui, retarget, PSEUDO_SELECTED, millis(180));
    schedule_movement(ui, mover, millis(1_250));
}

fn motion_tile(ui: &mut Ui, parent: ElementId, class: ClassId) -> ElementId {
    ui.append(
        parent,
        Panel::new()
            .class(class)
            .layout(Layout::new().logical_size(96.0, 74.0)),
    )
}

fn motion_lane_label(ui: &mut Ui, parent: ElementId, text: &str) -> ElementId {
    let id = ui.append(parent, Text::new(text).class(CLASS_DEMO_CARD_TITLE));
    ui.set_layout(id, Layout::new().logical_size(158.0, 16.0));
    id
}

fn schedule_toggle(ui: &mut Ui, target: ElementId, pseudo: PseudoClassId, interval: TimerDuration) {
    let active = Cell::new(false);
    let _ = ui.schedule_timer_repeating(target, interval);
    ui.on_timer(target, move |ui, _event| {
        let next = !active.get();
        active.set(next);
        set_pseudo(ui, target, pseudo, next);
    });
}

fn schedule_pair_toggle(
    ui: &mut Ui,
    targets: [ElementId; 2],
    pseudo: PseudoClassId,
    interval: TimerDuration,
) {
    let active = Cell::new(false);
    let _ = ui.schedule_timer_repeating(targets[0], interval);
    ui.on_timer(targets[0], move |ui, _event| {
        let next = !active.get();
        active.set(next);
        for target in targets {
            set_pseudo(ui, target, pseudo, next);
        }
    });
}

fn schedule_movement(ui: &mut Ui, target: ElementId, interval: TimerDuration) {
    let active = Cell::new(false);
    let _ = ui.schedule_timer_repeating(target, interval);
    ui.on_timer(target, move |ui, _event| {
        let next = !active.get();
        active.set(next);
        let _ = ui.request_layer_offset_transition(target);
        ui.set_layout(target, mover_layout(next));
    });
}

fn mover_layout(active: bool) -> Layout {
    let x = if active { 270.0 } else { 16.0 };
    Layout::new().absolute_at(x, 30.0).logical_size(42.0, 42.0)
}

fn set_pseudo(ui: &mut Ui, target: ElementId, pseudo: PseudoClassId, active: bool) {
    if active {
        ui.add_pseudo(target, pseudo);
    } else {
        ui.remove_pseudo(target, pseudo);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::pages::show_demo_page;

    #[test]
    fn motion_page_mover_timer_updates_layout_target() {
        let mut ui = build_ui();
        let stage = stage_element(&ui);
        show_demo_page(&mut ui, stage, DemoPage::Motion);
        ui.compute_layout(Size::new(
            f64::from(INITIAL_SIZE.0),
            f64::from(INITIAL_SIZE.1),
        ));

        let mover = find_by_class(&ui, ui.root(), CLASS_MOVER_SWATCH)
            .expect("motion page should contain the offset specimen");
        let start = ui.effective_bounds(mover);

        ui.pump_timers(millis(1_250));
        ui.compute_layout(Size::new(
            f64::from(INITIAL_SIZE.0),
            f64::from(INITIAL_SIZE.1),
        ));

        let after = ui.effective_bounds(mover);
        assert!(
            after.x0 > start.x0 + 100.0,
            "mover target should advance after its timer fires: {start:?} -> {after:?}",
        );
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

    fn find_by_class(ui: &Ui, element: ElementId, class: ClassId) -> Option<ElementId> {
        let current = ui.get(element)?;
        if current.classes().contains(&class) {
            return Some(element);
        }
        current
            .children()
            .iter()
            .find_map(|child| find_by_class(ui, *child, class))
    }
}
