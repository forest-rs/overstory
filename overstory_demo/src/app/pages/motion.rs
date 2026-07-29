use super::super::*;
use overstory::{
    AnimationEventKind, AnimationId, AnimationTarget, AnimationTiming, CompositeOperation,
    EffectClip, EffectKeyframe, FillMode, PlaybackDirection,
};

const RUNTIME_TRAVEL_MS: u64 = 1_260;
const RUNTIME_WOBBLE_MS: u64 = 1_200;
const MANUAL_SCRUB_MS: u64 = 1_600;
const MANUAL_SCRUB_VALUE: f64 = 1_600.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct MotionPage;

#[derive(Clone, Copy, Debug)]
struct RuntimeAnimationHandle {
    id: AnimationId,
    reverse_from: TimerDuration,
}

impl View for MotionPage {
    type Output = ();

    fn build(self, stage: &mut ChildUi<'_>) -> Self::Output {
        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            build_animation_runtime_card,
        );

        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            |row| {
                row.demo_responsive_card("COLOR MOTION", PAGE_CARD_WIDTH, |color| {
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

                build_expression_color_card(row);

                row.demo_responsive_card("COMPOSITION MOTION", PAGE_CARD_WIDTH, |composition| {
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

fn build_animation_runtime_card(row: &mut ChildUi<'_>) {
    row.demo_responsive_card("ANIMATION RUNTIME", PAGE_CARD_WIDTH, |runtime| {
        let status = runtime.local_state(String::from("No retained animation yet"));
        let active = Rc::new(RefCell::new(Vec::<RuntimeAnimationHandle>::new()));

        runtime.with_child(
            Panel::row(18.0).layout(Layout::row(18.0).wrap().fill_width()),
            |lanes| {
                lanes.with_child(motion_runtime_lane(), |lane| {
                    lane.add(caption("Retained target stack"));
                    let swatch = lane.with_child(motion_runtime_stage(), |stage| {
                        add_motion_rail(stage);
                        stage.add(
                            Panel::new()
                                .class(CLASS_RUNTIME_MOTION_SWATCH)
                                .layout(motion_dot_layout(18.0, 25.0)),
                        )
                    });
                    lane.with_child(
                        Panel::row(8.0).layout(Layout::row(8.0).fill_width()),
                        |controls| {
                            controls.add(Button::new("Run stack").font_size(13.0).on_activate({
                                let active = Rc::clone(&active);
                                move |ui, _event| {
                                    start_runtime_stack(ui, swatch, &active);
                                    set_motion_status(ui, status, "started");
                                }
                            }));
                            controls.add(Button::new("Pause").font_size(13.0).on_activate({
                                let active = Rc::clone(&active);
                                move |ui, _event| {
                                    for handle in active.borrow().iter().copied() {
                                        let _ = ui.pause_animation(handle.id);
                                    }
                                    set_motion_status(ui, status, "paused");
                                }
                            }));
                            controls.add(Button::new("Resume").font_size(13.0).on_activate({
                                let active = Rc::clone(&active);
                                move |ui, _event| {
                                    for handle in active.borrow().iter().copied() {
                                        let _ = ui.play_animation(handle.id);
                                    }
                                    set_motion_status(ui, status, "resumed");
                                }
                            }));
                            controls.add(Button::new("Reverse").font_size(13.0).on_activate({
                                let active = Rc::clone(&active);
                                move |ui, _event| {
                                    reverse_runtime_stack(ui, &active);
                                    set_motion_status(ui, status, "reversed");
                                }
                            }));
                            controls.add(Button::new("Finish").font_size(13.0).on_activate({
                                let active = Rc::clone(&active);
                                move |ui, _event| {
                                    finish_runtime_stack(ui, &active);
                                    set_motion_status(ui, status, "finished");
                                }
                            }));
                            controls.add(Button::new("Cancel").font_size(13.0).on_activate({
                                let active = Rc::clone(&active);
                                move |ui, _event| {
                                    cancel_runtime_stack(ui, &active);
                                    set_motion_status(ui, status, "canceled");
                                }
                            }));
                        },
                    );

                    lane.add(
                        Text::new("")
                            .class(CLASS_MOTION_STATUS)
                            .wrap()
                            .bind_text(status.binding()),
                    );
                });

                lanes.with_child(motion_runtime_lane(), |lane| {
                    let manual_status = lane.local_state(String::from("manual timeline: 0 ms"));
                    let manual_timeline = lane.ui_mut().create_manual_animation_timeline();
                    lane.add(caption("Manual timeline scrub"));
                    let manual_swatch = lane.with_child(motion_runtime_stage(), |stage| {
                        add_motion_rail(stage);
                        stage.add(
                            Panel::new()
                                .class(CLASS_MANUAL_MOTION_SWATCH)
                                .layout(motion_dot_layout(18.0, 25.0)),
                        )
                    });
                    let _ = lane.ui_mut().animate_target_on_timeline(
                        manual_swatch,
                        AnimationTarget::visual_translation(),
                        manual_scrub_clip(),
                        manual_timeline,
                    );
                    let _ = lane.ui_mut().drain_animation_events();

                    lane.add(
                        Slider::new()
                            .range(0.0, MANUAL_SCRUB_VALUE)
                            .step(80.0)
                            .value(0.0)
                            .fill_width()
                            .on_value_changed(move |ui, event| {
                                let time = rounded_millis(event.value);
                                let _ = ui
                                    .seek_manual_animation_timeline(manual_timeline, millis(time));
                                manual_status.set(ui, format!("manual timeline: {time} ms"));
                            }),
                    );
                    lane.add(
                        Text::new("")
                            .class(CLASS_MOTION_STATUS)
                            .bind_text(manual_status.binding()),
                    );
                });
            },
        );
    });
}

fn build_expression_color_card(row: &mut ChildUi<'_>) {
    row.demo_responsive_card("EXPRESSION COLOR", PAGE_CARD_WIDTH, |expression| {
        let amount = expression.local_state(0.52_f64);
        let status = expression.local_state(String::from("bound value: 0.52"));
        let value_property = expression.ui().properties().value;

        expression.add(
            Panel::new()
                .class(CLASS_EXPRESSION_SWATCH)
                .layout(Layout::new().logical_size(PAGE_CONTROL_WIDTH, 92.0))
                .bind_property(value_property, amount.binding()),
        );
        expression.add(
            Slider::new()
                .range(0.0, 1.0)
                .step(0.05)
                .value(0.52)
                .fill_width()
                .on_value_changed(move |ui, event| {
                    amount.set(ui, event.value);
                    status.set(ui, format!("bound value: {:.2}", event.value));
                }),
        );
        expression.add(
            Text::new("")
                .class(CLASS_MOTION_STATUS)
                .bind_text(status.binding()),
        );
        expression.add(
            caption(
                "The slider writes local state; binding writes Value; the style expression resolves the color.",
            )
            .wrap(),
        );
    });
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
        ui.with_motion(
            MotionSpec::new(millis(920)).with_easing(EASE_IN_OUT),
            |ui| ui.set_layout(target, mover_layout(next)),
        );
    });
}

fn mover_layout(active: bool) -> Layout {
    let x = if active { 270.0 } else { 16.0 };
    Layout::new().absolute_at(x, 30.0).logical_size(42.0, 42.0)
}

fn motion_runtime_stage() -> Panel {
    Panel::new().class(CLASS_DEMO_MOTION_STAGE).layout(
        Layout::new()
            .relative()
            .logical_size(PAGE_CONTROL_WIDTH, 86.0),
    )
}

fn motion_runtime_lane() -> Panel {
    Panel::column(10.0).layout(
        Layout::column(10.0)
            .size(
                LayoutSize::new()
                    .width(LayoutDimension::Logical(PAGE_CONTROL_WIDTH))
                    .min_width(LayoutDimension::Logical(PAGE_CONTROL_WIDTH)),
            )
            .flex_grow(1.0)
            .flex_shrink(1.0),
    )
}

fn add_motion_rail(stage: &mut ChildUi<'_>) {
    stage.add(
        Panel::new()
            .background(solid(0xff, 0xff, 0xff, 0x2d))
            .corner_radius(2.0)
            .layout(
                Layout::new()
                    .absolute_at(18.0, 42.0)
                    .logical_size(292.0, 4.0),
            ),
    );
}

fn motion_dot_layout(x: f64, y: f64) -> Layout {
    Layout::new().absolute_at(x, y).logical_size(34.0, 34.0)
}

fn rounded_millis(value: f64) -> u64 {
    if !value.is_finite() {
        return 0;
    }

    let value = value.clamp(0.0, MANUAL_SCRUB_VALUE);
    let mut time = 0;
    let mut midpoint = 40.0;
    while value >= midpoint && time < MANUAL_SCRUB_MS {
        time += 80;
        midpoint += 80.0;
    }
    time
}

fn runtime_travel_clip() -> EffectClip<Vec2> {
    EffectClip::from_keyframes([
        EffectKeyframe::new(millis(0), Vec2::ZERO),
        EffectKeyframe::new(millis(820), Vec2::new(240.0, 0.0)),
        EffectKeyframe::new(millis(RUNTIME_TRAVEL_MS), Vec2::new(188.0, 0.0)),
    ])
    .with_timing(AnimationTiming {
        duration: millis(RUNTIME_TRAVEL_MS),
        fill: FillMode::Forwards,
        easing: EASE_OUT,
        ..AnimationTiming::new(millis(RUNTIME_TRAVEL_MS))
    })
}

fn runtime_wobble_clip() -> EffectClip<Vec2> {
    EffectClip::from_keyframes([
        EffectKeyframe::new(millis(0), Vec2::ZERO),
        EffectKeyframe::new(millis(120), Vec2::new(0.0, -14.0)),
        EffectKeyframe::new(millis(240), Vec2::ZERO),
    ])
    .with_timing(AnimationTiming {
        duration: millis(240),
        iterations: 5.0,
        direction: PlaybackDirection::Alternate,
        fill: FillMode::None,
        easing: EASE_IN_OUT,
        ..AnimationTiming::new(millis(240))
    })
    .with_composite(CompositeOperation::Add)
}

fn manual_scrub_clip() -> EffectClip<Vec2> {
    EffectClip::from_keyframes([
        EffectKeyframe::new(millis(0), Vec2::ZERO),
        EffectKeyframe::new(millis(600), Vec2::new(96.0, 0.0)),
        EffectKeyframe::new(millis(1_100), Vec2::new(210.0, 0.0)),
        EffectKeyframe::new(millis(MANUAL_SCRUB_MS), Vec2::new(142.0, 0.0)),
    ])
    .with_timing(AnimationTiming {
        duration: millis(MANUAL_SCRUB_MS),
        fill: FillMode::Both,
        easing: EASE_IN_OUT,
        ..AnimationTiming::new(millis(MANUAL_SCRUB_MS))
    })
}

fn start_runtime_stack(
    ui: &mut Ui,
    swatch: ElementId,
    active: &RefCell<Vec<RuntimeAnimationHandle>>,
) {
    cancel_active_animations(ui, active);
    let mut handles = active.borrow_mut();
    if let Some(id) = ui.animate_target(
        swatch,
        AnimationTarget::visual_translation(),
        runtime_travel_clip(),
    ) {
        handles.push(RuntimeAnimationHandle {
            id,
            reverse_from: millis(RUNTIME_TRAVEL_MS),
        });
    }
    if let Some(id) = ui.animate_target(
        swatch,
        AnimationTarget::visual_translation(),
        runtime_wobble_clip(),
    ) {
        handles.push(RuntimeAnimationHandle {
            id,
            reverse_from: millis(RUNTIME_WOBBLE_MS),
        });
    }
}

fn finish_runtime_stack(ui: &mut Ui, active: &RefCell<Vec<RuntimeAnimationHandle>>) {
    for handle in active.borrow_mut().drain(..) {
        let _ = ui.finish_animation(handle.id);
    }
}

fn cancel_runtime_stack(ui: &mut Ui, active: &RefCell<Vec<RuntimeAnimationHandle>>) {
    for handle in active.borrow_mut().drain(..) {
        let _ = ui.cancel_animation(handle.id);
    }
}

fn reverse_runtime_stack(ui: &mut Ui, active: &RefCell<Vec<RuntimeAnimationHandle>>) {
    for handle in active.borrow().iter().copied() {
        if ui
            .animation_current_time(handle.id)
            .is_some_and(|time| time == 0)
        {
            let _ = ui.seek_animation(handle.id, handle.reverse_from);
        }
        let _ = ui.reverse_animation(handle.id);
    }
}

fn cancel_active_animations(ui: &mut Ui, active: &RefCell<Vec<RuntimeAnimationHandle>>) {
    cancel_runtime_stack(ui, active);
    let _ = ui.drain_animation_events();
}

fn set_motion_status(ui: &mut Ui, status: LocalState<String>, action: &str) {
    let events = ui.drain_animation_events();
    let summary = if events.is_empty() {
        String::from("no lifecycle events")
    } else {
        events
            .iter()
            .map(|event| match event.kind() {
                AnimationEventKind::Started => format!("#{} started", event.id().get()),
                AnimationEventKind::Iteration => format!(
                    "#{} iteration {}",
                    event.id().get(),
                    event.iteration().unwrap_or_default()
                ),
                AnimationEventKind::Finished => format!("#{} finished", event.id().get()),
                AnimationEventKind::Canceled => format!("#{} canceled", event.id().get()),
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    status.set(ui, format!("{action}: {summary}"));
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
    use overstory::PresentationTarget;
    use understory_property::DependencyObjectExt;

    #[test]
    fn motion_page_mover_timer_updates_layout_target() {
        let mut ui = build_ui();
        let stage = stage_element(&ui);
        show_demo_page(&mut ui, stage, DemoPage::Motion);
        ui.compute_layout(INITIAL_VIEWPORT_SIZE);

        let mover = find_by_class(&ui, ui.root(), CLASS_MOVER_SWATCH)
            .expect("motion page should contain the offset specimen");
        let start = ui.effective_bounds(mover);

        ui.pump_timers(millis(1_250));
        ui.compute_layout(INITIAL_VIEWPORT_SIZE);

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
        ui.compute_layout(INITIAL_VIEWPORT_SIZE);

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
        ui.compute_layout(INITIAL_VIEWPORT_SIZE);

        assert!(
            ui.get(foreground)
                .expect("foreground specimen should exist")
                .pseudos()
                .contains(&PSEUDO_SELECTED),
            "foreground specimen timer should toggle selected state",
        );
    }

    #[test]
    fn motion_page_expression_swatch_resolves_from_bound_value() {
        let mut ui = build_ui();
        let stage = stage_element(&ui);
        show_demo_page(&mut ui, stage, DemoPage::Motion);
        ui.compute_layout(INITIAL_VIEWPORT_SIZE);

        let swatch = find_by_class(&ui, ui.root(), CLASS_EXPRESSION_SWATCH)
            .expect("motion page should contain the expression color specimen");
        assert_eq!(
            ui.get(swatch)
                .expect("expression swatch should exist")
                .get_local_value(ui.properties().value),
            Some(&0.52),
            "binding should write the local state value into the swatch Value property",
        );
        assert_eq!(
            ui.resolved_value(swatch, ui.properties().surface.background),
            Some(opt_solid(0x18, 0xa0, 0x64, 0xff)),
            "initial bound value should resolve the middle color band",
        );

        ui.set_local(swatch, ui.properties().value, 0.9);

        assert_eq!(
            ui.resolved_value(swatch, ui.properties().surface.background),
            Some(opt_solid(0xf2, 0x6a, 0x8d, 0xff)),
            "changing the dependency property should resolve the high color band",
        );
    }

    #[test]
    fn motion_page_runtime_controls_start_retained_animation_stack() {
        let mut ui = build_ui();
        let stage = stage_element(&ui);
        show_demo_page(&mut ui, stage, DemoPage::Motion);
        ui.compute_layout(INITIAL_VIEWPORT_SIZE);

        assert!(
            find_by_class(&ui, ui.root(), CLASS_RUNTIME_MOTION_SWATCH).is_some(),
            "motion page should contain the retained runtime specimen",
        );
        assert!(
            find_by_class(&ui, ui.root(), CLASS_MANUAL_MOTION_SWATCH).is_some(),
            "motion page should contain the manual timeline specimen",
        );

        let run = find_by_content(&ui, ui.root(), "Run stack")
            .expect("motion page should expose a retained animation start button");
        assert!(
            ui.synthesize_click(run, Point::ZERO).handled,
            "retained animation start button should handle activation",
        );
        ui.flush_bindings();

        assert!(
            ui.has_active_presentation_motion(),
            "retained runtime animation should request presentation motion after activation",
        );
        assert!(
            subtree_contains_text_fragment(&ui, ui.root(), "started: #"),
            "runtime status should report retained animation lifecycle events",
        );
    }

    #[test]
    fn motion_page_runtime_lifecycle_buttons_report_events() {
        let mut ui = build_ui();
        let stage = stage_element(&ui);
        show_demo_page(&mut ui, stage, DemoPage::Motion);
        ui.compute_layout(INITIAL_VIEWPORT_SIZE);

        let run = find_by_content(&ui, ui.root(), "Run stack")
            .expect("motion page should expose a retained animation start button");
        let finish = find_by_content(&ui, ui.root(), "Finish")
            .expect("motion page should expose a retained animation finish button");
        let cancel = find_by_content(&ui, ui.root(), "Cancel")
            .expect("motion page should expose a retained animation cancel button");

        assert!(ui.synthesize_click(run, Point::ZERO).handled);
        assert!(ui.synthesize_click(cancel, Point::ZERO).handled);
        ui.flush_bindings();
        assert!(
            subtree_contains_text_fragment(&ui, ui.root(), "canceled: #"),
            "runtime status should report canceled lifecycle events",
        );

        assert!(ui.synthesize_click(run, Point::ZERO).handled);
        assert!(ui.synthesize_click(finish, Point::ZERO).handled);
        ui.flush_bindings();
        assert!(
            subtree_contains_text_fragment(&ui, ui.root(), "finished: #"),
            "runtime status should report finished lifecycle events",
        );
    }

    #[test]
    fn motion_page_reverse_from_start_moves_the_runtime_specimen() {
        let mut ui = build_ui();
        let stage = stage_element(&ui);
        show_demo_page(&mut ui, stage, DemoPage::Motion);
        ui.compute_layout(INITIAL_VIEWPORT_SIZE);

        let runtime_swatch = find_by_class(&ui, ui.root(), CLASS_RUNTIME_MOTION_SWATCH)
            .expect("motion page should contain the retained runtime specimen");
        let run = find_by_content(&ui, ui.root(), "Run stack")
            .expect("motion page should expose a retained animation start button");
        let reverse = find_by_content(&ui, ui.root(), "Reverse")
            .expect("motion page should expose a reverse button");

        assert!(ui.synthesize_click(run, Point::ZERO).handled);
        let before = runtime_animation_playback(&ui, runtime_swatch);
        assert!(
            before
                .iter()
                .all(|(current_time, playback_rate)| *current_time == Some(0)
                    && *playback_rate > 0.0),
            "runtime animations should start at local zero before immediate reverse: {before:?}",
        );

        assert!(ui.synthesize_click(reverse, Point::ZERO).handled);
        let after = runtime_animation_playback(&ui, runtime_swatch);
        assert!(
            after
                .iter()
                .all(
                    |(current_time, playback_rate)| current_time.is_some_and(|time| time > 0)
                        && *playback_rate < 0.0
                ),
            "reverse from local zero should seek to the endpoint before reversing: {after:?}",
        );
    }

    #[test]
    fn motion_page_runtime_section_is_visible_at_startup() {
        let mut ui = build_ui();
        let stage = stage_element(&ui);
        show_demo_page(&mut ui, stage, DemoPage::Motion);
        ui.compute_layout(INITIAL_VIEWPORT_SIZE);

        assert!(
            overstory::scroll_view_offset_y(&ui, stage).is_some(),
            "demo stage should be a scrollable viewport for pages that outgrow the window",
        );

        let manual_swatch = find_by_class(&ui, ui.root(), CLASS_MANUAL_MOTION_SWATCH)
            .expect("motion page should contain the manual timeline specimen");
        let viewport = ui.effective_bounds(stage);
        let bounds = ui.effective_bounds(manual_swatch);
        assert!(
            bounds.y0 >= viewport.y0 - 0.01 && bounds.y1 <= viewport.y1 + 0.01,
            "runtime manual timeline specimen should be visible at startup: viewport={viewport:?} bounds={bounds:?}",
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

    fn find_by_content(ui: &Ui, element: ElementId, expected: &str) -> Option<ElementId> {
        let current = ui.get(element)?;
        if current
            .get_local_value(ui.properties().content)
            .is_some_and(|content| content == expected)
        {
            return Some(element);
        }
        current
            .children()
            .iter()
            .find_map(|child| find_by_content(ui, *child, expected))
    }

    fn runtime_animation_playback(
        ui: &Ui,
        element: ElementId,
    ) -> Vec<(Option<TimerDuration>, f64)> {
        ui.inspection_snapshot()
            .timeline
            .animations
            .into_iter()
            .filter(|row| {
                row.element == element && row.target == PresentationTarget::visual_translation()
            })
            .map(|row| (row.current_time, row.playback_rate))
            .collect()
    }

    fn subtree_contains_text_fragment(ui: &Ui, element: ElementId, fragment: &str) -> bool {
        ui.get(element).is_some_and(|element_ref| {
            element_ref
                .get_local_value(ui.properties().text)
                .is_some_and(|text| text.contains(fragment))
                || element_ref
                    .children()
                    .iter()
                    .any(|child| subtree_contains_text_fragment(ui, *child, fragment))
        })
    }
}
