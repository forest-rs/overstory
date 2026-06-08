use super::super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct MotionPage;

impl View for MotionPage {
    type Output = ();

    fn build(self, stage: &mut ChildUi<'_>) -> Self::Output {
        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            |row| {
                row.demo_card("COLOR MOTION", PAGE_CARD_WIDTH, 376.0, |color| {
                    let [oklab, srgb] = color.with_child(
                        Panel::row(12.0)
                            .layout(Layout::row(12.0).logical_size(PAGE_CONTROL_WIDTH, 128.0)),
                        |lanes| {
                            [
                                lanes.add(
                                    Panel::new()
                                        .class(CLASS_OKLAB_SWATCH)
                                        .layout(Layout::new().logical_size(158.0, 128.0)),
                                ),
                                lanes.add(
                                    Panel::new()
                                        .class(CLASS_SRGB_SWATCH)
                                        .layout(Layout::new().logical_size(158.0, 128.0)),
                                ),
                            ]
                        },
                    );
                    color.with_child(
                        Panel::row(12.0)
                            .layout(Layout::row(12.0).logical_size(PAGE_CONTROL_WIDTH, 16.0)),
                        |labels| {
                            motion_lane_label(labels, "OKLab");
                            motion_lane_label(labels, "sRGB");
                        },
                    );
                    let foreground = foreground_specimen(color);
                    color.add(
                        body_text("Both lanes share endpoints. The midpoint exposes the interpolation policy while text rides the same state rhythm."),
                    );
                    schedule_pair_toggle(
                        color.ui_mut(),
                        [oklab, srgb],
                        PSEUDO_SELECTED,
                        millis(5_200),
                    );
                    schedule_toggle(
                        color.ui_mut(),
                        foreground,
                        PSEUDO_SELECTED,
                        millis(2_600),
                    );
                });

                row.demo_card("COMPOSITION MOTION", PAGE_CARD_WIDTH, 326.0, |composition| {
                    let [opacity, border, retarget] = composition.with_child(
                        Panel::row(12.0)
                            .layout(Layout::row(12.0).logical_size(PAGE_CONTROL_WIDTH, 74.0)),
                        |strip| {
                            [
                                strip.add(
                                    Panel::new()
                                        .class(CLASS_OPACITY_SWATCH)
                                        .layout(Layout::new().logical_size(96.0, 74.0)),
                                ),
                                strip.add(
                                    Panel::new()
                                        .class(CLASS_BORDER_SWATCH)
                                        .layout(Layout::new().logical_size(96.0, 74.0)),
                                ),
                                strip.add(
                                    Panel::new()
                                        .class(CLASS_RETARGET_SWATCH)
                                        .layout(Layout::new().logical_size(96.0, 74.0))
                                        .tooltip("Retargeting starts a new transition from the current sampled value"),
                                ),
                            ]
                        },
                    );

                    let mover = composition.with_child(
                        Panel::new().class(CLASS_DEMO_MOTION_STAGE).layout(
                            Layout::new()
                                .relative()
                                .logical_size(PAGE_CONTROL_WIDTH, 102.0),
                        ),
                        |motion_stage| {
                            motion_stage.add(
                                Panel::new()
                                    .class(CLASS_MOVER_SWATCH)
                                    .layout(mover_layout(false)),
                            )
                        },
                    );
                    composition.add(
                        caption("Opacity, border, retargeting, and layer offset all sample presentation values.")
                            .wrap(),
                    );

                    schedule_toggle(
                        composition.ui_mut(),
                        opacity,
                        PSEUDO_PRESSED,
                        millis(1_100),
                    );
                    schedule_toggle(
                        composition.ui_mut(),
                        border,
                        PSEUDO_FOCUS_VISIBLE,
                        millis(1_450),
                    );
                    schedule_toggle(composition.ui_mut(), retarget, PSEUDO_SELECTED, millis(180));
                    schedule_movement(composition.ui_mut(), mover, millis(1_250));
                });
            },
        );
    }
}

fn foreground_specimen(parent: &mut ChildUi<'_>) -> ElementId {
    parent.with_child(
        Panel::row(0.0).class(CLASS_DEMO_MOTION_STAGE).layout(
            Layout::row(0.0)
                .logical_size(PAGE_CONTROL_WIDTH, 48.0)
                .alignment(
                    LayoutAlignment::new()
                        .align_items(LayoutAlign::Center)
                        .justify_content(LayoutJustify::Center),
                ),
        ),
        |stage| stage.add(Text::new("Foreground signal").class(CLASS_FOREGROUND_SWATCH)),
    )
}

fn motion_lane_label(parent: &mut ChildUi<'_>, text: &str) -> ElementId {
    let id = parent.add(Text::new(text).class(CLASS_DEMO_CARD_TITLE));
    parent
        .ui_mut()
        .set_layout(id, Layout::new().logical_size(158.0, 16.0));
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
    use peniko::kurbo::Size;

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

    #[test]
    fn motion_page_foreground_timer_updates_text_state() {
        let mut ui = build_ui();
        let stage = stage_element(&ui);
        show_demo_page(&mut ui, stage, DemoPage::Motion);
        ui.compute_layout(Size::new(
            f64::from(INITIAL_SIZE.0),
            f64::from(INITIAL_SIZE.1),
        ));

        let foreground = find_by_class(&ui, ui.root(), CLASS_FOREGROUND_SWATCH)
            .expect("motion page should contain the foreground specimen");
        assert!(
            !ui.get(foreground)
                .expect("foreground specimen should exist")
                .pseudos()
                .contains(&PSEUDO_SELECTED),
            "foreground specimen should start idle",
        );

        ui.pump_timers(millis(2_600));
        ui.compute_layout(Size::new(
            f64::from(INITIAL_SIZE.0),
            f64::from(INITIAL_SIZE.1),
        ));

        assert!(
            ui.get(foreground)
                .expect("foreground specimen should exist")
                .pseudos()
                .contains(&PSEUDO_SELECTED),
            "foreground specimen timer should toggle selected state",
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

    fn find_by_class(
        ui: &Ui,
        element: ElementId,
        class: impl Into<overstory::StyleClass>,
    ) -> Option<ElementId> {
        let class = class.into().registered_id(ui.style_vocabulary())?;
        find_by_class_id(ui, element, class)
    }

    fn find_by_class_id(ui: &Ui, element: ElementId, class: ClassId) -> Option<ElementId> {
        let current = ui.get(element)?;
        if current.classes().contains(&class) {
            return Some(element);
        }
        current
            .children()
            .iter()
            .find_map(|child| find_by_class_id(ui, *child, class))
    }
}
