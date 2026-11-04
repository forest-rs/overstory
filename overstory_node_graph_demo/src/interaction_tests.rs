// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Exercise actual event dispatch through the retained native tree.

use super::*;
use ui_events::pointer::{
    PointerButton, PointerButtonEvent, PointerButtons, PointerEvent, PointerId, PointerInfo,
    PointerState, PointerType, PointerUpdate,
};

fn pointer() -> PointerInfo {
    PointerInfo {
        pointer_id: Some(PointerId::PRIMARY),
        persistent_device_id: None,
        pointer_type: PointerType::Mouse,
    }
}
fn state(point: Point, time: u64, pressed: bool) -> PointerState {
    let mut buttons = PointerButtons::new();
    if pressed {
        buttons.insert(PointerButton::Primary);
    }
    let mut state = PointerState {
        time,
        buttons,
        count: 1,
        ..PointerState::default()
    };
    state.position.x = point.x;
    state.position.y = point.y;
    state
}
fn down(point: Point) -> PointerEvent {
    PointerEvent::Down(PointerButtonEvent {
        pointer: pointer(),
        button: Some(PointerButton::Primary),
        state: state(point, 1, true),
    })
}
fn moved(point: Point) -> PointerEvent {
    PointerEvent::Move(PointerUpdate {
        pointer: pointer(),
        current: state(point, 2, true),
        coalesced: Vec::new(),
        predicted: Vec::new(),
    })
}
fn up(point: Point) -> PointerEvent {
    PointerEvent::Up(PointerButtonEvent {
        pointer: pointer(),
        button: Some(PointerButton::Primary),
        state: state(point, 3, false),
    })
}

#[test]
fn clicking_a_parameter_name_focuses_its_control_without_starting_a_graph_drag() {
    let (mut ui, runtime, _, _) = build_study_ui();
    ui.compute_layout(Size::new(1480., 820.));
    let (name, slider) = {
        let parts = runtime.parameter_ui.borrow();
        let parts = parts.as_ref().unwrap();
        (parts.names[1], parts.sliders[1])
    };
    let point = ui.effective_bounds(name).center();
    ui.dispatch_pointer_event(&down(point));
    ui.dispatch_pointer_event(&up(point));
    assert_eq!(ui.focused(), Some(slider));
    assert!(runtime.active_gesture.get().is_none());
}

#[test]
fn background_pan_keeps_capture_while_decorative_geometry_is_updated() {
    let (mut ui, runtime, host, _) = build_study_ui();
    ui.compute_layout(Size::new(1480., 820.));
    let point = ui.effective_bounds(host).origin() + Vec2::new(20., 20.);
    let before = runtime.graph.borrow().session.visible_world_rect();
    ui.dispatch_pointer_event(&down(point));
    assert_eq!(ui.pointer_capture(), Some(host));
    ui.dispatch_pointer_event(&moved(point + Vec2::new(40., 20.)));
    let first = runtime.graph.borrow().session.visible_world_rect();
    assert_ne!(first, before);
    assert_eq!(ui.pointer_capture(), Some(host));
    ui.dispatch_pointer_event(&moved(point + Vec2::new(80., 40.)));
    assert_ne!(runtime.graph.borrow().session.visible_world_rect(), first);
    assert_eq!(ui.pointer_capture(), Some(host));
    ui.dispatch_pointer_event(&up(point + Vec2::new(80., 40.)));
    assert_eq!(ui.pointer_capture(), None);
}

#[test]
fn slider_drag_changes_the_case_plan_without_panning_or_replacing_controls() {
    let (mut ui, runtime, _, _) = build_study_ui();
    ui.compute_layout(Size::new(1480., 820.));
    let sliders = runtime.parameter_ui.borrow().as_ref().unwrap().sliders;
    let rect = ui.effective_bounds(sliders[0]);
    let camera = runtime.graph.borrow().session.visible_world_rect();
    let point = Point::new(rect.x0 + rect.width() * 0.5, rect.center().y);
    ui.dispatch_pointer_event(&down(point));
    let end = Point::new(rect.x0 + rect.width() * 0.89, point.y);
    ui.dispatch_pointer_event(&moved(end));
    ui.dispatch_pointer_event(&up(end));
    let accepted = runtime.graph.borrow().parameters.value(Parameter::Columns);
    assert_eq!(accepted, 6.);
    assert_eq!(
        ui.resolved_value(sliders[0], ui.properties().value)
            .unwrap(),
        accepted
    );
    assert_eq!(runtime.graph.borrow().session.visible_world_rect(), camera);
    assert_eq!(
        runtime.parameter_ui.borrow().as_ref().unwrap().sliders,
        sliders
    );
    assert_eq!(runtime.graph.borrow().work.routes, 0);
    assert_eq!(runtime.view.borrow().work, ViewWork::default());
    assert!(runtime.active_gesture.get().is_none());
    assert_eq!(ui.pointer_capture(), None);
}

#[test]
fn body_drag_moves_only_the_selected_node_and_releases_capture() {
    let (mut ui, runtime, host, _) = build_study_ui();
    ui.compute_layout(Size::new(1480., 820.));
    let (node, origin, point, scale) = {
        let graph = runtime.graph.borrow();
        let node = graph.selected;
        let world = graph.computed.node_bounds(node).unwrap();
        let position = graph.session.viewport().world_to_view_point(world.origin());
        (
            node,
            world.origin(),
            ui.effective_bounds(host).origin() + position.to_vec2() + Vec2::new(30., 22.),
            graph.session.viewport().world_units_per_pixel(),
        )
    };
    let camera = runtime.graph.borrow().session.visible_world_rect();
    ui.dispatch_pointer_event(&down(point));
    assert_eq!(
        runtime.active_gesture.get(),
        Some(GraphGesture::DragNode(node))
    );
    let end = point + Vec2::new(32., 16.);
    assert_eq!(ui.pointer_capture(), Some(host), "capture after down");
    ui.dispatch_pointer_event(&moved(end));
    assert_eq!(ui.pointer_capture(), Some(host), "capture after move");
    assert_eq!(
        runtime
            .graph
            .borrow()
            .computed
            .node_bounds(node)
            .unwrap()
            .origin(),
        origin + Vec2::new(32., 16.) * scale
    );
    assert_eq!(runtime.graph.borrow().work.nodes, 1);
    assert_eq!(runtime.graph.borrow().work.routes, 1);
    assert_eq!(
        runtime.view.borrow().work,
        ViewWork {
            nodes: 1,
            ports: 1,
            edges: 1,
            ..ViewWork::default()
        }
    );
    assert_eq!(runtime.graph.borrow().session.visible_world_rect(), camera);
    ui.dispatch_pointer_event(&up(end));
    assert!(runtime.active_gesture.get().is_none());
    assert_eq!(ui.pointer_capture(), None);
}

#[test]
fn tie_drag_guides_only_its_members_without_becoming_a_node_drag() {
    let (mut ui, runtime, host, _) = build_study_ui();
    ui.compute_layout(Size::new(1480., 820.));
    let (center, point, scale) = {
        let graph = runtime.graph.borrow();
        (
            graph.tie.center,
            ui.effective_bounds(host).origin()
                + graph
                    .session
                    .viewport()
                    .world_to_view_point(graph.tie.center)
                    .to_vec2(),
            graph.session.viewport().world_units_per_pixel(),
        )
    };
    ui.dispatch_pointer_event(&down(point));
    assert_eq!(runtime.active_gesture.get(), Some(GraphGesture::MoveTie));
    ui.dispatch_pointer_event(&moved(point + Vec2::new(10., 24.)));
    assert_eq!(
        runtime.graph.borrow().tie.center,
        center + Vec2::new(10., 24.) * scale
    );
    assert_eq!(runtime.graph.borrow().work.routes, 3);
    assert_eq!(runtime.graph.borrow().work.nodes, 0);
    assert_eq!(
        runtime.view.borrow().work,
        ViewWork {
            edges: 3,
            ..ViewWork::default()
        }
    );
    ui.dispatch_pointer_event(&PointerEvent::Cancel(pointer()));
    assert!(runtime.active_gesture.get().is_none());
    assert_eq!(ui.pointer_capture(), None);
}
