// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use overstory_devtools::{
    DevtoolsSessionHandle, DevtoolsViewElements, LiveDevtools, UiInspectionTarget,
    append_session_view, refresh_session_view_in_place,
};

use super::*;

pub(super) fn wire_devtools_button(ui: &mut Ui, button: ElementId) {
    let open_popup = Rc::new(Cell::new(None));
    let open_popup_for_click = Rc::clone(&open_popup);
    ui.on_click(button, move |ui, event| {
        if !matches!(
            event.phase,
            overstory::Phase::Target | overstory::Phase::Bubble
        ) {
            return;
        }
        if let Some(popup) = open_popup_for_click.get() {
            if ui.close_popup(popup) {
                open_popup_for_click.set(None);
                ui.remove_pseudo(button, PSEUDO_SELECTED);
                event.mark_handled();
                return;
            }
            open_popup_for_click.set(None);
            ui.remove_pseudo(button, PSEUDO_SELECTED);
        }

        if let Some(popup) =
            open_in_process_devtools(ui, event.current, Rc::clone(&open_popup_for_click))
        {
            open_popup_for_click.set(Some(popup));
        }
        event.mark_handled();
    });
}

#[derive(Clone)]
struct InProcessDevtoolsShell {
    devtools: Rc<RefCell<LiveDevtools>>,
    session: DevtoolsSessionHandle,
    button: ElementId,
    popup: Rc<Cell<Option<ElementId>>>,
    view_elements: Rc<Cell<Option<DevtoolsViewElements>>>,
    status: Rc<Cell<Option<ElementId>>>,
    refresh_count: Rc<Cell<u32>>,
    open_popup: Rc<Cell<Option<ElementId>>>,
}

impl InProcessDevtoolsShell {
    fn new(
        devtools: Rc<RefCell<LiveDevtools>>,
        session: DevtoolsSessionHandle,
        button: ElementId,
        open_popup: Rc<Cell<Option<ElementId>>>,
    ) -> Self {
        Self {
            devtools,
            session,
            button,
            popup: Rc::new(Cell::new(None)),
            view_elements: Rc::new(Cell::new(None)),
            status: Rc::new(Cell::new(None)),
            refresh_count: Rc::new(Cell::new(0)),
            open_popup,
        }
    }

    fn set_popup(&self, popup: ElementId) {
        self.popup.set(Some(popup));
    }

    fn set_view_elements(&self, elements: DevtoolsViewElements) {
        self.view_elements.set(Some(elements));
    }

    fn set_status(&self, status: ElementId) {
        self.status.set(Some(status));
    }

    fn update_status(&self, ui: &mut Ui, stream_item_count: usize) {
        let Some(status) = self.status.get() else {
            return;
        };
        let devtools = self.devtools.borrow();
        let content =
            format_auto_capture_status(&devtools, self.refresh_count.get(), stream_item_count);
        ui.set_local(status, ui.properties().content, content);
    }

    fn refresh(&self, ui: &mut Ui) -> bool {
        let Some(popup) = self.popup.get() else {
            return false;
        };
        let Some(elements) = self.view_elements.get() else {
            return false;
        };
        if ui.get(popup).is_none() {
            self.open_popup.set(None);
            ui.remove_pseudo(self.button, PSEUDO_SELECTED);
            return false;
        }

        let previous_tree = self.devtools.borrow().mirror().tree().clone();
        let refresh = {
            let excluded = [popup];
            let target = UiInspectionTarget::new(ui).excluding(&excluded);
            self.devtools.borrow_mut().refresh_from(&target)
        };
        let Ok(refresh) = refresh else {
            if let Some(status) = self.status.get() {
                ui.set_local(
                    status,
                    ui.properties().content,
                    String::from("Live capture failed"),
                );
            }
            return false;
        };

        let current_tree = self.devtools.borrow().mirror().tree().clone();
        let refreshed = refresh_session_view_in_place(
            ui,
            elements,
            &previous_tree,
            &current_tree,
            self.session.clone(),
        );
        self.view_elements.set(Some(refreshed));
        self.refresh_count
            .set(self.refresh_count.get().saturating_add(1));
        self.update_status(ui, refresh.stream_item_count);
        true
    }

    fn close(&self, ui: &mut Ui) {
        let Some(popup) = self.popup.get() else {
            return;
        };
        if ui.close_popup(popup) {
            self.popup.set(None);
            self.open_popup.set(None);
            ui.remove_pseudo(self.button, PSEUDO_SELECTED);
        }
    }
}

fn open_in_process_devtools(
    ui: &mut Ui,
    anchor: ElementId,
    open_popup: Rc<Cell<Option<ElementId>>>,
) -> Option<ElementId> {
    let selected = ui.focused().or(Some(ui.root()));
    let target = UiInspectionTarget::new(ui);
    let devtools = Rc::new(RefCell::new(LiveDevtools::capture(&target, selected).ok()?));
    let session = devtools.borrow().session().clone();
    let shell = InProcessDevtoolsShell::new(devtools, session, anchor, open_popup);
    ui.add_pseudo(anchor, PSEUDO_SELECTED);

    let shell_for_build = shell.clone();
    let popup = ui.open_popup(
        anchor,
        Popup::new()
            .padding(0.0, 0.0)
            .offset(12.0, 0.0)
            .dismiss_on_outside_pointer(false),
        move |ui, popup| {
            shell_for_build.set_popup(popup);
            ui.with_children(popup, |popup| {
                let popup_id = popup.parent();
                let shell_for_panel = shell_for_build.clone();
                let panel = popup.add_with_children(
                    Panel::column(12.0)
                        .class(CLASS_DEMO_CARD)
                        .class(CLASS_DEVTOOLS_INSPECTOR_CARD)
                        .layout(
                            Layout::column(12.0)
                                .logical_size(DEVTOOLS_INSPECTOR_WIDTH, DEVTOOLS_CARD_HEIGHT)
                                .padding_all(14.0),
                        ),
                    |panel| {
                        panel.with_child(Panel::row(10.0).align_center(), |header| {
                            header.add(card_title("IN-PROCESS DEVTOOLS"));
                            let refresh = header.add(
                                Button::new("Refresh now")
                                    .font_size(13.0)
                                    .layout(Layout::new().logical_size(96.0, 30.0)),
                            );
                            wire_refresh_button(header.ui_mut(), refresh, shell_for_panel.clone());
                            let close = header.add(
                                Button::new("Close")
                                    .font_size(13.0)
                                    .layout(Layout::new().logical_size(58.0, 30.0)),
                            );
                            wire_close_button(header.ui_mut(), close, shell_for_panel);
                        });
                    },
                );
                let status = popup.ui_mut().append(
                    panel,
                    Text::new("")
                        .class(CLASS_DEMO_CAPTION)
                        .class(CLASS_DEVTOOLS_STATUS)
                        .wrap()
                        .layout(Layout::new().logical_size(492.0, 20.0)),
                );
                shell_for_build.set_status(status);
                shell_for_build.update_status(popup.ui_mut(), 0);
                let elements =
                    append_session_view(popup.ui_mut(), panel, shell_for_build.session.clone());
                shell_for_build.set_view_elements(elements);
                let _ = popup
                    .ui_mut()
                    .schedule_timer_repeating(popup_id, millis(DEVTOOLS_REFRESH_INTERVAL_MS));
                wire_auto_capture_timer(popup.ui_mut(), popup_id, shell_for_build.clone());
            });
        },
    );
    if popup.is_none() {
        ui.remove_pseudo(anchor, PSEUDO_SELECTED);
    }
    popup
}

fn wire_refresh_button(ui: &mut Ui, button: ElementId, shell: InProcessDevtoolsShell) {
    ui.on_click(button, move |ui, event| {
        if !matches!(
            event.phase,
            overstory::Phase::Target | overstory::Phase::Bubble
        ) {
            return;
        }
        let _ = shell.refresh(ui);
        event.mark_handled();
    });
}

fn wire_close_button(ui: &mut Ui, button: ElementId, shell: InProcessDevtoolsShell) {
    ui.on_click(button, move |ui, event| {
        if !matches!(
            event.phase,
            overstory::Phase::Target | overstory::Phase::Bubble
        ) {
            return;
        }
        shell.close(ui);
        event.mark_handled();
    });
}

fn wire_auto_capture_timer(ui: &mut Ui, popup: ElementId, shell: InProcessDevtoolsShell) {
    ui.on_timer(popup, move |ui, _event| {
        let _ = shell.refresh(ui);
    });
}

fn format_auto_capture_status(
    devtools: &LiveDevtools,
    refresh_count: u32,
    stream_item_count: usize,
) -> String {
    format!(
        "Auto capture - elements: {}, refreshes: {}, stream items: {}",
        devtools.mirror().tree().nodes.len(),
        refresh_count,
        stream_item_count,
    )
}
