// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Overstory inspection payloads carried by Spoor live-session messages.
//!
//! Spoor owns request ids, stream ids, and generic response/stream envelopes.
//! This module owns the Overstory-specific payloads that talk about element
//! trees, property details, selection, and inspection deltas.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use overstory::ElementId;
use overstory::inspection::ElementInspectionRow;
use spoor_session::{
    SessionMethod, SessionRequest, SessionRequestId, SessionResponse, SessionStreamId,
    SessionStreamItem, SessionStreamStart,
};

use crate::{DevtoolsSnapshot, ElementOutlineRow, PropertyGrid};

/// Domain used for Overstory inspection live-session methods.
pub const INSPECTION_DOMAIN: &str = "overstory.inspection";

/// Builds a Spoor live-session method in the Overstory inspection domain.
#[must_use]
pub fn inspection_method(name: impl Into<String>) -> SessionMethod {
    SessionMethod::new(INSPECTION_DOMAIN, name)
}

/// Builds the Spoor stream-start row for an accepted inspection subscription.
#[must_use]
pub fn inspection_stream_start(
    request_id: Option<SessionRequestId>,
    stream_id: SessionStreamId,
    subscription: InspectionSubscription,
) -> LiveInspectionStreamStart {
    SessionStreamStart::new(
        request_id,
        stream_id,
        inspection_method("updates"),
        InspectionResponse::Subscribed {
            stream_id,
            subscription,
        },
    )
}

/// Spoor live-session request carrying an Overstory inspection payload.
pub type LiveInspectionRequest = SessionRequest<InspectionRequest>;

/// Spoor live-session response carrying an Overstory inspection payload.
pub type LiveInspectionResponse = SessionResponse<InspectionResponse>;

/// Spoor live-session stream-start row carrying an Overstory inspection payload.
pub type LiveInspectionStreamStart = SessionStreamStart<InspectionResponse>;

/// Spoor live-session stream item carrying an Overstory inspection payload.
pub type LiveInspectionStreamItem = SessionStreamItem<InspectionStreamItem>;

/// Request payloads understood by the Overstory inspector domain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InspectionRequest {
    /// Capture a full owned snapshot.
    ///
    /// This is useful for initial attachment and offline dumps. Live inspector
    /// refresh should prefer compact deltas plus lazy detail requests.
    CaptureSnapshot,
    /// Return the retained element tree as compact outline rows.
    ElementTree,
    /// Return details for one element.
    ElementDetails {
        /// Element to inspect.
        element: ElementId,
    },
    /// Return the currently selected element.
    Selection,
    /// Set the currently selected element.
    SetSelection {
        /// Element to select, or `None` to clear selection.
        selected: Option<ElementId>,
    },
    /// Subscribe to a live inspection stream.
    Subscribe(InspectionSubscription),
}

/// Response payloads produced by the Overstory inspector domain.
#[derive(Clone, Debug, PartialEq)]
pub enum InspectionResponse {
    /// Full owned snapshot response.
    Snapshot(Box<DevtoolsSnapshot>),
    /// Compact retained element outline response.
    ElementTree(Vec<ElementOutlineRow>),
    /// Details for one selected element.
    ElementDetails(Box<InspectionElementDetails>),
    /// Current selection response.
    Selection {
        /// Selected element, if any.
        selected: Option<ElementId>,
    },
    /// A stream was accepted by the inspector domain.
    Subscribed {
        /// Stream opened for this subscription.
        stream_id: SessionStreamId,
        /// Accepted subscription.
        subscription: InspectionSubscription,
    },
}

/// Element-specific details returned by a lazy inspection query.
#[derive(Clone, Debug, PartialEq)]
pub struct InspectionElementDetails {
    /// Snapshot row for the inspected element.
    pub row: ElementInspectionRow,
    /// Structured property grid for the inspected element.
    pub properties: PropertyGrid,
}

/// Live inspection stream item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InspectionStreamItem {
    /// The target changed since the last item.
    Delta(InspectionDelta),
}

/// Subscription request for an Overstory inspection stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InspectionSubscription {
    /// Kind of inspection updates requested.
    pub kind: InspectionSubscriptionKind,
}

impl InspectionSubscription {
    /// Subscribes to all inspection domains.
    #[must_use]
    pub const fn all() -> Self {
        Self {
            kind: InspectionSubscriptionKind::All,
        }
    }

    /// Subscribes to one inspection subsystem.
    #[must_use]
    pub const fn subsystem(subsystem: InspectionSubsystem) -> Self {
        Self {
            kind: InspectionSubscriptionKind::Subsystem(subsystem),
        }
    }
}

/// Scope of an inspection stream subscription.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectionSubscriptionKind {
    /// All inspection changes.
    All,
    /// Changes for one subsystem.
    Subsystem(InspectionSubsystem),
}

/// Compact live inspection delta.
///
/// This deliberately avoids carrying a full snapshot. Consumers should apply
/// the signal to their retained mirror and issue lazy queries for details they
/// actually display.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InspectionDelta {
    /// Subsystem that changed.
    pub subsystem: InspectionSubsystem,
    /// Reason the stream item was emitted.
    pub reason: InspectionUpdateReason,
}

impl InspectionDelta {
    /// Creates a compact inspection delta.
    #[must_use]
    pub const fn new(subsystem: InspectionSubsystem, reason: InspectionUpdateReason) -> Self {
        Self { subsystem, reason }
    }
}

/// Inspectable subsystem that can produce updates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectionSubsystem {
    /// Retained element tree topology or element identity.
    Elements,
    /// Layout bounds or layout-derived geometry.
    Layout,
    /// Style state, classes, pseudos, or resolved style outputs.
    Style,
    /// Composition layers, surfaces, clips, opacity, or transforms.
    Composition,
    /// Property binding graph or binding results.
    Bindings,
    /// Timeline tracks, clips, or sampled presentation values.
    Timeline,
    /// Frame counters and latency summaries.
    Frame,
}

/// Reason an inspection update was emitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectionUpdateReason {
    /// Initial stream state.
    Initial,
    /// A full snapshot was replaced.
    SnapshotReplaced,
    /// User or tool selection changed.
    SelectionChanged,
    /// Target state changed and lazy details may be stale.
    TargetChanged,
}

#[cfg(test)]
mod tests {
    use super::{
        INSPECTION_DOMAIN, InspectionDelta, InspectionRequest, InspectionResponse,
        InspectionSubscription, InspectionSubsystem, InspectionUpdateReason, LiveInspectionRequest,
        inspection_method, inspection_stream_start,
    };
    use spoor_session::{SessionRequestId, SessionStreamId};

    #[test]
    fn inspection_method_uses_overstory_domain() {
        let method = inspection_method("elementDetails");

        assert_eq!(method.domain, INSPECTION_DOMAIN);
        assert_eq!(method.name, "elementDetails");
    }

    #[test]
    fn live_inspection_request_preserves_spoor_request_id() {
        let request = LiveInspectionRequest::new(
            SessionRequestId::new(12),
            inspection_method("snapshot"),
            InspectionRequest::CaptureSnapshot,
        );

        assert_eq!(request.id, SessionRequestId::new(12));
        assert_eq!(request.payload, InspectionRequest::CaptureSnapshot);
    }

    #[test]
    fn inspection_delta_is_compact() {
        let delta = InspectionDelta::new(
            InspectionSubsystem::Layout,
            InspectionUpdateReason::TargetChanged,
        );

        assert_eq!(delta.subsystem, InspectionSubsystem::Layout);
        assert_eq!(delta.reason, InspectionUpdateReason::TargetChanged);
    }

    #[test]
    fn inspection_stream_start_uses_spoor_stream_correlation() {
        let request_id = SessionRequestId::new(4);
        let stream_id = SessionStreamId::new(8);
        let subscription = InspectionSubscription::all();

        let start = inspection_stream_start(Some(request_id), stream_id, subscription);

        assert_eq!(start.request_id, Some(request_id));
        assert_eq!(start.stream_id, stream_id);
        assert_eq!(start.method.domain, INSPECTION_DOMAIN);
        assert_eq!(start.method.name, "updates");
        assert_eq!(
            start.payload,
            InspectionResponse::Subscribed {
                stream_id,
                subscription,
            }
        );
    }
}
