// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

use accesskit::{Action, ActionData, ActionRequest, Node, Orientation, Role, TreeId, TreeUpdate};
use kurbo::{Point, Size, Vec2};
use ui_events::ScrollDelta;

use super::ids::{DEFAULT_SCROLL_AREA_GAP, DEFAULT_SCROLL_BAR_THICKNESS, DEFAULT_SCROLL_LINE_Y};
use super::*;
use crate::accessibility::accesskit_node_id;
use crate::channels::ARRANGE;
use crate::element::ElementId;
use crate::layouts::{Layout, LayoutDirection};
use crate::scroll_runtime::ScrollInputSource;
use crate::test_support::{pointer_down, pointer_move, pointer_scroll, pointer_up};
use crate::ui::Ui;

#[test]
fn scroll_view_offsets_and_clips_children() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 100.0));
    let scroll = ui.append(root, ScrollView::new().size(120.0, 48.0));
    let first = append_scroll_test_child(&mut ui, scroll);
    append_scroll_test_child(&mut ui, scroll);
    append_scroll_test_child(&mut ui, scroll);

    ui.compute_layout(Size::new(180.0, 100.0));
    let before = ui.effective_bounds(first);
    assert!(set_scroll_view_offset_y(&mut ui, scroll, 12.0));
    ui.compute_layout(Size::new(180.0, 100.0));
    let after = ui.effective_bounds(first);
    assert!((after.y0 - (before.y0 - 12.0)).abs() <= 0.01);
    assert_eq!(scroll_view_offset_y(&ui, scroll), Some(12.0));
}

#[test]
fn horizontal_scroll_preserves_logical_position_across_direction_changes() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(0.0).logical_size(320.0, 100.0));
    let scroll = ui.append(root, ScrollView::new().size(100.0, 40.0));
    let content = ui.insert_child(scroll);
    ui.set_layout(content, Layout::new().logical_size(300.0, 24.0));

    ui.compute_layout(Size::new(320.0, 100.0));
    let ltr = ui
        .scroll_state_snapshot(scroll)
        .expect("scroll view should expose horizontal metrics");
    assert_eq!(ltr.max_offset.x, 200.0);
    assert_eq!(ltr.offset.x, 0.0);

    ui.set_layout_direction(scroll, LayoutDirection::Rtl);
    ui.compute_layout(Size::new(320.0, 100.0));
    let rtl = ui
        .scroll_state_snapshot(scroll)
        .expect("scroll state should survive the direction change");
    assert_eq!(rtl.max_offset.x, 200.0);
    assert_eq!(rtl.offset.x, 200.0);

    ui.set_layout(content, Layout::new().logical_size(400.0, 24.0));
    ui.compute_layout(Size::new(320.0, 100.0));
    let resized = ui.scroll_state_snapshot(scroll).unwrap();
    assert_eq!(resized.max_offset.x, 300.0);
    assert_eq!(
        resized.offset.x, 300.0,
        "content growth should preserve the RTL inline-start position"
    );
    let update = ui.accesskit_tree_update();
    let node = accesskit_node_for(&update, scroll);
    assert_eq!(
        node.text_direction(),
        Some(accesskit::TextDirection::RightToLeft)
    );
    assert_eq!(node.scroll_x(), Some(300.0));
    assert!(node.supports_action(Action::ScrollLeft));
    assert!(ui.handle_accesskit_action(&scroll_action(
        scroll,
        Action::ScrollLeft,
        accesskit::ScrollUnit::Item,
    )));
    let scrolled_rtl = ui.scroll_state_snapshot(scroll).unwrap();
    let logical_offset = scrolled_rtl.max_offset.x - scrolled_rtl.offset.x;
    assert!(logical_offset > 0.0);

    ui.set_layout_direction(scroll, LayoutDirection::Ltr);
    ui.compute_layout(Size::new(320.0, 100.0));
    let restored = ui.scroll_state_snapshot(scroll).unwrap();
    assert_eq!(restored.offset.x, logical_offset);
    assert!(ui.get(content).is_some());
}

#[test]
fn scroll_view_offset_reuses_layout_results() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 100.0));
    let scroll = ui.append(root, ScrollView::new().size(120.0, 48.0));
    let first = append_scroll_test_child(&mut ui, scroll);
    for _ in 0..3 {
        append_scroll_test_child(&mut ui, scroll);
    }

    ui.compute_layout(Size::new(180.0, 100.0));
    let before = ui.effective_bounds(first);
    ui.reset_frame_stats();

    assert!(set_scroll_view_offset_y(&mut ui, scroll, 12.0));
    assert!(!ui.invalidation().has_invalidated(crate::channels::LAYOUT));
    assert!(ui.invalidation().has_invalidated(ARRANGE));

    ui.compute_layout(Size::new(180.0, 100.0));
    let stats = ui.frame_stats();
    let after = ui.effective_bounds(first);

    assert_eq!(stats.layout_passes, 0);
    assert!(!stats.layout_requested);
    assert_eq!(stats.channel_marks.layout, 0);
    assert_eq!(stats.channel_marks.arrange, 1);
    assert!(stats.bounds_written > 0);
    assert!((after.y0 - (before.y0 - 12.0)).abs() <= 0.01);
}

#[test]
fn scroll_view_handles_routed_wheel_delta() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 120.0));
    let scroll = ui.append(root, ScrollView::new().size(120.0, 48.0));
    for _ in 0..4 {
        append_scroll_test_child(&mut ui, scroll);
    }

    ui.compute_layout(Size::new(180.0, 120.0));
    let result = ui.synthesize_scroll(scroll, Point::ZERO, ScrollDelta::LineDelta(0.0, -1.0));
    assert!(result.handled);
    assert!(scroll_view_offset_y(&ui, scroll).is_some_and(|offset| offset > 0.0));
}

#[test]
fn nested_scroll_views_translate_unconsumed_page_fraction() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 140.0));
    let outer = ui.append(root, ScrollView::new().size(120.0, 48.0));
    let inner = ui.append(outer, ScrollView::new().size(120.0, 24.0));
    for _ in 0..4 {
        append_scroll_test_child(&mut ui, inner);
        append_scroll_test_child(&mut ui, outer);
    }

    ui.compute_layout(Size::new(180.0, 140.0));
    let inner_snapshot = ui
        .scroll_state_snapshot(inner)
        .expect("inner viewport should expose scroll state");
    let outer_snapshot = ui
        .scroll_state_snapshot(outer)
        .expect("outer viewport should expose scroll state");
    let inner_max = inner_snapshot.max_offset.y;
    assert!(inner_max > 8.0);
    assert!(set_scroll_view_offset_y(&mut ui, inner, inner_max - 8.0));
    assert_eq!(scroll_view_offset_y(&ui, outer), Some(0.0));

    let result = ui.synthesize_scroll(inner, Point::ZERO, ScrollDelta::PageDelta(0.0, -1.0));

    let inner_remainder = inner_snapshot.viewport.height - 8.0;
    let outer_request =
        outer_snapshot.viewport.height * (inner_remainder / inner_snapshot.viewport.height);
    assert!(result.handled);
    assert_eq!(scroll_view_offset_y(&ui, inner), Some(inner_max));
    assert_eq!(scroll_view_offset_y(&ui, outer), Some(outer_request));
    let inner_change = ui
        .scroll_state_snapshot(inner)
        .and_then(|snapshot| snapshot.last_command)
        .expect("inner viewport should record its boundary hit");
    let outer_change = ui
        .scroll_state_snapshot(outer)
        .and_then(|snapshot| snapshot.last_command)
        .expect("outer viewport should receive the remainder");
    assert_eq!(inner_change.consumed_delta.y, 8.0);
    assert_eq!(inner_change.unconsumed_delta.y, inner_remainder);
    assert_eq!(outer_change.requested_delta.y, outer_request);
    assert_eq!(outer_change.consumed_delta.y, outer_request);
}

#[test]
fn root_scroll_boundary_leaves_the_unconsumed_delta_unhandled() {
    let mut ui = Ui::new();
    let root = ui.root();
    let scroll = ui.append(root, ScrollView::new().size(120.0, 48.0));
    for _ in 0..4 {
        append_scroll_test_child(&mut ui, scroll);
    }
    ui.compute_layout(Size::new(180.0, 120.0));
    let max = ui
        .scroll_state_snapshot(scroll)
        .expect("viewport should expose scroll state")
        .max_offset
        .y;
    assert!(set_scroll_view_offset_y(&mut ui, scroll, max));
    let recording = ui.begin_live_inspection_recording();
    ui.clear_inspection_changes(recording);

    let result = ui.synthesize_scroll(scroll, Point::ZERO, ScrollDelta::LineDelta(0.0, -1.0));

    assert!(!result.handled);
    let change = ui
        .scroll_state_snapshot(scroll)
        .and_then(|snapshot| snapshot.last_command)
        .expect("boundary command should remain inspectable");
    assert_eq!(change.consumed_delta, Vec2::ZERO);
    assert!(change.unconsumed_delta.y > 0.0);
    assert!(
        ui.drain_inspection_changes(recording)
            .expect("recording should be active")
            .tree
            .updated
            .iter()
            .any(|row| row.id == scroll
                && row
                    .scroll
                    .and_then(|scroll| scroll.last_command)
                    .is_some_and(|change| change.unconsumed_delta.y > 0.0))
    );
}

#[test]
fn pointer_wheel_scroll_uses_content_direction() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 120.0));
    let scroll = ui.append(root, ScrollView::new().size(120.0, 48.0));
    for _ in 0..4 {
        append_scroll_test_child(&mut ui, scroll);
    }

    ui.compute_layout(Size::new(180.0, 120.0));
    let position = ui.effective_bounds(scroll).center();
    let result = ui.dispatch_pointer_event(&pointer_scroll(
        position,
        1,
        ScrollDelta::LineDelta(0.0, -1.0),
    ));

    assert!(result.handled);
    assert!(scroll_view_offset_y(&ui, scroll).is_some_and(|offset| offset > 0.0));

    let before = scroll_view_offset_y(&ui, scroll).unwrap();
    let result = ui.dispatch_pointer_event(&pointer_scroll(
        position,
        2,
        ScrollDelta::LineDelta(0.0, 1.0),
    ));

    assert!(result.handled);
    assert!(scroll_view_offset_y(&ui, scroll).is_some_and(|offset| offset < before));
}

#[test]
fn non_finite_wheel_delta_is_ignored() {
    let mut ui = Ui::new();
    let root = ui.root();
    let scroll = ui.append(root, ScrollView::new().size(120.0, 48.0));
    for _ in 0..4 {
        append_scroll_test_child(&mut ui, scroll);
    }
    ui.compute_layout(Size::new(180.0, 120.0));
    assert!(set_scroll_view_offset_y(&mut ui, scroll, 12.0));
    let before = ui
        .scroll_state_snapshot(scroll)
        .expect("scroll view should expose state");

    let result = ui.synthesize_scroll(scroll, Point::ZERO, ScrollDelta::LineDelta(0.0, f32::NAN));
    let after = ui
        .scroll_state_snapshot(scroll)
        .expect("scroll view should retain state");

    assert!(!result.handled);
    assert_eq!(after.offset, before.offset);
    assert_eq!(after.command_count, before.command_count);
}

#[test]
fn scroll_state_tracks_viewport_and_content_metrics() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 120.0));
    let scroll = ui.append(root, ScrollView::new().size(120.0, 48.0));
    for _ in 0..4 {
        append_scroll_test_child(&mut ui, scroll);
    }

    ui.compute_layout(Size::new(180.0, 120.0));
    let state = ui
        .scroll_target(scroll)
        .map(|target| target.state())
        .expect("scroll view should own scroll state");

    assert_eq!(state.viewport_size(), Size::new(120.0, 48.0));
    let snapshot = state.snapshot();
    assert!(snapshot.content.height > snapshot.viewport.height);
    assert!(snapshot.max_offset.y > 0.0);
}

#[test]
fn removing_scroll_view_clears_scroll_target_registration() {
    let mut ui = Ui::new();
    let root = ui.root();
    let scroll = ui.append(root, ScrollView::new().size(120.0, 48.0));

    assert!(ui.scroll_target(scroll).is_some());

    ui.remove(scroll);

    assert!(ui.scroll_target(scroll).is_none());
}

#[test]
fn scroll_view_accessibility_exposes_offsets_and_handles_set_offset() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 120.0));
    let scroll = ui.append(root, ScrollView::new().size(120.0, 48.0));
    for _ in 0..4 {
        append_scroll_test_child(&mut ui, scroll);
    }

    ui.compute_layout(Size::new(180.0, 120.0));
    let update = ui.accesskit_tree_update();
    let node = accesskit_node_for(&update, scroll);
    let max = node
        .scroll_y_max()
        .expect("scroll view should expose max y");

    assert_eq!(node.role(), Role::ScrollView);
    assert_eq!(node.scroll_x(), Some(0.0));
    assert_eq!(node.scroll_y(), Some(0.0));
    assert_eq!(node.scroll_y_min(), Some(0.0));
    assert!(max > 0.0);
    assert!(node.supports_action(Action::ScrollDown));
    assert!(node.supports_action(Action::SetScrollOffset));

    let handled = ui.handle_accesskit_action(&scroll_offset_action(scroll, 0.0, max * 0.5));

    assert!(handled);
    assert!(scroll_view_offset_y(&ui, scroll).is_some_and(|offset| offset > 0.0));
}

#[test]
fn scroll_view_accessibility_scroll_actions_use_units() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 160.0));
    let scroll = ui.append(root, ScrollView::new().size(120.0, 48.0));
    for _ in 0..6 {
        append_scroll_test_child(&mut ui, scroll);
    }

    ui.compute_layout(Size::new(180.0, 160.0));

    assert!(ui.handle_accesskit_action(&scroll_action(
        scroll,
        Action::ScrollDown,
        accesskit::ScrollUnit::Page,
    )));
    assert!(
        scroll_view_offset_y(&ui, scroll).is_some_and(|offset| { (offset - 48.0).abs() <= 0.01 })
    );

    assert!(ui.handle_accesskit_action(&scroll_action(
        scroll,
        Action::ScrollUp,
        accesskit::ScrollUnit::Item,
    )));
    assert!(
        scroll_view_offset_y(&ui, scroll).is_some_and(|offset| { (offset - 12.0).abs() <= 0.01 })
    );
}

#[test]
fn scroll_area_creates_bound_viewport_and_vertical_bar() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(0.0).logical_size(160.0, 100.0));
    let area = ui.append(root, ScrollArea::new().size(120.0, 80.0));
    let viewport = scroll_area_viewport(&ui, area).expect("scroll area should expose viewport");
    let bar = scroll_area_vertical_bar(&ui, area).expect("scroll area should expose vertical bar");
    assert_eq!(scroll_area_horizontal_bar(&ui, area), None);

    for _ in 0..5 {
        append_scroll_test_child(&mut ui, viewport);
    }

    ui.compute_layout(Size::new(160.0, 100.0));
    let viewport_bounds = ui.effective_bounds(viewport);
    let bar_bounds = ui.effective_bounds(bar);

    assert_eq!(
        viewport_bounds.width(),
        120.0 - DEFAULT_SCROLL_AREA_GAP - DEFAULT_SCROLL_BAR_THICKNESS
    );
    assert_eq!(viewport_bounds.height(), 80.0);
    assert!(bar_bounds.x0 > viewport_bounds.x1);

    let target = Point::new(bar_bounds.center().x, bar_bounds.y1 - 1.0);
    assert!(
        ui.dispatch_pointer_event(&pointer_down(target, 1))
            .changed()
    );
    assert!(ui.dispatch_pointer_event(&pointer_up(target, 2)).handled);
    assert!(scroll_offset_y(&ui, viewport).is_some_and(|offset| offset > 0.0));
}

#[test]
fn scroll_area_children_follow_arranged_height() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::row(0.0).logical_size(160.0, 140.0));
    let area = ui.append(root, ScrollArea::new().size(120.0, 80.0));
    ui.set_layout(area, Layout::new().logical_size(120.0, 80.0).fill_height());
    let viewport = scroll_area_viewport(&ui, area).expect("scroll area should expose viewport");
    let bar = scroll_area_vertical_bar(&ui, area).expect("scroll area should expose vertical bar");

    for _ in 0..8 {
        append_scroll_test_child(&mut ui, viewport);
    }

    ui.compute_layout(Size::new(160.0, 140.0));
    let area_bounds = ui.effective_bounds(area);
    let viewport_bounds = ui.effective_bounds(viewport);
    let bar_bounds = ui.effective_bounds(bar);

    assert_eq!(area_bounds.height(), 140.0);
    assert_eq!(viewport_bounds.height(), 140.0);
    assert_eq!(bar_bounds.height(), 140.0);
    assert_eq!(
        viewport_bounds.width(),
        120.0 - DEFAULT_SCROLL_AREA_GAP - DEFAULT_SCROLL_BAR_THICKNESS
    );
    assert!(
        scroll_offset_y(&ui, viewport).is_some(),
        "arranged viewport should still be registered as scrollable"
    );
}

#[test]
fn scroll_area_can_reserve_horizontal_bar_only() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(0.0).logical_size(160.0, 100.0));
    let area = ui.append(
        root,
        ScrollArea::new()
            .size(120.0, 80.0)
            .vertical_scrollbar(false)
            .horizontal_scrollbar(true),
    );
    let viewport = scroll_area_viewport(&ui, area).expect("scroll area should expose viewport");
    let bar =
        scroll_area_horizontal_bar(&ui, area).expect("scroll area should expose horizontal bar");

    assert_eq!(scroll_area_vertical_bar(&ui, area), None);

    ui.compute_layout(Size::new(160.0, 100.0));
    let viewport_bounds = ui.effective_bounds(viewport);
    let bar_bounds = ui.effective_bounds(bar);

    assert_eq!(viewport_bounds.width(), 120.0);
    assert_eq!(
        viewport_bounds.height(),
        80.0 - DEFAULT_SCROLL_AREA_GAP - DEFAULT_SCROLL_BAR_THICKNESS
    );
    assert!(bar_bounds.y0 > viewport_bounds.y1);
}

#[test]
fn scroll_bar_thumb_tracks_target_scroll_state() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::row(8.0).logical_size(180.0, 80.0));
    let scroll = ui.append(root, ScrollView::new().size(100.0, 48.0));
    for _ in 0..4 {
        append_scroll_test_child(&mut ui, scroll);
    }
    let bar = ui.append(
        root,
        ScrollBar::vertical(scroll)
            .length(48.0)
            .thickness(8.0)
            .thumb_extent(12.0),
    );
    let thumb = ui
        .slot(bar, SLOT_SCROLL_BAR_THUMB)
        .expect("scrollbar template should create a thumb slot");

    ui.compute_layout(Size::new(180.0, 80.0));
    let before = ui.effective_bounds(thumb);
    assert!(set_scroll_view_offset_y(&mut ui, scroll, 24.0));
    ui.compute_layout(Size::new(180.0, 80.0));
    let after = ui.effective_bounds(thumb);

    assert!(after.y0 > before.y0);
}

#[test]
fn scroll_bar_track_click_pages_viewport() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = events.clone();

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::row(8.0).logical_size(180.0, 80.0));
    let scroll = ui.append(
        root,
        ScrollView::new()
            .size(100.0, 48.0)
            .on_offset_changed(move |_ui, event| {
                seen.borrow_mut().push(event);
            }),
    );
    for _ in 0..4 {
        append_scroll_test_child(&mut ui, scroll);
    }
    let bar = ui.append(
        root,
        ScrollBar::vertical(scroll)
            .length(48.0)
            .thickness(8.0)
            .thumb_extent(12.0),
    );

    ui.compute_layout(Size::new(180.0, 80.0));
    let bar_bounds = ui.effective_bounds(bar);
    let result = ui.synthesize_click(bar, Point::new(bar_bounds.center().x, bar_bounds.y1 - 1.0));

    assert!(result.handled);
    assert!(scroll_view_offset_y(&ui, scroll).is_some_and(|offset| offset > 0.0));
    assert_eq!(events.borrow().len(), 1);
    assert_eq!(events.borrow()[0].source, ScrollInputSource::ScrollBar);
}

#[test]
fn scroll_bar_track_pointer_click_pages_viewport() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::row(8.0).logical_size(180.0, 80.0));
    let scroll = ui.append(root, ScrollView::new().size(100.0, 48.0));
    for _ in 0..4 {
        append_scroll_test_child(&mut ui, scroll);
    }
    let bar = ui.append(
        root,
        ScrollBar::vertical(scroll)
            .length(48.0)
            .thickness(8.0)
            .thumb_extent(12.0),
    );

    ui.compute_layout(Size::new(180.0, 80.0));
    let bar_bounds = ui.effective_bounds(bar);
    let target = Point::new(bar_bounds.center().x, bar_bounds.y1 - 1.0);

    assert!(
        ui.dispatch_pointer_event(&pointer_down(target, 1))
            .changed()
    );
    assert!(ui.dispatch_pointer_event(&pointer_up(target, 2)).handled);
    assert!(scroll_view_offset_y(&ui, scroll).is_some_and(|offset| offset > 0.0));
}

#[test]
fn scroll_bar_thumb_drag_updates_viewport_offset() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::row(8.0).logical_size(180.0, 80.0));
    let scroll = ui.append(root, ScrollView::new().size(100.0, 48.0));
    for _ in 0..4 {
        append_scroll_test_child(&mut ui, scroll);
    }
    let bar = ui.append(
        root,
        ScrollBar::vertical(scroll)
            .length(48.0)
            .thickness(8.0)
            .thumb_extent(12.0),
    );
    let thumb = ui
        .slot(bar, SLOT_SCROLL_BAR_THUMB)
        .expect("scrollbar template should create a thumb slot");

    ui.compute_layout(Size::new(180.0, 80.0));
    let start = ui.effective_bounds(thumb).center();
    assert!(ui.dispatch_pointer_event(&pointer_down(start, 1)).changed());
    assert!(
        ui.dispatch_pointer_event(&pointer_move(Point::new(start.x, start.y + 20.0), 2, true,))
            .changed()
    );

    assert!(scroll_view_offset_y(&ui, scroll).is_some_and(|offset| offset > 0.0));
}

#[test]
fn scroll_view_offset_changed_reports_programmatic_source() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = events.clone();

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 100.0));
    let scroll = ui.append(
        root,
        ScrollView::new()
            .size(120.0, 48.0)
            .on_offset_changed(move |_ui, event| {
                seen.borrow_mut().push(event);
            }),
    );
    for _ in 0..4 {
        append_scroll_test_child(&mut ui, scroll);
    }

    ui.compute_layout(Size::new(180.0, 100.0));
    assert!(set_scroll_view_offset_y(&mut ui, scroll, 12.0));
    assert!(!set_scroll_view_offset_y(&mut ui, scroll, 12.0));

    assert_eq!(
        events.borrow().as_slice(),
        [ScrollOffsetChangedEvent {
            scroll_view: scroll,
            previous_offset: Vec2::ZERO,
            offset: Vec2::new(0.0, 12.0),
            source: ScrollInputSource::Programmatic,
        }]
    );
}

#[test]
fn scroll_view_offset_changed_reports_wheel_source() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = events.clone();

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(0.0).logical_size(180.0, 120.0));
    let scroll = ui.append(
        root,
        ScrollView::new()
            .size(120.0, 48.0)
            .on_offset_changed(move |_ui, event| {
                seen.borrow_mut().push(event);
            }),
    );
    for _ in 0..4 {
        append_scroll_test_child(&mut ui, scroll);
    }

    ui.compute_layout(Size::new(180.0, 120.0));
    let result = ui.synthesize_scroll(scroll, Point::ZERO, ScrollDelta::LineDelta(0.0, -1.0));

    assert!(result.handled);
    assert_eq!(events.borrow().len(), 1);
    let event = events.borrow()[0];
    assert_eq!(event.scroll_view, scroll);
    assert_eq!(event.previous_offset, Vec2::ZERO);
    assert!(event.offset.y > 0.0);
    assert_eq!(event.source, ScrollInputSource::Wheel);
}

#[test]
fn scroll_bar_accessibility_exposes_range_and_set_value_action() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::row(8.0).logical_size(180.0, 80.0));
    let scroll = ui.append(root, ScrollView::new().size(100.0, 48.0));
    for _ in 0..4 {
        append_scroll_test_child(&mut ui, scroll);
    }
    let bar = ui.append(
        root,
        ScrollBar::vertical(scroll)
            .length(48.0)
            .thickness(8.0)
            .thumb_extent(12.0),
    );

    ui.compute_layout(Size::new(180.0, 80.0));
    let update = ui.accesskit_tree_update();
    let node = accesskit_node_for(&update, bar);
    let max = node
        .max_numeric_value()
        .expect("scrollbar should expose max");

    assert_eq!(node.role(), Role::ScrollBar);
    assert_eq!(node.orientation(), Some(Orientation::Vertical));
    assert_eq!(node.numeric_value(), Some(0.0));
    assert!(max > 0.0);
    assert_eq!(node.min_numeric_value(), Some(0.0));
    assert_eq!(node.numeric_value_step(), Some(DEFAULT_SCROLL_LINE_Y));
    assert!(node.supports_action(Action::Increment));
    assert!(node.supports_action(Action::Decrement));
    assert!(node.supports_action(Action::SetValue));

    let handled = ui.handle_accesskit_action(&numeric_action(bar, Action::SetValue, max * 0.5));

    assert!(handled);
    assert!(scroll_view_offset_y(&ui, scroll).is_some_and(|offset| offset > 0.0));
}

#[test]
fn scroll_bar_uses_default_cursor() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::row(8.0).logical_size(180.0, 80.0));
    let scroll = ui.append(root, ScrollView::new().size(100.0, 48.0));
    for _ in 0..4 {
        append_scroll_test_child(&mut ui, scroll);
    }
    let bar = ui.append(
        root,
        ScrollBar::vertical(scroll)
            .length(48.0)
            .thickness(8.0)
            .thumb_extent(12.0),
    );

    ui.compute_layout(Size::new(180.0, 80.0));
    let bar_bounds = ui.effective_bounds(bar);
    let result = ui.dispatch_pointer_event(&pointer_move(bar_bounds.center(), 1, false));

    assert!(result.hover_changed);
    assert_eq!(ui.cursor_icon(), None);
}

fn append_scroll_test_child(ui: &mut Ui, scroll: ElementId) -> ElementId {
    let child = ui.insert_child(scroll);
    ui.set_layout(
        child,
        Layout::new().logical_size(100.0, 24.0).flex_shrink(0.0),
    );
    child
}

fn numeric_action(element: ElementId, action: Action, value: f64) -> ActionRequest {
    ActionRequest {
        action,
        target_tree: TreeId::ROOT,
        target_node: accesskit_node_id(element),
        data: Some(ActionData::NumericValue(value)),
    }
}

fn scroll_action(element: ElementId, action: Action, unit: accesskit::ScrollUnit) -> ActionRequest {
    ActionRequest {
        action,
        target_tree: TreeId::ROOT,
        target_node: accesskit_node_id(element),
        data: Some(ActionData::ScrollUnit(unit)),
    }
}

fn scroll_offset_action(element: ElementId, x: f64, y: f64) -> ActionRequest {
    ActionRequest {
        action: Action::SetScrollOffset,
        target_tree: TreeId::ROOT,
        target_node: accesskit_node_id(element),
        data: Some(ActionData::SetScrollOffset(accesskit::Point::new(x, y))),
    }
}

fn accesskit_node_for(update: &TreeUpdate, element: ElementId) -> &Node {
    update
        .nodes
        .iter()
        .find(|(id, _)| *id == accesskit_node_id(element))
        .map(|(_, node)| node)
        .expect("node should exist")
}
