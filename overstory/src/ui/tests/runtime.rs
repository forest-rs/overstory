// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

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
