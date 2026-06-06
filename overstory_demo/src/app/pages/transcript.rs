use overstory_transcript::{
    AnnotationLevel, EntryBody, EntryId, EntryKind, EntryStatus, EntryTemplate, MessageRole,
    NewEntry, ProcessStream, ToolOutcome, Transcript, TranscriptEntry, TranscriptEntryContext,
    TranscriptSession, TranscriptView, refresh_transcript_view,
};

use super::super::*;

const TRANSCRIPT_CARD_WIDTH: f32 = 758.0;
const TRANSCRIPT_VIEW_WIDTH: f32 = 704.0;
const TRANSCRIPT_VIEW_HEIGHT: f32 = 420.0;
const TRANSCRIPT_COMPOSER_WIDTH: f32 = 594.0;
const AGENT_GREEN: (u8, u8, u8) = (0x69, 0xd2, 0x9b);
const AGENT_BLUE: (u8, u8, u8) = (0x81, 0x9b, 0xff);
const AGENT_PURPLE: (u8, u8, u8) = (0xbd, 0x91, 0xff);
const AGENT_AMBER: (u8, u8, u8) = (0xff, 0xc8, 0x66);
const TRANSCRIPT_STREAM_INTERVAL: TimerDuration = millis(420);
const STREAM_CHUNKS: &[&str] = &[
    "I am keeping the session live while preserving the transcript model. ",
    "Rows are virtualized, tail anchoring comes from understory_virtual_list, ",
    "and the UI can keep accepting new prompts without rebuilding the whole log.",
];

#[derive(Clone, Copy, Debug, Default)]
struct ActiveStream {
    entry: Option<EntryId>,
    step: usize,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct TranscriptPage;

impl View for TranscriptPage {
    type Output = ();

    fn build(self, stage: &mut ChildUi<'_>) -> Self::Output {
        let (session, active_stream) = agent_session();
        let stream = Rc::new(RefCell::new(active_stream));
        stage.with_child(
            Panel::row(18.0).layout(
                Layout::row(18.0)
                    .fill_width()
                    .alignment(LayoutAlignment::new().align_items(LayoutAlign::Start)),
            ),
            move |row| {
                row.demo_auto_card("AGENT INTERACTION", TRANSCRIPT_CARD_WIDTH, |card| {
                    card.with_child(
                        Panel::row(12.0).layout(
                            Layout::row(12.0)
                                .fill_width()
                                .wrap()
                                .alignment(LayoutAlignment::new().align_items(LayoutAlign::Center)),
                        ),
                        |summary| {
                            add_status_pill(summary, "live session", AGENT_GREEN);
                            add_status_pill(summary, "3 tools", AGENT_BLUE);
                            add_status_pill(summary, "streaming", AGENT_PURPLE);
                        },
                    );
                    card.with_child(
                        Panel::row(12.0)
                            .layout(
                                Layout::row(12.0).fill_width().padding_all(12.0).alignment(
                                    LayoutAlignment::new().align_items(LayoutAlign::Center),
                                ),
                            )
                            .background(solid(0x1e, 0x21, 0x2b, 0xff))
                            .border_brush(solid(0x39, 0x3e, 0x4f, 0xff))
                            .border_width(1.0)
                            .corner_radius(12.0),
                        |state| {
                            state.add(
                                Text::new("Run state")
                                    .font_size(13.0)
                                    .foreground(solid(0xa9, 0xae, 0xbd, 0xff))
                                    .layout(Layout::new().logical_size(72.0, 24.0)),
                            );
                            state.add(ProgressBar::new().value(0.82).size(184.0, 12.0));
                            state.with_child(Panel::row(8.0).align_center(), |mode| {
                                mode.add(ToggleSwitch::new().checked(true));
                                mode.add(caption("interactive trace"));
                            });
                            state.add(
                                Text::new("Rows are ordinary retained elements.")
                                    .wrap()
                                    .font_size(13.0)
                                    .foreground(solid(0xd3, 0xd6, 0xe2, 0xff))
                                    .layout(
                                        Layout::new().min_logical_size(0.0, 0.0).flex_shrink(1.0),
                                    ),
                            );
                        },
                    );
                    let transcript = card.add(
                        TranscriptView::with_session(
                            session.clone(),
                            EntryTemplate::new(render_agent_entry),
                        )
                        .estimated_entry_extent(estimated_agent_entry_extent)
                        .size(TRANSCRIPT_VIEW_WIDTH, TRANSCRIPT_VIEW_HEIGHT)
                        .row_gap(14.0)
                        .background(solid(0x15, 0x17, 0x20, 0xff))
                        .border_brush(solid(0x35, 0x39, 0x48, 0xff))
                        .border_width(1.0)
                        .corner_radius(14.0),
                    );
                    schedule_stream(card.ui_mut(), transcript, session.clone(), stream.clone());
                    append_composer(card, transcript, session.clone(), stream.clone());
                });
            },
        );
    }
}

fn agent_session() -> (TranscriptSession, ActiveStream) {
    let mut transcript = Transcript::new();

    let system = transcript.append(NewEntry::message(
        MessageRole::System,
        "Use retained UI primitives. Keep the headless transcript independent from rendering.",
    ));
    let user = transcript.append(
        NewEntry::message(
            MessageRole::User,
            "Can you look at the frame hitch, explain what changed, and leave the UI in a state I can inspect?",
        )
        .with_cause(system),
    );
    let plan = transcript.append(
        NewEntry::message(
            MessageRole::Assistant,
            "I will sample layout and paint, keep the interaction log live, and surface the result as reusable controls rather than one-off strings.",
        )
        .with_cause(user),
    );
    let sample = transcript.append(
        NewEntry::tool_call(
            "overstory_wind_tunnel.sample",
            "layout_repeated_text_labels + paint_repeated_text_labels",
        )
        .with_cause(plan)
        .with_status(EntryStatus::InProgress),
    );
    transcript.append(
        NewEntry::process_output(
            ProcessStream::Stdout,
            "layout: 1.8 ms avg | paint scene build: 0.43 ms avg | text cache hit rate: 94%",
        )
        .with_parent(sample),
    );
    transcript.append(
        NewEntry::tool_result(
            "overstory_wind_tunnel.sample",
            ToolOutcome::Success,
            "The slow path is debug-only layout churn; release build stays inside the frame budget.",
        )
        .with_cause(sample),
    );
    transcript.append(
        NewEntry::annotation(
            AnnotationLevel::Info,
            "The transcript view is a control wrapper. The rows below are panels, text, buttons, progress bars, and toggles.",
        )
        .with_cause(sample),
    );
    transcript.append(
        NewEntry::state(
            "selection",
            "Entry #4 is linked as process output under the sampling tool call.",
        )
        .with_cause(sample),
    );
    let assistant = transcript.append(
        NewEntry::message(
            MessageRole::Assistant,
            "I found one real source of latency, kept the render path unchanged, and added a reusable transcript display that can grow into agent interaction history.",
        )
        .with_cause(sample)
        .with_status(EntryStatus::InProgress),
    );

    (
        TranscriptSession::from_transcript(transcript),
        ActiveStream {
            entry: Some(assistant),
            step: 0,
        },
    )
}

fn schedule_stream(
    ui: &mut Ui,
    transcript: ElementId,
    session: TranscriptSession,
    stream: Rc<RefCell<ActiveStream>>,
) {
    let _ = ui.schedule_timer_repeating(transcript, TRANSCRIPT_STREAM_INTERVAL);
    ui.on_timer(transcript, move |ui, _event| {
        let (entry, chunk, complete) = {
            let mut stream = stream.borrow_mut();
            let Some(entry) = stream.entry else {
                return;
            };
            if let Some(chunk) = STREAM_CHUNKS.get(stream.step).copied() {
                stream.step += 1;
                (entry, Some(chunk), false)
            } else {
                stream.entry = None;
                stream.step = 0;
                (entry, None, true)
            }
        };

        if let Some(chunk) = chunk {
            let _ = session.append_chunk(entry, chunk);
        }
        if complete {
            let _ = session.set_status(entry, EntryStatus::Complete);
        }
        let _ = refresh_transcript_view(ui, transcript);
    });
}

fn append_composer(
    card: &mut ChildUi<'_>,
    transcript: ElementId,
    session: TranscriptSession,
    stream: Rc<RefCell<ActiveStream>>,
) {
    card.with_child(
        Panel::row(10.0)
            .layout(
                Layout::row(10.0)
                    .fill_width()
                    .padding_all(10.0)
                    .align_center(),
            )
            .background(solid(0x1c, 0x20, 0x2a, 0xff))
            .border_brush(solid(0x3a, 0x41, 0x54, 0xff))
            .border_width(1.0)
            .corner_radius(14.0),
        move |composer| {
            let input = composer.add(
                TextInput::new()
                    .placeholder("Ask the agent to continue...")
                    .width(TRANSCRIPT_COMPOSER_WIDTH)
                    .font_size(14.0),
            );
            composer.add(
                Button::new("Send")
                    .font_size(13.0)
                    .on_click(move |ui, _event| {
                        let text = text_input_text(ui, input).unwrap_or_default();
                        let prompt = text.trim();
                        if prompt.is_empty() {
                            return;
                        }

                        let user = session.append(NewEntry::message(MessageRole::User, prompt));
                        let assistant = session.append(
                            NewEntry::message(MessageRole::Assistant, EntryBody::Empty)
                                .with_cause(user)
                                .with_status(EntryStatus::InProgress),
                        );
                        {
                            let mut stream = stream.borrow_mut();
                            stream.entry = Some(assistant);
                            stream.step = 0;
                        }
                        let _ = set_text_input_text(ui, input, "");
                        let _ = refresh_transcript_view(ui, transcript);
                    }),
            );
        },
    );
}

fn render_agent_entry(row: &mut ChildUi<'_>, context: TranscriptEntryContext<'_, EntryBody>) {
    match &context.entry().kind {
        EntryKind::Message(message) => render_message(row, context, message.role),
        EntryKind::ToolCall(tool) => render_tool_call(row, context, &tool.tool_name),
        EntryKind::ToolResult(result) => render_tool_result(row, context, &result.tool_name),
        EntryKind::ProcessOutput(output) => render_process_output(row, context, output.stream),
        EntryKind::Annotation(annotation) => render_annotation(row, context, annotation.level),
        EntryKind::State(state) => render_state(row, context, &state.label),
    }
}

fn estimated_agent_entry_extent(context: TranscriptEntryContext<'_, EntryBody>) -> f64 {
    let body_len = body_text_or_empty(context.entry()).len() as f64;
    let text_lines = (body_len / 92.0).ceil().max(1.0);
    match &context.entry().kind {
        EntryKind::Message(message) => match message.role {
            MessageRole::Assistant if context.index() > 6 => 184.0 + (text_lines * 20.0),
            MessageRole::Assistant => 104.0 + (text_lines * 20.0),
            MessageRole::User => 96.0 + (text_lines * 20.0),
            MessageRole::System => 92.0 + (text_lines * 18.0),
            MessageRole::Tool | MessageRole::Other => 96.0 + (text_lines * 18.0),
        },
        EntryKind::ToolCall(_) => 154.0,
        EntryKind::ToolResult(_) => 150.0,
        EntryKind::ProcessOutput(_) => 94.0 + (text_lines * 18.0),
        EntryKind::Annotation(_) => 82.0 + (text_lines * 18.0),
        EntryKind::State(_) => 76.0,
    }
}

fn render_message(
    row: &mut ChildUi<'_>,
    context: TranscriptEntryContext<'_, EntryBody>,
    role: MessageRole,
) {
    let (label, accent, background, width) = match role {
        MessageRole::System => (
            "system",
            AGENT_AMBER,
            (0x2b, 0x27, 0x1e),
            TRANSCRIPT_VIEW_WIDTH - 42.0,
        ),
        MessageRole::User => (
            "human",
            AGENT_BLUE,
            (0x24, 0x31, 0x5d),
            TRANSCRIPT_VIEW_WIDTH - 86.0,
        ),
        MessageRole::Assistant => (
            "assistant",
            AGENT_GREEN,
            (0x1f, 0x2f, 0x28),
            TRANSCRIPT_VIEW_WIDTH - 70.0,
        ),
        MessageRole::Tool | MessageRole::Other => (
            "message",
            AGENT_PURPLE,
            (0x27, 0x26, 0x34),
            TRANSCRIPT_VIEW_WIDTH - 70.0,
        ),
    };

    row.with_child(
        Panel::column(8.0)
            .layout(
                Layout::column(8.0)
                    .size(
                        LayoutSize::new()
                            .width(LayoutDimension::Logical(width))
                            .min_width(LayoutDimension::Logical(0.0)),
                    )
                    .padding_all(14.0),
            )
            .background(solid(background.0, background.1, background.2, 0xff))
            .border_brush(accent_brush(accent, 0xaa))
            .border_width(1.0)
            .corner_radius(16.0),
        |bubble| {
            add_status_pill(bubble, label, accent);
            bubble.add(
                Text::new(body_text_or_empty(context.entry()))
                    .wrap()
                    .font_size(15.0)
                    .foreground(solid(0xf6, 0xf7, 0xfb, 0xff)),
            );
            if role == MessageRole::Assistant && context.index() > 6 {
                bubble.with_child(
                    Panel::row(10.0).layout(Layout::row(10.0).wrap()),
                    |metrics| {
                        add_metric_tile(metrics, "latency", "ok", AGENT_GREEN);
                        add_metric_tile(metrics, "cache", "94%", AGENT_BLUE);
                        add_metric_tile(metrics, "next", "devtools", AGENT_PURPLE);
                    },
                );
            }
        },
    );
}

fn render_tool_call(
    row: &mut ChildUi<'_>,
    context: TranscriptEntryContext<'_, EntryBody>,
    tool_name: &str,
) {
    row.with_child(
        Panel::column(10.0)
            .layout(
                Layout::column(10.0)
                    .fill_width()
                    .padding_all(14.0)
                    .min_logical_size(0.0, 0.0),
            )
            .background(solid(0x1f, 0x21, 0x2c, 0xff))
            .border_brush(accent_brush(AGENT_PURPLE, 0xaa))
            .border_width(1.0)
            .corner_radius(14.0),
        |card| {
            card.with_child(Panel::row(10.0).align_center(), |header| {
                add_status_pill(header, "tool call", AGENT_PURPLE);
                header.add(caption(tool_name));
            });
            card.add(
                Text::new(body_text_or_empty(context.entry()))
                    .wrap()
                    .font_size(14.0)
                    .foreground(solid(0xec, 0xee, 0xf8, 0xff)),
            );
            card.with_child(Panel::row(12.0).align_center(), |controls| {
                controls.add(ProgressBar::new().value(0.82).size(210.0, 12.0));
                controls.add(Button::new("Open trace").font_size(13.0));
                add_status_pill(controls, status_label(context.entry().status), AGENT_AMBER);
            });
        },
    );
}

fn render_tool_result(
    row: &mut ChildUi<'_>,
    context: TranscriptEntryContext<'_, EntryBody>,
    tool_name: &str,
) {
    row.with_child(
        Panel::column(8.0)
            .layout(Layout::column(8.0).fill_width().padding_all(14.0))
            .background(solid(0x1d, 0x2b, 0x25, 0xff))
            .border_brush(accent_brush(AGENT_GREEN, 0xbb))
            .border_width(1.0)
            .corner_radius(14.0),
        |card| {
            card.with_child(Panel::row(10.0).align_center(), |header| {
                add_status_pill(header, "success", AGENT_GREEN);
                header.add(caption(tool_name));
            });
            card.add(
                Text::new(body_text_or_empty(context.entry()))
                    .wrap()
                    .font_size(14.0)
                    .foreground(solid(0xf2, 0xf8, 0xf5, 0xff)),
            );
            card.with_child(Panel::row(8.0).layout(Layout::row(8.0).wrap()), |row| {
                row.add(CheckBox::new("release fast").checked(true));
                row.add(CheckBox::new("debug explains hitch").checked(true));
            });
        },
    );
}

fn render_process_output(
    row: &mut ChildUi<'_>,
    context: TranscriptEntryContext<'_, EntryBody>,
    stream: ProcessStream,
) {
    let stream_label = match stream {
        ProcessStream::Stdin => "stdin",
        ProcessStream::Stdout => "stdout",
        ProcessStream::Stderr => "stderr",
    };
    row.with_child(
        Panel::column(6.0)
            .layout(Layout::column(6.0).fill_width().padding_all(12.0))
            .background(solid(0x10, 0x12, 0x18, 0xff))
            .border_brush(solid(0x38, 0x3d, 0x4c, 0xff))
            .border_width(1.0)
            .corner_radius(12.0),
        |terminal| {
            add_status_pill(terminal, stream_label, AGENT_BLUE);
            terminal.add(
                Text::new(body_text_or_empty(context.entry()))
                    .wrap()
                    .font_size(13.0)
                    .foreground(solid(0xd9, 0xe0, 0xff, 0xff)),
            );
        },
    );
}

fn render_annotation(
    row: &mut ChildUi<'_>,
    context: TranscriptEntryContext<'_, EntryBody>,
    level: AnnotationLevel,
) {
    let (label, accent) = match level {
        AnnotationLevel::Info => ("note", AGENT_BLUE),
        AnnotationLevel::Warning => ("warning", AGENT_AMBER),
        AnnotationLevel::Error => ("error", (0xff, 0x8c, 0x8c)),
    };
    row.with_child(
        Panel::row(10.0)
            .layout(
                Layout::row(10.0)
                    .fill_width()
                    .padding_all(12.0)
                    .align_center(),
            )
            .background(solid(0x24, 0x25, 0x30, 0xff))
            .border_brush(accent_brush(accent, 0x88))
            .border_width(1.0)
            .corner_radius(12.0),
        |note| {
            add_status_pill(note, label, accent);
            note.add(
                Text::new(body_text_or_empty(context.entry()))
                    .wrap()
                    .font_size(14.0)
                    .foreground(solid(0xf2, 0xf3, 0xf8, 0xff))
                    .layout(Layout::new().min_logical_size(0.0, 0.0).flex_shrink(1.0)),
            );
        },
    );
}

fn render_state(
    row: &mut ChildUi<'_>,
    context: TranscriptEntryContext<'_, EntryBody>,
    label: &str,
) {
    row.with_child(
        Panel::row(10.0)
            .layout(
                Layout::row(10.0)
                    .fill_width()
                    .padding_all(12.0)
                    .align_center(),
            )
            .background(solid(0x1e, 0x25, 0x32, 0xff))
            .border_brush(accent_brush(AGENT_BLUE, 0x88))
            .border_width(1.0)
            .corner_radius(12.0),
        |state| {
            add_status_pill(state, "state", AGENT_BLUE);
            state.add(control_label(label).font_size(14.0));
            state.add(
                caption(body_text_or_empty(context.entry()))
                    .wrap()
                    .layout(Layout::new().min_logical_size(0.0, 0.0).flex_shrink(1.0)),
            );
        },
    );
}

fn add_metric_tile(ui: &mut ChildUi<'_>, label: &str, value: &str, accent: (u8, u8, u8)) {
    ui.with_child(
        Panel::column(2.0)
            .layout(
                Layout::column(2.0)
                    .logical_size(108.0, 54.0)
                    .padding_all(8.0),
            )
            .background(solid(0x18, 0x1c, 0x25, 0xff))
            .border_brush(accent_brush(accent, 0x80))
            .border_width(1.0)
            .corner_radius(10.0),
        |tile| {
            tile.add(
                Text::new(label)
                    .font_size(11.0)
                    .foreground(solid(0xbd, 0xc3, 0xd2, 0xff)),
            );
            tile.add(
                Text::new(value)
                    .font_size(17.0)
                    .foreground(accent_brush(accent, 0xff)),
            );
        },
    );
}

fn add_status_pill(ui: &mut ChildUi<'_>, label: &str, accent: (u8, u8, u8)) {
    ui.with_child(
        Panel::new()
            .layout(
                Layout::row(0.0)
                    .logical_size(112.0, 24.0)
                    .padding_xy(10.0, 0.0)
                    .align_center(),
            )
            .background(accent_brush(accent, 0x33))
            .border_brush(accent_brush(accent, 0xbb))
            .border_width(1.0)
            .corner_radius(12.0),
        |pill| {
            pill.add(
                Text::new(label)
                    .font_size(11.0)
                    .foreground(accent_brush(accent, 0xff)),
            );
        },
    );
}

fn body_text_or_empty(entry: &TranscriptEntry) -> &str {
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

fn accent_brush((r, g, b): (u8, u8, u8), a: u8) -> Brush {
    solid(r, g, b, a)
}
