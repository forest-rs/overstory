// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::time::Duration;

use overstory_devtools::{
    DevtoolsSessionHandle, DevtoolsTreePatchCounts, DevtoolsViewElements, LiveDevtools,
    UiLiveInspectionTarget, append_session_view, refresh_session_view_from_mirror,
};
use overstory_transcript::{
    AnnotationLevel, EntryBody, EntryKind, EntryStatus, EntryTemplate, MessageRole, NewEntry,
    ProcessStream, TranscriptEntry, TranscriptEntryContext, TranscriptSession, TranscriptView,
    refresh_transcript_view,
};

use super::*;

const DEVTOOLS_EVENT_LOG_HEIGHT: f32 = 154.0;

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
    event_log: TranscriptSession,
    event_log_view: Rc<Cell<Option<ElementId>>>,
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
            event_log: TranscriptSession::new(),
            event_log_view: Rc::new(Cell::new(None)),
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

    fn set_event_log_view(&self, event_log_view: ElementId) {
        self.event_log_view.set(Some(event_log_view));
    }

    fn append_event_log(&self, ui: &mut Ui, entry: NewEntry<EntryBody>) {
        self.event_log.append(entry);
        if let Some(event_log_view) = self.event_log_view.get() {
            let _ = refresh_transcript_view(ui, event_log_view);
        }
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
        ui.set_local(status, ui.properties().text, content);
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
                    ui.properties().text,
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
        if should_log_refresh_event(&refresh) {
            self.append_event_log(
                ui,
                refresh_event_log_entry(&devtools_event_summary(
                    self.refresh_count.get(),
                    refresh.stream_item_count,
                    refresh.tree_patch_counts(),
                )),
            );
        }
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
        Panel::column(10.0)
            .class(CLASS_DEMO_CARD)
            .class(CLASS_DEVTOOLS_INSPECTOR_CARD)
            .layout(
                Layout::column(10.0)
                    .size(
                        LayoutSize::new()
                            .width(LayoutDimension::Logical(DEVTOOLS_INSPECTOR_WIDTH))
                            .height(LayoutDimension::Percent(1.0)),
                    )
                    .padding_all(12.0)
                    .flex_shrink(0.0),
            ),
    );
    shell.set_dock(dock);
    shell.append_event_log(
        ui,
        NewEntry::annotation(
            AnnotationLevel::Info,
            "Attached to the running gallery. Devtools subtree is excluded from target capture.",
        ),
    );
    ui.with_children(dock, |dock| {
        let dock_id = dock.parent();
        let shell_for_panel = shell.clone();
        dock.with_child(Panel::row(10.0).align_center(), |header| {
            header.add(card_title("DEVTOOLS INSPECTOR"));
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
                .layout(Layout::new().logical_size(DEVTOOLS_INSPECTOR_WIDTH - 24.0, 70.0)),
        );
        shell.set_status(status);
        shell.update_status(dock.ui_mut(), 0, None);
        let event_log = dock.add(
            TranscriptView::with_session(
                shell.event_log.clone(),
                EntryTemplate::new(render_devtools_event_entry),
            )
            .estimated_row_extent(62.0)
            .size(DEVTOOLS_INSPECTOR_WIDTH - 44.0, DEVTOOLS_EVENT_LOG_HEIGHT)
            .row_gap(8.0)
            .class(CLASS_DEVTOOLS_EVENT_LOG)
            .background(solid(0x13, 0x16, 0x20, 0xff))
            .border_brush(solid(0x39, 0x3f, 0x52, 0xff))
            .border_width(1.0)
            .corner_radius(12.0),
        );
        shell.set_event_log_view(event_log);
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

fn should_log_refresh_event(refresh: &overstory_devtools::DevtoolsRefresh) -> bool {
    refresh.stream_item_count > 0 || refresh.tree_patch_counts().is_some()
}

fn refresh_event_log_entry(summary: &str) -> NewEntry<EntryBody> {
    NewEntry::process_output(ProcessStream::Stdout, summary)
}

fn devtools_event_summary(
    refresh_count: u32,
    stream_item_count: usize,
    tree_patch_counts: Option<DevtoolsTreePatchCounts>,
) -> String {
    let patch = tree_patch_counts.map_or_else(
        || String::from("target state changed"),
        |counts| format!("tree patch +{} / -{}", counts.upserted, counts.removed),
    );
    format!("refresh {refresh_count}: {stream_item_count} stream item(s), {patch}")
}

fn render_devtools_event_entry(
    row: &mut ChildUi<'_>,
    context: TranscriptEntryContext<'_, EntryBody>,
) {
    let (label, accent, background) = devtools_event_style(context.entry());
    row.with_child(
        Panel::column(5.0)
            .layout(Layout::column(5.0).fill_width().padding_all(9.0))
            .background(solid(background.0, background.1, background.2, 0xff))
            .border_brush(solid(accent.0, accent.1, accent.2, 0xaa))
            .border_width(1.0)
            .corner_radius(10.0),
        |entry| {
            entry.with_child(Panel::row(7.0).align_center(), |header| {
                append_devtools_event_chip(header, label, accent);
                header.add(caption(status_label(context.entry().status)));
            });
            entry.add(
                Text::new(devtools_event_body(context.entry()))
                    .wrap()
                    .font_size(12.0)
                    .foreground(solid(0xee, 0xf1, 0xfa, 0xff))
                    .layout(Layout::new().min_logical_size(0.0, 0.0).flex_shrink(1.0)),
            );
        },
    );
}

fn devtools_event_style(entry: &TranscriptEntry) -> (&'static str, (u8, u8, u8), (u8, u8, u8)) {
    match &entry.kind {
        EntryKind::Annotation(annotation) => match annotation.level {
            AnnotationLevel::Info => ("attach", (0x81, 0x9b, 0xff), (0x1b, 0x20, 0x31)),
            AnnotationLevel::Warning => ("warning", (0xff, 0xc8, 0x66), (0x2a, 0x25, 0x1a)),
            AnnotationLevel::Error => ("error", (0xff, 0x8c, 0x8c), (0x2d, 0x1f, 0x24)),
        },
        EntryKind::ProcessOutput(_) => ("refresh", (0x69, 0xd2, 0x9b), (0x18, 0x26, 0x22)),
        EntryKind::Message(message) => match message.role {
            MessageRole::User => ("request", (0x81, 0x9b, 0xff), (0x1b, 0x22, 0x3a)),
            MessageRole::Assistant => ("response", (0x69, 0xd2, 0x9b), (0x18, 0x26, 0x22)),
            MessageRole::System | MessageRole::Tool | MessageRole::Other => {
                ("message", (0xbd, 0x91, 0xff), (0x22, 0x1f, 0x31))
            }
        },
        EntryKind::ToolCall(_) | EntryKind::ToolResult(_) | EntryKind::State(_) => {
            ("event", (0xbd, 0x91, 0xff), (0x22, 0x1f, 0x31))
        }
    }
}

fn append_devtools_event_chip(ui: &mut ChildUi<'_>, label: &str, accent: (u8, u8, u8)) {
    ui.with_child(
        Panel::row(0.0)
            .layout(
                Layout::row(0.0)
                    .logical_size(74.0, 20.0)
                    .padding_xy(8.0, 0.0)
                    .align_center(),
            )
            .background(solid(accent.0, accent.1, accent.2, 0x33))
            .border_brush(solid(accent.0, accent.1, accent.2, 0xbb))
            .border_width(1.0)
            .corner_radius(10.0),
        |chip| {
            chip.add(
                Text::new(label)
                    .font_size(10.0)
                    .foreground(solid(accent.0, accent.1, accent.2, 0xff)),
            );
        },
    );
}

fn devtools_event_body(entry: &TranscriptEntry) -> &str {
    entry.body().and_then(EntryBody::as_text).unwrap_or("")
}

fn status_label(status: EntryStatus) -> &'static str {
    match status {
        EntryStatus::InProgress => "running",
        EntryStatus::Complete => "complete",
        EntryStatus::Failed => "failed",
        EntryStatus::Cancelled => "cancelled",
    }
}

fn format_live_stream_status(
    devtools: &LiveDevtools,
    refresh_count: u32,
    stream_item_count: usize,
    tree_patch_counts: Option<DevtoolsTreePatchCounts>,
) -> String {
    let frame = devtools.mirror().summaries().frame;
    let timings = frame.timings;
    let renderer = frame.renderer_timings;
    let tree_patch = tree_patch_counts.map_or_else(
        || String::from("tree patch: none"),
        |counts| format!("tree patch: +{} / -{}", counts.upserted, counts.removed),
    );
    format!(
        "Live stream - elements: {}, refreshes: {}, stream items: {}, {tree_patch}.\nFrame - prepare {}, layout {}, surface refresh {}, compose {}, backend {}.\nElement realization - visited {}, changes {}, painted {}, surface updates {}",
        devtools.mirror().tree().nodes.len(),
        refresh_count,
        stream_item_count,
        duration_summary(timings.total),
        duration_summary(timings.layout),
        duration_summary(timings.surface_refresh),
        duration_summary(timings.compose),
        optional_duration_summary(renderer.total),
        frame.element_realize_visits,
        frame.element_realize_changes,
        frame.painted_elements,
        frame.composition_surface_updates,
    )
}

fn optional_duration_summary(duration: Duration) -> String {
    if duration.is_zero() {
        String::from("n/a")
    } else {
        duration_summary(duration)
    }
}

fn duration_summary(duration: Duration) -> String {
    let micros = duration.as_micros();
    format!("{}.{:02} ms", micros / 1_000, (micros % 1_000) / 10)
}
