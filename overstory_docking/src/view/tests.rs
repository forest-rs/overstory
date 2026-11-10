// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained docking presentation regression tests.

use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use overstory::{Layout, Panel, TabKey, TextResources, Ui, Visibility, select_tab_key};
use understory_tiling::{
    Axis, DropTargetId, LayoutInput, PaneId, Placement, Point, Rect, Size, TileOp, TileSpec,
    TileTree,
};

use super::{DockDragPreview, DockSurface};
use crate::DockState;

fn state() -> DockState {
    let mut tree = TileTree::from_spec(TileSpec::tabs([PaneId(1), PaneId(2)]))
        .expect("sample tabs should be valid");
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(3),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    DockState::new(
        tree,
        LayoutInput {
            bounds: Rect::new(0.0, 0.0, 600.0, 400.0),
            tab_bar_thickness: 28.0,
            split_handle_thickness: 6.0,
            min_pane_size: Size::new(80.0, 80.0),
        },
        PaneId(1),
    )
}

fn mount(ui: &mut Ui) -> DockSurface {
    let root = ui.root();
    ui.set_layout(root, Layout::new().logical_size(600.0, 400.0));
    ui.with_children(root, |ui| {
        DockSurface::mount(ui, Layout::new().logical_size(600.0, 400.0))
    })
}

#[test]
fn pane_realization_is_keyed_across_tabs_zoom_splits_and_removal() {
    let mut ui = Ui::with_text_resources(&TextResources::empty());
    let surface = mount(&mut ui);
    let mut state = state();
    let mut built = Vec::new();

    surface.sync(
        &mut ui,
        &state,
        |pane, ui| {
            built.push(pane);
            ui.add(Panel::new());
        },
        |_| String::from("Pane"),
    );

    assert_eq!(surface.realized_pane_count(), 3);
    assert!(
        built.contains(&PaneId(2)),
        "inactive tab content is retained"
    );
    let pane_one = surface.pane_element(PaneId(1)).unwrap();
    let pane_two = surface.pane_element(PaneId(2)).unwrap();

    state
        .apply_op(TileOp::ActivatePane { pane: PaneId(2) })
        .unwrap();
    surface.sync(
        &mut ui,
        &state,
        |_, _| panic!("tab activation must not rebuild pane content"),
        |_| String::from("Pane"),
    );
    assert_eq!(surface.pane_element(PaneId(1)), Some(pane_one));
    assert_eq!(surface.pane_element(PaneId(2)), Some(pane_two));

    state.set_selected(PaneId(2));
    state.zoom_selected();
    surface.sync(
        &mut ui,
        &state,
        |_, _| panic!("zoom must not rebuild pane content"),
        |_| String::from("Pane"),
    );
    assert_eq!(surface.realized_pane_count(), 3);
    assert_eq!(ui.visibility(pane_one), Some(Visibility::Collapsed));
    assert_eq!(ui.visibility(pane_two), Some(Visibility::Visible));
    state.unzoom();

    state
        .split_selected(Axis::Vertical, PaneId(4), 0.4)
        .unwrap();
    surface.sync(
        &mut ui,
        &state,
        |_, ui| {
            ui.add(Panel::new());
        },
        |_| String::from("Pane"),
    );
    assert_eq!(surface.realized_pane_count(), 4);
    assert_eq!(surface.pane_element(PaneId(1)), Some(pane_one));

    state
        .apply_op(TileOp::ClosePane { pane: PaneId(2) })
        .unwrap();
    surface.sync(
        &mut ui,
        &state,
        |_, _| panic!("closing a pane must not build content"),
        |_| String::from("Pane"),
    );
    assert_eq!(surface.realized_pane_count(), 3);
    assert!(surface.pane_element(PaneId(2)).is_none());
    assert!(
        ui.get(pane_two).is_none(),
        "removed pane subtree is cleaned up"
    );
}

#[test]
fn preview_preference_can_change_during_a_tab_insertion() {
    let mut ui = Ui::with_text_resources(&TextResources::empty());
    let surface = mount(&mut ui);
    let mut state = state();
    let build_pane = |_, _: &mut overstory::ChildUi<'_>| {};
    let title = |_| String::from("Pane");
    surface.sync(&mut ui, &state, build_pane, title);
    ui.compute_layout(Size::new(600.0, 400.0));

    let pane_one = surface.pane_element(PaneId(1)).unwrap();
    let pane_three = surface.pane_element(PaneId(3)).unwrap();
    let original_bounds = ui.effective_bounds(pane_three);
    let revision = state.tree().revision();
    let start = state
        .frame()
        .panes()
        .iter()
        .find(|pane| pane.pane == PaneId(3))
        .unwrap()
        .rect
        .center();
    let end = Point::new(20.0, 14.0);
    state.begin_pointer(start);
    state.update_pointer(end);
    assert!(matches!(
        state.overlay().active_target,
        Some(DropTargetId::TabBar(_))
    ));
    let overlay_layer = surface.inner.borrow().overlay_layer;

    surface.sync_with_drag_preview(&mut ui, &state, DockDragPreview::Outline, build_pane, title);
    ui.compute_layout(Size::new(600.0, 400.0));
    assert_eq!(ui.effective_bounds(pane_three), original_bounds);
    assert_eq!(ui.visibility(pane_one), Some(Visibility::Visible));
    assert_eq!(ui.get(overlay_layer).unwrap().children().len(), 3);

    surface.sync_with_drag_preview(
        &mut ui,
        &state,
        DockDragPreview::FullLayout,
        build_pane,
        title,
    );
    ui.compute_layout(Size::new(600.0, 400.0));
    let proposed_bounds = ui.effective_bounds(pane_three);
    assert!(proposed_bounds.width() > original_bounds.width());
    assert_eq!(ui.visibility(pane_one), Some(Visibility::Collapsed));
    assert_eq!(ui.get(overlay_layer).unwrap().children().len(), 1);
    assert_eq!(state.overlay().ghost_rects.len(), 1);
    assert_eq!(state.tree().revision(), revision);

    surface.sync_with_drag_preview(&mut ui, &state, DockDragPreview::Outline, build_pane, title);
    ui.compute_layout(Size::new(600.0, 400.0));
    assert_eq!(ui.effective_bounds(pane_three), original_bounds);
    assert_eq!(ui.get(overlay_layer).unwrap().children().len(), 3);

    state.end_pointer(end);
    surface.sync_with_drag_preview(&mut ui, &state, DockDragPreview::Outline, build_pane, title);
    ui.compute_layout(Size::new(600.0, 400.0));
    assert_eq!(surface.pane_element(PaneId(3)), Some(pane_three));
    assert_eq!(ui.effective_bounds(pane_three), proposed_bounds);
}

#[test]
fn passive_layers_do_not_cover_realized_pane_content() {
    let mut ui = Ui::with_text_resources(&TextResources::empty());
    let surface = mount(&mut ui);
    let state = state();
    surface.sync(&mut ui, &state, |_, _| {}, |_| String::from("Pane"));
    ui.compute_layout(Size::new(600.0, 400.0));
    let pane = state
        .frame()
        .panes()
        .iter()
        .find(|frame| frame.pane == PaneId(3))
        .unwrap();

    assert_eq!(
        ui.hit_test(pane.rect.center()),
        surface.pane_element(PaneId(3))
    );
}

#[test]
fn retained_tab_bounds_become_authoritative_tiling_geometry() {
    let mut ui = Ui::with_text_resources(&TextResources::empty());
    let surface = mount(&mut ui);
    let mut state = state();
    surface.sync(
        &mut ui,
        &state,
        |_, _| {},
        |pane| match pane {
            PaneId(1) => String::from("A very wide editor tab"),
            _ => String::from("Tab"),
        },
    );
    let bar = surface
        .inner
        .borrow()
        .tab_bars
        .iter()
        .find(|view| overstory::tab_bar_options(&ui, view.body).is_some_and(|tabs| tabs.len() == 2))
        .unwrap()
        .body;
    let items = overstory::tab_bar_layout_snapshot(&ui, bar).unwrap().items;
    ui.set_layout(items[0].element, Layout::new().logical_size(120.0, 20.0));
    ui.set_layout(items[1].element, Layout::new().logical_size(40.0, 20.0));
    ui.compute_layout(Size::new(600.0, 400.0));

    let geometry = surface.tab_group_geometry(&ui, &state).unwrap();
    assert!(!geometry.is_empty());
    let measured_group = geometry.iter().find(|group| group.tabs.len() == 2).unwrap();
    assert!(
        measured_group.tabs[0].rect.width() > measured_group.tabs[1].rect.width(),
        "retained text measurement should replace equal provisional tab widths: {:?}",
        measured_group.tabs
    );
    surface.apply_tab_geometry(&ui, &mut state).unwrap();

    for group in geometry {
        for tab in group.tabs {
            let resolved = state
                .frame()
                .tabs()
                .iter()
                .find(|candidate| candidate.group == group.group && candidate.pane == tab.pane)
                .unwrap();
            assert_eq!(resolved.rect, tab.rect);
        }
    }
}

#[test]
fn geometry_capture_rejects_a_state_relayout_after_surface_sync() {
    let mut ui = Ui::with_text_resources(&TextResources::empty());
    let surface = mount(&mut ui);
    let mut state = state();
    surface.sync(&mut ui, &state, |_, _| {}, |_| String::from("Pane"));
    ui.compute_layout(Size::new(600.0, 400.0));
    state.set_layout_input(LayoutInput {
        bounds: Rect::new(0.0, 0.0, 640.0, 400.0),
        ..state.layout_input()
    });

    assert!(matches!(
        surface.tab_group_geometry(&ui, &state),
        Err(super::DockGeometryError::StaleFrame { .. })
    ));
}

#[test]
fn geometry_capture_requires_arranged_ui_layout() {
    let mut ui = Ui::with_text_resources(&TextResources::empty());
    let surface = mount(&mut ui);
    let state = state();
    surface.sync(&mut ui, &state, |_, _| {}, |_| String::from("Pane"));

    assert_eq!(
        surface.tab_group_geometry(&ui, &state),
        Err(super::DockGeometryError::PendingLayout)
    );
}

#[test]
fn geometry_capture_reports_a_missing_realized_group() {
    let mut ui = Ui::with_text_resources(&TextResources::empty());
    let surface = mount(&mut ui);
    let state = state();
    surface.sync(&mut ui, &state, |_, _| {}, |_| String::from("Pane"));
    ui.compute_layout(Size::new(600.0, 400.0));
    let missing = surface.inner.borrow_mut().tab_bars.pop().unwrap().group;

    assert_eq!(
        surface.tab_group_geometry(&ui, &state),
        Err(super::DockGeometryError::MissingTabBar { group: missing })
    );
}

#[test]
fn overflow_geometry_remains_complete_for_offscreen_tabs() {
    let mut ui = Ui::with_text_resources(&TextResources::empty());
    let surface = mount(&mut ui);
    let tree = TileTree::from_spec(TileSpec::tabs((1_u32..=8).map(PaneId)))
        .expect("sample tabs should be valid");
    let state = DockState::new(
        tree,
        LayoutInput {
            bounds: Rect::new(0.0, 0.0, 100.0, 200.0),
            ..state().layout_input()
        },
        PaneId(1),
    );
    surface.sync(
        &mut ui,
        &state,
        |_, _| {},
        |pane| format!("A long tab title {}", pane.0),
    );
    let bar = surface.inner.borrow().tab_bars[0].body;
    for item in overstory::tab_bar_layout_snapshot(&ui, bar).unwrap().items {
        ui.set_layout(item.element, Layout::new().min_logical_size(80.0, 20.0));
    }
    ui.compute_layout(Size::new(600.0, 400.0));

    let geometry = surface.tab_group_geometry(&ui, &state).unwrap();

    assert_eq!(geometry[0].tabs.len(), 8);
    assert!(
        geometry[0]
            .tabs
            .iter()
            .any(|tab| tab.interaction_regions.is_empty()),
        "{:#?}",
        geometry[0]
    );
}

#[test]
fn user_tab_selection_bridges_to_application_pane_identity() {
    let mut ui = Ui::with_text_resources(&TextResources::empty());
    let surface = mount(&mut ui);
    let state = Rc::new(RefCell::new(state()));
    surface.sync(
        &mut ui,
        &state.borrow(),
        |_, _| {},
        |_| String::from("Pane"),
    );
    let event_state = Rc::clone(&state);
    let event_surface = surface.clone();
    surface.set_tab_activated_handler(move |ui, pane| {
        event_state.borrow_mut().activate_pane(pane).unwrap();
        event_surface.sync(
            ui,
            &event_state.borrow(),
            |_, _| panic!("retained pane contents must not rebuild"),
            |pane| format!("Pane {}", pane.0),
        );
    });
    let bar = surface.inner.borrow().tab_bars[0].body;

    assert!(select_tab_key(&mut ui, bar, Some(TabKey::new(2))));

    assert_eq!(state.borrow().selected(), PaneId(2));
    assert_eq!(
        state
            .borrow()
            .frame()
            .tab_bars()
            .iter()
            .find_map(|bar| bar.active_pane),
        Some(PaneId(2))
    );
}
