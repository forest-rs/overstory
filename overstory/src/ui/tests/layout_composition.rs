// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn compute_layout_writes_bounds() {
    let mut ui = Ui::new();
    let root = ui.root();
    let child = ui.insert_child(root);

    // Root fills the viewport with 10px padding.
    ui.set_layout(root, Layout::flex().percent_size(1.0, 1.0));
    ui.set_box_padding(root, understory_presentation::Edges::all(10.0));
    // Child fills the padded content.
    ui.set_layout(child, Layout::new().flex_grow(1.0).align_self_stretch());

    ui.compute_layout(Size::new(200.0, 100.0));
    // Root is the full viewport.
    assert_eq!(ui.effective_bounds(root), Rect::new(0.0, 0.0, 200.0, 100.0));
    // Child fills (10, 10) → (190, 90).
    assert_eq!(
        ui.effective_bounds(child),
        Rect::new(10.0, 10.0, 190.0, 90.0)
    );
}

#[test]
fn surface_border_and_padding_define_layout_content_inset() {
    use crate::SurfaceSpecExt;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::row(0.0).logical_size(120.0, 80.0));

    let child = ui.append(
        root,
        crate::Panel::new()
            .border_width(3.0)
            .padding_all(10.0)
            .layout(Layout::row(0.0).logical_size(80.0, 40.0)),
    );
    let grandchild = ui.append(
        child,
        crate::Panel::new().layout(Layout::new().logical_size(20.0, 10.0)),
    );

    ui.compute_layout(Size::new(120.0, 80.0));

    assert_eq!(ui.effective_bounds(child), Rect::new(0.0, 0.0, 80.0, 40.0));
    assert_eq!(
        ui.effective_bounds(grandchild),
        Rect::new(13.0, 13.0, 33.0, 23.0)
    );
}

#[test]
fn clean_compute_layout_skips_widget_layout_work() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    #[derive(Debug)]
    struct LayoutProbe {
        count: Rc<Cell<u32>>,
    }

    impl crate::Widget for LayoutProbe {
        fn realize_elements(
            &mut self,
            _ctx: &mut crate::ElementRealizeContext<'_>,
        ) -> crate::ElementRealizeResult {
            self.count.set(self.count.get() + 1);
            crate::ElementRealizeResult::UNCHANGED
        }
    }

    let mut ui = Ui::new();
    let root = ui.root();
    let count = Rc::new(Cell::new(0));
    let child = ui.append_widget(
        root,
        Box::new(LayoutProbe {
            count: Rc::clone(&count),
        }),
    );
    ui.set_layout(child, Layout::new().logical_size(10.0, 10.0));

    ui.compute_layout(Size::new(100.0, 50.0));
    assert_eq!(count.get(), 1);

    ui.set_now(1);
    ui.compute_layout(Size::new(100.0, 50.0));
    assert_eq!(count.get(), 1);

    ui.compute_layout(Size::new(120.0, 50.0));
    assert_eq!(count.get(), 2);

    ui.compute_layout(Size::new(120.0, 50.0));
    assert_eq!(count.get(), 2);

    ui.mark_channels(child, LAYOUT.into());
    ui.compute_layout(Size::new(120.0, 50.0));
    assert_eq!(count.get(), 3);
}

#[test]
fn layout_placement_does_not_replace_content_layout() {
    use crate::Panel;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(200.0, 100.0));
    let row = ui.append(
        root,
        Panel::new()
            .padding_all(10.0)
            .layout(Layout::row(0.0).logical_size(80.0, 40.0)),
    );
    let child = ui.append(
        row,
        Panel::new().layout(Layout::new().logical_size(20.0, 10.0)),
    );

    ui.set_layout_placement(row, LayoutPlacement::absolute_at(30.0, 20.0));
    ui.compute_layout(Size::new(200.0, 100.0));

    assert_eq!(ui.effective_bounds(row), Rect::new(30.0, 20.0, 110.0, 60.0));
    assert_eq!(
        ui.effective_bounds(child),
        Rect::new(40.0, 30.0, 60.0, 40.0)
    );
}

#[test]
fn set_layout_without_placement_keeps_styled_placement() {
    use crate::Panel;

    const FLOATING: ClassId = ClassId(0x34);

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(200.0, 100.0));
    let row = ui.append(
        root,
        Panel::new()
            .class(FLOATING)
            .padding_all(10.0)
            .layout(Layout::row(0.0).logical_size(80.0, 40.0)),
    );
    let child = ui.append(
        row,
        Panel::new().layout(Layout::new().logical_size(20.0, 10.0)),
    );
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC).with_class(FLOATING)).build(),
            StyleBuilder::new()
                .set(
                    ui.properties().layout_placement,
                    LayoutPlacement::absolute_at(30.0, 20.0),
                )
                .build(),
        )
        .build();
    ui.set_cascade(cascade);

    ui.compute_layout(Size::new(200.0, 100.0));

    assert_eq!(ui.effective_bounds(row), Rect::new(30.0, 20.0, 110.0, 60.0));
    assert_eq!(
        ui.effective_bounds(child),
        Rect::new(40.0, 30.0, 60.0, 40.0)
    );
}

#[test]
fn set_layout_size_does_not_replace_styled_box_padding() {
    use crate::Panel;

    const PADDED: ClassId = ClassId(0x35);

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(200.0, 100.0));
    let row = ui.append(
        root,
        Panel::new()
            .class(PADDED)
            .layout(Layout::row(0.0).logical_size(80.0, 40.0)),
    );
    let child = ui.append(
        row,
        Panel::new().layout(Layout::new().logical_size(20.0, 10.0)),
    );
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC).with_class(PADDED)).build(),
            StyleBuilder::new()
                .padding_all(ui.properties(), 10.0)
                .set(
                    ui.properties().layout_alignment,
                    crate::LayoutAlignment::new().justify_content(crate::LayoutJustify::Center),
                )
                .build(),
        )
        .build();
    ui.set_cascade(cascade);

    ui.set_layout_size(row, crate::LayoutSize::logical(120.0, 40.0));
    ui.compute_layout(Size::new(200.0, 100.0));

    assert_eq!(ui.effective_bounds(row), Rect::new(0.0, 0.0, 120.0, 40.0));
    assert_eq!(
        ui.effective_bounds(child),
        Rect::new(50.0, 10.0, 70.0, 20.0)
    );
}

#[test]
fn layout_longhands_extend_size_and_spacing_without_taffy_leakage() {
    use crate::{LayoutDimension, LayoutSize};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::row(0.0).logical_size(160.0, 80.0));
    let a = ui.append(
        root,
        crate::Panel::new().layout(
            Layout::new()
                .logical_size(20.0, 20.0)
                .margin_all(5.0)
                .min_logical_size(40.0, 30.0),
        ),
    );
    let b = ui.append(
        root,
        crate::Panel::new().layout(
            Layout::new()
                .logical_size(90.0, 30.0)
                .max_logical_size(50.0, 20.0)
                .flex_basis(LayoutDimension::Logical(60.0)),
        ),
    );

    ui.compute_layout(Size::new(160.0, 80.0));

    assert_eq!(ui.effective_bounds(a), Rect::new(5.0, 5.0, 45.0, 35.0));
    assert_eq!(ui.effective_bounds(b), Rect::new(50.0, 0.0, 100.0, 20.0));

    ui.set_layout_size(b, LayoutSize::logical(30.0, 10.0));
    ui.compute_layout(Size::new(160.0, 80.0));

    assert_eq!(ui.effective_bounds(b), Rect::new(50.0, 0.0, 80.0, 10.0));
}

#[test]
fn layout_wrap_places_children_on_additional_lines() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::row(0.0).wrap().logical_size(100.0, 40.0));
    let first = ui.append(
        root,
        crate::Panel::new().layout(Layout::new().logical_size(70.0, 20.0)),
    );
    let second = ui.append(
        root,
        crate::Panel::new().layout(Layout::new().logical_size(70.0, 20.0)),
    );

    ui.compute_layout(Size::new(100.0, 80.0));

    assert_eq!(ui.effective_bounds(first), Rect::new(0.0, 0.0, 70.0, 20.0));
    assert_eq!(
        ui.effective_bounds(second),
        Rect::new(0.0, 20.0, 70.0, 40.0)
    );
}

#[test]
fn composition_layers_track_layout_geometry() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(200.0, 100.0));
    ui.set_box_padding(root, understory_presentation::Edges::all(10.0));
    let child = ui.insert_child(root);
    ui.set_layout(child, Layout::new().logical_size(30.0, 20.0));

    ui.compute_layout(Size::new(200.0, 100.0));

    let root_layer = ui.composition.layer_for(root).unwrap();
    let child_layer = ui.composition.layer_for(child).unwrap();
    let layers = ui.composition_layers();

    assert_eq!(layers.parent(child_layer), Some(root_layer));
    assert_eq!(layers.bounds(root_layer), Size::new(200.0, 100.0));
    assert_eq!(layers.bounds(child_layer), Size::new(30.0, 20.0));
    assert_eq!(
        layers
            .world_transform(child_layer)
            .transform_point(Point::ZERO),
        Some(Point::new(10.0, 10.0))
    );
    assert_eq!(ui.composition.element_for_layer(child_layer), Some(child));
}

#[test]
fn inspection_snapshot_reports_elements_and_composition() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(240.0, 80.0));
    let button = ui.append(root, crate::Button::new("Inspect"));

    ui.compute_layout(Size::new(240.0, 80.0));
    assert!(ui.set_focus(Some(button)));
    let mut scene = Scene::new();
    ui.paint(&mut scene);

    let snapshot = ui.inspection_snapshot();
    assert_eq!(snapshot.root, root);
    assert_eq!(snapshot.storage.live_elements, ui.len());
    assert_eq!(
        snapshot.storage.allocated_element_slots,
        snapshot.storage.live_elements + snapshot.storage.free_element_slots
    );
    assert_eq!(snapshot.frame.elements, ui.len());

    let root_row = snapshot
        .elements
        .iter()
        .find(|row| row.id == root)
        .expect("root row should be present");
    assert!(root_row.children.contains(&button));
    assert!(root_row.layer.is_some());
    assert_eq!(
        root_row.composition_policy,
        Some(CompositionPolicy::InlineScene)
    );

    let button_row = snapshot
        .elements
        .iter()
        .find(|row| row.id == button)
        .expect("button row should be present");
    assert_eq!(button_row.parent, Some(root));
    assert_eq!(button_row.type_tag, Some(crate::TYPE_BUTTON));
    assert!(button_row.has_widget_identity);
    assert!(button_row.state.enabled);
    assert!(button_row.state.focusable);
    assert!(button_row.state.focused);
    assert!(button_row.state.focus_visible);
    assert!(button_row.layer.is_some());

    let button_label = ui
        .slot(button, crate::SLOT_BUTTON_CONTENT)
        .expect("button template should expose content slot");
    let label_row = snapshot
        .elements
        .iter()
        .find(|row| row.id == button_label)
        .expect("button label row should be present");
    assert_eq!(label_row.templated_parent, Some(button));
    assert_eq!(label_row.part_tag, Some(crate::PART_BUTTON_LABEL));
    assert_eq!(label_row.template_slot, Some("content"));
    assert_eq!(label_row.template_key, None);

    assert_eq!(
        snapshot.composition.layers.len(),
        snapshot.storage.composition_layers
    );
    assert!(!snapshot.composition.roots.is_empty());
    assert_eq!(
        snapshot.composition.surfaces.len(),
        snapshot.storage.composition_surfaces
    );
    assert!(
        snapshot
            .composition
            .layers
            .iter()
            .any(|row| row.element == Some(button))
    );
}

#[test]
fn inspection_changes_track_tree_insertions_and_removals() {
    let mut ui = Ui::new();
    let root = ui.root();

    let silent = ui.append(root, crate::Panel::new());
    assert!(ui.drain_inspection_changes().is_empty());
    ui.remove(silent);
    assert!(ui.drain_inspection_changes().is_empty());

    ui.begin_live_inspection_recording();
    ui.clear_inspection_changes();
    assert!(ui.is_live_inspection_recording());

    let parent = ui.append(root, crate::Panel::new());
    let child = ui.append(parent, crate::Panel::new());
    let inserted = ui.drain_inspection_changes();

    assert!(inserted.tree.removed.is_empty());
    assert!(inserted.tree.updated.is_empty());
    assert!(
        inserted
            .tree
            .upserted
            .iter()
            .any(|row| row.id == root && row.children.contains(&parent))
    );
    assert!(
        inserted
            .tree
            .upserted
            .iter()
            .any(|row| row.id == parent && row.children.contains(&child))
    );
    assert!(
        inserted
            .tree
            .upserted
            .iter()
            .any(|row| row.id == child && row.parent == Some(parent))
    );
    assert!(ui.drain_inspection_changes().is_empty());

    ui.remove(parent);
    let removed = ui.drain_inspection_changes();

    assert!(
        removed
            .tree
            .removed
            .iter()
            .any(|row| row.id == parent && row.parent == Some(root))
    );
    assert!(
        removed
            .tree
            .removed
            .iter()
            .any(|row| row.id == child && row.parent == Some(parent))
    );
    assert!(
        removed
            .tree
            .upserted
            .iter()
            .any(|row| row.id == root && !row.children.contains(&parent))
    );
    assert!(!removed.tree.upserted.iter().any(|row| row.id == parent));
    assert!(!removed.tree.updated.iter().any(|row| row.id == parent));

    ui.end_live_inspection_recording();
    assert!(!ui.is_live_inspection_recording());

    let quiet = ui.append(root, crate::Panel::new());
    assert!(ui.drain_inspection_changes().is_empty());
    ui.remove(quiet);
    assert!(ui.drain_inspection_changes().is_empty());
}

#[test]
fn inspection_changes_track_frame_summary_updates() {
    let mut ui = Ui::new();
    ui.set_frame_timings(UiFrameTimings {
        surface_refresh: Duration::from_millis(2),
        total: Duration::from_millis(5),
        ..UiFrameTimings::default()
    });
    assert!(
        ui.drain_inspection_changes().is_empty(),
        "frame timings should not accumulate before live recording starts"
    );

    ui.begin_live_inspection_recording();
    ui.clear_inspection_changes();
    ui.set_frame_timings(UiFrameTimings {
        surface_refresh: Duration::from_millis(3),
        total: Duration::from_millis(8),
        ..UiFrameTimings::default()
    });

    let changes = ui.drain_inspection_changes();

    assert!(changes.tree.is_empty());
    let frame = changes.frame.expect("frame summary should be drained once");
    assert_eq!(frame.timings.surface_refresh, Duration::from_millis(3));
    assert_eq!(frame.timings.total, Duration::from_millis(8));
    assert!(ui.drain_inspection_changes().is_empty());
}

#[test]
fn renderer_timings_survive_frame_counter_reset() {
    let mut ui = Ui::new();
    let timings = UiRendererTimings {
        scene_to_texture: Duration::from_millis(1),
        surface_acquire: Duration::from_millis(2),
        blit: Duration::from_millis(3),
        present: Duration::from_millis(4),
        total: Duration::from_millis(10),
    };

    ui.set_renderer_timings(timings);
    ui.reset_frame_stats();

    assert_eq!(ui.frame_stats().renderer_timings, timings);
    assert_eq!(ui.frame_stats().layout_passes, 0);
}

#[test]
fn prepared_frame_demand_resets_with_frame_counters() {
    let mut ui = Ui::new();

    ui.set_prepared_frame_demand(FrameDemand::ANIMATION);
    assert_eq!(ui.frame_stats().prepared_demand, FrameDemand::ANIMATION);

    ui.reset_frame_stats();

    assert_eq!(ui.frame_stats().prepared_demand, FrameDemand::NONE);
}

#[test]
fn inspection_changes_track_existing_element_state_updates() {
    use crate::inspection::ElementPropertyInspectionValue;

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, crate::Button::new("Inspect"));
    ui.begin_live_inspection_recording();
    ui.clear_inspection_changes();

    ui.add_pseudo(button, PSEUDO_HOVER);
    let hovered = ui.drain_inspection_changes();
    assert!(hovered.tree.upserted.is_empty());
    let hovered_row = hovered
        .tree
        .updated
        .iter()
        .find(|row| row.id == button)
        .expect("hover should update button row");
    assert!(hovered_row.state.hovered);
    assert!(hovered_row.pseudos.contains(&PSEUDO_HOVER));

    ui.set_focusable(button, false);
    let unfocusable = ui.drain_inspection_changes();
    assert!(
        unfocusable
            .tree
            .updated
            .iter()
            .any(|row| row.id == button && !row.state.focusable)
    );

    ui.set_focusable(button, true);
    assert!(ui.set_focus(Some(button)));
    let focused = ui.drain_inspection_changes();
    assert!(
        focused
            .tree
            .updated
            .iter()
            .any(|row| { row.id == button && row.state.focused && row.state.focus_visible })
    );

    assert!(ui.capture_pointer(button));
    let captured = ui.drain_inspection_changes();
    assert!(
        captured
            .tree
            .updated
            .iter()
            .any(|row| row.id == button && row.state.pointer_captured)
    );

    ui.set_local(button, ui.properties().opacity, 0.25);
    let property_update = ui.drain_inspection_changes();
    assert!(
        property_update.tree.updated.iter().any(|row| {
            row.id == button
                && row.properties.iter().any(|property| {
                    property.name == "Opacity"
                        && property.value == ElementPropertyInspectionValue::Number(0.25)
                })
        }),
        "property writes should update rows for live inspectors"
    );
}

#[test]
fn inspection_changes_coalesce_existing_element_updates_until_drain() {
    use crate::inspection::ElementPropertyInspectionValue;

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, crate::Button::new("Inspect"));
    ui.begin_live_inspection_recording();
    ui.clear_inspection_changes();

    ui.set_local(button, ui.properties().opacity, 0.25);
    ui.set_local(button, ui.properties().opacity, 0.5);
    ui.add_pseudo(button, PSEUDO_HOVER);

    let changes = ui.drain_inspection_changes();
    assert!(changes.tree.upserted.is_empty());
    let button_rows: Vec<_> = changes
        .tree
        .updated
        .iter()
        .filter(|row| row.id == button)
        .collect();

    assert_eq!(
        button_rows.len(),
        1,
        "live inspection should materialize one final row per dirty element"
    );
    let button_row = button_rows[0];
    assert!(button_row.state.hovered);
    assert!(
        button_row.properties.iter().any(|property| {
            property.name == "Opacity"
                && property.value == ElementPropertyInspectionValue::Number(0.5)
        }),
        "coalesced row should report the final property value"
    );
}

#[test]
fn inspection_changes_keep_inserted_elements_in_upsert_lane() {
    use crate::inspection::ElementPropertyInspectionValue;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.begin_live_inspection_recording();
    ui.clear_inspection_changes();

    let child = ui.append(root, crate::Panel::new());
    ui.set_local(child, ui.properties().opacity, 0.5);

    let changes = ui.drain_inspection_changes();
    assert!(
        changes.tree.updated.is_empty(),
        "new elements should not be duplicated as detail-only updates"
    );
    assert!(
        changes.tree.upserted.iter().any(|row| {
            row.id == child
                && row.properties.iter().any(|property| {
                    property.name == "Opacity"
                        && property.value == ElementPropertyInspectionValue::Number(0.5)
                })
        }),
        "upserted rows should be materialized with final pre-drain values"
    );
}

#[test]
fn inspection_changes_do_not_materialize_removed_dirty_elements() {
    let mut ui = Ui::new();
    let root = ui.root();
    let parent = ui.append(root, crate::Panel::new());
    let child = ui.append(parent, crate::Panel::new());
    ui.begin_live_inspection_recording();
    ui.clear_inspection_changes();

    ui.set_local(child, ui.properties().opacity, 0.25);
    ui.remove(child);

    let changes = ui.drain_inspection_changes();

    assert!(
        changes
            .tree
            .removed
            .iter()
            .any(|removal| removal.id == child && removal.parent == Some(parent))
    );
    assert!(
        !changes.tree.upserted.iter().any(|row| row.id == child),
        "removed dirty elements should not be materialized as upserts during drain"
    );
    assert!(
        !changes.tree.updated.iter().any(|row| row.id == child),
        "removed dirty elements should not be materialized as updates during drain"
    );
}

#[test]
fn inspection_snapshot_reports_timeline_counts() {
    let mut ui = Ui::new();
    let root = ui.root();
    let scoped = ui.append(root, crate::Panel::new());

    ui.schedule_timeline_cue(10, 1);
    ui.schedule_element_timeline_cue(scoped, 20, 2)
        .expect("fresh element should accept scoped cue");

    let snapshot = ui.inspection_snapshot();
    assert_eq!(snapshot.timeline.pending_cues, 2);
    assert_eq!(snapshot.timeline.targets, 0);
    assert_eq!(snapshot.timeline.active_transitions, 0);
    assert_eq!(snapshot.timeline.active_effects, 0);
    assert_eq!(snapshot.timeline.pending_transitions, 0);
}

#[test]
fn visual_transform_updates_composition_without_changing_layout_bounds() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(200.0, 100.0));
    ui.set_box_padding(root, understory_presentation::Edges::all(10.0));
    let child = ui.insert_child(root);
    ui.set_layout(child, Layout::new().logical_size(30.0, 20.0));

    ui.compute_layout(Size::new(200.0, 100.0));
    let child_layer = ui.composition.layer_for(child).unwrap();
    let bounds = ui.effective_bounds(child);

    ui.set_local(child, ui.properties().visual_transform, Affine::scale(2.0));
    ui.flush_composition();

    let layers = ui.composition_layers();
    assert_eq!(ui.effective_bounds(child), bounds);
    assert_eq!(
        layers
            .world_transform(child_layer)
            .transform_point(Point::ZERO),
        Some(Point::new(10.0, 10.0))
    );
    assert_eq!(
        layers
            .world_transform(child_layer)
            .transform_point(Point::new(5.0, 0.0)),
        Some(Point::new(20.0, 10.0))
    );
}

#[test]
fn scene_composition_applies_effective_layer_opacity() {
    use crate::Panel;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(100.0, 60.0));
    let child = ui.append(
        root,
        Panel::new().layout(Layout::new().logical_size(40.0, 30.0)),
    );
    ui.set_local(
        child,
        ui.properties().surface.background,
        Some(Brush::Solid(Color::from_rgba8(0x40, 0x80, 0xff, 0xff))),
    );
    ui.set_local(child, ui.properties().opacity, 0.5_f32);

    ui.compute_layout(Size::new(100.0, 60.0));
    let mut output = Scene::new();
    ui.paint(&mut output);

    let has_half_opacity_group = output.commands().iter().any(|command| {
        let Command::PushGroup(group) = command else {
            return false;
        };
        (output.group(*group).composite.alpha - 0.5).abs() <= f32::EPSILON
    });
    assert!(has_half_opacity_group);
}

#[test]
fn composition_layer_is_destroyed_with_element() {
    let mut ui = Ui::new();
    let root = ui.root();
    let child = ui.insert_child(root);
    let layer = ui.composition.layer_for(child).unwrap();

    ui.remove(child);
    ui.flush_composition();

    assert!(ui.composition.layer_for(child).is_none());
    assert!(!ui.composition_layers().is_alive(layer));
    assert!(ui.composition_changes().removed.contains(&layer.index()));
}

#[test]
fn paint_rebuilds_root_composition_surface() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(40.0, 20.0));
    ui.set_local(
        root,
        ui.properties().surface.background,
        Some(Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff))),
    );
    ui.compute_layout(Size::new(40.0, 20.0));

    let mut output = Scene::new();
    ui.paint(&mut output);

    let root_layer = ui.composition.layer_for(root).unwrap();
    let root_surface = ui.composition.surface_for(root).unwrap();
    assert_eq!(
        ui.composition_layers().content(root_layer),
        Some(root_surface)
    );
    assert!(!ui.composition.surface(root_surface).unwrap().is_empty());
    assert!(!output.is_empty());

    let updates: Vec<_> = ui
        .composition_surface_updates()
        .map(|surface| surface.surface())
        .collect();
    assert_eq!(updates, vec![root_surface]);

    assert!(
        ui.composition_surfaces()
            .any(|surface| surface.surface() == root_surface && !surface.scene().is_empty())
    );
}

#[test]
fn paint_rebuilds_child_composition_surfaces() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(0.0).logical_size(40.0, 40.0));
    let child = ui.insert_child(root);
    ui.set_layout(child, Layout::new().logical_size(20.0, 20.0));
    ui.set_local(
        child,
        ui.properties().surface.background,
        Some(Brush::Solid(Color::from_rgba8(0x00, 0xff, 0x00, 0xff))),
    );
    ui.compute_layout(Size::new(40.0, 40.0));

    let mut output = Scene::new();
    ui.paint(&mut output);

    let child_layer = ui.composition.layer_for(child).unwrap();
    let child_surface = ui.composition.surface_for(child).unwrap();
    assert_eq!(
        ui.composition_layers().content(child_layer),
        Some(child_surface)
    );
    assert!(!ui.composition.surface(child_surface).unwrap().is_empty());
    assert!(
        ui.composition_surface_updates()
            .any(|surface| surface.surface() == child_surface)
    );
    assert!(!output.is_empty());

    ui.set_local(
        child,
        ui.properties().surface.background,
        Some(Brush::Solid(Color::from_rgba8(0xff, 0xff, 0x00, 0xff))),
    );
    ui.paint(&mut output);
    let updates: Vec<_> = ui
        .composition_surface_updates()
        .map(|surface| surface.surface())
        .collect();
    assert_eq!(updates, vec![child_surface]);
}

#[test]
fn composition_policy_can_make_container_only_layer() {
    use crate::widget::{AppendSpec, Widget};

    #[derive(Debug)]
    struct ContainerOnlyWidget;

    impl Widget for ContainerOnlyWidget {
        fn type_tag(&self) -> Option<understory_style::TypeTag> {
            None
        }

        fn composition_policy(&self) -> CompositionPolicy {
            CompositionPolicy::Container
        }
    }

    struct ContainerOnly;

    impl AppendSpec for ContainerOnly {
        fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
            ui.append_widget(parent, Box::new(ContainerOnlyWidget))
        }
    }

    let mut ui = Ui::new();
    let root = ui.root();
    let container = ui.append(root, ContainerOnly);

    let layer = ui.composition.layer_for(container).unwrap();
    assert_eq!(ui.composition_layers().content(layer), None);
    assert_eq!(ui.composition.surface_for(container), None);
}

#[test]
fn overlay_composition_policy_parents_layer_under_overlay_root() {
    use crate::widget::{AppendSpec, Widget};

    #[derive(Debug)]
    struct OverlayWidget;

    impl Widget for OverlayWidget {
        fn type_tag(&self) -> Option<understory_style::TypeTag> {
            None
        }

        fn composition_policy(&self) -> CompositionPolicy {
            CompositionPolicy::OverlayScene
        }
    }

    struct OverlayOnly;

    impl AppendSpec for OverlayOnly {
        fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
            ui.append_widget(parent, Box::new(OverlayWidget))
        }
    }

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(0.0).logical_size(100.0, 100.0));
    ui.set_box_padding(root, understory_presentation::Edges::all(10.0));
    let parent = ui.insert_child(root);
    ui.set_layout(parent, Layout::column(0.0).logical_size(50.0, 50.0));
    ui.set_box_padding(parent, understory_presentation::Edges::all(7.0));
    let overlay = ui.append(parent, OverlayOnly);
    ui.set_layout(overlay, Layout::new().logical_size(10.0, 10.0));

    ui.compute_layout(Size::new(100.0, 100.0));

    let overlay_root = ui
        .composition
        .overlay_root()
        .expect("composition should create an overlay root");
    let overlay_layer = ui.composition.layer_for(overlay).unwrap();
    let layers = ui.composition_layers();
    assert_eq!(layers.parent(overlay_layer), Some(overlay_root));
    assert_eq!(layers.content(overlay_root), None);
    assert_eq!(layers.roots().last(), Some(&overlay_root));
    assert_eq!(
        layers
            .world_transform(overlay_layer)
            .transform_point(Point::ZERO),
        Some(ui.effective_bounds(overlay).origin())
    );
}
