// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn numeric_value_writes_use_property_metadata_coercion() {
    let mut ui = Ui::new();
    let root = ui.root();

    ui.set_local(root, ui.properties().minimum, 10.0);
    ui.set_local(root, ui.properties().maximum, 20.0);
    ui.set_local(root, ui.properties().value, 30.0);

    assert_eq!(
        ui.get(root)
            .and_then(|element| element.get_local_value(ui.properties().value)),
        Some(&20.0),
    );
}

#[test]
fn changing_a_range_bound_recoerces_the_stored_value() {
    let mut ui = Ui::new();
    let root = ui.root();

    ui.set_local(root, ui.properties().maximum, 10.0);
    ui.set_local(root, ui.properties().value, 8.0);
    ui.set_local(root, ui.properties().minimum, 9.0);

    assert_eq!(
        ui.get(root)
            .and_then(|element| element.get_local_value(ui.properties().value)),
        Some(&9.0),
    );
}

#[test]
fn set_local_marks_paint_dirty() {
    let mut ui = Ui::new();
    let root = ui.root();
    let bounds = ui.properties().bounds;
    let background = ui.properties().surface.background;

    let _ = ui
        .invalidation
        .drain_sorted(PAINT)
        .collect::<Vec<ElementId>>();

    ui.set_local(root, bounds, Rect::new(0.0, 0.0, 100.0, 50.0));
    assert!(ui.invalidation().is_invalidated(root, PAINT));

    ui.invalidation.clear(PAINT);
    ui.set_local(root, bounds, Rect::new(0.0, 0.0, 100.0, 50.0));
    assert!(!ui.invalidation().is_invalidated(root, PAINT));

    ui.set_local(
        root,
        background,
        Some(Brush::Solid(Color::from_rgba8(0, 0, 0, 0xff))),
    );
    assert!(ui.invalidation().is_invalidated(root, PAINT));
}

#[test]
fn clear_local_removes_override_and_reveals_styled_value() {
    let mut ui = Ui::new();
    let root = ui.root();
    let background = ui.properties().surface.background;
    let styled = Some(Brush::Solid(Color::from_rgba8(0x24, 0x78, 0xff, 0xff)));
    let local = Some(Brush::Solid(Color::from_rgba8(0xff, 0x44, 0x55, 0xff)));
    let style = StyleBuilder::new().set(background, styled.clone()).build();
    let selector = SelectorBuilder::new(SelectorStep::type_tag(TYPE_ROOT)).build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(StyleOrigin::Base, selector, style)
        .build();
    ui.set_cascade(cascade);
    ui.set_local(root, background, local.clone());
    assert_eq!(
        ui.resolve_value_for(ui.get(root).unwrap(), background),
        local
    );

    ui.invalidation.clear(PAINT);
    assert!(ui.clear_local(root, background));

    assert_eq!(ui.get(root).unwrap().get_local_value(background), None);
    assert_eq!(
        ui.resolve_value_for(ui.get(root).unwrap(), background),
        styled
    );
    assert!(ui.invalidation().is_invalidated(root, PAINT));
}

#[test]
fn surface_background_only_invalidates_paint() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.invalidation.clear(LAYOUT);
    ui.invalidation.clear(GEOMETRY);
    ui.invalidation.clear(PAINT);

    ui.set_local(
        root,
        ui.properties().surface.background,
        Some(Brush::Solid(Color::from_rgba8(0x22, 0x33, 0x44, 0xff))),
    );

    assert!(!ui.invalidation().is_invalidated(root, LAYOUT));
    assert!(!ui.invalidation().is_invalidated(root, GEOMETRY));
    assert!(ui.invalidation().is_invalidated(root, PAINT));
}

#[test]
fn surface_corner_shape_invalidates_geometry_without_layout() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.invalidation.clear(LAYOUT);
    ui.invalidation.clear(GEOMETRY);
    ui.invalidation.clear(PAINT);

    ui.set_local(
        root,
        ui.properties().surface.corner_shapes.top_left,
        crate::CornerShape::Bevel,
    );

    assert!(!ui.invalidation().is_invalidated(root, LAYOUT));
    assert!(ui.invalidation().is_invalidated(root, GEOMETRY));
    assert!(ui.invalidation().is_invalidated(root, PAINT));
}

#[test]
fn surface_inset_properties_invalidate_layout() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.invalidation.clear(LAYOUT);
    ui.invalidation.clear(GEOMETRY);
    ui.invalidation.clear(PAINT);

    ui.set_local(root, ui.properties().surface.padding_widths.left, 12.0);

    assert!(ui.invalidation().is_invalidated(root, LAYOUT));
    assert!(ui.invalidation().is_invalidated(root, GEOMETRY));
    assert!(ui.invalidation().is_invalidated(root, PAINT));
}

#[test]
fn surface_border_style_invalidates_layout() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.invalidation.clear(LAYOUT);
    ui.invalidation.clear(GEOMETRY);
    ui.invalidation.clear(PAINT);

    ui.set_local(
        root,
        ui.properties().surface.border_styles.left,
        crate::BorderStyle::Solid,
    );

    assert!(ui.invalidation().is_invalidated(root, LAYOUT));
    assert!(ui.invalidation().is_invalidated(root, GEOMETRY));
    assert!(ui.invalidation().is_invalidated(root, PAINT));
}

#[test]
fn styled_surface_inset_properties_invalidate_layout() {
    const PADDED: ClassId = ClassId(0xB0D);

    let mut ui = Ui::new();
    let root = ui.root();
    let style = StyleBuilder::new()
        .set(ui.properties().surface.padding_widths.left, 12.0_f64)
        .build();
    let selector =
        SelectorBuilder::new(SelectorStep::type_tag(TYPE_ROOT).with_class(PADDED)).build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(StyleOrigin::Base, selector, style)
        .build();
    ui.set_cascade(cascade);
    ui.invalidation.clear(LAYOUT);
    ui.invalidation.clear(GEOMETRY);
    ui.invalidation.clear(PAINT);

    ui.add_class(root, PADDED);

    assert!(ui.invalidation().is_invalidated(root, LAYOUT));
    assert!(ui.invalidation().is_invalidated(root, GEOMETRY));
    assert!(ui.invalidation().is_invalidated(root, PAINT));
}

#[test]
fn styled_surface_corner_shape_invalidates_geometry_without_layout() {
    const SHAPED: ClassId = ClassId(0xB0E);

    let mut ui = Ui::new();
    let root = ui.root();
    let style = StyleBuilder::new()
        .set(
            ui.properties().surface.corner_shapes.top_left,
            crate::CornerShape::Bevel,
        )
        .build();
    let selector =
        SelectorBuilder::new(SelectorStep::type_tag(TYPE_ROOT).with_class(SHAPED)).build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(StyleOrigin::Base, selector, style)
        .build();
    ui.set_cascade(cascade);
    ui.invalidation.clear(LAYOUT);
    ui.invalidation.clear(GEOMETRY);
    ui.invalidation.clear(PAINT);

    ui.add_class(root, SHAPED);

    assert!(!ui.invalidation().is_invalidated(root, LAYOUT));
    assert!(ui.invalidation().is_invalidated(root, GEOMETRY));
    assert!(ui.invalidation().is_invalidated(root, PAINT));
}

#[test]
fn geometry_channel_cascades_to_paint_and_drains_during_refresh() {
    let mut ui = Ui::new();
    let root = ui.root();

    ui.mark_channels(root, GEOMETRY.into());

    assert!(ui.invalidation().is_invalidated(root, GEOMETRY));
    assert!(ui.invalidation().is_invalidated(root, PAINT));
    assert!(ui.invalidation().is_invalidated(root, COMPOSE));

    ui.refresh_presentation_surfaces();

    assert!(!ui.invalidation().is_invalidated(root, GEOMETRY));
    assert!(!ui.invalidation().is_invalidated(root, PAINT));
    assert_eq!(ui.frame_stats().geometry_elements, 1);
}

#[test]
fn arrange_channel_cascades_to_geometry_without_layout() {
    let mut ui = Ui::new();
    let root = ui.root();

    ui.mark_channels(root, ARRANGE.into());

    assert!(!ui.invalidation().is_invalidated(root, LAYOUT));
    assert!(ui.invalidation().is_invalidated(root, ARRANGE));
    assert!(ui.invalidation().is_invalidated(root, GEOMETRY));
    assert!(ui.invalidation().is_invalidated(root, PAINT));
    assert!(ui.invalidation().is_invalidated(root, COMPOSE));
}

#[test]
fn layout_channel_cascades_to_arrange() {
    let mut ui = Ui::new();
    let root = ui.root();

    ui.mark_channels(root, LAYOUT.into());

    assert!(ui.invalidation().is_invalidated(root, LAYOUT));
    assert!(ui.invalidation().is_invalidated(root, ARRANGE));
    assert!(ui.invalidation().is_invalidated(root, GEOMETRY));
    assert!(ui.invalidation().is_invalidated(root, PAINT));
    assert!(ui.invalidation().is_invalidated(root, COMPOSE));
}

#[test]
fn paint_drains_dirty_and_emits_output() {
    let mut ui = Ui::new();
    let root = ui.root();
    let layout_prop = ui.properties().layout;
    let background = ui.properties().surface.background;

    ui.set_local(root, layout_prop, Layout::new().logical_size(10.0, 10.0));
    ui.set_local(
        root,
        background,
        Some(Brush::Solid(Color::from_rgba8(0xff, 0, 0, 0xff))),
    );
    ui.compute_layout(Size::new(100.0, 100.0));

    let mut output = Scene::new();
    ui.paint(&mut output);

    assert!(!ui.invalidation().has_invalidated(PAINT));
    assert!(!output.is_empty());
}

#[test]
fn paint_skips_zero_area_elements() {
    let mut ui = Ui::new();
    let root = ui.root();
    let background = ui.properties().surface.background;

    ui.set_local(
        root,
        background,
        Some(Brush::Solid(Color::from_rgba8(0xff, 0, 0, 0xff))),
    );

    let mut output = Scene::new();
    ui.paint(&mut output);
    assert!(output.is_empty(), "zero-area element should record nothing");
}
