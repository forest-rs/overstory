// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Overstory-side attachment of headless sockets to arranged node content.

use super::*;
use overstory::{ElementRealizeContext, ElementRealizeResult, TemplateSlot, Widget};

#[derive(Clone, Copy, Debug)]
pub(super) struct SocketAttachment {
    pub(super) node: NodeId,
    element: ElementId,
    slot: Option<TemplateSlot>,
}

pub(super) fn bind_socket(
    runtime: &GraphDemoRuntime,
    node: NodeId,
    label: &str,
    element: ElementId,
    slot: Option<TemplateSlot>,
) {
    let graph = runtime.graph.borrow();
    let port = graph
        .doc
        .iter_ports()
        .find(|(_, data)| data.owner == node && data.meta.label == label)
        .expect("node content names one of its semantic ports")
        .0;
    runtime.view.borrow_mut().attachments.insert(
        port,
        SocketAttachment {
            node,
            element,
            slot,
        },
    );
}

/// Owns graph and inspector children, so realization can update the complete
/// view after measuring their arranged content without changing the UI core.
#[derive(Debug)]
pub(super) struct MeasuredGraph {
    runtime: Rc<GraphDemoRuntime>,
}

impl MeasuredGraph {
    pub(super) fn new(runtime: Rc<GraphDemoRuntime>) -> Self {
        Self { runtime }
    }
}

impl Widget for MeasuredGraph {
    fn is_hit_test_visible(&self, _: &overstory::Element) -> bool {
        false
    }

    fn realize_elements(&mut self, ctx: &mut ElementRealizeContext<'_>) -> ElementRealizeResult {
        let Some((host, sidebar)) = self.runtime.surface.get() else {
            return ElementRealizeResult::UNCHANGED;
        };
        let resized = parameter_view::resize_preview(ctx.ui_mut(), &self.runtime);
        let changed = publish_socket_geometry(ctx.ui(), &self.runtime);
        if changed {
            refresh_graph_view(ctx.ui_mut(), host, sidebar, Rc::clone(&self.runtime));
        }
        ElementRealizeResult::from_changed(changed || resized)
    }
}

fn publish_socket_geometry(ui: &Ui, runtime: &GraphDemoRuntime) -> bool {
    let updates = {
        let graph = runtime.graph.borrow();
        let view = runtime.view.borrow();
        let scale = graph.session.viewport().world_units_per_pixel_y();
        view.attachments
            .iter()
            .filter_map(|(port, attachment)| {
                let data = graph.doc.port(*port)?;
                let owner = view.element_for_node(attachment.node)?;
                let node = graph.projection.node_view(attachment.node)?;
                let element = match attachment.slot {
                    Some(slot) => ui.slot(attachment.element, slot)?,
                    None => attachment.element,
                };
                ui.get(element)?;
                let bounds = ui.effective_bounds(element);
                if bounds.width() <= 0. || bounds.height() <= 0. {
                    return None;
                }
                let point = Point::new(
                    if data.direction == PortDirection::Output {
                        node.size.width
                    } else {
                        0.
                    },
                    (bounds.center().y - ui.effective_bounds(owner).y0) * scale,
                );
                let mut port_view = graph
                    .projection
                    .port_view(*port)
                    .cloned()
                    .unwrap_or_else(|| PortView::new(()));
                // Pixel rounding while panning must not feed subpixel noise back
                // into layout or repeatedly reroute an otherwise unchanged socket.
                if let PortAnchor::NodeLocal(previous) = port_view.anchor
                    && (previous - point).hypot() / scale <= 0.5
                {
                    return None;
                }
                port_view.anchor = PortAnchor::NodeLocal(point);
                Some((*port, port_view))
            })
            .collect::<Vec<_>>()
    };
    if updates.is_empty() {
        return false;
    }
    let mut graph = runtime.graph.borrow_mut();
    let (_, receipt) = graph.projection.edit(|edit| {
        for (port, view) in updates {
            edit.set_port_view(port, view);
        }
    });
    graph.invalidation.record(&receipt);
    graph.rebuild_computed();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_attached(ui: &Ui, runtime: &GraphDemoRuntime, host: ElementId) {
        let graph = runtime.graph.borrow();
        let view = runtime.view.borrow();
        assert_eq!(view.attachments.len(), graph.doc.port_count());
        for (port, attachment) in &view.attachments {
            let element = attachment
                .slot
                .map_or(Some(attachment.element), |slot| {
                    ui.slot(attachment.element, slot)
                })
                .unwrap();
            let expected = ui.effective_bounds(element).center().y;
            let anchor = graph.computed.port_anchor(*port).unwrap();
            let actual = ui.effective_bounds(host).y0
                + graph.session.viewport().world_to_view_point(anchor).y;
            assert!(
                (actual - expected).abs() <= 1.,
                "socket {port:?} at {actual}, arranged content at {expected}"
            );
        }
    }

    #[test]
    fn wrapped_content_moves_only_its_sockets_and_preserves_controls_and_focus() {
        let (mut ui, runtime, host, _) = build_study_ui();
        let size = Size::new(1480., 820.);
        assert!(matches!(
            ui.compute_layout(size),
            overstory::LayoutOutcome::Stable { .. }
        ));
        assert_attached(&ui, &runtime, host);
        let (name, sliders) = {
            let parts = runtime.parameter_ui.borrow();
            let parts = parts.as_ref().unwrap();
            (parts.names[1], parts.sliders)
        };
        ui.set_focus(Some(sliders[1]));
        let document = runtime.graph.borrow().doc.revision();
        ui.set_local(name, ui.properties().font_size, 18.);
        ui.set_local(
            name,
            ui.properties().text,
            "Minimum clearance between instruments".into(),
        );
        assert!(matches!(
            ui.compute_layout(size),
            overstory::LayoutOutcome::Stable { .. }
        ));
        assert_attached(&ui, &runtime, host);
        assert_eq!(runtime.graph.borrow().doc.revision(), document);
        assert_eq!(
            runtime.parameter_ui.borrow().as_ref().unwrap().sliders,
            sliders
        );
        assert_eq!(ui.focused(), Some(sliders[1]));
        assert_eq!(
            runtime.view.borrow().work,
            ViewWork {
                ports: 2,
                edges: 2,
                ..ViewWork::default()
            }
        );
        let revision = runtime.graph.borrow().projection.revision();
        assert_eq!(
            ui.compute_layout(size),
            overstory::LayoutOutcome::Stable { passes: 0 }
        );
        assert_eq!(runtime.graph.borrow().projection.revision(), revision);
    }

    #[test]
    fn replacing_a_control_template_resolves_the_new_track_slot() {
        #[derive(Debug)]
        struct LowerTrack;
        impl overstory::Template for LowerTrack {
            fn instantiate(&self, cx: &mut overstory::TemplateContext<'_>) {
                let track = cx.surface_slot(overstory::SLOT_SLIDER_TRACK);
                cx.set_layout(
                    track,
                    Layout::new().absolute_at(0., 20.).size(
                        overstory::LayoutSize::new()
                            .width(overstory::LayoutDimension::Percent(1.))
                            .height(overstory::LayoutDimension::Logical(4.)),
                    ),
                );
                let thumb = cx.surface_slot(overstory::SLOT_SLIDER_THUMB);
                cx.set_layout(
                    thumb,
                    Layout::new().absolute_at(0., 15.).logical_size(14., 14.),
                );
            }
        }
        let (mut ui, runtime, host, _) = build_study_ui();
        let size = Size::new(1480., 820.);
        ui.compute_layout(size);
        let slider = runtime.parameter_ui.borrow().as_ref().unwrap().sliders[1];
        let old_track = ui.slot(slider, overstory::SLOT_SLIDER_TRACK).unwrap();
        ui.set_focus(Some(slider));
        ui.set_local(
            slider,
            ui.properties().control_template,
            Some(overstory::ControlTemplate::new(LowerTrack)),
        );
        assert!(matches!(
            ui.compute_layout(size),
            overstory::LayoutOutcome::Stable { .. }
        ));
        assert_ne!(
            ui.slot(slider, overstory::SLOT_SLIDER_TRACK).unwrap(),
            old_track
        );
        assert_attached(&ui, &runtime, host);
        assert_eq!(ui.focused(), Some(slider));
        assert_eq!(
            runtime.view.borrow().work,
            ViewWork {
                ports: 1,
                edges: 1,
                ..ViewWork::default()
            }
        );
    }

    #[test]
    fn density_width_and_zoom_changes_remeasure_without_layout_feedback() {
        let (mut ui, runtime, host, sidebar) = build_study_ui();
        let size = Size::new(1480., 820.);
        ui.compute_layout(size);
        for (width, factor) in [(440., 1.), (360., 0.85), (400., 1.15)] {
            parameter_view::toggle_roomy_controls(&mut ui, &runtime);
            {
                let mut graph = runtime.graph.borrow_mut();
                let node = graph.parameter_node;
                let mut view = graph.projection.node_view(node).unwrap().clone();
                view.size.width = width;
                let (_, receipt) = graph.projection.edit(|edit| edit.set_node_view(node, view));
                graph.invalidation.record(&receipt);
                graph.session.update_viewport(|view| {
                    view.zoom_about_view_point(Point::new(300., 300.), factor);
                });
                graph.rebuild_computed();
            }
            refresh_graph_view(&mut ui, host, sidebar, Rc::clone(&runtime));
            assert!(matches!(
                ui.compute_layout(size),
                overstory::LayoutOutcome::Stable { .. }
            ));
            assert_attached(&ui, &runtime, host);
            let revision = runtime.graph.borrow().projection.revision();
            assert_eq!(
                ui.compute_layout(size),
                overstory::LayoutOutcome::Stable { passes: 0 }
            );
            assert_eq!(runtime.graph.borrow().projection.revision(), revision);
        }
    }
}
