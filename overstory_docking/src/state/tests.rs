// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Docking state-machine regression tests.

use alloc::vec;

use super::*;
use understory_tiling::{DockPolicy, Rect, Size, TabGeometry, TabGroupGeometry, TileSpec};

fn input() -> LayoutInput {
    LayoutInput {
        bounds: Rect::new(0.0, 0.0, 600.0, 400.0),
        tab_bar_thickness: 28.0,
        split_handle_thickness: 6.0,
        min_pane_size: Size::new(80.0, 80.0),
    }
}

fn sample_tree() -> TileTree {
    TileTree::from_spec(TileSpec::equal_split(
        Axis::Horizontal,
        [
            TileSpec::tabs([PaneId(1), PaneId(2)]),
            TileSpec::tabs([PaneId(3), PaneId(4)]),
        ],
    ))
    .expect("sample tab groups should be valid")
}

fn resolved_tab_geometry(state: &DockState) -> Vec<TabGroupGeometry> {
    state
        .frame()
        .tab_bars()
        .iter()
        .map(|bar| TabGroupGeometry {
            group: bar.group,
            viewport: bar.viewport,
            tabs: state
                .frame()
                .tabs()
                .iter()
                .filter(|tab| tab.group == bar.group)
                .map(|tab| TabGeometry {
                    pane: tab.pane,
                    rect: tab.rect,
                    interaction_regions: vec![tab.rect],
                })
                .collect(),
            bar_drag_regions: Vec::new(),
        })
        .collect()
}

#[test]
fn a_collapsed_strip_expands_and_a_bookmark_preserves_unrelated_edits() {
    let mut state = DockState::new(sample_tree(), input(), PaneId(1));
    let before = state.frame().panes()[0].rect;
    state.collapse_selected(40.0).unwrap();
    let strip = state.frame().collapsed_tiles()[0];
    state.begin_pointer(strip.rect.center());
    assert!(matches!(
        state.end_pointer(strip.rect.center()),
        DockOutcome::Committed(TileOp::SetCollapsed { extent: None, .. })
    ));
    assert_eq!(state.frame().panes()[0].rect, before);

    let bookmark = state.close_selected_with_bookmark().unwrap();
    state.activate_pane(PaneId(4)).unwrap();
    assert_eq!(state.reopen_pane(&bookmark), Ok(ReopenPlacement::Exact));
    assert_eq!(state.frame().panes()[0].pane, PaneId(1));
    assert_eq!(state.frame().panes()[1].pane, PaneId(4));
}

#[test]
fn clicking_tab_activates_without_drag() {
    let mut state = DockState::new(sample_tree(), input(), PaneId(1));
    let tab = state
        .frame()
        .tabs()
        .iter()
        .find(|tab| tab.pane == PaneId(2))
        .copied()
        .expect("sample should expose pane 2 as a tab");
    let revision = state.tree().revision();

    assert!(matches!(
        state.begin_pointer(tab.rect.center()),
        DockOutcome::InteractionStarted(DockInteractionKind::PendingDrag)
    ));
    assert!(matches!(
        state.end_pointer(tab.rect.center()),
        DockOutcome::Committed(TileOp::ActivatePane { pane }) if pane == PaneId(2)
    ));
    assert_ne!(state.tree().revision(), revision);
    assert_eq!(state.selected(), PaneId(2));
}

#[test]
fn resize_updates_preview_before_commit() {
    let mut state = DockState::new(sample_tree(), input(), PaneId(1));
    let handle = state
        .frame()
        .split_handles()
        .first()
        .copied()
        .expect("sample should include a split handle");
    let start = handle.rect.center();
    let end = match handle.axis {
        Axis::Horizontal => Point::new(start.x + 50.0, start.y),
        Axis::Vertical => Point::new(start.x, start.y + 50.0),
    };
    let revision = state.tree().revision();

    assert!(matches!(
        state.begin_pointer(start),
        DockOutcome::InteractionStarted(DockInteractionKind::Resize)
    ));
    state.update_pointer(end);

    assert_eq!(state.tree().revision(), revision);
    assert!(state.preview_frame().is_some());
    assert!(matches!(state.end_pointer(end), DockOutcome::Committed(_)));
    assert_ne!(state.tree().revision(), revision);
    assert!(state.preview_frame().is_none());
}

#[test]
fn policy_lock_rejects_interaction_commit() {
    let mut state = DockState::new(sample_tree(), input(), PaneId(1));
    state.set_policy_locked(true);
    let source = state.frame().panes()[0];
    let target = state.frame().panes()[1];

    state.begin_pointer(source.rect.center());
    state.update_pointer(Point::new(target.rect.x0 + 8.0, target.rect.center().y));

    assert!(matches!(
        state.end_pointer(Point::new(target.rect.x0 + 8.0, target.rect.center().y)),
        DockOutcome::Rejected(TileError::PolicyRejected)
    ));
    assert!(state.overlay().drop_targets.is_empty());
    assert!(state.preview_frame().is_none());
    assert!(matches!(state.interaction(), InteractionState::None));
}

#[test]
fn ending_drag_without_proposal_clears_transient_frames() {
    let mut state = DockState::new(sample_tree(), input(), PaneId(1));
    let source = state.frame().panes()[0];
    let outside = Point::new(-100.0, -100.0);

    state.begin_pointer(source.rect.center());
    state.update_pointer(outside);
    assert!(state.overlay().dragged.is_some());

    assert!(matches!(
        state.end_pointer(outside),
        DockOutcome::NoProposal(DockInteractionKind::Drag)
    ));
    assert!(state.overlay().drop_targets.is_empty());
    assert!(state.overlay().ghost_rects.is_empty());
    assert!(state.overlay().dragged.is_none());
    assert!(state.preview_frame().is_none());
    assert!(matches!(state.interaction(), InteractionState::None));
}

#[test]
fn split_selected_applies_requested_new_pane() {
    let mut state = DockState::new(sample_tree(), input(), PaneId(1));
    let op = state
        .split_selected(Axis::Vertical, PaneId(9), 0.4)
        .expect("split should commit");
    assert!(matches!(
        op,
        TileOp::SplitPane {
            new_pane: PaneId(9),
            ..
        }
    ));
    assert_eq!(state.selected(), PaneId(9));
}

#[test]
fn zoom_selected_projects_without_mutating_the_tree() {
    let mut state = DockState::new(sample_tree(), input(), PaneId(3));
    let revision = state.tree().revision();

    assert_eq!(state.zoom_selected(), PaneId(3));
    assert_eq!(state.zoomed_pane(), Some(PaneId(3)));
    assert_eq!(state.tree().revision(), revision);
    assert_eq!(state.frame().panes().len(), 1);
    assert_eq!(state.frame().panes()[0].pane, PaneId(3));
    assert_eq!(state.frame().panes()[0].rect, input().bounds);

    assert_eq!(state.unzoom(), Some(PaneId(3)));
    assert_eq!(state.zoomed_pane(), None);
    assert_eq!(state.tree().revision(), revision);
    assert!(state.frame().panes().len() > 1);
}

#[test]
fn selected_visible_pane_falls_back_from_an_inactive_tab() {
    let mut state = DockState::new(sample_tree(), input(), PaneId(2));

    assert_eq!(state.selected_visible_pane(), PaneId(1));
    assert_eq!(state.zoom_selected(), PaneId(1));
    assert_eq!(state.zoomed_pane(), Some(PaneId(1)));
}

#[test]
fn layout_changes_preserve_application_interaction_policy() {
    let mut options = InteractionOptions::from_layout_input(input());
    options.drag.allow_split = false;
    options.drag_threshold = 12.0;
    let mut state = DockState::with_options(
        sample_tree(),
        input(),
        options,
        DockPolicy::default(),
        PaneId(1),
    );
    let updated = LayoutInput {
        bounds: Rect::new(0.0, 0.0, 800.0, 500.0),
        min_pane_size: Size::new(120.0, 100.0),
        ..input()
    };

    state.set_layout_input(updated);

    let options = state.interaction_options();
    assert!(!options.drag.allow_split);
    assert_eq!(options.drag_threshold, 12.0);
    assert_eq!(options.resize.min_pane_size, updated.min_pane_size);
    assert_eq!(options.preview_layout.unwrap().bounds, updated.bounds);
}

#[test]
fn drag_target_uses_core_overlay_candidates() {
    let mut state = DockState::new(sample_tree(), input(), PaneId(1));
    let source = state.frame().panes()[0];
    let target = state.frame().panes()[1];

    state.begin_pointer(source.rect.center());
    let outcome = state.update_pointer(Point::new(target.rect.x0 + 8.0, target.rect.center().y));

    assert!(matches!(outcome, DockOutcome::ProposalValid(_)));
    assert!(state.overlay().drop_targets.len() <= 1);
    assert!(matches!(state.interaction(), InteractionState::Drag(_)));
    assert!(state.overlay().active_target.is_some());
}

#[test]
fn unchanged_resolved_geometry_preserves_but_changed_geometry_cancels_interaction() {
    let mut state = DockState::new(sample_tree(), input(), PaneId(1));
    let geometry = resolved_tab_geometry(&state);
    state.apply_tab_group_geometry(&geometry).unwrap();
    let handle = state.frame().split_handles()[0];
    state.begin_pointer(handle.rect.center());
    assert!(matches!(state.interaction(), InteractionState::Resize(_)));

    state.apply_tab_group_geometry(&geometry).unwrap();
    assert!(matches!(state.interaction(), InteractionState::Resize(_)));

    let mut changed = geometry;
    changed[0].tabs[0].rect.x1 -= 1.0;
    changed[0].tabs[0].interaction_regions[0].x1 -= 1.0;
    state.apply_tab_group_geometry(&changed).unwrap();
    assert!(matches!(state.interaction(), InteractionState::None));
}
