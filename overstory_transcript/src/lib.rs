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

use alloc::boxed::Box;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::ops::Range;

use overstory::{
    AccessibilityContext, AppendSpec, ArrangeContext, ArrangeResult, Brush, ChildUi, Color,
    Element, ElementConfig, ElementId, ElementRealizeContext, ElementRealizeResult, ElementSpec,
    Layout, LayoutDimension, LayoutSize, Panel, Phase, ScrollBar, ScrollEvent, ScrollState, Size,
    SurfaceSpecExt, Text, Ui, Vec2, Widget, WidgetEventContext, accesskit,
};
pub use understory_transcript::{
    AnnotationLevel, EntryBody, EntryId, EntryKind, EntryStatus, MessageRole, NewEntry,
    ProcessStream, Timestamp, ToolOutcome, Transcript, TranscriptEntry, TranscriptError,
};
use understory_virtual_list::{PrefixSumExtentModel, TailAnchoredExtentModel, VirtualList};

const DEFAULT_ROW_GAP: f64 = 12.0;
const DEFAULT_ROW_EXTENT: f64 = 132.0;
const DEFAULT_OVERSCAN: f64 = 264.0;
const DEFAULT_SCROLLBAR_GAP: f64 = 10.0;
const DEFAULT_SCROLLBAR_THICKNESS: f64 = 10.0;
const DEFAULT_SCROLLBAR_THUMB_EXTENT: f64 = 42.0;
const DEFAULT_WIDTH: f64 = 640.0;
const DEFAULT_HEIGHT: f64 = 360.0;
const SCROLL_LINE: f64 = 48.0;

type TranscriptVirtualList = VirtualList<TailAnchoredExtentModel<PrefixSumExtentModel<f64>>>;

type EntryTemplateFn<P> =
    dyn for<'ui, 'entry> Fn(&mut ChildUi<'ui>, TranscriptEntryContext<'entry, P>);
type EstimatedEntryExtentFn<P> = dyn for<'entry> Fn(TranscriptEntryContext<'entry, P>) -> f64;

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

/// Shared live transcript handle used by [`TranscriptView`].
///
/// `TranscriptSession` is intentionally a thin UI-facing wrapper around the
/// headless [`Transcript`]. It gives retained UI code a cloneable shared handle
/// while content revision tracking stays owned by the transcript model itself.
#[derive(Clone)]
pub struct TranscriptSession<P = EntryBody> {
    inner: Rc<RefCell<Transcript<P>>>,
}

impl<P> core::fmt::Debug for TranscriptSession<P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let transcript = self.inner.borrow();
        f.debug_struct("TranscriptSession")
            .field("entry_count", &transcript.len())
            .field("revision", &transcript.revision())
            .finish()
    }
}

impl<P> TranscriptSession<P> {
    /// Creates an empty live transcript session.
    #[must_use]
    pub fn new() -> Self {
        Self::from_transcript(Transcript::new())
    }

    /// Wraps an existing transcript in a live session.
    #[must_use]
    pub fn from_transcript(transcript: Transcript<P>) -> Self {
        Self {
            inner: Rc::new(RefCell::new(transcript)),
        }
    }

    /// Returns the number of entries currently stored.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.borrow().len()
    }

    /// Returns whether the session currently has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.borrow().is_empty()
    }

    /// Returns the current content revision.
    ///
    /// The revision comes from the headless transcript and increments only when
    /// transcript storage actually changes. Widgets can cache this value to
    /// avoid rebuilding realized rows when only layout or scroll position
    /// changed.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.inner.borrow().revision()
    }

    /// Reads the underlying transcript for the duration of `read`.
    ///
    /// The borrowed transcript must not escape the callback. This keeps the
    /// session handle small while still allowing row templates and diagnostics
    /// to inspect entries without cloning the whole log.
    pub fn with_transcript<R>(&self, read: impl FnOnce(&Transcript<P>) -> R) -> R {
        read(&self.inner.borrow())
    }

    /// Appends `entry` and returns its assigned id.
    pub fn append(&self, entry: NewEntry<P>) -> EntryId {
        self.inner.borrow_mut().append(entry)
    }

    /// Sets the lifecycle status for an entry.
    ///
    /// Returns `Ok(true)` when the stored status changed and the session
    /// revision advanced.
    pub fn set_status(&self, id: EntryId, status: EntryStatus) -> Result<bool, TranscriptError> {
        self.inner.borrow_mut().set_status(id, status)
    }
}

impl<P> Default for TranscriptSession<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl TranscriptSession<EntryBody> {
    /// Appends a text or byte chunk to an existing built-in entry body.
    pub fn append_chunk(
        &self,
        id: EntryId,
        chunk: impl Into<EntryBody>,
    ) -> Result<bool, TranscriptError> {
        self.inner.borrow_mut().append_chunk(id, chunk)
    }
}

/// Composite transcript view backed by [`understory_transcript::Transcript`].
///
/// `TranscriptView` appends a frame containing a virtualized transcript
/// viewport and a reserved vertical scrollbar. The viewport realizes only the
/// visible/overscanned transcript rows and uses
/// [`understory_virtual_list::TailAnchoredExtentModel`] so append-heavy
/// transcript sessions stay pinned to the tail when the user was already at
/// the tail.
///
/// The row body comes from an [`EntryTemplate`], which keeps the headless
/// transcript model free of Overstory element types while still allowing rich
/// rows in applications.
pub struct TranscriptView<P = EntryBody> {
    session: TranscriptSession<P>,
    entry_template: EntryTemplate<P>,
    row_gap: f64,
    estimated_entry_extent: Rc<EstimatedEntryExtentFn<P>>,
    overscan: f64,
    size: Option<(f64, f64)>,
    scroll_offset_y: f64,
    tail_anchored: bool,
    element: ElementConfig,
}

impl<P> core::fmt::Debug for TranscriptView<P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TranscriptView")
            .field("session", &self.session)
            .field("entry_template", &self.entry_template)
            .field("row_gap", &self.row_gap)
            .field("estimated_entry_extent", &"<estimated entry extent policy>")
            .field("overscan", &self.overscan)
            .field("size", &self.size)
            .field("scroll_offset_y", &self.scroll_offset_y)
            .field("tail_anchored", &self.tail_anchored)
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
        Self::with_session(
            TranscriptSession::from_transcript(transcript),
            entry_template,
        )
    }

    /// Creates a transcript view from a live session and explicit row template.
    #[must_use]
    pub fn with_session(session: TranscriptSession<P>, entry_template: EntryTemplate<P>) -> Self {
        Self {
            session,
            entry_template,
            row_gap: DEFAULT_ROW_GAP,
            estimated_entry_extent: Rc::new(|_| DEFAULT_ROW_EXTENT),
            overscan: DEFAULT_OVERSCAN,
            size: None,
            scroll_offset_y: 0.0,
            tail_anchored: true,
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
    pub const fn row_gap(mut self, gap: f64) -> Self {
        self.row_gap = gap;
        self
    }

    /// Sets the estimated logical-pixel stride for one virtualized row.
    ///
    /// The estimate includes any visual gap after the row. Realized rows feed
    /// their measured heights back into the virtual list after layout, so this
    /// value is a first guess for offscreen rows rather than a hard row height.
    /// Use [`Self::estimated_entry_extent`] when rows have meaningfully
    /// different likely heights.
    #[must_use]
    pub fn estimated_row_extent(mut self, extent: f64) -> Self {
        let extent = normalize_extent(extent);
        self.estimated_entry_extent = Rc::new(move |_| extent);
        self
    }

    /// Sets the estimated logical-pixel stride policy for each virtualized row.
    ///
    /// This is the offscreen estimate. Realized rows feed their measured height
    /// back into the prefix-sum model after layout. The estimate is still
    /// important for initial tail anchoring and for entries that have not been
    /// materialized yet.
    #[must_use]
    pub fn estimated_entry_extent<F>(mut self, extent: F) -> Self
    where
        F: for<'entry> Fn(TranscriptEntryContext<'entry, P>) -> f64 + 'static,
    {
        self.estimated_entry_extent = Rc::new(extent);
        self
    }

    /// Sets the logical-pixel overscan before and after the viewport.
    #[must_use]
    pub const fn overscan(mut self, overscan: f64) -> Self {
        self.overscan = overscan;
        self
    }

    /// Sets the fixed logical-pixel size of the scroll viewport.
    #[must_use]
    pub const fn size(mut self, width: f64, height: f64) -> Self {
        self.size = Some((width, height));
        self
    }

    /// Sets the initial vertical scroll offset.
    #[must_use]
    pub const fn scroll_offset_y(mut self, offset: f64) -> Self {
        self.scroll_offset_y = offset;
        self.tail_anchored = false;
        self
    }

    /// Sets whether the initial viewport should pin itself to the transcript tail.
    #[must_use]
    pub const fn tail_anchored(mut self, anchored: bool) -> Self {
        self.tail_anchored = anchored;
        self
    }
}

impl<P: 'static> AppendSpec for TranscriptView<P> {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let (width, height) = self.size.unwrap_or((DEFAULT_WIDTH, DEFAULT_HEIGHT));
        let frame_width = width + DEFAULT_SCROLLBAR_GAP + DEFAULT_SCROLLBAR_THICKNESS;
        let frame = ui.append(
            parent,
            Panel::row(DEFAULT_SCROLLBAR_GAP).layout(
                Layout::row(DEFAULT_SCROLLBAR_GAP)
                    .logical_size(frame_width, height)
                    .align_center(),
            ),
        );

        let scroll_state = ScrollState::with_offset_y(self.scroll_offset_y);
        let viewport = ui.append_widget(
            frame,
            Box::new(TranscriptViewportWidget::new(
                self.session,
                self.entry_template,
                self.estimated_entry_extent,
                scroll_state.clone(),
                self.row_gap,
                self.overscan,
                self.tail_anchored,
            )),
        );
        ui.set_layout(viewport, Layout::new().logical_size(width, height));
        ui.register_scroll_target(viewport, scroll_state);
        self.element.apply_to(ui, viewport);

        let scrollbar = ui.append(
            frame,
            ScrollBar::vertical(viewport)
                .length(height)
                .thickness(DEFAULT_SCROLLBAR_THICKNESS)
                .thumb_extent(DEFAULT_SCROLLBAR_THUMB_EXTENT),
        );
        ui.set_layout(
            scrollbar,
            Layout::new().logical_size(DEFAULT_SCROLLBAR_THICKNESS, height),
        );

        frame
    }
}

impl<P: 'static> ElementSpec for TranscriptView<P> {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

/// Returns the transcript viewport element inside a [`TranscriptView`] frame.
#[must_use]
pub fn transcript_view_viewport(ui: &Ui, frame: ElementId) -> Option<ElementId> {
    ui.get(frame)
        .and_then(|element| element.children().first().copied())
}

/// Returns the reserved vertical scrollbar inside a [`TranscriptView`] frame.
#[must_use]
pub fn transcript_view_scrollbar(ui: &Ui, frame: ElementId) -> Option<ElementId> {
    ui.get(frame)
        .and_then(|element| element.children().get(1).copied())
}

/// Requests realization/layout refresh for a [`TranscriptView`] frame.
///
/// Call this after mutating a [`TranscriptSession`] that backs an already
/// appended view. The viewport compares the session revision to its cached
/// revision and only rebuilds realized rows when content actually changed.
pub fn refresh_transcript_view(ui: &mut Ui, frame: ElementId) -> bool {
    let Some(viewport) = transcript_view_viewport(ui, frame) else {
        return false;
    };
    ui.mark_channels(viewport, overstory::channels::LAYOUT.into());
    true
}

#[derive(Clone, Copy, Debug)]
struct RealizedTranscriptRow {
    index: usize,
    element: ElementId,
}

struct TranscriptViewportWidget<P = EntryBody> {
    session: TranscriptSession<P>,
    entry_template: EntryTemplate<P>,
    estimated_entry_extent: Rc<EstimatedEntryExtentFn<P>>,
    scroll_state: ScrollState,
    virtual_list: TranscriptVirtualList,
    row_gap: f64,
    overscan: f64,
    tail_anchored: bool,
    did_initial_tail_anchor: bool,
    realized: Vec<RealizedTranscriptRow>,
    last_viewport: Size,
    last_revision: u64,
}

impl<P> core::fmt::Debug for TranscriptViewportWidget<P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TranscriptViewportWidget")
            .field("session", &self.session)
            .field("estimated_entry_extent", &"<estimated entry extent policy>")
            .field("overscan", &self.overscan)
            .field("tail_anchored", &self.tail_anchored)
            .field("realized", &self.realized)
            .field("last_viewport", &self.last_viewport)
            .field("last_revision", &self.last_revision)
            .finish_non_exhaustive()
    }
}

impl<P> TranscriptViewportWidget<P> {
    fn new(
        session: TranscriptSession<P>,
        entry_template: EntryTemplate<P>,
        estimated_entry_extent: Rc<EstimatedEntryExtentFn<P>>,
        scroll_state: ScrollState,
        row_gap: f64,
        overscan: f64,
        tail_anchored: bool,
    ) -> Self {
        let model = PrefixSumExtentModel::new();
        let virtual_list = VirtualList::new(
            TailAnchoredExtentModel::with_default_epsilon(model),
            0.0,
            normalize_scroll_extent(overscan),
        );
        Self {
            session,
            entry_template,
            estimated_entry_extent,
            scroll_state,
            virtual_list,
            row_gap: row_gap.max(0.0),
            overscan: normalize_scroll_extent(overscan),
            tail_anchored,
            did_initial_tail_anchor: false,
            realized: Vec::new(),
            last_viewport: Size::ZERO,
            last_revision: u64::MAX,
        }
    }
}

impl<P: 'static> Widget for TranscriptViewportWidget<P> {
    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::Log)
    }

    fn configure_accessibility(
        &self,
        _element: &Element,
        _ctx: &AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        let offset = self.scroll_state.offset();
        let max = self.scroll_state.max_offset();
        node.set_scroll_y(offset.y);
        node.set_scroll_y_min(0.0);
        node.set_scroll_y_max(max.y);
        node.set_size_of_set(self.session.len());
    }

    fn clips_subtree(&self, _element: &Element) -> bool {
        true
    }

    fn child_layout_offset(&self, _element: &Element) -> Vec2 {
        -self.scroll_state.offset()
    }

    fn realize_elements(&mut self, ctx: &mut ElementRealizeContext<'_>) -> ElementRealizeResult {
        let viewport = ctx.id();
        ElementRealizeResult::from_changed(self.sync_rows(viewport, ctx.ui_mut()))
    }

    fn arrange(&mut self, ctx: &mut ArrangeContext<'_>) -> ArrangeResult {
        ArrangeResult::from_layout_changed(self.sync_measured_extents(ctx.ui()))
    }

    fn handle_scroll(&mut self, ctx: &mut WidgetEventContext<'_>, event: ScrollEvent<'_>) {
        if !matches!(event.phase(), Phase::Target | Phase::Bubble) || !ctx.is_enabled(ctx.id()) {
            return;
        }
        let bounds = ctx.effective_bounds(ctx.id());
        let page = Vec2::new(bounds.width(), bounds.height());
        let raw = event.logical_delta(Vec2::new(SCROLL_LINE, SCROLL_LINE), page);
        let delta_y = -raw.y;
        if delta_y == 0.0 {
            return;
        }
        self.set_scroll_offset_y(ctx.id(), ctx, self.scroll_state.offset_y() + delta_y);
        event.mark_handled();
    }
}

impl<P: 'static> TranscriptViewportWidget<P> {
    fn sync_rows(&mut self, viewport: ElementId, ui: &mut Ui) -> bool {
        let sync = self.sync_virtual_metrics(viewport, ui);
        let mut changed = sync.changed;
        if sync.revision_changed {
            changed |= self.clear_realized(ui);
        }
        changed |= self.reconcile_realized(viewport, ui, sync.range);
        changed |= self.layout_realized(ui, sync.viewport);
        changed
    }

    fn sync_virtual_metrics(
        &mut self,
        viewport: ElementId,
        ui: &mut Ui,
    ) -> TranscriptVirtualRowsSync {
        let bounds = ui.effective_bounds(viewport);
        let viewport_size = Size::new(bounds.width().max(0.0), bounds.height().max(0.0));
        let viewport_changed = self.last_viewport != viewport_size;
        self.last_viewport = viewport_size;

        self.virtual_list
            .set_scroll_offset(self.scroll_state.offset_y());
        self.virtual_list.set_viewport_extent(viewport_size.height);
        self.virtual_list.set_overscan(self.overscan, self.overscan);
        let was_at_tail = self.tail_anchored && self.virtual_list.is_at_tail();

        let len = self.session.len();
        let revision = self.session.revision();
        let revision_changed = self.last_revision != revision;
        let len_changed = self.virtual_list.len() != len;
        let estimated_extents_changed =
            self.sync_estimated_extents(len, revision_changed, len_changed);
        self.last_revision = revision;

        if self.tail_anchored && !self.did_initial_tail_anchor {
            self.virtual_list.scroll_to_tail();
            self.did_initial_tail_anchor = true;
        } else {
            self.virtual_list.restore_tail_anchor(was_at_tail);
        }
        self.virtual_list.clamp_scroll_to_content();

        let offset_changed = self
            .scroll_state
            .set_offset_y_raw(self.virtual_list.scroll_offset());
        let content_height = self.virtual_list.total_extent();
        self.scroll_state.set_metrics(
            viewport_size,
            Size::new(viewport_size.width, content_height),
        );

        if offset_changed {
            ui.mark_channels(viewport, overstory::channels::LAYOUT.into());
        }

        TranscriptVirtualRowsSync {
            viewport: viewport_size,
            range: self.virtual_list.materialized_range(),
            changed: viewport_changed || len_changed || estimated_extents_changed || offset_changed,
            revision_changed,
        }
    }

    fn sync_measured_extents(&mut self, ui: &Ui) -> bool {
        let mut updates = Vec::new();
        for row in &self.realized {
            let bounds = ui.effective_bounds(row.element);
            if bounds.height() <= 0.0 {
                continue;
            }
            let measured = normalize_extent(bounds.height() + self.row_gap);
            if (self.virtual_list.extent_of(row.index) - measured).abs() > f64::EPSILON {
                updates.push((row.index, measured));
            }
        }
        if updates.is_empty() {
            return false;
        }
        let was_at_tail = self.tail_anchored && self.virtual_list.is_at_tail();
        let model = self.virtual_list.model_mut().inner_mut();
        for (index, extent) in updates {
            model.set_extent(index, extent);
        }
        self.virtual_list.restore_tail_anchor(was_at_tail);
        self.virtual_list.clamp_scroll_to_content();
        self.scroll_state
            .set_offset_y_raw(self.virtual_list.scroll_offset());
        self.scroll_state.set_metrics(
            self.last_viewport,
            Size::new(self.last_viewport.width, self.virtual_list.total_extent()),
        );
        true
    }

    fn sync_estimated_extents(
        &mut self,
        len: usize,
        revision_changed: bool,
        len_changed: bool,
    ) -> bool {
        if !revision_changed && !len_changed {
            return false;
        }

        let estimate = Rc::clone(&self.estimated_entry_extent);
        let extents = self.session.with_transcript(|transcript| {
            transcript
                .entries()
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    let children = transcript.children_of(entry.id);
                    normalize_extent(estimate(TranscriptEntryContext::new(
                        index, entry, children,
                    )))
                })
                .collect::<Vec<_>>()
        });

        let mut changed = len_changed;
        if len_changed {
            self.virtual_list.set_len(len);
        }
        for (index, extent) in extents.into_iter().enumerate() {
            if (self.virtual_list.extent_of(index) - extent).abs() <= f64::EPSILON {
                continue;
            }
            self.virtual_list
                .model_mut()
                .inner_mut()
                .set_extent(index, extent);
            changed = true;
        }
        changed
    }

    fn clear_realized(&mut self, ui: &mut Ui) -> bool {
        let mut changed = false;
        for row in self.realized.drain(..) {
            if ui.get(row.element).is_some() {
                ui.remove(row.element);
                changed = true;
            }
        }
        changed
    }

    fn reconcile_realized(
        &mut self,
        viewport: ElementId,
        ui: &mut Ui,
        range: Range<usize>,
    ) -> bool {
        let mut changed = false;
        let mut kept = Vec::with_capacity(self.realized.len());
        for row in self.realized.drain(..) {
            if range.contains(&row.index) && ui.get(row.element).is_some() {
                kept.push(row);
            } else if ui.get(row.element).is_some() {
                ui.remove(row.element);
                changed = true;
            }
        }
        self.realized = kept;

        self.session.with_transcript(|transcript| {
            for index in range {
                if self.realized.iter().any(|row| row.index == index) {
                    continue;
                }
                let Some(entry) = transcript.entries().get(index) else {
                    continue;
                };
                let children = transcript.children_of(entry.id);
                let element = append_realized_transcript_row(
                    ui,
                    viewport,
                    index,
                    entry,
                    children,
                    &self.entry_template,
                );
                self.realized.push(RealizedTranscriptRow { index, element });
                changed = true;
            }
        });

        self.realized.sort_by_key(|row| row.index);
        changed
    }

    fn layout_realized(&mut self, ui: &mut Ui, _viewport: Size) -> bool {
        let mut changed = false;
        for row in &self.realized {
            let y = self.virtual_list.offset_of(row.index);
            let layout = Layout::new()
                .absolute_at(0.0, y)
                .fill_width()
                .min_logical_size(0.0, 0.0);
            ui.set_layout(row.element, layout);
            changed = true;
        }
        changed
    }

    fn set_scroll_offset_y(&mut self, viewport: ElementId, ui: &mut Ui, offset: f64) {
        self.virtual_list
            .set_scroll_offset(normalize_scroll_extent(offset));
        self.virtual_list.clamp_scroll_to_content();
        if self
            .scroll_state
            .set_offset_y_raw(self.virtual_list.scroll_offset())
        {
            ui.mark_channels(viewport, overstory::channels::LAYOUT.into());
        }
    }
}

#[derive(Clone, Debug)]
struct TranscriptVirtualRowsSync {
    viewport: Size,
    range: Range<usize>,
    changed: bool,
    revision_changed: bool,
}

fn append_realized_transcript_row<P>(
    ui: &mut Ui,
    viewport: ElementId,
    index: usize,
    entry: &TranscriptEntry<P>,
    children: &[EntryId],
    entry_template: &EntryTemplate<P>,
) -> ElementId {
    let row = ui.append(
        viewport,
        Panel::new()
            .padding_xy(2.0, 0.0)
            .layout(Layout::column(0.0).fill_width().min_logical_size(0.0, 0.0)),
    );
    ui.with_children(row, |row| {
        entry_template.render(row, TranscriptEntryContext::new(index, entry, children));
    });
    row
}

fn normalize_extent(extent: f64) -> f64 {
    normalize_scroll_extent(extent).max(1.0)
}

fn normalize_scroll_extent(extent: f64) -> f64 {
    if extent.is_finite() {
        extent.max(0.0)
    } else {
        0.0
    }
}

fn default_entry_template(row: &mut ChildUi<'_>, context: TranscriptEntryContext<'_, EntryBody>) {
    let (background, border) = default_row_colors(context.entry());
    row.with_child(
        Panel::column(6.0)
            .padding_all(12.0)
            .layout(
                Layout::column(6.0).size(
                    LayoutSize::new()
                        .width(LayoutDimension::Percent(1.0))
                        .min_width(LayoutDimension::Logical(0.0)),
                ),
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

    use overstory::{Panel, ProgressBar, Size, Text, scroll_offset_y};

    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Payload {
        Card(&'static str),
    }

    #[test]
    fn template_realizes_one_row_per_entry() {
        let mut transcript: Transcript<EntryBody> = Transcript::new();
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
        ui.compute_layout(Size::new(760.0, 440.0));

        let viewport = transcript_view_viewport(&ui, view).unwrap();
        assert_eq!(ui.get(viewport).unwrap().children().len(), 2);
        assert!(transcript_view_scrollbar(&ui, view).is_some());
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
        ui.compute_layout(Size::new(760.0, 440.0));

        let viewport = transcript_view_viewport(&ui, view).unwrap();
        let row = ui.get(viewport).unwrap().children()[0];
        let card = ui.get(row).unwrap().children()[0];
        assert_eq!(ui.get(card).unwrap().children().len(), 2);
    }

    #[test]
    fn session_mutations_advance_revision() {
        let session: TranscriptSession<EntryBody> = TranscriptSession::new();
        assert_eq!(session.revision(), 0);

        let entry = session.append(NewEntry::message(MessageRole::Assistant, EntryBody::Empty));
        assert_eq!(session.revision(), 1);

        assert!(session.append_chunk(entry, "hello").unwrap());
        assert_eq!(session.revision(), 2);

        assert!(!session.append_chunk(entry, EntryBody::Empty).unwrap());
        assert_eq!(session.revision(), 2);

        assert!(session.set_status(entry, EntryStatus::InProgress).unwrap());
        assert_eq!(session.revision(), 3);
        assert!(!session.set_status(entry, EntryStatus::InProgress).unwrap());
        assert_eq!(session.revision(), 3);
    }

    #[test]
    fn refresh_transcript_view_materializes_appended_tail_rows() {
        let session: TranscriptSession<EntryBody> = TranscriptSession::new();
        session.append(NewEntry::message(MessageRole::User, "first"));

        let mut ui = Ui::new();
        let root = ui.root();
        let view = ui.append(
            root,
            TranscriptView::with_session(
                session.clone(),
                EntryTemplate::new(|row, entry| {
                    row.add(Text::new(format!("entry {}", entry.index())));
                }),
            )
            .size(240.0, 160.0)
            .estimated_row_extent(40.0),
        );
        ui.compute_layout(Size::new(320.0, 220.0));

        let viewport = transcript_view_viewport(&ui, view).unwrap();
        assert_eq!(ui.get(viewport).unwrap().children().len(), 1);

        session.append(NewEntry::message(MessageRole::Assistant, "second"));
        assert!(refresh_transcript_view(&mut ui, view));
        ui.compute_layout(Size::new(320.0, 220.0));

        assert_eq!(ui.get(viewport).unwrap().children().len(), 2);
    }

    #[test]
    fn measured_rich_rows_feed_back_into_virtual_offsets() {
        let mut transcript: Transcript<EntryBody> = Transcript::new();
        transcript.append(NewEntry::message(MessageRole::Assistant, EntryBody::Empty));
        transcript.append(NewEntry::message(MessageRole::Assistant, EntryBody::Empty));

        let mut ui = Ui::new();
        let root = ui.root();
        let view = ui.append(
            root,
            TranscriptView::with_entry_template(
                transcript,
                EntryTemplate::new(|row, _entry| {
                    row.add(Panel::new().layout(Layout::new().logical_size(200.0, 148.0)));
                }),
            )
            .size(240.0, 220.0)
            .estimated_row_extent(24.0)
            .overscan(400.0),
        );

        ui.compute_layout(Size::new(320.0, 280.0));

        let viewport = transcript_view_viewport(&ui, view).unwrap();
        let children = ui.get(viewport).unwrap().children();
        assert_eq!(children.len(), 2);

        let first = ui.effective_bounds(children[0]);
        let second = ui.effective_bounds(children[1]);
        assert!(
            second.y0 >= first.y1 + DEFAULT_ROW_GAP,
            "second row should be laid out after the measured first row, got first={first:?}, second={second:?}",
        );
    }

    #[test]
    fn measured_rows_restore_tail_anchor_after_short_estimates() {
        let mut transcript: Transcript<EntryBody> = Transcript::new();
        for _ in 0..6 {
            transcript.append(NewEntry::message(MessageRole::Assistant, EntryBody::Empty));
        }

        let mut ui = Ui::new();
        let root = ui.root();
        let view = ui.append(
            root,
            TranscriptView::with_entry_template(
                transcript,
                EntryTemplate::new(|row, _entry| {
                    row.add(Panel::new().layout(Layout::new().logical_size(200.0, 50.0)));
                }),
            )
            .size(240.0, 80.0)
            .row_gap(0.0)
            .estimated_row_extent(8.0)
            .overscan(600.0),
        );

        ui.compute_layout(Size::new(320.0, 160.0));

        let viewport = transcript_view_viewport(&ui, view).unwrap();
        let offset = scroll_offset_y(&ui, viewport).expect("transcript viewport should scroll");
        assert!(
            offset > 100.0,
            "measured rows should keep the initially anchored viewport pinned to the tail, got {offset}",
        );
    }
}
