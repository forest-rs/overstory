use alloc::{rc::Rc, string::String, vec::Vec};
use core::cell::{Cell, RefCell};

use super::widget::text_input_text_run_node_id;
use super::*;
use crate::accessibility::accesskit_node_id;
use crate::element::ElementId;
use crate::layouts::{Layout, LayoutDirection};
use crate::pseudos::{PSEUDO_PLACEHOLDER_SHOWN, PSEUDO_READ_ONLY};
use crate::test_support::{pointer_down, pointer_move, pointer_up};
use crate::ui::Ui;
use crate::{Language, ParagraphDirection, TextSpecExt};
use accesskit::{
    Action, ActionData, ActionRequest, Node, NodeId, Role, TextPosition, TextSelection, TreeId,
    TreeUpdate,
};
use cursor_icon::CursorIcon;
use kurbo::{Point, Size};
use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};
use understory_property::DependencyObjectExt;

#[test]
fn text_input_edits_content_from_keyboard() {
    let mut ui = Ui::new();
    let root = ui.root();
    let input = ui.append(root, TextInput::new().placeholder("Search"));
    ui.set_focus(Some(input));

    let key = KeyboardEvent::key_down(Key::Character(String::from("a")), Code::KeyA);
    let result = ui.dispatch_key(&key);
    assert!(result.handled);
    assert_eq!(
        ui.get(input)
            .and_then(|el| el.get_local_value(ui.properties().content))
            .map(String::as_str),
        Some("a")
    );

    let backspace = KeyboardEvent::key_down(Key::Named(NamedKey::Backspace), Code::Backspace);
    let result = ui.dispatch_key(&backspace);
    assert!(result.handled);
    assert_eq!(
        ui.get(input)
            .and_then(|el| el.get_local_value(ui.properties().content))
            .map(String::as_str),
        Some("")
    );
}

#[test]
fn text_input_enter_submits_text_and_can_clear() {
    let submitted = Rc::new(RefCell::new(Vec::new()));
    let seen = submitted.clone();

    let mut ui = Ui::new();
    let root = ui.root();
    let input = ui.append(
        root,
        TextInput::new()
            .text("send me")
            .on_submit(move |_ui, event| {
                seen.borrow_mut().push(String::from(event.text()));
                TextInputSubmitAction::Clear
            }),
    );
    ui.set_focus(Some(input));

    let enter = KeyboardEvent::key_down(Key::Named(NamedKey::Enter), Code::Enter);
    let result = ui.dispatch_key(&enter);

    assert!(result.handled);
    assert_eq!(submitted.borrow().as_slice(), ["send me"]);
    assert_eq!(text_input_text(&ui, input).as_deref(), Some(""));
}

#[test]
fn text_input_text_changed_reports_old_new_and_source() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let seen = changes.clone();

    let mut ui = Ui::new();
    let root = ui.root();
    let input = ui.append(
        root,
        TextInput::new().on_text_changed(move |_ui, event| {
            seen.borrow_mut().push(event);
        }),
    );
    ui.set_focus(Some(input));

    let key = KeyboardEvent::key_down(Key::Character(String::from("a")), Code::KeyA);
    assert!(ui.dispatch_key(&key).handled);

    assert_eq!(
        changes.borrow().as_slice(),
        [TextInputTextChangedEvent {
            input,
            previous_text: String::new(),
            text: String::from("a"),
            source: TextInputTextChangeSource::Keyboard,
        }]
    );
}

#[test]
fn text_input_text_changed_reports_programmatic_and_accessibility_sources() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let seen = changes.clone();

    let mut ui = Ui::new();
    let root = ui.root();
    let input = ui.append(
        root,
        TextInput::new()
            .text("old")
            .on_text_changed(move |_ui, event| {
                seen.borrow_mut().push(event);
            }),
    );

    assert!(set_text_input_text(&mut ui, input, "new"));
    assert!(ui.handle_accesskit_action(&text_action(input, Action::SetValue, "final")));

    assert_eq!(
        changes.borrow().as_slice(),
        [
            TextInputTextChangedEvent {
                input,
                previous_text: String::from("old"),
                text: String::from("new"),
                source: TextInputTextChangeSource::Programmatic,
            },
            TextInputTextChangedEvent {
                input,
                previous_text: String::from("new"),
                text: String::from("final"),
                source: TextInputTextChangeSource::Accessibility,
            },
        ]
    );
}

#[test]
fn text_changed_handler_can_query_and_mutate_same_input() {
    let input_id = Rc::new(Cell::new(None));
    let input_id_for_handler = Rc::clone(&input_id);
    let observations = Rc::new(RefCell::new(Vec::new()));
    let observations_for_handler = Rc::clone(&observations);
    let mut ui = Ui::new();
    let root = ui.root();
    let input = ui.append(
        root,
        TextInput::new().on_text_changed(move |ui, event| {
            let input = input_id_for_handler
                .get()
                .expect("text input id is installed before text changes");
            observations_for_handler.borrow_mut().push((
                ui.get(input).and_then(|element| element.widget()).is_some(),
                text_input_text(ui, input),
                event.text.clone(),
            ));
            if event.text == "first" {
                assert!(set_text_input_text(ui, input, "second"));
            }
        }),
    );
    input_id.set(Some(input));

    assert!(set_text_input_text(&mut ui, input, "first"));

    assert_eq!(
        observations.borrow().as_slice(),
        [
            (true, Some(String::from("first")), String::from("first")),
            (true, Some(String::from("second")), String::from("second")),
        ]
    );
    assert_eq!(text_input_text(&ui, input).as_deref(), Some("second"));
}

#[test]
fn submit_handler_can_mutate_same_input_before_submit_action() {
    let input_id = Rc::new(Cell::new(None));
    let input_id_for_handler = Rc::clone(&input_id);
    let widget_was_installed = Rc::new(Cell::new(false));
    let widget_was_installed_for_handler = Rc::clone(&widget_was_installed);
    let mut ui = Ui::new();
    let root = ui.root();
    let input = ui.append(
        root,
        TextInput::new()
            .text("send me")
            .on_submit(move |ui, _event| {
                let input = input_id_for_handler
                    .get()
                    .expect("text input id is installed before submission");
                widget_was_installed_for_handler
                    .set(ui.get(input).and_then(|element| element.widget()).is_some());
                assert!(set_text_input_text(ui, input, "handler edit"));
                TextInputSubmitAction::Replace(String::from("submit action"))
            }),
    );
    input_id.set(Some(input));
    ui.set_focus(Some(input));

    let enter = KeyboardEvent::key_down(Key::Named(NamedKey::Enter), Code::Enter);
    assert!(ui.dispatch_key(&enter).handled);

    assert!(widget_was_installed.get());
    assert_eq!(
        text_input_text(&ui, input).as_deref(),
        Some("submit action")
    );
}

#[test]
fn text_input_escape_rejects_edit_session() {
    let rejected = Rc::new(RefCell::new(Vec::new()));
    let seen = rejected.clone();

    let mut ui = Ui::new();
    let root = ui.root();
    let input = ui.append(
        root,
        TextInput::new()
            .text("draft")
            .on_edit_rejected(move |_ui, event| {
                seen.borrow_mut().push(event);
            }),
    );
    ui.set_focus(Some(input));

    let key = KeyboardEvent::key_down(Key::Character(String::from("x")), Code::KeyX);
    assert!(ui.dispatch_key(&key).handled);
    assert_eq!(text_input_text(&ui, input).as_deref(), Some("draftx"));

    let escape = KeyboardEvent::key_down(Key::Named(NamedKey::Escape), Code::Escape);
    assert!(ui.dispatch_key(&escape).handled);

    assert_eq!(text_input_text(&ui, input).as_deref(), Some("draft"));
    assert_eq!(
        rejected.borrow().as_slice(),
        [TextInputEditRejectedEvent {
            input,
            restored_text: String::from("draft"),
            rejected_text: String::from("draftx"),
            reason: TextInputEditRejectReason::Escape,
        }]
    );
}

#[test]
fn text_input_focus_loss_commits_edit_session() {
    let committed = Rc::new(RefCell::new(Vec::new()));
    let seen = committed.clone();

    let mut ui = Ui::new();
    let root = ui.root();
    let input = ui.append(
        root,
        TextInput::new()
            .text("draft")
            .on_edit_committed(move |_ui, event| {
                seen.borrow_mut().push(event);
            }),
    );
    ui.set_focus(Some(input));

    let key = KeyboardEvent::key_down(Key::Character(String::from("x")), Code::KeyX);
    assert!(ui.dispatch_key(&key).handled);
    assert!(ui.set_focus(None));

    assert_eq!(
        committed.borrow().as_slice(),
        [TextInputEditCommittedEvent {
            input,
            previous_text: String::from("draft"),
            text: String::from("draftx"),
            reason: TextInputEditCommitReason::FocusLost,
        }]
    );
}

#[test]
fn text_input_placeholder_shown_tracks_empty_text() {
    let mut ui = Ui::new();
    let root = ui.root();
    let input = ui.append(root, TextInput::new().placeholder("Search"));

    assert!(
        ui.get(input)
            .map(|el| el.pseudos().contains(&PSEUDO_PLACEHOLDER_SHOWN))
            .unwrap_or(false)
    );

    ui.set_focus(Some(input));
    let key = KeyboardEvent::key_down(Key::Character(String::from("a")), Code::KeyA);
    assert!(ui.dispatch_key(&key).handled);
    assert!(
        !ui.get(input)
            .map(|el| el.pseudos().contains(&PSEUDO_PLACEHOLDER_SHOWN))
            .unwrap_or(false)
    );

    let backspace = KeyboardEvent::key_down(Key::Named(NamedKey::Backspace), Code::Backspace);
    assert!(ui.dispatch_key(&backspace).handled);
    assert!(
        ui.get(input)
            .map(|el| el.pseudos().contains(&PSEUDO_PLACEHOLDER_SHOWN))
            .unwrap_or(false)
    );
}

#[test]
fn text_input_select_all_replaces_text() {
    use ui_events::keyboard::Modifiers;

    for modifiers in [Modifiers::CONTROL, Modifiers::META] {
        let mut ui = Ui::new();
        let root = ui.root();
        let input = ui.append(root, TextInput::new().text("hello"));
        ui.set_focus(Some(input));

        let mut select_all = KeyboardEvent::key_down(Key::Character(String::from("a")), Code::KeyA);
        select_all.modifiers = modifiers;
        assert!(ui.dispatch_key(&select_all).handled);

        let replacement = KeyboardEvent::key_down(Key::Character(String::from("x")), Code::KeyX);
        assert!(ui.dispatch_key(&replacement).handled);
        assert_eq!(
            ui.get(input)
                .and_then(|el| el.get_local_value(ui.properties().content))
                .map(String::as_str),
            Some("x")
        );
    }
}

#[test]
fn text_input_shift_arrows_extend_selection_for_replacement() {
    let mut ui = Ui::new();
    let root = ui.root();
    let input = ui.append(root, TextInput::new().text("hello"));
    ui.set_focus(Some(input));

    let mut left = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowLeft), Code::ArrowLeft);
    left.modifiers = ui_events::keyboard::Modifiers::SHIFT;
    assert!(ui.dispatch_key(&left).handled);
    assert!(ui.dispatch_key(&left).handled);

    let replacement = KeyboardEvent::key_down(Key::Character(String::from("y")), Code::KeyY);
    assert!(ui.dispatch_key(&replacement).handled);
    assert_eq!(
        ui.get(input)
            .and_then(|el| el.get_local_value(ui.properties().content))
            .map(String::as_str),
        Some("hely")
    );
}

#[test]
fn text_input_caret_tracks_shaped_text_width() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(360.0, 80.0));
    let input = ui.append(root, TextInput::new().text("George of he Jungle"));
    let text = ui.slot(input, SLOT_TEXT_INPUT_TEXT).unwrap();
    let caret = ui.slot(input, SLOT_TEXT_INPUT_CARET).unwrap();

    ui.compute_layout(Size::new(360.0, 80.0));

    let text_bounds = ui.effective_bounds(text);
    let caret_bounds = ui.effective_bounds(caret);
    assert!(
        caret_bounds.x0 > text_bounds.x0 && caret_bounds.x1 <= text_bounds.x1,
        "caret should land at the shaped position inside the text line; text={text_bounds:?}, caret={caret_bounds:?}"
    );
}

#[test]
fn runtime_direction_change_realigns_editor_without_losing_selection_or_focus() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(360.0, 80.0));
    let input = ui.append(root, TextInput::new().text("George"));
    let caret = ui.slot(input, SLOT_TEXT_INPUT_CARET).unwrap();
    assert!(ui.set_focus(Some(input)));

    ui.compute_layout(Size::new(360.0, 80.0));
    let ltr_caret = ui.effective_bounds(caret);
    let ltr_update = ui.accesskit_tree_update();
    let selection = accesskit_node_for(&ltr_update, input)
        .text_selection()
        .cloned();

    ui.set_layout_direction(root, LayoutDirection::Rtl);
    ui.compute_layout(Size::new(360.0, 80.0));
    let rtl_caret = ui.effective_bounds(caret);
    let rtl_update = ui.accesskit_tree_update();
    let input_node = accesskit_node_for(&rtl_update, input);
    let text_run = accesskit_node_for_id(&rtl_update, text_input_text_run_node_id(input));

    assert!(rtl_caret.x0 > ltr_caret.x0);
    assert_eq!(ui.focused(), Some(input));
    assert_eq!(input_node.text_selection(), selection.as_ref());
    assert_eq!(
        input_node.text_direction(),
        Some(accesskit::TextDirection::RightToLeft)
    );
    assert_eq!(
        text_run.text_direction(),
        Some(accesskit::TextDirection::RightToLeft)
    );
}

#[test]
fn runtime_paragraph_override_realigns_editor_without_changing_application_flow() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(360.0, 80.0));
    let input = ui.append(
        root,
        TextInput::new()
            .text("AR-1082-B")
            .paragraph_direction(ParagraphDirection::Ltr),
    );
    let caret = ui.slot(input, SLOT_TEXT_INPUT_CARET).unwrap();
    assert!(ui.set_focus(Some(input)));

    ui.compute_layout(Size::new(360.0, 80.0));
    let ltr_caret = ui.effective_bounds(caret);
    let ltr_update = ui.accesskit_tree_update();
    let selection = accesskit_node_for(&ltr_update, input)
        .text_selection()
        .cloned();
    assert_eq!(
        accesskit_node_for(&ltr_update, input).text_direction(),
        Some(accesskit::TextDirection::LeftToRight)
    );

    let property = ui.properties().paragraph_direction;
    ui.set_local(input, property, ParagraphDirection::Rtl);
    ui.compute_layout(Size::new(360.0, 80.0));
    let rtl_caret = ui.effective_bounds(caret);
    let rtl_update = ui.accesskit_tree_update();
    let input_node = accesskit_node_for(&rtl_update, input);
    let text_run = accesskit_node_for_id(&rtl_update, text_input_text_run_node_id(input));

    assert!(rtl_caret.x0 > ltr_caret.x0);
    assert_eq!(ui.layout_direction(input), Some(LayoutDirection::Ltr));
    assert_eq!(ui.focused(), Some(input));
    assert_eq!(input_node.text_selection(), selection.as_ref());
    assert_eq!(
        input_node.text_direction(),
        Some(accesskit::TextDirection::RightToLeft)
    );
    assert_eq!(
        text_run.text_direction(),
        Some(accesskit::TextDirection::RightToLeft)
    );

    ui.set_local(input, property, ParagraphDirection::Auto);
    ui.compute_layout(Size::new(360.0, 80.0));
    let auto_update = ui.accesskit_tree_update();
    assert_eq!(
        accesskit_node_for(&auto_update, input).text_direction(),
        None
    );
    assert_eq!(
        accesskit_node_for_id(&auto_update, text_input_text_run_node_id(input)).text_direction(),
        None
    );
    assert_eq!(ui.focused(), Some(input));
}

#[test]
fn text_input_reports_text_cursor_on_hover() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(360.0, 80.0));
    let input = ui.append(root, TextInput::new().text("edit me"));
    ui.compute_layout(Size::new(360.0, 80.0));

    let hover = pointer_move(ui.effective_bounds(input).center(), 1, false);
    assert!(ui.dispatch_pointer_event(&hover).changed());

    assert_eq!(ui.cursor_icon(), Some(CursorIcon::Text));
}

#[test]
fn text_input_collapses_selection_geometry_after_typing() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(360.0, 80.0));
    let input = ui.append(root, TextInput::new().text("hello"));
    let selection = ui.slot(input, SLOT_TEXT_INPUT_SELECTION).unwrap();
    ui.set_focus(Some(input));

    let mut left = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowLeft), Code::ArrowLeft);
    left.modifiers = ui_events::keyboard::Modifiers::SHIFT;
    assert!(ui.dispatch_key(&left).handled);
    ui.compute_layout(Size::new(360.0, 80.0));
    assert!(ui.effective_bounds(selection).width() > 0.0);

    let replacement = KeyboardEvent::key_down(Key::Character(String::from("y")), Code::KeyY);
    assert!(ui.dispatch_key(&replacement).handled);
    ui.compute_layout(Size::new(360.0, 80.0));
    assert_eq!(ui.effective_bounds(selection).width(), 0.0);
}

#[test]
fn read_only_text_input_allows_selection_without_mutating_content() {
    let mut ui = Ui::new();
    let root = ui.root();
    let input = ui.append(root, TextInput::new().text("locked").read_only(true));
    ui.set_focus(Some(input));

    let key = KeyboardEvent::key_down(Key::Character(String::from("x")), Code::KeyX);
    let result = ui.dispatch_key(&key);

    assert!(result.handled);
    assert_eq!(
        ui.get(input)
            .and_then(|el| el.get_local_value(ui.properties().content))
            .map(String::as_str),
        Some("locked")
    );
    assert!(
        ui.get(input)
            .map(|el| el.pseudos().contains(&PSEUDO_READ_ONLY))
            .unwrap_or(false)
    );
}

#[test]
fn direct_is_read_only_write_updates_pseudo_and_behavior() {
    let mut ui = Ui::new();
    let root = ui.root();
    let input = ui.append(root, TextInput::new().text("locked"));
    ui.set_focus(Some(input));

    ui.set_local(input, ui.properties().is_read_only, true);

    assert!(
        ui.get(input)
            .map(|el| el.pseudos().contains(&PSEUDO_READ_ONLY))
            .unwrap_or(false)
    );
    let key = KeyboardEvent::key_down(Key::Character(String::from("x")), Code::KeyX);
    assert!(ui.dispatch_key(&key).handled);
    assert_eq!(
        ui.get(input)
            .and_then(|el| el.get_local_value(ui.properties().content))
            .map(String::as_str),
        Some("locked")
    );

    ui.set_local(input, ui.properties().is_read_only, false);

    assert!(
        !ui.get(input)
            .map(|el| el.pseudos().contains(&PSEUDO_READ_ONLY))
            .unwrap_or(false)
    );
}

#[test]
fn text_input_accessibility_exposes_editable_value_and_placeholder() {
    let mut ui = Ui::new();
    let root = ui.root();
    let input = ui.append(
        root,
        TextInput::new()
            .text("George")
            .placeholder("Character name"),
    );
    ui.compute_layout(Size::new(360.0, 80.0));

    let update = ui.accesskit_tree_update();
    let node = accesskit_node_for(&update, input);

    assert_eq!(node.role(), Role::TextInput);
    assert_eq!(node.value(), Some("George"));
    assert_eq!(node.placeholder(), Some("Character name"));
    assert_eq!(node.label(), None);
    assert!(node.supports_action(Action::ReplaceSelectedText));
    assert!(node.supports_action(Action::SetValue));
    assert!(node.supports_action(Action::SetTextSelection));
    assert!(!node.is_read_only());

    let text_run_id = text_input_text_run_node_id(input);
    assert_eq!(node.children(), &[text_run_id]);
    assert_eq!(
        node.text_selection(),
        Some(&TextSelection {
            anchor: TextPosition {
                node: text_run_id,
                character_index: 6
            },
            focus: TextPosition {
                node: text_run_id,
                character_index: 6
            }
        })
    );

    let text_run = accesskit_node_for_id(&update, text_run_id);
    assert_eq!(text_run.role(), Role::TextRun);
    assert_eq!(text_run.value(), Some("George"));
    assert_eq!(text_run.character_lengths(), &[1, 1, 1, 1, 1, 1]);
}

#[test]
fn text_input_accessibility_uses_one_runtime_language_for_owner_and_text_run() {
    let arabic = Language::parse("ar-SA").unwrap();
    let english = Language::parse("EN-gb").unwrap();
    let french = Language::parse("fr-FR").unwrap();
    let mut ui = Ui::new();
    let root = ui.root();
    let property = ui.properties().text_language;
    ui.set_local(root, property, Some(arabic));
    let input = ui.append(
        root,
        TextInput::new().text("AR-1082-B").text_language(english),
    );
    ui.compute_layout(Size::new(360.0, 80.0));

    let update = ui.accesskit_tree_update();
    assert_eq!(accesskit_node_for(&update, input).language(), Some("en-GB"));
    assert_eq!(
        accesskit_node_for_id(&update, text_input_text_run_node_id(input)).language(),
        Some("en-GB")
    );

    ui.set_local(input, property, Some(french));
    let update = ui.accesskit_tree_update();
    assert_eq!(accesskit_node_for(&update, input).language(), Some("fr-FR"));
    assert_eq!(
        accesskit_node_for_id(&update, text_input_text_run_node_id(input)).language(),
        Some("fr-FR")
    );
    assert!(ui.get(input).is_some());
}

#[test]
fn read_only_text_input_accessibility_rejects_mutating_actions() {
    let mut ui = Ui::new();
    let root = ui.root();
    let input = ui.append(root, TextInput::new().text("locked").read_only(true));
    ui.compute_layout(Size::new(360.0, 80.0));

    let update = ui.accesskit_tree_update();
    let node = accesskit_node_for(&update, input);
    assert_eq!(node.value(), Some("locked"));
    assert!(node.is_read_only());
    assert!(node.supports_action(Action::SetTextSelection));
    assert!(!node.supports_action(Action::ReplaceSelectedText));
    assert!(!node.supports_action(Action::SetValue));

    let handled = ui.handle_accesskit_action(&text_action(input, Action::SetValue, "changed"));

    assert!(!handled);
    assert_eq!(
        ui.get(input)
            .and_then(|el| el.get_local_value(ui.properties().content))
            .map(String::as_str),
        Some("locked")
    );
}

#[test]
fn accesskit_set_value_updates_text_input_content_and_parts() {
    let mut ui = Ui::new();
    let root = ui.root();
    let input = ui.append(root, TextInput::new().text("old"));
    let text = ui.slot(input, SLOT_TEXT_INPUT_TEXT).unwrap();
    ui.compute_layout(Size::new(360.0, 80.0));

    let handled = ui.handle_accesskit_action(&text_action(input, Action::SetValue, "new"));

    assert!(handled);
    assert_eq!(
        ui.get(input)
            .and_then(|el| el.get_local_value(ui.properties().content))
            .map(String::as_str),
        Some("new")
    );
    assert_eq!(
        ui.get(text)
            .and_then(|el| el.get_local_value(ui.properties().text))
            .map(String::as_str),
        Some("new")
    );
}

#[test]
fn set_text_input_text_updates_editor_and_content() {
    let mut ui = Ui::new();
    let root = ui.root();
    let input = ui.append(root, TextInput::new().text("draft"));
    let text = ui.slot(input, SLOT_TEXT_INPUT_TEXT).unwrap();

    assert_eq!(text_input_text(&ui, input).as_deref(), Some("draft"));
    assert!(set_text_input_text(&mut ui, input, "sent"));
    assert_eq!(text_input_text(&ui, input).as_deref(), Some("sent"));
    assert_eq!(
        ui.get(text)
            .and_then(|el| el.get_local_value(ui.properties().text))
            .map(String::as_str),
        Some("sent")
    );
    assert!(!set_text_input_text(&mut ui, input, "sent"));
}

#[test]
fn accesskit_replace_selected_text_uses_plain_editor_selection() {
    let mut ui = Ui::new();
    let root = ui.root();
    let input = ui.append(root, TextInput::new().text("old"));
    ui.set_focus(Some(input));

    let mut select_all = KeyboardEvent::key_down(Key::Character(String::from("a")), Code::KeyA);
    select_all.modifiers = ui_events::keyboard::Modifiers::CONTROL;
    assert!(ui.dispatch_key(&select_all).handled);

    let handled =
        ui.handle_accesskit_action(&text_action(input, Action::ReplaceSelectedText, "new"));

    assert!(handled);
    assert_eq!(
        ui.get(input)
            .and_then(|el| el.get_local_value(ui.properties().content))
            .map(String::as_str),
        Some("new")
    );
}

#[test]
fn accesskit_set_text_selection_updates_plain_editor_selection() {
    let mut ui = Ui::new();
    let root = ui.root();
    let input = ui.append(root, TextInput::new().text("old"));
    ui.compute_layout(Size::new(360.0, 80.0));

    let selection = TextSelection {
        anchor: TextPosition {
            node: text_input_text_run_node_id(input),
            character_index: 1,
        },
        focus: TextPosition {
            node: text_input_text_run_node_id(input),
            character_index: 3,
        },
    };
    let selected = ui.handle_accesskit_action(&selection_action(input, selection));
    let replaced =
        ui.handle_accesskit_action(&text_action(input, Action::ReplaceSelectedText, "X"));

    assert!(selected);
    assert!(replaced);
    assert_eq!(
        ui.get(input)
            .and_then(|el| el.get_local_value(ui.properties().content))
            .map(String::as_str),
        Some("oX")
    );
}

#[test]
fn text_input_drag_extends_selection_and_ignores_trailing_click() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(360.0, 80.0));
    let input = ui.append(root, TextInput::new().text("drag selection"));
    let selection = ui.slot(input, SLOT_TEXT_INPUT_SELECTION).unwrap();
    ui.compute_layout(Size::new(360.0, 80.0));

    let bounds = ui.effective_bounds(input);
    let press = Point::new(bounds.x0 + 16.0, bounds.center().y);
    let drag = Point::new(bounds.x0 + 96.0, bounds.center().y);
    assert!(ui.dispatch_pointer_event(&pointer_down(press, 1)).changed());
    assert!(
        ui.dispatch_pointer_event(&pointer_move(drag, 2, true))
            .changed()
    );
    assert!(ui.dispatch_pointer_event(&pointer_up(drag, 3)).changed());
    ui.compute_layout(Size::new(360.0, 80.0));

    assert!(ui.effective_bounds(selection).width() > 0.0);
}

fn text_action(input: ElementId, action: Action, value: &str) -> ActionRequest {
    ActionRequest {
        action,
        target_tree: TreeId::ROOT,
        target_node: accesskit_node_id(input),
        data: Some(ActionData::Value(String::from(value).into_boxed_str())),
    }
}

fn selection_action(input: ElementId, selection: TextSelection) -> ActionRequest {
    ActionRequest {
        action: Action::SetTextSelection,
        target_tree: TreeId::ROOT,
        target_node: accesskit_node_id(input),
        data: Some(ActionData::SetTextSelection(selection)),
    }
}

fn accesskit_node_for(update: &TreeUpdate, element: ElementId) -> &Node {
    accesskit_node_for_id(update, accesskit_node_id(element))
}

fn accesskit_node_for_id(update: &TreeUpdate, node_id: NodeId) -> &Node {
    update
        .nodes
        .iter()
        .find(|(id, _)| *id == node_id)
        .map(|(_, node)| node)
        .expect("node should exist")
}
