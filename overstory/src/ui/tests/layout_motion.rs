// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn owner_state_layout_change_transitions_descendant_layer_offset() {
    const MOTION_ROW: ClassId = ClassId(94);
    const MOTION_CHILD: ClassId = ClassId(95);

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(220.0, 80.0));

    let row_selector =
        SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC).with_class(MOTION_ROW)).build();
    let selected_row_selector = SelectorBuilder::new(
        SelectorStep::type_tag(TYPE_GENERIC)
            .with_class(MOTION_ROW)
            .with_pseudo(PSEUDO_SELECTED),
    )
    .build();
    let child_selector =
        SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC).with_class(MOTION_CHILD)).build();
    ui.set_cascade(
        StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                row_selector,
                StyleBuilder::new()
                    .set(
                        ui.properties().layout_alignment,
                        crate::LayoutAlignment::new()
                            .align_items(crate::LayoutAlign::Center)
                            .justify_content(crate::LayoutJustify::Start),
                    )
                    .build(),
            )
            .push_rule(
                StyleOrigin::Base,
                selected_row_selector,
                StyleBuilder::new()
                    .set(
                        ui.properties().layout_alignment,
                        crate::LayoutAlignment::new()
                            .align_items(crate::LayoutAlign::Center)
                            .justify_content(crate::LayoutJustify::End),
                    )
                    .build(),
            )
            .push_rule(
                StyleOrigin::Base,
                child_selector,
                StyleBuilder::new()
                    .set(
                        ui.properties().transitions,
                        TransitionRules::single(TransitionRule::interaction(
                            PresentationTarget::layer_offset(),
                            TransitionSpec::linear(millis(100)),
                        )),
                    )
                    .build(),
            )
            .build(),
    );

    let row = ui.append(
        root,
        crate::Panel::new()
            .class(MOTION_ROW)
            .layout(Layout::row(0.0).logical_size(140.0, 40.0)),
    );
    let child = ui.append(
        row,
        crate::Panel::new()
            .class(MOTION_CHILD)
            .layout(Layout::new().logical_size(20.0, 20.0)),
    );
    ui.compute_layout(Size::new(220.0, 80.0));
    let start_x = layer_x(&ui, child);
    let start_y = layer_y(&ui, child);

    ui.set_now(0);
    ui.add_pseudo(row, PSEUDO_SELECTED);
    ui.compute_layout(Size::new(220.0, 80.0));
    let target_x = ui.effective_bounds(child).x0;
    assert!(
        target_x > start_x + 60.0,
        "selected row alignment should move the child right: {start_x} -> {target_x}",
    );
    assert_close(layer_x(&ui, child), start_x);
    assert_close(layer_y(&ui, child), start_y);

    ui.set_now(millis(50));
    ui.compute_layout(Size::new(220.0, 80.0));
    assert_close(layer_x(&ui, child), start_x + ((target_x - start_x) * 0.5));
    assert_close(layer_y(&ui, child), start_y);

    ui.set_now(millis(100));
    ui.compute_layout(Size::new(220.0, 80.0));
    assert_close(layer_x(&ui, child), target_x);
    assert_close(layer_y(&ui, child), start_y);
}

#[test]
fn paint_only_interaction_does_not_arm_layer_offset_transition() {
    const MOTION_SURFACE: ClassId = ClassId(96);

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(160.0, 80.0));

    let surface_selector =
        SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC).with_class(MOTION_SURFACE))
            .build();
    let selected_surface_selector = SelectorBuilder::new(
        SelectorStep::type_tag(TYPE_GENERIC)
            .with_class(MOTION_SURFACE)
            .with_pseudo(PSEUDO_SELECTED),
    )
    .build();
    ui.set_cascade(
        StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                surface_selector,
                StyleBuilder::new()
                    .set(
                        ui.properties().transitions,
                        TransitionRules::single(TransitionRule::interaction(
                            PresentationTarget::layer_offset(),
                            TransitionSpec::linear(millis(100)),
                        )),
                    )
                    .build(),
            )
            .push_rule(
                StyleOrigin::Base,
                selected_surface_selector,
                StyleBuilder::new()
                    .set(
                        ui.properties().surface.background,
                        Some(Brush::Solid(Color::from_rgba8(0x40, 0x80, 0xff, 0xff))),
                    )
                    .build(),
            )
            .build(),
    );

    let surface = ui.append(
        root,
        crate::Panel::new()
            .class(MOTION_SURFACE)
            .layout(Layout::new().logical_size(40.0, 40.0)),
    );
    ui.compute_layout(Size::new(160.0, 80.0));

    ui.add_pseudo(surface, PSEUDO_SELECTED);

    assert_eq!(ui.inspection_snapshot().timeline.pending_transitions, 0);
}

#[test]
fn generic_layer_offset_transition_tracks_layout_target_changes() {
    use crate::Panel;

    const MOVING: ClassId = ClassId(90);

    let mut ui = Ui::new();
    let transitions = ui.properties().transitions;
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC).with_class(MOVING)).build(),
            StyleBuilder::new()
                .set(
                    transitions,
                    TransitionRules::single(TransitionRule::interaction(
                        PresentationTarget::layer_offset(),
                        TransitionSpec::linear(millis(100)),
                    )),
                )
                .build(),
        )
        .build();
    ui.set_cascade(cascade);

    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(200.0, 80.0));
    let stage = ui.append(
        root,
        Panel::new().layout(Layout::flex().relative().logical_size(180.0, 60.0)),
    );
    let mover = ui.append(
        stage,
        Panel::new().class(MOVING).layout(
            Layout::new()
                .absolute_at(10.0, 20.0)
                .logical_size(24.0, 24.0),
        ),
    );

    ui.compute_layout(Size::new(200.0, 80.0));
    let start_x = layer_x(&ui, mover);
    assert_close(start_x, 10.0);

    ui.set_now(0);
    ui.set_layout(
        mover,
        Layout::new()
            .absolute_at(130.0, 20.0)
            .logical_size(24.0, 24.0),
    );
    ui.compute_layout(Size::new(200.0, 80.0));
    assert_close(ui.effective_bounds(mover).x0, 130.0);
    assert_close(layer_x(&ui, mover), 10.0);

    ui.set_now(millis(50));
    ui.compute_layout(Size::new(200.0, 80.0));
    assert_close(layer_x(&ui, mover), 70.0);

    ui.set_now(millis(100));
    ui.compute_layout(Size::new(200.0, 80.0));
    assert_close(layer_x(&ui, mover), 130.0);
}

#[test]
fn snap_layout_placement_cancels_active_layer_offset_transition() {
    use crate::Panel;

    const MOVING: ClassId = ClassId(91);

    let mut ui = Ui::new();
    let transitions = ui.properties().transitions;
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC).with_class(MOVING)).build(),
            StyleBuilder::new()
                .set(
                    transitions,
                    TransitionRules::single(TransitionRule::interaction(
                        PresentationTarget::layer_offset(),
                        TransitionSpec::linear(millis(100)),
                    )),
                )
                .build(),
        )
        .build();
    ui.set_cascade(cascade);

    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(200.0, 80.0));
    let stage = ui.append(
        root,
        Panel::new().layout(Layout::flex().relative().logical_size(180.0, 60.0)),
    );
    let mover = ui.append(
        stage,
        Panel::new().class(MOVING).layout(
            Layout::new()
                .absolute_at(10.0, 20.0)
                .logical_size(24.0, 24.0),
        ),
    );

    ui.compute_layout(Size::new(200.0, 80.0));
    ui.set_now(0);
    ui.set_layout_placement(mover, LayoutPlacement::absolute_at(130.0, 20.0));
    ui.compute_layout(Size::new(200.0, 80.0));
    assert_close(layer_x(&ui, mover), 10.0);

    ui.set_now(millis(50));
    ui.compute_layout(Size::new(200.0, 80.0));
    assert_close(layer_x(&ui, mover), 70.0);

    ui.set_layout_placement_without_motion(mover, LayoutPlacement::absolute_at(130.0, 20.0));
    ui.compute_layout(Size::new(200.0, 80.0));
    assert_close(layer_x(&ui, mover), 130.0);
}
