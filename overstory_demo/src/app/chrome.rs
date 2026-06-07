// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct NavChrome {
    mode_toggle: ElementId,
    devtools_button: ElementId,
}

impl NavChrome {
    pub(super) const fn devtools_button(self) -> ElementId {
        self.devtools_button
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NavKeyAction {
    Activate(DemoPage),
    MoveTo(DemoPage),
}
pub(super) fn build_nav(nav: &mut ChildUi<'_>) -> NavChrome {
    nav.add(heading("Overstory"));
    nav.add(caption("Focused widget gallery"));

    nav.add(
        Panel::new()
            .class(CLASS_DEMO_NAV_MARKER)
            .layout(Layout::new().logical_size(NAV_CONTENT_WIDTH, 4.0)),
    );

    let mode_toggle = nav.with_child(
        Panel::row(12.0).layout(
            Layout::row(12.0)
                .logical_size(NAV_CONTENT_WIDTH, 36.0)
                .align_center(),
        ),
        |mode_row| {
            let mode_toggle = mode_row.add(ToggleSwitch::new());
            mode_row.add(control_label("Light mode"));
            mode_toggle
        },
    );

    let devtools_button = nav.add(
        Button::new("Inspect app")
            .class(CLASS_DEVTOOLS_BUTTON)
            .font_size(13.0)
            .layout(Layout::new().logical_size(NAV_CONTENT_WIDTH, 34.0)),
    );

    NavChrome {
        mode_toggle,
        devtools_button,
    }
}

pub(super) fn build_nav_rows(nav: &mut ChildUi<'_>) -> Rc<[ElementId]> {
    let row_count = DemoPage::ALL.len() as f32;
    let stack_height = (row_count * 40.0) + ((row_count - 1.0) * 4.0);
    let mut rows = Vec::with_capacity(DemoPage::ALL.len());
    nav.with_child(
        Panel::column(4.0)
            .layout(Layout::column(4.0).logical_size(NAV_CONTENT_WIDTH, stack_height)),
        |stack| {
            for page in DemoPage::ALL {
                rows.push(append_nav_row(stack, page, page == DemoPage::Overview));
            }
        },
    );
    Rc::from(rows.into_boxed_slice())
}

pub(super) fn append_nav_row(stack: &mut ChildUi<'_>, page: DemoPage, selected: bool) -> ElementId {
    let row = stack.add_with_children(
        Panel::row(0.0)
            .class(CLASS_DEMO_NAV_ROW)
            .padding_xy(14.0, 0.0)
            .layout(
                Layout::row(0.0)
                    .logical_size(NAV_CONTENT_WIDTH, 40.0)
                    .align_center(),
            ),
        |row| {
            row.add(Text::new(page.nav_label()).class(CLASS_DEMO_NAV_LABEL));
        },
    );
    style_nav_row(stack.ui_mut(), row, selected);
    row
}

pub(super) fn wire_nav_rows(
    ui: &mut Ui,
    nav_rows: &Rc<[ElementId]>,
    stage: ElementId,
    shell: Rc<DemoShellState>,
) {
    for (page, row) in DemoPage::ALL.iter().copied().zip(nav_rows.iter().copied()) {
        ui.set_focusable(row, true);

        let nav_rows_for_click = Rc::clone(nav_rows);
        let shell_for_click = Rc::clone(&shell);
        ui.on_click(row, move |ui, event| {
            if !matches!(
                event.phase,
                overstory::Phase::Target | overstory::Phase::Bubble
            ) {
                return;
            }
            let _ = ui.set_pointer_focus(Some(row));
            activate_nav_page(ui, &nav_rows_for_click, &shell_for_click, stage, page);
            event.mark_handled();
        });

        let nav_rows_for_key = Rc::clone(nav_rows);
        let shell_for_key = Rc::clone(&shell);
        ui.on_key(row, move |ui, event| {
            if !matches!(
                event.phase,
                overstory::Phase::Target | overstory::Phase::Bubble
            ) {
                return;
            }
            let Some(action) = nav_key_action(page, event.key) else {
                return;
            };
            match action {
                NavKeyAction::Activate(page) => {
                    activate_nav_page(ui, &nav_rows_for_key, &shell_for_key, stage, page);
                }
                NavKeyAction::MoveTo(page) => {
                    focus_nav_page(ui, &nav_rows_for_key, page);
                    activate_nav_page(ui, &nav_rows_for_key, &shell_for_key, stage, page);
                }
            }
            event.mark_handled();
        });
    }
}

pub(super) fn wire_theme_toggle(ui: &mut Ui, nav_chrome: NavChrome, shell: Rc<DemoShellState>) {
    let shell_for_click = Rc::clone(&shell);
    ui.on_click(nav_chrome.mode_toggle, move |ui, event| {
        if !matches!(
            event.phase,
            overstory::Phase::Target | overstory::Phase::Bubble
        ) {
            return;
        }
        switch_demo_theme(ui, &shell_for_click);
    });

    let shell_for_key = Rc::clone(&shell);
    ui.on_key(nav_chrome.mode_toggle, move |ui, event| {
        if !matches!(event.phase, overstory::Phase::Target) || !is_activation_key(event.key) {
            return;
        }
        switch_demo_theme(ui, &shell_for_key);
    });
}

pub(super) fn switch_demo_theme(ui: &mut Ui, shell: &DemoShellState) {
    ui.set_theme(shell.toggle_theme().theme());
}

pub(super) fn activate_nav_page(
    ui: &mut Ui,
    nav_rows: &[ElementId],
    shell: &DemoShellState,
    stage: ElementId,
    page: DemoPage,
) {
    shell.set_selected_page(page);
    style_nav_rows(ui, nav_rows, page);
    pages::show_demo_page(ui, stage, page);
}

pub(super) fn focus_nav_page(ui: &mut Ui, nav_rows: &[ElementId], page: DemoPage) {
    if let Some(row) = nav_rows.get(page.index()).copied() {
        let _ = ui.set_focus(Some(row));
    }
}

pub(super) fn nav_key_action(
    page: DemoPage,
    event: &ui_events::keyboard::KeyboardEvent,
) -> Option<NavKeyAction> {
    use ui_events::keyboard::{Key, KeyState, NamedKey};

    if event.state != KeyState::Down {
        return None;
    }
    match &event.key {
        Key::Named(NamedKey::ArrowDown | NamedKey::ArrowRight) => {
            Some(NavKeyAction::MoveTo(page_offset(page, 1)))
        }
        Key::Named(NamedKey::ArrowUp | NamedKey::ArrowLeft) => {
            Some(NavKeyAction::MoveTo(page_offset(page, -1)))
        }
        Key::Named(NamedKey::Home) => DemoPage::from_index(0).map(NavKeyAction::MoveTo),
        Key::Named(NamedKey::End) => {
            DemoPage::from_index(DemoPage::ALL.len() - 1).map(NavKeyAction::MoveTo)
        }
        Key::Named(NamedKey::Enter) => Some(NavKeyAction::Activate(page)),
        Key::Character(s) if s == " " => Some(NavKeyAction::Activate(page)),
        _ => None,
    }
}

pub(super) fn is_activation_key(event: &ui_events::keyboard::KeyboardEvent) -> bool {
    use ui_events::keyboard::{Key, KeyState, NamedKey};

    if event.state != KeyState::Down {
        return false;
    }
    match &event.key {
        Key::Named(NamedKey::Enter) => true,
        Key::Character(s) if s == " " => true,
        _ => false,
    }
}

pub(super) fn page_offset(page: DemoPage, delta: isize) -> DemoPage {
    let len = DemoPage::ALL.len() as isize;
    let index = (page.index() as isize + delta).rem_euclid(len) as usize;
    DemoPage::from_index(index).expect("wrapped page index should be valid")
}

pub(super) fn style_nav_rows(ui: &mut Ui, nav_rows: &[ElementId], selected_page: DemoPage) {
    for (page, row) in DemoPage::ALL.iter().copied().zip(nav_rows.iter().copied()) {
        style_nav_row(ui, row, page == selected_page);
    }
}

pub(super) fn style_nav_row(ui: &mut Ui, row: ElementId, selected: bool) {
    if selected {
        ui.add_pseudo(row, PSEUDO_SELECTED);
    } else {
        ui.remove_pseudo(row, PSEUDO_SELECTED);
    }
}
