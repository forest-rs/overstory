// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained controls and an application-owned inline preview.

use super::*;
use overstory::Slider;

#[derive(Debug)]
pub(super) struct ParameterUi {
    tiles: [ElementId; 12],
    case_outline: ElementId,
    labels: [ElementId; 3],
    pub(super) sliders: [ElementId; 3],
    size: Size,
}

fn label_at(
    ui: &mut Ui,
    parent: ElementId,
    text: impl Into<String>,
    (x, y): (f64, f64),
    width: f64,
    font_size: f64,
    color: Color,
) -> ElementId {
    ui.append(
        parent,
        Text::new(text)
            .font_size(font_size)
            .foreground(color)
            .layout(
                Layout::new()
                    .absolute_at(x + 14., y + 14.)
                    .logical_size(width, font_size + 7.),
            ),
    )
}

pub(super) fn append_parameter_controls(
    ui: &mut Ui,
    parent: ElementId,
    sidebar: ElementId,
    size: Size,
    runtime: Rc<GraphDemoRuntime>,
) {
    let scale = size.width / 380.;
    let ink = Color::from_rgba8(0xed, 0xff, 0xf9, 0xff);
    label_at(
        ui,
        parent,
        "PACKING / FIELD 01",
        (0., 0.),
        size.width - 28.,
        11.,
        ValueKind::Count.color(),
    );
    label_at(
        ui,
        parent,
        "Instrument layout",
        (0., 26. * scale),
        size.width - 28.,
        16.,
        ink,
    );
    let values = runtime.graph.borrow().parameters;
    let mut labels = Vec::new();
    let mut sliders = Vec::new();
    for (index, parameter) in Parameter::ALL.into_iter().enumerate() {
        let y = (76. + index as f64 * 100.) * scale;
        labels.push(label_at(
            ui,
            parent,
            format!("{}   {:.0}", parameter.label(), values.value(parameter)),
            (0., y),
            size.width - 28.,
            12.,
            ink,
        ));
        let (min, max) = parameter.range();
        let callback_runtime = Rc::clone(&runtime);
        let slider = ui.append(
            parent,
            Slider::new()
                .range(min, max)
                .step(1.)
                .value(values.value(parameter))
                .layout(
                    Layout::new()
                        .absolute_at(14., y + 36.)
                        .logical_size(size.width - 36., 20.),
                )
                .on_value_changed(move |ui, event| {
                    let changed = {
                        let mut graph = callback_runtime.graph.borrow_mut();
                        let changed = graph.parameters.set(parameter, event.value);
                        if changed {
                            graph.rebuild_computed();
                        }
                        changed
                    };
                    let accepted = callback_runtime.graph.borrow().parameters.value(parameter);
                    overstory::set_slider_value(ui, event.element, accepted);
                    if changed {
                        callback_runtime
                            .view
                            .borrow()
                            .sync_readings(ui, &callback_runtime.graph.borrow());
                        refresh_parameter_preview(ui, &callback_runtime);
                        refresh_inspector(ui, sidebar, &callback_runtime.graph.borrow());
                    }
                }),
        );
        ui.set_tooltip(slider, Tooltip::new(parameter.label()));
        sliders.push(slider);
    }
    label_at(
        ui,
        parent,
        "CASE PLAN / 300 × 240 mm",
        (0., 380. * scale),
        size.width - 28.,
        10.,
        ValueKind::Count.color(),
    );
    let preview = ui.append(
        parent,
        Panel::new().layout(
            Layout::new()
                .absolute_at(14., 416. * scale + 14.)
                .logical_size(size.width - 28., 210. * scale),
        ),
    );
    label_at(
        ui,
        parent,
        "12 instruments · fixed 44 × 32 mm",
        (0., 640. * scale),
        size.width - 28.,
        10.,
        ink,
    );
    let case_outline = append_rect(ui, preview, Rect::ZERO, Color::TRANSPARENT, 0.);
    let tiles = (0..12)
        .map(|_| append_rect(ui, preview, Rect::ZERO, ValueKind::Count.color(), 0.))
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    runtime.parameter_ui.replace(Some(ParameterUi {
        tiles,
        case_outline,
        labels: labels.try_into().unwrap(),
        sliders: sliders.try_into().unwrap(),
        size: Size::new(size.width - 28., 210. * scale),
    }));
    refresh_parameter_preview(ui, &runtime);
}

pub(super) fn refresh_parameter_preview(ui: &mut Ui, runtime: &GraphDemoRuntime) {
    let parts = runtime.parameter_ui.borrow();
    let Some(parts) = parts.as_ref() else {
        return;
    };
    let graph = runtime.graph.borrow();
    let text = ui.properties().text;
    for (label, parameter) in parts.labels.iter().zip(Parameter::ALL) {
        ui.set_local(
            *label,
            text,
            format!(
                "{}   {:.0}",
                parameter.label(),
                graph.parameters.value(parameter)
            ),
        );
    }
    let Some(values) = graph.effective_parameters() else {
        return;
    };
    let bounds = Rect::from_origin_size(Point::ZERO, parts.size);
    let case = values.case_rect(bounds);
    ui.set_layout(
        parts.case_outline,
        Layout::new()
            .absolute_at(case.x0, case.y0)
            .logical_size(case.width(), case.height()),
    );
    style_surface(
        ui,
        parts.case_outline,
        Some(Color::TRANSPARENT),
        Some(Color::from_rgba8(0xe2, 0xd2, 0xaf, 0xff)),
        1.,
        2.,
    );
    for (index, (rect, radius)) in values.preview(bounds).into_iter().enumerate() {
        let element = parts.tiles[index];
        ui.set_layout(
            element,
            Layout::new()
                .absolute_at(rect.x0, rect.y0)
                .logical_size(rect.width(), rect.height()),
        );
        style_surface(
            ui,
            element,
            Some(if !case.inflate(0.001, 0.001).contains_rect(rect) {
                Color::from_rgba8(0xe7, 0x99, 0x80, 0xff)
            } else if index % 3 == 0 {
                ValueKind::Count.color()
            } else {
                Color::from_rgba8(0x39, 0x70, 0x68, 0xff)
            }),
            None,
            0.,
            radius,
        );
        ui.set_tooltip(
            element,
            Tooltip::new(format!("Instrument module {}", index + 1)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnecting_a_length_source_updates_the_retained_case_plan() {
        let (mut ui, runtime, host, sidebar) = build_study_ui();
        ui.compute_layout(Size::new(1480., 820.));
        let tile = runtime.parameter_ui.borrow().as_ref().unwrap().tiles[1];
        let before = ui.effective_bounds(tile);
        {
            let mut graph = runtime.graph.borrow_mut();
            let from = graph
                .doc
                .iter_ports()
                .find(|(_, p)| p.owner == graph.parameter_node && p.meta.label == "radius")
                .unwrap()
                .0;
            let to = graph
                .doc
                .iter_ports()
                .find(|(_, p)| p.owner != graph.parameter_node && p.meta.label == "spacing")
                .unwrap()
                .0;
            assert!(graph.rebind(from, to));
            graph.rebuild_computed();
        }
        refresh_graph_view(&mut ui, host, sidebar, Rc::clone(&runtime));
        ui.compute_layout(Size::new(1480., 820.));
        assert_eq!(
            runtime.parameter_ui.borrow().as_ref().unwrap().tiles[1],
            tile
        );
        assert_ne!(ui.effective_bounds(tile), before);
    }
}
