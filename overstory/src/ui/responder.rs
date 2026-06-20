// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Adapter between Overstory's retained element/geometry state and
//! `understory_responder`.

use alloc::vec::Vec;

use kurbo::Point;
use understory_box_tree::{Hit as BoxHit, NodeId};
use understory_responder::router::Router;
use understory_responder::types::{
    DepthKey, Dispatch, Localizer, ParentLookup, ResolvedHitRef, WidgetLookup,
};

use crate::element::{Element, ElementId};

use super::Ui;

pub(in crate::ui) type UiDispatchStep = Dispatch<ElementId, ElementId, ()>;

/// Owned responder output for a selected pointer hit.
pub(in crate::ui) struct ResponderHit {
    /// Physical element selected by geometry hit testing.
    pub target: ElementId,
    /// Responder dispatch sequence for the selected hit path.
    pub dispatch: Vec<UiDispatchStep>,
}

/// Read-only adapter from `Ui` state to the responder router.
pub(in crate::ui) struct UiResponder<'a> {
    ui: &'a Ui,
}

impl<'a> UiResponder<'a> {
    pub(in crate::ui) const fn new(ui: &'a Ui) -> Self {
        Self { ui }
    }

    /// Builds a responder dispatch sequence by reconstructing `source`'s
    /// element ancestry.
    pub(in crate::ui) fn dispatch_for(&self, source: ElementId) -> Vec<UiDispatchStep> {
        self.router().dispatch_for::<()>(source)
    }

    /// Hit-tests `point` and builds a responder dispatch sequence from the
    /// selected box-tree hit path.
    ///
    /// The box tree currently returns the single topmost precise hit rather
    /// than every candidate under the pointer. This still gives the responder
    /// the selected path directly, avoiding a second ancestry reconstruction.
    /// A future box-tree candidate query can feed multiple `ResolvedHitRef`
    /// values here so responder owns final hit selection as well.
    pub(in crate::ui) fn hit_dispatch(&self, point: Point) -> Option<ResponderHit> {
        let hit = self.ui.geometry.hit_test(point)?;
        self.dispatch_for_box_hit(&hit)
    }

    /// Returns the topmost element hit at `point`.
    pub(in crate::ui) fn hit_test(&self, point: Point) -> Option<ElementId> {
        self.ui
            .geometry
            .hit_test(point)
            .and_then(|hit| self.ui.node_to_element(hit.node))
    }

    /// Returns the selected hit path from root to target in element id space.
    pub(in crate::ui) fn hit_path(&self, point: Point) -> Vec<ElementId> {
        self.ui
            .geometry
            .hit_test(point)
            .map(|hit| self.element_path(&hit.path))
            .unwrap_or_default()
    }

    fn dispatch_for_box_hit(&self, hit: &BoxHit) -> Option<ResponderHit> {
        let target = self.ui.node_to_element(hit.node)?;
        let path = self.element_path(&hit.path);
        let resolved = ResolvedHitRef {
            node: target,
            path: Some(&path),
            depth_key: DepthKey::Z(0),
            localizer: Localizer::default(),
            meta: (),
        };
        Some(ResponderHit {
            target,
            dispatch: self.router().handle_with_hits(&[resolved]),
        })
    }

    fn element_path(&self, nodes: &[NodeId]) -> Vec<ElementId> {
        nodes
            .iter()
            .filter_map(|node| self.ui.node_to_element(*node))
            .collect()
    }

    fn router(&self) -> Router<ElementId, UiWidgetLookup, UiParentLookup<'a>> {
        Router::with_parent(UiWidgetLookup, UiParentLookup { ui: self.ui })
    }
}

struct UiWidgetLookup;

impl WidgetLookup<ElementId> for UiWidgetLookup {
    type WidgetId = ElementId;

    fn widget_of(&self, node: &ElementId) -> Option<ElementId> {
        Some(*node)
    }
}

struct UiParentLookup<'a> {
    ui: &'a Ui,
}

impl ParentLookup<ElementId> for UiParentLookup<'_> {
    fn parent_of(&self, node: &ElementId) -> Option<ElementId> {
        self.ui.get(*node).and_then(Element::parent)
    }
}

impl Ui {
    pub(in crate::ui) const fn responder(&self) -> UiResponder<'_> {
        UiResponder::new(self)
    }
}
