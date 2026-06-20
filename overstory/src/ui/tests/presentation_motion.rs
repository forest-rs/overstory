// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn slider_initial_value_positions_thumb() {
    use crate::Slider;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_local(
        root,
        ui.properties().layout,
        Layout::flex().logical_size(300.0, 80.0),
    );

    let slider = ui.append(root, Slider::new().value(0.4));
    let thumb = ui.part(slider, crate::PART_SLIDER_THUMB).unwrap();

    ui.compute_layout(Size::new(300.0, 80.0));

    let slider_bounds = ui.effective_bounds(slider);
    let thumb_bounds = ui.effective_bounds(thumb);
    let thumb_offset = thumb_bounds.x0 - slider_bounds.x0;
    assert!(
        thumb_offset > 70.0,
        "initial value should move thumb along the track; got x offset {thumb_offset}"
    );
}

#[test]
fn slider_snap_value_change_updates_layer_without_waiting_for_second_layout() {
    use crate::{SLOT_SLIDER_THUMB, Slider, set_slider_value};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(300.0, 80.0));
    let slider = ui.append(root, Slider::new().value(0.0));
    ui.compute_layout(Size::new(300.0, 80.0));
    let thumb = ui.slot(slider, SLOT_SLIDER_THUMB).unwrap();

    set_slider_value(&mut ui, slider, 1.0);
    ui.compute_layout(Size::new(300.0, 80.0));

    assert_close(layer_x(&ui, thumb), ui.effective_bounds(thumb).x0);
    assert_close(layer_y(&ui, thumb), ui.effective_bounds(thumb).y0);
}

#[test]
fn slider_value_binding_updates_thumb_layout() {
    use crate::{BindingSpecExt, SLOT_SLIDER_THUMB, Slider};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(520.0, 90.0));
    let value = ui.local_state(0.25_f64);
    let value_property = ui.properties().value;
    let first = ui.append(
        root,
        Slider::new().bind_property(value_property, value.binding()),
    );
    let second = ui.append(
        root,
        Slider::new().bind_property(value_property, value.binding()),
    );

    ui.compute_layout(Size::new(520.0, 90.0));
    let first_thumb = ui.slot(first, SLOT_SLIDER_THUMB).unwrap();
    let second_thumb = ui.slot(second, SLOT_SLIDER_THUMB).unwrap();
    let first_start = ui.effective_bounds(first_thumb).x0 - ui.effective_bounds(first).x0;
    let second_start = ui.effective_bounds(second_thumb).x0 - ui.effective_bounds(second).x0;
    assert_close(first_start, second_start);

    assert!(value.set(&mut ui, 0.75));
    ui.compute_layout(Size::new(520.0, 90.0));
    let first_after = ui.effective_bounds(first_thumb).x0 - ui.effective_bounds(first).x0;
    let second_after = ui.effective_bounds(second_thumb).x0 - ui.effective_bounds(second).x0;
    assert!(
        first_after > first_start + 50.0,
        "binding update should move the first slider thumb: {first_start} -> {first_after}",
    );
    assert_close(first_after, second_after);
}

#[test]
fn default_templates_label_named_parts() {
    use crate::{
        Button, PART_BUTTON_LABEL, PART_SLIDER_THUMB, PART_SLIDER_TRACK, PART_TOGGLE_THUMB,
        SLOT_BUTTON_CONTENT, SLOT_SLIDER_THUMB, SLOT_SLIDER_TRACK, SLOT_TOGGLE_THUMB, Slider,
        ToggleSwitch,
    };

    let mut ui = Ui::new();
    let root = ui.root();

    let button = ui.append(root, Button::new("Run"));
    let button_label = ui.part(button, PART_BUTTON_LABEL).unwrap();
    assert_eq!(
        ui.get(button_label).unwrap().part_tag(),
        Some(PART_BUTTON_LABEL)
    );
    assert_eq!(
        ui.template_instance(button)
            .unwrap()
            .part(PART_BUTTON_LABEL),
        Some(button_label)
    );
    assert_eq!(ui.slot(button, SLOT_BUTTON_CONTENT), Some(button_label));

    let slider = ui.append(root, Slider::new());
    let slider_track = ui.part(slider, PART_SLIDER_TRACK).unwrap();
    let slider_thumb = ui.part(slider, PART_SLIDER_THUMB).unwrap();
    assert_eq!(ui.slot(slider, SLOT_SLIDER_TRACK), Some(slider_track));
    assert_eq!(ui.slot(slider, SLOT_SLIDER_THUMB), Some(slider_thumb));
    assert_eq!(
        ui.get(slider_track).unwrap().part_tag(),
        Some(PART_SLIDER_TRACK)
    );
    assert_eq!(
        ui.get(slider_thumb).unwrap().part_tag(),
        Some(PART_SLIDER_THUMB)
    );

    let toggle = ui.append(root, ToggleSwitch::new());
    let toggle_thumb = ui.part(toggle, PART_TOGGLE_THUMB).unwrap();
    assert_eq!(ui.slot(toggle, SLOT_TOGGLE_THUMB), Some(toggle_thumb));
    assert_eq!(
        ui.get(toggle_thumb).unwrap().part_tag(),
        Some(PART_TOGGLE_THUMB)
    );
}

#[test]
fn toggle_thumb_transition_samples_composition_offset() {
    use crate::SLOT_TOGGLE_THUMB;

    let (mut ui, toggle, thumb) = animated_toggle_fixture();
    let start_x = layer_x(&ui, thumb);
    let start_y = layer_y(&ui, thumb);

    ui.set_now(0);
    ui.synthesize_click(toggle, ui.effective_bounds(toggle).center());
    ui.compute_layout(Size::new(200.0, 80.0));

    let target_x = ui.effective_bounds(thumb).x0;
    assert!(
        target_x > start_x,
        "checked layout target should move thumb to the right"
    );
    assert_close(layer_x(&ui, thumb), start_x);
    assert_close(layer_y(&ui, thumb), start_y);

    ui.set_now(millis(60));
    ui.flush_composition();
    assert_close(layer_x(&ui, thumb), thumb_timing(start_x, target_x, 0.5));
    assert_close(layer_y(&ui, thumb), start_y);

    ui.set_now(millis(120));
    ui.flush_composition();
    assert_close(layer_x(&ui, thumb), target_x);
    assert_close(layer_y(&ui, thumb), start_y);

    assert_eq!(ui.slot(toggle, SLOT_TOGGLE_THUMB), Some(thumb));
}

#[test]
fn toggle_thumb_transition_retargets_from_sampled_offset() {
    let (mut ui, toggle, thumb) = animated_toggle_fixture();
    let start_x = layer_x(&ui, thumb);

    ui.set_now(0);
    ui.synthesize_click(toggle, ui.effective_bounds(toggle).center());
    ui.compute_layout(Size::new(200.0, 80.0));
    let checked_target = ui.effective_bounds(thumb).x0;

    ui.set_now(millis(60));
    ui.compute_layout(Size::new(200.0, 80.0));
    let midpoint = layer_x(&ui, thumb);
    assert_close(midpoint, thumb_timing(start_x, checked_target, 0.5));

    ui.synthesize_click(toggle, ui.effective_bounds(toggle).center());
    ui.compute_layout(Size::new(200.0, 80.0));
    assert_close(layer_x(&ui, thumb), midpoint);

    ui.set_now(millis(120));
    ui.compute_layout(Size::new(200.0, 80.0));
    assert_close(layer_x(&ui, thumb), thumb_timing(midpoint, start_x, 0.5));

    ui.set_now(millis(180));
    ui.compute_layout(Size::new(200.0, 80.0));
    assert_close(layer_x(&ui, thumb), start_x);
}

#[test]
fn button_hover_transitions_background_color() {
    use crate::{Button, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(240.0, 80.0));
    let button = ui.append(root, Button::new("Hover"));
    ui.compute_layout(Size::new(240.0, 80.0));

    let idle = Color::from_rgba8(0x3a, 0x69, 0xd6, 0xff);
    let hover = Color::from_rgba8(0x4d, 0x81, 0xf2, 0xff);
    ui.set_now(0);
    let _ = ui.dispatch_pointer_event(&pointer_move(
        ui.effective_bounds(button).center(),
        0,
        false,
    ));
    ui.compute_layout(Size::new(240.0, 80.0));

    let mut scene = Scene::new();
    ui.paint(&mut scene);
    assert_eq!(element_background_color(&ui, button), Some(idle));

    ui.set_now(millis(45));
    ui.compute_layout(Size::new(240.0, 80.0));
    ui.paint(&mut scene);
    assert_eq!(
        element_background_color(&ui, button),
        Some(interpolate_srgb_color(idle, hover, 0.5))
    );

    ui.set_now(millis(90));
    ui.compute_layout(Size::new(240.0, 80.0));
    ui.paint(&mut scene);
    assert_eq!(element_background_color(&ui, button), Some(hover));
}

#[test]
fn styled_effect_rule_triggers_visual_translation_slot() {
    const EFFECTFUL: ClassId = ClassId(901);
    const TRIGGER: EffectTriggerId = EffectTriggerId::new(901);

    let mut ui = Ui::new();
    let root = ui.root();
    let effects = ui.properties().effects;
    ui.set_cascade(
        StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC).with_class(EFFECTFUL))
                    .build(),
                StyleBuilder::new()
                    .set(
                        effects,
                        EffectRules::single(EffectRule::new(
                            TRIGGER,
                            PresentationTarget::visual_translation(),
                            EffectClip::from_keyframes([
                                EffectKeyframe::new(0, PresentationValue::Offset2d(Vec2::ZERO)),
                                EffectKeyframe::new(
                                    millis(50),
                                    PresentationValue::Offset2d(Vec2::new(10.0, 0.0)),
                                ),
                                EffectKeyframe::new(
                                    millis(100),
                                    PresentationValue::Offset2d(Vec2::ZERO),
                                ),
                            ]),
                        )),
                    )
                    .build(),
            )
            .build(),
    );
    let panel = ui.append(root, crate::Panel::new().class(EFFECTFUL));

    ui.set_now(0);
    assert!(ui.trigger_effect(panel, TRIGGER));
    assert!(ui.invalidation().is_invalidated(panel, COMPOSE));

    ui.set_now(millis(50));
    ui.flush_composition();
    let translation = ui.presented_value(
        panel,
        PresentationTarget::slot(PresentationSlot::VisualTranslation),
        Vec2::ZERO,
    );
    assert_close(translation.x, 10.0);

    ui.set_now(millis(100));
    ui.flush_composition();
    let translation = ui.presented_value(
        panel,
        PresentationTarget::slot(PresentationSlot::VisualTranslation),
        Vec2::ZERO,
    );
    assert_close(translation.x, 0.0);
}

#[test]
fn styled_effect_rule_targets_property_animation_layer() {
    const EFFECTFUL: ClassId = ClassId(902);
    const TRIGGER: EffectTriggerId = EffectTriggerId::new(902);

    let mut ui = Ui::new();
    let root = ui.root();
    let opacity = ui.properties().opacity;
    let effects = ui.properties().effects;
    ui.set_cascade(
        StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC).with_class(EFFECTFUL))
                    .build(),
                StyleBuilder::new()
                    .set(
                        effects,
                        EffectRules::single(EffectRule::new(
                            TRIGGER,
                            PresentationTarget::property(opacity.id()),
                            EffectClip::from_keyframes([
                                EffectKeyframe::new(0, PresentationValue::Scalar32(1.0)),
                                EffectKeyframe::new(millis(50), PresentationValue::Scalar32(0.25)),
                                EffectKeyframe::new(millis(100), PresentationValue::Scalar32(1.0)),
                            ]),
                        )),
                    )
                    .build(),
            )
            .build(),
    );
    let panel = ui.append(root, crate::Panel::new().class(EFFECTFUL));

    ui.set_now(0);
    assert!(ui.trigger_effect(panel, TRIGGER));
    assert!(ui.invalidation().is_invalidated(panel, COMPOSE));

    ui.set_now(millis(50));
    ui.flush_composition();
    let element = ui.get(panel).expect("panel should exist");
    assert_close(f64::from(ui.resolve_value_for(element, opacity)), 0.25);

    ui.set_now(millis(100));
    ui.flush_composition();
    let element = ui.get(panel).expect("panel should exist");
    assert_close(f64::from(ui.resolve_value_for(element, opacity)), 1.0);
}

#[test]
fn styled_transition_rule_controls_button_hover_duration() {
    use crate::{Button, TYPE_BUTTON};

    const TEST_HOVER_DURATION: ResourceKey = ResourceKey::new(900);

    let mut ui = Ui::new();
    ui.set_theme(
        ThemeBuilder::new()
            .set(TEST_HOVER_DURATION, millis(180))
            .build(),
    );
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(240.0, 80.0));

    let idle = Color::from_rgba8(0x10, 0x20, 0x30, 0xff);
    let hover = Color::from_rgba8(0x50, 0x60, 0x70, 0xff);
    let background = ui.properties().surface.background;
    let transitions = ui.properties().transitions;
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            crate::selectors::type_tag(TYPE_BUTTON),
            StyleBuilder::new()
                .set(background, Some(Brush::Solid(idle)))
                .set(
                    transitions,
                    TransitionRules::single(TransitionRule::interaction(
                        PresentationTarget::property(background.id()),
                        TransitionSpec::linear(millis(90))
                            .with_duration_resource(TEST_HOVER_DURATION)
                            .with_color_interpolation(ColorInterpolation::SRGB),
                    )),
                )
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            crate::selectors::state(TYPE_BUTTON, PSEUDO_HOVER),
            StyleBuilder::new()
                .set(background, Some(Brush::Solid(hover)))
                .build(),
        )
        .build();
    ui.set_cascade(cascade);

    let button = ui.append(root, Button::new("Hover"));
    ui.set_layout(button, Layout::flex().logical_size(160.0, 48.0));
    ui.compute_layout(Size::new(240.0, 80.0));

    ui.set_now(0);
    let _ = ui.dispatch_pointer_event(&pointer_move(
        ui.effective_bounds(button).center(),
        0,
        false,
    ));
    ui.compute_layout(Size::new(240.0, 80.0));

    ui.set_now(millis(90));
    ui.compute_layout(Size::new(240.0, 80.0));
    let mut scene = Scene::new();
    ui.paint(&mut scene);
    assert_eq!(
        element_background_color(&ui, button),
        Some(interpolate_srgb_color(idle, hover, 0.5))
    );
}

#[test]
fn transition_rules_drive_registered_foreground_property() {
    const MOTION_TEXT: ClassId = ClassId(91);

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(240.0, 80.0));

    let idle = Color::from_rgba8(0xb0, 0xb8, 0xc8, 0xff);
    let selected = Color::from_rgba8(0x56, 0xd3, 0xff, 0xff);
    let foreground = ui.properties().foreground;
    let transitions = ui.properties().transitions;
    let selector =
        SelectorBuilder::new(SelectorStep::type_tag(crate::TYPE_TEXT).with_class(MOTION_TEXT))
            .build();
    let selected_selector = SelectorBuilder::new(
        SelectorStep::type_tag(crate::TYPE_TEXT)
            .with_class(MOTION_TEXT)
            .with_pseudo(PSEUDO_SELECTED),
    )
    .build();
    ui.set_cascade(
        StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                selector,
                StyleBuilder::new()
                    .set(foreground, Brush::Solid(idle))
                    .set(
                        transitions,
                        TransitionRules::single(TransitionRule::interaction(
                            PresentationTarget::property(foreground.id()),
                            TransitionSpec::linear(millis(100))
                                .with_color_interpolation(ColorInterpolation::SRGB),
                        )),
                    )
                    .build(),
            )
            .push_rule(
                StyleOrigin::Base,
                selected_selector,
                StyleBuilder::new()
                    .set(foreground, Brush::Solid(selected))
                    .build(),
            )
            .build(),
    );

    let text = ui.append(
        root,
        crate::Text::new("Foreground").class(MOTION_TEXT).layout(
            Layout::flex()
                .logical_size(180.0, 32.0)
                .alignment(crate::LayoutAlignment::new().align_items(crate::LayoutAlign::Center)),
        ),
    );
    ui.compute_layout(Size::new(240.0, 80.0));
    assert_eq!(element_foreground_color(&ui, text), Some(idle));

    ui.set_now(0);
    ui.add_pseudo(text, PSEUDO_SELECTED);
    ui.compute_layout(Size::new(240.0, 80.0));
    assert_eq!(element_foreground_color(&ui, text), Some(idle));

    ui.set_now(millis(50));
    ui.compute_layout(Size::new(240.0, 80.0));
    assert_eq!(
        element_foreground_color(&ui, text),
        Some(interpolate_srgb_color(idle, selected, 0.5))
    );

    ui.set_now(millis(100));
    ui.compute_layout(Size::new(240.0, 80.0));
    assert_eq!(element_foreground_color(&ui, text), Some(selected));
}

#[test]
fn interaction_transitions_track_descendant_effective_value_changes() {
    const MOTION_ROW: ClassId = ClassId(92);
    const MOTION_LABEL: ClassId = ClassId(93);

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(260.0, 80.0));

    let idle = Color::from_rgba8(0xd0, 0xd4, 0xdf, 0xff);
    let selected = Color::from_rgba8(0x60, 0x7f, 0xff, 0xff);
    let foreground = ui.properties().foreground;
    let transitions = ui.properties().transitions;
    let label_selector =
        SelectorBuilder::new(SelectorStep::type_tag(crate::TYPE_TEXT).with_class(MOTION_LABEL))
            .build();
    let selected_label_selector = understory_style::Selector::from([
        SelectorStep::type_tag(TYPE_GENERIC)
            .with_class(MOTION_ROW)
            .with_pseudo(PSEUDO_SELECTED),
        SelectorStep::type_tag(crate::TYPE_TEXT).with_class(MOTION_LABEL),
    ]);
    ui.set_cascade(
        StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                label_selector,
                StyleBuilder::new()
                    .set(foreground, Brush::Solid(idle))
                    .set(
                        transitions,
                        TransitionRules::single(TransitionRule::interaction(
                            PresentationTarget::property(foreground.id()),
                            TransitionSpec::linear(millis(100))
                                .with_color_interpolation(ColorInterpolation::SRGB),
                        )),
                    )
                    .build(),
            )
            .push_rule(
                StyleOrigin::Base,
                selected_label_selector,
                StyleBuilder::new()
                    .set(foreground, Brush::Solid(selected))
                    .build(),
            )
            .build(),
    );

    let row = ui.append(
        root,
        crate::Panel::new()
            .class(MOTION_ROW)
            .layout(Layout::flex().logical_size(220.0, 40.0)),
    );
    let label = ui.append(row, crate::Text::new("Descendant").class(MOTION_LABEL));
    ui.compute_layout(Size::new(260.0, 80.0));
    assert_eq!(element_foreground_color(&ui, label), Some(idle));

    ui.set_now(0);
    ui.add_pseudo(row, PSEUDO_SELECTED);
    ui.compute_layout(Size::new(260.0, 80.0));
    assert_eq!(element_foreground_color(&ui, label), Some(idle));

    ui.set_now(millis(50));
    ui.compute_layout(Size::new(260.0, 80.0));
    assert_eq!(
        element_foreground_color(&ui, label),
        Some(interpolate_srgb_color(idle, selected, 0.5))
    );

    ui.set_now(millis(100));
    ui.compute_layout(Size::new(260.0, 80.0));
    assert_eq!(element_foreground_color(&ui, label), Some(selected));
}

#[test]
fn inherited_timeline_property_samples_invalidate_descendants() {
    const MOTION_ROW: ClassId = ClassId(94);

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(260.0, 80.0));

    let idle = Color::from_rgba8(0xd0, 0xd4, 0xdf, 0xff);
    let selected = Color::from_rgba8(0x60, 0x7f, 0xff, 0xff);
    let foreground = ui.properties().foreground;
    let transitions = ui.properties().transitions;
    let row_selector =
        SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC).with_class(MOTION_ROW)).build();
    let selected_row_selector = SelectorBuilder::new(
        SelectorStep::type_tag(TYPE_GENERIC)
            .with_class(MOTION_ROW)
            .with_pseudo(PSEUDO_SELECTED),
    )
    .build();
    ui.set_cascade(
        StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                row_selector,
                StyleBuilder::new()
                    .set(foreground, Brush::Solid(idle))
                    .set(
                        transitions,
                        TransitionRules::single(TransitionRule::interaction(
                            PresentationTarget::property(foreground.id()),
                            TransitionSpec::linear(millis(100))
                                .with_color_interpolation(ColorInterpolation::SRGB),
                        )),
                    )
                    .build(),
            )
            .push_rule(
                StyleOrigin::Base,
                selected_row_selector,
                StyleBuilder::new()
                    .set(foreground, Brush::Solid(selected))
                    .build(),
            )
            .build(),
    );

    let row = ui.append(
        root,
        crate::Panel::new()
            .class(MOTION_ROW)
            .layout(Layout::flex().logical_size(220.0, 40.0)),
    );
    let label = ui.append(row, crate::Text::new("Inherited"));
    ui.compute_layout(Size::new(260.0, 80.0));

    ui.set_now(0);
    ui.add_pseudo(row, PSEUDO_SELECTED);
    ui.compute_layout(Size::new(260.0, 80.0));
    ui.invalidation.clear(PAINT);

    ui.set_now(millis(50));

    assert_eq!(
        element_foreground_color(&ui, label),
        Some(interpolate_srgb_color(idle, selected, 0.5))
    );
    assert!(ui.invalidation().is_invalidated(row, PAINT));
    assert!(ui.invalidation().is_invalidated(label, PAINT));
}

#[test]
fn local_background_change_after_transition_paints_new_target() {
    use crate::{Button, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(240.0, 80.0));
    let button = ui.append(root, Button::new("Hover"));
    ui.compute_layout(Size::new(240.0, 80.0));

    ui.set_now(0);
    let _ = ui.dispatch_pointer_event(&pointer_move(
        ui.effective_bounds(button).center(),
        0,
        false,
    ));
    ui.set_now(millis(90));
    let mut scene = Scene::new();
    ui.paint(&mut scene);

    let red = Color::from_rgba8(0xff, 0x00, 0x00, 0xff);
    ui.set_local(
        button,
        ui.properties().surface.background,
        Some(Brush::Solid(red)),
    );
    ui.paint(&mut scene);

    assert_eq!(element_background_color(&ui, button), Some(red));
}

#[test]
fn button_press_transitions_layer_opacity() {
    use crate::{Button, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(240.0, 80.0));
    let button = ui.append(root, Button::new("Press"));
    ui.compute_layout(Size::new(240.0, 80.0));

    let center = ui.effective_bounds(button).center();
    let _ = ui.dispatch_pointer_event(&pointer_down(center, 0));
    ui.compute_layout(Size::new(240.0, 80.0));
    assert_close(layer_opacity(&ui, button), 1.0);

    ui.set_now(millis(35));
    ui.compute_layout(Size::new(240.0, 80.0));
    assert_close(layer_opacity(&ui, button), 0.96);

    ui.set_now(millis(70));
    ui.compute_layout(Size::new(240.0, 80.0));
    assert_close(layer_opacity(&ui, button), 0.92);
}

#[test]
fn text_input_hover_transitions_border_brush_color() {
    use crate::{TextInput, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(260.0, 80.0));
    let input = ui.append(root, TextInput::new().text("Hover"));
    ui.compute_layout(Size::new(260.0, 80.0));

    let idle = Color::from_rgba8(0x62, 0x62, 0x70, 0xff);
    let hover = Color::from_rgba8(0x86, 0x91, 0xaa, 0xff);
    assert_eq!(element_border_color(&ui, input), Some(idle));

    ui.set_now(0);
    let _ = ui.dispatch_pointer_event(&pointer_move(ui.effective_bounds(input).center(), 0, false));
    ui.compute_layout(Size::new(260.0, 80.0));
    assert_eq!(element_border_color(&ui, input), Some(idle));

    ui.set_now(millis(45));
    ui.compute_layout(Size::new(260.0, 80.0));
    assert_eq!(
        element_border_color(&ui, input),
        Some(interpolate_srgb_color(idle, hover, 0.5))
    );

    ui.set_now(millis(90));
    ui.compute_layout(Size::new(260.0, 80.0));
    assert_eq!(element_border_color(&ui, input), Some(hover));
}

#[test]
fn text_input_focus_visible_transitions_border_width() {
    use crate::{TextInput, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(260.0, 80.0));
    let input = ui.append(root, TextInput::new().text("Focus"));
    ui.compute_layout(Size::new(260.0, 80.0));

    assert_close(element_border_width(&ui, input), 1.0);

    ui.set_now(0);
    assert!(ui.set_focus(Some(input)));
    ui.compute_layout(Size::new(260.0, 80.0));
    assert_close(element_border_width(&ui, input), 1.0);

    ui.set_now(millis(35));
    ui.compute_layout(Size::new(260.0, 80.0));
    assert_close(element_border_width(&ui, input), 1.5);

    ui.set_now(millis(70));
    ui.compute_layout(Size::new(260.0, 80.0));
    assert_close(element_border_width(&ui, input), 2.0);
}

#[test]
fn slider_keyboard_transitions_thumb_offset_but_drag_snaps() {
    use crate::{SLOT_SLIDER_THUMB, Slider, theme};
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(300.0, 80.0));
    let slider = ui.append(root, Slider::new().value(0.0).small_change(0.5));
    assert!(ui.set_focus(Some(slider)));
    ui.compute_layout(Size::new(300.0, 80.0));
    let thumb = ui.slot(slider, SLOT_SLIDER_THUMB).unwrap();
    let start_x = layer_x(&ui, thumb);
    let start_y = layer_y(&ui, thumb);

    ui.set_now(0);
    let right = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
    assert!(ui.dispatch_key(&right).handled);
    ui.compute_layout(Size::new(300.0, 80.0));
    let target_x = ui.effective_bounds(thumb).x0;
    assert!(target_x > start_x);
    assert_close(layer_x(&ui, thumb), start_x);
    assert_close(layer_y(&ui, thumb), start_y);

    ui.set_now(millis(45));
    ui.compute_layout(Size::new(300.0, 80.0));
    assert_close(layer_x(&ui, thumb), thumb_timing(start_x, target_x, 0.5));
    assert_close(layer_y(&ui, thumb), start_y);

    let drag_target = Point::new(target_x + 30.0, ui.effective_bounds(thumb).center().y);
    let _ = ui.dispatch_pointer_event(&pointer_down(ui.effective_bounds(thumb).center(), 100));
    let _ = ui.dispatch_pointer_event(&pointer_move(drag_target, 101, true));
    ui.compute_layout(Size::new(300.0, 80.0));
    assert_close(layer_x(&ui, thumb), ui.effective_bounds(thumb).x0);
    assert_close(layer_y(&ui, thumb), start_y);
}

#[test]
fn slider_thumb_snaps_without_styled_transition_rule() {
    use crate::controls::slider::{PART_SLIDER_THUMB, TYPE_SLIDER};
    use crate::{SLOT_SLIDER_THUMB, Slider};
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(300.0, 80.0));
    let style = StyleBuilder::new()
        .set(ui.properties().transitions, TransitionRules::new())
        .build();
    ui.set_cascade(
        StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Sheet,
                crate::selectors::part(crate::selectors::template_part(
                    TYPE_SLIDER,
                    PART_SLIDER_THUMB,
                )),
                style,
            )
            .build(),
    );
    let slider = ui.append(root, Slider::new().value(0.0).small_change(0.5));
    assert!(ui.set_focus(Some(slider)));
    ui.compute_layout(Size::new(300.0, 80.0));
    let thumb = ui.slot(slider, SLOT_SLIDER_THUMB).unwrap();
    let start_y = layer_y(&ui, thumb);

    ui.set_now(0);
    let right = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
    assert!(ui.dispatch_key(&right).handled);
    ui.compute_layout(Size::new(300.0, 80.0));

    assert_close(layer_x(&ui, thumb), ui.effective_bounds(thumb).x0);
    assert_close(layer_y(&ui, thumb), start_y);
}

fn animated_toggle_fixture() -> (Ui, ElementId, ElementId) {
    use crate::{SLOT_TOGGLE_THUMB, ToggleSwitch, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(200.0, 80.0));
    let toggle = ui.append(root, ToggleSwitch::new());
    ui.compute_layout(Size::new(200.0, 80.0));
    let thumb = ui
        .slot(toggle, SLOT_TOGGLE_THUMB)
        .expect("toggle template should create a thumb slot");
    (ui, toggle, thumb)
}

fn layer_opacity(ui: &Ui, id: ElementId) -> f64 {
    let layer = ui
        .composition
        .layer_for(id)
        .expect("element should have a composition layer");
    f64::from(ui.composition_layers().local_opacity(layer))
}

fn element_background_color(ui: &Ui, id: ElementId) -> Option<Color> {
    let node = ui.box_for(id)?;
    let surface = ui.presentation.presentation(node)?.surface()?;
    let brush = &surface.backgrounds.first()?.brush;
    match brush {
        Brush::Solid(color) => Some(*color),
        _ => None,
    }
}

fn element_border_color(ui: &Ui, id: ElementId) -> Option<Color> {
    let element = ui.get(id)?;
    match ui.resolve_value_for(element, ui.properties().surface.border_brushes.top) {
        Some(Brush::Solid(color)) => Some(color),
        _ => None,
    }
}

fn element_foreground_color(ui: &Ui, id: ElementId) -> Option<Color> {
    let element = ui.get(id)?;
    match ui.resolve_value_for(element, ui.properties().foreground) {
        Brush::Solid(color) => Some(color),
        _ => None,
    }
}

fn element_border_width(ui: &Ui, id: ElementId) -> f64 {
    let element = ui.get(id).expect("element should exist");
    ui.resolve_value_for(element, ui.properties().surface.border_widths.top)
}

fn thumb_timing(from: f64, to: f64, t: f64) -> f64 {
    let timing = TimingFunction::cubic_bezier(0.215, 0.61, 0.355, 1.0);
    from.interpolate(to, timing.sample(t))
}

fn interpolate_srgb_color(from: Color, to: Color, t: f32) -> Color {
    DynamicColor::from_alpha_color(from)
        .interpolate(
            DynamicColor::from_alpha_color(to),
            ColorSpaceTag::Srgb,
            HueDirection::Shorter,
        )
        .eval(t)
        .to_alpha_color::<Srgb>()
}
