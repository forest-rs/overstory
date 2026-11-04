// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Host-owned identity mapping from graph geometry to retained elements.

use super::*;

#[derive(Debug)]
struct Layer {
    interactive_children: bool,
}

impl overstory::Widget for Layer {
    fn is_hit_test_visible(&self, _: &overstory::Element) -> bool {
        false
    }
    fn is_hit_test_subtree_visible(&self, _: &overstory::Element) -> bool {
        self.interactive_children
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_geometry_edits_retain_controls_and_unaffected_route_elements() {
        let mut ui = Ui::new();
        let root = ui.root();
        let host = ui.append(root, Viewport::new());
        let sidebar = ui.append(root, Panel::new());
        let runtime = Rc::new(GraphDemoRuntime::new(build_demo_graph()));
        refresh_graph_view(&mut ui, host, sidebar, Rc::clone(&runtime));
        let sliders = runtime.parameter_ui.borrow().as_ref().unwrap().sliders;
        assert!(ui.set_focus(Some(sliders[0])));
        let routes = runtime
            .view
            .borrow()
            .edges
            .iter()
            .map(|(e, _, id)| (*e, *id))
            .collect::<Vec<_>>();
        {
            let mut graph = runtime.graph.borrow_mut();
            let node = graph.selected;
            move_node_by_view_delta(&mut graph, node, Vec2::new(12., 5.));
            graph.rebuild_computed();
        }
        refresh_graph_view(&mut ui, host, sidebar, Rc::clone(&runtime));
        assert_eq!(
            runtime.parameter_ui.borrow().as_ref().unwrap().sliders,
            sliders
        );
        assert!(sliders.iter().all(|id| ui.get(*id).is_some()));
        assert_eq!(ui.focused(), Some(sliders[0]));
        for (edge, before) in routes {
            let view = runtime.view.borrow();
            let after = &view.edges.iter().find(|(e, _, _)| *e == edge).unwrap().2;
            assert_eq!(&before, after);
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Layers {
    pub(super) grid: ElementId,
    pub(super) edges: ElementId,
    pub(super) preview: ElementId,
    pub(super) nodes: ElementId,
    pub(super) ports: ElementId,
    pub(super) ties: ElementId,
}

#[derive(Debug, Default)]
pub(super) struct GraphView {
    pub(super) tie: Option<ElementId>,
    layers: Option<Layers>,
    pub(super) visible_world: Option<Rect>,
    nodes: Vec<(NodeId, ElementId, Size)>,
    ports: Vec<(PortId, ElementId)>,
    edges: Vec<(EdgeId, Vec<Point>, ElementId)>,
    readings: Vec<(NodeId, ElementId)>,
}

impl GraphView {
    pub(super) fn prune_nodes(&mut self, ui: &mut Ui, graph: &DemoGraph) -> bool {
        let mut removed_parameters = false;
        self.nodes.retain(|(node, element, _)| {
            if graph.computed.visible_nodes().contains(node) {
                return true;
            }
            ui.remove(*element);
            removed_parameters |= *node == graph.parameter_node;
            false
        });
        self.readings
            .retain(|(node, _)| graph.computed.visible_nodes().contains(node));
        self.ports.retain(|(port, element)| {
            if graph.computed.port_anchor(*port).is_some() {
                return true;
            }
            ui.remove(*element);
            false
        });
        removed_parameters
    }
    pub(super) fn layers(&mut self, ui: &mut Ui, parent: ElementId) -> Layers {
        *self.layers.get_or_insert_with(|| {
            let mut layer = |interactive_children| {
                let id = ui.append_widget(
                    parent,
                    Box::new(Layer {
                        interactive_children,
                    }),
                );
                ui.set_layout(
                    id,
                    Layout::new()
                        .absolute_at(0., 0.)
                        .logical_size(GRAPH_VIEW.width(), GRAPH_VIEW.height()),
                );
                id
            };
            Layers {
                grid: layer(false),
                edges: layer(false),
                preview: layer(false),
                nodes: layer(true),
                ports: layer(true),
                ties: layer(true),
            }
        })
    }

    pub(super) fn node_element(&self, node: NodeId, size: Size) -> Option<ElementId> {
        self.nodes
            .iter()
            // Moving a transformed rectangle can introduce subpixel rounding
            // in its measured size. It must not replace the pressed subtree.
            .find(|(n, _, s)| {
                *n == node
                    && (s.width - size.width).abs() < 1e-6
                    && (s.height - size.height).abs() < 1e-6
            })
            .map(|(_, id, _)| *id)
    }
    pub(super) fn remove_node(&mut self, node: NodeId) -> Option<ElementId> {
        self.readings.retain(|(n, _)| *n != node);
        let index = self.nodes.iter().position(|(n, _, _)| *n == node)?;
        Some(self.nodes.swap_remove(index).1)
    }
    pub(super) fn insert_node(&mut self, node: NodeId, id: ElementId, size: Size) {
        self.nodes.push((node, id, size));
    }
    pub(super) fn port_element(&self, port: PortId) -> Option<ElementId> {
        self.ports
            .iter()
            .find(|(p, _)| *p == port)
            .map(|(_, id)| *id)
    }
    pub(super) fn insert_port(&mut self, port: PortId, id: ElementId) {
        self.ports.push((port, id));
    }

    pub(super) fn sync_readings(&self, ui: &mut Ui, graph: &DemoGraph) {
        for (node, label) in &self.readings {
            ui.set_local(*label, ui.properties().text, graph.node_summary(*node));
        }
    }
    pub(super) fn insert_reading(&mut self, node: NodeId, label: ElementId) {
        self.readings.retain(|(n, _)| *n != node);
        self.readings.push((node, label));
    }
    pub(super) fn sync_edges(&mut self, ui: &mut Ui, parent: ElementId, graph: &DemoGraph) {
        self.edges.retain(|(edge, _, element)| {
            if graph.computed.visible_edges().contains(edge) {
                return true;
            }
            ui.remove(*element);
            false
        });
        for edge in graph.computed.visible_edges() {
            let points: Vec<_> = graph
                .computed
                .edge_route(*edge)
                .unwrap()
                .points
                .iter()
                .map(|p| graph.session.viewport().world_to_view_point(*p))
                .collect();
            let data = graph.doc.edge(*edge).unwrap();
            let color = graph.doc.port(data.output).unwrap().meta.color();
            let primitive = wire_primitive(&points, color);
            if let Some((_, previous, element)) =
                self.edges.iter_mut().find(|(id, _, _)| id == edge)
            {
                if *previous != points {
                    overstory::set_vector_path(ui, *element, primitive);
                    *previous = points;
                }
            } else {
                let element = ui.append(
                    parent,
                    overstory::VectorPath::from_primitive(primitive).layout(
                        Layout::new()
                            .absolute_at(0., 0.)
                            .logical_size(GRAPH_VIEW.width(), GRAPH_VIEW.height()),
                    ),
                );
                self.edges.push((*edge, points, element));
            }
        }
    }
}

pub(super) fn wire_primitive(points: &[Point], color: Color) -> overstory::PathPrimitive {
    let mut path = kurbo::BezPath::new();
    if let Some(first) = points.first() {
        path.move_to(*first);
        for corner in points.windows(3) {
            let incoming = corner[0] - corner[1];
            let outgoing = corner[2] - corner[1];
            let radius = 8_f64
                .min(incoming.hypot() * 0.5)
                .min(outgoing.hypot() * 0.5);
            if radius <= 0.001 {
                continue;
            }
            path.line_to(corner[1] + incoming * (radius / incoming.hypot()));
            path.quad_to(
                corner[1],
                corner[1] + outgoing * (radius / outgoing.hypot()),
            );
        }
        if let Some(last) = points.last() {
            path.line_to(*last);
        }
    }
    overstory::PathPrimitive::new(path).stroke(overstory::PathStroke::new(color, 2.25))
}
