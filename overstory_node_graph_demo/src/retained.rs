// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Host-owned identity mapping from graph geometry to retained elements.

use super::*;
use std::collections::{HashMap, HashSet};

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
    fn unchanged_routes_and_repeated_refreshes_do_no_geometry_work() {
        let (mut ui, runtime, host, sidebar) = build_study_ui();
        let routes = runtime.view.borrow().edges.clone();
        {
            let mut graph = runtime.graph.borrow_mut();
            let edge = graph.tie.members()[0];
            graph.invalidation.mark_routing_edge(edge);
            graph.rebuild_computed();
            assert_eq!(graph.work.routes, 1);
            assert!(graph.result_changes.is_empty());
        }
        refresh_graph_view(&mut ui, host, sidebar, Rc::clone(&runtime));
        assert_eq!(runtime.view.borrow().work, ViewWork::default());
        assert_eq!(runtime.view.borrow().edges, routes);
        refresh_graph_view(&mut ui, host, sidebar, Rc::clone(&runtime));
        assert_eq!(runtime.view.borrow().work, ViewWork::default());
    }

    #[test]
    fn a_missed_report_resynchronizes_from_the_current_snapshot() {
        let (mut ui, runtime, host, sidebar) = build_study_ui();
        let before = runtime.parameter_ui.borrow().as_ref().unwrap().sliders;
        {
            let mut graph = runtime.graph.borrow_mut();
            let node = graph.selected;
            for _ in 0..2 {
                move_node_by_view_delta(&mut graph, node, Vec2::new(10., 5.));
                graph.rebuild_computed();
            }
        }
        refresh_graph_view(&mut ui, host, sidebar, Rc::clone(&runtime));
        let view = runtime.view.borrow();
        let graph = runtime.graph.borrow();
        assert!(view.work.resync);
        assert_eq!(view.nodes.len(), graph.computed.visible_nodes().len());
        assert_eq!(view.edges.len(), graph.computed.visible_edges().len());
        assert_eq!(view.ports.len(), graph.doc.port_count());
        assert!(before.into_iter().all(|id| ui.get(id).is_none()));
        assert_ne!(
            runtime.parameter_ui.borrow().as_ref().unwrap().sliders,
            before
        );
    }

    #[test]
    fn camera_motion_updates_screen_geometry_without_replacing_controls() {
        let (mut ui, runtime, host, sidebar) = build_study_ui();
        let before = runtime.parameter_ui.borrow().as_ref().unwrap().sliders;
        {
            let mut graph = runtime.graph.borrow_mut();
            graph
                .session
                .update_viewport(|view| view.pan_by_view(Vec2::new(8., 4.)));
            graph.rebuild_computed();
            assert!(graph.result_changes.nodes.is_empty());
            assert!(graph.result_changes.ports.is_empty());
            assert!(graph.result_changes.edges.is_empty());
        }
        refresh_graph_view(&mut ui, host, sidebar, Rc::clone(&runtime));
        let view = runtime.view.borrow();
        let graph = runtime.graph.borrow();
        assert!(!view.work.resync);
        assert_eq!(view.work.nodes, graph.computed.visible_nodes().len());
        assert_eq!(view.work.ports, graph.doc.port_count());
        assert_eq!(view.work.edges, graph.computed.visible_edges().len());
        assert_eq!(
            runtime.parameter_ui.borrow().as_ref().unwrap().sliders,
            before
        );
    }

    #[test]
    fn cascading_removal_retires_only_the_reported_elements() {
        let (mut ui, runtime, host, sidebar) = build_study_ui();
        let before = runtime.parameter_ui.borrow().as_ref().unwrap().sliders;
        let (removed_node, removed_ports, removed_edges) = {
            let graph = runtime.graph.borrow();
            let view = runtime.view.borrow();
            let node = graph.story.check_node();
            let ports = graph.doc.node_ports(node).unwrap();
            let edges = graph.doc.port_edges(ports[0]).unwrap();
            (
                view.element_for_node(node).unwrap(),
                ports.iter().map(|id| view.ports[id]).collect::<Vec<_>>(),
                edges.iter().map(|id| view.edges[id]).collect::<Vec<_>>(),
            )
        };
        {
            let mut graph = runtime.graph.borrow_mut();
            let node = graph.story.check_node();
            let (_, changes) = graph.doc.edit(|edit| edit.remove_node(node));
            graph.invalidation.record(&changes);
            graph.rebuild_computed();
        }
        refresh_graph_view(&mut ui, host, sidebar, Rc::clone(&runtime));
        assert!(ui.get(removed_node).is_none());
        assert!(
            removed_ports
                .into_iter()
                .chain(removed_edges)
                .all(|id| ui.get(id).is_none())
        );
        assert_eq!(
            runtime.parameter_ui.borrow().as_ref().unwrap().sliders,
            before
        );
        assert_eq!(
            runtime.view.borrow().work,
            ViewWork {
                removed: 3,
                ..ViewWork::default()
            }
        );
    }

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
            .map(|(e, id)| (*e, *id))
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
            let after = view.edges.get(&edge).unwrap();
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ViewWork {
    pub(super) nodes: usize,
    pub(super) ports: usize,
    pub(super) edges: usize,
    pub(super) removed: usize,
    pub(super) resync: bool,
}

#[derive(Debug, Default)]
pub(super) struct ViewPlan {
    pub(super) nodes: Vec<NodeId>,
    pub(super) ports: Vec<PortId>,
    pub(super) edges: Vec<EdgeId>,
    pub(super) camera_changed: bool,
    pub(super) preview_changed: bool,
    pub(super) removed_parameters: bool,
}

#[derive(Debug, Default)]
pub(super) struct GraphView {
    pub(super) tie: Option<ElementId>,
    layers: Option<Layers>,
    revision: Option<understory_node_graph::Revision>,
    camera: Option<(Rect, Rect)>,
    visible_nodes: HashSet<NodeId>,
    visible_edges: HashSet<EdgeId>,
    selected: Option<NodeId>,
    nodes: HashMap<NodeId, (ElementId, Size)>,
    ports: HashMap<PortId, ElementId>,
    edges: HashMap<EdgeId, ElementId>,
    readings: HashMap<NodeId, ElementId>,
    pub(super) work: ViewWork,
}

impl GraphView {
    pub(super) fn update_plan(&mut self, ui: &mut Ui, graph: &DemoGraph) -> ViewPlan {
        let changes = &graph.result_changes;
        let new_results = self.revision != Some(graph.computed.revision());
        let resync = self.revision.is_none()
            || (new_results && (changes.reset || self.revision != Some(changes.before)));
        let camera = (
            graph
                .session
                .viewport()
                .world_to_view_rect(Rect::new(0., 0., 1., 1.)),
            graph.session.viewport().view_rect(),
        );
        let camera_changed = self.camera != Some(camera);
        let mut plan = ViewPlan {
            camera_changed,
            ..ViewPlan::default()
        };
        self.work = ViewWork {
            resync,
            ..ViewWork::default()
        };
        let mut nodes = HashSet::new();
        let mut ports = HashSet::new();
        let mut edges = HashSet::new();
        if resync {
            // Missing a report is safe, but requires a snapshot baseline. This
            // path also prevents document-local ids from leaking across reset.
            for (element, _) in self.nodes.drain().map(|(_, value)| value) {
                ui.remove(element);
                self.work.removed += 1;
            }
            for element in self
                .ports
                .drain()
                .map(|(_, id)| id)
                .chain(self.edges.drain().map(|(_, id)| id))
            {
                ui.remove(element);
                self.work.removed += 1;
            }
            self.readings.clear();
            self.selected = None;
            self.visible_nodes = graph.computed.visible_nodes().iter().copied().collect();
            self.visible_edges = graph.computed.visible_edges().iter().copied().collect();
            nodes.extend(&self.visible_nodes);
            edges.extend(&self.visible_edges);
            ports.extend(
                graph
                    .doc
                    .iter_ports()
                    .filter_map(|(id, _)| graph.computed.port_anchor(id).map(|_| id)),
            );
            plan.removed_parameters = true;
            plan.preview_changed = true;
        } else if new_results {
            for id in changes
                .visible_nodes
                .exited
                .iter()
                .chain(&changes.nodes.removed)
            {
                self.visible_nodes.remove(id);
                if let Some(element) = self.remove_node(*id) {
                    ui.remove(element);
                    self.work.removed += 1;
                    plan.removed_parameters |= *id == graph.parameter_node;
                }
            }
            self.visible_nodes.extend(&changes.visible_nodes.entered);
            for id in changes
                .visible_edges
                .exited
                .iter()
                .chain(&changes.edges.removed)
            {
                self.visible_edges.remove(id);
                if let Some(element) = self.edges.remove(id) {
                    ui.remove(element);
                    self.work.removed += 1;
                }
            }
            self.visible_edges.extend(&changes.visible_edges.entered);
            for id in &changes.ports.removed {
                if let Some(element) = self.ports.remove(id) {
                    ui.remove(element);
                    self.work.removed += 1;
                }
            }
            nodes.extend(
                changes
                    .nodes
                    .updated
                    .iter()
                    .chain(&changes.visible_nodes.entered)
                    .copied()
                    .filter(|id| self.visible_nodes.contains(id)),
            );
            edges.extend(
                changes
                    .edges
                    .updated
                    .iter()
                    .chain(&changes.visible_edges.entered)
                    .copied()
                    .filter(|id| self.visible_edges.contains(id)),
            );
            ports.extend(&changes.ports.updated);
            plan.preview_changed = changes.preview_changed;
        }
        if camera_changed {
            // Result geometry stays in world space. A changed host transform
            // needs new screen coordinates for every materialized element.
            nodes.extend(&self.visible_nodes);
            edges.extend(&self.visible_edges);
            ports.extend(self.ports.keys());
            plan.preview_changed = true;
        }
        plan.nodes = nodes.into_iter().collect();
        plan.ports = ports.into_iter().collect();
        plan.edges = edges.into_iter().collect();
        plan.nodes.sort_unstable();
        plan.ports.sort_unstable();
        plan.edges.sort_unstable();
        self.work.nodes = plan.nodes.len();
        self.work.ports = plan.ports.len();
        self.work.edges = plan.edges.len();
        self.revision = Some(graph.computed.revision());
        self.camera = Some(camera);
        plan
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

    pub(super) fn element_for_node(&self, node: NodeId) -> Option<ElementId> {
        self.nodes.get(&node).map(|(id, _)| *id)
    }

    pub(super) fn node_element(&self, node: NodeId, size: Size) -> Option<ElementId> {
        self.nodes
            .get(&node)
            // Subpixel transform rounding must not replace a pressed subtree.
            .filter(|(_, previous)| {
                (previous.width - size.width).abs() < 1e-6
                    && (previous.height - size.height).abs() < 1e-6
            })
            .map(|(id, _)| *id)
    }
    pub(super) fn remove_node(&mut self, node: NodeId) -> Option<ElementId> {
        self.readings.remove(&node);
        self.nodes.remove(&node).map(|(id, _)| id)
    }
    pub(super) fn insert_node(&mut self, node: NodeId, id: ElementId, size: Size) {
        self.nodes.insert(node, (id, size));
    }
    pub(super) fn port_element(&self, port: PortId) -> Option<ElementId> {
        self.ports.get(&port).copied()
    }
    pub(super) fn insert_port(&mut self, port: PortId, id: ElementId) {
        self.ports.insert(port, id);
    }
    pub(super) fn sync_selection(&mut self, ui: &mut Ui, graph: &DemoGraph) {
        if self.selected == Some(graph.selected) {
            return;
        }
        for node in self.selected.into_iter().chain(Some(graph.selected)) {
            if let Some(element) = self.element_for_node(node)
                && let Some(data) = graph.doc.node(node)
            {
                style_node(ui, element, data.meta.accent, node == graph.selected);
            }
        }
        self.selected = Some(graph.selected);
    }
    pub(super) fn sync_readings(&self, ui: &mut Ui, graph: &DemoGraph) {
        for (node, label) in &self.readings {
            ui.set_local(*label, ui.properties().text, graph.node_summary(*node));
        }
    }
    pub(super) fn insert_reading(&mut self, node: NodeId, label: ElementId) {
        self.readings.insert(node, label);
    }
    pub(super) fn sync_edges(
        &mut self,
        ui: &mut Ui,
        parent: ElementId,
        graph: &DemoGraph,
        edges: &[EdgeId],
    ) {
        for &edge in edges {
            let points: Vec<_> = graph
                .computed
                .edge_route(edge)
                .unwrap()
                .points
                .iter()
                .map(|p| graph.session.viewport().world_to_view_point(*p))
                .collect();
            let data = graph.doc.edge(edge).unwrap();
            let color = graph.doc.port(data.output).unwrap().meta.color();
            let primitive = wire_primitive(&points, color);
            if let Some(element) = self.edges.get(&edge) {
                overstory::set_vector_path(ui, *element, primitive);
            } else {
                let element = ui.append(
                    parent,
                    overstory::VectorPath::from_primitive(primitive).layout(
                        Layout::new()
                            .absolute_at(0., 0.)
                            .logical_size(GRAPH_VIEW.width(), GRAPH_VIEW.height()),
                    ),
                );
                self.edges.insert(edge, element);
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
