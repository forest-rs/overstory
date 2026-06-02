// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use overstory_devtools::{
    DevtoolsSessionHandle, DevtoolsViewElements, LiveDevtools, UiInspectionTarget,
    append_session_view, refresh_session_view,
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
                event.mark_handled();
                return;
            }
            open_popup_for_click.set(None);
        }

        if let Some(popup) = open_in_process_devtools(ui, event.current) {
            open_popup_for_click.set(Some(popup));
        }
        event.mark_handled();
    });
}

fn open_in_process_devtools(ui: &mut Ui, anchor: ElementId) -> Option<ElementId> {
    let selected = ui.focused().or(Some(ui.root()));
    let target = UiInspectionTarget::new(ui);
    let devtools = Rc::new(RefCell::new(LiveDevtools::capture(&target, selected).ok()?));
    let session = devtools.borrow().session().clone();
    let popup_id = Rc::new(Cell::new(None));
    let view_elements = Rc::new(Cell::new(None::<DevtoolsViewElements>));

    let popup_for_build = Rc::clone(&popup_id);
    let elements_for_build = Rc::clone(&view_elements);
    let devtools_for_build = Rc::clone(&devtools);
    let session_for_build = session.clone();
    ui.open_popup(
        anchor,
        Popup::new()
            .padding(0.0, 0.0)
            .offset(12.0, 0.0)
            .dismiss_on_outside_pointer(false),
        move |ui, popup| {
            popup_for_build.set(Some(popup));
            ui.with_children(popup, |popup| {
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
                            let refresh = header.add(Button::new("Refresh").font_size(13.0));
                            wire_refresh_button(
                                header.ui_mut(),
                                refresh,
                                Rc::clone(&devtools_for_build),
                                session_for_build.clone(),
                                Rc::clone(&popup_for_build),
                                Rc::clone(&elements_for_build),
                            );
                        });
                    },
                );
                let elements = append_session_view(popup.ui_mut(), panel, session_for_build);
                elements_for_build.set(Some(elements));
            });
        },
    )
}

fn wire_refresh_button(
    ui: &mut Ui,
    button: ElementId,
    devtools: Rc<RefCell<LiveDevtools>>,
    session: DevtoolsSessionHandle,
    popup_id: Rc<Cell<Option<ElementId>>>,
    view_elements: Rc<Cell<Option<DevtoolsViewElements>>>,
) {
    ui.on_click(button, move |ui, event| {
        if !matches!(
            event.phase,
            overstory::Phase::Target | overstory::Phase::Bubble
        ) {
            return;
        }
        let Some(popup) = popup_id.get() else {
            return;
        };
        let Some(elements) = view_elements.get() else {
            return;
        };
        let excluded = [popup];
        let target = UiInspectionTarget::new(ui).excluding(&excluded);
        if devtools.borrow_mut().refresh_from(&target).is_ok() {
            let refreshed = refresh_session_view(ui, elements, session.clone());
            view_elements.set(Some(refreshed));
        }
        event.mark_handled();
    });
}
