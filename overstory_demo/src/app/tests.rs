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

    assert_first_page_card_height(&mut ui, 322.0, "nav row click should switch page content");
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
    assert_first_page_card_height(&mut ui, 224.0, "Enter should switch to Inputs");

    let down = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowDown), Code::ArrowDown);
    let result = ui.dispatch_key(&down);
    assert!(result.handled, "ArrowDown should move and activate nav");
    assert_eq!(ui.focused(), Some(nav_rows[DemoPage::Disclosure.index()]));
    assert_first_page_card_height(&mut ui, 326.0, "ArrowDown should switch to Disclosure");

    let home = KeyboardEvent::key_down(Key::Named(NamedKey::Home), Code::Home);
    let result = ui.dispatch_key(&home);
    assert!(result.handled, "Home should move to the first nav row");
    assert_eq!(ui.focused(), Some(nav_rows[0]));
    assert_first_page_card_height(&mut ui, 222.0, "Home should switch to Overview");
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

    let status = find_by_class(&ui, ui.root(), CLASS_DEVTOOLS_STATUS)
        .expect("devtools dock should contain status text");
    let outline = find_by_type(&ui, ui.root(), overstory::TYPE_OUTLINE_VIEW)
        .expect("devtools dock should contain an outline");
    let initial_status = text_content(&ui, status);
    let initial_count =
        status_element_count(&initial_status).expect("initial status should report elements");

    show_demo_page(&mut ui, stage, DemoPage::Svg);
    ui.pump_timers(millis(DEVTOOLS_REFRESH_INTERVAL_MS));

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
        refreshed_status.contains("Last frame: total")
            && refreshed_status.contains("layout")
            && refreshed_status.contains("paint"),
        "timer refresh should show recent frame diagnostics: {refreshed_status}",
    );
    assert!(
        ui.get(outline).is_some(),
        "timer refresh should preserve the rendered outline element",
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

fn assert_first_page_card_height(ui: &mut Ui, height: f64, label: &str) {
    let stage = stage_element(ui);
    ui.compute_layout(Size::new(
        f64::from(INITIAL_SIZE.0),
        f64::from(INITIAL_SIZE.1),
    ));
    let row = page_row(ui, stage);
    let first_card = ui
        .get(row)
        .expect("page row should exist")
        .children()
        .first()
        .copied()
        .expect("page should contain a first card");
    assert_close(ui.effective_bounds(first_card).height(), height, label);
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

fn find_by_class(ui: &Ui, element: ElementId, class: ClassId) -> Option<ElementId> {
    let current = ui.get(element)?;
    if current.classes().contains(&class) {
        return Some(element);
    }
    current
        .children()
        .iter()
        .find_map(|child| find_by_class(ui, *child, class))
}

fn text_content(ui: &Ui, element: ElementId) -> String {
    ui.get(element)
        .and_then(|element| element.get_local_value(ui.properties().content))
        .cloned()
        .expect("element should have text content")
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
