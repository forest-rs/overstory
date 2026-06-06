// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg_attr(not(feature = "std"), no_std)]

//! Overstory transcript controls.
//!
//! `overstory_transcript` adapts the headless [`understory_transcript`]
//! interaction model into retained Overstory element trees.
//!
//! ## Fence
//!
//! This crate owns visual transcript realization: scrolling, row construction,
//! and the entry-template hook that lets transcript entries become rich
//! Overstory subtrees. It deliberately does not own transcript storage,
//! streaming mutation semantics, persistence, protocol semantics, or agent
//! runtime policy; those stay outside the control wrapper.
//!
//! ## Rich rows
//!
//! [`TranscriptView::new`] provides a plain built-in renderer for the default
//! [`EntryBody`] payload. Apps that need agent-style transcripts can use
//! [`TranscriptView::with_entry_template`] to turn each entry into any mix of
//! Overstory elements:
//!
//! ```ignore
//! use overstory::prelude::*;
//! use overstory_transcript::{
//!     EntryBody, EntryTemplate, MessageRole, NewEntry, Transcript, TranscriptView,
//! };
//!
//! let mut transcript = Transcript::new();
//! transcript.append(NewEntry::message(MessageRole::User, "Profile the renderer"));
//!
//! let view = TranscriptView::with_entry_template(
//!     transcript,
//!     EntryTemplate::new(|row, entry| {
//!         row.add(Text::new(entry.body_text().unwrap_or("")).wrap());
//!         row.add(ProgressBar::new().value(0.72).size(160.0, 10.0));
//!     }),
//! );
//! ```

extern crate alloc;

use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;

use overstory::{
    AppendSpec, Brush, ChildUi, Color, ElementConfig, ElementId, ElementSpec, Layout,
    LayoutDimension, LayoutSize, Panel, ScrollArea, SurfaceSpecExt, Text, Ui, scroll_area_viewport,
};
pub use understory_transcript::{
    AnnotationLevel, EntryBody, EntryId, EntryKind, EntryStatus, MessageRole, NewEntry,
    ProcessStream, Timestamp, ToolOutcome, Transcript, TranscriptEntry, TranscriptError,
};

const DEFAULT_ROW_GAP: f32 = 12.0;

type EntryTemplateFn<P> =
    dyn for<'ui, 'entry> Fn(&mut ChildUi<'ui>, TranscriptEntryContext<'entry, P>);

/// Callback used to realize one transcript entry into Overstory elements.
///
/// The template receives a [`ChildUi`] already scoped to that entry's row. It
/// can append text, controls, progress indicators, nested panels, or any
/// downstream custom Overstory widgets.
pub struct EntryTemplate<P = EntryBody> {
    render: Rc<EntryTemplateFn<P>>,
}

impl<P> Clone for EntryTemplate<P> {
    fn clone(&self) -> Self {
        Self {
            render: Rc::clone(&self.render),
        }
    }
}

impl<P> core::fmt::Debug for EntryTemplate<P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EntryTemplate").finish_non_exhaustive()
    }
}

impl<P> EntryTemplate<P> {
    /// Creates an entry template from a row-building callback.
    ///
    /// The callback is invoked during append, not retained as live mutable UI
    /// state. Rebuilding a transcript view therefore re-runs the template for
    /// the current transcript entries.
    #[must_use]
    pub fn new<F>(render: F) -> Self
    where
        F: for<'ui, 'entry> Fn(&mut ChildUi<'ui>, TranscriptEntryContext<'entry, P>) + 'static,
    {
        Self {
            render: Rc::new(render),
        }
    }

    fn render(&self, row: &mut ChildUi<'_>, entry: TranscriptEntryContext<'_, P>) {
        (self.render)(row, entry);
    }
}

/// Per-row context passed to a transcript entry template.
#[derive(Clone, Copy, Debug)]
pub struct TranscriptEntryContext<'a, P = EntryBody> {
    index: usize,
    entry: &'a TranscriptEntry<P>,
    children: &'a [EntryId],
}

impl<'a, P> TranscriptEntryContext<'a, P> {
    /// Creates a row context.
    #[must_use]
    pub const fn new(index: usize, entry: &'a TranscriptEntry<P>, children: &'a [EntryId]) -> Self {
        Self {
            index,
            entry,
            children,
        }
    }

    /// Append-order index of this entry.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Stored transcript entry.
    #[must_use]
    pub const fn entry(&self) -> &'a TranscriptEntry<P> {
        self.entry
    }

    /// Direct child entry ids for this entry.
    #[must_use]
    pub const fn children(&self) -> &'a [EntryId] {
        self.children
    }
}

impl TranscriptEntryContext<'_, EntryBody> {
    /// Returns the entry payload as text when it uses the built-in text body.
    #[must_use]
    pub fn body_text(&self) -> Option<&str> {
        self.entry.body().and_then(EntryBody::as_text)
    }
}

/// Composite transcript view backed by [`understory_transcript::Transcript`].
///
/// `TranscriptView` appends a scroll area and realizes one child row per
/// transcript entry inside the area viewport. The row body comes from an
/// [`EntryTemplate`], which keeps
/// the headless transcript model free of Overstory element types while still
/// allowing rich rows in applications.
pub struct TranscriptView<P = EntryBody> {
    transcript: Transcript<P>,
    entry_template: EntryTemplate<P>,
    row_gap: f32,
    size: Option<(f32, f32)>,
    scroll_offset_y: f64,
    element: ElementConfig,
}

impl<P> core::fmt::Debug for TranscriptView<P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TranscriptView")
            .field("entry_count", &self.transcript.len())
            .field("entry_template", &self.entry_template)
            .field("row_gap", &self.row_gap)
            .field("size", &self.size)
            .field("scroll_offset_y", &self.scroll_offset_y)
            .field("element", &self.element)
            .finish()
    }
}

impl TranscriptView<EntryBody> {
    /// Creates a transcript view with the built-in text-oriented row template.
    #[must_use]
    pub fn new(transcript: Transcript) -> Self {
        Self::with_entry_template(transcript, EntryTemplate::new(default_entry_template))
    }
}

impl<P> TranscriptView<P> {
    /// Creates a transcript view with an explicit row template.
    #[must_use]
    pub fn with_entry_template(
        transcript: Transcript<P>,
        entry_template: EntryTemplate<P>,
    ) -> Self {
        Self {
            transcript,
            entry_template,
            row_gap: DEFAULT_ROW_GAP,
            size: None,
            scroll_offset_y: 0.0,
            element: ElementConfig::default(),
        }
    }

    /// Replaces the row template.
    #[must_use]
    pub fn entry_template(mut self, entry_template: EntryTemplate<P>) -> Self {
        self.entry_template = entry_template;
        self
    }

    /// Sets the gap between realized transcript rows.
    #[must_use]
    pub const fn row_gap(mut self, gap: f32) -> Self {
        self.row_gap = gap;
        self
    }

    /// Sets the fixed logical-pixel size of the scroll viewport.
    #[must_use]
    pub const fn size(mut self, width: f32, height: f32) -> Self {
        self.size = Some((width, height));
        self
    }

    /// Sets the initial vertical scroll offset.
    #[must_use]
    pub const fn scroll_offset_y(mut self, offset: f64) -> Self {
        self.scroll_offset_y = offset;
        self
    }
}

impl<P> AppendSpec for TranscriptView<P> {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let mut scroll_area = ScrollArea::new()
            .horizontal_scrollbar(false)
            .vertical_scrollbar(true)
            .scroll_offset_y(self.scroll_offset_y);
        if let Some((width, height)) = self.size {
            scroll_area = scroll_area.size(width, height);
        }

        let root = ui.append(parent, scroll_area);
        self.element.apply_to(ui, root);

        let viewport = scroll_area_viewport(ui, root).unwrap_or(root);
        let content_layout = Layout::column(self.row_gap).fill_width().padding_all(2.0);
        let content = ui.append(viewport, Panel::new().layout(content_layout));

        for (index, entry) in self.transcript.iter().enumerate() {
            let children = self.transcript.children_of(entry.id);
            ui.with_children(content, |content| {
                content.with_child(
                    Panel::new().layout(Layout::column(0.0).fill_width()),
                    |row| {
                        self.entry_template
                            .render(row, TranscriptEntryContext::new(index, entry, children));
                    },
                );
            });
        }

        root
    }
}

impl<P> ElementSpec for TranscriptView<P> {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

fn default_entry_template(row: &mut ChildUi<'_>, context: TranscriptEntryContext<'_, EntryBody>) {
    let (background, border) = default_row_colors(context.entry());
    row.with_child(
        Panel::column(6.0)
            .layout(
                Layout::column(6.0)
                    .size(
                        LayoutSize::new()
                            .width(LayoutDimension::Percent(1.0))
                            .min_width(LayoutDimension::Logical(0.0)),
                    )
                    .padding_all(12.0),
            )
            .background(background)
            .border_brush(border)
            .border_width(1.0)
            .corner_radius(12.0),
        |entry| {
            entry.add(
                Text::new(default_entry_heading(context.entry()))
                    .font_size(12.0)
                    .foreground(Color::from_rgba8(0xc7, 0xcd, 0xdc, 0xff)),
            );
            if let Some(body) = default_entry_body(context.entry()) {
                entry.add(
                    Text::new(body)
                        .wrap()
                        .font_size(14.0)
                        .foreground(Color::from_rgba8(0xf6, 0xf7, 0xfb, 0xff)),
                );
            }
        },
    );
}

fn default_row_colors(entry: &TranscriptEntry) -> (Option<Brush>, Option<Brush>) {
    match &entry.kind {
        EntryKind::Message(message) if message.role == MessageRole::User => (
            Some(Color::from_rgba8(0x2f, 0x45, 0x87, 0xff).into()),
            Some(Color::from_rgba8(0x87, 0xa2, 0xff, 0xff).into()),
        ),
        EntryKind::Message(message) if message.role == MessageRole::Assistant => (
            Some(Color::from_rgba8(0x23, 0x32, 0x2c, 0xff).into()),
            Some(Color::from_rgba8(0x76, 0xdc, 0xa4, 0xff).into()),
        ),
        EntryKind::ToolCall(_) | EntryKind::ToolResult(_) | EntryKind::ProcessOutput(_) => (
            Some(Color::from_rgba8(0x20, 0x22, 0x2d, 0xff).into()),
            Some(Color::from_rgba8(0x69, 0x70, 0x83, 0xff).into()),
        ),
        EntryKind::Annotation(_) | EntryKind::State(_) | EntryKind::Message(_) => (
            Some(Color::from_rgba8(0x26, 0x27, 0x30, 0xff).into()),
            Some(Color::from_rgba8(0x51, 0x55, 0x66, 0xff).into()),
        ),
    }
}

fn default_entry_heading(entry: &TranscriptEntry) -> String {
    let kind = match &entry.kind {
        EntryKind::Message(message) => match message.role {
            MessageRole::System => "System",
            MessageRole::User => "User",
            MessageRole::Assistant => "Assistant",
            MessageRole::Tool => "Tool",
            MessageRole::Other => "Message",
        },
        EntryKind::ToolCall(entry) => return format!("Tool call: {}", entry.tool_name),
        EntryKind::ToolResult(entry) => return format!("Tool result: {}", entry.tool_name),
        EntryKind::ProcessOutput(entry) => match entry.stream {
            ProcessStream::Stdin => "stdin",
            ProcessStream::Stdout => "stdout",
            ProcessStream::Stderr => "stderr",
        },
        EntryKind::Annotation(entry) => match entry.level {
            AnnotationLevel::Info => "Note",
            AnnotationLevel::Warning => "Warning",
            AnnotationLevel::Error => "Error",
        },
        EntryKind::State(entry) => return format!("State: {}", entry.label),
    };

    if entry.status == EntryStatus::Complete {
        String::from(kind)
    } else {
        format!("{kind} ({})", status_label(entry.status))
    }
}

fn default_entry_body(entry: &TranscriptEntry) -> Option<String> {
    match entry.body()? {
        EntryBody::Text(text) => Some(text.clone()),
        EntryBody::Bytes(bytes) => Some(format!("{} bytes", bytes.len())),
        EntryBody::Empty => None,
    }
}

fn status_label(status: EntryStatus) -> &'static str {
    match status {
        EntryStatus::InProgress => "in progress",
        EntryStatus::Complete => "complete",
        EntryStatus::Failed => "failed",
        EntryStatus::Cancelled => "cancelled",
    }
}

#[cfg(test)]
mod tests {
    use alloc::format;

    use overstory::{Panel, ProgressBar, Text};

    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Payload {
        Card(&'static str),
    }

    #[test]
    fn template_realizes_one_row_per_entry() {
        let mut transcript = Transcript::new();
        transcript.append(NewEntry::message(MessageRole::User, "inspect"));
        transcript.append(NewEntry::message(MessageRole::Assistant, "done"));

        let mut ui = Ui::new();
        let root = ui.root();
        let view = ui.append(
            root,
            TranscriptView::new(transcript).entry_template(EntryTemplate::new(|row, entry| {
                row.add(Text::new(format!("entry {}", entry.index())));
            })),
        );

        let viewport = scroll_area_viewport(&ui, view).unwrap();
        let content = ui.get(viewport).unwrap().children()[0];
        assert_eq!(ui.get(content).unwrap().children().len(), 2);
    }

    #[test]
    fn generic_payload_template_can_append_rich_children() {
        let mut transcript = Transcript::<Payload>::new();
        transcript.append(NewEntry::state("preview", Payload::Card("metrics")));

        let mut ui = Ui::new();
        let root = ui.root();
        let view = ui.append(
            root,
            TranscriptView::with_entry_template(
                transcript,
                EntryTemplate::new(|row, entry| {
                    let Payload::Card(label) = entry.entry().body().unwrap();
                    row.with_child(Panel::column(4.0), |card| {
                        card.add(Text::new(*label));
                        card.add(ProgressBar::new().value(0.5).size(80.0, 8.0));
                    });
                }),
            ),
        );

        let viewport = scroll_area_viewport(&ui, view).unwrap();
        let content = ui.get(viewport).unwrap().children()[0];
        let row = ui.get(content).unwrap().children()[0];
        let card = ui.get(row).unwrap().children()[0];
        assert_eq!(ui.get(card).unwrap().children().len(), 2);
    }
}
