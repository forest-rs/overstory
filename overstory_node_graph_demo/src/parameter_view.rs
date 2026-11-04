// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained controls and an application-owned inline preview.

use super::*;
use kurbo::Shape;
use overstory::{
    ControlTemplateSpecExt, LayoutDimension, LayoutSize, MotionSpec, Slider, TimingFunction, millis,
};

const PREVIEW_MOTION_MS: u64 = 280;

/// Presentation geometry follows the timeline while layout and packing
/// retain their immediate target values.
#[derive(Debug)]
struct InstrumentTile {
    radius: understory_property::Property<f64>,
    extent: understory_property::Property<Vec2>,
}

impl overstory::Widget for InstrumentTile {
    fn present(
        &self,
        element: &overstory::Element,
        ctx: &overstory::PresentContext<'_>,
        store: &mut understory_presentation::PresentationStore<
            understory_box_tree::NodeId,
            ElementId,
        >,
        node: understory_box_tree::NodeId,
    ) {
        let extent = ctx.get_value(element, self.extent);
        let bounds = Rect::new(0., 0., extent.x.max(0.), extent.y.max(0.));
        let radius = ctx.get_value(element, self.radius).max(0.);
        let mut primitive =
            overstory::PathPrimitive::new(bounds.to_rounded_rect(radius).to_path(0.1));
        if let Some(brush) = ctx.get_value(element, ctx.properties.surface.background) {
            primitive = primitive.fill(overstory::PathFill::new(brush));
        }
        let _ = store.set_path(node, primitive);
    }
}

#[derive(Debug)]
pub(super) struct ParameterUi {
    tiles: [ElementId; 12],
    case_footprint: ElementId,
    labels: [ElementId; 3],
    pub(super) names: [ElementId; 3],
    status: ElementId,
    pub(super) sliders: [ElementId; 3],
    size: Size,
    preview: ElementId,
    caption: ElementId,
    rows: ElementId,
    preview_intent: Cell<Option<(CaseParameters, Size)>>,
    radius: understory_property::Property<f64>,
    extent: understory_property::Property<Vec2>,
}

impl ParameterUi {
    pub(super) fn owns_control(&self, source: ElementId) -> bool {
        self.sliders.contains(&source) || self.names.contains(&source)
    }

    fn plot_bounds(&self) -> Rect {
        // Keep the case's label above the drawing, including when modules
        // extend below or to the right of the case itself.
        Rect::new(12., 32., self.size.width - 12., self.size.height - 12.)
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

fn flow_text(
    ui: &mut Ui,
    parent: ElementId,
    text: &str,
    font_size: f64,
    color: Color,
) -> ElementId {
    ui.append(
        parent,
        Text::new(text)
            .wrap()
            .font_size(font_size)
            .foreground(color)
            .layout(Layout::new().fill_width().flex_shrink(0.)),
    )
}

pub(super) fn toggle_roomy_controls(ui: &mut Ui, runtime: &GraphDemoRuntime) {
    let roomy = !runtime.roomy_controls.get();
    runtime.roomy_controls.set(roomy);
    let parts = runtime.parameter_ui.borrow();
    if let Some(parts) = parts.as_ref() {
        ui.set_layout(parts.rows, Layout::column(if roomy { 32. } else { 20. }));
        for element in parts.names.iter().chain(&parts.labels) {
            ui.set_local(
                *element,
                ui.properties().font_size,
                if roomy { 17. } else { 13. },
            );
        }
    }
}

pub(super) fn resize_preview(ui: &mut Ui, runtime: &GraphDemoRuntime) -> bool {
    let changed = {
        let mut parts = runtime.parameter_ui.borrow_mut();
        let Some(parts) = parts.as_mut() else {
            return false;
        };
        let size = ui.effective_bounds(parts.preview).size();
        if size.width <= 0. || size.height <= 0. || size == parts.size {
            return false;
        }
        parts.size = size;
        ui.set_layout_size(
            parts.caption,
            LayoutSize::logical((size.width - 24.).max(0.), 17.),
        );
        true
    };
    if changed {
        refresh_parameter_preview(ui, runtime);
    }
    changed
}

#[derive(Debug)]
struct ParameterSliderTemplate {
    color: Color,
}
impl overstory::Template for ParameterSliderTemplate {
    fn instantiate(&self, cx: &mut overstory::TemplateContext<'_>) {
        let surface = cx.properties().surface;
        let track = cx.surface_slot(overstory::SLOT_SLIDER_TRACK);
        cx.set_layout(
            track,
            Layout::new().absolute_at(0., 12.).size(
                LayoutSize::new()
                    .width(LayoutDimension::Percent(1.))
                    .height(LayoutDimension::Logical(4.)),
            ),
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
    graph_host: ElementId,
    runtime: Rc<GraphDemoRuntime>,
) {
    let roomy = runtime.roomy_controls.get();
    let font_size = if roomy { 17. } else { 13. };
    ui.set_layout(parent, Layout::column(12.));
    let padding = ui.properties().surface.padding_widths;
    for property in [padding.top, padding.right, padding.bottom, padding.left] {
        ui.set_local(parent, property, 18.);
    }
    let ink = Color::from_rgba8(0xed, 0xf3, 0xf5, 0xff);
    let muted = Color::from_rgba8(0xa3, 0xb5, 0xbf, 0xff);
    let header = ui.append(
        parent,
        Panel::column(8.).layout(Layout::column(8.).fill_width().flex_shrink(0.)),
    );
    flow_text(ui, header, "FIELD KIT / 12 MODULES", 10., muted);
    flow_text(ui, header, "Instrument layout", 18., ink);
    let rows = ui.append(
        parent,
        Panel::column(if roomy { 32. } else { 20. }).layout(
            Layout::column(if roomy { 32. } else { 20. })
                .fill_width()
                .flex_shrink(0.),
        ),
    );
    let values = runtime.graph.borrow().parameters;
    let mut labels = Vec::new();
    let mut names = Vec::new();
    let mut sliders = Vec::new();
    for parameter in Parameter::ALL {
        let row = ui.append(
            rows,
            Panel::column(14.).layout(Layout::column(14.).fill_width().flex_shrink(0.)),
        );
        let heading = ui.append(
            row,
            Panel::row(8.).layout(Layout::row(8.).fill_width().flex_shrink(0.)),
        );
        let name = ui.append(
            heading,
            Text::new(parameter.label())
                .wrap()
                .font_size(font_size)
                .foreground(ink)
                .layout(
                    Layout::new()
                        .flex_basis(LayoutDimension::Logical(0.))
                        .flex_grow(1.)
                        .min_logical_size(0., 0.),
                ),
        );
        names.push(name);
        let label = ui.append(
            heading,
            Text::new("")
                .font_size(font_size)
                .foreground(parameter.color())
                .layout(
                    Layout::new()
                        .size(LayoutSize::new().width(LayoutDimension::Logical(74.)))
                        .flex_shrink(0.),
                ),
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
            row,
            Slider::new()
                .range(min, max)
                .step(1.)
                .value(values.value(parameter))
                .template(overstory::ControlTemplate::new(ParameterSliderTemplate {
                    color: parameter.color(),
                }))
                .layout(
                    Layout::new()
                        .fill_width()
                        .size(LayoutSize::new().height(LayoutDimension::Logical(28.)))
                        .flex_shrink(0.),
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
                        refresh_graph_view(ui, graph_host, sidebar, Rc::clone(&callback_runtime));
                    }
                }),
        );
        ui.set_tooltip(slider, Tooltip::new(parameter.label()));
        ui.events(name).click(move |ui, event| {
            ui.set_focus(Some(slider));
            event.mark_handled();
        });
        let node = runtime.graph.borrow().parameter_node;
        measured::bind_socket(
            &runtime,
            node,
            &parameter.label().to_lowercase(),
            slider,
            Some(overstory::SLOT_SLIDER_TRACK),
        );
        sliders.push(slider);
    }
    let preview = ui.append(
        parent,
        Panel::new().layout(
            Layout::new()
                .fill_width()
                .flex_grow(1.)
                .min_logical_size(0., 96.),
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
    // Modules are siblings of the footprint, so the case never clips an
    // instrument that extends beyond its usable interior.
    let case_footprint = append_rect(
        ui,
        preview,
        Rect::ZERO,
        Color::from_rgba8(0x2a, 0x39, 0x40, 0xff),
        0.,
    );
    let caption = label_at(
        ui,
        preview,
        "CASE INTERIOR · 300 × 240 mm",
        (12., 10.),
        1.,
        10.,
        Color::from_rgba8(0xbc, 0xcb, 0xd3, 0xff),
    );
    let radius = runtime.instrument_radius.get().unwrap_or_else(|| {
        let property = ui.registry_mut().register(
            "FieldStation.InstrumentRadius",
            understory_property::PropertyMetadataBuilder::new(0_f64)
                .affects_channels(overstory::channels::GEOMETRY.into_set())
                .build(),
        );
        ui.register_animation_target_property(property);
        runtime.instrument_radius.set(Some(property));
        property
    });
    let extent = runtime.instrument_extent.get().unwrap_or_else(|| {
        let property = ui.registry_mut().register(
            "FieldStation.InstrumentExtent",
            understory_property::PropertyMetadataBuilder::new(Vec2::ZERO)
                .affects_channels(overstory::channels::GEOMETRY.into_set())
                .build(),
        );
        ui.register_animation_target_property(property);
        runtime.instrument_extent.set(Some(property));
        property
    });
    let tiles = (0..12)
        .map(|_| ui.append_widget(preview, Box::new(InstrumentTile { radius, extent })))
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let status = flow_text(ui, parent, "", 11., ink);
    flow_text(ui, parent, "12 modules · each 44 × 32 mm", 10., muted);
    runtime.parameter_ui.replace(Some(ParameterUi {
        tiles,
        case_footprint,
        labels: labels.try_into().unwrap(),
        names: names.try_into().unwrap(),
        sliders: sliders.try_into().unwrap(),
        status,
        size: Size::ZERO,
        preview,
        caption,
        rows,
        preview_intent: Cell::new(None),
        radius,
        extent,
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
    if parts.size.width <= 0. || parts.size.height <= 0. {
        return;
    }
    // Initial arrangement and viewport reflow establish geometry directly.
    // Only changes to the packing plan animate between established readings.
    let previous = parts.preview_intent.replace(Some((values, parts.size)));
    let animate = previous.is_some_and(|(before, size)| before != values && size == parts.size);
    let bounds = parts.plot_bounds();
    let case = values.case_rect(bounds);
    ui.set_layout(
        parts.case_footprint,
        Layout::new()
            .absolute_at(case.x0, case.y0)
            .logical_size(case.width(), case.height()),
    );
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
    let update_instruments = |ui: &mut Ui| {
        for (index, (rect, radius)) in values.preview(bounds).into_iter().enumerate() {
            let element = parts.tiles[index];
            ui.set_local(element, parts.radius, radius);
            ui.set_local(
                element,
                parts.extent,
                Vec2::new(rect.width(), rect.height()),
            );
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
    };
    if animate {
        ui.with_motion(
            MotionSpec::new(millis(PREVIEW_MOTION_MS))
                .with_easing(TimingFunction::cubic_bezier(0.215, 0.61, 0.355, 1.))
                .with_color_space(peniko::color::ColorSpaceTag::Oklab),
            update_instruments,
        );
    } else if previous != Some((values, parts.size)) {
        update_instruments(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use understory_property::DependencyObjectExt;

    fn presented_origin(ui: &Ui, element: ElementId) -> Point {
        let row = ui
            .inspection_snapshot()
            .composition
            .layers
            .into_iter()
            .find(|row| row.element == Some(element))
            .unwrap();
        ui.composition_layers()
            .world_transform_at(row.id.index)
            .transform_point(Point::ZERO)
            .unwrap()
    }

    fn change_plan(
        ui: &mut Ui,
        runtime: &Rc<GraphDemoRuntime>,
        host: ElementId,
        sidebar: ElementId,
        columns: f64,
        spacing: f64,
    ) {
        {
            let mut graph = runtime.graph.borrow_mut();
            graph.parameters.set(Parameter::Columns, columns);
            graph.parameters.set(Parameter::Spacing, spacing);
            graph.rebuild_computed();
        }
        refresh_graph_view(ui, host, sidebar, Rc::clone(runtime));
    }

    #[test]
    fn instrument_motion_retargets_from_the_presented_position_without_replacing_controls() {
        let (mut ui, runtime, host, sidebar) = build_study_ui();
        let size = Size::new(1480., 820.);
        ui.compute_layout(size);
        assert!(!ui.has_active_presentation_motion());
        let (tile, sliders) = {
            let parts = runtime.parameter_ui.borrow();
            let parts = parts.as_ref().unwrap();
            (parts.tiles[3], parts.sliders)
        };
        let original = presented_origin(&ui, tile);
        let geometry = runtime.graph.borrow().computed.revision();
        ui.set_now(0);
        change_plan(&mut ui, &runtime, host, sidebar, 3., 12.);
        ui.compute_layout(size);
        assert!((presented_origin(&ui, tile) - original).hypot() < 0.01);
        let target = ui.effective_bounds(tile).origin();
        assert!((target - original).hypot() > 20.);
        assert!(ui.has_active_presentation_motion());
        ui.set_now(millis(PREVIEW_MOTION_MS / 2));
        ui.compute_layout(size);
        let middle = presented_origin(&ui, tile);
        assert!((middle - original).hypot() > 1.);
        assert!((middle - target).hypot() > 1.);
        refresh_graph_view(&mut ui, host, sidebar, Rc::clone(&runtime));
        ui.compute_layout(size);
        assert!((presented_origin(&ui, tile) - middle).hypot() < 0.01);
        change_plan(&mut ui, &runtime, host, sidebar, 2., 12.);
        ui.compute_layout(size);
        assert!((presented_origin(&ui, tile) - middle).hypot() < 0.01);
        ui.set_now(millis(PREVIEW_MOTION_MS * 2));
        ui.compute_layout(size);
        assert!((presented_origin(&ui, tile) - ui.effective_bounds(tile).origin()).hypot() < 0.01);
        assert!(!ui.has_active_presentation_motion());
        assert_eq!(runtime.graph.borrow().computed.revision(), geometry);
        assert_eq!(
            runtime.parameter_ui.borrow().as_ref().unwrap().sliders,
            sliders
        );
        assert_eq!(runtime.view.borrow().work, ViewWork::default());
    }

    #[test]
    fn rescaling_interpolates_both_dimensions_and_retargets_without_layout_work() {
        let (mut ui, runtime, host, sidebar) = build_study_ui();
        let size = Size::new(1480., 820.);
        ui.compute_layout(size);
        let (tile, extent) = {
            let parts = runtime.parameter_ui.borrow();
            let parts = parts.as_ref().unwrap();
            (parts.tiles[0], parts.extent)
        };
        let initial = ui.resolved_value(tile, extent).unwrap();
        ui.set_now(0);
        change_plan(&mut ui, &runtime, host, sidebar, 6., 24.);
        ui.compute_layout(size);
        let target = *ui.get(tile).unwrap().get_local_value(extent).unwrap();
        assert!(target.x < initial.x && target.y < initial.y);
        assert_eq!(ui.resolved_value(tile, extent).unwrap(), initial);
        let revision = runtime.graph.borrow().computed.revision();
        ui.reset_frame_stats();
        ui.set_now(millis(PREVIEW_MOTION_MS / 2));
        ui.compute_layout(size);
        let middle = ui.resolved_value(tile, extent).unwrap();
        assert!(target.x < middle.x && middle.x < initial.x);
        assert!(target.y < middle.y && middle.y < initial.y);
        assert!((middle.x / middle.y - initial.x / initial.y).abs() < 1e-9);
        assert_eq!(ui.frame_stats().layout_passes, 0);
        refresh_parameter_preview(&mut ui, &runtime);
        assert_eq!(ui.resolved_value(tile, extent).unwrap(), middle);
        change_plan(&mut ui, &runtime, host, sidebar, 4., 12.);
        ui.compute_layout(size);
        assert_eq!(ui.resolved_value(tile, extent).unwrap(), middle);
        ui.set_now(millis(PREVIEW_MOTION_MS * 2));
        ui.compute_layout(size);
        assert_eq!(ui.resolved_value(tile, extent).unwrap(), initial);
        assert!(!ui.has_active_presentation_motion());
        assert_eq!(runtime.graph.borrow().computed.revision(), revision);
        assert_eq!(runtime.view.borrow().work, ViewWork::default());
    }

    #[test]
    fn radius_motion_samples_and_retargets_without_layout_or_graph_work() {
        let (mut ui, runtime, host, sidebar) = build_study_ui();
        let size = Size::new(1480., 820.);
        ui.compute_layout(size);
        let (tile, radius) = {
            let parts = runtime.parameter_ui.borrow();
            let parts = parts.as_ref().unwrap();
            (parts.tiles[0], parts.radius)
        };
        let initial = ui.resolved_value(tile, radius).unwrap();
        let geometry = runtime.graph.borrow().computed.revision();
        ui.set_now(0);
        runtime
            .graph
            .borrow_mut()
            .parameters
            .set(Parameter::Radius, 0.);
        change_plan(&mut ui, &runtime, host, sidebar, 4., 12.);
        ui.compute_layout(size);
        assert_eq!(ui.resolved_value(tile, radius).unwrap(), initial);
        ui.reset_frame_stats();
        ui.set_now(millis(PREVIEW_MOTION_MS / 2));
        ui.compute_layout(size);
        let middle = ui.resolved_value(tile, radius).unwrap();
        assert!(middle > 0. && middle < initial);
        assert_eq!(ui.frame_stats().layout_passes, 0);
        runtime
            .graph
            .borrow_mut()
            .parameters
            .set(Parameter::Radius, 16.);
        change_plan(&mut ui, &runtime, host, sidebar, 4., 12.);
        ui.compute_layout(size);
        assert_eq!(ui.resolved_value(tile, radius).unwrap(), middle);
        ui.set_now(millis(PREVIEW_MOTION_MS * 2));
        ui.compute_layout(size);
        assert!(ui.resolved_value(tile, radius).unwrap() > initial);
        assert!(!ui.has_active_presentation_motion());
        assert_eq!(runtime.graph.borrow().computed.revision(), geometry);
        assert_eq!(runtime.view.borrow().work, ViewWork::default());
    }

    #[test]
    fn warning_colors_fade_and_reverse_while_clearance_updates_immediately() {
        let (mut ui, runtime, host, sidebar) = build_study_ui();
        let size = Size::new(1480., 820.);
        ui.compute_layout(size);
        let tile = runtime.parameter_ui.borrow().as_ref().unwrap().tiles[5];
        let background = ui.properties().surface.background;
        let green = ui.resolved_value(tile, background).unwrap();
        let coral = Some(Brush::from(Color::from_rgba8(0xe7, 0x99, 0x80, 0xff)));
        ui.set_now(0);
        change_plan(&mut ui, &runtime, host, sidebar, 6., 24.);
        ui.compute_layout(size);
        assert_eq!(ui.resolved_value(tile, background).unwrap(), green);
        assert_eq!(
            runtime
                .graph
                .borrow()
                .effective_parameters()
                .unwrap()
                .clearance()
                .width,
            -84.
        );
        ui.set_now(millis(PREVIEW_MOTION_MS / 2));
        ui.compute_layout(size);
        let middle = ui.resolved_value(tile, background).unwrap();
        assert_ne!(middle, green);
        assert_ne!(middle, coral);
        change_plan(&mut ui, &runtime, host, sidebar, 4., 12.);
        ui.compute_layout(size);
        assert_eq!(ui.resolved_value(tile, background).unwrap(), middle);
        ui.set_now(millis(PREVIEW_MOTION_MS * 2));
        ui.compute_layout(size);
        assert_eq!(ui.resolved_value(tile, background).unwrap(), green);
        change_plan(&mut ui, &runtime, host, sidebar, 6., 24.);
        ui.compute_layout(size);
        ui.set_now(millis(PREVIEW_MOTION_MS * 3));
        ui.compute_layout(size);
        assert_eq!(ui.resolved_value(tile, background).unwrap(), coral);
    }

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
        let case = ui.effective_bounds(parts.case_footprint).inflate(1., 1.);
        for tile in parts.tiles {
            assert!(case.contains_rect(ui.effective_bounds(tile)));
        }
    }

    #[test]
    fn case_footprint_tracks_the_plan_without_hiding_overflow_on_either_axis() {
        let (mut ui, runtime, _, _) = build_study_ui();
        let viewport = Size::new(1480., 820.);
        ui.compute_layout(viewport);
        let (footprint, tiles) = {
            let parts = runtime.parameter_ui.borrow();
            let parts = parts.as_ref().unwrap();
            (parts.case_footprint, parts.tiles)
        };
        for (columns, spacing, expected_overflow) in [(6., 24., 4), (2., 24., 4), (4., 12., 0)] {
            {
                let mut graph = runtime.graph.borrow_mut();
                graph.parameters.set(Parameter::Columns, columns);
                graph.parameters.set(Parameter::Spacing, spacing);
                graph.rebuild_computed();
            }
            refresh_parameter_preview(&mut ui, &runtime);
            ui.compute_layout(viewport);
            ui.set_now(ui.now() + millis(PREVIEW_MOTION_MS));
            ui.compute_layout(viewport);
            let case = ui.effective_bounds(footprint);
            let preview = ui.get(footprint).unwrap().parent().unwrap();
            let visible = ui.effective_bounds(preview).inflate(1., 1.);
            let mut overflowing = 0;
            for tile in tiles {
                let rect = ui.effective_bounds(tile);
                let outside = !case.inflate(1., 1.).contains_rect(rect);
                overflowing += usize::from(outside);
                assert!(visible.contains_rect(rect));
                assert_eq!(ui.get(tile).unwrap().parent(), Some(preview));
                let expected = if outside {
                    Color::from_rgba8(0xe7, 0x99, 0x80, 0xff)
                } else {
                    Color::from_rgba8(0x58, 0xb3, 0x97, 0xff)
                };
                assert_eq!(
                    ui.resolved_value(tile, ui.properties().surface.background),
                    Some(Some(expected.into()))
                );
            }
            assert_eq!(overflowing, expected_overflow);
            assert_eq!(
                runtime
                    .parameter_ui
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .case_footprint,
                footprint
            );
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
