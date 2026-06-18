// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Default slider template and tick projection.

use alloc::{format, string::String};

use understory_axis::{
    AxisMapping1D, AxisRuler1D, AxisRulerMark, AxisRulerOptions, AxisScale1D, AxisScaleOptions,
    AxisSubdivisionPolicy, AxisTickKind,
};

use crate::controls::support::{f32_from_logical, read_local_or};
use crate::layouts::{LayoutPlacement, LayoutSize};
use crate::template::{Template, TemplateContext, TemplateItemKey};
use crate::ui::Ui;

use super::behavior::thumb_left_for_value;
use super::ids::{
    CLASS_SLIDER_MAJOR_TICK, CLASS_SLIDER_MINOR_TICK, LABEL_CHAR_WIDTH, LABEL_MIN_WIDTH, LABEL_TOP,
    MAJOR_TICK_HEIGHT, MINOR_TICK_HEIGHT, SLIDER_HEIGHT, SLIDER_WIDTH, SLOT_SLIDER_AXIS,
    SLOT_SLIDER_THUMB, SLOT_SLIDER_TICK, SLOT_SLIDER_TICK_LABEL, SLOT_SLIDER_TRACK, THUMB_DIAMETER,
    TICK_TOP, TICK_WIDTH, TRACK_HEIGHT,
};

static SLIDER_TEMPLATE: SliderTemplate = SliderTemplate;

pub(crate) fn default_slider_template() -> crate::ControlTemplate {
    crate::ControlTemplate::from_static(&SLIDER_TEMPLATE)
}

#[derive(Debug, Default)]
struct SliderTemplate;

impl Template for SliderTemplate {
    fn instantiate(&self, cx: &mut TemplateContext<'_>) {
        let owner = cx.owner();
        let value_prop = cx.properties().value;
        let min_prop = cx.properties().minimum;
        let max_prop = cx.properties().maximum;
        let text_prop = cx.properties().text;

        // Track: full width, vertically centered.
        let track = cx.surface_slot(SLOT_SLIDER_TRACK);
        let track_inset_y = (SLIDER_HEIGHT - TRACK_HEIGHT) / 2.0;
        cx.set_layout_placement(track, LayoutPlacement::absolute_at(0.0, track_inset_y));
        cx.set_layout_size(track, LayoutSize::logical(SLIDER_WIDTH, TRACK_HEIGHT));

        let axis = cx.surface_slot(SLOT_SLIDER_AXIS);
        cx.set_layout_placement(axis, LayoutPlacement::absolute());
        cx.set_layout_size(axis, LayoutSize::logical(SLIDER_WIDTH, SLIDER_HEIGHT));
        let min = cx.read_local_or(owner, min_prop, 0.0);
        let max = cx.read_local_or(owner, max_prop, 1.0);
        let ruler = slider_ruler(min, max);
        for (index, mark) in ruler
            .marks()
            .iter()
            .copied()
            .filter(|mark| ruler_mark_in_view(&ruler, *mark))
            .enumerate()
        {
            let key = tick_key(index);
            let class = tick_class(mark.kind);
            let tick = cx.keyed_surface_slot(SLOT_SLIDER_TICK, key);
            cx.add_class(tick, class);
            cx.set_layout_placement(tick, tick_placement(mark));
            cx.set_layout_size(tick, tick_size(mark));

            if mark.labeled {
                let label = cx.keyed_text_slot(SLOT_SLIDER_TICK_LABEL, key);
                cx.add_class(label, class);
                let content = format_tick_value(mark.value);
                cx.set_layout_placement(label, tick_label_placement(mark, &content));
                cx.set_layout_size(label, tick_label_size(&content));
                cx.set_local(label, text_prop, content);
            }
        }

        // Thumb: absolute, recomputed left whenever Value changes.
        let thumb = cx.surface_slot(SLOT_SLIDER_THUMB);
        let thumb_top = (SLIDER_HEIGHT - THUMB_DIAMETER) / 2.0;
        let initial_left = thumb_left_for_value(
            cx.read_local_or(owner, value_prop, 0.0),
            cx.read_local_or(owner, min_prop, 0.0),
            cx.read_local_or(owner, max_prop, 1.0),
        );
        cx.set_layout_placement(thumb, LayoutPlacement::absolute_at(initial_left, thumb_top));
        cx.set_layout_size(thumb, LayoutSize::logical(THUMB_DIAMETER, THUMB_DIAMETER));
    }
}

fn slider_ruler(min: f64, max: f64) -> AxisRuler1D {
    let mapping = AxisMapping1D::linear(0.0..f64::from(SLIDER_WIDTH), min..max);
    let scale = AxisScale1D::from_mapping(&mapping, slider_scale_options());
    AxisRuler1D::from_mapping(&mapping, &scale, slider_ruler_options())
}

fn slider_scale_options() -> AxisScaleOptions {
    AxisScaleOptions {
        target_major_spacing_px: f64::from(SLIDER_WIDTH) / 2.0,
        medium_label_min_spacing_px: 0.0,
        subdivision_policy: AxisSubdivisionPolicy::Fixed(2),
        ..AxisScaleOptions::default()
    }
}

fn slider_ruler_options() -> AxisRulerOptions {
    AxisRulerOptions {
        major_mark_extent: f64::from(MAJOR_TICK_HEIGHT),
        medium_mark_extent: f64::from(MINOR_TICK_HEIGHT),
        minor_mark_extent: f64::from(MINOR_TICK_HEIGHT),
    }
}

fn ruler_mark_in_view(ruler: &AxisRuler1D, mark: AxisRulerMark) -> bool {
    let span = ruler.view_span();
    const EPSILON: f64 = 1e-6;
    mark.view_position >= span.start - EPSILON && mark.view_position <= span.end + EPSILON
}

fn tick_placement(mark: AxisRulerMark) -> LayoutPlacement {
    LayoutPlacement::absolute_at(tick_left(mark.view_position, TICK_WIDTH), TICK_TOP)
}

fn tick_size(mark: AxisRulerMark) -> LayoutSize {
    LayoutSize::logical(TICK_WIDTH, f32_from_logical(mark.mark_extent))
}

fn tick_label_placement(mark: AxisRulerMark, label: &str) -> LayoutPlacement {
    let width = label_width(label);
    LayoutPlacement::absolute_at(tick_left(mark.view_position, width), LABEL_TOP)
}

fn tick_label_size(label: &str) -> LayoutSize {
    LayoutSize::logical(label_width(label), SLIDER_HEIGHT - LABEL_TOP)
}

fn label_width(label: &str) -> f32 {
    let chars = u16::try_from(label.chars().count()).unwrap_or(u16::MAX);
    (f32::from(chars) * LABEL_CHAR_WIDTH).max(LABEL_MIN_WIDTH)
}

fn tick_left(view_position: f64, width: f32) -> f32 {
    (f32_from_logical(view_position) - width / 2.0).clamp(0.0, SLIDER_WIDTH - width)
}

fn tick_class(kind: AxisTickKind) -> understory_style::ClassId {
    match kind {
        AxisTickKind::Major => CLASS_SLIDER_MAJOR_TICK,
        AxisTickKind::Medium | AxisTickKind::Minor => CLASS_SLIDER_MINOR_TICK,
    }
}

pub(super) fn format_tick_value(value: f64) -> String {
    let mut label = format!("{value:.2}");
    while label.contains('.') && label.ends_with('0') {
        label.pop();
    }
    if label.ends_with('.') {
        label.pop();
    }
    label
}

pub(super) fn sync_slider_tick_labels(ui: &mut Ui, slider: crate::ElementId) {
    let min = read_local_or(ui, slider, ui.properties().minimum, 0.0);
    let max = read_local_or(ui, slider, ui.properties().maximum, 1.0);
    let text_prop = ui.properties().text;
    let ruler = slider_ruler(min, max);
    for (index, mark) in ruler
        .marks()
        .iter()
        .copied()
        .filter(|mark| ruler_mark_in_view(&ruler, *mark))
        .enumerate()
    {
        if let Some(tick) = ui
            .template_instance(slider)
            .and_then(|instance| instance.keyed_slot(SLOT_SLIDER_TICK, tick_key(index)))
        {
            ui.remove_class(tick, CLASS_SLIDER_MAJOR_TICK);
            ui.remove_class(tick, CLASS_SLIDER_MINOR_TICK);
            ui.add_class(tick, tick_class(mark.kind));
            ui.set_layout_placement(tick, tick_placement(mark));
            ui.set_layout_size(tick, tick_size(mark));
        }
        let Some(label) = ui
            .template_instance(slider)
            .and_then(|instance| instance.keyed_slot(SLOT_SLIDER_TICK_LABEL, tick_key(index)))
        else {
            continue;
        };
        ui.remove_class(label, CLASS_SLIDER_MAJOR_TICK);
        ui.remove_class(label, CLASS_SLIDER_MINOR_TICK);
        ui.add_class(label, tick_class(mark.kind));
        let content = format_tick_value(mark.value);
        ui.set_layout_placement(label, tick_label_placement(mark, &content));
        ui.set_layout_size(label, tick_label_size(&content));
        ui.set_local(label, text_prop, content);
    }
}

fn tick_key(index: usize) -> TemplateItemKey {
    let value = u64::try_from(index).expect("slider tick index should fit u64");
    TemplateItemKey::new(value)
}
