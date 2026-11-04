// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Host-owned identity mapping from graph geometry to retained elements.

use super::*;

#[derive(Debug)]
struct Layer;

impl overstory::Widget for Layer {
    fn is_hit_test_visible(&self, _: &overstory::Element) -> bool {
        false
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
        let routes = runtime
            .view
            .borrow()
            .edges
            .iter()
            .map(|(e, _, ids)| (*e, ids.clone()))
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
}

#[derive(Debug, Default)]
pub(super) struct GraphView {
    layers: Option<Layers>,
    pub(super) visible_world: Option<Rect>,
    nodes: Vec<(NodeId, ElementId, Size)>,
    ports: Vec<(PortId, ElementId)>,
    edges: Vec<(EdgeId, Vec<Point>, Vec<ElementId>)>,
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
            let mut layer = || {
                let id = ui.append_widget(parent, Box::new(Layer));
                ui.set_layout(
                    id,
                    Layout::new()
                        .absolute_at(0., 0.)
                        .logical_size(GRAPH_VIEW.width(), GRAPH_VIEW.height()),
                );
                id
            };
            Layers {
                grid: layer(),
                edges: layer(),
                preview: layer(),
                nodes: layer(),
                ports: layer(),
            }
        })
    }

    pub(super) fn node_element(&self, node: NodeId, size: Size) -> Option<ElementId> {
        self.nodes
            .iter()
            .find(|(n, _, s)| *n == node && *s == size)
            .map(|(_, id, _)| *id)
    }
    pub(super) fn remove_node(&mut self, node: NodeId) -> Option<ElementId> {
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

    pub(super) fn sync_edges(&mut self, ui: &mut Ui, parent: ElementId, graph: &DemoGraph) {
        self.edges.retain(|(edge, _, elements)| {
            if graph.computed.visible_edges().contains(edge) {
                return true;
            }
            for element in elements {
                ui.remove(*element);
            }
            false
        });
        // The current computed API exposes snapshots. Retain identities while
        // comparing them; a future derived-change stream can remove this scan.
        for edge in graph.computed.visible_edges() {
            let route = graph
                .computed
                .edge_route(*edge)
                .expect("visible edge is routed");
            let points: Vec<_> = route
                .points
                .iter()
                .map(|p| graph.session.viewport().world_to_view_point(*p))
                .collect();
            let index = self
                .edges
                .iter()
                .position(|(e, _, _)| e == edge)
                .unwrap_or_else(|| {
                    self.edges.push((*edge, Vec::new(), Vec::new()));
                    self.edges.len() - 1
                });
            let (_, old, elements) = &mut self.edges[index];
            if *old == points {
                continue;
            }
            while elements.len() > points.len().saturating_sub(1) {
                ui.remove(elements.pop().unwrap());
            }
            for (index, segment) in points.windows(2).enumerate() {
                let (start, end) = (segment[0], segment[1]);
                let half = EDGE_THICKNESS * 0.5;
                let rect = if (start.x - end.x).abs() >= (start.y - end.y).abs() {
                    Rect::new(
                        start.x.min(end.x),
                        start.y - half,
                        start.x.max(end.x),
                        start.y + half,
                    )
                } else {
                    Rect::new(
                        start.x - half,
                        start.y.min(end.y),
                        start.x + half,
                        start.y.max(end.y),
                    )
                };
                if let Some(id) = elements.get(index) {
                    ui.set_layout(
                        *id,
                        Layout::new()
                            .absolute_at(rect.x0, rect.y0)
                            .logical_size(rect.width().max(1.), rect.height().max(1.)),
                    );
                } else {
                    elements.push(append_rect(
                        ui,
                        parent,
                        rect,
                        Color::from_rgba8(0x62, 0xd8, 0xff, 0xff),
                        half,
                    ));
                }
            }
            *old = points;
        }
    }
}
