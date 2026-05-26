// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Overstory motion showcase.

extern crate alloc;

use core::cell::Cell;

use overstory::motion::{
    ColorInterpolation, TimingFunction, TransitionRule, TransitionRules, TransitionSpec,
    TransitionTarget, millis,
};
use overstory::pseudos::{PSEUDO_FOCUS_VISIBLE, PSEUDO_PRESSED, PSEUDO_SELECTED};
use overstory::{
    BuiltInProperties, ElementId, Layout, Panel, TYPE_GENERIC, TYPE_TOOLTIP, Text, TimerDuration,
    Tooltip, Ui, layouts, theme,
};
use overstory_host_winit::WindowOptions;
use peniko::{Brush, Color};
use understory_style::{
    ClassId, PseudoClassId, Selector, SelectorStep, StyleBuilder, StyleCascade,
    StyleCascadeBuilder, StyleOrigin,
};

const INITIAL_SIZE: (u32, u32) = (1160, 760);

const CLASS_OKLAB_SWATCH: ClassId = ClassId(100);
const CLASS_OPACITY_SWATCH: ClassId = ClassId(101);
const CLASS_BORDER_SWATCH: ClassId = ClassId(102);
const CLASS_RETARGET_SWATCH: ClassId = ClassId(103);
const CLASS_MOVER_SWATCH: ClassId = ClassId(104);
const CLASS_SRGB_SWATCH: ClassId = ClassId(105);

const EASE_OUT: TimingFunction = TimingFunction::cubic_bezier(0.16, 1.0, 0.3, 1.0);
const EASE_IN_OUT: TimingFunction = TimingFunction::cubic_bezier(0.65, 0.0, 0.35, 1.0);
const MID_SLOW: TimingFunction = TimingFunction::cubic_bezier(0.2, 0.48, 0.8, 0.52);

fn main() -> overstory_host_winit::HostResult<()> {
    overstory_host_winit::run(
        WindowOptions::new("overstory_motion_demo").initial_size(INITIAL_SIZE.0, INITIAL_SIZE.1),
        build_ui,
    )
}

fn build_ui() -> Ui {
    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(motion_cascade(ui.properties()));

    let properties = *ui.properties();
    let root = ui.root();
    ui.set_layout(
        root,
        layouts::column_with_gap(26.0)
            .justify_center()
            .align_center()
            .percent_size(1.0, 1.0)
            .padding_all(40.0),
    );
    ui.set_local(
        root,
        properties.background,
        opt_solid(0x17, 0x17, 0x1f, 0xff),
    );

    ui.append(root, heading("Overstory Motion Lab"));
    ui.append(
        root,
        caption("Animated presentation values from style-authored state changes"),
    );

    let grid = ui.append(root, Panel::column(22.0).align_center());
    let top = ui.append(grid, Panel::row(22.0).align_center().justify_center());
    let bottom = ui.append(grid, Panel::row(22.0).align_center().justify_center());

    let color_swatches = color_comparison_specimen(&mut ui, top);
    let opacity = specimen(&mut ui, top, "Layer Opacity", CLASS_OPACITY_SWATCH);
    let mover = moving_specimen(&mut ui, top);
    let border = specimen(&mut ui, bottom, "Border Width", CLASS_BORDER_SWATCH);
    let (retarget, retarget_label) =
        specimen_with_label(&mut ui, bottom, "Retargeting", CLASS_RETARGET_SWATCH);
    ui.set_tooltip(
        retarget_label,
        Tooltip::new("Retarget: new target starts from current sampled value").font_size(12.0),
    );

    schedule_pair_toggle(&mut ui, color_swatches, PSEUDO_SELECTED, millis(6_200));
    schedule_toggle(&mut ui, opacity, PSEUDO_PRESSED, millis(1_100));
    schedule_movement(&mut ui, mover, millis(1_250));
    schedule_toggle(&mut ui, border, PSEUDO_FOCUS_VISIBLE, millis(1_450));
    schedule_toggle(&mut ui, retarget, PSEUDO_SELECTED, millis(180));

    ui
}

fn motion_cascade(properties: &BuiltInProperties) -> StyleCascade {
    StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_OKLAB_SWATCH),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0xff, 0x2a, 0x78, 0xff))
                .set(properties.corner_radius, 18.0_f64)
                .set(
                    properties.transitions,
                    color_transition_rules(
                        properties,
                        millis(5_500),
                        MID_SLOW,
                        ColorInterpolation::OKLAB,
                    ),
                )
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(CLASS_OKLAB_SWATCH, PSEUDO_SELECTED),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0x26, 0xd9, 0x8e, 0xff))
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_SRGB_SWATCH),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0xff, 0x2a, 0x78, 0xff))
                .set(properties.corner_radius, 18.0_f64)
                .set(
                    properties.transitions,
                    color_transition_rules(
                        properties,
                        millis(5_500),
                        MID_SLOW,
                        ColorInterpolation::SRGB,
                    ),
                )
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(CLASS_SRGB_SWATCH, PSEUDO_SELECTED),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0x26, 0xd9, 0x8e, 0xff))
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_OPACITY_SWATCH),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0x6d, 0xf2, 0x96, 0xff))
                .set(properties.corner_radius, 18.0_f64)
                .set(properties.opacity, 1.0_f32)
                .set(
                    properties.transitions,
                    scalar_transition_rules(
                        TransitionTarget::property(properties.opacity.id()),
                        TransitionSpec::timed(millis(780), EASE_IN_OUT),
                    ),
                )
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(CLASS_OPACITY_SWATCH, PSEUDO_PRESSED),
            StyleBuilder::new()
                .set(properties.opacity, 0.16_f32)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_BORDER_SWATCH),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0x25, 0x29, 0x38, 0xff))
                .set(properties.border_brush, opt_solid(0x79, 0x86, 0xff, 0xff))
                .set(properties.border_width, 2.0_f64)
                .set(properties.corner_radius, 18.0_f64)
                .set(properties.transitions, border_transition_rules(properties))
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(CLASS_BORDER_SWATCH, PSEUDO_FOCUS_VISIBLE),
            StyleBuilder::new()
                .set(properties.border_brush, opt_solid(0xff, 0xff, 0xff, 0xff))
                .set(properties.border_width, 12.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_MOVER_SWATCH),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0xff, 0x9f, 0x1c, 0xff))
                .set(properties.corner_radius, 21.0_f64)
                .set(
                    properties.transitions,
                    scalar_transition_rules(
                        TransitionTarget::layer_offset(),
                        TransitionSpec::timed(millis(920), EASE_IN_OUT),
                    ),
                )
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_RETARGET_SWATCH),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0xd8, 0xb4, 0xfe, 0xff))
                .set(properties.corner_radius, 18.0_f64)
                .set(
                    properties.transitions,
                    color_transition_rules(
                        properties,
                        millis(780),
                        EASE_OUT,
                        ColorInterpolation::OKLAB,
                    ),
                )
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(CLASS_RETARGET_SWATCH, PSEUDO_SELECTED),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0xf9, 0xf8, 0x71, 0xff))
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            Selector::single(SelectorStep::type_tag(TYPE_TOOLTIP)),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0x2c, 0x2c, 0x35, 0xff))
                .set(properties.border_brush, opt_solid(0x86, 0x91, 0xaa, 0xff))
                .set(
                    properties.foreground,
                    Brush::Solid(Color::from_rgba8(0xf4, 0xf4, 0xf7, 0xff)),
                )
                .set(properties.border_width, 1.0_f64)
                .set(properties.corner_radius, 6.0_f64)
                .build(),
        )
        .build()
}

fn specimen(ui: &mut Ui, parent: ElementId, label: &str, class: ClassId) -> ElementId {
    specimen_with_label(ui, parent, label, class).0
}

fn specimen_with_label(
    ui: &mut Ui,
    parent: ElementId,
    label: &str,
    class: ClassId,
) -> (ElementId, ElementId) {
    let card = specimen_card(ui, parent);
    let swatch = ui.append(
        card,
        Panel::new()
            .class(class)
            .layout(Layout::flex().logical_size(226.0, 142.0)),
    );
    let label = ui.append(card, specimen_label(label));
    (swatch, label)
}

fn color_comparison_specimen(ui: &mut Ui, parent: ElementId) -> [ElementId; 2] {
    let card = specimen_card(ui, parent);
    let swatches = ui.append(
        card,
        Panel::row(10.0).layout(Layout::row(10.0).logical_size(226.0, 124.0)),
    );
    let oklab = ui.append(
        swatches,
        Panel::new()
            .class(CLASS_OKLAB_SWATCH)
            .layout(Layout::flex().logical_size(108.0, 124.0)),
    );
    let srgb = ui.append(
        swatches,
        Panel::new()
            .class(CLASS_SRGB_SWATCH)
            .layout(Layout::flex().logical_size(108.0, 124.0)),
    );
    let labels = ui.append(
        card,
        Panel::row(10.0).layout(Layout::row(10.0).logical_size(226.0, 16.0)),
    );
    lane_label(ui, labels, "OKLab");
    lane_label(ui, labels, "sRGB");
    ui.append(card, specimen_label("Color Spaces"));
    [oklab, srgb]
}

fn moving_specimen(ui: &mut Ui, parent: ElementId) -> ElementId {
    let card = specimen_card(ui, parent);
    let stage = ui.append(
        card,
        Panel::new().layout(
            Layout::flex()
                .relative()
                .logical_size(226.0, 142.0)
                .padding_all(0.0),
        ),
    );
    ui.set_local(
        stage,
        ui.properties().background,
        opt_solid(0x19, 0x19, 0x22, 0xff),
    );
    ui.set_local(stage, ui.properties().corner_radius, 18.0_f64);

    let mover = ui.append(
        stage,
        Panel::new()
            .class(CLASS_MOVER_SWATCH)
            .layout(mover_layout(false)),
    );
    ui.append(card, specimen_label("Layer Offset"));
    mover
}

fn specimen_card(ui: &mut Ui, parent: ElementId) -> ElementId {
    let card = ui.append(
        parent,
        Panel::column(15.0).layout(
            Layout::column(15.0)
                .logical_size(258.0, 226.0)
                .padding_all(16.0),
        ),
    );
    ui.set_local(
        card,
        ui.properties().background,
        opt_solid(0x27, 0x27, 0x31, 0xff),
    );
    ui.set_local(
        card,
        ui.properties().foreground,
        Brush::Solid(Color::from_rgba8(0xf4, 0xf4, 0xf7, 0xff)),
    );
    ui.set_local(
        card,
        ui.properties().border_brush,
        opt_solid(0x3b, 0x3b, 0x46, 0xff),
    );
    ui.set_local(card, ui.properties().border_width, 1.0_f64);
    ui.set_local(card, ui.properties().corner_radius, 12.0_f64);
    card
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
    let layout = ui.properties().layout;
    let _ = ui.schedule_timer_repeating(target, interval);
    ui.on_timer(target, move |ui, _event| {
        let next = !active.get();
        active.set(next);
        let _ = ui.request_layer_offset_transition(target);
        ui.set_local(target, layout, mover_layout(next));
    });
}

fn mover_layout(active: bool) -> Layout {
    let x = if active { 168.0 } else { 16.0 };
    Layout::new().absolute_at(x, 50.0).logical_size(42.0, 42.0)
}

fn set_pseudo(ui: &mut Ui, target: ElementId, pseudo: PseudoClassId, active: bool) {
    if active {
        ui.add_pseudo(target, pseudo);
    } else {
        ui.remove_pseudo(target, pseudo);
    }
}

fn class_selector(class: ClassId) -> Selector {
    Selector::single(SelectorStep::type_tag(TYPE_GENERIC).with_class(class))
}

fn class_state_selector(class: ClassId, pseudo: PseudoClassId) -> Selector {
    Selector::single(
        SelectorStep::type_tag(TYPE_GENERIC)
            .with_class(class)
            .with_pseudo(pseudo),
    )
}

fn color_transition_rules(
    properties: &BuiltInProperties,
    duration: TimerDuration,
    timing: TimingFunction,
    interpolation: ColorInterpolation,
) -> TransitionRules {
    TransitionRules::single(TransitionRule::interaction(
        TransitionTarget::property(properties.background.id()),
        TransitionSpec::timed(duration, timing).with_color_interpolation(interpolation),
    ))
}

fn border_transition_rules(properties: &BuiltInProperties) -> TransitionRules {
    TransitionRules::new()
        .with_rule(TransitionRule::interaction(
            TransitionTarget::property(properties.border_brush.id()),
            TransitionSpec::timed(millis(720), EASE_OUT)
                .with_color_interpolation(ColorInterpolation::OKLAB),
        ))
        .with_rule(TransitionRule::interaction(
            TransitionTarget::property(properties.border_width.id()),
            TransitionSpec::timed(millis(520), EASE_IN_OUT),
        ))
}

fn scalar_transition_rules(target: TransitionTarget, spec: TransitionSpec) -> TransitionRules {
    TransitionRules::single(TransitionRule::interaction(target, spec))
}

fn heading(text: &str) -> Text {
    Text::new(text)
        .font_size(28.0)
        .foreground(Color::from_rgba8(0xf4, 0xf4, 0xf7, 0xff))
}

fn caption(text: &str) -> Text {
    Text::new(text)
        .font_size(13.0)
        .foreground(Color::from_rgba8(0xc8, 0xc8, 0xd2, 0xff))
}

fn specimen_label(text: &str) -> Text {
    Text::new(text)
        .font_size(18.0)
        .foreground(Color::from_rgba8(0xf4, 0xf4, 0xf7, 0xff))
}

fn lane_label(ui: &mut Ui, parent: ElementId, text: &str) -> ElementId {
    let id = ui.append(
        parent,
        Text::new(text)
            .font_size(12.0)
            .foreground(Color::from_rgba8(0xc8, 0xc8, 0xd2, 0xff)),
    );
    ui.set_layout(id, Layout::new().logical_size(108.0, 16.0));
    id
}

fn opt_solid(r: u8, g: u8, b: u8, a: u8) -> Option<Brush> {
    Some(Color::from_rgba8(r, g, b, a).into())
}
