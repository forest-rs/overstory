// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Element-based node graph showcase.

extern crate alloc;

use alloc::rc::Rc;
use core::cell::{Cell, RefCell};
use kurbo::{Point, Rect, Size, Vec2};
use overstory::{
    ClickEvent, CursorIcon, DragEvent, ElementId, Layout, Panel, Phase, PointerEventKind,
    PointerRoutedEvent, ScrollEvent, SurfaceSpecExt, Text, Tooltip, Ui, Viewport, theme,
};
use overstory_host_winit::WindowOptions;
use peniko::{Brush, Color};
use understory_node_graph::{
    EdgePreviewTarget, GraphComputed, GraphDoc, GraphElementId, GraphInvalidation, GraphProjection,
    GraphSession, HitTarget, InteractionState, NodeData, NodeId, NodeView, NoopGraphDeriveObserver,
    OrthogonalEdgeRouter, PortDirection, PortId, PortView, RejectDuplicateConnections,
};
use understory_view2d::{ClampMode, FitMode};

const INITIAL_SIZE: (u32, u32) = (1480, 820);
const GRAPH_VIEW: Rect = Rect::new(0.0, 0.0, 1040.0, 640.0);
const EDGE_THICKNESS: f64 = 4.0;
const GRID_LINE_LIMIT: usize = 240;
const PORT_RADIUS: f64 = 7.0;
const ZOOM_SENSITIVITY: f64 = 0.0015;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GraphGesture {
    DragNode(NodeId),
    CreateEdge(PortId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ValueKind {
    Event,
    State,
    Geometry,
    Primitive,
    Layer,
    Frame,
}

impl ValueKind {
    fn color(self) -> Color {
        match self {
            Self::Event => Color::from_rgba8(0xff, 0xd1, 0x66, 0xff),
            Self::State => Color::from_rgba8(0x70, 0xe2, 0x9a, 0xff),
            Self::Geometry => Color::from_rgba8(0x69, 0xd2, 0xff, 0xff),
            Self::Primitive => Color::from_rgba8(0x9d, 0x7d, 0xff, 0xff),
            Self::Layer => Color::from_rgba8(0xff, 0x6d, 0xb1, 0xff),
            Self::Frame => Color::from_rgba8(0xff, 0xd1, 0x66, 0xff),
        }
    }
}

impl core::fmt::Display for ValueKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Event => f.write_str("event"),
            Self::State => f.write_str("state"),
            Self::Geometry => f.write_str("geometry"),
            Self::Primitive => f.write_str("primitive"),
            Self::Layer => f.write_str("layer"),
            Self::Frame => f.write_str("frame"),
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
    projection: DemoProjection,
    session: GraphSession,
    computed: GraphComputed,
    selected: NodeId,
}

impl DemoGraph {
    fn rebuild_computed(&mut self) {
        let mut invalidation = GraphInvalidation::new();
        invalidation.mark_viewport();
        let mut observer = NoopGraphDeriveObserver;
        self.computed.rebuild_with_compatibility(
            &self.doc,
            &self.projection,
            &self.session,
            &mut invalidation,
            &OrthogonalEdgeRouter {
                min_horizontal_run: 44.0,
            },
            &RejectDuplicateConnections,
            &mut observer,
        );
    }
}

#[derive(Debug)]
struct GraphDemoRuntime {
    graph: RefCell<DemoGraph>,
    selected_element: Cell<Option<ElementId>>,
    active_gesture: Cell<Option<GraphGesture>>,
}

impl GraphDemoRuntime {
    fn new(graph: DemoGraph) -> Self {
        Self {
            graph: RefCell::new(graph),
            selected_element: Cell::new(None),
            active_gesture: Cell::new(None),
        }
    }
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
    ui.set_layout(root, Layout::new().relative().percent_size(1.0, 1.0));
    style_surface(
        &mut ui,
        root,
        Some(Color::from_rgba8(0x14, 0x14, 0x1c, 0xff)),
        None,
        0.0,
        0.0,
    );

    let runtime = Rc::new(GraphDemoRuntime::new(build_demo_graph()));

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
        Text::new("Overstory Frame Graph")
            .font_size(30.0)
            .foreground(Color::from_rgba8(0xf7, 0xf3, 0xff, 0xff)),
    );
    ui.append(
        title_panel,
        Text::new("Input, layout, presentation, layers, and final frame")
            .font_size(13.0)
            .foreground(Color::from_rgba8(0xb7, 0xb8, 0xc8, 0xff)),
    );

    let graph_host = ui.append(root, Viewport::new().cursor_icon(CursorIcon::Grab));
    ui.set_layout(
        graph_host,
        Layout::new()
            .relative()
            .absolute_at(36.0, 128.0)
            .logical_size(
                f32_coord(GRAPH_VIEW.width()),
                f32_coord(GRAPH_VIEW.height()),
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

    let sidebar = ui.append(
        root,
        Panel::column(14.0).padding_all(18.0).layout(
            Layout::column(14.0)
                .absolute_at(1110.0, 128.0)
                .logical_size(322.0, 640.0),
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

    install_viewport_interaction(&mut ui, graph_host, sidebar, Rc::clone(&runtime));
    refresh_graph_view(&mut ui, graph_host, sidebar, Rc::clone(&runtime));

    ui
}

fn build_demo_graph() -> DemoGraph {
    let mut doc = DemoDoc::new();
    let events = add_node(
        &mut doc,
        NodeMeta {
            title: "Winit Input",
            subtitle: "pointer, key, wheel",
            accent: Color::from_rgba8(0xff, 0xd1, 0x66, 0xff),
        },
    );
    let ui_state = add_node(
        &mut doc,
        NodeMeta {
            title: "Ui State",
            subtitle: "tree, focus, timers",
            accent: Color::from_rgba8(0x7a, 0xf1, 0xa8, 0xff),
        },
    );
    let layout = add_node(
        &mut doc,
        NodeMeta {
            title: "Style + Layout",
            subtitle: "cascade and boxes",
            accent: Color::from_rgba8(0x61, 0xd5, 0xff, 0xff),
        },
    );
    let presentation = add_node(
        &mut doc,
        NodeMeta {
            title: "Presentation",
            subtitle: "surfaces and text",
            accent: Color::from_rgba8(0xa6, 0x7b, 0xff, 0xff),
        },
    );
    let layers = add_node(
        &mut doc,
        NodeMeta {
            title: "Layer Plan",
            subtitle: "root, popups, clips",
            accent: Color::from_rgba8(0xff, 0x70, 0xb8, 0xff),
        },
    );
    let present = add_node(
        &mut doc,
        NodeMeta {
            title: "GPU Present",
            subtitle: "cached scene surfaces",
            accent: Color::from_rgba8(0xff, 0xd5, 0x70, 0xff),
        },
    );

    let _platform_events = input(&mut doc, events, "platform", ValueKind::Event);
    let routed_events = output(&mut doc, events, "routed", ValueKind::Event);
    let state_events = input(&mut doc, ui_state, "events", ValueKind::Event);
    let ui_tree = output(&mut doc, ui_state, "tree", ValueKind::State);
    let layout_tree = input(&mut doc, layout, "tree", ValueKind::State);
    let boxes = output(&mut doc, layout, "boxes", ValueKind::Geometry);
    let presentation_boxes = input(&mut doc, presentation, "boxes", ValueKind::Geometry);
    let primitives = output(&mut doc, presentation, "primitives", ValueKind::Primitive);
    let layer_primitives = input(&mut doc, layers, "primitives", ValueKind::Primitive);
    let layer_tree = output(&mut doc, layers, "layers", ValueKind::Layer);
    let present_layers = input(&mut doc, present, "layers", ValueKind::Layer);
    let _frame = output(&mut doc, present, "frame", ValueKind::Frame);

    doc.add_edge(routed_events, state_events, ()).unwrap();
    doc.add_edge(ui_tree, layout_tree, ()).unwrap();
    doc.add_edge(boxes, presentation_boxes, ()).unwrap();
    doc.add_edge(primitives, layer_primitives, ()).unwrap();
    doc.add_edge(layer_tree, present_layers, ()).unwrap();

    let mut projection = DemoProjection::new();
    projection.set_node_view(
        events,
        NodeView::new(Point::new(0.0, 170.0), Size::new(196.0, 118.0), ()),
    );
    projection.set_node_view(
        ui_state,
        NodeView::new(Point::new(286.0, 104.0), Size::new(216.0, 136.0), ()),
    );
    projection.set_node_view(
        layout,
        NodeView::new(Point::new(600.0, 48.0), Size::new(218.0, 132.0), ()),
    );
    projection.set_node_view(
        presentation,
        NodeView::new(Point::new(600.0, 282.0), Size::new(224.0, 132.0), ()),
    );
    projection.set_node_view(
        layers,
        NodeView::new(Point::new(930.0, 124.0), Size::new(216.0, 142.0), ()),
    );
    projection.set_node_view(
        present,
        NodeView::new(Point::new(1236.0, 172.0), Size::new(222.0, 126.0), ()),
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
        let _ = selection.select_only(GraphElementId::Node(presentation));
    });

    let mut computed = GraphComputed::new();
    let mut invalidation = GraphInvalidation::new();
    let mut observer = NoopGraphDeriveObserver;
    computed.rebuild_with_compatibility(
        &doc,
        &projection,
        &session,
        &mut invalidation,
        &OrthogonalEdgeRouter {
            min_horizontal_run: 44.0,
        },
        &RejectDuplicateConnections,
        &mut observer,
    );

    DemoGraph {
        doc,
        projection,
        session,
        computed,
        selected: presentation,
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

fn install_viewport_interaction(
    ui: &mut Ui,
    graph_host: ElementId,
    sidebar: ElementId,
    runtime: Rc<GraphDemoRuntime>,
) {
    let drag_runtime = Rc::clone(&runtime);
    ui.on_drag(graph_host, move |ui, event| {
        drag_graph_view(ui, event, graph_host, sidebar, Rc::clone(&drag_runtime));
    });

    let scroll_runtime = Rc::clone(&runtime);
    ui.on_scroll(graph_host, move |ui, event| {
        zoom_graph_view(ui, event, graph_host, sidebar, Rc::clone(&scroll_runtime));
    });

    let up_runtime = Rc::clone(&runtime);
    ui.on_pointer(graph_host, PointerEventKind::Up, move |ui, event| {
        finish_graph_gesture(ui, event, graph_host, sidebar, Rc::clone(&up_runtime));
    });

    let cancel_runtime = Rc::clone(&runtime);
    ui.on_pointer(graph_host, PointerEventKind::Cancel, move |ui, event| {
        cancel_graph_gesture(ui, event, graph_host, sidebar, Rc::clone(&cancel_runtime));
    });
}

fn refresh_graph_view(
    ui: &mut Ui,
    graph_host: ElementId,
    sidebar: ElementId,
    runtime: Rc<GraphDemoRuntime>,
) {
    runtime.selected_element.set(None);
    clear_children(ui, graph_host);
    clear_children(ui, sidebar);

    {
        let graph = runtime.graph.borrow();
        append_grid(ui, graph_host, &graph);
        append_graph_edges(ui, graph_host, &graph);
        append_graph_preview(ui, graph_host, &graph);
    }
    append_graph_nodes(ui, graph_host, sidebar, Rc::clone(&runtime));
    append_graph_ports(ui, graph_host, sidebar, Rc::clone(&runtime));
    {
        let graph = runtime.graph.borrow();
        append_inspector(ui, sidebar, &graph);
    }
}

fn refresh_inspector(ui: &mut Ui, sidebar: ElementId, graph: &DemoGraph) {
    clear_children(ui, sidebar);
    append_inspector(ui, sidebar, graph);
}

fn clear_children(ui: &mut Ui, parent: ElementId) {
    let children = ui
        .get(parent)
        .map(|element| element.children().to_vec())
        .unwrap_or_default();
    for child in children {
        if ui.get(child).is_some() {
            ui.remove(child);
        }
    }
}

fn drag_graph_view(
    ui: &mut Ui,
    event: DragEvent<'_>,
    graph_host: ElementId,
    sidebar: ElementId,
    runtime: Rc<GraphDemoRuntime>,
) {
    if !matches!(event.phase, Phase::Capture | Phase::Target) {
        return;
    }
    if event.delta.hypot() <= 0.0 {
        return;
    }

    let _ = ui.capture_pointer(graph_host);
    match runtime.active_gesture.get() {
        Some(GraphGesture::DragNode(node)) => {
            let mut graph = runtime.graph.borrow_mut();
            move_node_by_view_delta(&mut graph, node, event.delta);
            graph.rebuild_computed();
        }
        Some(GraphGesture::CreateEdge(port)) => {
            let pointer = local_point(ui, graph_host, event.position);
            let mut graph = runtime.graph.borrow_mut();
            let hover = graph.computed.hit_test_view(
                &graph.doc,
                &graph.projection,
                &graph.session,
                pointer,
            );
            graph.session.set_hover(hover);
            graph.session.set_interaction(InteractionState::CreateEdge {
                from: port,
                pointer,
            });
            graph.rebuild_computed();
        }
        None => {
            let mut graph = runtime.graph.borrow_mut();
            graph
                .session
                .update_viewport(|viewport| viewport.pan_by_view(event.delta));
            graph.rebuild_computed();
        }
    }
    refresh_graph_view(ui, graph_host, sidebar, runtime);
    event.mark_handled();
    event.stop_propagation();
}

fn zoom_graph_view(
    ui: &mut Ui,
    event: ScrollEvent<'_>,
    graph_host: ElementId,
    sidebar: ElementId,
    runtime: Rc<GraphDemoRuntime>,
) {
    if !matches!(event.phase, Phase::Capture | Phase::Target) {
        return;
    }
    if runtime.active_gesture.get().is_some() {
        return;
    }
    let delta = event.logical_delta(Vec2::new(48.0, 48.0), Vec2::new(260.0, 260.0));
    let amount = if delta.y.abs() >= delta.x.abs() {
        delta.y
    } else {
        delta.x
    };
    if amount.abs() <= 0.01 {
        return;
    }

    let anchor = local_point(ui, graph_host, event.position);
    let factor = (-amount * ZOOM_SENSITIVITY).exp().clamp(0.2, 5.0);
    {
        let mut graph = runtime.graph.borrow_mut();
        graph
            .session
            .update_viewport(|viewport| viewport.zoom_about_view_point(anchor, factor));
        graph.rebuild_computed();
    }
    refresh_graph_view(ui, graph_host, sidebar, runtime);
    event.mark_handled();
    event.stop_propagation();
}

fn move_node_by_view_delta(graph: &mut DemoGraph, node: NodeId, delta: Vec2) {
    let scale = graph.session.viewport().world_units_per_pixel();
    graph.projection.translate_node(node, delta * scale);
}

fn finish_graph_gesture(
    ui: &mut Ui,
    event: PointerRoutedEvent<'_>,
    graph_host: ElementId,
    sidebar: ElementId,
    runtime: Rc<GraphDemoRuntime>,
) {
    if !matches!(event.phase, Phase::Target) {
        return;
    }
    let Some(gesture) = runtime.active_gesture.replace(None) else {
        return;
    };

    match gesture {
        GraphGesture::CreateEdge(from) => {
            let mut graph = runtime.graph.borrow_mut();
            if let Some(position) = event.position {
                let point = local_point(ui, graph_host, position);
                let target = graph.computed.hit_test_view(
                    &graph.doc,
                    &graph.projection,
                    &graph.session,
                    point,
                );
                if let Some(HitTarget::Port(to)) = target
                    && from != to
                {
                    let _ = graph
                        .doc
                        .add_edge_with(from, to, (), &RejectDuplicateConnections);
                }
            }
            graph.session.set_hover(None);
            graph.session.set_interaction(InteractionState::Idle);
            graph.rebuild_computed();
        }
        GraphGesture::DragNode(_) => {
            let mut graph = runtime.graph.borrow_mut();
            graph.session.set_hover(None);
            graph.session.set_interaction(InteractionState::Idle);
            graph.rebuild_computed();
        }
    }

    refresh_graph_view(ui, graph_host, sidebar, runtime);
    event.mark_handled();
}

fn cancel_graph_gesture(
    ui: &mut Ui,
    event: PointerRoutedEvent<'_>,
    graph_host: ElementId,
    sidebar: ElementId,
    runtime: Rc<GraphDemoRuntime>,
) {
    if !matches!(event.phase, Phase::Target) {
        return;
    }
    if runtime.active_gesture.replace(None).is_none() {
        return;
    }
    {
        let mut graph = runtime.graph.borrow_mut();
        graph.session.set_hover(None);
        graph.session.set_interaction(InteractionState::Idle);
        graph.rebuild_computed();
    }
    refresh_graph_view(ui, graph_host, sidebar, runtime);
    event.mark_handled();
}

fn local_point(ui: &Ui, parent: ElementId, position: Point) -> Point {
    let bounds = ui.effective_bounds(parent);
    Point::new(position.x - bounds.x0, position.y - bounds.y0)
}

fn append_grid(ui: &mut Ui, parent: ElementId, graph: &DemoGraph) {
    let viewport = graph.session.viewport();
    let visible = graph.session.visible_world_rect();
    let spacing = viewport.suggest_grid_spacing(48.0).max(1.0);
    append_grid_axis(ui, parent, graph, visible.x0, visible.x1, spacing, true);
    append_grid_axis(ui, parent, graph, visible.y0, visible.y1, spacing, false);
}

fn append_grid_axis(
    ui: &mut Ui,
    parent: ElementId,
    graph: &DemoGraph,
    start: f64,
    end: f64,
    spacing: f64,
    vertical: bool,
) {
    let mut world = (start / spacing).floor() * spacing;
    let mut count = 0;
    while world <= end && count < GRID_LINE_LIMIT {
        let index = (world / spacing).round();
        let major = ((index / 5.0) - (index / 5.0).round()).abs() <= 0.001;
        let color = if major {
            Color::from_rgba8(0x46, 0x48, 0x58, 0x92)
        } else {
            Color::from_rgba8(0x31, 0x31, 0x3d, 0x72)
        };
        let view = if vertical {
            graph
                .session
                .viewport()
                .world_to_view_point(Point::new(world, 0.0))
                .x
        } else {
            graph
                .session
                .viewport()
                .world_to_view_point(Point::new(0.0, world))
                .y
        };
        let rect = if vertical {
            Rect::new(view, GRAPH_VIEW.y0, view + 1.0, GRAPH_VIEW.y1)
        } else {
            Rect::new(GRAPH_VIEW.x0, view, GRAPH_VIEW.x1, view + 1.0)
        };
        append_rect(ui, parent, rect, color, 0.0);
        world += spacing;
        count += 1;
    }
}

fn append_graph_edges(ui: &mut Ui, parent: ElementId, graph: &DemoGraph) {
    for edge in graph.computed.visible_edges() {
        let Some(route) = graph.computed.edge_route(*edge) else {
            continue;
        };
        for (start, end) in route.segments() {
            let start = graph.session.viewport().world_to_view_point(start);
            let end = graph.session.viewport().world_to_view_point(end);
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
    sidebar: ElementId,
    runtime: Rc<GraphDemoRuntime>,
) {
    let nodes = {
        let graph = runtime.graph.borrow();
        graph.computed.visible_nodes().to_vec()
    };
    for node in nodes {
        let (rect, meta, selected) = {
            let graph = runtime.graph.borrow();
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
            Panel::column(8.0).padding_all(14.0).layout(
                Layout::column(8.0)
                    .absolute_at(f32_coord(rect.x0), f32_coord(rect.y0))
                    .logical_size(f32_coord(rect.width()), f32_coord(rect.height())),
            ),
        );
        style_node(ui, node_element, meta.accent, selected);
        if selected {
            runtime.selected_element.set(Some(node_element));
        }
        ui.set_tooltip(
            node_element,
            Tooltip::new(format!("GraphDoc node: {}", meta.title)),
        );
        let runtime_for_handler = Rc::clone(&runtime);
        ui.on_pointer(node_element, PointerEventKind::Down, move |ui, event| {
            start_node_drag(
                ui,
                event,
                parent,
                sidebar,
                node,
                meta.accent,
                Rc::clone(&runtime_for_handler),
            );
        });
        let runtime_for_handler = Rc::clone(&runtime);
        ui.on_click(node_element, move |ui, event| {
            select_node(
                ui,
                event,
                node,
                meta.accent,
                sidebar,
                Rc::clone(&runtime_for_handler),
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

fn append_graph_ports(
    ui: &mut Ui,
    parent: ElementId,
    sidebar: ElementId,
    runtime: Rc<GraphDemoRuntime>,
) {
    let ports = {
        let graph = runtime.graph.borrow();
        graph
            .doc
            .iter_ports()
            .map(|(port, data)| (port, data.direction, data.meta))
            .collect::<Vec<_>>()
    };
    for (port, direction, meta) in ports {
        let view = {
            let graph = runtime.graph.borrow();
            let Some(anchor) = graph.computed.port_anchor(port) else {
                continue;
            };
            graph.session.viewport().world_to_view_point(anchor)
        };
        let color = meta.kind.color();
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
            Tooltip::new(format!("{} {}", meta.kind, meta.label)),
        );
        if matches!(direction, PortDirection::Output) {
            let runtime_for_handler = Rc::clone(&runtime);
            ui.on_pointer(port_element, PointerEventKind::Down, move |ui, event| {
                start_port_drag(
                    ui,
                    event,
                    parent,
                    sidebar,
                    port,
                    Rc::clone(&runtime_for_handler),
                );
            });
        }
    }
}

fn append_graph_preview(ui: &mut Ui, parent: ElementId, graph: &DemoGraph) {
    let Some(preview) = graph.computed.preview() else {
        return;
    };
    let color = match preview.target {
        EdgePreviewTarget::Port {
            compatible: true, ..
        } => Color::from_rgba8(0x70, 0xe2, 0x9a, 0xff),
        EdgePreviewTarget::Port {
            compatible: false, ..
        } => Color::from_rgba8(0xff, 0x66, 0x78, 0xff),
        EdgePreviewTarget::Pointer(_) => Color::from_rgba8(0xf2, 0xf4, 0xff, 0xc8),
    };
    for (start, end) in preview.route.segments() {
        let start = graph.session.viewport().world_to_view_point(start);
        let end = graph.session.viewport().world_to_view_point(end);
        if (start - end).hypot() <= 0.5 {
            continue;
        }
        append_edge_segment_with_color(ui, parent, start, end, color);
    }
}

fn start_node_drag(
    ui: &mut Ui,
    event: PointerRoutedEvent<'_>,
    graph_host: ElementId,
    sidebar: ElementId,
    node: NodeId,
    accent: Color,
    runtime: Rc<GraphDemoRuntime>,
) {
    if !matches!(event.phase, Phase::Target | Phase::Bubble) {
        return;
    }
    let Some(position) = event.position else {
        return;
    };
    runtime
        .active_gesture
        .set(Some(GraphGesture::DragNode(node)));
    let _ = ui.capture_pointer(graph_host);

    let previous_node = runtime.graph.borrow().selected;
    if let Some(previous) = runtime.selected_element.replace(Some(event.current))
        && ui.get(previous).is_some()
        && let Some(previous_data) = runtime.graph.borrow().doc.node(previous_node)
    {
        style_node(ui, previous, previous_data.meta.accent, false);
    }
    {
        let mut graph = runtime.graph.borrow_mut();
        graph.selected = node;
        graph.session.update_selection(|selection| {
            let _ = selection.select_only(GraphElementId::Node(node));
        });
        graph.session.set_interaction(InteractionState::DragNode {
            node,
            pointer_origin: local_point(ui, graph_host, position),
        });
        refresh_inspector(ui, sidebar, &graph);
    }
    style_node(ui, event.current, accent, true);
}

fn start_port_drag(
    ui: &mut Ui,
    event: PointerRoutedEvent<'_>,
    graph_host: ElementId,
    sidebar: ElementId,
    port: PortId,
    runtime: Rc<GraphDemoRuntime>,
) {
    if !matches!(event.phase, Phase::Target) {
        return;
    }
    let Some(position) = event.position else {
        return;
    };
    runtime
        .active_gesture
        .set(Some(GraphGesture::CreateEdge(port)));
    let _ = ui.capture_pointer(graph_host);
    {
        let mut graph = runtime.graph.borrow_mut();
        graph.session.set_interaction(InteractionState::CreateEdge {
            from: port,
            pointer: local_point(ui, graph_host, position),
        });
        graph.rebuild_computed();
    }
    refresh_graph_view(ui, graph_host, sidebar, runtime);
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
    let selected = graph
        .doc
        .node(graph.selected)
        .map(|node| node.meta.title)
        .unwrap_or("none");
    for line in [
        format!("selected: {selected}"),
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
    sidebar: ElementId,
    runtime: Rc<GraphDemoRuntime>,
) {
    if !matches!(event.phase, Phase::Target | Phase::Bubble) {
        return;
    }
    let previous_node = runtime.graph.borrow().selected;
    if let Some(previous) = runtime.selected_element.replace(Some(event.current))
        && ui.get(previous).is_some()
        && let Some(previous_data) = runtime.graph.borrow().doc.node(previous_node)
    {
        style_node(ui, previous, previous_data.meta.accent, false);
    }
    {
        let mut graph = runtime.graph.borrow_mut();
        graph.selected = node;
        graph.session.update_selection(|selection| {
            let _ = selection.select_only(GraphElementId::Node(node));
        });
        refresh_inspector(ui, sidebar, &graph);
    }
    style_node(ui, event.current, accent, true);
    event.mark_handled();
}

fn append_edge_segment(ui: &mut Ui, parent: ElementId, start: Point, end: Point) {
    append_edge_segment_with_color(
        ui,
        parent,
        start,
        end,
        Color::from_rgba8(0x62, 0xd8, 0xff, 0xff),
    );
}

fn append_edge_segment_with_color(
    ui: &mut Ui,
    parent: ElementId,
    start: Point,
    end: Point,
    color: Color,
) {
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
    append_rect(ui, parent, rect, color, EDGE_THICKNESS * 0.5);
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
    let surface = ui.properties().surface;
    if let Some(background) = background {
        ui.set_local(element, surface.background, Some(background.into()));
    }
    let border = border.map(Brush::from);
    ui.set_local(element, surface.border_brushes.top, border.clone());
    ui.set_local(element, surface.border_brushes.right, border.clone());
    ui.set_local(element, surface.border_brushes.bottom, border.clone());
    ui.set_local(element, surface.border_brushes.left, border);
    ui.set_local(element, surface.border_widths.top, border_width);
    ui.set_local(element, surface.border_widths.right, border_width);
    ui.set_local(element, surface.border_widths.bottom, border_width);
    ui.set_local(element, surface.border_widths.left, border_width);

    let radius = overstory::CornerRadius::circular(radius);
    ui.set_local(element, surface.corner_radii.top_left, radius);
    ui.set_local(element, surface.corner_radii.top_right, radius);
    ui.set_local(element, surface.corner_radii.bottom_right, radius);
    ui.set_local(element, surface.corner_radii.bottom_left, radius);
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
