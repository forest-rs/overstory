// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use overstory_devtools::{
    DevtoolsSessionHandle, DevtoolsTreePatchCounts, DevtoolsViewElements, LiveDevtools,
    UiLiveInspectionTarget, append_session_view, refresh_session_view_from_mirror,
};

use super::*;

pub(super) fn wire_devtools_button(ui: &mut Ui, button: ElementId) {
    let open_dock = Rc::new(Cell::new(None));
    let open_dock_for_click = Rc::clone(&open_dock);
    ui.on_click(button, move |ui, event| {
        if !matches!(
            event.phase,
            overstory::Phase::Target | overstory::Phase::Bubble
        ) {
            return;
        }
        if let Some(dock) = open_dock_for_click.get() {
            ui.end_live_inspection_recording();
            if ui.get(dock).is_some() {
                ui.remove(dock);
            }
            open_dock_for_click.set(None);
            ui.remove_pseudo(button, PSEUDO_SELECTED);
            event.mark_handled();
            return;
        }

        if let Some(dock) =
            open_in_process_devtools(ui, event.current, Rc::clone(&open_dock_for_click))
        {
            open_dock_for_click.set(Some(dock));
        }
        event.mark_handled();
    });
}

#[derive(Clone)]
struct InProcessDevtoolsShell {
    devtools: Rc<RefCell<LiveDevtools>>,
    session: DevtoolsSessionHandle,
    button: ElementId,
    dock: Rc<Cell<Option<ElementId>>>,
    view_elements: Rc<Cell<Option<DevtoolsViewElements>>>,
    status: Rc<Cell<Option<ElementId>>>,
    refresh_count: Rc<Cell<u32>>,
    open_dock: Rc<Cell<Option<ElementId>>>,
}

impl InProcessDevtoolsShell {
    fn new(
        devtools: Rc<RefCell<LiveDevtools>>,
        session: DevtoolsSessionHandle,
        button: ElementId,
        open_dock: Rc<Cell<Option<ElementId>>>,
    ) -> Self {
        Self {
            devtools,
            session,
            button,
            dock: Rc::new(Cell::new(None)),
            view_elements: Rc::new(Cell::new(None)),
            status: Rc::new(Cell::new(None)),
            refresh_count: Rc::new(Cell::new(0)),
            open_dock,
        }
    }

    fn set_dock(&self, dock: ElementId) {
        self.dock.set(Some(dock));
    }

    fn set_view_elements(&self, elements: DevtoolsViewElements) {
        self.view_elements.set(Some(elements));
    }

    fn set_status(&self, status: ElementId) {
        self.status.set(Some(status));
    }

    fn update_status(
        &self,
        ui: &mut Ui,
        stream_item_count: usize,
        tree_patch_counts: Option<DevtoolsTreePatchCounts>,
    ) {
        let Some(status) = self.status.get() else {
            return;
        };
        let devtools = self.devtools.borrow();
        let content = format_live_stream_status(
            &devtools,
            self.refresh_count.get(),
            stream_item_count,
            tree_patch_counts,
        );
        ui.set_local(status, ui.properties().content, content);
    }

    fn refresh(&self, ui: &mut Ui) -> bool {
        let Some(dock) = self.dock.get() else {
            return false;
        };
        let Some(elements) = self.view_elements.get() else {
            return false;
        };
        if ui.get(dock).is_none() {
            self.open_dock.set(None);
            self.devtools
                .borrow_mut()
                .detach_from_live(&mut UiLiveInspectionTarget::new(ui));
            ui.remove_pseudo(self.button, PSEUDO_SELECTED);
            return false;
        }

        let previous_tree = self.devtools.borrow().mirror().tree().clone();
        let refresh = {
            let excluded = [dock];
            let mut target = UiLiveInspectionTarget::new(ui).excluding(&excluded);
            self.devtools.borrow_mut().refresh_from_live(&mut target)
        };
        let Ok(refresh) = refresh else {
            if let Some(status) = self.status.get() {
                ui.set_local(
                    status,
                    ui.properties().content,
                    String::from("Live refresh failed"),
                );
            }
            return false;
        };

        let refreshed = {
            let devtools = self.devtools.borrow();
            refresh_session_view_from_mirror(
                ui,
                elements,
                &previous_tree,
                devtools.mirror(),
                self.session.clone(),
            )
        };
        self.view_elements.set(Some(refreshed));
        self.refresh_count
            .set(self.refresh_count.get().saturating_add(1));
        self.update_status(ui, refresh.stream_item_count, refresh.tree_patch_counts());
        true
    }

    fn close(&self, ui: &mut Ui) {
        let Some(dock) = self.dock.get() else {
            return;
        };
        self.devtools
            .borrow_mut()
            .detach_from_live(&mut UiLiveInspectionTarget::new(ui));
        if ui.get(dock).is_some() {
            ui.remove(dock);
            self.dock.set(None);
            self.open_dock.set(None);
            ui.remove_pseudo(self.button, PSEUDO_SELECTED);
        }
    }
}

fn open_in_process_devtools(
    ui: &mut Ui,
    anchor: ElementId,
    open_dock: Rc<Cell<Option<ElementId>>>,
) -> Option<ElementId> {
    let selected = ui.focused().or(Some(ui.root()));
    let mut target = UiLiveInspectionTarget::new(ui);
    let devtools = Rc::new(RefCell::new(
        LiveDevtools::capture_from_live(&mut target, selected).ok()?,
    ));
    let session = devtools.borrow().session().clone();
    let shell = InProcessDevtoolsShell::new(devtools, session, anchor, open_dock);
    ui.add_pseudo(anchor, PSEUDO_SELECTED);

    let dock = ui.append(
        ui.root(),
        Panel::column(12.0)
            .class(CLASS_DEMO_CARD)
            .class(CLASS_DEVTOOLS_INSPECTOR_CARD)
            .layout(
                Layout::column(12.0)
                    .size(
                        LayoutSize::new()
                            .width(LayoutDimension::Logical(DEVTOOLS_INSPECTOR_WIDTH))
                            .height(LayoutDimension::Percent(1.0)),
                    )
                    .padding_all(14.0)
                    .flex_shrink(0.0),
            ),
    );
    shell.set_dock(dock);
    ui.with_children(dock, |dock| {
        let dock_id = dock.parent();
        let shell_for_panel = shell.clone();
        dock.with_child(Panel::row(10.0).align_center(), |header| {
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
        let status = dock.add(
            Text::new("")
                .class(CLASS_DEMO_CAPTION)
                .class(CLASS_DEVTOOLS_STATUS)
                .wrap()
                .layout(Layout::new().logical_size(492.0, 44.0)),
        );
        shell.set_status(status);
        shell.update_status(dock.ui_mut(), 0, None);
        let elements = append_session_view(dock.ui_mut(), dock_id, shell.session.clone());
        shell.set_view_elements(elements);
        let _ = dock
            .ui_mut()
            .schedule_timer_repeating(dock_id, millis(DEVTOOLS_REFRESH_INTERVAL_MS));
        wire_live_stream_timer(dock.ui_mut(), dock_id, shell.clone());
    });
    Some(dock)
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

fn wire_live_stream_timer(ui: &mut Ui, dock: ElementId, shell: InProcessDevtoolsShell) {
    ui.on_timer(dock, move |ui, _event| {
        let _ = shell.refresh(ui);
    });
}

fn format_live_stream_status(
    devtools: &LiveDevtools,
    refresh_count: u32,
    stream_item_count: usize,
    tree_patch_counts: Option<DevtoolsTreePatchCounts>,
) -> String {
    let frame = devtools.mirror().summaries().frame;
    let timings = frame.timings;
    let tree_patch = tree_patch_counts.map_or_else(
        || String::from("tree patch: none"),
        |counts| format!("tree patch: +{} / -{}", counts.upserted, counts.removed),
    );
    format!(
        "Live stream - elements: {}, refreshes: {}, stream items: {}, {tree_patch}. Last frame: total {}, layout {}, paint {}, painted {}, surfaces {}",
        devtools.mirror().tree().nodes.len(),
        refresh_count,
        stream_item_count,
        duration_summary(timings.total),
        duration_summary(timings.layout),
        duration_summary(timings.paint),
        frame.painted_elements,
        frame.composition_surface_updates,
    )
}

fn duration_summary(duration: core::time::Duration) -> String {
    let micros = duration.as_micros();
    format!("{}.{:02} ms", micros / 1_000, (micros % 1_000) / 10)
}
