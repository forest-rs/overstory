// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use crate::channels::{ARRANGE, COMPOSE, GEOMETRY, LAYOUT, PAINT};
use crate::pseudos::{
    PSEUDO_CHECKED, PSEUDO_DISABLED, PSEUDO_FOCUS_VISIBLE, PSEUDO_FOCUSED, PSEUDO_HOVER,
    PSEUDO_PRESSED, PSEUDO_SELECTED,
};
use crate::test_support::{pointer_down, pointer_leave, pointer_move, pointer_scroll, pointer_up};
use crate::{
    BindingOwner, CompositionPolicy, ControlTemplateSpecExt, ElementRef, ElementRefSpecExt,
    ElementSpecExt, FocusChangeReason, KeyDispatchResult, Layout, LayoutPlacement, Navigation,
    Phase, PointerEventKind, RoutedEventKind, RoutedHandlerOptions, StyleTokenSet, StyleVocabulary,
    SurfaceSpecExt, SurfaceStyleExt, TYPE_GENERIC, TemplateContext, TemplateSlot, TooltipSpecExt,
    UiResolveLookup,
};
use alloc::boxed::Box;
use alloc::string::String;
use core::time::Duration;
use frameclock::FrameDemand;
use imaging::record::{Command, Scene};
use kurbo::{Affine, Point, Rect, Size, Vec2};
use peniko::color::{ColorSpaceTag, DynamicColor, HueDirection, Srgb};
use peniko::{Brush, Color};
use understory_property::{DependencyObject, DependencyObjectExt};
use understory_style::{
    ClassId, PartTag, ResolveCx, ResourceKey, SelectorBuilder, SelectorStep, StyleBuilder,
    StyleCascadeBuilder, StyleOrigin, ThemeBuilder,
};

use crate::motion::{
    ColorInterpolation, EffectClip, EffectKeyframe, EffectRule, EffectRules, EffectTriggerId,
    Interpolate, PresentationSlot, PresentationTarget, PresentationValue, TimingFunction,
    TransitionRule, TransitionRules, TransitionSpec, millis,
};

mod content_presenter;
mod events;
mod invalidation;
mod layout_motion;

#[test]
fn new_ui_has_root() {
    let ui = Ui::new();
    let root = ui.root();
    assert!(ui.get(root).is_some());
    assert_eq!(ui.len(), 1);
    assert_eq!(ui.get(root).unwrap().parent(), None);
    assert!(ui.get(root).unwrap().children().is_empty());
}

#[test]
fn insert_child_appends_to_parent_and_marks_dirty() {
    let mut ui = Ui::new();
    let root = ui.root();
    let a = ui.insert_child(root);
    let b = ui.insert_child(root);

    assert_eq!(ui.len(), 3);
    let root_children = ui.get(root).unwrap().children();
    assert_eq!(root_children, &[a, b]);
    assert_eq!(ui.get(a).unwrap().parent(), Some(root));
    assert_eq!(ui.get(b).unwrap().parent(), Some(root));

    assert!(ui.invalidation().is_invalidated(a, PAINT));
    assert!(ui.invalidation().is_invalidated(b, PAINT));
    assert!(ui.invalidation().is_invalidated(root, COMPOSE));
}

#[test]
fn child_ui_adds_children_under_one_parent() {
    let mut ui = Ui::new();
    let root = ui.root();

    let (panel, (label, nested, nested_label)) =
        ui.children(root, crate::Panel::column(4.0), |children| {
            let label = children.add(crate::Text::new("Scoped"));
            let (nested, nested_label) = children.children(crate::Panel::row(2.0), |children| {
                children.add(crate::Text::new("Nested"))
            });
            (label, nested, nested_label)
        });

    assert_eq!(ui.get(root).unwrap().children(), &[panel]);
    assert_eq!(ui.get(panel).unwrap().children(), &[label, nested]);
    assert_eq!(ui.get(nested).unwrap().children(), &[nested_label]);
    assert_eq!(ui.get(label).unwrap().parent(), Some(panel));
    assert_eq!(ui.get(nested_label).unwrap().parent(), Some(nested));
}

#[test]
fn child_ui_with_child_returns_nested_result_without_owner_tuple() {
    let mut ui = Ui::new();
    let root = ui.root();

    let (label, nested_label) = ui.with_child(root, crate::Panel::column(4.0), |panel| {
        let label = panel.add(crate::Text::new("Scoped"));
        let nested_label = panel.with_child(crate::Panel::row(2.0), |row| {
            row.add(crate::Text::new("Nested"))
        });
        (label, nested_label)
    });
    let panel = ui.get(root).unwrap().children()[0];
    let nested = ui.get(panel).unwrap().children()[1];

    assert_eq!(ui.get(root).unwrap().children(), &[panel]);
    assert_eq!(ui.get(panel).unwrap().children(), &[label, nested]);
    assert_eq!(ui.get(nested).unwrap().children(), &[nested_label]);
}

#[test]
fn child_ui_add_with_children_returns_inserted_owner_id() {
    let mut ui = Ui::new();
    let root = ui.root();

    let panel = ui.add_with_children(root, crate::Panel::column(4.0), |panel| {
        panel.add(crate::Text::new("Child"));
    });

    assert_eq!(ui.get(root).unwrap().children(), &[panel]);
    assert_eq!(ui.get(panel).unwrap().children().len(), 1);
}

#[test]
fn child_ui_add_with_configures_inserted_element() {
    let mut ui = Ui::new();
    let root = ui.root();

    let button = ui.with_children(root, |root| {
        root.add_with(crate::Button::new("Apply"), |ui, id| {
            ui.add_class(id, ClassId(42));
        })
    });

    assert!(ui.get(button).unwrap().classes().contains(&ClassId(42)));
}

#[test]
fn child_ui_panel_helpers_keep_parentage_lexical() {
    let mut ui = Ui::new();
    let root = ui.root();

    let (title, row, action, flag, progress) = ui.with_children(root, |root| {
        let title = root.add(crate::Text::new("Controls"));
        let (row, (action, flag, progress)) = root.row(6.0, |row| {
            let action = row.add(crate::Button::new("Apply"));
            let flag = row.add(crate::CheckBox::new("Enabled"));
            let progress = row.add(crate::ProgressBar::new());
            (action, flag, progress)
        });
        (title, row, action, flag, progress)
    });

    assert_eq!(ui.get(root).unwrap().children(), &[title, row]);
    assert_eq!(ui.get(row).unwrap().children(), &[action, flag, progress]);
    assert_eq!(ui.get(action).unwrap().parent(), Some(row));
    assert_eq!(ui.get(flag).unwrap().parent(), Some(row));
    assert_eq!(ui.get(progress).unwrap().parent(), Some(row));
}

#[test]
fn child_ui_view_builds_reusable_authoring_units() {
    struct StatusView(&'static str);

    impl crate::View for StatusView {
        type Output = ElementId;

        fn build(self, ui: &mut ChildUi<'_>) -> Self::Output {
            ui.column(4.0, |status| {
                status.add(crate::Text::new("Status"));
                status.add(crate::Text::new(self.0))
            })
            .1
        }
    }

    let mut ui = Ui::new();
    let root = ui.root();
    let value = ui.view(root, StatusView("Ready"));
    let panel = ui.get(root).unwrap().children()[0];

    assert_eq!(ui.get(root).unwrap().children(), &[panel]);
    assert_eq!(ui.get(panel).unwrap().children().len(), 2);
    assert_eq!(ui.get(value).unwrap().parent(), Some(panel));
}

#[test]
fn authoring_view_example_builds_and_binds_with_real_api() {
    use crate::{BindingSpecExt, RangeSpecExt};
    use alloc::format;

    struct DeploymentView;

    struct DeploymentElements {
        card: ElementId,
        deploy: ElementId,
        count: crate::LocalState<u32>,
        count_label: ElementId,
    }

    impl crate::View for DeploymentView {
        type Output = DeploymentElements;

        fn build(self, ui: &mut ChildUi<'_>) -> Self::Output {
            let (card, (deploy, count, count_label)) =
                ui.children(crate::Panel::column(16.0), |card| {
                    let deploy_count = card.local_state(0_u32);

                    card.add(crate::Text::new("Deployment").font_size(18.0));
                    card.with_child(crate::Panel::row(12.0).align_center(), |row| {
                        row.add(crate::ToggleSwitch::new().checked(true));
                        row.add(crate::Text::new("Live rollout"));
                    });

                    card.add_with_children(crate::Panel::column(6.0), |status| {
                        status.add(crate::Text::new("Rollout status"));
                        status.add(crate::Slider::new().range(0.0, 100.0).value(42.0));
                    });

                    let (_, count_label) = card.content(
                        crate::Expander::new("Advanced").expanded(true),
                        |advanced| {
                            advanced.add(crate::CheckBox::new("Require review").checked(true));
                            advanced.add(crate::CheckBox::new("Attach timing traces"));
                            advanced.add(crate::Text::new("").bind_text(
                                deploy_count.map(|count| format!("Deploy attempts: {count}")),
                            ))
                        },
                    );
                    let count_label =
                        count_label.expect("default expander template exposes content slot");

                    let deploy =
                        card.with_child(crate::Panel::row(10.0).align_center(), |actions| {
                            actions.add(crate::Button::new("Deploy").on_click(move |ui, _event| {
                                deploy_count.update(ui, |count| *count += 1);
                            }))
                        });

                    (deploy, deploy_count, count_label)
                });

            DeploymentElements {
                card,
                deploy,
                count,
                count_label,
            }
        }
    }

    fn resolved_text(ui: &Ui, id: ElementId) -> Option<String> {
        ui.get(id)
            .unwrap()
            .property_store()
            .effective_local_erased(ui.properties().text.id())
            .and_then(|value| value.downcast_ref::<String>().cloned())
    }

    let mut ui = Ui::new();
    let root = ui.root();
    let elements = ui.view(root, DeploymentView);

    assert_eq!(ui.get(root).unwrap().children(), &[elements.card]);
    assert!(ui.get(elements.deploy).unwrap().parent().is_some());

    ui.flush_bindings();
    assert_eq!(
        resolved_text(&ui, elements.count_label).as_deref(),
        Some("Deploy attempts: 0")
    );

    assert!(elements.count.set(&mut ui, 3));
    ui.flush_bindings();
    assert_eq!(
        resolved_text(&ui, elements.count_label).as_deref(),
        Some("Deploy attempts: 3")
    );
}

#[test]
fn child_ui_content_scopes_children_to_declared_slot() {
    let mut ui = Ui::new();
    let root = ui.root();

    let (expander, nested) = ui.content(root, crate::Expander::new("Advanced"), |content| {
        content.add(crate::Text::new("Nested"))
    });
    let nested = nested.expect("default expander template exposes content slot");
    let content = crate::expander_content(&ui, expander).expect("content slot should exist");

    assert_eq!(ui.get(root).unwrap().children(), &[expander]);
    assert_eq!(ui.get(content).unwrap().children(), &[nested]);
    assert_eq!(ui.get(nested).unwrap().parent(), Some(content));
}

#[test]
fn element_ref_records_inserted_element_and_ignores_stale_ids() {
    let mut ui = Ui::new();
    let root = ui.root();
    let action = ElementRef::<crate::Button>::new();

    let button = ui.append(root, crate::Button::new("Apply").reference(action.clone()));

    assert_eq!(action.current_id(), Some(button));
    assert_eq!(action.resolve(&ui), Some(button));

    ui.remove(button);

    assert_eq!(action.current_id(), Some(button));
    assert_eq!(action.resolve(&ui), None);
    assert!(!action.is_live(&ui));
}

#[test]
fn element_ref_can_write_properties_without_raw_element_id() {
    let mut ui = Ui::new();
    let root = ui.root();
    let label = ElementRef::<crate::Text>::new();

    ui.append(root, crate::Text::new("Before").reference(label.clone()));
    let text = ui.properties().text;

    label.set_local(&mut ui, text, String::from("After"));

    let id = label.resolve(&ui).expect("label ref should resolve");
    assert_eq!(
        ui.resolve_value_for(ui.get(id).unwrap(), text),
        String::from("After")
    );
}

#[test]
fn element_ref_can_clear_properties_without_raw_element_id() {
    let mut ui = Ui::new();
    let root = ui.root();
    let label = ElementRef::<crate::Text>::new();

    ui.append(root, crate::Text::new("Before").reference(label.clone()));
    let font_size = ui.properties().font_size;

    label.set_local(&mut ui, font_size, 24.0);
    assert!(label.clear_local(&mut ui, font_size));

    let id = label.resolve(&ui).expect("label ref should resolve");
    assert_eq!(
        ui.get(id).unwrap().get_local_value(font_size),
        None,
        "clear_local should remove the ref-targeted local override",
    );
}

#[test]
fn tooltip_spec_ext_attaches_tooltip_during_append() {
    let mut ui = Ui::new();
    let root = ui.root();

    let button = ui.append(
        root,
        crate::Button::new("Apply").tooltip("Runs the workflow"),
    );
    ui.set_layout(root, Layout::new().logical_size(240.0, 120.0));
    ui.set_layout(button, Layout::new().logical_size(80.0, 30.0));
    ui.compute_layout(Size::new(240.0, 120.0));
    let result = ui.dispatch_pointer_event(&pointer_move(Point::new(20.0, 12.0), 1, false));

    assert!(result.changed());
    assert_eq!(
        ui.overlays.active_tooltip().map(|active| active.target),
        Some(button)
    );
}

#[test]
fn remove_drops_subtree_and_invalidates_handles() {
    let mut ui = Ui::new();
    let root = ui.root();
    let a = ui.insert_child(root);
    let grandchild = ui.insert_child(a);
    let b = ui.insert_child(root);

    ui.remove(a);

    assert!(ui.get(a).is_none());
    assert!(ui.get(grandchild).is_none());
    assert!(ui.get(b).is_some());
    assert_eq!(ui.get(root).unwrap().children(), &[b]);
    assert_eq!(ui.len(), 2);
}

#[test]
fn slot_reuse_invalidates_old_handle() {
    let mut ui = Ui::new();
    let root = ui.root();
    let a = ui.insert_child(root);
    ui.remove(a);

    let c = ui.insert_child(root);
    assert_eq!(c.slot(), a.slot());
    assert_ne!(c.generation(), a.generation());
    assert!(ui.get(a).is_none());
    assert!(ui.get(c).is_some());
}

#[test]
#[should_panic(expected = "cannot remove the tree root")]
fn remove_root_panics() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.remove(root);
}

#[test]
#[should_panic(expected = "parent id is stale")]
fn insert_child_with_stale_parent_panics() {
    let mut ui = Ui::new();
    let root = ui.root();
    let a = ui.insert_child(root);
    ui.remove(a);
    let _ = ui.insert_child(a);
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
    let selector = SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC)).build();
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
fn styled_surface_inset_properties_invalidate_layout() {
    const PADDED: ClassId = ClassId(0xB0D);

    let mut ui = Ui::new();
    let root = ui.root();
    let style = StyleBuilder::new()
        .set(ui.properties().surface.padding_widths.left, 12.0_f64)
        .build();
    let selector =
        SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC).with_class(PADDED)).build();
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
        SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC).with_class(SHAPED)).build();
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

#[test]
fn styled_background_resolves_through_cascade() {
    // A style for `Type(TYPE_GENERIC)` should supply the
    // background value for elements that don't have a local override.
    let mut ui = Ui::new();
    let root = ui.root();
    let layout_prop = ui.properties().layout;
    let background = ui.properties().surface.background;

    // Build a cascade with one rule.
    let red = Some(Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff)));
    let style = StyleBuilder::new().set(background, red).build();
    let selector = SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC)).build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(StyleOrigin::Base, selector, style)
        .build();
    ui.set_cascade(cascade);

    ui.set_local(root, layout_prop, Layout::new().logical_size(10.0, 10.0));
    ui.compute_layout(Size::new(100.0, 100.0));

    let mut output = Scene::new();
    ui.paint(&mut output);
    // Resolution should have surfaced the styled red brush, so the scene
    // is non-empty.
    assert!(
        !output.is_empty(),
        "cascade should supply the background and produce paint output"
    );
}

#[test]
fn three_rule_cascade_picks_class_over_type() {
    // Mirror the demo: rules at Base origin keyed on TYPE_GENERIC.
    use understory_style::{ClassId, SelectorBuilder, SelectorStep};

    const CLASS_FG: ClassId = ClassId(1);

    let mut ui = Ui::new();
    let root = ui.root();
    let fg = ui.insert_child(root);
    ui.add_class(fg, CLASS_FG);

    let dark = Some(Brush::Solid(Color::from_rgba8(0x1e, 0x1e, 0x24, 0xff)));
    let blue = Some(Brush::Solid(Color::from_rgba8(0x6b, 0x9d, 0xff, 0xff)));
    let bg_prop = ui.properties().surface.background;

    let generic = StyleBuilder::new().set(bg_prop, dark.clone()).build();
    let foreground = StyleBuilder::new().set(bg_prop, blue.clone()).build();

    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC)).build(),
            generic,
        )
        .push_rule(
            StyleOrigin::Base,
            SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC).with_class(CLASS_FG)).build(),
            foreground,
        )
        .build();
    ui.set_cascade(cascade);

    // Confirm the class survived set_cascade (which clears match_states).
    assert_eq!(
        ui.get(fg).unwrap().classes(),
        &[CLASS_FG],
        "class should survive set_cascade"
    );

    let lookup = UiResolveLookup::new(&ui);
    let cx = ResolveCx::new(ui.registry(), ui.theme(), lookup);

    // Root: only the generic rule matches → dark.
    let root_el = ui.get(root).unwrap();
    let resolved_root: Option<Brush> =
        cx.get_value(root_el, bg_prop, Some((ui.cascade(), ui.match_state(root))));
    assert_eq!(resolved_root, dark, "root should resolve to dark");

    // Foreground: both rules match; the class-bearing rule has higher
    // specificity → blue.
    let fg_el = ui.get(fg).unwrap();
    let resolved_fg: Option<Brush> =
        cx.get_value(fg_el, bg_prop, Some((ui.cascade(), ui.match_state(fg))));
    assert_eq!(resolved_fg, blue, "foreground should resolve to blue");

    // The class survived set_cascade (which clears match_states).
    assert_eq!(
        ui.get(fg).unwrap().classes(),
        &[CLASS_FG],
        "class should survive set_cascade"
    );
}

#[test]
fn styled_button_foreground_reaches_template_label() {
    use crate::{Button, PART_BUTTON_LABEL, TYPE_BUTTON, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    let root = ui.root();
    let button = ui.append(root, Button::new("Run"));
    let label = ui.part(button, PART_BUTTON_LABEL).unwrap();

    let white = Brush::Solid(Color::from_rgba8(0xff, 0xff, 0xff, 0xff));
    let style = StyleBuilder::new()
        .set_resource(ui.properties().foreground, theme::ON_ACCENT_FG)
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            SelectorBuilder::new(SelectorStep::type_tag(TYPE_BUTTON)).build(),
            style,
        )
        .build();
    ui.set_cascade(cascade);

    let button_fg = ui.resolve_value_for(ui.get(button).unwrap(), ui.properties().foreground);
    assert_eq!(
        button_fg, white,
        "button itself should resolve styled foreground"
    );
    let label_fg = ui.resolve_value_for(ui.get(label).unwrap(), ui.properties().foreground);
    assert_eq!(
        label_fg, white,
        "label should inherit the button's styled foreground"
    );

    let mut output = Scene::new();
    ui.paint(&mut output);

    let label_node = ui.box_for(label).expect("label should have a box node");
    let foreground = ui
        .presentation
        .presentation(label_node)
        .expect("label should have presentation")
        .plain_text()
        .expect("label should present text")
        .foreground
        .clone();
    assert_eq!(foreground, Some(white));
}

#[test]
fn default_cascade_supplies_control_font_sizes() {
    use crate::controls::list::realized_list_item;
    use crate::{
        Button, CheckBox, ListBox, PART_BUTTON_LABEL, PART_CHECK_BOX_LABEL, SLOT_LIST_ITEM_CONTENT,
        SLOT_TEXT_INPUT_TEXT, SLOT_TOOLTIP_CONTENT, TextInput, Tooltip, theme,
    };

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    let font_size = ui.properties().font_size;

    let button = ui.append(root, Button::new("Run"));
    let button_label = ui.part(button, PART_BUTTON_LABEL).unwrap();
    let checkbox = ui.append(root, CheckBox::new("Enabled"));
    let checkbox_label = ui.part(checkbox, PART_CHECK_BOX_LABEL).unwrap();
    let listbox = ui.append(root, ListBox::new().item("Row").size(120.0, 40.0));
    ui.compute_layout(Size::new(400.0, 200.0));
    let list_item = realized_list_item(&ui, listbox, 0).unwrap();
    let list_item_label = ui.slot(list_item, SLOT_LIST_ITEM_CONTENT).unwrap();
    let text_input = ui.append(root, TextInput::new());
    let text_input_label = ui.slot(text_input, SLOT_TEXT_INPUT_TEXT).unwrap();
    let tooltip = ui.append(root, Tooltip::new("Tip"));
    let tooltip_label = ui.slot(tooltip, SLOT_TOOLTIP_CONTENT).unwrap();

    for id in [button, checkbox, listbox, list_item, text_input, tooltip] {
        assert!(
            ui.get(id).unwrap().get_local_value(font_size).is_none(),
            "default builder should leave font size to styling"
        );
    }

    assert_eq!(
        ui.resolve_value_for(ui.get(button_label).unwrap(), font_size),
        22.0
    );
    assert_eq!(
        ui.resolve_value_for(ui.get(checkbox_label).unwrap(), font_size),
        15.0
    );
    assert_eq!(
        ui.resolve_value_for(ui.get(list_item_label).unwrap(), font_size),
        14.0
    );
    assert_eq!(
        ui.resolve_value_for(ui.get(text_input_label).unwrap(), font_size),
        15.0
    );
    assert_eq!(
        ui.resolve_value_for(ui.get(tooltip_label).unwrap(), font_size),
        13.0
    );

    let explicit = ui.append(root, Button::new("Small").font_size(12.0));
    assert_eq!(
        ui.get(explicit)
            .unwrap()
            .get_local_value(font_size)
            .copied(),
        Some(12.0)
    );
}

#[test]
fn text_measure_uses_styled_text_and_font_size() {
    use crate::{TYPE_TEXT, Text};
    use alloc::string::String;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_local(
        root,
        ui.properties().layout,
        Layout::flex().logical_size(200.0, 100.0),
    );
    let text = ui.append(root, Text::new(""));

    let style = StyleBuilder::new()
        .set(ui.properties().text, String::from("Styled text"))
        .set(ui.properties().font_size, 40.0_f64)
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            SelectorBuilder::new(SelectorStep::type_tag(TYPE_TEXT)).build(),
            style,
        )
        .build();
    ui.set_cascade(cascade);

    ui.compute_layout(Size::new(200.0, 100.0));

    let bounds = ui.effective_bounds(text);
    assert!(
        bounds.width() > 0.0 && bounds.height() > 30.0,
        "styled text and font size should participate in measure; got {bounds:?}"
    );
}

#[test]
fn text_measure_uses_inherited_styled_font_size() {
    use crate::Text;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_local(
        root,
        ui.properties().layout,
        Layout::flex().logical_size(200.0, 100.0),
    );
    let text = ui.append(root, Text::new("Inherited font"));

    let style = StyleBuilder::new()
        .set(ui.properties().font_size, 40.0_f64)
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC)).build(),
            style,
        )
        .build();
    ui.set_cascade(cascade);

    ui.compute_layout(Size::new(200.0, 100.0));

    let bounds = ui.effective_bounds(text);
    assert!(
        bounds.height() > 30.0,
        "inherited styled font size should participate in measure; got {bounds:?}"
    );
}

#[test]
fn repeated_text_widgets_share_shaped_layout_cache() {
    use crate::Text;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(4.0).logical_size(240.0, 120.0));

    ui.append(root, Text::new("Shared label").font_size(14.0));
    ui.append(root, Text::new("Shared label").font_size(14.0));
    ui.append(root, Text::new("Shared label").font_size(14.0));

    ui.compute_layout(Size::new(240.0, 120.0));

    let stats = ui.text_layout_cache_stats();
    assert_eq!(stats.misses, 1);
    assert!(
        stats.hits >= 2,
        "expected repeated labels to hit cache: {stats:?}"
    );
    assert_eq!(ui.text_layout_cache_len(), 1);
}

#[test]
fn text_foreground_repaint_reuses_shaped_layout() {
    use crate::Text;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(4.0).logical_size(240.0, 120.0));
    let text = ui.append(root, Text::new("Shared label").font_size(14.0));

    ui.compute_layout(Size::new(240.0, 120.0));
    let mut first = Scene::new();
    ui.paint(&mut first);
    let before = ui.text_layout_cache_stats();

    ui.set_local(
        text,
        ui.properties().foreground,
        Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff)),
    );
    assert!(!ui.invalidation().has_invalidated(LAYOUT));
    assert!(ui.invalidation().has_invalidated(PAINT));

    let mut second = Scene::new();
    ui.paint(&mut second);
    let after = ui.text_layout_cache_stats();

    assert_eq!(after.misses, before.misses);
    assert!(after.hits > before.hits);
}

#[test]
fn paint_keeps_default_text_on_one_line() {
    use crate::Text;
    use imaging::record::{Command, Draw};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_local(
        root,
        ui.properties().layout,
        Layout::flex().logical_size(80.0, 200.0),
    );
    let text = ui.append(
        root,
        Text::new("alpha beta gamma delta epsilon").font_size(20.0),
    );
    ui.set_layout(text, Layout::new().logical_size(80.0, 200.0));

    ui.compute_layout(Size::new(80.0, 200.0));

    let mut output = Scene::new();
    ui.paint(&mut output);

    let mut saw_glyph_run = false;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for command in output.commands() {
        let Command::Draw(id) = command else {
            continue;
        };
        let Draw::GlyphRun(run) = output.draw_op(*id) else {
            continue;
        };
        saw_glyph_run = true;
        assert!(run.hint, "UI text glyph runs should enable hinting");
        for glyph in &run.glyphs {
            min_y = min_y.min(glyph.y);
            max_y = max_y.max(glyph.y);
        }
    }

    assert!(saw_glyph_run, "text should paint at least one glyph run");
    assert!(
        max_y - min_y < 20.0,
        "default text should paint on one line; glyph y span was {}",
        max_y - min_y
    );
}

#[test]
fn paint_wraps_opted_in_text_to_element_bounds() {
    use crate::Text;
    use imaging::record::{Command, Draw};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_local(
        root,
        ui.properties().layout,
        Layout::flex().logical_size(80.0, 200.0),
    );
    let text = ui.append(
        root,
        Text::new("alpha beta gamma delta epsilon")
            .font_size(20.0)
            .wrap(),
    );
    ui.set_layout(text, Layout::new().logical_size(80.0, 200.0));

    ui.compute_layout(Size::new(80.0, 200.0));

    let bounds = ui.effective_bounds(text);
    assert!(
        bounds.width() <= 80.0,
        "text should be laid out within the constrained width; got {bounds:?}"
    );

    let mut output = Scene::new();
    ui.paint(&mut output);

    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for command in output.commands() {
        let Command::Draw(id) = command else {
            continue;
        };
        let Draw::GlyphRun(run) = output.draw_op(*id) else {
            continue;
        };
        for glyph in &run.glyphs {
            min_y = min_y.min(glyph.y);
            max_y = max_y.max(glyph.y);
        }
    }

    assert!(
        max_y - min_y > 20.0,
        "paint should render wrapped text lines; glyph y span was {}",
        max_y - min_y
    );
}

#[test]
fn row_text_uses_natural_width_until_width_is_chosen() {
    use crate::{Panel, Text};
    use imaging::record::{Command, Draw};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::row(12.0).logical_size(284.0, 80.0));

    let spacer = ui.append(root, Panel::new());
    ui.set_layout(spacer, Layout::new().logical_size(64.0, 32.0));
    let label = ui.append(root, Text::new("Live rendering").font_size(15.0));

    ui.compute_layout(Size::new(284.0, 80.0));

    let label_bounds = ui.effective_bounds(label);
    assert!(
        label_bounds.width() > 75.0,
        "row label should keep its natural width instead of measuring wrapped; got {label_bounds:?}"
    );

    let mut output = Scene::new();
    ui.paint(&mut output);

    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for command in output.commands() {
        let Command::Draw(id) = command else {
            continue;
        };
        let Draw::GlyphRun(run) = output.draw_op(*id) else {
            continue;
        };
        for glyph in &run.glyphs {
            min_y = min_y.min(glyph.y);
            max_y = max_y.max(glyph.y);
        }
    }

    assert!(
        max_y - min_y < 20.0,
        "row label should paint on one line; glyph y span was {}",
        max_y - min_y
    );
}

#[test]
fn control_row_labels_keep_single_line_widths() {
    use crate::{Spinner, Text, ToggleSwitch, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    let root = ui.root();
    ui.set_layout(root, Layout::column(14.0).logical_size(320.0, 350.0));
    ui.set_box_padding(root, understory_presentation::Edges::all(18.0));

    let toggle_row = ui.append(root, crate::Panel::row(12.0).align_center());
    ui.append(toggle_row, ToggleSwitch::new().checked(true));
    let toggle_label = ui.append(toggle_row, Text::new("Live rendering").font_size(15.0));

    let spinner_row = ui.append(root, crate::Panel::row(12.0).align_center());
    ui.append(spinner_row, Spinner::new());
    let spinner_label = ui.append(spinner_row, Text::new("Streaming updates").font_size(13.0));

    ui.compute_layout(Size::new(320.0, 350.0));

    let toggle_style = understory_presentation::TextStyle {
        font_size: 15.0,
        ..understory_presentation::TextStyle::default()
    };
    let spinner_style = understory_presentation::TextStyle {
        font_size: 13.0,
        ..understory_presentation::TextStyle::default()
    };
    let toggle_natural_width = f64::from(
        ui.text
            .borrow_mut()
            .shape("Live rendering", &toggle_style, None)
            .width(),
    );
    let spinner_natural_width = f64::from(
        ui.text
            .borrow_mut()
            .shape("Streaming updates", &spinner_style, None)
            .width(),
    );

    let toggle_bounds = ui.effective_bounds(toggle_label);
    let spinner_bounds = ui.effective_bounds(spinner_label);
    assert!(
        toggle_bounds.width() >= toggle_natural_width - 1.0,
        "toggle label should retain natural width; natural={toggle_natural_width}, bounds={toggle_bounds:?}"
    );
    assert!(
        spinner_bounds.width() >= spinner_natural_width - 1.0,
        "spinner label should retain natural width; natural={spinner_natural_width}, bounds={spinner_bounds:?}"
    );

    let mut output = Scene::new();
    ui.paint(&mut output);

    let mut max_span = 0.0_f32;
    for command in output.commands() {
        let Command::Draw(id) = command else {
            continue;
        };
        let imaging::record::Draw::GlyphRun(run) = output.draw_op(*id) else {
            continue;
        };
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for glyph in &run.glyphs {
            min_y = min_y.min(glyph.y);
            max_y = max_y.max(glyph.y);
        }
        if min_y.is_finite() && max_y.is_finite() {
            max_span = max_span.max(max_y - min_y);
        }
    }
    assert!(
        max_span < 20.0,
        "control row labels should paint on one line; max glyph y span was {max_span}"
    );
}

#[test]
fn text_style_resolution_carries_font_feature_settings() {
    let mut ui = Ui::new();
    let root = ui.root();
    let text = ui.append(root, crate::Text::new("Variable"));
    let variations: Box<[parley::FontVariation]> = Box::from([parley::FontVariation::new(
        parley::setting::Tag::new(b"opsz"),
        12.0,
    )]);
    let features: Box<[parley::FontFeature]> = Box::from([parley::FontFeature::new(
        parley::setting::Tag::new(b"kern"),
        0,
    )]);

    ui.set_local(text, ui.properties().font_variations, variations.clone());
    ui.set_local(text, ui.properties().font_features, features.clone());

    let element = ui.get(text).unwrap();
    let style = crate::controls::text::text_style(element, &ui);

    assert_eq!(style.font_variations.as_ref(), variations.as_ref());
    assert_eq!(style.font_features.as_ref(), features.as_ref());
}

#[test]
fn slider_initial_value_positions_thumb() {
    use crate::Slider;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_local(
        root,
        ui.properties().layout,
        Layout::flex().logical_size(300.0, 80.0),
    );

    let slider = ui.append(root, Slider::new().value(0.4));
    let thumb = ui.part(slider, crate::PART_SLIDER_THUMB).unwrap();

    ui.compute_layout(Size::new(300.0, 80.0));

    let slider_bounds = ui.effective_bounds(slider);
    let thumb_bounds = ui.effective_bounds(thumb);
    let thumb_offset = thumb_bounds.x0 - slider_bounds.x0;
    assert!(
        thumb_offset > 70.0,
        "initial value should move thumb along the track; got x offset {thumb_offset}"
    );
}

#[test]
fn slider_snap_value_change_updates_layer_without_waiting_for_second_layout() {
    use crate::{SLOT_SLIDER_THUMB, Slider, set_slider_value};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(300.0, 80.0));
    let slider = ui.append(root, Slider::new().value(0.0));
    ui.compute_layout(Size::new(300.0, 80.0));
    let thumb = ui.slot(slider, SLOT_SLIDER_THUMB).unwrap();

    set_slider_value(&mut ui, slider, 1.0);
    ui.compute_layout(Size::new(300.0, 80.0));

    assert_close(layer_x(&ui, thumb), ui.effective_bounds(thumb).x0);
    assert_close(layer_y(&ui, thumb), ui.effective_bounds(thumb).y0);
}

#[test]
fn slider_value_binding_updates_thumb_layout() {
    use crate::{BindingSpecExt, SLOT_SLIDER_THUMB, Slider};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(520.0, 90.0));
    let value = ui.local_state(0.25_f64);
    let value_property = ui.properties().value;
    let first = ui.append(
        root,
        Slider::new().bind_property(value_property, value.binding()),
    );
    let second = ui.append(
        root,
        Slider::new().bind_property(value_property, value.binding()),
    );

    ui.compute_layout(Size::new(520.0, 90.0));
    let first_thumb = ui.slot(first, SLOT_SLIDER_THUMB).unwrap();
    let second_thumb = ui.slot(second, SLOT_SLIDER_THUMB).unwrap();
    let first_start = ui.effective_bounds(first_thumb).x0 - ui.effective_bounds(first).x0;
    let second_start = ui.effective_bounds(second_thumb).x0 - ui.effective_bounds(second).x0;
    assert_close(first_start, second_start);

    assert!(value.set(&mut ui, 0.75));
    ui.compute_layout(Size::new(520.0, 90.0));
    let first_after = ui.effective_bounds(first_thumb).x0 - ui.effective_bounds(first).x0;
    let second_after = ui.effective_bounds(second_thumb).x0 - ui.effective_bounds(second).x0;
    assert!(
        first_after > first_start + 50.0,
        "binding update should move the first slider thumb: {first_start} -> {first_after}",
    );
    assert_close(first_after, second_after);
}

#[test]
fn default_templates_label_named_parts() {
    use crate::{
        Button, PART_BUTTON_LABEL, PART_SLIDER_THUMB, PART_SLIDER_TRACK, PART_TOGGLE_THUMB,
        SLOT_BUTTON_CONTENT, SLOT_SLIDER_THUMB, SLOT_SLIDER_TRACK, SLOT_TOGGLE_THUMB, Slider,
        ToggleSwitch,
    };

    let mut ui = Ui::new();
    let root = ui.root();

    let button = ui.append(root, Button::new("Run"));
    let button_label = ui.part(button, PART_BUTTON_LABEL).unwrap();
    assert_eq!(
        ui.get(button_label).unwrap().part_tag(),
        Some(PART_BUTTON_LABEL)
    );
    assert_eq!(
        ui.template_instance(button)
            .unwrap()
            .part(PART_BUTTON_LABEL),
        Some(button_label)
    );
    assert_eq!(ui.slot(button, SLOT_BUTTON_CONTENT), Some(button_label));

    let slider = ui.append(root, Slider::new());
    let slider_track = ui.part(slider, PART_SLIDER_TRACK).unwrap();
    let slider_thumb = ui.part(slider, PART_SLIDER_THUMB).unwrap();
    assert_eq!(ui.slot(slider, SLOT_SLIDER_TRACK), Some(slider_track));
    assert_eq!(ui.slot(slider, SLOT_SLIDER_THUMB), Some(slider_thumb));
    assert_eq!(
        ui.get(slider_track).unwrap().part_tag(),
        Some(PART_SLIDER_TRACK)
    );
    assert_eq!(
        ui.get(slider_thumb).unwrap().part_tag(),
        Some(PART_SLIDER_THUMB)
    );

    let toggle = ui.append(root, ToggleSwitch::new());
    let toggle_thumb = ui.part(toggle, PART_TOGGLE_THUMB).unwrap();
    assert_eq!(ui.slot(toggle, SLOT_TOGGLE_THUMB), Some(toggle_thumb));
    assert_eq!(
        ui.get(toggle_thumb).unwrap().part_tag(),
        Some(PART_TOGGLE_THUMB)
    );
}

#[test]
fn toggle_thumb_transition_samples_composition_offset() {
    use crate::SLOT_TOGGLE_THUMB;

    let (mut ui, toggle, thumb) = animated_toggle_fixture();
    let start_x = layer_x(&ui, thumb);
    let start_y = layer_y(&ui, thumb);

    ui.set_now(0);
    ui.synthesize_click(toggle, ui.effective_bounds(toggle).center());
    ui.compute_layout(Size::new(200.0, 80.0));

    let target_x = ui.effective_bounds(thumb).x0;
    assert!(
        target_x > start_x,
        "checked layout target should move thumb to the right"
    );
    assert_close(layer_x(&ui, thumb), start_x);
    assert_close(layer_y(&ui, thumb), start_y);

    ui.set_now(millis(60));
    ui.flush_composition();
    assert_close(layer_x(&ui, thumb), thumb_timing(start_x, target_x, 0.5));
    assert_close(layer_y(&ui, thumb), start_y);

    ui.set_now(millis(120));
    ui.flush_composition();
    assert_close(layer_x(&ui, thumb), target_x);
    assert_close(layer_y(&ui, thumb), start_y);

    assert_eq!(ui.slot(toggle, SLOT_TOGGLE_THUMB), Some(thumb));
}

#[test]
fn toggle_thumb_transition_retargets_from_sampled_offset() {
    let (mut ui, toggle, thumb) = animated_toggle_fixture();
    let start_x = layer_x(&ui, thumb);

    ui.set_now(0);
    ui.synthesize_click(toggle, ui.effective_bounds(toggle).center());
    ui.compute_layout(Size::new(200.0, 80.0));
    let checked_target = ui.effective_bounds(thumb).x0;

    ui.set_now(millis(60));
    ui.compute_layout(Size::new(200.0, 80.0));
    let midpoint = layer_x(&ui, thumb);
    assert_close(midpoint, thumb_timing(start_x, checked_target, 0.5));

    ui.synthesize_click(toggle, ui.effective_bounds(toggle).center());
    ui.compute_layout(Size::new(200.0, 80.0));
    assert_close(layer_x(&ui, thumb), midpoint);

    ui.set_now(millis(120));
    ui.compute_layout(Size::new(200.0, 80.0));
    assert_close(layer_x(&ui, thumb), thumb_timing(midpoint, start_x, 0.5));

    ui.set_now(millis(180));
    ui.compute_layout(Size::new(200.0, 80.0));
    assert_close(layer_x(&ui, thumb), start_x);
}

#[test]
fn button_hover_transitions_background_color() {
    use crate::{Button, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(240.0, 80.0));
    let button = ui.append(root, Button::new("Hover"));
    ui.compute_layout(Size::new(240.0, 80.0));

    let idle = Color::from_rgba8(0x3a, 0x69, 0xd6, 0xff);
    let hover = Color::from_rgba8(0x4d, 0x81, 0xf2, 0xff);
    ui.set_now(0);
    let _ = ui.dispatch_pointer_event(&pointer_move(
        ui.effective_bounds(button).center(),
        0,
        false,
    ));
    ui.compute_layout(Size::new(240.0, 80.0));

    let mut scene = Scene::new();
    ui.paint(&mut scene);
    assert_eq!(element_background_color(&ui, button), Some(idle));

    ui.set_now(millis(45));
    ui.compute_layout(Size::new(240.0, 80.0));
    ui.paint(&mut scene);
    assert_eq!(
        element_background_color(&ui, button),
        Some(interpolate_srgb_color(idle, hover, 0.5))
    );

    ui.set_now(millis(90));
    ui.compute_layout(Size::new(240.0, 80.0));
    ui.paint(&mut scene);
    assert_eq!(element_background_color(&ui, button), Some(hover));
}

#[test]
fn styled_effect_rule_triggers_visual_translation_slot() {
    const EFFECTFUL: ClassId = ClassId(901);
    const TRIGGER: EffectTriggerId = EffectTriggerId::new(901);

    let mut ui = Ui::new();
    let root = ui.root();
    let effects = ui.properties().effects;
    ui.set_cascade(
        StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC).with_class(EFFECTFUL))
                    .build(),
                StyleBuilder::new()
                    .set(
                        effects,
                        EffectRules::single(EffectRule::new(
                            TRIGGER,
                            PresentationTarget::visual_translation(),
                            EffectClip::from_keyframes([
                                EffectKeyframe::new(0, PresentationValue::Offset2d(Vec2::ZERO)),
                                EffectKeyframe::new(
                                    millis(50),
                                    PresentationValue::Offset2d(Vec2::new(10.0, 0.0)),
                                ),
                                EffectKeyframe::new(
                                    millis(100),
                                    PresentationValue::Offset2d(Vec2::ZERO),
                                ),
                            ]),
                        )),
                    )
                    .build(),
            )
            .build(),
    );
    let panel = ui.append(root, crate::Panel::new().class(EFFECTFUL));

    ui.set_now(0);
    assert!(ui.trigger_effect(panel, TRIGGER));
    assert!(ui.invalidation().is_invalidated(panel, COMPOSE));

    ui.set_now(millis(50));
    ui.flush_composition();
    let translation = ui.presented_value(
        panel,
        PresentationTarget::slot(PresentationSlot::VisualTranslation),
        Vec2::ZERO,
    );
    assert_close(translation.x, 10.0);

    ui.set_now(millis(100));
    ui.flush_composition();
    let translation = ui.presented_value(
        panel,
        PresentationTarget::slot(PresentationSlot::VisualTranslation),
        Vec2::ZERO,
    );
    assert_close(translation.x, 0.0);
}

#[test]
fn styled_effect_rule_targets_property_animation_layer() {
    const EFFECTFUL: ClassId = ClassId(902);
    const TRIGGER: EffectTriggerId = EffectTriggerId::new(902);

    let mut ui = Ui::new();
    let root = ui.root();
    let opacity = ui.properties().opacity;
    let effects = ui.properties().effects;
    ui.set_cascade(
        StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC).with_class(EFFECTFUL))
                    .build(),
                StyleBuilder::new()
                    .set(
                        effects,
                        EffectRules::single(EffectRule::new(
                            TRIGGER,
                            PresentationTarget::property(opacity.id()),
                            EffectClip::from_keyframes([
                                EffectKeyframe::new(0, PresentationValue::Scalar32(1.0)),
                                EffectKeyframe::new(millis(50), PresentationValue::Scalar32(0.25)),
                                EffectKeyframe::new(millis(100), PresentationValue::Scalar32(1.0)),
                            ]),
                        )),
                    )
                    .build(),
            )
            .build(),
    );
    let panel = ui.append(root, crate::Panel::new().class(EFFECTFUL));

    ui.set_now(0);
    assert!(ui.trigger_effect(panel, TRIGGER));
    assert!(ui.invalidation().is_invalidated(panel, COMPOSE));

    ui.set_now(millis(50));
    ui.flush_composition();
    let element = ui.get(panel).expect("panel should exist");
    assert_close(f64::from(ui.resolve_value_for(element, opacity)), 0.25);

    ui.set_now(millis(100));
    ui.flush_composition();
    let element = ui.get(panel).expect("panel should exist");
    assert_close(f64::from(ui.resolve_value_for(element, opacity)), 1.0);
}

#[test]
fn styled_transition_rule_controls_button_hover_duration() {
    use crate::{Button, TYPE_BUTTON};

    const TEST_HOVER_DURATION: ResourceKey = ResourceKey::new(900);

    let mut ui = Ui::new();
    ui.set_theme(
        ThemeBuilder::new()
            .set(TEST_HOVER_DURATION, millis(180))
            .build(),
    );
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(240.0, 80.0));

    let idle = Color::from_rgba8(0x10, 0x20, 0x30, 0xff);
    let hover = Color::from_rgba8(0x50, 0x60, 0x70, 0xff);
    let background = ui.properties().surface.background;
    let transitions = ui.properties().transitions;
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            crate::selectors::type_tag(TYPE_BUTTON),
            StyleBuilder::new()
                .set(background, Some(Brush::Solid(idle)))
                .set(
                    transitions,
                    TransitionRules::single(TransitionRule::interaction(
                        PresentationTarget::property(background.id()),
                        TransitionSpec::linear(millis(90))
                            .with_duration_resource(TEST_HOVER_DURATION)
                            .with_color_interpolation(ColorInterpolation::SRGB),
                    )),
                )
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            crate::selectors::state(TYPE_BUTTON, PSEUDO_HOVER),
            StyleBuilder::new()
                .set(background, Some(Brush::Solid(hover)))
                .build(),
        )
        .build();
    ui.set_cascade(cascade);

    let button = ui.append(root, Button::new("Hover"));
    ui.set_layout(button, Layout::flex().logical_size(160.0, 48.0));
    ui.compute_layout(Size::new(240.0, 80.0));

    ui.set_now(0);
    let _ = ui.dispatch_pointer_event(&pointer_move(
        ui.effective_bounds(button).center(),
        0,
        false,
    ));
    ui.compute_layout(Size::new(240.0, 80.0));

    ui.set_now(millis(90));
    ui.compute_layout(Size::new(240.0, 80.0));
    let mut scene = Scene::new();
    ui.paint(&mut scene);
    assert_eq!(
        element_background_color(&ui, button),
        Some(interpolate_srgb_color(idle, hover, 0.5))
    );
}

#[test]
fn transition_rules_drive_registered_foreground_property() {
    const MOTION_TEXT: ClassId = ClassId(91);

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(240.0, 80.0));

    let idle = Color::from_rgba8(0xb0, 0xb8, 0xc8, 0xff);
    let selected = Color::from_rgba8(0x56, 0xd3, 0xff, 0xff);
    let foreground = ui.properties().foreground;
    let transitions = ui.properties().transitions;
    let selector =
        SelectorBuilder::new(SelectorStep::type_tag(crate::TYPE_TEXT).with_class(MOTION_TEXT))
            .build();
    let selected_selector = SelectorBuilder::new(
        SelectorStep::type_tag(crate::TYPE_TEXT)
            .with_class(MOTION_TEXT)
            .with_pseudo(PSEUDO_SELECTED),
    )
    .build();
    ui.set_cascade(
        StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                selector,
                StyleBuilder::new()
                    .set(foreground, Brush::Solid(idle))
                    .set(
                        transitions,
                        TransitionRules::single(TransitionRule::interaction(
                            PresentationTarget::property(foreground.id()),
                            TransitionSpec::linear(millis(100))
                                .with_color_interpolation(ColorInterpolation::SRGB),
                        )),
                    )
                    .build(),
            )
            .push_rule(
                StyleOrigin::Base,
                selected_selector,
                StyleBuilder::new()
                    .set(foreground, Brush::Solid(selected))
                    .build(),
            )
            .build(),
    );

    let text = ui.append(
        root,
        crate::Text::new("Foreground").class(MOTION_TEXT).layout(
            Layout::flex()
                .logical_size(180.0, 32.0)
                .alignment(crate::LayoutAlignment::new().align_items(crate::LayoutAlign::Center)),
        ),
    );
    ui.compute_layout(Size::new(240.0, 80.0));
    assert_eq!(element_foreground_color(&ui, text), Some(idle));

    ui.set_now(0);
    ui.add_pseudo(text, PSEUDO_SELECTED);
    ui.compute_layout(Size::new(240.0, 80.0));
    assert_eq!(element_foreground_color(&ui, text), Some(idle));

    ui.set_now(millis(50));
    ui.compute_layout(Size::new(240.0, 80.0));
    assert_eq!(
        element_foreground_color(&ui, text),
        Some(interpolate_srgb_color(idle, selected, 0.5))
    );

    ui.set_now(millis(100));
    ui.compute_layout(Size::new(240.0, 80.0));
    assert_eq!(element_foreground_color(&ui, text), Some(selected));
}

#[test]
fn interaction_transitions_track_descendant_effective_value_changes() {
    const MOTION_ROW: ClassId = ClassId(92);
    const MOTION_LABEL: ClassId = ClassId(93);

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(260.0, 80.0));

    let idle = Color::from_rgba8(0xd0, 0xd4, 0xdf, 0xff);
    let selected = Color::from_rgba8(0x60, 0x7f, 0xff, 0xff);
    let foreground = ui.properties().foreground;
    let transitions = ui.properties().transitions;
    let label_selector =
        SelectorBuilder::new(SelectorStep::type_tag(crate::TYPE_TEXT).with_class(MOTION_LABEL))
            .build();
    let selected_label_selector = understory_style::Selector::from([
        SelectorStep::type_tag(TYPE_GENERIC)
            .with_class(MOTION_ROW)
            .with_pseudo(PSEUDO_SELECTED),
        SelectorStep::type_tag(crate::TYPE_TEXT).with_class(MOTION_LABEL),
    ]);
    ui.set_cascade(
        StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                label_selector,
                StyleBuilder::new()
                    .set(foreground, Brush::Solid(idle))
                    .set(
                        transitions,
                        TransitionRules::single(TransitionRule::interaction(
                            PresentationTarget::property(foreground.id()),
                            TransitionSpec::linear(millis(100))
                                .with_color_interpolation(ColorInterpolation::SRGB),
                        )),
                    )
                    .build(),
            )
            .push_rule(
                StyleOrigin::Base,
                selected_label_selector,
                StyleBuilder::new()
                    .set(foreground, Brush::Solid(selected))
                    .build(),
            )
            .build(),
    );

    let row = ui.append(
        root,
        crate::Panel::new()
            .class(MOTION_ROW)
            .layout(Layout::flex().logical_size(220.0, 40.0)),
    );
    let label = ui.append(row, crate::Text::new("Descendant").class(MOTION_LABEL));
    ui.compute_layout(Size::new(260.0, 80.0));
    assert_eq!(element_foreground_color(&ui, label), Some(idle));

    ui.set_now(0);
    ui.add_pseudo(row, PSEUDO_SELECTED);
    ui.compute_layout(Size::new(260.0, 80.0));
    assert_eq!(element_foreground_color(&ui, label), Some(idle));

    ui.set_now(millis(50));
    ui.compute_layout(Size::new(260.0, 80.0));
    assert_eq!(
        element_foreground_color(&ui, label),
        Some(interpolate_srgb_color(idle, selected, 0.5))
    );

    ui.set_now(millis(100));
    ui.compute_layout(Size::new(260.0, 80.0));
    assert_eq!(element_foreground_color(&ui, label), Some(selected));
}

#[test]
fn inherited_timeline_property_samples_invalidate_descendants() {
    const MOTION_ROW: ClassId = ClassId(94);

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(260.0, 80.0));

    let idle = Color::from_rgba8(0xd0, 0xd4, 0xdf, 0xff);
    let selected = Color::from_rgba8(0x60, 0x7f, 0xff, 0xff);
    let foreground = ui.properties().foreground;
    let transitions = ui.properties().transitions;
    let row_selector =
        SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC).with_class(MOTION_ROW)).build();
    let selected_row_selector = SelectorBuilder::new(
        SelectorStep::type_tag(TYPE_GENERIC)
            .with_class(MOTION_ROW)
            .with_pseudo(PSEUDO_SELECTED),
    )
    .build();
    ui.set_cascade(
        StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Base,
                row_selector,
                StyleBuilder::new()
                    .set(foreground, Brush::Solid(idle))
                    .set(
                        transitions,
                        TransitionRules::single(TransitionRule::interaction(
                            PresentationTarget::property(foreground.id()),
                            TransitionSpec::linear(millis(100))
                                .with_color_interpolation(ColorInterpolation::SRGB),
                        )),
                    )
                    .build(),
            )
            .push_rule(
                StyleOrigin::Base,
                selected_row_selector,
                StyleBuilder::new()
                    .set(foreground, Brush::Solid(selected))
                    .build(),
            )
            .build(),
    );

    let row = ui.append(
        root,
        crate::Panel::new()
            .class(MOTION_ROW)
            .layout(Layout::flex().logical_size(220.0, 40.0)),
    );
    let label = ui.append(row, crate::Text::new("Inherited"));
    ui.compute_layout(Size::new(260.0, 80.0));

    ui.set_now(0);
    ui.add_pseudo(row, PSEUDO_SELECTED);
    ui.compute_layout(Size::new(260.0, 80.0));
    ui.invalidation.clear(PAINT);

    ui.set_now(millis(50));

    assert_eq!(
        element_foreground_color(&ui, label),
        Some(interpolate_srgb_color(idle, selected, 0.5))
    );
    assert!(ui.invalidation().is_invalidated(row, PAINT));
    assert!(ui.invalidation().is_invalidated(label, PAINT));
}

#[test]
fn local_background_change_after_transition_paints_new_target() {
    use crate::{Button, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(240.0, 80.0));
    let button = ui.append(root, Button::new("Hover"));
    ui.compute_layout(Size::new(240.0, 80.0));

    ui.set_now(0);
    let _ = ui.dispatch_pointer_event(&pointer_move(
        ui.effective_bounds(button).center(),
        0,
        false,
    ));
    ui.set_now(millis(90));
    let mut scene = Scene::new();
    ui.paint(&mut scene);

    let red = Color::from_rgba8(0xff, 0x00, 0x00, 0xff);
    ui.set_local(
        button,
        ui.properties().surface.background,
        Some(Brush::Solid(red)),
    );
    ui.paint(&mut scene);

    assert_eq!(element_background_color(&ui, button), Some(red));
}

#[test]
fn button_press_transitions_layer_opacity() {
    use crate::{Button, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(240.0, 80.0));
    let button = ui.append(root, Button::new("Press"));
    ui.compute_layout(Size::new(240.0, 80.0));

    let center = ui.effective_bounds(button).center();
    let _ = ui.dispatch_pointer_event(&pointer_down(center, 0));
    ui.compute_layout(Size::new(240.0, 80.0));
    assert_close(layer_opacity(&ui, button), 1.0);

    ui.set_now(millis(35));
    ui.compute_layout(Size::new(240.0, 80.0));
    assert_close(layer_opacity(&ui, button), 0.96);

    ui.set_now(millis(70));
    ui.compute_layout(Size::new(240.0, 80.0));
    assert_close(layer_opacity(&ui, button), 0.92);
}

#[test]
fn text_input_hover_transitions_border_brush_color() {
    use crate::{TextInput, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(260.0, 80.0));
    let input = ui.append(root, TextInput::new().text("Hover"));
    ui.compute_layout(Size::new(260.0, 80.0));

    let idle = Color::from_rgba8(0x62, 0x62, 0x70, 0xff);
    let hover = Color::from_rgba8(0x86, 0x91, 0xaa, 0xff);
    assert_eq!(element_border_color(&ui, input), Some(idle));

    ui.set_now(0);
    let _ = ui.dispatch_pointer_event(&pointer_move(ui.effective_bounds(input).center(), 0, false));
    ui.compute_layout(Size::new(260.0, 80.0));
    assert_eq!(element_border_color(&ui, input), Some(idle));

    ui.set_now(millis(45));
    ui.compute_layout(Size::new(260.0, 80.0));
    assert_eq!(
        element_border_color(&ui, input),
        Some(interpolate_srgb_color(idle, hover, 0.5))
    );

    ui.set_now(millis(90));
    ui.compute_layout(Size::new(260.0, 80.0));
    assert_eq!(element_border_color(&ui, input), Some(hover));
}

#[test]
fn text_input_focus_visible_transitions_border_width() {
    use crate::{TextInput, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(260.0, 80.0));
    let input = ui.append(root, TextInput::new().text("Focus"));
    ui.compute_layout(Size::new(260.0, 80.0));

    assert_close(element_border_width(&ui, input), 1.0);

    ui.set_now(0);
    assert!(ui.set_focus(Some(input)));
    ui.compute_layout(Size::new(260.0, 80.0));
    assert_close(element_border_width(&ui, input), 1.0);

    ui.set_now(millis(35));
    ui.compute_layout(Size::new(260.0, 80.0));
    assert_close(element_border_width(&ui, input), 1.5);

    ui.set_now(millis(70));
    ui.compute_layout(Size::new(260.0, 80.0));
    assert_close(element_border_width(&ui, input), 2.0);
}

#[test]
fn slider_keyboard_transitions_thumb_offset_but_drag_snaps() {
    use crate::{SLOT_SLIDER_THUMB, Slider, theme};
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(300.0, 80.0));
    let slider = ui.append(root, Slider::new().value(0.0).small_change(0.5));
    assert!(ui.set_focus(Some(slider)));
    ui.compute_layout(Size::new(300.0, 80.0));
    let thumb = ui.slot(slider, SLOT_SLIDER_THUMB).unwrap();
    let start_x = layer_x(&ui, thumb);
    let start_y = layer_y(&ui, thumb);

    ui.set_now(0);
    let right = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
    assert!(ui.dispatch_key(&right).handled);
    ui.compute_layout(Size::new(300.0, 80.0));
    let target_x = ui.effective_bounds(thumb).x0;
    assert!(target_x > start_x);
    assert_close(layer_x(&ui, thumb), start_x);
    assert_close(layer_y(&ui, thumb), start_y);

    ui.set_now(millis(45));
    ui.compute_layout(Size::new(300.0, 80.0));
    assert_close(layer_x(&ui, thumb), thumb_timing(start_x, target_x, 0.5));
    assert_close(layer_y(&ui, thumb), start_y);

    let drag_target = Point::new(target_x + 30.0, ui.effective_bounds(thumb).center().y);
    let _ = ui.dispatch_pointer_event(&pointer_down(ui.effective_bounds(thumb).center(), 100));
    let _ = ui.dispatch_pointer_event(&pointer_move(drag_target, 101, true));
    ui.compute_layout(Size::new(300.0, 80.0));
    assert_close(layer_x(&ui, thumb), ui.effective_bounds(thumb).x0);
    assert_close(layer_y(&ui, thumb), start_y);
}

#[test]
fn slider_thumb_snaps_without_styled_transition_rule() {
    use crate::controls::slider::{PART_SLIDER_THUMB, TYPE_SLIDER};
    use crate::{SLOT_SLIDER_THUMB, Slider};
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(300.0, 80.0));
    let style = StyleBuilder::new()
        .set(ui.properties().transitions, TransitionRules::new())
        .build();
    ui.set_cascade(
        StyleCascadeBuilder::new()
            .push_rule(
                StyleOrigin::Sheet,
                crate::selectors::part(crate::selectors::template_part(
                    TYPE_SLIDER,
                    PART_SLIDER_THUMB,
                )),
                style,
            )
            .build(),
    );
    let slider = ui.append(root, Slider::new().value(0.0).small_change(0.5));
    assert!(ui.set_focus(Some(slider)));
    ui.compute_layout(Size::new(300.0, 80.0));
    let thumb = ui.slot(slider, SLOT_SLIDER_THUMB).unwrap();
    let start_y = layer_y(&ui, thumb);

    ui.set_now(0);
    let right = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
    assert!(ui.dispatch_key(&right).handled);
    ui.compute_layout(Size::new(300.0, 80.0));

    assert_close(layer_x(&ui, thumb), ui.effective_bounds(thumb).x0);
    assert_close(layer_y(&ui, thumb), start_y);
}

fn animated_toggle_fixture() -> (Ui, ElementId, ElementId) {
    use crate::{SLOT_TOGGLE_THUMB, ToggleSwitch, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(200.0, 80.0));
    let toggle = ui.append(root, ToggleSwitch::new());
    ui.compute_layout(Size::new(200.0, 80.0));
    let thumb = ui
        .slot(toggle, SLOT_TOGGLE_THUMB)
        .expect("toggle template should create a thumb slot");
    (ui, toggle, thumb)
}

fn layer_x(ui: &Ui, thumb: ElementId) -> f64 {
    layer_origin(ui, thumb).x
}

fn layer_y(ui: &Ui, thumb: ElementId) -> f64 {
    layer_origin(ui, thumb).y
}

fn layer_origin(ui: &Ui, thumb: ElementId) -> Point {
    let layer = ui
        .composition
        .layer_for(thumb)
        .expect("thumb should have a composition layer");
    ui.composition_layers()
        .world_transform(layer)
        .transform_point(Point::ZERO)
        .expect("thumb layer transform should be affine")
}

fn layer_opacity(ui: &Ui, id: ElementId) -> f64 {
    let layer = ui
        .composition
        .layer_for(id)
        .expect("element should have a composition layer");
    f64::from(ui.composition_layers().local_opacity(layer))
}

fn element_background_color(ui: &Ui, id: ElementId) -> Option<Color> {
    let node = ui.box_for(id)?;
    let surface = ui.presentation.presentation(node)?.surface()?;
    let brush = &surface.backgrounds.first()?.brush;
    match brush {
        Brush::Solid(color) => Some(*color),
        _ => None,
    }
}

fn element_border_color(ui: &Ui, id: ElementId) -> Option<Color> {
    let element = ui.get(id)?;
    match ui.resolve_value_for(element, ui.properties().surface.border_brushes.top) {
        Some(Brush::Solid(color)) => Some(color),
        _ => None,
    }
}

fn element_foreground_color(ui: &Ui, id: ElementId) -> Option<Color> {
    let element = ui.get(id)?;
    match ui.resolve_value_for(element, ui.properties().foreground) {
        Brush::Solid(color) => Some(color),
        _ => None,
    }
}

fn element_border_width(ui: &Ui, id: ElementId) -> f64 {
    let element = ui.get(id).expect("element should exist");
    ui.resolve_value_for(element, ui.properties().surface.border_widths.top)
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.001,
        "expected {expected}, got {actual}"
    );
}

fn thumb_timing(from: f64, to: f64, t: f32) -> f64 {
    let timing = TimingFunction::cubic_bezier(0.215, 0.61, 0.355, 1.0);
    from.interpolate(to, timing.sample(t))
}

fn interpolate_srgb_color(from: Color, to: Color, t: f32) -> Color {
    DynamicColor::from_alpha_color(from)
        .interpolate(
            DynamicColor::from_alpha_color(to),
            ColorSpaceTag::Srgb,
            HueDirection::Shorter,
        )
        .eval(t)
        .to_alpha_color::<Srgb>()
}

#[test]
fn template_overrides_replace_visuals_not_control_behavior() {
    use crate::{
        Button, CheckBox, SLOT_BUTTON_CONTENT, SLOT_CHECK_BOX_BOX, SLOT_CHECK_BOX_LABEL,
        SLOT_CHECK_BOX_MARK, SLOT_SLIDER_THUMB, SLOT_TOGGLE_THUMB, Slider, Template, TemplateChild,
        Text, TextWidget, ToggleSwitch, set_slider_value,
    };

    const OUTER_FRAME: PartTag = PartTag(0x40);
    const INNER_FRAME: PartTag = PartTag(0x41);

    struct DoubleFrameButtonTemplate;
    impl Template for DoubleFrameButtonTemplate {
        fn instantiate(&self, cx: &mut TemplateContext<'_>) {
            let outer = cx.insert(TemplateChild::surface_part(OUTER_FRAME));
            let inner = cx.insert_under(outer, TemplateChild::surface_part(INNER_FRAME));
            let _content =
                cx.insert_under(inner, TemplateChild::content_presenter(SLOT_BUTTON_CONTENT));
        }
    }

    struct SingleSlotTemplate(TemplateSlot);
    impl Template for SingleSlotTemplate {
        fn instantiate(&self, cx: &mut TemplateContext<'_>) {
            let _part = cx.insert(TemplateChild::surface_slot(self.0));
        }
    }

    static BUTTON_TEMPLATE: DoubleFrameButtonTemplate = DoubleFrameButtonTemplate;
    static TOGGLE_TEMPLATE: SingleSlotTemplate = SingleSlotTemplate(SLOT_TOGGLE_THUMB);
    static SLIDER_TEMPLATE: SingleSlotTemplate = SingleSlotTemplate(SLOT_SLIDER_THUMB);

    struct CheckBoxTemplate;
    impl Template for CheckBoxTemplate {
        fn instantiate(&self, cx: &mut TemplateContext<'_>) {
            let box_part = cx.insert(TemplateChild::surface_slot(SLOT_CHECK_BOX_BOX));
            let _mark = cx.insert_under(
                box_part,
                TemplateChild::slot(SLOT_CHECK_BOX_MARK, Box::new(TextWidget)),
            );
            let _label = cx.insert(TemplateChild::content_presenter(SLOT_CHECK_BOX_LABEL));
        }
    }

    static CHECK_BOX_TEMPLATE: CheckBoxTemplate = CheckBoxTemplate;

    let mut ui = Ui::new();
    let root = ui.root();

    let button = ui.append(root, Button::new("Run").template(&BUTTON_TEMPLATE));
    let outer = ui.part(button, OUTER_FRAME).unwrap();
    let inner = ui.part(button, INNER_FRAME).unwrap();
    let content = ui.slot(button, SLOT_BUTTON_CONTENT).unwrap();
    assert_eq!(ui.get(outer).unwrap().parent(), Some(button));
    assert_eq!(ui.get(inner).unwrap().parent(), Some(outer));
    assert_eq!(ui.get(content).unwrap().parent(), Some(inner));
    assert_eq!(
        presented_button_label_text(&mut ui, button).as_deref(),
        Some("Run")
    );
    let structured_label = ui
        .slot_children(button, SLOT_BUTTON_CONTENT, |content| {
            content.add(Text::new("Retained"))
        })
        .expect("custom button template should expose its content slot");
    assert_eq!(ui.get(structured_label).unwrap().parent(), Some(content));
    assert_eq!(presented_element_text(&mut ui, content), None);
    assert_eq!(
        presented_element_text(&mut ui, structured_label).as_deref(),
        Some("Retained")
    );

    let toggle = ui.append(root, ToggleSwitch::new().template(&TOGGLE_TEMPLATE));
    ui.synthesize_click(toggle, Point::ZERO);
    let checked = ui
        .get(toggle)
        .and_then(|el| el.get_local_value(ui.properties().is_checked).copied())
        .unwrap_or(false);
    assert!(
        checked,
        "custom toggle template should keep toggle behavior"
    );

    let slider = ui.append(root, Slider::new().template(&SLIDER_TEMPLATE).value(0.75));
    assert!(ui.slot(slider, SLOT_SLIDER_THUMB).is_some());
    assert_eq!(
        ui.get(slider)
            .unwrap()
            .get_local_value(ui.properties().value)
            .copied(),
        Some(0.75)
    );
    set_slider_value(&mut ui, slider, 0.25);
    assert_eq!(
        ui.get(slider)
            .unwrap()
            .get_local_value(ui.properties().value)
            .copied(),
        Some(0.25)
    );

    let checkbox = ui.append(root, CheckBox::new("Confirm").template(&CHECK_BOX_TEMPLATE));
    assert!(ui.slot(checkbox, SLOT_CHECK_BOX_BOX).is_some());
    assert!(ui.slot(checkbox, SLOT_CHECK_BOX_MARK).is_some());
    let checkbox_label = ui.slot(checkbox, SLOT_CHECK_BOX_LABEL).unwrap();
    assert_eq!(
        presented_element_text(&mut ui, checkbox_label).as_deref(),
        Some("Confirm")
    );
    ui.synthesize_click(checkbox, Point::ZERO);
    assert_eq!(
        ui.get(checkbox)
            .unwrap()
            .get_local_value(ui.properties().is_checked)
            .copied(),
        Some(true)
    );
}

#[test]
fn owned_template_specs_work_through_control_builders() {
    use crate::{Button, SLOT_BUTTON_CONTENT, TemplateNode, TemplateSpec};

    const OWNED_FRAME: PartTag = PartTag(0x44);
    let owned_class = ClassId(0x45);

    let template = TemplateSpec::from_nodes([TemplateNode::surface_part(OWNED_FRAME)
        .with_class(owned_class)
        .with_child(TemplateNode::content_presenter(SLOT_BUTTON_CONTENT))]);

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, Button::new("Owned").template(template));

    let frame = ui
        .part(button, OWNED_FRAME)
        .expect("owned template should create its surface part");
    let content = ui
        .slot(button, SLOT_BUTTON_CONTENT)
        .expect("owned template should create its content slot");

    assert!(ui.get(frame).unwrap().classes().contains(&owned_class));
    assert!(
        ui.get(button)
            .unwrap()
            .get_local_value(ui.properties().control_template)
            .is_some()
    );
    assert_eq!(ui.get(content).unwrap().parent(), Some(frame));
    assert_eq!(
        presented_element_text(&mut ui, content).as_deref(),
        Some("Owned")
    );
}

#[test]
fn style_can_supply_initial_control_template() {
    use crate::{
        Button, ControlTemplate, SLOT_BUTTON_CONTENT, TYPE_BUTTON, TemplateNode, TemplateSpec,
    };

    const TEMPLATE_CLASS: ClassId = ClassId(0x46);
    const STYLE_FRAME: PartTag = PartTag(0x47);
    let style_template =
        ControlTemplate::from(TemplateSpec::from_nodes([TemplateNode::surface_part(
            STYLE_FRAME,
        )
        .with_child(TemplateNode::content_presenter(SLOT_BUTTON_CONTENT))]));

    let mut ui = Ui::new();
    let root = ui.root();
    let style = StyleBuilder::new()
        .set(ui.properties().control_template, Some(style_template))
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            SelectorBuilder::new(SelectorStep::type_tag(TYPE_BUTTON).with_class(TEMPLATE_CLASS))
                .build(),
            style,
        )
        .build();
    ui.set_cascade(cascade);

    let button = ui.append(root, Button::new("Styled").class(TEMPLATE_CLASS));
    let frame = ui
        .part(button, STYLE_FRAME)
        .expect("style-authored template should create its frame");
    let content = ui
        .slot(button, SLOT_BUTTON_CONTENT)
        .expect("style-authored template should create its content slot");

    assert_eq!(ui.get(frame).unwrap().parent(), Some(button));
    assert_eq!(ui.get(content).unwrap().parent(), Some(frame));
    assert_eq!(
        presented_element_text(&mut ui, content).as_deref(),
        Some("Styled")
    );
}

#[test]
fn ui_new_supplies_framework_control_template_property() {
    use crate::controls::button::default_button_template;
    use crate::{Button, SLOT_BUTTON_CONTENT};

    let mut ui = Ui::new();

    let button = ui.append(ui.root(), Button::new("Default"));
    let element = ui.get(button).unwrap();
    let resolved = ui.resolve_value_for(element, ui.properties().control_template);

    assert_eq!(
        resolved.as_ref().map(crate::ControlTemplate::id),
        Some(default_button_template().id())
    );
    assert_eq!(
        ui.template_instance(button).unwrap().template_id(),
        default_button_template().id()
    );
    assert!(
        ui.slot(button, SLOT_BUTTON_CONTENT).is_some(),
        "default template should still expose button content"
    );
}

#[test]
fn empty_app_cascade_preserves_framework_control_template_property() {
    use crate::controls::button::default_button_template;
    use crate::{Button, SLOT_BUTTON_CONTENT};

    let mut ui = Ui::new();
    ui.set_cascade(StyleCascadeBuilder::new().build());

    let button = ui.append(ui.root(), Button::new("Default"));
    let element = ui.get(button).unwrap();
    let resolved = ui.resolve_value_for(element, ui.properties().control_template);

    assert_eq!(
        resolved.as_ref().map(crate::ControlTemplate::id),
        Some(default_button_template().id())
    );
    assert_eq!(
        ui.template_instance(button).unwrap().template_id(),
        default_button_template().id()
    );
    assert!(
        ui.slot(button, SLOT_BUTTON_CONTENT).is_some(),
        "framework template should still expose button content"
    );
}

#[test]
fn sheet_style_control_template_replaces_default_cascade_template() {
    use crate::controls::button::TYPE_BUTTON;
    use crate::{Button, ControlTemplate, SLOT_BUTTON_CONTENT, TemplateNode, TemplateSpec};

    const STYLE_FRAME: PartTag = PartTag(0x51);
    let style_template =
        ControlTemplate::from(TemplateSpec::from_nodes([TemplateNode::surface_part(
            STYLE_FRAME,
        )
        .with_child(TemplateNode::content_presenter(SLOT_BUTTON_CONTENT))]));
    let style_template_id = style_template.id();

    let mut ui = Ui::new();
    let style = StyleBuilder::new()
        .set(ui.properties().control_template, Some(style_template))
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Sheet,
            crate::selectors::type_tag(TYPE_BUTTON),
            style,
        )
        .build();
    ui.set_cascade(cascade);

    let button = ui.append(ui.root(), Button::new("Styled"));

    assert_eq!(
        ui.template_instance(button).unwrap().template_id(),
        style_template_id
    );
    let frame = ui.part(button, STYLE_FRAME).unwrap();
    let content = ui.slot(button, SLOT_BUTTON_CONTENT).unwrap();
    assert_eq!(
        ui.get(content).unwrap().parent(),
        Some(frame),
        "the stronger style template should own the realized visual structure"
    );
}

#[test]
fn style_change_can_materialize_template_after_insert() {
    use crate::{ControlTemplate, TemplateNode, TemplateSpec, Widget};

    const TYPE_LATE_TEMPLATE: understory_style::TypeTag = understory_style::TypeTag(0x52);
    const LATE_PART: PartTag = PartTag(0x53);

    #[derive(Debug)]
    struct LateTemplateWidget;

    impl Widget for LateTemplateWidget {
        fn type_tag(&self) -> Option<understory_style::TypeTag> {
            Some(TYPE_LATE_TEMPLATE)
        }
    }

    let mut ui = Ui::new();
    let owner = ui.append_widget(ui.root(), Box::new(LateTemplateWidget));
    assert!(
        ui.template_instance(owner).is_none(),
        "the widget starts without a resolved ControlTemplate"
    );

    let template = ControlTemplate::from(TemplateSpec::from_nodes([TemplateNode::surface_part(
        LATE_PART,
    )]));
    let template_id = template.id();
    let style = StyleBuilder::new()
        .set(ui.properties().control_template, Some(template))
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Sheet,
            crate::selectors::type_tag(TYPE_LATE_TEMPLATE),
            style,
        )
        .build();

    ui.set_cascade(cascade);

    assert_eq!(
        ui.template_instance(owner).unwrap().template_id(),
        template_id
    );
    assert!(ui.part(owner, LATE_PART).is_some());
}

#[test]
fn changing_control_template_replaces_parts_and_preserves_slot_content() {
    use crate::{Button, ControlTemplate, SLOT_BUTTON_CONTENT, TemplateNode, TemplateSpec, Text};

    const OLD_FRAME: PartTag = PartTag(0x48);
    const NEW_FRAME: PartTag = PartTag(0x49);
    let old_template =
        ControlTemplate::from(TemplateSpec::from_nodes([TemplateNode::surface_part(
            OLD_FRAME,
        )
        .with_child(TemplateNode::content_presenter(SLOT_BUTTON_CONTENT))]));
    let new_template =
        ControlTemplate::from(TemplateSpec::from_nodes([TemplateNode::surface_part(
            NEW_FRAME,
        )
        .with_child(TemplateNode::content_presenter(SLOT_BUTTON_CONTENT))]));

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, Button::new("Button").template(old_template));
    let old_frame = ui
        .part(button, OLD_FRAME)
        .expect("old template should create its frame");
    let authored = ui
        .slot_children(button, SLOT_BUTTON_CONTENT, |content| {
            content.add(Text::new("Retained child"))
        })
        .expect("old template should expose the content slot");

    ui.set_local(button, ui.properties().control_template, Some(new_template));

    let new_frame = ui
        .part(button, NEW_FRAME)
        .expect("new template should create its frame");
    let content = ui
        .slot(button, SLOT_BUTTON_CONTENT)
        .expect("new template should expose the content slot");

    assert!(
        ui.get(old_frame).is_none(),
        "old generated frame is removed"
    );
    assert_eq!(ui.part(button, OLD_FRAME), None);
    assert_eq!(ui.get(new_frame).unwrap().parent(), Some(button));
    assert_eq!(ui.get(content).unwrap().parent(), Some(new_frame));
    assert_eq!(ui.get(authored).unwrap().parent(), Some(content));
    assert_eq!(
        presented_element_text(&mut ui, authored).as_deref(),
        Some("Retained child")
    );
}

#[test]
fn clearing_control_template_reexpands_default_template() {
    use crate::{Button, ControlTemplate, SLOT_BUTTON_CONTENT, TemplateNode, TemplateSpec, Text};

    const LOCAL_FRAME: PartTag = PartTag(0x4B);
    let local_template =
        ControlTemplate::from(TemplateSpec::from_nodes([TemplateNode::surface_part(
            LOCAL_FRAME,
        )
        .with_child(TemplateNode::content_presenter(SLOT_BUTTON_CONTENT))]));

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, Button::new("Button").template(local_template));
    let local_frame = ui
        .part(button, LOCAL_FRAME)
        .expect("local template should create its frame");
    let authored = ui
        .slot_children(button, SLOT_BUTTON_CONTENT, |content| {
            content.add(Text::new("Retained child"))
        })
        .expect("local template should expose the content slot");

    assert!(ui.clear_local(button, ui.properties().control_template));

    let content = ui
        .slot(button, SLOT_BUTTON_CONTENT)
        .expect("default button template should expose the content slot");
    assert!(
        ui.get(local_frame).is_none(),
        "local template frame is removed"
    );
    assert_eq!(ui.part(button, LOCAL_FRAME), None);
    assert_eq!(ui.get(content).unwrap().parent(), Some(button));
    assert_eq!(ui.get(authored).unwrap().parent(), Some(content));
    assert_eq!(
        presented_element_text(&mut ui, authored).as_deref(),
        Some("Retained child")
    );
}

#[test]
fn binding_to_control_template_reexpands_template() {
    use crate::{Button, ControlTemplate, SLOT_BUTTON_CONTENT, TemplateNode, TemplateSpec, Text};
    use understory_property_binding::PropertyEndpoint;

    const BOUND_FRAME: PartTag = PartTag(0x4A);
    let bound_template =
        ControlTemplate::from(TemplateSpec::from_nodes([TemplateNode::surface_part(
            BOUND_FRAME,
        )
        .with_child(TemplateNode::content_presenter(SLOT_BUTTON_CONTENT))]));

    let mut ui = Ui::new();
    let root = ui.root();
    let template = ui.local_state(Some(bound_template));
    let button = ui.append(root, Button::new("Button"));
    let authored = ui
        .slot_children(button, SLOT_BUTTON_CONTENT, |content| {
            content.add(Text::new("Retained child"))
        })
        .expect("default template should expose the content slot");

    ui.bind(
        template.endpoint(),
        PropertyEndpoint::new(
            BindingOwner::element(button),
            ui.properties().control_template,
        ),
    )
    .expect("control template binding registers");
    ui.flush_bindings();

    let frame = ui
        .part(button, BOUND_FRAME)
        .expect("bound template should create its frame");
    let content = ui
        .slot(button, SLOT_BUTTON_CONTENT)
        .expect("bound template should expose the content slot");

    assert_eq!(ui.get(frame).unwrap().parent(), Some(button));
    assert_eq!(ui.get(content).unwrap().parent(), Some(frame));
    assert_eq!(ui.get(authored).unwrap().parent(), Some(content));
    assert_eq!(
        presented_element_text(&mut ui, authored).as_deref(),
        Some("Retained child")
    );
}

#[test]
fn inspection_reports_optional_and_recommended_template_slots() {
    use crate::{
        SLOT_SLIDER_AXIS, SLOT_SLIDER_THUMB, SLOT_SLIDER_TICK, SLOT_SLIDER_TICK_LABEL,
        SLOT_SLIDER_TRACK, Slider, Template, TemplateChild,
    };

    struct MinimalSliderTemplate;
    impl Template for MinimalSliderTemplate {
        fn instantiate(&self, cx: &mut TemplateContext<'_>) {
            let _thumb = cx.insert(TemplateChild::surface_slot(SLOT_SLIDER_THUMB));
        }
    }

    static MINIMAL_SLIDER_TEMPLATE: MinimalSliderTemplate = MinimalSliderTemplate;

    let mut ui = Ui::new();
    let root = ui.root();
    let slider = ui.append(root, Slider::new().template(&MINIMAL_SLIDER_TEMPLATE));

    let report = ui
        .template_expansion_report(slider)
        .expect("slider should expose a template expansion report");
    assert_eq!(report.required_slots()[0].slot(), SLOT_SLIDER_THUMB);
    assert!(report.required_slots()[0].is_present());
    assert_eq!(report.recommended_slots()[0].slot(), SLOT_SLIDER_TRACK);
    assert!(!report.recommended_slots()[0].is_present());
    assert!(!report.has_missing_required_slots());
    assert!(report.has_missing_recommended_slots());
    assert!(!report.has_issues());

    let snapshot = ui.inspection_snapshot();
    let contract = snapshot
        .elements
        .iter()
        .find(|row| row.id == slider)
        .and_then(|row| row.template_contract.as_ref())
        .expect("slider should expose a template contract");

    assert_eq!(contract.required_slots[0].name, "slider-thumb");
    assert_eq!(
        contract.required_slots[0].element,
        ui.slot(slider, SLOT_SLIDER_THUMB)
    );
    assert!(!contract.has_missing_required_slots());

    assert_eq!(contract.recommended_slots.len(), 1);
    assert_eq!(contract.recommended_slots[0].name, "slider-track");
    assert_eq!(contract.recommended_slots[0].element, None);
    assert!(contract.has_missing_recommended_slots());
    assert!(
        !contract.has_issues(),
        "missing recommended slots should be a quality signal, not a contract failure"
    );

    assert_eq!(contract.optional_slots.len(), 3);
    assert_eq!(contract.optional_slots[0].name, SLOT_SLIDER_AXIS.name());
    assert_eq!(contract.optional_slots[1].name, SLOT_SLIDER_TICK.name());
    assert_eq!(
        contract.optional_slots[2].name,
        SLOT_SLIDER_TICK_LABEL.name()
    );
    assert!(
        contract
            .optional_slots
            .iter()
            .all(|slot| slot.element.is_none())
    );
    assert_eq!(ui.slot(slider, SLOT_SLIDER_TRACK), None);
}

#[test]
fn checkbox_template_contract_allows_optional_mark_slot() {
    use crate::{
        CheckBox, SLOT_CHECK_BOX_BOX, SLOT_CHECK_BOX_LABEL, SLOT_CHECK_BOX_MARK, Template,
        TemplateChild,
    };

    struct MinimalCheckBoxTemplate;
    impl Template for MinimalCheckBoxTemplate {
        fn instantiate(&self, cx: &mut TemplateContext<'_>) {
            let _box = cx.insert(TemplateChild::surface_slot(SLOT_CHECK_BOX_BOX));
            let _label = cx.insert(TemplateChild::content_presenter(SLOT_CHECK_BOX_LABEL));
        }
    }

    static MINIMAL_CHECK_BOX_TEMPLATE: MinimalCheckBoxTemplate = MinimalCheckBoxTemplate;

    let mut ui = Ui::new();
    let root = ui.root();
    let checkbox = ui.append(
        root,
        CheckBox::new("Confirm").template(&MINIMAL_CHECK_BOX_TEMPLATE),
    );

    let report = ui
        .template_expansion_report(checkbox)
        .expect("checkbox should expose a template expansion report");
    assert_eq!(report.required_slots()[0].slot(), SLOT_CHECK_BOX_BOX);
    assert_eq!(report.required_slots()[1].slot(), SLOT_CHECK_BOX_LABEL);
    assert!(report.required_slots().iter().all(|slot| slot.is_present()));
    assert_eq!(report.optional_slots()[0].slot(), SLOT_CHECK_BOX_MARK);
    assert!(!report.optional_slots()[0].is_present());
    assert!(!report.has_issues());

    ui.synthesize_click(checkbox, Point::ZERO);
    assert_eq!(
        ui.get(checkbox)
            .unwrap()
            .get_local_value(ui.properties().is_checked)
            .copied(),
        Some(true),
        "the optional visual mark slot should not be required for checkbox behavior",
    );
}

#[test]
fn inspection_reports_template_contract_slot_status() {
    use crate::{
        Button, SLOT_BUTTON_CONTENT, Template, TemplateChild, TemplateDiagnostic, TextWidget,
    };

    struct EmptyButtonTemplate;
    impl Template for EmptyButtonTemplate {
        fn instantiate(&self, _cx: &mut TemplateContext<'_>) {}
    }

    struct ContentButtonTemplate;
    impl Template for ContentButtonTemplate {
        fn instantiate(&self, cx: &mut TemplateContext<'_>) {
            let _content = cx.insert(TemplateChild::slot(
                SLOT_BUTTON_CONTENT,
                Box::new(TextWidget),
            ));
        }
    }

    struct DuplicateButtonTemplate;
    impl Template for DuplicateButtonTemplate {
        fn instantiate(&self, cx: &mut TemplateContext<'_>) {
            let _first = cx.insert(TemplateChild::slot(
                SLOT_BUTTON_CONTENT,
                Box::new(TextWidget),
            ));
            let _duplicate = cx.insert(TemplateChild::slot(
                SLOT_BUTTON_CONTENT,
                Box::new(TextWidget),
            ));
        }
    }

    static EMPTY_TEMPLATE: EmptyButtonTemplate = EmptyButtonTemplate;
    static CONTENT_TEMPLATE: ContentButtonTemplate = ContentButtonTemplate;
    static DUPLICATE_TEMPLATE: DuplicateButtonTemplate = DuplicateButtonTemplate;

    let mut ui = Ui::new();
    let root = ui.root();
    let default_button = ui.append(root, Button::new("Default"));
    let empty_button = ui.append(root, Button::new("Empty").template(&EMPTY_TEMPLATE));
    let custom_button = ui.append(root, Button::new("Custom").template(&CONTENT_TEMPLATE));
    let duplicate_button = ui.append(root, Button::new("Duplicate").template(&DUPLICATE_TEMPLATE));

    let snapshot = ui.inspection_snapshot();
    let default_contract = snapshot
        .elements
        .iter()
        .find(|row| row.id == default_button)
        .and_then(|row| row.template_contract.as_ref())
        .expect("default button should expose a template contract");
    assert_eq!(default_contract.required_slots[0].name, "content");
    assert_eq!(
        default_contract.required_slots[0].element,
        ui.slot(default_button, SLOT_BUTTON_CONTENT)
    );
    assert!(!default_contract.has_missing_required_slots());

    let empty_contract = snapshot
        .elements
        .iter()
        .find(|row| row.id == empty_button)
        .and_then(|row| row.template_contract.as_ref())
        .expect("custom button should expose the same control contract");
    assert_eq!(empty_contract.required_slots[0].name, "content");
    assert_eq!(empty_contract.required_slots[0].element, None);
    assert!(empty_contract.has_missing_required_slots());

    let custom_contract = snapshot
        .elements
        .iter()
        .find(|row| row.id == custom_button)
        .and_then(|row| row.template_contract.as_ref())
        .expect("custom template with content should expose contract status");
    assert_eq!(
        custom_contract.required_slots[0].element,
        ui.slot(custom_button, SLOT_BUTTON_CONTENT)
    );
    assert!(!custom_contract.has_missing_required_slots());

    let duplicate_contract = snapshot
        .elements
        .iter()
        .find(|row| row.id == duplicate_button)
        .and_then(|row| row.template_contract.as_ref())
        .expect("duplicate template should expose contract diagnostics");
    let content_part = SLOT_BUTTON_CONTENT.part_tag().unwrap();
    assert!(!duplicate_contract.has_missing_required_slots());
    assert!(duplicate_contract.has_issues());
    assert_eq!(duplicate_contract.diagnostics.len(), 2);
    assert!(
        duplicate_contract
            .diagnostics
            .iter()
            .any(|diagnostic| matches!(
                *diagnostic,
                TemplateDiagnostic::DuplicateSlot {
                    slot,
                    key: None,
                    ..
                } if slot == SLOT_BUTTON_CONTENT
            ))
    );
    assert!(
        duplicate_contract
            .diagnostics
            .iter()
            .any(|diagnostic| matches!(
                *diagnostic,
                TemplateDiagnostic::DuplicatePart {
                    part_tag,
                    key: None,
                    ..
                } if part_tag == content_part
            ))
    );
}

#[test]
fn surface_template_parts_do_not_match_generic_type_selector() {
    use crate::Slider;

    let mut ui = Ui::new();
    let root = ui.root();
    let slider = ui.append(root, Slider::new());
    let track = ui.part(slider, crate::PART_SLIDER_TRACK).unwrap();

    let red = Some(Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff)));
    let style = StyleBuilder::new()
        .set(ui.properties().surface.background, red)
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            crate::selectors::type_tag(TYPE_GENERIC),
            style,
        )
        .build();
    ui.set_cascade(cascade);

    let background =
        ui.resolve_value_for(ui.get(track).unwrap(), ui.properties().surface.background);
    assert_eq!(
        background, None,
        "surface-only template parts should not inherit TYPE_GENERIC as a fake type"
    );
}

#[test]
fn slider_template_parts_match_owner_local_part_selectors() {
    use crate::{PART_SLIDER_THUMB, PART_SLIDER_TRACK, SLOT_SLIDER_THUMB, Slider, TYPE_SLIDER};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_local(
        root,
        ui.properties().layout,
        Layout::flex().logical_size(300.0, 80.0),
    );

    let slider = ui.append(root, Slider::new());
    let track = ui.part(slider, PART_SLIDER_TRACK).unwrap();
    let thumb = ui
        .slot(slider, SLOT_SLIDER_THUMB)
        .expect("slider template should expose a thumb slot");
    let red = Some(Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff)));
    let style = StyleBuilder::new()
        .set(ui.properties().surface.background, red.clone())
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            crate::selectors::part(crate::selectors::template_part(
                TYPE_SLIDER,
                PART_SLIDER_TRACK,
            )),
            style,
        )
        .build();
    ui.set_cascade(cascade);

    let background =
        ui.resolve_value_for(ui.get(track).unwrap(), ui.properties().surface.background);
    assert_eq!(
        background, red,
        "owner-local part rule should match the nested slider track"
    );

    let snapshot = ui.inspection_snapshot();
    let slider_row = snapshot
        .elements
        .iter()
        .find(|row| row.id == slider)
        .expect("slider should be inspectable");
    assert_eq!(slider_row.type_tag, Some(TYPE_SLIDER));
    assert!(slider_row.has_widget_identity);

    let track_row = snapshot
        .elements
        .iter()
        .find(|row| row.id == track)
        .expect("track should be inspectable");
    assert_eq!(track_row.templated_parent, Some(slider));
    assert_eq!(track_row.type_tag, None);
    assert!(track_row.has_widget_identity);
    assert_eq!(track_row.part_tag, Some(PART_SLIDER_TRACK));
    assert_eq!(track_row.template_slot, Some("slider-track"));

    let thumb_row = snapshot
        .elements
        .iter()
        .find(|row| row.id == thumb)
        .expect("thumb should be inspectable");
    assert_eq!(thumb_row.templated_parent, Some(slider));
    assert_eq!(thumb_row.type_tag, None);
    assert!(thumb_row.has_widget_identity);
    assert_eq!(thumb_row.part_tag, Some(PART_SLIDER_THUMB));
    assert_eq!(thumb_row.template_slot, Some("slider-thumb"));

    ui.compute_layout(Size::new(300.0, 80.0));
    let mut output = Scene::new();
    ui.paint(&mut output);
    assert!(
        !output.is_empty(),
        "styled slider track should contribute painted output"
    );
}

#[test]
fn slider_template_creates_keyed_major_and_minor_tick_parts() {
    use crate::{
        CLASS_SLIDER_MAJOR_TICK, CLASS_SLIDER_MINOR_TICK, PART_SLIDER_TICK, PART_SLIDER_TICK_LABEL,
        SLOT_SLIDER_TICK, SLOT_SLIDER_TICK_LABEL, Slider, TemplateItemKey,
    };

    let mut ui = Ui::new();
    let root = ui.root();
    let slider = ui.append(root, Slider::new().min(10.0).max(20.0));
    let ticks: Vec<_> = ui.parts(slider, PART_SLIDER_TICK).collect();
    let labels: Vec<_> = ui.parts(slider, PART_SLIDER_TICK_LABEL).collect();

    assert_eq!(ticks.len(), 5);
    assert_eq!(labels.len(), 5);

    let instance = ui.template_instance(slider).unwrap();
    let first_tick = instance
        .keyed_part(PART_SLIDER_TICK, TemplateItemKey::new(0))
        .unwrap();
    assert_eq!(
        instance.keyed_slot(SLOT_SLIDER_TICK, TemplateItemKey::new(0)),
        Some(first_tick)
    );
    let second_tick = instance
        .keyed_part(PART_SLIDER_TICK, TemplateItemKey::new(1))
        .unwrap();
    assert!(
        ui.get(first_tick)
            .unwrap()
            .classes()
            .contains(&CLASS_SLIDER_MAJOR_TICK)
    );
    assert!(
        ui.get(second_tick)
            .unwrap()
            .classes()
            .contains(&CLASS_SLIDER_MINOR_TICK)
    );

    let middle_label = instance
        .keyed_part(PART_SLIDER_TICK_LABEL, TemplateItemKey::new(2))
        .unwrap();
    assert_eq!(
        instance.keyed_slot(SLOT_SLIDER_TICK_LABEL, TemplateItemKey::new(2)),
        Some(middle_label)
    );
    let text = ui.properties().text;
    let label = ui.get(middle_label).unwrap().get_local_value(text).cloned();
    assert_eq!(label.as_deref(), Some("15"));
}

#[test]
fn slider_keyboard_uses_small_and_large_changes() {
    use crate::Slider;
    use ui_events::keyboard::{Code, Key, KeyboardEvent, Modifiers, NamedKey};

    let mut ui = Ui::new();
    let root = ui.root();
    let slider = ui.append(
        root,
        Slider::new()
            .value(0.5)
            .small_change(0.1)
            .large_change(0.25),
    );
    assert!(ui.set_focus(Some(slider)));

    let right = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
    assert!(ui.dispatch_key(&right).handled);
    assert_eq!(slider_value(&ui, slider), Some(0.6));

    let mut shift_right =
        KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
    shift_right.modifiers = Modifiers::SHIFT;
    assert!(ui.dispatch_key(&shift_right).handled);
    assert_eq!(slider_value(&ui, slider), Some(0.85));

    let page_down = KeyboardEvent::key_down(Key::Named(NamedKey::PageDown), Code::PageDown);
    assert!(ui.dispatch_key(&page_down).handled);
    assert_eq!(slider_value(&ui, slider), Some(0.6));

    let home = KeyboardEvent::key_down(Key::Named(NamedKey::Home), Code::Home);
    assert!(ui.dispatch_key(&home).handled);
    assert_eq!(slider_value(&ui, slider), Some(0.0));

    let end = KeyboardEvent::key_down(Key::Named(NamedKey::End), Code::End);
    assert!(ui.dispatch_key(&end).handled);
    assert_eq!(slider_value(&ui, slider), Some(1.0));
}

#[test]
fn slider_thumb_click_and_drag_preserve_thumb_grab_offset() {
    use crate::{SLOT_SLIDER_THUMB, Slider};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_local(
        root,
        ui.properties().layout,
        Layout::flex().logical_size(300.0, 80.0),
    );
    let slider = ui.append(root, Slider::new().value(0.5));
    ui.compute_layout(Size::new(300.0, 80.0));

    let thumb = ui.slot(slider, SLOT_SLIDER_THUMB).unwrap();
    let before = ui.effective_bounds(thumb);
    let press = Point::new(before.x0 + 3.0, before.center().y);

    assert!(ui.dispatch_pointer_event(&pointer_down(press, 1)).changed());
    assert!(ui.dispatch_pointer_event(&pointer_up(press, 2)).changed());
    assert_eq!(
        slider_value(&ui, slider),
        Some(0.5),
        "clicking the thumb itself should not recenter it under the pointer"
    );

    assert!(ui.dispatch_pointer_event(&pointer_down(press, 3)).changed());
    assert!(
        ui.dispatch_pointer_event(&pointer_move(Point::new(press.x + 20.0, press.y), 4, true,))
            .changed()
    );
    ui.compute_layout(Size::new(300.0, 80.0));

    let after = ui.effective_bounds(thumb);
    assert!(
        ((after.center().x - before.center().x) - 20.0).abs() < 0.001,
        "thumb should move by the drag delta instead of snapping its center to the pointer"
    );
}

#[test]
fn default_cascade_styles_template_parts_by_owner_part() {
    use crate::{PART_SLIDER_TICK, Slider, TemplateItemKey, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    let cascade = theme::default_cascade(ui.properties());
    ui.set_cascade(cascade);

    let root = ui.root();
    let slider = ui.append(root, Slider::new());
    let track = ui.part(slider, crate::PART_SLIDER_TRACK).unwrap();
    let major_tick = ui
        .template_instance(slider)
        .unwrap()
        .keyed_part(PART_SLIDER_TICK, TemplateItemKey::new(0))
        .unwrap();
    let expected = Some(Brush::Solid(Color::from_rgba8(0x44, 0x44, 0x4a, 0xff)));
    let background =
        ui.resolve_value_for(ui.get(track).unwrap(), ui.properties().surface.background);
    let tick_background = ui.resolve_value_for(
        ui.get(major_tick).unwrap(),
        ui.properties().surface.background,
    );

    assert_eq!(background, expected);
    assert_eq!(
        tick_background,
        Some(Brush::Solid(Color::from_rgba8(0xc8, 0xc8, 0xd2, 0xff)))
    );
}

#[test]
fn default_cascade_leaves_generic_panels_transparent() {
    use crate::{Panel, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));

    let panel = ui.append(ui.root(), Panel::new());
    let background =
        ui.resolve_value_for(ui.get(panel).unwrap(), ui.properties().surface.background);

    assert_eq!(background, None);
}

#[test]
fn owner_pseudo_selector_can_style_template_descendant() {
    use crate::{Button, PART_BUTTON_LABEL, TYPE_BUTTON};

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, Button::new("Run"));
    let label = ui.part(button, PART_BUTTON_LABEL).unwrap();

    let red = Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff));
    let style = StyleBuilder::new()
        .set(ui.properties().foreground, red.clone())
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            crate::selectors::part_when(
                SelectorStep::type_tag(TYPE_BUTTON).with_pseudo(PSEUDO_PRESSED),
                crate::selectors::template_part(TYPE_BUTTON, PART_BUTTON_LABEL),
            ),
            style,
        )
        .build();
    ui.set_cascade(cascade);
    ui.add_pseudo(button, PSEUDO_PRESSED);

    let mut output = Scene::new();
    ui.paint(&mut output);

    let label_node = ui.box_for(label).expect("label should have a box node");
    let foreground = ui
        .presentation
        .presentation(label_node)
        .expect("label should have presentation")
        .plain_text()
        .expect("label should present text")
        .foreground
        .clone();
    assert_eq!(foreground, Some(red));
}

#[test]
fn hit_test_picks_topmost_child() {
    // Hit testing now goes through `understory_box_tree`'s
    // spatially-indexed box-tree hit testing, which reads the
    // tree's `local_bounds` populated by `compute_layout`. Drive
    // the test through the layout pipeline: root 200×200 with
    // 50px padding all around; child fills the content area,
    // giving it 50..150 × 50..150.
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(200.0, 200.0));
    ui.set_box_padding(root, understory_presentation::Edges::all(50.0));
    let child = ui.insert_child(root);
    ui.set_layout(child, Layout::new().flex_grow(1.0).align_self_stretch());
    ui.compute_layout(Size::new(200.0, 200.0));

    // Inside child's bounds → child wins (deepest, topmost).
    assert_eq!(ui.hit_test(Point::new(100.0, 100.0)), Some(child));
    // Inside root only.
    assert_eq!(ui.hit_test(Point::new(10.0, 10.0)), Some(root));
    // Outside root entirely.
    assert_eq!(ui.hit_test(Point::new(300.0, 300.0)), None);
}

#[test]
fn viewport_clips_subtree_and_sets_cursor() {
    use crate::{CursorIcon, Viewport};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_local(
        root,
        ui.properties().layout,
        Layout::flex().logical_size(100.0, 100.0),
    );
    let viewport = ui.append(
        root,
        Viewport::new()
            .layout(Layout::new().logical_size(100.0, 100.0))
            .cursor_icon(CursorIcon::Grab),
    );
    let element = ui.get(viewport).unwrap();
    assert!(element.widget().unwrap().clips_subtree(element));

    ui.compute_layout(Size::new(100.0, 100.0));
    assert!(
        ui.dispatch_pointer_event(&pointer_move(Point::new(10.0, 10.0), 1, false))
            .hover_changed
    );
    assert_eq!(ui.cursor_icon(), Some(CursorIcon::Grab));
}

#[test]
fn repeating_timer_rearms_while_handler_remains() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    let mut ui = Ui::new();
    let target = ui.root();
    let count = Rc::new(Cell::new(0_u32));
    let count_for_handler = Rc::clone(&count);

    ui.on_timer(target, move |_ui, _event| {
        count_for_handler.set(count_for_handler.get() + 1);
    });
    ui.schedule_timer_repeating(target, 10);

    ui.pump_timers(10);
    assert_eq!(count.get(), 1, "first timer deadline should fire");
    assert_eq!(
        ui.next_timer_deadline(),
        Some(20),
        "live repeating timer should re-arm"
    );

    ui.pump_timers(20);
    assert_eq!(count.get(), 2, "re-armed timer should fire again");
}

#[test]
fn repeating_timer_stops_when_handler_removes_itself() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    let mut ui = Ui::new();
    let target = ui.root();
    let count = Rc::new(Cell::new(0_u32));
    let count_for_handler = Rc::clone(&count);

    ui.on_timer(target, move |ui, _event| {
        count_for_handler.set(count_for_handler.get() + 1);
        ui.remove_timer_handler(target);
    });
    ui.schedule_timer_repeating(target, 10);

    ui.pump_timers(10);
    assert_eq!(count.get(), 1, "first timer deadline should fire");
    assert_eq!(
        ui.next_timer_deadline(),
        None,
        "repeating timer should stop after handler self-removal"
    );

    ui.pump_timers(20);
    assert_eq!(count.get(), 1, "removed handler should not fire again");
}

#[test]
fn repeating_timer_stops_when_target_is_removed() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    let mut ui = Ui::new();
    let root = ui.root();
    let target = ui.insert_child(root);
    let count = Rc::new(Cell::new(0_u32));
    let count_for_handler = Rc::clone(&count);

    ui.on_timer(target, move |ui, _event| {
        count_for_handler.set(count_for_handler.get() + 1);
        ui.remove(target);
    });
    ui.schedule_timer_repeating(target, 10);

    ui.pump_timers(10);
    assert_eq!(count.get(), 1, "first timer deadline should fire");
    assert!(
        ui.get(target).is_none(),
        "timer target should have been removed"
    );
    assert_eq!(
        ui.next_timer_deadline(),
        None,
        "repeating timer should not re-arm for a removed target"
    );

    ui.pump_timers(20);
    assert_eq!(count.get(), 1, "removed target should not receive timers");
}

#[test]
fn timeline_cues_drain_by_time_and_drop_removed_targets() {
    let mut ui = Ui::new();
    let root = ui.root();
    let scoped_target = ui.append(root, crate::Panel::new());
    let _removed = ui
        .schedule_element_timeline_cue(scoped_target, 10, 7)
        .expect("fresh element should accept scoped cue");
    let kept = ui.schedule_timeline_cue(10, 9);

    assert_eq!(ui.next_wakeup_deadline(), Some(10));
    ui.remove(scoped_target);

    assert_eq!(ui.drain_timeline_cues(9), alloc::vec![]);
    assert_eq!(
        ui.drain_timeline_cues(10),
        alloc::vec![crate::TimelineCue {
            id: kept,
            target: None,
            payload: 9,
            due_at: 10,
        }]
    );
    assert_eq!(ui.next_wakeup_deadline(), None);
}

#[test]
fn hover_path_marks_ancestor_pseudoclasses() {
    // A child hover should mark every element along the
    // root→target path as `:hover` — the more correct CSS/WPF
    // behavior we get from hover path tracking. Sanity-check
    // by laying out a parent with a smaller padded child, hovering
    // over the child, and verifying both elements have :hover.
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(200.0, 200.0));
    ui.set_box_padding(root, understory_presentation::Edges::all(50.0));
    let child = ui.insert_child(root);
    ui.set_layout(child, Layout::new().flex_grow(1.0).align_self_stretch());
    ui.compute_layout(Size::new(200.0, 200.0));

    // Hover via update_hover_at-equivalent path.
    let changed = ui.update_hover_at(Point::new(100.0, 100.0));
    assert!(changed);

    // Both root and child have :hover; the child is the deepest.
    assert!(ui.get(root).unwrap().pseudos().contains(&PSEUDO_HOVER));
    assert!(ui.get(child).unwrap().pseudos().contains(&PSEUDO_HOVER));
    assert_eq!(ui.hovered(), Some(child));

    // Move pointer outside the child but still in the root's
    // padding band: leaves the child, keeps the root hovered.
    let changed = ui.update_hover_at(Point::new(10.0, 10.0));
    assert!(changed);
    assert!(ui.get(root).unwrap().pseudos().contains(&PSEUDO_HOVER));
    assert!(!ui.get(child).unwrap().pseudos().contains(&PSEUDO_HOVER));
    assert_eq!(ui.hovered(), Some(root));

    // Out of root entirely.
    let changed = ui.update_hover_at(Point::new(500.0, 500.0));
    assert!(changed);
    assert!(!ui.get(root).unwrap().pseudos().contains(&PSEUDO_HOVER));
    assert_eq!(ui.hovered(), None);
}

#[test]
fn element_to_tree_node_round_trips() {
    let mut ui = Ui::new();
    let root = ui.root();
    let a = ui.insert_child(root);
    let b = ui.insert_child(root);

    // Every element has at least one node; `node_to_element`
    // recovers it.
    for elem in [root, a, b] {
        let nodes = ui
            .geometry
            .nodes_for(elem)
            .expect("element should have tree nodes");
        assert!(!nodes.is_empty(), "element should have a tree node");
        for &node in nodes {
            assert_eq!(ui.node_to_element(node), Some(elem));
        }
    }

    // Tree structure mirrors element-tree structure: `a` and `b`
    // are children of `root`'s tree node.
    let root_node = ui.geometry.nodes_for(root).unwrap()[0];
    let a_node = ui.geometry.nodes_for(a).unwrap()[0];
    let b_node = ui.geometry.nodes_for(b).unwrap()[0];
    let children = ui.geometry.children_of(root_node);
    assert_eq!(children, &[a_node, b_node]);
    assert_eq!(ui.geometry.parent_of(a_node), Some(root_node));
}

#[test]
fn flush_bindings_propagates_template_binding() {
    // Two generic elements; binding from source.content → target.content.
    // Writing source.content then flushing should propagate the value
    // into target's `TemplateBinding` slot so its effective value reads
    // the new content.
    use alloc::string::String;
    use understory_property_binding::PropertyEndpoint;

    let mut ui = Ui::new();
    let root = ui.root();
    let source = ui.insert_child(root);
    let target = ui.insert_child(root);

    let content_prop = ui.properties().content;
    ui.bind(
        PropertyEndpoint::new(BindingOwner::element(source), content_prop),
        PropertyEndpoint::new(BindingOwner::element(target), content_prop),
    )
    .expect("binding registers");

    ui.set_local(source, content_prop, String::from("hello"));

    // Before flush, the binding is dirty; target still has its default.
    assert_eq!(ui.get(target).unwrap().get_local_value(content_prop), None);

    ui.flush_bindings();

    // After flush, target's effective local content is "hello".
    let stored = ui
        .get(target)
        .unwrap()
        .property_store()
        .effective_local_erased(content_prop.id())
        .and_then(|v| v.downcast_ref::<String>().cloned());
    assert_eq!(stored.as_deref(), Some("hello"));
}

#[test]
fn element_endpoint_supports_app_element_bind_map() {
    use crate::{PropertyEndpoint, Text};
    use alloc::format;

    let mut ui = Ui::new();
    let root = ui.root();
    let source = ui.append(root, Text::new("source"));
    let target = ui.append(root, Text::new(""));

    let text_prop = ui.properties().text;
    let source_endpoint: PropertyEndpoint<BindingOwner, String> =
        Ui::element_endpoint(source, text_prop);
    ui.bind_map(
        source_endpoint,
        Ui::element_endpoint(target, text_prop),
        |value| format!("value: {value}"),
    )
    .expect("element binding registers");

    ui.set_local(source, text_prop, String::from("42"));
    ui.flush_bindings();

    let target_text = ui.resolve_value_for(ui.get(target).unwrap(), text_prop);
    assert_eq!(target_text, "value: 42");
}

#[test]
fn flush_bindings_respects_local_precedence() {
    // If the target has a user `Local` value, the binding's
    // `TemplateBinding` write should be shadowed — `Local` outranks
    // `TemplateBinding`.
    use alloc::string::String;
    use understory_property_binding::PropertyEndpoint;

    let mut ui = Ui::new();
    let root = ui.root();
    let source = ui.insert_child(root);
    let target = ui.insert_child(root);

    let content_prop = ui.properties().content;
    // User sets a Local first.
    ui.set_local(target, content_prop, String::from("user"));
    ui.bind(
        PropertyEndpoint::new(BindingOwner::element(source), content_prop),
        PropertyEndpoint::new(BindingOwner::element(target), content_prop),
    )
    .expect("binding registers");
    ui.set_local(source, content_prop, String::from("from-binding"));
    ui.flush_bindings();

    // Target's effective value is still the user's "user" because Local
    // wins over TemplateBinding.
    let stored = ui
        .get(target)
        .unwrap()
        .property_store()
        .effective_local_erased(content_prop.id())
        .and_then(|v| v.downcast_ref::<String>().cloned());
    assert_eq!(stored.as_deref(), Some("user"));
}

#[test]
fn try_flush_bindings_reports_type_mismatch() {
    use alloc::string::String;
    use understory_property::Property;
    use understory_property_binding::{BindingError, PropertyEndpoint};

    let mut ui = Ui::new();
    let root = ui.root();
    let source = ui.insert_child(root);
    let target = ui.insert_child(root);

    let content_prop = ui.properties().content;
    let content_as_u32: Property<u32> = Property::from_id(content_prop.id());
    ui.bind(
        PropertyEndpoint::new(BindingOwner::element(source), content_as_u32),
        PropertyEndpoint::new(BindingOwner::element(target), content_as_u32),
    )
    .expect("binding registers because typed handles carry the same property id");

    ui.set_local(source, content_prop, String::from("not a u32"));

    let error = ui
        .try_flush_bindings()
        .expect_err("wrong erased source type should be reported");
    assert!(matches!(
        error.error(),
        BindingError::SourceTypeMismatch { .. }
    ));
}

#[test]
fn remove_clears_bindings_for_subtree() {
    use alloc::string::String;
    use understory_property_binding::PropertyEndpoint;

    let mut ui = Ui::new();
    let root = ui.root();
    let source = ui.insert_child(root);
    let target = ui.insert_child(root);

    let content_prop = ui.properties().content;
    ui.bind(
        PropertyEndpoint::new(BindingOwner::element(source), content_prop),
        PropertyEndpoint::new(BindingOwner::element(target), content_prop),
    )
    .expect("binding registers");

    ui.remove(target);
    ui.set_local(source, content_prop, String::from("after-remove"));

    let report = ui
        .try_flush_bindings()
        .expect("removed target binding should have been cleared");
    assert_eq!(report.evaluated_bindings(), 0);
}

#[test]
fn local_state_binding_updates_element_text() {
    use crate::{BindingSpecExt, Text};
    use alloc::format;

    let mut ui = Ui::new();
    let count = ui.local_state(0_u32);
    let label = ui.append(
        ui.root(),
        Text::new("").bind_text(count.map(|count| format!("Count: {count}"))),
    );

    ui.flush_bindings();
    assert_eq!(
        ui.get(label)
            .unwrap()
            .property_store()
            .effective_local_erased(ui.properties().text.id())
            .and_then(|value| value.downcast_ref::<String>().cloned())
            .as_deref(),
        Some("Count: 0")
    );

    assert!(count.set(&mut ui, 42));
    ui.flush_bindings();
    assert_eq!(
        ui.get(label)
            .unwrap()
            .property_store()
            .effective_local_erased(ui.properties().text.id())
            .and_then(|value| value.downcast_ref::<String>().cloned())
            .as_deref(),
        Some("Count: 42")
    );
}

#[test]
fn local_state_update_skips_unchanged_values() {
    let mut ui = Ui::new();
    let count = ui.local_state(7_u32);

    assert!(!count.update(&mut ui, |_| {}));
    assert!(count.update(&mut ui, |count| *count += 1));
    assert_eq!(count.get(&ui), Some(8));
}

#[test]
fn external_source_binding_refreshes_when_notified() {
    use alloc::format;
    use alloc::rc::Rc;
    use core::cell::Cell;

    use crate::{BindingSpecExt, Text};

    let mut ui = Ui::new();
    let external_value = Rc::new(Cell::new(3_u32));
    let external = ui.external_source({
        let external_value = external_value.clone();
        move || external_value.get()
    });
    let label = ui.append(
        ui.root(),
        Text::new("").bind_text(external.map(|value| format!("External: {value}"))),
    );

    ui.flush_bindings();
    assert_eq!(
        ui.get(label)
            .unwrap()
            .property_store()
            .effective_local_erased(ui.properties().text.id())
            .and_then(|value| value.downcast_ref::<String>().cloned())
            .as_deref(),
        Some("External: 3")
    );

    external_value.set(9);
    assert!(external.notify_changed(&mut ui));
    ui.flush_bindings();
    assert_eq!(
        ui.get(label)
            .unwrap()
            .property_store()
            .effective_local_erased(ui.properties().text.id())
            .and_then(|value| value.downcast_ref::<String>().cloned())
            .as_deref(),
        Some("External: 9")
    );
}

#[test]
fn template_expansion_inserts_parts_under_owner() {
    use crate::widget::{AppendSpec, Widget};
    use crate::{ControlTemplate, TemplateNode, TemplateSpec};
    use understory_style::TypeTag;

    // A unit-test widget whose ControlTemplate inserts two child panels. Both
    // should have templated_parent = the owner.
    const TEST_OWNER_TAG: TypeTag = TypeTag(0xFEED_BEEF);
    const TEST_PART_A: PartTag = PartTag(0xA);
    const TEST_PART_B: PartTag = PartTag(0xB);

    #[derive(Debug)]
    struct TestWidget;
    impl Widget for TestWidget {
        fn type_tag(&self) -> Option<TypeTag> {
            Some(TEST_OWNER_TAG)
        }
    }
    struct TestSpec;
    impl AppendSpec for TestSpec {
        fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
            ui.append_widget(parent, Box::new(TestWidget))
        }
    }

    let mut ui = Ui::new();
    let template = ControlTemplate::from(TemplateSpec::from_nodes([
        TemplateNode::surface_part(TEST_PART_A),
        TemplateNode::surface_part(TEST_PART_B),
    ]));
    let style = StyleBuilder::new()
        .set(ui.properties().control_template, Some(template))
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Sheet,
            crate::selectors::type_tag(TEST_OWNER_TAG),
            style,
        )
        .build();
    ui.set_cascade(cascade);
    let root = ui.root();
    let owner = ui.append(root, TestSpec);

    // The template should have created two children under owner, each
    // tagged with templated_parent = Some(owner).
    let owner_children = ui.get(owner).unwrap().children().to_vec();
    assert_eq!(owner_children.len(), 2, "template should insert 2 parts");
    let instance = ui
        .template_instance(owner)
        .expect("template instance should be retained");
    assert_eq!(instance.owner(), owner);
    assert_eq!(instance.part(TEST_PART_A), Some(owner_children[0]));
    assert_eq!(instance.part(TEST_PART_B), Some(owner_children[1]));
    for part_id in owner_children {
        let part = ui.get(part_id).unwrap();
        assert_eq!(
            part.templated_parent(),
            Some(owner),
            "part should track owner"
        );
        assert_eq!(part.parent(), Some(owner), "part is a child of owner");
    }

    // The owner itself has no templated parent (it's a logical child of root).
    assert_eq!(ui.get(owner).unwrap().templated_parent(), None);
}

#[test]
fn template_spec_builds_nested_parts_classes_and_keys() {
    use crate::widget::{AppendSpec, Widget};
    use crate::{
        ControlTemplate, Layout, TemplateItemKey, TemplateNode, TemplateSlot, TemplateSpec,
    };
    use understory_style::TypeTag;

    const TEST_OWNER_TAG: TypeTag = TypeTag(0xFEED_CAFE);
    const OUTER_PART: PartTag = PartTag(0x10);
    const INNER_PART: PartTag = PartTag(0x11);
    const OUTER_SLOT: TemplateSlot = TemplateSlot::new("outer").with_part_tag(OUTER_PART);
    const OUTER_KEY: TemplateItemKey = TemplateItemKey::new(7);
    const OUTER_CLASS: &str = ".template-outer";
    let outer_layout = Layout::new().absolute_at(3.0, 4.0).logical_size(20.0, 10.0);
    let template = ControlTemplate::from(TemplateSpec::from_nodes([TemplateNode::surface_slot(
        OUTER_SLOT,
    )
    .with_key(OUTER_KEY)
    .with_class(OUTER_CLASS)
    .with_layout(outer_layout.clone())
    .with_child(TemplateNode::surface_part(INNER_PART))]));

    #[derive(Debug)]
    struct TestWidget;
    impl Widget for TestWidget {
        fn type_tag(&self) -> Option<TypeTag> {
            Some(TEST_OWNER_TAG)
        }
    }
    struct TestSpec;
    impl AppendSpec for TestSpec {
        fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
            ui.append_widget(parent, Box::new(TestWidget))
        }
    }

    let mut ui = Ui::new();
    let style = StyleBuilder::new()
        .set(ui.properties().control_template, Some(template))
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Sheet,
            crate::selectors::type_tag(TEST_OWNER_TAG),
            style,
        )
        .build();
    ui.set_cascade(cascade);
    let owner = ui.append(ui.root(), TestSpec);
    let instance = ui.template_instance(owner).unwrap();
    let outer = instance.keyed_part(OUTER_PART, OUTER_KEY).unwrap();
    let inner = instance.part(INNER_PART).unwrap();

    assert_eq!(instance.keyed_slot(OUTER_SLOT, OUTER_KEY), Some(outer));
    assert_eq!(ui.get(outer).unwrap().parent(), Some(owner));
    assert_eq!(ui.get(inner).unwrap().parent(), Some(outer));
    assert_eq!(ui.get(outer).unwrap().templated_parent(), Some(owner));
    assert_eq!(ui.get(inner).unwrap().templated_parent(), Some(owner));
    let outer_class = ui
        .style_vocabulary()
        .class_id_by_name(OUTER_CLASS)
        .expect("template class name should be interned during expansion");
    assert!(ui.get(outer).unwrap().classes().contains(&outer_class));
    assert_eq!(ui.effective_layout(outer), outer_layout);
}

#[test]
fn two_independent_bindings_both_propagate() {
    // Pure-binding analogue of the two-button scenario: two
    // source/target pairs registered + written in sequence.
    // Both targets should receive their bound value on flush.
    use alloc::string::String;
    use understory_property_binding::PropertyEndpoint;

    let mut ui = Ui::new();
    let root = ui.root();
    let s1 = ui.insert_child(root);
    let t1 = ui.insert_child(root);
    let s2 = ui.insert_child(root);
    let t2 = ui.insert_child(root);

    let content_prop = ui.properties().content;
    ui.bind(
        PropertyEndpoint::new(BindingOwner::element(s1), content_prop),
        PropertyEndpoint::new(BindingOwner::element(t1), content_prop),
    )
    .unwrap();
    ui.set_local(s1, content_prop, String::from("one"));

    ui.bind(
        PropertyEndpoint::new(BindingOwner::element(s2), content_prop),
        PropertyEndpoint::new(BindingOwner::element(t2), content_prop),
    )
    .unwrap();
    ui.set_local(s2, content_prop, String::from("two"));

    ui.flush_bindings();

    let read = |ui: &Ui, id: ElementId| -> Option<String> {
        ui.get(id)
            .unwrap()
            .property_store()
            .effective_local_erased(content_prop.id())
            .and_then(|v| v.downcast_ref::<String>().cloned())
    };
    assert_eq!(read(&ui, t1).as_deref(), Some("one"));
    assert_eq!(read(&ui, t2).as_deref(), Some("two"));
}

fn presented_element_text(ui: &mut Ui, element: ElementId) -> Option<String> {
    ui.compute_layout(Size::new(400.0, 120.0));
    let mut output = Scene::new();
    ui.paint(&mut output);

    let node = ui.box_for(element)?;
    ui.presentation
        .presentation(node)?
        .plain_text()
        .map(|text| String::from(text.content.as_str()))
}

fn presented_button_label_text(ui: &mut Ui, button: ElementId) -> Option<String> {
    let label = ui.part(button, crate::PART_BUTTON_LABEL)?;
    presented_element_text(ui, label)
}

#[test]
fn button_label_content_presenter_reads_owner_content() {
    // Regression guard for two sibling buttons: a template label is a
    // presenter over its owner content, so each generated label must display
    // its own templated owner rather than copied or shared state.
    use crate::Button;

    let mut ui = Ui::new();
    let root = ui.root();
    let b1 = ui.append(root, Button::new("Click me"));
    let b2 = ui.append(root, Button::new("Or me"));

    assert_eq!(
        presented_button_label_text(&mut ui, b1).as_deref(),
        Some("Click me")
    );
    assert_eq!(
        presented_button_label_text(&mut ui, b2).as_deref(),
        Some("Or me")
    );
}

#[test]
fn content_presenter_hosts_retained_children_over_text_fallback() {
    use crate::{Button, SLOT_BUTTON_CONTENT, Text};

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, Button::new("Fallback"));
    let presenter = ui
        .slot(button, SLOT_BUTTON_CONTENT)
        .expect("button template should expose content slot");

    assert_eq!(
        presented_element_text(&mut ui, presenter).as_deref(),
        Some("Fallback")
    );

    let child = ui
        .slot_children(button, SLOT_BUTTON_CONTENT, |content| {
            content.add(Text::new("Child"))
        })
        .expect("button template should expose content slot");

    assert_eq!(ui.get(child).unwrap().parent(), Some(presenter));
    assert_eq!(presented_element_text(&mut ui, presenter), None);
    assert_eq!(
        presented_element_text(&mut ui, child).as_deref(),
        Some("Child")
    );

    ui.remove(child);
    assert_eq!(
        presented_element_text(&mut ui, presenter).as_deref(),
        Some("Fallback")
    );
}

#[test]
fn button_uses_shared_content_slot_authoring() {
    use crate::{Button, Text, TooltipSpecExt};

    let mut ui = Ui::new();
    let root = ui.root();
    let (button, child) = ui.content(
        root,
        Button::new("Fallback").tooltip("Runs the deploy workflow"),
        |content| content.add(Text::new("Deploy")),
    );
    let child = child.expect("button template should expose content slot");
    let presenter = ui
        .slot(button, crate::SLOT_BUTTON_CONTENT)
        .expect("button template should expose content slot");

    assert_eq!(ui.get(child).unwrap().parent(), Some(presenter));
    assert_eq!(presented_element_text(&mut ui, presenter), None);
    assert_eq!(
        presented_element_text(&mut ui, child).as_deref(),
        Some("Deploy")
    );
}

#[test]
fn label_controls_use_shared_content_slot_authoring() {
    use crate::{
        CheckBox, RadioButton, SLOT_CHECK_BOX_LABEL, SLOT_RADIO_BUTTON_LABEL, SLOT_TOOLTIP_CONTENT,
        Text, Tooltip,
    };

    let mut ui = Ui::new();
    let root = ui.root();

    let (checkbox, checkbox_child) = ui.content(root, CheckBox::new(""), |content| {
        content.add(Text::new("Check"))
    });
    let checkbox_child = checkbox_child.expect("checkbox template should expose label slot");
    assert_eq!(
        ui.get(checkbox_child).unwrap().parent(),
        ui.slot(checkbox, SLOT_CHECK_BOX_LABEL)
    );

    let (radio, radio_child) = ui.content(root, RadioButton::new(""), |content| {
        content.add(Text::new("Radio"))
    });
    let radio_child = radio_child.expect("radio template should expose label slot");
    assert_eq!(
        ui.get(radio_child).unwrap().parent(),
        ui.slot(radio, SLOT_RADIO_BUTTON_LABEL)
    );

    let (tooltip, tooltip_child) = ui.content(root, Tooltip::new(""), |content| {
        content.add(Text::new("Tooltip"))
    });
    let tooltip_child = tooltip_child.expect("tooltip template should expose content slot");
    assert_eq!(
        ui.get(tooltip_child).unwrap().parent(),
        ui.slot(tooltip, SLOT_TOOLTIP_CONTENT)
    );
}

#[test]
fn app_bound_button_content_updates_template_label() {
    use crate::{BindingSpecExt, Button};
    use alloc::format;

    let mut ui = Ui::new();
    let root = ui.root();
    let activations = ui.local_state(0_u32);
    let button = ui.append(
        root,
        Button::new("").bind_content(activations.map(|count| {
            if *count == 0 {
                String::from("Deploy")
            } else {
                format!("Clicked {count}")
            }
        })),
    );

    ui.flush_bindings();

    assert_eq!(
        presented_button_label_text(&mut ui, button).as_deref(),
        Some("Deploy")
    );

    assert!(activations.set(&mut ui, 2));
    ui.flush_bindings();
    assert_eq!(
        presented_button_label_text(&mut ui, button).as_deref(),
        Some("Clicked 2")
    );
}

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

#[test]
fn tooltip_shows_as_overlay_for_hovered_target() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().logical_size(220.0, 120.0));
    let target = ui.insert_child(root);
    ui.set_layout(target, Layout::new().logical_size(80.0, 30.0));
    ui.set_tooltip(target, crate::Tooltip::new("Deploys the service"));

    ui.compute_layout(Size::new(220.0, 120.0));
    assert_eq!(ui.overlays.active_tooltip(), None);

    let result = ui.dispatch_pointer_event(&pointer_move(Point::new(20.0, 12.0), 1, false));
    assert!(result.changed());
    let active = ui
        .overlays
        .active_tooltip()
        .expect("hovering a tooltip target should create an overlay tooltip");
    assert_eq!(active.target, target);
    assert_eq!(ui.get(active.element).unwrap().parent(), Some(root));

    ui.compute_layout(Size::new(220.0, 120.0));
    let target_bounds = ui.effective_bounds(target);
    let tooltip_bounds = ui.effective_bounds(active.element);
    assert_eq!(
        tooltip_bounds.origin(),
        Point::new(target_bounds.x0, target_bounds.y1 + 8.0)
    );
    assert!(tooltip_bounds.width() > 0.0);
    assert!(tooltip_bounds.height() > 0.0);

    let overlay_root = ui
        .composition
        .overlay_root()
        .expect("tooltip should create an overlay composition root");
    let tooltip_layer = ui.composition.layer_for(active.element).unwrap();
    assert_eq!(
        ui.composition_layers().parent(tooltip_layer),
        Some(overlay_root)
    );
    assert_ne!(ui.hit_test(tooltip_bounds.center()), Some(active.element));

    let leave = ui.dispatch_pointer_event(&pointer_move(Point::new(180.0, 100.0), 2, false));
    assert!(leave.changed());
    assert_eq!(ui.overlays.active_tooltip(), None);
    assert!(ui.get(active.element).is_none());
}

#[test]
fn tooltip_padding_writes_surface_padding_not_layout_padding() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().logical_size(220.0, 120.0));
    let target = ui.insert_child(root);
    ui.set_layout(target, Layout::new().logical_size(80.0, 30.0));
    ui.set_tooltip(target, crate::Tooltip::new("Padded").padding(13.0, 7.0));

    ui.compute_layout(Size::new(220.0, 120.0));
    let _ = ui.dispatch_pointer_event(&pointer_move(Point::new(20.0, 12.0), 1, false));
    let active = ui
        .overlays
        .active_tooltip()
        .expect("hovering a tooltip target should create an overlay tooltip");

    assert_surface_padding(&ui, active.element, 7.0, 13.0, 7.0, 13.0);
    assert_layout_spacing_has_no_padding(&ui, active.element);
}

#[test]
fn default_cascade_styles_tooltip_surface_and_label() {
    use crate::{SLOT_TOOLTIP_CONTENT, Tooltip, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::new().logical_size(220.0, 120.0));
    let target = ui.insert_child(root);
    ui.set_layout(target, Layout::new().logical_size(80.0, 30.0));
    ui.set_tooltip(target, Tooltip::new("Deploys the service"));

    ui.compute_layout(Size::new(220.0, 120.0));
    let result = ui.dispatch_pointer_event(&pointer_move(Point::new(20.0, 12.0), 1, false));
    assert!(result.changed());
    let active = ui
        .overlays
        .active_tooltip()
        .expect("hovering a tooltip target should create an overlay tooltip");

    ui.compute_layout(Size::new(220.0, 120.0));
    let mut output = Scene::new();
    ui.paint(&mut output);

    let tooltip_node = ui
        .box_for(active.element)
        .expect("tooltip should have a box node");
    let surface = ui
        .presentation
        .presentation(tooltip_node)
        .expect("tooltip should have presentation")
        .surface()
        .expect("tooltip should present a surface");
    assert!(
        !surface.backgrounds.is_empty(),
        "default cascade should give tooltips an opaque background"
    );
    assert!(
        !surface.border.is_empty(),
        "default cascade should give tooltips a border"
    );

    let label = ui
        .slot(active.element, SLOT_TOOLTIP_CONTENT)
        .expect("tooltip should create a content label");
    let label_node = ui.box_for(label).expect("label should have a box node");
    let foreground = ui
        .presentation
        .presentation(label_node)
        .expect("label should have presentation")
        .plain_text()
        .expect("label should present text")
        .foreground
        .clone();
    assert_eq!(
        foreground,
        Some(Brush::Solid(Color::from_rgba8(0xf4, 0xf4, 0xf7, 0xff))),
        "tooltip label should inherit the styled tooltip foreground"
    );
}

#[test]
fn tooltip_attached_to_owner_activates_from_template_part_hover() {
    let mut ui = Ui::new();
    ui.set_cascade(crate::theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::new().logical_size(220.0, 120.0));
    let button = ui.append(root, crate::Button::new("Deploy").font_size(12.0));
    ui.set_layout(button, Layout::new().logical_size(92.0, 36.0));
    ui.set_tooltip(button, crate::Tooltip::new("Runs the deploy workflow"));

    ui.compute_layout(Size::new(220.0, 120.0));
    let center = ui.effective_bounds(button).center();
    let _ = ui.dispatch_pointer_event(&pointer_move(center, 1, false));

    assert_eq!(
        ui.overlays.active_tooltip().map(|active| active.target),
        Some(button),
        "tooltip lookup should walk from hovered template parts to the owning control"
    );
}

#[test]
fn tooltip_follows_keyboard_focus_and_clears_with_focus() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().logical_size(220.0, 120.0));
    let target = ui.insert_child(root);
    ui.set_layout(target, Layout::new().logical_size(80.0, 30.0));
    ui.set_focusable(target, true);
    ui.set_tooltip(target, crate::Tooltip::new("Focusable helper"));

    ui.compute_layout(Size::new(220.0, 120.0));
    assert!(ui.set_focus(Some(target)));
    let active = ui
        .overlays
        .active_tooltip()
        .expect("focused tooltip target should create an overlay tooltip");
    assert_eq!(active.target, target);

    assert!(ui.set_focus(None));
    assert_eq!(ui.overlays.active_tooltip(), None);
    assert!(ui.get(active.element).is_none());
}

#[test]
fn pointer_leave_does_not_restore_previous_focused_tooltip() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::row(20.0).logical_size(240.0, 120.0));
    let first = ui.insert_child(root);
    ui.set_layout(first, Layout::new().logical_size(70.0, 30.0));
    ui.set_focusable(first, true);
    ui.set_tooltip(first, crate::Tooltip::new("First"));
    let second = ui.insert_child(root);
    ui.set_layout(second, Layout::new().logical_size(70.0, 30.0));
    ui.set_tooltip(second, crate::Tooltip::new("Second"));

    ui.compute_layout(Size::new(240.0, 120.0));
    assert!(ui.set_focus(Some(first)));
    assert_eq!(
        ui.overlays.active_tooltip().map(|active| active.target),
        Some(first)
    );

    let second_center = ui.effective_bounds(second).center();
    let _ = ui.dispatch_pointer_event(&pointer_move(second_center, 1, false));
    assert_eq!(
        ui.overlays.active_tooltip().map(|active| active.target),
        Some(second)
    );

    let _ = ui.dispatch_pointer_event(&pointer_move(Point::new(220.0, 90.0), 2, false));
    assert_eq!(
        ui.overlays.active_tooltip(),
        None,
        "leaving a hover tooltip should hide it instead of restoring a focused target"
    );
}

#[test]
fn tooltip_placement_flips_when_bottom_would_overflow() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(220.0, 90.0));
    let target = ui.insert_child(root);
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(10.0, 50.0)
            .logical_size(70.0, 30.0),
    );
    ui.set_tooltip(target, crate::Tooltip::new("Near the bottom edge"));

    ui.compute_layout(Size::new(220.0, 90.0));
    let _ = ui.dispatch_pointer_event(&pointer_move(
        ui.effective_bounds(target).center(),
        1,
        false,
    ));
    ui.compute_layout(Size::new(220.0, 90.0));

    let active = ui.overlays.active_tooltip().unwrap();
    let target_bounds = ui.effective_bounds(target);
    let tooltip_bounds = ui.effective_bounds(active.element);
    assert!(
        tooltip_bounds.y1 <= target_bounds.y0,
        "bottom placement should flip above the target when it would overflow"
    );
}

#[test]
fn tooltip_can_be_placed_to_the_right_of_target() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(260.0, 100.0));
    let target = ui.insert_child(root);
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(20.0, 20.0)
            .logical_size(60.0, 24.0),
    );
    ui.set_tooltip(
        target,
        crate::Tooltip::new("Side")
            .placement(crate::Placement::Right)
            .offset(6.0, 0.0),
    );

    ui.compute_layout(Size::new(260.0, 100.0));
    let _ = ui.dispatch_pointer_event(&pointer_move(
        ui.effective_bounds(target).center(),
        1,
        false,
    ));
    ui.compute_layout(Size::new(260.0, 100.0));

    let active = ui.overlays.active_tooltip().unwrap();
    let target_bounds = ui.effective_bounds(target);
    let tooltip_bounds = ui.effective_bounds(active.element);
    assert_eq!(
        tooltip_bounds.origin(),
        Point::new(target_bounds.x1 + 6.0, target_bounds.y0)
    );
}

#[test]
fn popup_opens_as_overlay_container_with_content() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(260.0, 120.0));
    let target = ui.insert_child(root);
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(20.0, 20.0)
            .logical_size(80.0, 28.0),
    );
    ui.compute_layout(Size::new(260.0, 120.0));

    let popup = ui
        .open_popup(target, crate::Popup::new(), |ui, popup| {
            ui.append(popup, crate::Text::new("Popup content").font_size(12.0));
        })
        .expect("live target should open popup");
    ui.compute_layout(Size::new(260.0, 120.0));

    assert_eq!(ui.get(popup).unwrap().parent(), Some(root));
    assert_eq!(ui.get(popup).unwrap().children().len(), 1);
    let target_bounds = ui.effective_bounds(target);
    let popup_bounds = ui.effective_bounds(popup);
    assert_eq!(
        popup_bounds.origin(),
        Point::new(target_bounds.x0, target_bounds.y1 + 6.0)
    );

    let overlay_root = ui.composition.overlay_root().unwrap();
    let popup_layer = ui.composition.layer_for(popup).unwrap();
    assert_eq!(
        ui.composition_layers().parent(popup_layer),
        Some(overlay_root)
    );
}

#[test]
fn popup_padding_writes_surface_padding_not_layout_padding() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(260.0, 120.0));
    let target = ui.insert_child(root);
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(20.0, 20.0)
            .logical_size(80.0, 28.0),
    );
    ui.compute_layout(Size::new(260.0, 120.0));

    let popup = ui
        .open_popup(
            target,
            crate::Popup::new().padding(17.0, 11.0),
            |ui, popup| {
                ui.append(popup, crate::Text::new("Popup content").font_size(12.0));
            },
        )
        .expect("live target should open popup");

    assert_surface_padding(&ui, popup, 11.0, 17.0, 11.0, 17.0);
    assert_layout_spacing_has_no_padding(&ui, popup);
}

#[test]
fn popup_repositions_with_target_and_closes_by_root_element() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(260.0, 140.0));
    let target = ui.insert_child(root);
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(20.0, 20.0)
            .logical_size(70.0, 24.0),
    );
    ui.compute_layout(Size::new(260.0, 140.0));

    let popup = ui
        .open_popup(target, crate::Popup::new(), |ui, popup| {
            ui.append(popup, crate::Text::new("Popup").font_size(12.0));
        })
        .unwrap();
    ui.compute_layout(Size::new(260.0, 140.0));
    let original = ui.effective_bounds(popup).origin();

    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(120.0, 40.0)
            .logical_size(70.0, 24.0),
    );
    ui.compute_layout(Size::new(260.0, 140.0));
    let moved = ui.effective_bounds(popup).origin();
    assert_ne!(moved, original);
    assert_eq!(moved, Point::new(120.0, 70.0));

    assert!(ui.close_popup(popup));
    assert!(ui.get(popup).is_none());
    assert!(!ui.close_popup(popup));
}

#[test]
fn escape_closes_topmost_popup_and_restores_focus() {
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(260.0, 140.0));
    let target = ui.append(root, crate::Button::new("Open"));
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(20.0, 20.0)
            .logical_size(70.0, 28.0),
    );
    ui.compute_layout(Size::new(260.0, 140.0));
    assert!(ui.set_focus(Some(target)));

    let mut focused_inside = None;
    let popup = ui
        .open_popup(target, crate::Popup::new(), |ui, popup| {
            let child = ui.insert_child(popup);
            ui.set_layout(child, Layout::new().logical_size(80.0, 24.0));
            ui.set_focusable(child, true);
            focused_inside = Some(child);
        })
        .unwrap();
    ui.compute_layout(Size::new(260.0, 140.0));
    assert!(ui.set_focus(focused_inside));

    let escape = KeyboardEvent::key_down(Key::Named(NamedKey::Escape), Code::Escape);
    let result = ui.dispatch_key(&escape);

    assert!(result.handled);
    assert!(result.focus_changed);
    assert_eq!(result.focused, Some(target));
    assert!(ui.get(popup).is_none());
}

#[test]
fn outside_pointer_down_closes_popup_without_consuming_click() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(320.0, 120.0));
    let target = ui.append(root, crate::Button::new("Open"));
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(20.0, 20.0)
            .logical_size(70.0, 28.0),
    );
    let outside = ui.append(root, crate::Button::new("Other"));
    ui.set_layout(
        outside,
        Layout::new()
            .absolute_at(210.0, 20.0)
            .logical_size(80.0, 28.0),
    );
    let clicked = Rc::new(Cell::new(false));
    let clicked_for_handler = Rc::clone(&clicked);
    ui.on_click(outside, move |_ui, _event| {
        clicked_for_handler.set(true);
    });
    ui.compute_layout(Size::new(320.0, 120.0));

    let popup = ui
        .open_popup(target, crate::Popup::new(), |ui, popup| {
            ui.append(popup, crate::Text::new("Popup").font_size(12.0));
        })
        .unwrap();
    ui.compute_layout(Size::new(320.0, 120.0));
    assert!(ui.get(popup).is_some());

    let outside_center = ui.effective_bounds(outside).center();
    let down = ui.dispatch_pointer_event(&pointer_down(outside_center, 1));
    assert!(down.changed());
    assert!(ui.get(popup).is_none());

    let up = ui.dispatch_pointer_event(&pointer_up(outside_center, 2));
    assert!(up.changed());
    assert!(
        clicked.get(),
        "outside click should continue after dismissal"
    );
}

#[test]
fn scroll_view_clip_applies_to_child_surfaces() {
    use imaging::record::Command;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(0.0).logical_size(120.0, 80.0));
    let scroll = ui.append(root, crate::ScrollView::new().size(80.0, 32.0));
    let child = ui.insert_child(scroll);
    ui.set_layout(
        child,
        Layout::new().logical_size(80.0, 64.0).flex_shrink(0.0),
    );
    ui.set_local(
        child,
        ui.properties().surface.background,
        Some(Brush::Solid(Color::from_rgba8(0x00, 0x00, 0xff, 0xff))),
    );

    ui.compute_layout(Size::new(120.0, 80.0));
    let mut output = Scene::new();
    ui.paint(&mut output);

    let scroll_layer = ui.composition.layer_for(scroll).unwrap();
    let child_layer = ui.composition.layer_for(child).unwrap();
    assert!(ui.composition_layers().clip(scroll_layer).is_some());
    assert!(ui.composition_layers().content(child_layer).is_some());
    assert!(
        output
            .commands()
            .iter()
            .any(|command| matches!(command, Command::PushClip(_)))
    );
}

#[test]
fn press_marks_templated_control_not_inner_part() {
    // Pressing on a templated control's part (Button's label
    // `Text`) should put `:pressed` on the Button itself, not
    // on the label. Without this, the cascade rule keyed on
    // `TYPE_BUTTON:pressed` never matches and the button shows
    // no press feedback. Use `press_visual_target` directly so
    // the test doesn't depend on layout + hit-test plumbing.
    use crate::{Button, Text};

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, Button::new("Click me"));
    let label = ui.part(button, crate::PART_BUTTON_LABEL).unwrap();
    let retained_label = ui
        .slot_children(button, crate::SLOT_BUTTON_CONTENT, |content| {
            content.add(Text::new("Retained"))
        })
        .expect("button template should expose content slot");

    assert_eq!(ui.press_visual_target(label), button);
    assert_eq!(ui.press_visual_target(retained_label), button);
    assert_eq!(ui.press_visual_target(button), button);
    assert_eq!(ui.press_visual_target(root), root);
}

#[test]
fn disabled_button_carries_state_and_ignores_builder_click() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    use crate::Button;

    let mut ui = Ui::new();
    let root = ui.root();
    let fired = Rc::new(Cell::new(false));
    let fired_for_handler = Rc::clone(&fired);
    let button = ui.append(
        root,
        Button::new("No")
            .disabled()
            .on_click(move |_ui, _event| fired_for_handler.set(true)),
    );

    assert!(!ui.is_enabled(button));
    assert!(ui.get(button).unwrap().pseudos().contains(&PSEUDO_DISABLED));

    ui.synthesize_click(button, Point::ZERO);
    assert!(
        !fired.get(),
        "disabled button click handler should not fire"
    );

    assert!(!ui.set_focus(Some(button)));
    assert_eq!(ui.focused(), None, "disabled button should reject focus");
}

#[test]
fn set_enabled_false_clears_focus_and_pressed_state() {
    use crate::Button;

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, Button::new("Run"));

    assert!(ui.set_focus(Some(button)));
    ui.set_press_target(Some(button));
    ui.set_pointer_capture(Some(button));
    ui.set_enabled(button, false);

    assert!(!ui.is_enabled(button));
    assert_eq!(ui.focused(), None);
    assert_eq!(ui.events.press_target(), None);
    assert_eq!(ui.pointer_capture(), None);
    let pseudos = ui.get(button).unwrap().pseudos();
    assert!(pseudos.contains(&PSEUDO_DISABLED));
    assert!(!pseudos.contains(&PSEUDO_FOCUSED));
    assert!(!pseudos.contains(&PSEUDO_FOCUS_VISIBLE));
    assert!(!pseudos.contains(&PSEUDO_PRESSED));

    ui.set_enabled(button, true);
    assert!(ui.is_enabled(button));
    assert!(
        ui.is_focusable(button),
        "re-enabling should restore the stored focusable preference"
    );
    assert!(ui.set_focus(Some(button)));
}

#[test]
fn direct_is_checked_write_updates_checked_pseudo() {
    use crate::CheckBox;

    let mut ui = Ui::new();
    let root = ui.root();
    let checkbox = ui.append(root, CheckBox::new("Enabled"));

    ui.set_local(checkbox, ui.properties().is_checked, true);

    assert!(
        ui.get(checkbox)
            .unwrap()
            .pseudos()
            .contains(&PSEUDO_CHECKED)
    );
}

#[test]
fn direct_is_enabled_write_clears_focus_and_pressed_state() {
    use crate::Button;

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, Button::new("Run"));

    assert!(ui.set_focus(Some(button)));
    ui.set_press_target(Some(button));
    ui.set_pointer_capture(Some(button));
    ui.set_local(button, ui.properties().is_enabled, false);

    assert!(!ui.is_enabled(button));
    assert_eq!(ui.focused(), None);
    assert_eq!(ui.events.press_target(), None);
    assert_eq!(ui.pointer_capture(), None);
    let pseudos = ui.get(button).unwrap().pseudos();
    assert!(pseudos.contains(&PSEUDO_DISABLED));
    assert!(!pseudos.contains(&PSEUDO_FOCUSED));
    assert!(!pseudos.contains(&PSEUDO_FOCUS_VISIBLE));
    assert!(!pseudos.contains(&PSEUDO_PRESSED));
}

#[test]
fn disabled_toggle_and_slider_share_control_state_policy() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    use crate::{Slider, ToggleSwitch};

    let mut ui = Ui::new();
    let root = ui.root();

    let toggle = ui.append(root, ToggleSwitch::new().disabled());
    assert!(!ui.is_enabled(toggle));
    assert!(!ui.is_focusable(toggle));
    assert!(ui.get(toggle).unwrap().pseudos().contains(&PSEUDO_DISABLED));
    ui.synthesize_click(toggle, Point::ZERO);
    let checked = ui
        .get(toggle)
        .and_then(|el| el.get_local_value(ui.properties().is_checked).copied())
        .unwrap_or(false);
    assert!(!checked, "disabled toggle should ignore user activation");

    let fired = Rc::new(Cell::new(false));
    let fired_for_handler = Rc::clone(&fired);
    let slider = ui.append(root, Slider::new().disabled());
    ui.on_click(slider, move |_ui, _event| fired_for_handler.set(true));
    assert!(!ui.is_enabled(slider));
    assert!(!ui.is_focusable(slider));
    assert!(ui.get(slider).unwrap().pseudos().contains(&PSEUDO_DISABLED));
    ui.synthesize_click(slider, Point::ZERO);
    assert!(
        !fired.get(),
        "disabled slider should suppress click dispatch"
    );
}

#[test]
fn pointer_leave_clears_hover() {
    // Move events require constructing a PointerUpdate with platform
    // position types; we exercise that end-to-end via the demo. Here we
    // verify the dispatch shape: a Leave event clears whatever was
    // currently hovered.

    let mut ui = Ui::new();
    let root = ui.root();
    // Manually mark something hovered (this is what dispatch_pointer_event
    // does internally via add_pseudo on a Move).
    ui.add_pseudo(root, PSEUDO_HOVER);
    // Match the internal state. (Phase 1d's dispatch is the only writer
    // of `hovered` in real code, so this is a deliberate test-only nudge.)
    // We test the Leave path which clears via the public API.
    // Even if hovered is None (we never set it via dispatch), Leave
    // should be a no-op rather than panic.
    let _ = ui.dispatch_pointer_event(&pointer_leave());
    assert_eq!(ui.hovered(), None);
}

#[test]
fn add_class_restyles_subtree() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.add_class(root, ClassId(1));
    // `add_class` restyles synchronously — the class is on the
    // element afterward and any cascade-driven channel changes
    // are already marked via `restyle_subtree`. With an empty
    // cascade (this test's case) no channels move.
    assert!(ui.get(root).unwrap().classes().contains(&ClassId(1)));
}

#[test]
fn add_and_remove_class_by_name_use_style_vocabulary() {
    let mut ui = Ui::new();
    let root = ui.root();

    ui.add_class(root, ".runtime-class");
    let class = ui
        .style_vocabulary()
        .class_id_by_name(".runtime-class")
        .expect("adding by name should intern the class");
    assert!(ui.get(root).unwrap().classes().contains(&class));

    ui.remove_class(root, ".runtime-class");
    assert!(!ui.get(root).unwrap().classes().contains(&class));
    ui.remove_class(root, ".not-registered");
    assert_eq!(
        ui.style_vocabulary().class_id_by_name(".not-registered"),
        None,
        "removing an unknown name should not intern it",
    );
}

#[test]
fn text_builder_attaches_classes() {
    let mut ui = Ui::new();
    let root = ui.root();
    let text = ui.append(root, crate::Text::new("Styled").class(ClassId(42)));

    assert!(ui.get(text).unwrap().classes().contains(&ClassId(42)));
}

#[test]
fn text_builder_resolves_named_classes() {
    let mut ui = Ui::new();
    let root = ui.root();
    let text = ui.append(root, crate::Text::new("Styled").class(".named-label"));

    let class = ui
        .style_vocabulary()
        .class_id_by_name(".named-label")
        .expect("appending a named class should intern it");
    assert!(ui.get(text).unwrap().classes().contains(&class));
}

#[test]
fn element_spec_ext_applies_shared_settings() {
    use crate::ElementSpecExt;

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(
        root,
        crate::Button::new("Run")
            .class(ClassId(77))
            .layout(Layout::new().logical_size(80.0, 30.0))
            .disabled(),
    );

    assert!(ui.get(button).unwrap().classes().contains(&ClassId(77)));
    assert!(!ui.is_enabled(button));
    assert!(!ui.is_focusable(button));
    assert_eq!(
        ui.effective_layout(button),
        Layout::new()
            .container(crate::LayoutContainer::row())
            .logical_size(80.0, 30.0)
            .align_center()
            .justify_center()
    );
    assert_eq!(
        ui.resolve_value_for(
            ui.get(button).unwrap(),
            ui.properties().surface.padding_widths.top
        ),
        18.0
    );
    assert_eq!(
        ui.resolve_value_for(
            ui.get(button).unwrap(),
            ui.properties().surface.padding_widths.left
        ),
        32.0
    );
}

#[test]
fn inspection_snapshot_reports_effective_property_summaries() {
    use crate::inspection::{ElementPropertyInspectionGroup, ElementPropertyInspectionValue};
    use crate::{SurfaceSpecExt, TextSpecExt};

    let mut ui = Ui::new();
    let root = ui.root();
    let background = Color::from_rgba8(0x20, 0x40, 0x80, 0xff);
    let panel = ui.append(
        root,
        crate::Panel::new()
            .background(Some(background.into()))
            .padding_xy(12.0, 6.0)
            .font_size(18.0)
            .layout(Layout::row(8.0).logical_size(120.0, 64.0))
            .opacity(0.75),
    );

    let snapshot = ui.inspection_snapshot();
    let row = snapshot
        .elements
        .iter()
        .find(|row| row.id == panel)
        .expect("configured panel should be inspectable");

    assert!(
        row.properties.iter().any(|property| {
            property.name == "Background"
                && property.group == ElementPropertyInspectionGroup::Appearance
                && property.value == ElementPropertyInspectionValue::Color(background.to_rgba8())
        }),
        "snapshot should expose effective appearance properties"
    );
    assert!(
        row.properties.iter().any(|property| {
            property.name == "Padding left"
                && property.group == ElementPropertyInspectionGroup::Box
                && property.value == ElementPropertyInspectionValue::Number(12.0)
        }),
        "snapshot should expose box model properties separately from appearance"
    );
    assert!(
        row.properties.iter().any(|property| {
            property.name == "FontSize"
                && property.group == ElementPropertyInspectionGroup::Text
                && property.value == ElementPropertyInspectionValue::Number(18.0)
        }),
        "snapshot should expose effective text properties"
    );
    assert!(
        row.properties.iter().any(|property| {
            property.name == "Opacity"
                && property.group == ElementPropertyInspectionGroup::Motion
                && property.value == ElementPropertyInspectionValue::Number(0.75)
        }),
        "snapshot should expose effective motion properties"
    );
    assert!(
        row.properties.iter().any(|property| {
            property.name == "LayoutContainer"
                && property.group == ElementPropertyInspectionGroup::Layout
                && matches!(
                    &property.value,
                    ElementPropertyInspectionValue::Record { summary, fields }
                        if summary == "display flex, direction row"
                            && fields.iter().any(|field| field.name == "Display"
                                && field.value == ElementPropertyInspectionValue::Token(String::from("flex")))
                            && fields.iter().any(|field| field.name == "Direction"
                                && field.value == ElementPropertyInspectionValue::Token(String::from("row")))
                )
        }),
        "snapshot should expose readable container layout properties"
    );
    assert!(
        row.properties.iter().any(|property| {
            property.name == "LayoutSize"
                && property.group == ElementPropertyInspectionGroup::Layout
                && matches!(
                    &property.value,
                    ElementPropertyInspectionValue::Record { summary, fields }
                        if summary == "width 120px, height 64px"
                            && fields.iter().any(|field| field.name == "Width"
                                && field.value == ElementPropertyInspectionValue::Token(String::from("120px")))
                            && fields.iter().any(|field| field.name == "Height"
                                && field.value == ElementPropertyInspectionValue::Token(String::from("64px")))
                )
        }),
        "snapshot should expose readable size layout properties"
    );
    assert!(
        row.properties.iter().any(|property| {
            property.name == "LayoutSpacing"
                && property.group == ElementPropertyInspectionGroup::Layout
                && matches!(
                    &property.value,
                    ElementPropertyInspectionValue::Record { summary, fields }
                        if summary == "gap 8px"
                            && fields.iter().any(|field| field.name == "Gap"
                                && field.value == ElementPropertyInspectionValue::Token(String::from("8px")))
                )
        }),
        "snapshot should expose readable spacing layout properties"
    );
}

#[test]
fn inspection_snapshot_reports_style_vocabulary_names() {
    let mut ui = Ui::new();
    assert_eq!(
        ui.style_vocabulary().resource_name(crate::theme::ACCENT_BG),
        Some("accent.bg")
    );
    let vocabulary = ui.style_vocabulary_mut();
    let class = vocabulary.class_id(".named-class");
    let pseudo = vocabulary.pseudo_class_id(":named-pseudo");
    let type_tag = vocabulary.type_tag("NamedWidget");
    let resource = vocabulary.resource_key("named.resource");
    let part_tag = vocabulary.part_tag(type_tag, "NamedWidget.Part");

    let snapshot = ui.inspection_snapshot();

    assert_eq!(
        snapshot.style_vocabulary.class_name(class),
        Some(".named-class")
    );
    assert_eq!(
        snapshot.style_vocabulary.pseudo_name(pseudo),
        Some(":named-pseudo")
    );
    assert_eq!(
        snapshot.style_vocabulary.type_name(type_tag),
        Some("NamedWidget")
    );
    assert_eq!(
        snapshot.style_vocabulary.resource_name(resource),
        Some("named.resource")
    );
    assert_eq!(
        snapshot.style_vocabulary.part_name(type_tag, part_tag),
        Some("NamedWidget.Part")
    );
}

#[test]
fn ui_resolves_style_token_sets() {
    struct TestTokens;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct TestClasses {
        class: ClassId,
        resource: ResourceKey,
    }

    impl StyleTokenSet for TestTokens {
        type Resolved = TestClasses;

        fn resolve(vocabulary: &mut StyleVocabulary) -> Self::Resolved {
            TestClasses {
                class: vocabulary.class_id(".ui-token"),
                resource: vocabulary.resource_key("ui.token.resource"),
            }
        }
    }

    let mut ui = Ui::new();

    let tokens = ui.style_tokens::<TestTokens>();

    assert_eq!(
        ui.style_vocabulary().class_name(tokens.class),
        Some(".ui-token")
    );
    assert_eq!(
        ui.style_vocabulary().resource_name(tokens.resource),
        Some("ui.token.resource")
    );
    assert_eq!(ui.style_tokens::<TestTokens>(), tokens);
}

#[test]
fn element_spec_ext_applies_initial_local_properties() {
    use crate::{SurfaceSpecExt, TextSpecExt};

    let mut ui = Ui::new();
    let root = ui.root();
    let background = Brush::from(Color::from_rgba8(0x22, 0x33, 0x44, 0xff));
    let foreground = Brush::from(Color::from_rgba8(0xee, 0xdd, 0xcc, 0xff));
    let corner_radius = crate::CornerRadius::circular(6.0);
    let panel = ui.append(
        root,
        crate::Panel::new()
            .background(background.clone())
            .foreground(foreground.clone())
            .border_width(2.0)
            .corner_radius(6.0)
            .padding_xy(14.0, 8.0)
            .opacity(0.5)
            .content("Configured"),
    );
    let element = ui.get(panel).unwrap();

    assert_eq!(
        element.get_local_value(ui.properties().surface.background),
        Some(&Some(background))
    );
    assert_eq!(
        element.get_local_value(ui.properties().foreground),
        Some(&foreground)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.border_widths.top),
        Some(&2.0)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.border_widths.right),
        Some(&2.0)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.corner_radii.top_left),
        Some(&corner_radius)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.corner_radii.bottom_right),
        Some(&corner_radius)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.padding_widths.top),
        Some(&8.0)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.padding_widths.left),
        Some(&14.0)
    );
    assert_eq!(element.get_local_value(ui.properties().opacity), Some(&0.5));
    assert_eq!(
        element.get_local_value(ui.properties().content),
        Some(&String::from("Configured"))
    );
}

#[test]
fn surface_padding_shorthand_writes_surface_padding() {
    let mut ui = Ui::new();
    let root = ui.root();
    let panel = ui.append(
        root,
        crate::Panel::new()
            .padding_xy(12.0, 8.0)
            .layout(Layout::row(4.0)),
    );
    let element = ui.get(panel).unwrap();

    assert_eq!(
        element.get_local_value(ui.properties().surface.padding_widths.top),
        Some(&8.0)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.padding_widths.right),
        Some(&12.0)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.padding_widths.bottom),
        Some(&8.0)
    );
    assert_eq!(
        element.get_local_value(ui.properties().surface.padding_widths.left),
        Some(&12.0)
    );
    assert_eq!(
        element.get_local_value(ui.properties().layout_spacing),
        Some(&crate::LayoutSpacing::gap(4.0))
    );
}

fn assert_surface_padding(
    ui: &Ui,
    element: ElementId,
    top: f64,
    right: f64,
    bottom: f64,
    left: f64,
) {
    let element = ui.get(element).expect("element should be live");
    let padding = ui.properties().surface.padding_widths;
    assert_eq!(element.get_local_value(padding.top), Some(&top));
    assert_eq!(element.get_local_value(padding.right), Some(&right));
    assert_eq!(element.get_local_value(padding.bottom), Some(&bottom));
    assert_eq!(element.get_local_value(padding.left), Some(&left));
}

fn assert_layout_spacing_has_no_padding(ui: &Ui, element: ElementId) {
    let element = ui.get(element).expect("element should be live");
    if let Some(spacing) = element.get_local_value(ui.properties().layout_spacing) {
        assert!(
            !spacing.inspection_summary().contains("padding"),
            "layout spacing should not carry box padding"
        );
    }
}

#[test]
fn element_spec_ext_applies_text_and_range_property_helpers() {
    use crate::{RangeSpecExt, TextSpecExt};

    let mut ui = Ui::new();
    let root = ui.root();
    let feature = parley::FontFeature::parse_css_list("\"tnum\" on")
        .next()
        .expect("feature should parse")
        .expect("feature should be valid");
    let text = ui.append(
        root,
        crate::Text::new("0000")
            .text("1234")
            .font_features(Box::from([feature]))
            .letter_spacing(1.25)
            .text_wrap_mode(crate::TextWrapMode::Wrap),
    );
    let slider = ui.append(
        root,
        crate::Panel::new()
            .range(-1.0, 1.0)
            .value(0.25)
            .small_change(0.1)
            .large_change(0.5),
    );

    assert_eq!(
        ui.get(text).unwrap().get_local_value(ui.properties().text),
        Some(&String::from("1234"))
    );
    assert_eq!(
        ui.get(text)
            .unwrap()
            .get_local_value(ui.properties().font_features)
            .map(|features| features.as_ref()),
        Some(&[feature][..])
    );
    assert_eq!(
        ui.get(text)
            .unwrap()
            .get_local_value(ui.properties().letter_spacing),
        Some(&1.25)
    );
    assert_eq!(
        ui.get(text)
            .unwrap()
            .get_local_value(ui.properties().text_wrap_mode),
        Some(&crate::TextWrapMode::Wrap)
    );
    assert_eq!(
        ui.get(slider)
            .unwrap()
            .get_local_value(ui.properties().minimum),
        Some(&-1.0)
    );
    assert_eq!(
        ui.get(slider)
            .unwrap()
            .get_local_value(ui.properties().maximum),
        Some(&1.0)
    );
    assert_eq!(
        ui.get(slider)
            .unwrap()
            .get_local_value(ui.properties().value),
        Some(&0.25)
    );
    assert_eq!(
        ui.get(slider)
            .unwrap()
            .get_local_value(ui.properties().small_change),
        Some(&0.1)
    );
    assert_eq!(
        ui.get(slider)
            .unwrap()
            .get_local_value(ui.properties().large_change),
        Some(&0.5)
    );
}

#[test]
fn element_spec_ext_local_property_accepts_registered_property_handles() {
    use crate::ElementSpecExt;

    let mut ui = Ui::new();
    let root = ui.root();
    let letter_spacing = ui.properties().letter_spacing;
    let text = ui.append(root, crate::Text::new("Tracked").local(letter_spacing, 2.5));

    assert_eq!(
        ui.get(text)
            .unwrap()
            .get_local_value(ui.properties().letter_spacing),
        Some(&2.5)
    );
}

#[test]
fn element_spec_ext_replaces_repeated_initial_property_writes() {
    use crate::SurfaceSpecExt;

    let mut ui = Ui::new();
    let root = ui.root();
    let panel = ui.append(root, crate::Panel::new().opacity(0.25).opacity(0.75));

    assert_eq!(
        ui.get(panel)
            .unwrap()
            .get_local_value(ui.properties().opacity),
        Some(&0.75)
    );
}

#[test]
fn more_control_specs_share_element_settings() {
    use crate::ElementSpecExt;

    let mut ui = Ui::new();
    let root = ui.root();
    let class = ClassId(88);
    let layout_size = crate::LayoutSize::logical(72.0, 24.0);
    let layout = Layout::new().size(layout_size);

    let divider = ui.append(
        root,
        crate::Divider::horizontal()
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );
    let spinner = ui.append(
        root,
        crate::Spinner::new()
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );
    let viewport = ui.append(
        root,
        crate::Viewport::new()
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );
    let splitter = ui.append(
        root,
        crate::Splitter::vertical()
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );
    let list = ui.append(
        root,
        crate::ListBox::new()
            .items(["A", "B"])
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );
    let select = ui.append(
        root,
        crate::Select::new()
            .items(["A", "B"])
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );
    let outline = ui.append(
        root,
        crate::OutlineView::new()
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );
    let scroll_view = ui.append(
        root,
        crate::ScrollView::new()
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );
    let scroll_area = ui.append(
        root,
        crate::ScrollArea::new()
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );
    let scroll_bar = ui.append(
        root,
        crate::ScrollBar::vertical(scroll_view)
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );
    let popup = ui.append(
        root,
        crate::Popup::new()
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );
    let tooltip = ui.append(
        root,
        crate::Tooltip::new("Details")
            .class(class)
            .layout(layout.clone())
            .disabled(),
    );

    for id in [
        divider,
        spinner,
        viewport,
        splitter,
        list,
        select,
        outline,
        scroll_view,
        scroll_area,
        scroll_bar,
        popup,
        tooltip,
    ] {
        assert!(ui.get(id).unwrap().classes().contains(&class));
        assert!(!ui.is_enabled(id));
        assert_eq!(
            ui.get(id)
                .unwrap()
                .get_local_value(ui.properties().layout_size),
            Some(&layout_size)
        );
    }
}

fn slider_value(ui: &Ui, slider: ElementId) -> Option<f64> {
    ui.get(slider)
        .and_then(|el| el.get_local_value(ui.properties().value).copied())
}
