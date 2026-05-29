// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Same-process devtools session state.

use core::cell::RefCell;

use alloc::rc::Rc;
#[cfg(feature = "spoor-session")]
use alloc::vec::Vec;
use overstory::{ElementId, OutlineNodeKey};
#[cfg(feature = "spoor-session")]
use spoor_session::{OwnedSessionEnvelope, OwnedSessionPayload};

use crate::{DevtoolsSnapshot, DevtoolsTarget};

/// Same-process devtools session state.
///
/// This type owns Overstory-specific snapshot and selection state; it
/// explicitly does not own host event loops, rendering, network transport, or
/// Spoor's generic telemetry vocabulary.
#[derive(Clone, Debug, PartialEq)]
pub struct DevtoolsSession {
    snapshot: DevtoolsSnapshot,
    selected_element: Option<ElementId>,
    #[cfg(feature = "spoor-session")]
    spoor: DevtoolsSpoorSession,
}

impl DevtoolsSession {
    /// Creates a session for `snapshot` with no explicit selected element.
    #[must_use]
    pub fn new(snapshot: DevtoolsSnapshot) -> Self {
        Self {
            snapshot,
            selected_element: None,
            #[cfg(feature = "spoor-session")]
            spoor: DevtoolsSpoorSession::default(),
        }
    }

    /// Captures a snapshot from `target` and starts a session around it.
    #[must_use]
    pub fn capture(target: &impl DevtoolsTarget) -> Self {
        Self::new(DevtoolsSnapshot::capture(target))
    }

    /// Returns a copy of this session with `selected` as the selected element.
    #[must_use]
    pub fn with_selected_element(mut self, selected: Option<ElementId>) -> Self {
        self.set_selected_element(selected);
        self
    }

    /// Returns the currently attached inspection snapshot.
    #[must_use]
    pub const fn snapshot(&self) -> &DevtoolsSnapshot {
        &self.snapshot
    }

    /// Returns the currently selected element, if one is selected.
    #[must_use]
    pub const fn selected_element(&self) -> Option<ElementId> {
        self.selected_element
    }

    /// Returns the outline key for the currently selected element, if one is selected.
    #[must_use]
    pub fn selected_outline_key(&self) -> Option<OutlineNodeKey> {
        self.selected_element
            .and_then(|id| self.snapshot.outline_key_for_element(id))
    }

    /// Replaces the current selected element.
    ///
    /// Stale element ids are normalized to `None` because a session selection
    /// must be meaningful against its current snapshot.
    pub fn set_selected_element(&mut self, selected: Option<ElementId>) -> Option<DevtoolsEvent> {
        let selected = selected.filter(|id| self.snapshot.element(*id).is_some());
        if self.selected_element == selected {
            return None;
        }
        self.selected_element = selected;
        Some(DevtoolsEvent::SelectionChanged { selected })
    }

    /// Selects the element represented by `key`.
    pub fn select_outline_key(&mut self, key: Option<OutlineNodeKey>) -> Option<DevtoolsEvent> {
        let selected = key.and_then(|key| self.snapshot.element_for_outline_key(key));
        self.set_selected_element(selected)
    }

    /// Replaces the current inspection snapshot.
    ///
    /// If the selected element is not present in the new snapshot, selection is
    /// cleared.
    pub fn replace_snapshot(&mut self, snapshot: DevtoolsSnapshot) -> DevtoolsEvent {
        self.snapshot = snapshot;
        if self
            .selected_element
            .is_some_and(|id| self.snapshot.element(id).is_none())
        {
            self.selected_element = None;
        }
        DevtoolsEvent::SnapshotReplaced {
            selected: self.selected_element,
        }
    }

    /// Replaces the attached Spoor session envelopes.
    #[cfg(feature = "spoor-session")]
    pub fn replace_spoor_envelopes(&mut self, envelopes: Vec<OwnedSessionEnvelope>) {
        self.spoor = DevtoolsSpoorSession::new(envelopes);
    }

    /// Returns the attached Spoor session envelope summary.
    #[cfg(feature = "spoor-session")]
    #[must_use]
    pub const fn spoor_summary(&self) -> DevtoolsSpoorSummary {
        self.spoor.summary
    }

    /// Returns the attached Spoor session envelopes.
    #[cfg(feature = "spoor-session")]
    #[must_use]
    pub fn spoor_envelopes(&self) -> &[OwnedSessionEnvelope] {
        &self.spoor.envelopes
    }
}

/// Shared handle for a same-process devtools session.
///
/// The handle is intentionally small: it lets inspector UI callbacks update
/// session state without making the target `Ui` borrowable through the devtools
/// view.
#[derive(Clone, Debug)]
pub struct DevtoolsSessionHandle {
    inner: Rc<RefCell<DevtoolsSession>>,
}

impl DevtoolsSessionHandle {
    /// Wraps `session` in a shareable handle.
    #[must_use]
    pub fn new(session: DevtoolsSession) -> Self {
        Self {
            inner: Rc::new(RefCell::new(session)),
        }
    }

    /// Reads the session through a short-lived callback.
    pub fn with<R>(&self, f: impl FnOnce(&DevtoolsSession) -> R) -> R {
        f(&self.inner.borrow())
    }

    /// Mutates the session through a short-lived callback.
    pub fn with_mut<R>(&self, f: impl FnOnce(&mut DevtoolsSession) -> R) -> R {
        f(&mut self.inner.borrow_mut())
    }
}

/// Event produced by a [`DevtoolsSession`] mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DevtoolsEvent {
    /// The selected element changed.
    SelectionChanged {
        /// Newly selected element, or `None` when selection was cleared.
        selected: Option<ElementId>,
    },
    /// The current snapshot changed.
    SnapshotReplaced {
        /// Selection after stale ids were normalized.
        selected: Option<ElementId>,
    },
}

/// Summary of attached Spoor session envelopes.
#[cfg(feature = "spoor-session")]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DevtoolsSpoorSummary {
    /// Number of envelopes attached to the session.
    pub envelope_count: usize,
    /// Latest observed Spoor session sequence.
    pub session_sequence: Option<u64>,
    /// Number of context rows in the attached envelope batch.
    pub context_count: usize,
    /// Number of counter rows in the attached envelope batch.
    pub counter_count: usize,
    /// Number of event rows in the attached envelope batch.
    pub event_count: usize,
    /// Context rows dropped by the producer.
    pub dropped_contexts: u64,
    /// Counter rows dropped by the producer.
    pub dropped_counters: u64,
    /// Event rows dropped by the producer.
    pub dropped_events: u64,
}

#[cfg(feature = "spoor-session")]
#[derive(Clone, Debug, Default, PartialEq)]
struct DevtoolsSpoorSession {
    envelopes: Vec<OwnedSessionEnvelope>,
    summary: DevtoolsSpoorSummary,
}

#[cfg(feature = "spoor-session")]
impl DevtoolsSpoorSession {
    fn new(envelopes: Vec<OwnedSessionEnvelope>) -> Self {
        let summary = summarize_spoor_envelopes(&envelopes);
        Self { envelopes, summary }
    }
}

#[cfg(feature = "spoor-session")]
fn summarize_spoor_envelopes(envelopes: &[OwnedSessionEnvelope]) -> DevtoolsSpoorSummary {
    let mut summary = DevtoolsSpoorSummary {
        envelope_count: envelopes.len(),
        ..DevtoolsSpoorSummary::default()
    };
    for envelope in envelopes {
        summary.session_sequence = Some(envelope.header.session_sequence);
        match &envelope.payload {
            OwnedSessionPayload::SnapshotStart(snapshot) => {
                summary.context_count =
                    summary.context_count.saturating_add(snapshot.context_count);
                summary.counter_count =
                    summary.counter_count.saturating_add(snapshot.counter_count);
                summary.event_count = summary.event_count.saturating_add(snapshot.event_count);
            }
            OwnedSessionPayload::Contexts(_)
            | OwnedSessionPayload::Counters(_)
            | OwnedSessionPayload::Events(_) => {}
            OwnedSessionPayload::SnapshotEnd(dropped) => {
                summary.dropped_contexts =
                    summary.dropped_contexts.saturating_add(dropped.contexts);
                summary.dropped_counters =
                    summary.dropped_counters.saturating_add(dropped.counters);
                summary.dropped_events = summary.dropped_events.saturating_add(dropped.events);
            }
        }
    }
    summary
}
