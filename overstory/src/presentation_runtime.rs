// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained presentation primitives keyed by geometry nodes.
//!
//! Roadmap:
//! - Keep presentation as renderer-neutral drawing intent, not a scene renderer
//!   and not a widget tree.
//! - Add keyed prepared resources for shaped text, picture primitives, images,
//!   and borrowed scenes so layout and paint can share expensive work.
//! - Explore a fuller presentation tree when sharing vectors, unboxed primitive
//!   storage, or cross-node reuse would make lowering cheaper and clearer.
//! - Preserve dirty-node tracking so composition and hosts can update only the
//!   surfaces whose presentation changed.

use alloc::vec::Vec;

use understory_box_tree::NodeId;
use understory_presentation::{PresentationNode, PresentationStore};

use crate::element::ElementId;

/// Retained presentation primitives keyed by box-tree node identity.
pub(crate) struct PresentationRuntime {
    /// Presentation store for the current paint pass.
    ///
    /// Temporarily `None` while paint code takes ownership to rebuild a node
    /// without fighting the surrounding `&mut Ui` borrow.
    presentation: Option<PresentationStore<NodeId, ElementId>>,
}

impl PresentationRuntime {
    pub(crate) fn new() -> Self {
        Self {
            presentation: Some(PresentationStore::new()),
        }
    }

    pub(crate) fn insert_node(&mut self, node: NodeId, id: ElementId) {
        let _ = self.store_mut().insert(node, id);
    }

    pub(crate) fn remove_node(&mut self, node: NodeId) {
        let _ = self.store_mut().remove(node);
    }

    pub(crate) fn take_presentation(&mut self) -> PresentationStore<NodeId, ElementId> {
        self.presentation
            .take()
            .expect("presentation always Some outside present_element")
    }

    /// Restores the presentation store after a temporary rebuild borrow.
    pub(crate) fn restore_presentation(
        &mut self,
        presentation: PresentationStore<NodeId, ElementId>,
    ) {
        self.presentation = Some(presentation);
    }

    pub(crate) fn drain_dirty(&mut self) -> Vec<NodeId> {
        self.store_mut().take_dirty().collect()
    }

    pub(crate) fn presentation(&self, node: NodeId) -> Option<&PresentationNode<ElementId>> {
        self.presentation.as_ref()?.node(node)
    }

    fn store_mut(&mut self) -> &mut PresentationStore<NodeId, ElementId> {
        self.presentation
            .as_mut()
            .expect("presentation always Some outside present_element")
    }
}
