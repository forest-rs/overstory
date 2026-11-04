// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A small host model: packing twelve fixed-size instruments into one case.

use super::*;
use understory_node_graph::{ConnectionContext, PortCompatibility};

#[derive(Debug)]
pub(super) struct StoryNodes {
    layout: NodeId,
    check: NodeId,
    cut: NodeId,
}

pub(super) struct StudyCompatibility;

impl PortCompatibility<NodeMeta, PortMeta, ()> for StudyCompatibility {
    fn can_connect(&self, cx: ConnectionContext<'_, NodeMeta, PortMeta, ()>) -> bool {
        cx.output_port().meta.kind == cx.input_port().meta.kind
            && !cx.input_edges().iter().any(|id| {
                cx.doc()
                    .edge(*id)
                    .is_some_and(|edge| edge.output == cx.output())
            })
    }
}

impl StoryNodes {
    pub(super) fn check_node(&self) -> NodeId {
        self.check
    }
}

impl DemoGraph {
    /// The layout reads its actual incoming bindings. This is host arithmetic,
    /// with no scheduler or execution engine hidden in the graph library.
    pub(super) fn effective_parameters(&self) -> Option<CaseParameters> {
        let mut effective = self.parameters;
        for (target, parameter) in ["columns", "spacing", "radius"]
            .into_iter()
            .zip(Parameter::ALL)
        {
            let (input, _) = self.doc.iter_ports().find(|(_, p)| {
                p.owner == self.story.layout
                    && p.direction == PortDirection::Input
                    && p.meta.label == target
            })?;
            let edges = self.doc.port_edges(input)?;
            if edges.len() != 1 {
                return None;
            }
            let output = self.doc.port(self.doc.edge(edges[0])?.output)?;
            if output.owner != self.parameter_node {
                return None;
            }
            let source = ["columns", "spacing", "radius"]
                .into_iter()
                .zip(Parameter::ALL)
                .find(|(label, _)| *label == output.meta.label)?
                .1;
            effective.set(parameter, self.parameters.value(source));
        }
        Some(effective)
    }

    pub(super) fn node_summary(&self, node: NodeId) -> String {
        let Some(values) = self.effective_parameters() else {
            return "Connect the three layout inputs".into();
        };
        let size = values.footprint();
        let clearance = values.clearance();
        if node == self.story.layout {
            format!(
                "{} columns × {} rows\n{:.0} × {:.0} mm footprint",
                values.columns(),
                values.rows(),
                size.width,
                size.height
            )
        } else if node == self.story.check {
            if clearance.width < 0. || clearance.height < 0. {
                format!(
                    "Does not fit\n{:.0} mm too wide / {:.0} mm too tall",
                    (-clearance.width).max(0.),
                    (-clearance.height).max(0.)
                )
            } else {
                format!(
                    "Fits the case\n{:.0} mm / {:.0} mm spare",
                    clearance.width, clearance.height
                )
            }
        } else if node == self.story.cut {
            format!(
                "12 plates · 44 × 32 mm\nCorner radius {:.0} mm",
                values.value(Parameter::Radius)
            )
        } else {
            "Twelve instruments / one field case".into()
        }
    }

    pub(super) fn rebind(&mut self, from: PortId, to: PortId) -> bool {
        if self.doc.can_connect_with(from, to, &StudyCompatibility) != Ok(true) {
            return false;
        }
        let old = self.doc.port_edges(to).unwrap().to_vec();
        let (new, changes) = self.doc.edit(|edit| {
            for edge in &old {
                edit.remove_edge(*edge);
            }
            edit.add_edge(from, to, ())
                .expect("endpoints were checked before editing")
        });
        for edge in old {
            self.tie.replace_member(edge, new);
        }
        self.invalidation.record(&changes);
        true
    }
}

pub(super) fn build_demo_graph() -> DemoGraph {
    let mut doc = DemoDoc::new();
    let parameter_node = add_node(
        &mut doc,
        NodeMeta {
            title: "Instrument layout",
            accent: ValueKind::Count.color(),
        },
    );
    let layout = add_node(
        &mut doc,
        NodeMeta {
            title: "Arrange instruments",
            accent: ValueKind::Length.color(),
        },
    );
    let check = add_node(
        &mut doc,
        NodeMeta {
            title: "Case clearance",
            accent: ValueKind::Geometry.color(),
        },
    );
    let cut = add_node(
        &mut doc,
        NodeMeta {
            title: "Plate specification",
            accent: ValueKind::Geometry.color(),
        },
    );
    let mut projection = DemoProjection::new();
    for (node, origin, size) in [
        (parameter_node, Point::new(0., 0.), Size::new(380., 700.)),
        (layout, Point::new(530., 180.), Size::new(270., 220.)),
        (check, Point::new(960., 90.), Size::new(270., 180.)),
        (cut, Point::new(960., 390.), Size::new(270., 180.)),
    ] {
        projection.set_node_view(node, NodeView::new(origin, size, ()));
    }
    let mut members = Vec::new();
    for (index, label) in ["columns", "spacing", "radius"].into_iter().enumerate() {
        let kind = if index == 0 {
            ValueKind::Count
        } else {
            ValueKind::Length
        };
        let from = output(&mut doc, parameter_node, label, kind);
        let to = input(&mut doc, layout, label, kind);
        members.push(doc.add_edge(from, to, ()).unwrap());
        for (port, point) in [
            (from, Point::new(380., 118. + index as f64 * 100.)),
            (to, Point::new(0., 60. + index as f64 * 50.)),
        ] {
            let mut view = PortView::new(());
            view.anchor = PortAnchor::NodeLocal(point);
            view.hit_radius = 12.;
            projection.set_port_view(port, view);
        }
    }
    let arrangement = output(&mut doc, layout, "arrangement", ValueKind::Geometry);
    for node in [check, cut] {
        let to = input(&mut doc, node, "arrangement", ValueKind::Geometry);
        doc.add_edge(arrangement, to, ()).unwrap();
    }
    let tie = CableTie::new(Point::new(435., 280.), members);
    let mut session = GraphSession::new(GRAPH_VIEW);
    let bounds = projection_bounds(&projection).inflate(72., 72.);
    session.update_viewport(|viewport| {
        viewport.set_fit_mode(FitMode::Center);
        viewport.set_clamp_mode(ClampMode::KeepSomeVisible);
        viewport.set_world_bounds(Some(bounds.inflate(220., 180.)));
        viewport.fit_rect(bounds);
    });
    session.update_selection(|selection| {
        let _ = selection.select_only(GraphElementId::Node(check));
    });
    let mut graph = DemoGraph {
        doc,
        projection,
        session,
        computed: GraphComputed::new(),
        selected: check,
        invalidation: GraphInvalidation::new(),
        work: DeriveWork::default(),
        parameter_node,
        parameters: CaseParameters::default(),
        tie,
        story: StoryNodes { layout, check, cut },
    };
    graph.rebuild_computed();
    graph
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewiring_length_inputs_changes_the_study_and_keeps_tie_membership_current() {
        let mut graph = build_demo_graph();
        let from = graph
            .doc
            .iter_ports()
            .find(|(_, p)| p.owner == graph.parameter_node && p.meta.label == "radius")
            .unwrap()
            .0;
        let to = graph
            .doc
            .iter_ports()
            .find(|(_, p)| p.owner == graph.story.layout && p.meta.label == "spacing")
            .unwrap()
            .0;
        assert!(graph.rebind(from, to));
        graph.rebuild_computed();
        assert_eq!(
            graph
                .effective_parameters()
                .unwrap()
                .value(Parameter::Spacing),
            10.
        );
        assert_eq!(graph.work.routes, 1);
        assert!(
            graph
                .tie
                .members()
                .iter()
                .all(|edge| graph.doc.edge(*edge).is_some())
        );
        assert!(!graph.rebind(from, to));
    }
}
