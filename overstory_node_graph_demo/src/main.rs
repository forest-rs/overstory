// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Element-based node graph showcase.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use kurbo::{Point, Rect, Size};
use overstory::{ClickEvent, ElementId, Layout, Panel, Phase, Text, Tooltip, Ui, theme};
use overstory_host_winit::WindowOptions;
use peniko::{Brush, Color};
use understory_node_graph::{
    GraphComputed, GraphDoc, GraphElementId, GraphInvalidation, GraphProjection, GraphSession,
    NodeData, NodeId, NodeView, NoopGraphDeriveObserver, OrthogonalEdgeRouter, PortDirection,
    PortId, PortView,
};
use understory_view2d::{ClampMode, FitMode};

const INITIAL_SIZE: (u32, u32) = (1480, 820);
const GRAPH_VIEW: Rect = Rect::new(0.0, 0.0, 1040.0, 640.0);
const EDGE_THICKNESS: f64 = 4.0;
const PORT_RADIUS: f64 = 7.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ValueKind {
    Event,
    Image,
    Tensor,
    Scalar,
    Color,
}

impl ValueKind {
    fn color(self) -> Color {
        match self {
            Self::Event => Color::from_rgba8(0xff, 0xd1, 0x66, 0xff),
            Self::Image => Color::from_rgba8(0x69, 0xd2, 0xff, 0xff),
            Self::Tensor => Color::from_rgba8(0x9d, 0x7d, 0xff, 0xff),
            Self::Scalar => Color::from_rgba8(0x70, 0xe2, 0x9a, 0xff),
            Self::Color => Color::from_rgba8(0xff, 0x6d, 0xb1, 0xff),
        }
    }
}

impl core::fmt::Display for ValueKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Event => f.write_str("event"),
            Self::Image => f.write_str("image"),
            Self::Tensor => f.write_str("tensor"),
            Self::Scalar => f.write_str("scalar"),
            Self::Color => f.write_str("color"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct NodeMeta {
    title: &'static str,
    subtitle: &'static str,
    accent: Color,
}

#[derive(Clone, Copy, Debug)]
struct PortMeta {
    label: &'static str,
    kind: ValueKind,
}

type DemoDoc = GraphDoc<NodeMeta, PortMeta, ()>;
type DemoProjection = GraphProjection<(), (), ()>;

#[derive(Debug)]
struct DemoGraph {
    doc: DemoDoc,
    session: GraphSession,
    computed: GraphComputed,
    selected: NodeId,
}

fn main() -> overstory_host_winit::HostResult<()> {
    overstory_host_winit::run(
        WindowOptions::new("overstory_node_graph_demo")
            .initial_size(INITIAL_SIZE.0, INITIAL_SIZE.1),
        build_ui,
    )
}

fn build_ui() -> Ui {
    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    let root = ui.root();
    ui.set_layout(
        root,
        Layout::new()
            .relative()
            .percent_size(1.0, 1.0)
            .padding_all(0.0),
    );
    style_surface(
        &mut ui,
        root,
        Some(Color::from_rgba8(0x14, 0x14, 0x1c, 0xff)),
        None,
        0.0,
        0.0,
    );

    let graph = Rc::new(RefCell::new(build_demo_graph()));
    let selected_element = Rc::new(Cell::new(None));

    let title_panel = ui.append(
        root,
        Panel::column(8.0).layout(
            Layout::column(8.0)
                .absolute_at(36.0, 28.0)
                .logical_size(640.0, 72.0),
        ),
    );
    ui.append(
        title_panel,
        Text::new("Vision Pipeline")
            .font_size(30.0)
            .foreground(Color::from_rgba8(0xf7, 0xf3, 0xff, 0xff)),
    );
    ui.append(
        title_panel,
        Text::new("Realtime image processing graph")
            .font_size(13.0)
            .foreground(Color::from_rgba8(0xb7, 0xb8, 0xc8, 0xff)),
    );

    let graph_host = ui.append(
        root,
        Panel::new().layout(
            Layout::new()
                .relative()
                .absolute_at(36.0, 128.0)
                .logical_size(
                    f32_coord(GRAPH_VIEW.width()),
                    f32_coord(GRAPH_VIEW.height()),
                ),
        ),
    );
    style_surface(
        &mut ui,
        graph_host,
        Some(Color::from_rgba8(0x1d, 0x1d, 0x28, 0xff)),
        Some(Color::from_rgba8(0x43, 0x44, 0x52, 0xff)),
        1.0,
        18.0,
    );

    append_grid(&mut ui, graph_host, GRAPH_VIEW);
    append_graph_edges(&mut ui, graph_host, &graph.borrow());
    append_graph_nodes(
        &mut ui,
        graph_host,
        Rc::clone(&graph),
        Rc::clone(&selected_element),
    );
    append_graph_ports(&mut ui, graph_host, &graph.borrow());

    let sidebar = ui.append(
        root,
        Panel::column(14.0).layout(
            Layout::column(14.0)
                .absolute_at(1110.0, 128.0)
                .logical_size(322.0, 640.0)
                .padding_all(18.0),
        ),
    );
    style_surface(
        &mut ui,
        sidebar,
        Some(Color::from_rgba8(0x25, 0x25, 0x30, 0xff)),
        Some(Color::from_rgba8(0x45, 0x46, 0x54, 0xff)),
        1.0,
        14.0,
    );
    append_inspector(&mut ui, sidebar, &graph.borrow());

    ui
}

fn build_demo_graph() -> DemoGraph {
    let mut doc = DemoDoc::new();
    let camera = add_node(
        &mut doc,
        NodeMeta {
            title: "Camera Feed",
            subtitle: "captures frames",
            accent: Color::from_rgba8(0x61, 0xd5, 0xff, 0xff),
        },
    );
    let denoise = add_node(
        &mut doc,
        NodeMeta {
            title: "Temporal Denoise",
            subtitle: "stabilizes motion",
            accent: Color::from_rgba8(0x7a, 0xf1, 0xa8, 0xff),
        },
    );
    let segment = add_node(
        &mut doc,
        NodeMeta {
            title: "Segment Objects",
            subtitle: "tensor model",
            accent: Color::from_rgba8(0xa6, 0x7b, 0xff, 0xff),
        },
    );
    let grade = add_node(
        &mut doc,
        NodeMeta {
            title: "Color Grade",
            subtitle: "scene look",
            accent: Color::from_rgba8(0xff, 0x70, 0xb8, 0xff),
        },
    );
    let composite = add_node(
        &mut doc,
        NodeMeta {
            title: "Composite Output",
            subtitle: "final frame",
            accent: Color::from_rgba8(0xff, 0xd5, 0x70, 0xff),
        },
    );

    let _camera_tick = input(&mut doc, camera, "tick", ValueKind::Event);
    let camera_image = output(&mut doc, camera, "image", ValueKind::Image);
    let denoise_image = input(&mut doc, denoise, "image", ValueKind::Image);
    let _denoise_strength = input(&mut doc, denoise, "strength", ValueKind::Scalar);
    let denoise_clean = output(&mut doc, denoise, "clean", ValueKind::Image);
    let segment_image = input(&mut doc, segment, "image", ValueKind::Image);
    let segment_mask = output(&mut doc, segment, "mask", ValueKind::Tensor);
    let grade_image = input(&mut doc, grade, "image", ValueKind::Image);
    let _grade_lut = input(&mut doc, grade, "look", ValueKind::Color);
    let grade_out = output(&mut doc, grade, "graded", ValueKind::Image);
    let composite_bg = input(&mut doc, composite, "background", ValueKind::Image);
    let composite_mask = input(&mut doc, composite, "mask", ValueKind::Tensor);
    let _composite_out = output(&mut doc, composite, "frame", ValueKind::Image);

    doc.add_edge(camera_image, denoise_image, ()).unwrap();
    doc.add_edge(denoise_clean, segment_image, ()).unwrap();
    doc.add_edge(denoise_clean, grade_image, ()).unwrap();
    doc.add_edge(segment_mask, composite_mask, ()).unwrap();
    doc.add_edge(grade_out, composite_bg, ()).unwrap();

    let mut projection = DemoProjection::new();
    projection.set_node_view(
        camera,
        NodeView::new(Point::new(0.0, 150.0), Size::new(190.0, 118.0), ()),
    );
    projection.set_node_view(
        denoise,
        NodeView::new(Point::new(280.0, 88.0), Size::new(220.0, 148.0), ()),
    );
    projection.set_node_view(
        segment,
        NodeView::new(Point::new(620.0, 22.0), Size::new(218.0, 118.0), ()),
    );
    projection.set_node_view(
        grade,
        NodeView::new(Point::new(620.0, 260.0), Size::new(218.0, 136.0), ()),
    );
    projection.set_node_view(
        composite,
        NodeView::new(Point::new(990.0, 156.0), Size::new(236.0, 148.0), ()),
    );
    for port in doc.iter_ports().map(|(port, _)| port).collect::<Vec<_>>() {
        let mut view = PortView::new(());
        view.hit_radius = 10.0;
        projection.set_port_view(port, view);
    }

    let mut session = GraphSession::new(GRAPH_VIEW);
    let world_bounds = projection_bounds(&projection).inflate(72.0, 72.0);
    session.update_viewport(|viewport| {
        viewport.set_fit_mode(FitMode::Center);
        viewport.set_clamp_mode(ClampMode::KeepSomeVisible);
        viewport.set_world_bounds(Some(world_bounds.inflate(220.0, 180.0)));
        viewport.fit_rect(world_bounds);
    });
    session.update_selection(|selection| {
        let _ = selection.select_only(GraphElementId::Node(denoise));
    });

    let mut computed = GraphComputed::new();
    let mut invalidation = GraphInvalidation::new();
    let mut observer = NoopGraphDeriveObserver;
    computed.rebuild(
        &doc,
        &projection,
        &session,
        &mut invalidation,
        &OrthogonalEdgeRouter {
            min_horizontal_run: 44.0,
        },
        &mut observer,
    );

    DemoGraph {
        doc,
        session,
        computed,
        selected: denoise,
    }
}

fn add_node(doc: &mut DemoDoc, meta: NodeMeta) -> NodeId {
    doc.add_node(NodeData { meta })
}

fn input(doc: &mut DemoDoc, node: NodeId, label: &'static str, kind: ValueKind) -> PortId {
    doc.add_port(node, PortDirection::Input, PortMeta { label, kind })
        .expect("node exists")
}

fn output(doc: &mut DemoDoc, node: NodeId, label: &'static str, kind: ValueKind) -> PortId {
    doc.add_port(node, PortDirection::Output, PortMeta { label, kind })
        .expect("node exists")
}

fn append_grid(ui: &mut Ui, parent: ElementId, bounds: Rect) {
    for x in (40..=1000).step_by(80) {
        append_rect(
            ui,
            parent,
            Rect::new(f64::from(x), 0.0, f64::from(x) + 1.0, bounds.height()),
            Color::from_rgba8(0x31, 0x31, 0x3d, 0x80),
            0.0,
        );
    }
    for y in (40..=600).step_by(80) {
        append_rect(
            ui,
            parent,
            Rect::new(0.0, f64::from(y), bounds.width(), f64::from(y) + 1.0),
            Color::from_rgba8(0x31, 0x31, 0x3d, 0x80),
            0.0,
        );
    }
}

fn append_graph_edges(ui: &mut Ui, parent: ElementId, graph: &DemoGraph) {
    for edge in graph.computed.visible_edges() {
        let Some(route) = graph.computed.edge_route(*edge) else {
            continue;
        };
        for points in route.points.windows(2) {
            let start = graph.session.viewport().world_to_view_point(points[0]);
            let end = graph.session.viewport().world_to_view_point(points[1]);
            if (start - end).hypot() <= 0.5 {
                continue;
            }
            append_edge_segment(ui, parent, start, end);
        }
    }
}

fn append_graph_nodes(
    ui: &mut Ui,
    parent: ElementId,
    graph: Rc<RefCell<DemoGraph>>,
    selected_element: Rc<Cell<Option<ElementId>>>,
) {
    let nodes = {
        let graph = graph.borrow();
        graph.computed.visible_nodes().to_vec()
    };
    for node in nodes {
        let (rect, meta, selected) = {
            let graph = graph.borrow();
            let world = graph
                .computed
                .node_bounds(node)
                .expect("visible node has bounds");
            let rect = graph.session.viewport().world_to_view_rect(world);
            let meta = graph.doc.node(node).expect("visible node is live").meta;
            let selected = graph
                .session
                .selection()
                .contains(&GraphElementId::Node(node));
            (rect, meta, selected)
        };
        let node_element = ui.append(
            parent,
            Panel::column(8.0).layout(
                Layout::column(8.0)
                    .absolute_at(f32_coord(rect.x0), f32_coord(rect.y0))
                    .logical_size(f32_coord(rect.width()), f32_coord(rect.height()))
                    .padding_all(14.0),
            ),
        );
        style_node(ui, node_element, meta.accent, selected);
        if selected {
            selected_element.set(Some(node_element));
        }
        ui.set_tooltip(
            node_element,
            Tooltip::new(format!("GraphDoc node: {}", meta.title)),
        );
        let selected_for_handler = Rc::clone(&selected_element);
        let graph_for_handler = Rc::clone(&graph);
        ui.on_click(node_element, move |ui, event| {
            select_node(
                ui,
                event,
                node,
                meta.accent,
                Rc::clone(&selected_for_handler),
                Rc::clone(&graph_for_handler),
            );
        });

        ui.append(
            node_element,
            Text::new(meta.title)
                .font_size(15.0)
                .foreground(Color::from_rgba8(0xf8, 0xf7, 0xff, 0xff)),
        );
        ui.append(
            node_element,
            Text::new(meta.subtitle)
                .font_size(11.0)
                .foreground(Color::from_rgba8(0xb9, 0xbb, 0xca, 0xff)),
        );
    }
}

fn append_graph_ports(ui: &mut Ui, parent: ElementId, graph: &DemoGraph) {
    for (port, data) in graph.doc.iter_ports() {
        let Some(anchor) = graph.computed.port_anchor(port) else {
            continue;
        };
        let view = graph.session.viewport().world_to_view_point(anchor);
        let color = data.meta.kind.color();
        let port_element = append_rect(
            ui,
            parent,
            Rect::new(
                view.x - PORT_RADIUS,
                view.y - PORT_RADIUS,
                view.x + PORT_RADIUS,
                view.y + PORT_RADIUS,
            ),
            color,
            PORT_RADIUS,
        );
        ui.set_tooltip(
            port_element,
            Tooltip::new(format!("{} {}", data.meta.kind, data.meta.label)),
        );
    }
}

fn append_inspector(ui: &mut Ui, parent: ElementId, graph: &DemoGraph) {
    ui.append(
        parent,
        Text::new("Frame Inspector")
            .font_size(20.0)
            .foreground(Color::from_rgba8(0xf7, 0xf3, 0xff, 0xff)),
    );
    let visible_world = graph.session.visible_world_rect();
    let zoom = graph.session.viewport().zoom();
    for line in [
        format!("nodes: {}", graph.doc.node_count()),
        format!("ports: {}", graph.doc.port_count()),
        format!("edges: {}", graph.doc.edge_count()),
        format!("visible nodes: {}", graph.computed.visible_nodes().len()),
        format!("visible edges: {}", graph.computed.visible_edges().len()),
        format!("viewport zoom: {zoom:.2}"),
        format!(
            "world rect: {:.0},{:.0} - {:.0},{:.0}",
            visible_world.x0, visible_world.y0, visible_world.x1, visible_world.y1
        ),
    ] {
        ui.append(
            parent,
            Text::new(line)
                .font_size(13.0)
                .foreground(Color::from_rgba8(0xc8, 0xca, 0xd8, 0xff)),
        );
    }
}

fn select_node(
    ui: &mut Ui,
    event: ClickEvent<'_>,
    node: NodeId,
    accent: Color,
    selected_element: Rc<Cell<Option<ElementId>>>,
    graph: Rc<RefCell<DemoGraph>>,
) {
    if !matches!(event.phase, Phase::Target | Phase::Bubble) {
        return;
    }
    let previous_node = graph.borrow().selected;
    if let Some(previous) = selected_element.replace(Some(event.current))
        && ui.get(previous).is_some()
        && let Some(previous_data) = graph.borrow().doc.node(previous_node)
    {
        style_node(ui, previous, previous_data.meta.accent, false);
    }
    graph.borrow_mut().selected = node;
    graph.borrow_mut().session.update_selection(|selection| {
        let _ = selection.select_only(GraphElementId::Node(node));
    });
    style_node(ui, event.current, accent, true);
    event.mark_handled();
}

fn append_edge_segment(ui: &mut Ui, parent: ElementId, start: Point, end: Point) {
    let horizontal = (start.x - end.x).abs() >= (start.y - end.y).abs();
    let half = EDGE_THICKNESS * 0.5;
    let rect = if horizontal {
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
    append_rect(
        ui,
        parent,
        rect,
        Color::from_rgba8(0x62, 0xd8, 0xff, 0xff),
        EDGE_THICKNESS * 0.5,
    );
}

fn append_rect(ui: &mut Ui, parent: ElementId, rect: Rect, color: Color, radius: f64) -> ElementId {
    let element = ui.append(
        parent,
        Panel::new().layout(
            Layout::new()
                .absolute_at(f32_coord(rect.x0), f32_coord(rect.y0))
                .logical_size(
                    f32_coord(rect.width().max(1.0)),
                    f32_coord(rect.height().max(1.0)),
                ),
        ),
    );
    style_surface(ui, element, Some(color), None, 0.0, radius);
    element
}

fn style_node(ui: &mut Ui, element: ElementId, accent: Color, selected: bool) {
    let bg = if selected {
        Color::from_rgba8(0x31, 0x33, 0x42, 0xff)
    } else {
        Color::from_rgba8(0x27, 0x28, 0x35, 0xff)
    };
    style_surface(ui, element, Some(bg), Some(accent), 1.4, 12.0);
}

fn style_surface(
    ui: &mut Ui,
    element: ElementId,
    background: Option<Color>,
    border: Option<Color>,
    border_width: f64,
    radius: f64,
) {
    if let Some(background) = background {
        ui.set_local(element, ui.properties().background, Some(background.into()));
    }
    ui.set_local(
        element,
        ui.properties().border_brush,
        border.map(Brush::from),
    );
    ui.set_local(element, ui.properties().border_width, border_width);
    ui.set_local(element, ui.properties().corner_radius, radius);
}

fn projection_bounds(projection: &DemoProjection) -> Rect {
    let mut bounds: Option<Rect> = None;
    for (_, view) in projection.iter_node_views() {
        bounds = Some(match bounds {
            Some(existing) => existing.union(view.rect()),
            None => view.rect(),
        });
    }
    bounds.unwrap_or(Rect::new(0.0, 0.0, 1.0, 1.0))
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "demo logical coordinates fit f32 layout values"
)]
fn f32_coord(value: f64) -> f32 {
    value as f32
}
