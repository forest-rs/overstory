// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

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
    use crate::BindingSpecExt;
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
                            actions.add(crate::Button::new("Deploy").on_activate(
                                move |ui, _event| {
                                    deploy_count.update(ui, |count| *count += 1);
                                },
                            ))
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
