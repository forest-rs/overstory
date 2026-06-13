use kurbo::{Point, Size};
use understory_property::DependencyObjectExt;

use super::pages::show_demo_page;
use super::*;

fn assert_close(actual: f64, expected: f64, label: &str) {
    assert!(
        (actual - expected).abs() < 0.01,
        "{label}: expected {expected}, got {actual}",
    );
}

#[test]
fn gallery_shell_fills_window_content_and_nav_accent() {
    let mut ui = build_ui();
    ui.compute_layout(Size::new(
        f64::from(INITIAL_SIZE.0),
        f64::from(INITIAL_SIZE.1),
    ));

    let root = ui.root();
    let root_children = ui.get(root).expect("root should exist").children();
    let [nav, stage] = *root_children else {
        panic!("gallery root should contain nav and stage");
    };

    let nav_bounds = ui.effective_bounds(nav);
    let stage_bounds = ui.effective_bounds(stage);
    let expected_content_height = f64::from(INITIAL_SIZE.1) - (2.0 * f64::from(ROOT_PADDING));
    assert_close(nav_bounds.height(), expected_content_height, "nav height");
    assert_close(
        stage_bounds.height(),
        expected_content_height,
        "stage height",
    );

    let nav_children = ui.get(nav).expect("nav should exist").children();
    let marker = nav_children
        .get(2)
        .copied()
        .expect("nav marker should be third child");
    assert_close(
        ui.effective_bounds(marker).width(),
        f64::from(NAV_CONTENT_WIDTH),
        "nav marker width",
    );
}

#[test]
fn gallery_registers_demo_style_token_names() {
    let ui = build_ui();
    let vocabulary = ui.style_vocabulary();

    for class_name in [
        CLASS_DEMO_ROOT,
        CLASS_DEMO_CARD,
        CLASS_FOREGROUND_SWATCH,
        CLASS_DEMO_TEMPLATE_SLIDER_THUMB,
    ] {
        let class = vocabulary
            .class_id_by_name(class_name)
            .expect("demo style token set should register every owned class");
        assert_eq!(vocabulary.class_name(class), Some(class_name));
    }

    for resource_name in [
        RESOURCE_DEMO_ROOT_BG,
        RESOURCE_DEMO_CARD_BG,
        RESOURCE_DEMO_ACCENT_BG,
    ] {
        let resource = vocabulary
            .resource_key_by_name(resource_name)
            .expect("demo style token set should register every owned resource");
        assert_eq!(vocabulary.resource_name(resource), Some(resource_name));
    }

    let root_class = vocabulary
        .class_id_by_name(CLASS_DEMO_ROOT)
        .expect("demo root class should be registered");
    assert!(
        ui.get(ui.root())
            .expect("root should exist")
            .classes()
            .contains(&root_class),
        "element authoring should use the same registered class id",
    );
}

#[test]
fn gallery_devtools_snapshot_labels_demo_classes_by_name() {
    let ui = build_ui();
    let card =
        find_by_class(&ui, ui.root(), CLASS_DEMO_CARD).expect("gallery should contain a demo card");

    let snapshot = overstory_devtools::DevtoolsSnapshot::capture(&ui);
    let row = snapshot
        .element(card)
        .expect("demo card should be inspectable");

    assert!(
        row.classes.iter().any(
            |class| snapshot.inspection().style_vocabulary.class_name(*class)
                == Some(CLASS_DEMO_CARD)
        ),
        "devtools snapshots should carry demo class names, not just raw ids",
    );
}

#[test]
fn gallery_nav_uses_shell_rows_instead_of_a_list_box() {
    let mut ui = build_ui();
    let nav_rows = nav_rows(&ui).to_vec();

    assert_eq!(nav_rows.len(), DemoPage::ALL.len());
    for row in &nav_rows {
        assert_ne!(
            ui.get(*row).and_then(|element| element.type_tag()),
            Some(overstory::TYPE_LIST_BOX),
            "navigation should not render as the listbox control demo",
        );
    }

    let systems_row = nav_rows[DemoPage::Systems.index()];
    let point = nav_row_activation_point(&mut ui, systems_row);
    let down = ui.dispatch_pointer_event(&pointer_down(point, 1));
    assert!(down.changed(), "pointer down should focus the nav row");
    assert_eq!(
        ui.focused(),
        Some(systems_row),
        "pointer activation should move focus to the nav row",
    );
    assert!(
        !ui.get(systems_row)
            .expect("systems nav row should exist")
            .pseudos()
            .contains(&PSEUDO_FOCUS_VISIBLE),
        "pointer focus should not show keyboard-visible focus styling",
    );
    let up = ui.dispatch_pointer_event(&pointer_up(point, 2));
    assert!(up.handled, "pointer up should dispatch handled nav click");

    assert_selected_page_title(
        &mut ui,
        DemoPage::Systems,
        "nav row click should switch page content",
    );
}

#[test]
fn gallery_nav_supports_focus_and_keyboard_controls() {
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

    let mut ui = build_ui();
    let nav_rows = nav_rows(&ui).to_vec();
    for row in &nav_rows {
        assert!(ui.is_focusable(*row), "nav rows should be focusable");
    }

    assert!(ui.set_focus(Some(nav_rows[1])));
    assert!(
        ui.get(nav_rows[1])
            .expect("focused nav row should exist")
            .pseudos()
            .contains(&PSEUDO_FOCUS_VISIBLE),
        "keyboard focus should show focus-visible styling",
    );
    let enter = KeyboardEvent::key_down(Key::Named(NamedKey::Enter), Code::Enter);
    let result = ui.dispatch_key(&enter);
    assert!(result.handled, "Enter should activate the focused nav row");
    assert_selected_page_title(&mut ui, DemoPage::Inputs, "Enter should switch to Inputs");

    let down = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowDown), Code::ArrowDown);
    let result = ui.dispatch_key(&down);
    assert!(result.handled, "ArrowDown should move and activate nav");
    assert_eq!(ui.focused(), Some(nav_rows[DemoPage::Disclosure.index()]));
    assert_selected_page_title(
        &mut ui,
        DemoPage::Disclosure,
        "ArrowDown should switch to Disclosure",
    );

    let home = KeyboardEvent::key_down(Key::Named(NamedKey::Home), Code::Home);
    let result = ui.dispatch_key(&home);
    assert!(result.handled, "Home should move to the first nav row");
    assert_eq!(ui.focused(), Some(nav_rows[0]));
    assert_selected_page_title(
        &mut ui,
        DemoPage::Overview,
        "Home should switch to Overview",
    );
}

#[test]
fn gallery_theme_toggle_switches_control_theme() {
    let mut ui = build_ui();
    let toggle = mode_toggle(&ui);

    let result = ui.synthesize_click(toggle, Point::ZERO);
    assert!(result.handled, "theme toggle should retain toggle behavior");
    assert!(
        ui.get(toggle)
            .expect("theme toggle should exist")
            .pseudos()
            .contains(&overstory::pseudos::PSEUDO_CHECKED),
        "theme toggle should be checked in light mode",
    );
    assert_eq!(
        ui.theme()
            .get::<Brush>(theme::ON_SURFACE_FG)
            .expect("light theme should provide foreground"),
        &Brush::from(Color::from_rgba8(0x16, 0x1a, 0x24, 0xff)),
        "theme toggle should swap the active control palette",
    );
}

#[test]
fn gallery_devtools_button_opens_in_process_inspector() {
    let mut ui = build_ui();
    let button = devtools_button(&ui);

    let result = ui.synthesize_click(button, Point::ZERO);

    assert!(
        result.handled,
        "devtools button should open the docked inspector"
    );
    assert_eq!(
        ui.get(ui.root())
            .expect("root should exist")
            .children()
            .len(),
        3,
        "docked devtools should become a root-level pane"
    );
    assert!(
        ui.get(button)
            .expect("devtools button should still exist")
            .pseudos()
            .contains(&PSEUDO_SELECTED),
        "devtools button should show its open state",
    );
    assert!(
        count_type(&ui, ui.root(), overstory::TYPE_OUTLINE_VIEW) >= 1,
        "in-process devtools should render an inspector outline",
    );
    assert!(
        find_by_class(&ui, ui.root(), CLASS_DEVTOOLS_STATUS).is_some(),
        "in-process devtools should render auto-capture status",
    );
    assert!(
        find_by_class(&ui, ui.root(), CLASS_DEVTOOLS_EVENT_LOG).is_some(),
        "in-process devtools should render a live event log",
    );

    assert!(
        ui.synthesize_click(button, Point::ZERO).handled,
        "second devtools button click should close the dock",
    );
    assert_eq!(
        ui.get(ui.root())
            .expect("root should exist")
            .children()
            .len(),
        2,
        "closing devtools should remove the docked pane",
    );
    assert!(
        !ui.get(button)
            .expect("devtools button should still exist")
            .pseudos()
            .contains(&PSEUDO_SELECTED),
        "devtools button should clear its open state after close",
    );
}

#[test]
fn gallery_devtools_dock_refreshes_running_gallery_tree() {
    let mut ui = build_ui();
    let stage = stage_element(&ui);
    let button = devtools_button(&ui);
    assert!(ui.synthesize_click(button, Point::ZERO).handled);
    ui.compute_layout(Size::new(
        f64::from(INITIAL_SIZE.0),
        f64::from(INITIAL_SIZE.1),
    ));

    let status = find_by_class(&ui, ui.root(), CLASS_DEVTOOLS_STATUS)
        .expect("devtools dock should contain status text");
    let event_log = find_by_class(&ui, ui.root(), CLASS_DEVTOOLS_EVENT_LOG)
        .expect("devtools dock should contain an event log");
    let outline = find_by_type(&ui, ui.root(), overstory::TYPE_OUTLINE_VIEW)
        .expect("devtools dock should contain an outline");
    let initial_status = text_content(&ui, status);
    let initial_count =
        status_element_count(&initial_status).expect("initial status should report elements");
    assert!(
        subtree_contains_text_fragment(&ui, event_log, "Attached to the running gallery"),
        "opening devtools should log the live attachment"
    );

    show_demo_page(&mut ui, stage, DemoPage::Svg);
    ui.pump_timers(millis(DEVTOOLS_REFRESH_INTERVAL_MS));
    ui.compute_layout(Size::new(
        f64::from(INITIAL_SIZE.0),
        f64::from(INITIAL_SIZE.1),
    ));

    let refreshed_status = text_content(&ui, status);
    let refreshed_count =
        status_element_count(&refreshed_status).expect("refreshed status should report elements");
    assert_ne!(
        refreshed_count, initial_count,
        "devtools should stream the changed running gallery tree",
    );
    assert!(
        refreshed_status.contains("refreshes: 1"),
        "timer refresh should update the auto-capture status: {refreshed_status}",
    );
    assert!(
        refreshed_status.contains("Frame - demand")
            && refreshed_status.contains("layout")
            && refreshed_status.contains("surface refresh")
            && refreshed_status.contains("backend"),
        "timer refresh should show recent frame diagnostics: {refreshed_status}",
    );
    assert!(
        refreshed_status.contains("Element realization - visited")
            && refreshed_status.contains("changes")
            && refreshed_status.contains("painted")
            && refreshed_status.contains("surface updates"),
        "timer refresh should show element-realization diagnostics: {refreshed_status}",
    );
    assert!(
        ui.get(outline).is_some(),
        "timer refresh should preserve the rendered outline element",
    );
    assert!(
        subtree_contains_text_fragment(&ui, event_log, "refresh 1:")
            && subtree_contains_text_fragment(&ui, event_log, "stream item(s)")
            && subtree_contains_text_fragment(&ui, event_log, "tree patch +"),
        "timer refresh should append a concrete event-log row"
    );

    ui.pump_timers(millis(DEVTOOLS_REFRESH_INTERVAL_MS));
    ui.compute_layout(Size::new(
        f64::from(INITIAL_SIZE.0),
        f64::from(INITIAL_SIZE.1),
    ));
    assert!(
        !subtree_contains_text_fragment(&ui, event_log, "refresh 2:"),
        "frame-only or empty refreshes should not append event-log rows"
    );
}

#[test]
fn gallery_pages_stay_inside_initial_window_width() {
    for page in DemoPage::ALL {
        let mut ui = build_ui();
        let stage = stage_element(&ui);
        show_demo_page(&mut ui, stage, page);
        ui.compute_layout(Size::new(
            f64::from(INITIAL_SIZE.0),
            f64::from(INITIAL_SIZE.1),
        ));

        assert_subtree_inside_width(&ui, ui.root(), f64::from(INITIAL_SIZE.0));
    }
}

#[test]
fn gallery_pages_stay_inside_narrow_window_width() {
    const NARROW_WIDTH: f64 = 1180.0;

    for page in DemoPage::ALL {
        let mut ui = build_ui();
        let stage = stage_element(&ui);
        show_demo_page(&mut ui, stage, page);
        ui.compute_layout(Size::new(NARROW_WIDTH, f64::from(INITIAL_SIZE.1)));

        assert_subtree_inside_width(&ui, ui.root(), NARROW_WIDTH);
    }
}

#[test]
fn gallery_pages_keep_primary_cards_on_one_line_at_narrow_width() {
    const NARROW_WIDTH: f64 = 1180.0;

    for page in DemoPage::ALL {
        if page == DemoPage::Svg {
            continue;
        }
        let mut ui = build_ui();
        let stage = stage_element(&ui);
        show_demo_page(&mut ui, stage, page);
        ui.compute_layout(Size::new(NARROW_WIDTH, f64::from(INITIAL_SIZE.1)));

        let row = page_row(&ui, stage);
        let row_children = ui
            .get(row)
            .expect("page row should exist")
            .children()
            .to_vec();
        let Some(first) = row_children.first().copied() else {
            continue;
        };
        let expected_y = ui.effective_bounds(first).y0;
        for child in row_children {
            assert_close(
                ui.effective_bounds(child).y0,
                expected_y,
                "page card y position",
            );
        }
    }
}

#[test]
fn inputs_page_groups_input_and_choice_cards_without_card_spill() {
    let mut ui = build_ui();
    let stage = stage_element(&ui);
    show_demo_page(&mut ui, stage, DemoPage::Inputs);
    ui.compute_layout(Size::new(
        f64::from(INITIAL_SIZE.0),
        f64::from(INITIAL_SIZE.1),
    ));

    let stage_children = ui
        .get(stage)
        .expect("stage should exist")
        .children()
        .to_vec();
    assert_eq!(
        stage_children.len(),
        3,
        "Inputs should have a header plus two rows"
    );

    let primary_cards = ui
        .get(stage_children[1])
        .expect("primary input row should exist")
        .children()
        .to_vec();
    let secondary_cards = ui
        .get(stage_children[2])
        .expect("secondary input row should exist")
        .children()
        .to_vec();
    assert_eq!(
        primary_cards.len(),
        2,
        "text/actions should be separate cards"
    );
    assert_eq!(
        secondary_cards.len(),
        2,
        "radio/segmented should be separate cards"
    );

    for card in primary_cards.into_iter().chain(secondary_cards) {
        assert_card_contains_subtree(&ui, card);
    }
}

#[test]
fn inputs_page_custom_button_template_frames_retained_content() {
    let mut ui = build_ui();
    let stage = stage_element(&ui);
    show_demo_page(&mut ui, stage, DemoPage::Inputs);
    ui.compute_layout(Size::new(
        f64::from(INITIAL_SIZE.0),
        f64::from(INITIAL_SIZE.1),
    ));

    let outer = find_by_class(&ui, stage, CLASS_DEMO_TEMPLATE_BUTTON_OUTER)
        .expect("inputs page should render the custom template outer frame");
    let inner = find_by_class(&ui, stage, CLASS_DEMO_TEMPLATE_BUTTON_INNER)
        .expect("inputs page should render the custom template inner frame");
    let content = ui
        .get(inner)
        .expect("inner frame should exist")
        .children()
        .first()
        .copied()
        .expect("inner frame should host the content presenter");
    let retained_label = ui
        .get(content)
        .expect("content presenter should exist")
        .children()
        .first()
        .copied()
        .expect("content presenter should host retained label content");
    let retained_label_text = *ui
        .get(retained_label)
        .expect("retained label row should exist")
        .children()
        .get(1)
        .expect("retained label row should contain text after the status dot");

    let outer_bounds = ui.effective_bounds(outer);
    let inner_bounds = ui.effective_bounds(inner);
    let retained_bounds = ui.effective_bounds(retained_label);

    assert!(
        outer_bounds.width() > inner_bounds.width(),
        "outer frame should add visible padding around inner frame: {outer_bounds:?} <= {inner_bounds:?}",
    );
    assert!(
        inner_bounds.width() > retained_bounds.width(),
        "inner frame should add visible padding around retained content: {inner_bounds:?} <= {retained_bounds:?}",
    );
    assert!(
        inner_bounds.x0 <= retained_bounds.x0
            && inner_bounds.y0 <= retained_bounds.y0
            && inner_bounds.x1 >= retained_bounds.x1
            && inner_bounds.y1 >= retained_bounds.y1,
        "inner frame should contain retained content: {inner_bounds:?} vs {retained_bounds:?}",
    );

    assert_eq!(text_content(&ui, retained_label_text), "Deploy");
    let text_center = ui.effective_bounds(retained_label_text).center();
    assert_eq!(
        ui.hit_test(text_center),
        Some(retained_label_text),
        "retained label text should be the hit-tested target at its center",
    );
    let down = ui.dispatch_pointer_event(&pointer_down(text_center, 1));
    assert!(
        down.changed(),
        "pointer down on retained text should press the owning button",
    );
    assert!(
        !down.handled,
        "pointer down on retained text should leave click recognition active",
    );
    let up = ui.dispatch_pointer_event(&pointer_up(text_center, 2));
    assert!(
        up.changed(),
        "pointer up on retained text should release the owning button press",
    );
    ui.flush_bindings();
    assert_eq!(text_content(&ui, retained_label_text), "Clicked!");
}

#[test]
fn inputs_page_custom_checkbox_template_keeps_checked_behavior() {
    let mut ui = build_ui();
    let stage = stage_element(&ui);
    show_demo_page(&mut ui, stage, DemoPage::Inputs);
    ui.compute_layout(Size::new(
        f64::from(INITIAL_SIZE.0),
        f64::from(INITIAL_SIZE.1),
    ));

    let check_box_box = find_by_class(&ui, stage, CLASS_DEMO_TEMPLATE_CHECK_BOX_BOX)
        .expect("inputs page should render the custom checkbox box part");
    let checkbox = ui
        .get(check_box_box)
        .and_then(overstory::Element::templated_parent)
        .expect("custom checkbox box should be generated by the checkbox template");
    assert_eq!(ui.slot(checkbox, SLOT_CHECK_BOX_BOX), Some(check_box_box));
    assert_eq!(
        ui.get(checkbox)
            .and_then(|element| element.get_local_value(ui.properties().is_checked))
            .copied(),
        Some(true),
        "demo checkbox should start checked",
    );

    ui.synthesize_click(checkbox, Point::ZERO);
    assert_eq!(
        ui.get(checkbox)
            .and_then(|element| element.get_local_value(ui.properties().is_checked))
            .copied(),
        Some(false),
        "custom checkbox template should preserve checkbox toggling",
    );
}

#[test]
fn systems_page_custom_slider_template_keeps_value_behavior() {
    let mut ui = build_ui();
    let stage = stage_element(&ui);
    show_demo_page(&mut ui, stage, DemoPage::Systems);
    ui.compute_layout(Size::new(
        f64::from(INITIAL_SIZE.0),
        f64::from(INITIAL_SIZE.1),
    ));

    assert_eq!(
        count_type(&ui, stage, overstory::TYPE_SLIDER),
        2,
        "systems page should show default and custom slider templates together",
    );
    let track = find_by_class(&ui, stage, CLASS_DEMO_TEMPLATE_SLIDER_TRACK)
        .expect("systems page should render the custom slider track");
    let thumb = find_by_class(&ui, stage, CLASS_DEMO_TEMPLATE_SLIDER_THUMB)
        .expect("systems page should render the custom slider thumb");
    let slider = ui
        .get(thumb)
        .and_then(overstory::Element::templated_parent)
        .expect("custom slider thumb should be generated by the slider template");

    assert_eq!(ui.slot(slider, SLOT_SLIDER_TRACK), Some(track));
    assert_eq!(ui.slot(slider, SLOT_SLIDER_THUMB), Some(thumb));
    overstory::set_slider_value(&mut ui, slider, 0.9);
    assert_eq!(
        ui.get(slider)
            .and_then(|element| element.get_local_value(ui.properties().value))
            .copied(),
        Some(0.9),
        "custom slider template should preserve value synchronization",
    );
}

#[test]
fn disclosure_page_keeps_expander_and_accordion_cards_together() {
    let mut ui = build_ui();
    let stage = stage_element(&ui);
    show_demo_page(&mut ui, stage, DemoPage::Disclosure);
    ui.compute_layout(Size::new(
        f64::from(INITIAL_SIZE.0),
        f64::from(INITIAL_SIZE.1),
    ));

    let stage_children = ui
        .get(stage)
        .expect("stage should exist")
        .children()
        .to_vec();
    assert_eq!(
        stage_children.len(),
        2,
        "Disclosure should have a header plus one row"
    );

    let cards = ui
        .get(stage_children[1])
        .expect("disclosure row should exist")
        .children()
        .to_vec();
    assert_eq!(
        cards.len(),
        2,
        "expander/accordion should be separate cards"
    );

    for card in cards {
        assert_card_contains_subtree(&ui, card);
    }
}

fn assert_subtree_inside_width(ui: &Ui, element: ElementId, max_x: f64) {
    let bounds = ui.effective_bounds(element);
    assert!(
        bounds.x1 <= max_x + 0.01,
        "{element:?} overflows window width: {bounds:?} > {max_x}",
    );
    let children = ui
        .get(element)
        .expect("element should exist")
        .children()
        .to_vec();
    for child in children {
        assert_subtree_inside_width(ui, child, max_x);
    }
}

fn assert_card_contains_subtree(ui: &Ui, card: ElementId) {
    let card_bounds = ui.effective_bounds(card);
    let children = ui.get(card).expect("card should exist").children().to_vec();
    for child in children {
        assert_subtree_inside_bottom(ui, child, card_bounds.y1);
    }
}

fn assert_subtree_inside_bottom(ui: &Ui, element: ElementId, max_y: f64) {
    let bounds = ui.effective_bounds(element);
    assert!(
        bounds.y1 <= max_y + 0.01,
        "{element:?} overflows card height: {bounds:?} > {max_y}",
    );
    let children = ui
        .get(element)
        .expect("element should exist")
        .children()
        .to_vec();
    for child in children {
        assert_subtree_inside_bottom(ui, child, max_y);
    }
}

fn stage_element(ui: &Ui) -> ElementId {
    let root_children = ui
        .get(ui.root())
        .expect("root should exist")
        .children()
        .to_vec();
    let [_, stage] = *root_children else {
        panic!("gallery root should contain nav and stage");
    };
    stage
}

fn nav_element(ui: &Ui) -> ElementId {
    let root_children = ui
        .get(ui.root())
        .expect("root should exist")
        .children()
        .to_vec();
    let [nav, _] = *root_children else {
        panic!("gallery root should contain nav and stage");
    };
    nav
}

fn nav_rows(ui: &Ui) -> &[ElementId] {
    let nav = nav_element(ui);
    let nav_children = ui.get(nav).expect("nav should exist").children();
    let nav_stack = nav_children
        .get(5)
        .copied()
        .expect("nav should contain a compact row stack");
    ui.get(nav_stack)
        .expect("nav stack should exist")
        .children()
}

fn devtools_button(ui: &Ui) -> ElementId {
    let nav = nav_element(ui);
    let nav_children = ui.get(nav).expect("nav should exist").children();
    nav_children
        .get(4)
        .copied()
        .expect("nav should contain a devtools button")
}

fn mode_toggle(ui: &Ui) -> ElementId {
    let nav = nav_element(ui);
    let nav_children = ui.get(nav).expect("nav should exist").children();
    let mode_row = nav_children
        .get(3)
        .copied()
        .expect("nav should contain a theme mode row");
    ui.get(mode_row)
        .expect("theme mode row should exist")
        .children()
        .first()
        .copied()
        .expect("theme mode row should contain a toggle")
}

fn assert_selected_page_title(ui: &mut Ui, page: DemoPage, label: &str) {
    let stage = stage_element(ui);
    ui.compute_layout(Size::new(
        f64::from(INITIAL_SIZE.0),
        f64::from(INITIAL_SIZE.1),
    ));
    let title = find_by_class(ui, stage, CLASS_DEMO_PAGE_HEADING)
        .expect("page header should contain a title");
    assert_eq!(text_content(ui, title), page.title(), "{label}");
}

fn nav_row_activation_point(ui: &mut Ui, row: ElementId) -> Point {
    ui.compute_layout(Size::new(
        f64::from(INITIAL_SIZE.0),
        f64::from(INITIAL_SIZE.1),
    ));
    let bounds = ui.effective_bounds(row);
    Point::new(bounds.x1 - 8.0, bounds.center().y)
}

fn count_type(ui: &Ui, element: ElementId, type_tag: understory_style::TypeTag) -> usize {
    let Some(current) = ui.get(element) else {
        return 0;
    };
    let current_count = usize::from(current.type_tag() == Some(type_tag));
    current_count
        + current
            .children()
            .iter()
            .map(|child| count_type(ui, *child, type_tag))
            .sum::<usize>()
}

fn find_by_type(
    ui: &Ui,
    element: ElementId,
    type_tag: understory_style::TypeTag,
) -> Option<ElementId> {
    let current = ui.get(element)?;
    if current.type_tag() == Some(type_tag) {
        return Some(element);
    }
    current
        .children()
        .iter()
        .find_map(|child| find_by_type(ui, *child, type_tag))
}

fn find_by_class(
    ui: &Ui,
    element: ElementId,
    class: impl Into<overstory::StyleClass>,
) -> Option<ElementId> {
    let class = class.into().registered_id(ui.style_vocabulary())?;
    find_by_class_id(ui, element, class)
}

fn find_by_class_id(ui: &Ui, element: ElementId, class: ClassId) -> Option<ElementId> {
    let current = ui.get(element)?;
    if current.classes().contains(&class) {
        return Some(element);
    }
    current
        .children()
        .iter()
        .find_map(|child| find_by_class_id(ui, *child, class))
}

fn text_content(ui: &Ui, element: ElementId) -> String {
    ui.get(element)
        .and_then(|element| element.get_local_value(ui.properties().text))
        .cloned()
        .expect("element should have text content")
}

fn subtree_contains_text_fragment(ui: &Ui, element: ElementId, fragment: &str) -> bool {
    ui.get(element).is_some_and(|element_ref| {
        element_ref
            .get_local_value(ui.properties().text)
            .is_some_and(|text| text.contains(fragment))
            || element_ref
                .children()
                .iter()
                .any(|child| subtree_contains_text_fragment(ui, *child, fragment))
    })
}

fn status_element_count(status: &str) -> Option<usize> {
    status
        .split("elements: ")
        .nth(1)?
        .split(',')
        .next()?
        .parse()
        .ok()
}

fn pointer_down(point: Point, time: u64) -> ui_events::pointer::PointerEvent {
    use ui_events::pointer::{PointerButton, PointerButtonEvent, PointerEvent};

    PointerEvent::Down(PointerButtonEvent {
        pointer: pointer_info(),
        button: Some(PointerButton::Primary),
        state: pointer_state(point, time, true),
    })
}

fn pointer_up(point: Point, time: u64) -> ui_events::pointer::PointerEvent {
    use ui_events::pointer::{PointerButton, PointerButtonEvent, PointerEvent};

    PointerEvent::Up(PointerButtonEvent {
        pointer: pointer_info(),
        button: Some(PointerButton::Primary),
        state: pointer_state(point, time, false),
    })
}

fn pointer_info() -> ui_events::pointer::PointerInfo {
    use ui_events::pointer::{PointerId, PointerInfo, PointerType};

    PointerInfo {
        pointer_id: Some(PointerId::PRIMARY),
        persistent_device_id: None,
        pointer_type: PointerType::Mouse,
    }
}

fn pointer_state(point: Point, time: u64, primary_down: bool) -> ui_events::pointer::PointerState {
    use ui_events::pointer::{PointerButton, PointerButtons, PointerState};

    let mut buttons = PointerButtons::new();
    if primary_down {
        buttons.insert(PointerButton::Primary);
    }
    let mut state = PointerState {
        time,
        buttons,
        count: 1,
        pressure: if primary_down { 0.5 } else { 0.0 },
        ..PointerState::default()
    };
    state.position.x = point.x;
    state.position.y = point.y;
    state
}

fn page_row(ui: &Ui, stage: ElementId) -> ElementId {
    ui.get(stage)
        .expect("stage should exist")
        .children()
        .get(1)
        .copied()
        .expect("stage should contain header and page row")
}
