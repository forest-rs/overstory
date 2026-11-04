// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained controls and an application-owned inline preview.

use super::*;
use overstory::Slider;

#[derive(Debug)]
pub(super) struct ParameterUi {
    tiles: [ElementId; 12],
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
                    .absolute_at(x, y)
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
        "SURFACE / 01",
        (0., 0.),
        size.width - 28.,
        11.,
        ValueKind::Number.color(),
    );
    label_at(
        ui,
        parent,
        "Surface parameters",
        (0., 26. * scale),
        size.width - 28.,
        16.,
        ink,
    );
    let values = runtime.graph.borrow().parameters;
    let mut labels = Vec::new();
    let mut sliders = Vec::new();
    for (index, parameter) in Parameter::ALL.into_iter().enumerate() {
        let y = (76. + index as f64 * 84.) * scale;
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
                        .absolute_at(0., y + 22.)
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
                    if changed {
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
        "INLINE PREVIEW / 12 SURFACES",
        (0., 340. * scale),
        size.width - 28.,
        10.,
        ValueKind::Number.color(),
    );
    let preview = ui.append(
        parent,
        Panel::new().layout(
            Layout::new()
                .absolute_at(0., 370. * scale)
                .logical_size(size.width - 28., 110. * scale),
        ),
    );
    label_at(
        ui,
        parent,
        "Values change paint, not topology.",
        (0., 490. * scale),
        size.width - 28.,
        10.,
        ink,
    );
    let tiles = (0..12)
        .map(|_| append_rect(ui, preview, Rect::ZERO, ValueKind::Number.color(), 0.))
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    runtime.parameter_ui.replace(Some(ParameterUi {
        tiles,
        labels: labels.try_into().unwrap(),
        sliders: sliders.try_into().unwrap(),
        size: Size::new(size.width - 28., 110. * scale),
    }));
    refresh_parameter_preview(ui, &runtime);
}

fn refresh_parameter_preview(ui: &mut Ui, runtime: &GraphDemoRuntime) {
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
    for (index, (rect, radius)) in graph
        .parameters
        .preview(Rect::from_origin_size(Point::ZERO, parts.size))
        .into_iter()
        .enumerate()
    {
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
            Some(if index % 3 == 0 {
                ValueKind::Number.color()
            } else {
                Color::from_rgba8(0x39, 0x70, 0x68, 0xff)
            }),
            None,
            0.,
            radius,
        );
        ui.set_tooltip(element, Tooltip::new(format!("Surface {}", index + 1)));
    }
}
