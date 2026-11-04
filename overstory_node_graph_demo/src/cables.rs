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

    pub(super) fn replace_member(&mut self, old: EdgeId, new: EdgeId) {
        if let Some(member) = self.members.iter_mut().find(|edge| **edge == old) {
            *member = new;
        }
    }

    pub(super) fn members(&self) -> &[EdgeId] {
        &self.members
    }

    pub(super) fn lane(&self, edge: EdgeId) -> Option<f64> {
        let index = self.members.iter().position(|id| *id == edge)?;
        Some(
            self.center.y
                + (index as f64 - (self.members.len().saturating_sub(1)) as f64 * 0.5) * 14.,
        )
    }
}

pub(super) struct CableRouter<'a>(pub(super) &'a CableTie);

impl EdgeRouter for CableRouter<'_> {
    fn route(&self, request: RouteRequest, cx: &RouteContext) -> RoutedEdge {
        if !self.0.enabled
            && let RouteRequest::Edge(edge) = request
            && let Some(index) = self.0.members.iter().position(|id| *id == edge)
        {
            let rank = if cx.input_anchor.y >= cx.output_anchor.y {
                self.0.members.len() - index
            } else {
                index + 1
            };
            let fraction = rank as f64 / (self.0.members.len() + 1) as f64;
            let x = cx.output_anchor.x + (cx.input_anchor.x - cx.output_anchor.x) * fraction;
            return RoutedEdge::from_points(vec![
                cx.output_anchor,
                Point::new(x, cx.output_anchor.y),
                Point::new(x, cx.input_anchor.y),
                cx.input_anchor,
            ]);
        }
        if self.0.enabled
            && let RouteRequest::Edge(edge) = request
            && let Some(y) = self.0.lane(edge)
        {
            let x = self.0.center.x;
            let output = cx.output_anchor;
            let input = cx.input_anchor;
            let index = self.0.members.iter().position(|id| *id == edge).unwrap();
            let last = self.0.members.len().saturating_sub(1);
            let entry_rank = if output.y < y { index } else { last - index };
            let exit_rank = if input.y > y { last - index } else { index };
            let entry = x - 22. - 24. - entry_rank as f64 * 14.;
            let exit = x + 22. + 24. + exit_rank as f64 * 14.;
            return RoutedEdge::from_points(vec![
                output,
                Point::new(entry, output.y),
                Point::new(entry, y),
                Point::new(x - 22., y),
                Point::new(x + 22., y),
                Point::new(exit, y),
                Point::new(exit, input.y),
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
    fn ordered_parameter_routes_do_not_cross_or_share_runs() {
        let mut graph = build_demo_graph();
        for (enabled, y) in [(true, 262.), (true, 342.), (true, 422.), (false, 342.)] {
            graph.tie.enabled = enabled;
            graph.tie.center.y = y;
            graph.invalidate_tie();
            graph.rebuild_computed();
            let routes = graph
                .tie
                .members()
                .iter()
                .map(|edge| graph.computed.edge_route(*edge).unwrap())
                .collect::<Vec<_>>();
            for (index, first) in routes.iter().enumerate() {
                for second in routes.iter().skip(index + 1) {
                    for (a, b) in first.segments() {
                        for (c, d) in second.segments() {
                            let ar =
                                Rect::new(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y));
                            let br =
                                Rect::new(c.x.min(d.x), c.y.min(d.y), c.x.max(d.x), c.y.max(d.y));
                            assert!(
                                ar.x1 < br.x0 || br.x1 < ar.x0 || ar.y1 < br.y0 || br.y1 < ar.y0,
                                "crossing {a:?} {b:?} with {c:?} {d:?}"
                            );
                        }
                    }
                }
            }
            let colors = graph
                .tie
                .members()
                .iter()
                .map(|edge| {
                    graph
                        .doc
                        .port(graph.doc.edge(*edge).unwrap().output)
                        .unwrap()
                        .meta
                        .color()
                })
                .collect::<Vec<_>>();
            assert!(colors[0] != colors[1] && colors[1] != colors[2] && colors[0] != colors[2]);
        }
    }

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
        assert!(lanes.windows(2).all(|ys| ys[1] - ys[0] == 14.));
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
                graph.tie.center.x - 22.,
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
