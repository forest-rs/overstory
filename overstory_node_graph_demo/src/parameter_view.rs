// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained controls and an application-owned inline preview.

use super::*;
use overstory::{ControlTemplateSpecExt, Slider};

#[derive(Debug)]
pub(super) struct ParameterUi {
    tiles: [ElementId; 12],
    case_outline: ElementId,
    labels: [ElementId; 3],
    pub(super) names: [ElementId; 3],
    status: ElementId,
    pub(super) sliders: [ElementId; 3],
    size: Size,
}

impl ParameterUi {
    pub(super) fn owns_control(&self, source: ElementId) -> bool {
        self.sliders.contains(&source) || self.names.contains(&source)
    }
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

#[derive(Debug)]
struct ParameterSliderTemplate {
    width: f64,
    color: Color,
}
impl overstory::Template for ParameterSliderTemplate {
    fn instantiate(&self, cx: &mut overstory::TemplateContext<'_>) {
        let surface = cx.properties().surface;
        let track = cx.surface_slot(overstory::SLOT_SLIDER_TRACK);
        cx.set_layout(
            track,
            Layout::new()
                .absolute_at(0., 12.)
                .logical_size(self.width, 4.),
        );
        cx.set_local(
            track,
            surface.background,
            Some(Color::from_rgba8(0x3e, 0x4c, 0x55, 0xff).into()),
        );
        let thumb = cx.surface_slot(overstory::SLOT_SLIDER_THUMB);
        cx.set_layout(
            thumb,
            Layout::new().absolute_at(0., 7.).logical_size(14., 14.),
        );
        cx.set_local(thumb, surface.background, Some(self.color.into()));
        let radius = overstory::CornerRadius::circular(7.);
        for property in [
            surface.corner_radii.top_left,
            surface.corner_radii.top_right,
            surface.corner_radii.bottom_left,
            surface.corner_radii.bottom_right,
        ] {
            cx.set_local(thumb, property, radius);
        }
    }
}

pub(super) fn append_parameter_controls(
    ui: &mut Ui,
    parent: ElementId,
    sidebar: ElementId,
    size: Size,
    runtime: Rc<GraphDemoRuntime>,
) {
    let scale = size.width / 380.;
    let margin = 18.;
    let width = size.width - margin * 2.;
    let ink = Color::from_rgba8(0xed, 0xf3, 0xf5, 0xff);
    let muted = Color::from_rgba8(0xa3, 0xb5, 0xbf, 0xff);
    label_at(
        ui,
        parent,
        "FIELD KIT / 12 MODULES",
        (margin, 16.),
        width,
        10.,
        muted,
    );
    label_at(
        ui,
        parent,
        "Instrument layout",
        (margin, 36.),
        width,
        18.,
        ink,
    );
    let values = runtime.graph.borrow().parameters;
    let mut labels = Vec::new();
    let mut names = Vec::new();
    let mut sliders = Vec::new();
    for (index, parameter) in Parameter::ALL.into_iter().enumerate() {
        let center = Parameter::socket_y(index) * scale;
        let name = label_at(
            ui,
            parent,
            parameter.label(),
            (margin, center - 46.),
            width - 74.,
            13.,
            ink,
        );
        names.push(name);
        let label = label_at(
            ui,
            parent,
            "",
            (size.width - margin - 74., center - 46.),
            74.,
            13.,
            parameter.color(),
        );
        ui.set_local(label, ui.properties().text_align, overstory::TextAlign::End);
        ui.set_local(
            label,
            ui.properties().font_weight,
            overstory::FontWeight::MEDIUM,
        );
        ui.set_local(
            label,
            ui.properties().text_wrap_mode,
            overstory::TextWrapMode::Wrap,
        );
        labels.push(label);
        let (min, max) = parameter.range();
        let callback_runtime = Rc::clone(&runtime);
        let slider = ui.append(
            parent,
            Slider::new()
                .range(min, max)
                .step(1.)
                .value(values.value(parameter))
                .template(overstory::ControlTemplate::new(ParameterSliderTemplate {
                    width,
                    color: parameter.color(),
                }))
                .layout(
                    Layout::new()
                        .absolute_at(margin, center - 14.)
                        .logical_size(width, 28.),
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
        ui.events(name).click(move |ui, event| {
            ui.set_focus(Some(slider));
            event.mark_handled();
        });
        sliders.push(slider);
    }
    let caption = Parameter::socket_y(2) * scale + 32.;
    label_at(
        ui,
        parent,
        "CASE INTERIOR / 300 × 240 mm",
        (margin, caption),
        width,
        10.,
        muted,
    );
    let preview_top = caption + 24.;
    let preview_size = Size::new(width, (size.height - preview_top - 54.).max(40.));
    let preview = ui.append(
        parent,
        Panel::new().layout(
            Layout::new()
                .absolute_at(margin, preview_top)
                .logical_size(preview_size.width, preview_size.height),
        ),
    );
    style_surface(
        ui,
        preview,
        Some(Color::from_rgba8(0x16, 0x1e, 0x23, 0xff)),
        Some(Color::from_rgba8(0x39, 0x48, 0x51, 0xff)),
        1.,
        6.,
    );
    let case_outline = append_rect(ui, preview, Rect::ZERO, Color::TRANSPARENT, 0.);
    let tiles = (0..12)
        .map(|_| append_rect(ui, preview, Rect::ZERO, Parameter::Columns.color(), 0.))
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let status = label_at(
        ui,
        parent,
        "",
        (margin, preview_top + preview_size.height + 10.),
        width,
        11.,
        ink,
    );
    label_at(
        ui,
        parent,
        "12 modules · each 44 × 32 mm",
        (margin, size.height - 22.),
        width,
        10.,
        muted,
    );
    runtime.parameter_ui.replace(Some(ParameterUi {
        tiles,
        case_outline,
        labels: labels.try_into().unwrap(),
        names: names.try_into().unwrap(),
        sliders: sliders.try_into().unwrap(),
        status,
        size: preview_size,
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
            match parameter {
                Parameter::Columns => format!("{:.0}", graph.parameters.value(parameter)),
                _ => format!("{:.0} mm", graph.parameters.value(parameter)),
            },
        );
    }
    let Some(values) = graph.effective_parameters() else {
        return;
    };
    let bounds = Rect::from_origin_size(Point::ZERO, parts.size).inflate(-12., -12.);
    let case = values.case_rect(bounds);
    let frame = case.inflate(4., 4.);
    let clearance = values.clearance();
    let fits = clearance.width >= 0. && clearance.height >= 0.;
    let status = if fits {
        format!(
            "Fits · {:.0} × {:.0} mm spare",
            clearance.width, clearance.height
        )
    } else if clearance.width < 0. {
        format!("{:.0} mm wider than the case", -clearance.width)
    } else {
        format!("{:.0} mm taller than the case", -clearance.height)
    };
    ui.set_local(parts.status, text, status);
    ui.set_local(
        parts.status,
        ui.properties().foreground,
        Brush::from(if fits {
            Parameter::Columns.color()
        } else {
            Color::from_rgba8(0xf1, 0xa0, 0x87, 0xff)
        }),
    );
    ui.set_layout(
        parts.case_outline,
        Layout::new()
            .absolute_at(frame.x0, frame.y0)
            .logical_size(frame.width(), frame.height()),
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
            } else {
                Color::from_rgba8(0x58, 0xb3, 0x97, 0xff)
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
    fn parameter_labels_controls_and_sockets_share_the_same_rows() {
        let (mut ui, runtime, host, _) = build_study_ui();
        ui.compute_layout(Size::new(1480., 820.));
        let parts = runtime.parameter_ui.borrow();
        let parts = parts.as_ref().unwrap();
        let graph = runtime.graph.borrow();
        for (index, parameter) in Parameter::ALL.into_iter().enumerate() {
            let name = ui.effective_bounds(parts.names[index]);
            let value = ui.effective_bounds(parts.labels[index]);
            let control = ui.effective_bounds(parts.sliders[index]);
            assert!((name.y0 - value.y0).abs() < 0.1);
            assert!(name.y1 + 6. <= control.y0);
            assert!(name.x1 <= value.x0);
            let track = ui.effective_bounds(
                ui.slot(parts.sliders[index], overstory::SLOT_SLIDER_TRACK)
                    .unwrap(),
            );
            let slug = parameter.label().to_lowercase();
            let port = graph
                .doc
                .iter_ports()
                .find(|(_, p)| p.owner == graph.parameter_node && p.meta.label == slug)
                .unwrap()
                .0;
            let socket = graph
                .session
                .viewport()
                .world_to_view_point(graph.computed.port_anchor(port).unwrap());
            let socket_y = ui.effective_bounds(host).y0 + socket.y;
            assert!(
                // Native layout rounds element bounds to logical pixels.
                (socket_y - track.center().y).abs() <= 1.,
                "socket {socket_y}, track {}",
                track.center().y
            );
        }
        let frame = ui.effective_bounds(parts.case_outline).inflate(-2., -2.);
        for tile in parts.tiles {
            assert!(frame.contains_rect(ui.effective_bounds(tile)));
        }
    }

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
