// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn motion_scope_transitions_an_ordinary_property_write() {
    const DURATION: ResourceKey = ResourceKey::new(1_201);

    let mut ui = Ui::new();
    ui.set_theme(ThemeBuilder::new().set(DURATION, millis(100)).build());
    let root = ui.root();
    let panel = ui.append(
        root,
        crate::Panel::new().layout(Layout::flex().logical_size(120.0, 40.0)),
    );
    let opacity = ui.properties().opacity;
    ui.compute_layout(Size::new(160.0, 80.0));

    ui.set_now(0);
    ui.with_motion(MotionSpec::from_duration_resource(DURATION), |ui| {
        ui.set_local(panel, opacity, 0.25_f32);
    });

    assert_eq!(
        ui.get(panel)
            .and_then(|element| element.get_local_value(opacity)),
        Some(&0.25),
    );
    assert_eq!(ui.presented_layer_opacity(panel), 1.0);
    assert!(ui.has_active_presentation_motion());

    ui.set_now(millis(50));
    assert_close(f64::from(ui.presented_layer_opacity(panel)), 0.625);
    ui.set_now(millis(100));
    assert_eq!(ui.presented_layer_opacity(panel), 0.25);
    assert_eq!(ui.resolved_value(panel, opacity), Some(0.25));
}

#[test]
fn motion_scope_retargets_a_property_from_its_presented_value() {
    let mut ui = Ui::new();
    let panel = ui.append(
        ui.root(),
        crate::Panel::new().layout(Layout::flex().logical_size(120.0, 40.0)),
    );
    let opacity = ui.properties().opacity;
    ui.compute_layout(Size::new(160.0, 80.0));

    ui.set_now(0);
    ui.with_motion(MotionSpec::new(millis(100)), |ui| {
        ui.set_local(panel, opacity, 0.0_f32);
    });
    ui.set_now(millis(50));
    assert_close(f64::from(ui.presented_layer_opacity(panel)), 0.5);

    ui.with_motion(MotionSpec::new(millis(100)), |ui| {
        ui.set_local(panel, opacity, 0.8_f32);
    });

    assert_close(f64::from(ui.presented_layer_opacity(panel)), 0.5);
    ui.set_now(millis(100));
    assert_close(f64::from(ui.presented_layer_opacity(panel)), 0.65);
    ui.set_now(millis(150));
    assert_eq!(ui.presented_layer_opacity(panel), 0.8);
}

#[test]
fn motion_scope_preserves_reversing_shortening() {
    let mut ui = Ui::new();
    let panel = ui.append(
        ui.root(),
        crate::Panel::new().layout(Layout::flex().logical_size(120.0, 40.0)),
    );
    let opacity = ui.properties().opacity;
    ui.compute_layout(Size::new(160.0, 80.0));

    ui.set_now(0);
    ui.with_motion(MotionSpec::new(millis(100)), |ui| {
        ui.set_local(panel, opacity, 0.0_f32);
    });
    ui.set_now(millis(50));
    assert_close(f64::from(ui.presented_layer_opacity(panel)), 0.5);

    ui.with_motion(MotionSpec::new(millis(100)), |ui| {
        ui.set_local(panel, opacity, 1.0_f32);
    });

    ui.set_now(millis(75));
    assert_close(f64::from(ui.presented_layer_opacity(panel)), 0.75);
    ui.set_now(millis(100));
    assert_eq!(ui.presented_layer_opacity(panel), 1.0);
}

#[test]
fn motion_scope_applies_to_synchronous_binding_destinations() {
    use crate::BindingSpecExt;

    let mut ui = Ui::new();
    let opacity = ui.properties().opacity;
    let source = ui.local_state(1.0_f32);
    let panel = ui.append(
        ui.root(),
        crate::Panel::new()
            .bind_property(opacity, source.binding())
            .layout(Layout::flex().logical_size(120.0, 40.0)),
    );
    ui.compute_layout(Size::new(160.0, 80.0));

    ui.set_now(0);
    ui.with_motion(MotionSpec::new(millis(100)), |ui| {
        assert!(source.set(ui, 0.4));
        ui.flush_bindings();
    });

    assert_eq!(ui.presented_layer_opacity(panel), 1.0);
    ui.set_now(millis(50));
    assert_close(f64::from(ui.presented_layer_opacity(panel)), 0.7);
    ui.set_now(millis(100));
    assert_eq!(ui.resolved_value(panel, opacity), Some(0.4));

    ui.with_motion(MotionSpec::new(millis(100)), |ui| {
        assert!(source.set(ui, 0.8));
    });
    ui.flush_bindings();

    assert_eq!(ui.presented_layer_opacity(panel), 0.8);
    assert!(!ui.has_active_presentation_motion());
}

#[test]
fn motion_scope_overrides_missing_style_motion_for_state_projection() {
    const MOTION_PANEL: ClassId = ClassId(120);

    let mut ui = Ui::new();
    let opacity = ui.properties().opacity;
    ui.set_cascade(
        StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                SelectorBuilder::new(
                    SelectorStep::type_tag(TYPE_GENERIC)
                        .with_class(MOTION_PANEL)
                        .with_pseudo(PSEUDO_SELECTED),
                )
                .build(),
                StyleBuilder::new().set(opacity, 0.2_f32).build(),
            )
            .build(),
    );
    let panel = ui.append(ui.root(), crate::Panel::new().class(MOTION_PANEL));
    ui.compute_layout(Size::new(160.0, 80.0));

    ui.set_now(0);
    ui.with_motion(MotionSpec::new(millis(100)), |ui| {
        ui.add_pseudo(panel, PSEUDO_SELECTED);
    });

    assert_eq!(ui.presented_layer_opacity(panel), 1.0);
    ui.set_now(millis(50));
    assert_close(f64::from(ui.presented_layer_opacity(panel)), 0.6);
    ui.set_now(millis(100));
    assert_eq!(ui.resolved_value(panel, opacity), Some(0.2));
}

#[test]
fn nested_motion_scope_temporarily_overrides_its_parent() {
    let mut ui = Ui::new();
    let root = ui.root();
    let outer = ui.append(root, crate::Panel::new());
    let inner = ui.append(root, crate::Panel::new());
    let opacity = ui.properties().opacity;
    ui.compute_layout(Size::new(160.0, 80.0));

    ui.set_now(0);
    ui.with_motion(MotionSpec::new(millis(100)), |ui| {
        ui.set_local(outer, opacity, 0.0_f32);
        ui.with_motion(MotionSpec::new(millis(200)), |ui| {
            ui.set_local(inner, opacity, 0.0_f32);
        });
    });

    ui.set_now(millis(50));
    assert_close(f64::from(ui.presented_layer_opacity(outer)), 0.5);
    assert_close(f64::from(ui.presented_layer_opacity(inner)), 0.75);
}

#[test]
fn motion_scope_does_not_animate_initial_insertion() {
    let mut ui = Ui::new();
    let root = ui.root();
    let panel = ui.with_motion(MotionSpec::new(millis(100)), |ui| {
        ui.append(root, crate::Panel::new().opacity(0.25))
    });

    assert_eq!(ui.presented_layer_opacity(panel), 0.25);
    assert!(!ui.has_active_presentation_motion());
    ui.compute_layout(Size::new(160.0, 80.0));
    assert!(!ui.has_active_presentation_motion());
}

#[test]
fn removing_a_motion_target_tears_down_its_transaction_track() {
    let mut ui = Ui::new();
    let panel = ui.append(ui.root(), crate::Panel::new());
    let opacity = ui.properties().opacity;
    ui.compute_layout(Size::new(160.0, 80.0));

    ui.with_motion(MotionSpec::new(millis(100)), |ui| {
        ui.set_local(panel, opacity, 0.0_f32);
        ui.remove(panel);
    });

    assert!(ui.get(panel).is_none());
    assert!(!ui.has_active_presentation_motion());
}

#[test]
fn motion_scope_includes_value_changes_derived_from_range_coercion() {
    let mut ui = Ui::new();
    let panel = ui.append(ui.root(), crate::Panel::new());
    let value = ui.properties().value;
    let maximum = ui.properties().maximum;
    ui.register_animation_target_property(value);
    ui.set_local(panel, maximum, 100.0);
    ui.set_local(panel, value, 80.0);
    ui.compute_layout(Size::new(160.0, 80.0));

    ui.set_now(0);
    ui.with_motion(MotionSpec::new(millis(100)), |ui| {
        ui.set_local(panel, maximum, 40.0);
    });

    assert_eq!(
        ui.resolve_value_for(ui.get(panel).unwrap(), value),
        80.0,
        "the derived coerced value should begin at its presented value"
    );
    ui.set_now(millis(50));
    assert_eq!(ui.resolve_value_for(ui.get(panel).unwrap(), value), 60.0);
}

#[test]
fn motion_scope_honors_color_interpolation_for_custom_color_properties() {
    let mut ui = Ui::new();
    let from = Color::from_rgba8(0xff, 0x00, 0x00, 0xff);
    let to = Color::from_rgba8(0x00, 0xff, 0x00, 0xff);
    let color = ui
        .registry_mut()
        .register("CustomColor", PropertyMetadataBuilder::new(from).build());
    ui.register_animation_target_property(color);
    let panel = ui.append(ui.root(), crate::Panel::new());
    ui.compute_layout(Size::new(160.0, 80.0));

    ui.set_now(0);
    ui.with_motion(
        MotionSpec::new(millis(100)).with_color_interpolation(
            ColorSpaceTag::Oklab,
            HueDirection::Shorter,
            InterpolationAlphaSpace::Premultiplied,
        ),
        |ui| ui.set_local(panel, color, to),
    );
    ui.set_now(millis(50));

    let expected = DynamicColor::from_alpha_color(from)
        .interpolate(
            DynamicColor::from_alpha_color(to),
            ColorSpaceTag::Oklab,
            HueDirection::Shorter,
        )
        .eval(0.5)
        .to_alpha_color::<Srgb>();
    assert_eq!(
        ui.resolve_value_for(ui.get(panel).unwrap(), color),
        expected
    );
}

#[test]
fn motion_scope_resolves_complete_color_interpolation_from_theme() {
    use understory_motion::ColorInterpolation as RuntimeColorInterpolation;

    const COLOR_SPACE: ResourceKey = ResourceKey::new(1_206);
    const HUE_DIRECTION: ResourceKey = ResourceKey::new(1_207);
    const ALPHA_SPACE: ResourceKey = ResourceKey::new(1_208);

    let mut ui = Ui::new();
    ui.set_theme(
        ThemeBuilder::new()
            .set(COLOR_SPACE, ColorSpaceTag::Hsl)
            .set(HUE_DIRECTION, HueDirection::Increasing)
            .set(ALPHA_SPACE, InterpolationAlphaSpace::Unpremultiplied)
            .build(),
    );
    let from = Color::from_rgba8(0xff, 0x00, 0x00, 0x20);
    let to = Color::from_rgba8(0x00, 0x00, 0xff, 0xe0);
    let color = ui.registry_mut().register(
        "ThemeResolvedCustomColor",
        PropertyMetadataBuilder::new(from).build(),
    );
    ui.register_animation_target_property(color);
    let panel = ui.append(ui.root(), crate::Panel::new());
    ui.compute_layout(Size::new(160.0, 80.0));

    ui.set_now(0);
    ui.with_motion(
        MotionSpec::new(millis(100))
            .with_color_space_resource(COLOR_SPACE)
            .with_hue_direction_resource(HUE_DIRECTION)
            .with_alpha_space_resource(ALPHA_SPACE),
        |ui| ui.set_local(panel, color, to),
    );
    ui.set_now(millis(50));

    assert_eq!(
        ui.resolve_value_for(ui.get(panel).unwrap(), color),
        RuntimeColorInterpolation::new(
            ColorSpaceTag::Hsl,
            HueDirection::Increasing,
            InterpolationAlphaSpace::Unpremultiplied,
        )
        .interpolate(from, to, 0.5),
    );
}

#[test]
fn custom_color_registration_preserves_explicit_typed_animation() {
    let mut ui = Ui::new();
    let from = Color::from_rgba8(0xff, 0x00, 0x00, 0xff);
    let transitioned = Color::from_rgba8(0x00, 0xff, 0x00, 0xff);
    let animated = Color::from_rgba8(0x00, 0x00, 0xff, 0xff);
    let color = ui.registry_mut().register(
        "AnimatedCustomColor",
        PropertyMetadataBuilder::new(from).build(),
    );
    ui.register_animation_target_property(color);
    let panel = ui.append(ui.root(), crate::Panel::new());
    ui.compute_layout(Size::new(160.0, 80.0));

    ui.with_motion(MotionSpec::new(millis(100)), |ui| {
        ui.set_local(panel, color, transitioned);
    });
    ui.set_now(millis(100));

    let animation = ui.animate_target(
        panel,
        AnimationTarget::property(color),
        EffectClip::from_keyframes([
            EffectKeyframe::new(0, transitioned),
            EffectKeyframe::new(millis(100), animated),
        ]),
    );

    assert!(
        animation.is_some(),
        "a transitioned property must retain the same typed runtime track for explicit effects"
    );
    ui.set_now(millis(150));
    assert_ne!(
        ui.resolve_value_for(ui.get(panel).unwrap(), color),
        transitioned,
        "color transition registration must retain the native typed adapter"
    );
}

#[test]
fn explicit_animation_then_scoped_transition_share_a_presentation_property_track() {
    let mut ui = Ui::new();
    let opacity = ui.properties().opacity;
    let panel = ui.append(ui.root(), crate::Panel::new());
    ui.compute_layout(Size::new(160.0, 80.0));

    let animation = ui
        .animate_property(
            panel,
            opacity,
            EffectClip::from_keyframes([
                EffectKeyframe::new(0, 1.0_f32),
                EffectKeyframe::new(millis(100), 0.5_f32),
            ]),
        )
        .expect("the explicit animation should start");
    ui.set_now(millis(50));

    ui.with_motion(MotionSpec::new(millis(100)), |ui| {
        ui.set_local(panel, opacity, 0.25_f32);
    });

    assert_eq!(
        ui.get(panel)
            .and_then(|element| element.get_local_value(opacity)),
        Some(&0.25),
    );
    assert!(
        ui.animation_state(animation).is_some(),
        "changing the underlying target must not discard the explicit effect"
    );
}

#[test]
fn registered_custom_brush_property_honors_typed_color_transition() {
    const PANEL: ClassId = ClassId(1_204);

    let mut ui = Ui::new();
    let from = Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff));
    let to = Brush::Solid(Color::from_rgba8(0x00, 0xff, 0x00, 0xff));
    let custom = ui.registry_mut().register(
        "CustomBrush",
        PropertyMetadataBuilder::new(from.clone()).build(),
    );
    ui.register_color_animation_target_property(custom);
    let selector =
        SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC).with_class(PANEL)).build();
    let selected = SelectorBuilder::new(
        SelectorStep::type_tag(TYPE_GENERIC)
            .with_class(PANEL)
            .with_pseudo(PSEUDO_SELECTED),
    )
    .build();
    ui.set_cascade(
        StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                selector,
                StyleBuilder::new()
                    .set(custom, from.clone())
                    .set(
                        ui.properties().transitions,
                        Transitions::new().color(custom, MotionSpec::new(millis(100))),
                    )
                    .build(),
            )
            .push_rule(
                StyleOrigin::Base,
                selected,
                StyleBuilder::new().set(custom, to.clone()).build(),
            )
            .build(),
    );
    let panel = ui.append(ui.root(), crate::Panel::new().class(PANEL));
    ui.compute_layout(Size::new(160.0, 80.0));

    ui.set_now(0);
    ui.add_pseudo(panel, PSEUDO_SELECTED);

    assert_eq!(
        ui.resolve_value_for(ui.get(panel).unwrap(), custom),
        from,
        "accepted typed color properties should not silently snap"
    );
    assert!(ui.has_active_presentation_motion());
}

#[test]
#[should_panic(expected = "is not registered")]
fn unregistered_custom_brush_transition_fails_at_the_state_change() {
    const PANEL: ClassId = ClassId(1_205);

    let mut ui = Ui::new();
    let custom = ui.registry_mut().register(
        "UnregisteredCustomBrush",
        PropertyMetadataBuilder::new(Brush::Solid(Color::BLACK)).build(),
    );
    ui.set_cascade(
        StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC).with_class(PANEL))
                    .build(),
                StyleBuilder::new()
                    .set(
                        ui.properties().transitions,
                        Transitions::new().color(custom, MotionSpec::new(millis(100))),
                    )
                    .build(),
            )
            .build(),
    );
    let panel = ui.append(ui.root(), crate::Panel::new().class(PANEL));
    ui.compute_layout(Size::new(160.0, 80.0));

    ui.add_pseudo(panel, PSEUDO_SELECTED);
}

#[test]
fn initial_disabled_style_settles_without_transition_motion() {
    use crate::{Button, TYPE_BUTTON};

    let install = |ui: &mut Ui| {
        let opacity = ui.properties().opacity;
        ui.set_cascade(
            StyleCascadeBuilder::new()
                .push_rule(
                    StyleOrigin::Base,
                    SelectorBuilder::new(SelectorStep::type_tag(TYPE_BUTTON)).build(),
                    StyleBuilder::new()
                        .set(
                            ui.properties().transitions,
                            transition(AnimationTarget::property(opacity), millis(120)),
                        )
                        .build(),
                )
                .push_rule(
                    StyleOrigin::Base,
                    SelectorBuilder::new(
                        SelectorStep::type_tag(TYPE_BUTTON).with_pseudo(PSEUDO_DISABLED),
                    )
                    .build(),
                    StyleBuilder::new().set(opacity, 0.5_f32).build(),
                )
                .build(),
        );
    };

    for install_before_build in [true, false] {
        let mut ui = Ui::new();
        if install_before_build {
            install(&mut ui);
        }
        let root = ui.root();
        let button = ui.append(root, Button::new("Disabled").disabled());
        if !install_before_build {
            install(&mut ui);
        }
        let opacity = ui.properties().opacity;

        assert_eq!(ui.resolved_value(button, opacity), Some(0.5));
        assert_eq!(ui.presented_layer_opacity(button), 0.5);
        assert!(!ui.has_active_presentation_motion());
        let timeline = ui.inspection_snapshot().timeline;
        assert_eq!(timeline.active_transitions, 0);
        assert_eq!(timeline.pending_transitions, 0);

        ui.compute_layout(Size::new(200.0, 80.0));
        ui.set_now(millis(10_000));
        assert!(!ui.has_active_presentation_motion());

        ui.set_enabled(button, true);
        assert!(ui.has_active_presentation_motion());
        assert_eq!(ui.presented_layer_opacity(button), 0.5);
        ui.set_now(millis(10_060));
        assert_close(
            f64::from(ui.presented_layer_opacity(button)),
            f64::from(0.75_f32),
        );
        ui.set_now(millis(10_120));
        assert_eq!(ui.presented_layer_opacity(button), 1.0);
        assert!(!ui.has_active_presentation_motion());
    }
}

#[test]
fn initial_template_state_does_not_queue_layout_motion() {
    use crate::{SLOT_TOGGLE_THUMB, ToggleSwitch, theme, toggle_checked};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(200.0, 80.0));

    let toggle = ui.append(root, ToggleSwitch::new().checked(true));
    let thumb = ui
        .slot(toggle, SLOT_TOGGLE_THUMB)
        .expect("toggle template should create a thumb slot");
    let timeline = ui.inspection_snapshot().timeline;
    assert_eq!(timeline.active_transitions, 0);
    assert_eq!(timeline.pending_transitions, 0);

    ui.compute_layout(Size::new(200.0, 80.0));
    let checked_x = layer_x(&ui, thumb);
    assert_close(checked_x, ui.effective_bounds(thumb).x0);
    assert!(!ui.has_active_presentation_motion());

    ui.set_now(millis(10_000));
    ui.compute_layout(Size::new(200.0, 80.0));
    assert_close(layer_x(&ui, thumb), checked_x);
    assert!(!ui.has_active_presentation_motion());

    toggle_checked(&mut ui, toggle);
    ui.compute_layout(Size::new(200.0, 80.0));
    assert!(ui.has_active_presentation_motion());
    assert_close(layer_x(&ui, thumb), checked_x);
}

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
    let reversing = layer_x(&ui, thumb);
    assert!(reversing < midpoint);
    assert!(reversing > start_x);

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
fn triggered_effect_queues_animation_lifecycle_events() {
    const EFFECTFUL: ClassId = ClassId(903);
    const TRIGGER: EffectTriggerId = EffectTriggerId::new(903);

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
                                    millis(100),
                                    PresentationValue::Offset2d(Vec2::new(10.0, 0.0)),
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

    let started = ui.drain_animation_events();
    assert_eq!(started.len(), 1);
    assert_eq!(started[0].element(), panel);
    assert_eq!(
        started[0].target(),
        PresentationTarget::visual_translation()
    );
    assert_eq!(started[0].kind(), crate::AnimationEventKind::Started);
    assert_eq!(started[0].at(), 0);

    ui.set_now(millis(100));
    ui.flush_composition();

    let finished = ui.drain_animation_events();
    assert_eq!(finished.len(), 1);
    assert_eq!(finished[0].id(), started[0].id());
    assert_eq!(finished[0].kind(), crate::AnimationEventKind::Finished);
    assert_eq!(finished[0].at(), millis(100));
}

#[test]
fn ui_animate_returns_id_and_controls_playback() {
    let mut ui = Ui::new();
    let root = ui.root();
    let panel = ui.append(root, crate::Panel::new());

    ui.set_now(0);
    let animation = ui
        .animate(
            panel,
            PresentationTarget::visual_translation(),
            EffectClip::from_keyframes([
                EffectKeyframe::new(0, PresentationValue::Offset2d(Vec2::ZERO)),
                EffectKeyframe::new(
                    millis(100),
                    PresentationValue::Offset2d(Vec2::new(10.0, 0.0)),
                ),
            ]),
        )
        .expect("visual translation clip should match the target");

    let started = ui.drain_animation_events();
    assert_eq!(started.len(), 1);
    assert_eq!(started[0].id(), animation);
    assert_eq!(started[0].kind(), crate::AnimationEventKind::Started);
    assert_eq!(
        ui.animation_state(animation),
        Some(crate::AnimationPlayState::Running)
    );
    assert_eq!(ui.animation_current_time(animation), Some(0));
    assert_eq!(ui.animation_playback_rate(animation), Some(1.0));
    assert!(ui.has_active_presentation_motion());

    ui.set_now(millis(50));
    assert!(ui.pause_animation(animation));
    assert_eq!(
        ui.animation_state(animation),
        Some(crate::AnimationPlayState::Paused)
    );
    assert_eq!(ui.animation_current_time(animation), Some(millis(50)));
    assert!(!ui.has_active_presentation_motion());

    ui.set_now(millis(90));
    assert_eq!(ui.animation_current_time(animation), Some(millis(50)));
    assert!(ui.seek_animation(animation, millis(25)));
    assert_eq!(ui.animation_current_time(animation), Some(millis(25)));
    assert!(ui.play_animation(animation));
    assert!(ui.has_active_presentation_motion());

    assert!(ui.cancel_animation(animation));
    let canceled = ui.drain_animation_events();
    assert_eq!(canceled.len(), 1);
    assert_eq!(canceled[0].id(), animation);
    assert_eq!(canceled[0].kind(), crate::AnimationEventKind::Canceled);
    assert_eq!(ui.animation_state(animation), None);
    assert!(!ui.has_active_presentation_motion());
}

#[test]
fn ui_animate_uses_clip_timing_policy() {
    let mut ui = Ui::new();
    let root = ui.root();
    let panel = ui.append(root, crate::Panel::new());

    ui.set_now(0);
    let animation = ui
        .animate_target(
            panel,
            AnimationTarget::visual_translation(),
            EffectClip::from_keyframes([
                EffectKeyframe::new(0, Vec2::ZERO),
                EffectKeyframe::new(millis(100), Vec2::new(10.0, 0.0)),
            ])
            .with_timing(crate::AnimationTiming {
                duration: millis(200),
                fill: crate::FillMode::Forwards,
                ..crate::AnimationTiming::new(millis(200))
            }),
        )
        .expect("visual translation clip should match the target");

    ui.set_now(millis(50));
    ui.flush_composition();

    let translation = ui.presented_value(
        panel,
        PresentationTarget::slot(PresentationSlot::VisualTranslation),
        Vec2::ZERO,
    );
    assert_close(translation.x, 2.5);
    let timing_sample = ui
        .animation_timing_sample(animation)
        .expect("running effect should expose timing");
    assert_eq!(timing_sample.phase, crate::TimingPhase::Active);
    assert_eq!(timing_sample.iteration, Some(0));
    assert_eq!(timing_sample.progress, Some(0.25));
    assert_eq!(timing_sample.eased_progress, Some(0.25));
    assert_eq!(
        ui.animation_state(animation),
        Some(crate::AnimationPlayState::Running)
    );
    let row = ui
        .inspection_snapshot()
        .timeline
        .animations
        .into_iter()
        .find(|row| row.id == animation)
        .expect("typed visual translation animation should be inspectable");
    assert_eq!(row.value_kind, Some(PresentationValueKind::Offset2d));
}

#[test]
fn ui_animate_rejects_clip_kind_that_does_not_match_target() {
    let mut ui = Ui::new();
    let root = ui.root();
    let panel = ui.append(root, crate::Panel::new());

    assert!(
        ui.animate(
            panel,
            PresentationTarget::visual_translation(),
            EffectClip::from_keyframes([
                EffectKeyframe::new(0, PresentationValue::Scalar32(0.0)),
                EffectKeyframe::new(millis(100), PresentationValue::Scalar32(1.0)),
            ]),
        )
        .is_none()
    );
}

#[test]
fn custom_animatable_property_requires_registration() {
    let mut ui = Ui::new();
    let root = ui.root();
    let rotation = ui.registry_mut().register(
        "RotationDegrees",
        PropertyMetadataBuilder::new(Degrees(0.0)).build(),
    );
    let panel = ui.append(root, crate::Panel::new());
    let target = PresentationTarget::property(rotation.id());
    let clip = EffectClip::from_keyframes([
        EffectKeyframe::new(0, PresentationValue::Scalar64(0.0)),
        EffectKeyframe::new(millis(100), PresentationValue::Scalar64(90.0)),
    ]);

    ui.set_now(0);
    assert!(
        ui.animate(panel, target, clip.clone()).is_none(),
        "unregistered custom properties should not be timeline targets"
    );

    ui.register_animatable_property(rotation);
    let animation = ui
        .animate(panel, target, clip)
        .expect("registered custom property should accept matching scalar animation");
    assert_eq!(
        ui.animation_state(animation),
        Some(crate::AnimationPlayState::Running)
    );

    ui.set_now(millis(50));
    let element = ui.get(panel).expect("panel should exist");
    assert_close(ui.resolve_value_for(element, rotation).0, 45.0);
}

#[test]
fn native_property_animation_registers_custom_property_target() {
    let mut ui = Ui::new();
    let root = ui.root();
    let rotation = ui.registry_mut().register(
        "RotationDegrees",
        PropertyMetadataBuilder::new(Degrees(0.0)).build(),
    );
    let panel = ui.append(root, crate::Panel::new());
    let clip = EffectClip::from_keyframes([
        EffectKeyframe::new(0, Degrees(0.0)),
        EffectKeyframe::new(millis(100), Degrees(90.0)),
    ]);

    ui.set_now(0);
    let animation = ui
        .animate_target(panel, AnimationTarget::property(rotation), clip)
        .expect("native target animation should register and start the property target");
    assert_eq!(
        ui.animation_state(animation),
        Some(crate::AnimationPlayState::Running)
    );

    ui.set_now(millis(50));
    let element = ui.get(panel).expect("panel should exist");
    assert_close(ui.resolve_value_for(element, rotation).0, 45.0);
}

#[test]
fn native_property_animation_does_not_require_presentation_value_kind() {
    let mut ui = Ui::new();
    let root = ui.root();
    let pressure = ui.registry_mut().register(
        "Pressure",
        PropertyMetadataBuilder::new(PressureReading(0.0)).build(),
    );
    let panel = ui.append(root, crate::Panel::new());

    ui.set_now(0);
    let animation = ui
        .animate_property(
            panel,
            pressure,
            EffectClip::from_keyframes([
                EffectKeyframe::new(0, PressureReading(10.0)),
                EffectKeyframe::new(millis(100), PressureReading(30.0)),
            ]),
        )
        .expect("native target stack should accept custom animatable property values");

    let started = ui.drain_animation_events();
    assert_eq!(started.len(), 1);
    assert_eq!(started[0].id(), animation);
    assert_eq!(started[0].kind(), crate::AnimationEventKind::Started);
    let row = ui
        .inspection_snapshot()
        .timeline
        .animations
        .into_iter()
        .find(|row| row.id == animation)
        .expect("custom native property animation should be inspectable");
    assert_eq!(row.value_kind, None);

    ui.set_now(millis(50));
    let element = ui.get(panel).expect("panel should exist");
    assert_eq!(
        ui.resolve_value_for(element, pressure),
        PressureReading(20.0)
    );
    assert_eq!(ui.animation_current_time(animation), Some(millis(50)));
    assert_eq!(
        ui.animation_state(animation),
        Some(crate::AnimationPlayState::Running)
    );

    assert!(ui.pause_animation(animation));
    assert_eq!(
        ui.animation_state(animation),
        Some(crate::AnimationPlayState::Paused)
    );
    assert!(ui.seek_animation(animation, millis(75)));
    let element = ui.get(panel).expect("panel should exist");
    assert_eq!(
        ui.resolve_value_for(element, pressure),
        PressureReading(25.0)
    );
}

#[test]
fn transition_rules_drive_custom_native_property_target() {
    const MOTION_PANEL: ClassId = ClassId(903);

    let mut ui = Ui::new();
    let root = ui.root();
    let pressure = ui.registry_mut().register(
        "Pressure",
        PropertyMetadataBuilder::new(PressureReading(0.0)).build(),
    );
    ui.register_animation_target_property(pressure);

    let transitions = ui.properties().transitions;
    let selector =
        SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC).with_class(MOTION_PANEL)).build();
    let selected_selector = SelectorBuilder::new(
        SelectorStep::type_tag(TYPE_GENERIC)
            .with_class(MOTION_PANEL)
            .with_pseudo(PSEUDO_SELECTED),
    )
    .build();
    ui.set_cascade(
        StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                selector,
                StyleBuilder::new()
                    .set(pressure, PressureReading(10.0))
                    .set(
                        transitions,
                        transition(AnimationTarget::property(pressure), millis(100)),
                    )
                    .build(),
            )
            .push_rule(
                StyleOrigin::Base,
                selected_selector,
                StyleBuilder::new()
                    .set(pressure, PressureReading(30.0))
                    .build(),
            )
            .build(),
    );

    let panel = ui.append(root, crate::Panel::new().class(MOTION_PANEL));
    ui.compute_layout(Size::new(100.0, 50.0));
    assert_eq!(
        ui.resolve_value_for(ui.get(panel).expect("panel should exist"), pressure),
        PressureReading(10.0)
    );

    ui.set_now(0);
    ui.add_pseudo(panel, PSEUDO_SELECTED);
    assert_eq!(
        ui.resolve_value_for(ui.get(panel).expect("panel should exist"), pressure),
        PressureReading(10.0)
    );

    ui.set_now(millis(50));
    assert_eq!(
        ui.resolve_value_for(ui.get(panel).expect("panel should exist"), pressure),
        PressureReading(20.0)
    );
    let snapshot = ui.inspection_snapshot();
    assert_eq!(snapshot.timeline.active_transitions, 1);
    assert!(snapshot.timeline.animations.is_empty());

    ui.set_now(millis(100));
    assert_eq!(
        ui.resolve_value_for(ui.get(panel).expect("panel should exist"), pressure),
        PressureReading(30.0)
    );
}

#[test]
fn manual_timeline_drives_native_property_animation_without_frame_demand() {
    let mut ui = Ui::new();
    let root = ui.root();
    let rotation = ui.registry_mut().register(
        "RotationDegrees",
        PropertyMetadataBuilder::new(Degrees(0.0)).build(),
    );
    let panel = ui.append(root, crate::Panel::new());
    let timeline = ui.create_manual_animation_timeline();
    let clip = EffectClip::from_keyframes([
        EffectKeyframe::new(0, Degrees(0.0)),
        EffectKeyframe::new(millis(100), Degrees(90.0)),
    ]);

    ui.set_now(1_000);
    let animation = ui
        .animate_target_on_timeline(panel, AnimationTarget::property(rotation), clip, timeline)
        .expect("manual timeline should drive native property animation");
    assert_eq!(
        ui.animation_state(animation),
        Some(crate::AnimationPlayState::Running)
    );
    assert_eq!(ui.manual_animation_timeline_time(timeline), Some(0));
    assert!(
        !ui.has_active_presentation_motion(),
        "manual timeline progress should not request document animation frames"
    );
    assert_eq!(
        ui.inspection_snapshot().timeline.animations[0].timeline,
        timeline
    );
    let _ = ui.drain_animation_events();

    assert!(ui.seek_manual_animation_timeline(timeline, millis(50)));
    assert_eq!(ui.animation_current_time(animation), Some(millis(50)));
    let element = ui.get(panel).expect("panel should exist");
    assert_close(ui.resolve_value_for(element, rotation).0, 45.0);

    assert!(ui.seek_manual_animation_timeline(timeline, millis(100)));
    assert_eq!(
        ui.animation_state(animation),
        Some(crate::AnimationPlayState::Running),
        "manual timeline scrubbing should not drop a finite animation at its endpoint",
    );
    assert!(
        ui.drain_animation_events()
            .iter()
            .all(|event| event.kind() != crate::AnimationEventKind::Finished),
        "manual timeline seeking should not queue finished events that make later scrubs inert",
    );
    let element = ui.get(panel).expect("panel should exist");
    assert_close(ui.resolve_value_for(element, rotation).0, 90.0);

    assert!(ui.seek_manual_animation_timeline(timeline, millis(25)));
    assert_eq!(ui.animation_current_time(animation), Some(millis(25)));
    let element = ui.get(panel).expect("panel should exist");
    assert_close(ui.resolve_value_for(element, rotation).0, 22.5);
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Degrees(f64);

impl Interpolate for Degrees {
    fn interpolate(&self, to: &Self, progress: f64) -> Self {
        Self(self.0.interpolate(&to.0, progress))
    }
}

impl Animatable for Degrees {
    type Velocity = f64;

    fn is_close(&self, other: &Self) -> bool {
        self.0.is_close(&other.0)
    }

    fn add(&self, other: &Self) -> Option<Self> {
        self.0.add(&other.0).map(Self)
    }

    fn accumulate(&self, other: &Self, count: u64) -> Option<Self> {
        self.0.accumulate(&other.0, count).map(Self)
    }
}

impl crate::PresentationAnimationValue for Degrees {
    const KIND: PresentationValueKind = PresentationValueKind::Scalar64;

    fn into_presentation_value(self) -> PresentationValue {
        PresentationValue::Scalar64(self.0)
    }

    fn from_presentation_value(value: PresentationValue) -> Option<Self> {
        match value {
            PresentationValue::Scalar64(value) => Some(Self(value)),
            PresentationValue::Scalar32(_)
            | PresentationValue::Color(_)
            | PresentationValue::Offset2d(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct PressureReading(f64);

impl Interpolate for PressureReading {
    fn interpolate(&self, to: &Self, progress: f64) -> Self {
        Self(self.0.interpolate(&to.0, progress))
    }
}

impl Animatable for PressureReading {
    type Velocity = f64;

    fn is_close(&self, other: &Self) -> bool {
        self.0.is_close(&other.0)
    }

    fn add(&self, other: &Self) -> Option<Self> {
        self.0.add(&other.0).map(Self)
    }

    fn accumulate(&self, other: &Self, count: u64) -> Option<Self> {
        self.0.accumulate(&other.0, count).map(Self)
    }
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
                        EffectRules::single(EffectRule::for_target(
                            TRIGGER,
                            AnimationTarget::property(opacity),
                            EffectClip::from_keyframes([
                                EffectKeyframe::new(0, 1.0_f32),
                                EffectKeyframe::new(millis(50), 0.25_f32),
                                EffectKeyframe::new(millis(100), 1.0_f32),
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
                    Transitions::new().color(
                        background,
                        MotionSpec::from_duration_resource(TEST_HOVER_DURATION)
                            .with_color_space(ColorSpaceTag::Srgb),
                    ),
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
                    .set(transitions, color_transition(foreground, millis(100)))
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
                    .set(transitions, color_transition(foreground, millis(100)))
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
                    .set(transitions, color_transition(foreground, millis(100)))
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
        .set(ui.properties().transitions, Transitions::new())
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
    from.interpolate(&to, timing.sample(t))
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
