// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Application-owned cable-tie experiment. Membership is not semantic topology.

use kurbo::Point;
use understory_node_graph::{
    EdgeId, EdgeRouter, OrthogonalEdgeRouter, RouteContext, RouteRequest, RoutedEdge,
};

#[derive(Debug)]
pub(super) struct CableTie {
    pub(super) center: Point,
    pub(super) enabled: bool,
    // Member order is lane order; document iteration cannot reshuffle lanes.
    members: Vec<EdgeId>,
}

impl CableTie {
    pub(super) fn new(center: Point, members: Vec<EdgeId>) -> Self {
        Self {
            center,
            enabled: true,
            members,
        }
    }

    pub(super) fn members(&self) -> &[EdgeId] {
        &self.members
    }

    pub(super) fn lane(&self, edge: EdgeId) -> Option<f64> {
        let index = self.members.iter().position(|id| *id == edge)?;
        Some(
            self.center.y
                + (index as f64 - (self.members.len().saturating_sub(1)) as f64 * 0.5) * 8.,
        )
    }
}

pub(super) struct CableRouter<'a>(pub(super) &'a CableTie);

impl EdgeRouter for CableRouter<'_> {
    fn route(&self, request: RouteRequest, cx: &RouteContext) -> RoutedEdge {
        if self.0.enabled
            && let RouteRequest::Edge(edge) = request
            && let Some(y) = self.0.lane(edge)
        {
            let x = self.0.center.x;
            let output = cx.output_anchor;
            let input = cx.input_anchor;
            return RoutedEdge::from_points(vec![
                output,
                Point::new(x - 30., output.y),
                Point::new(x - 30., y),
                Point::new(x - 14., y),
                Point::new(x + 14., y),
                Point::new(input.x - 44., y),
                Point::new(input.x - 44., input.y),
                input,
            ]);
        }
        OrthogonalEdgeRouter {
            min_horizontal_run: 44.,
        }
        .route(request, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    #[test]
    fn moving_a_tie_only_reroutes_its_members_without_editing_topology() {
        let mut graph = build_demo_graph();
        let document = graph.doc.revision();
        let projection = graph.projection.revision();
        let before = graph
            .tie
            .members()
            .iter()
            .map(|e| graph.computed.edge_route(*e).unwrap().points.clone())
            .collect::<Vec<_>>();
        graph.move_tie(Vec2::new(14., 20.));
        graph.rebuild_computed();
        assert_eq!(graph.doc.revision(), document);
        assert_eq!(graph.projection.revision(), projection);
        assert_eq!(graph.work.nodes, 0);
        assert_eq!(graph.work.routes, 3);
        assert!(!graph.work.full_geometry);
        let lanes = graph
            .tie
            .members()
            .iter()
            .map(|edge| graph.tie.lane(*edge).unwrap())
            .collect::<Vec<_>>();
        assert!(lanes.windows(2).all(|ys| ys[1] - ys[0] == 8.));
        for (edge, old) in graph.tie.members().iter().zip(before) {
            let route = graph.computed.edge_route(*edge).unwrap();
            assert_ne!(route.points, old);
            let edge_data = graph.doc.edge(*edge).unwrap();
            assert_eq!(
                route.points.first().copied(),
                graph.computed.port_anchor(edge_data.output)
            );
            assert_eq!(
                route.points.last().copied(),
                graph.computed.port_anchor(edge_data.input)
            );
            assert!(route.points.contains(&Point::new(
                graph.tie.center.x - 14.,
                graph.tie.lane(*edge).unwrap()
            )));
        }
        graph.tie.enabled = false;
        graph.invalidate_tie();
        graph.rebuild_computed();
        assert_eq!(graph.work.routes, 3);
    }

    #[test]
    fn transient_routes_do_not_join_a_committed_bundle() {
        let graph = build_demo_graph();
        let edge = graph.doc.edge(graph.tie.members()[0]).unwrap();
        let output_anchor = graph.computed.port_anchor(edge.output).unwrap();
        let input_anchor = Point::new(600., 300.);
        let cx = RouteContext {
            output_anchor,
            input_anchor,
        };
        let route = CableRouter(&graph.tie).route(
            RouteRequest::Preview {
                from: edge.output,
                to: None,
            },
            &cx,
        );
        let ordinary = OrthogonalEdgeRouter {
            min_horizontal_run: 44.,
        }
        .route(
            RouteRequest::Preview {
                from: edge.output,
                to: None,
            },
            &cx,
        );
        assert_eq!(route.points, ordinary.points);
    }
}
