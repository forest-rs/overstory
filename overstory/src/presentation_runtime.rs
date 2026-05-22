// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use hashbrown::HashMap;
use kurbo::{Affine, Point, Rect, RoundedRect};
use smallvec::SmallVec;
use understory_box_tree::{Hit, LocalNode, NodeFlags, NodeId, QueryFilter, Tree};
use understory_presentation::{PresentationNode, PresentationStore};

use crate::element::ElementId;

/// Box-tree geometry, hit testing, and per-node presentation data.
pub(crate) struct PresentationRuntime {
    presentation: Option<PresentationStore<NodeId, ElementId>>,
    tree: Tree,
    element_to_nodes: HashMap<ElementId, SmallVec<[NodeId; 1]>>,
    node_to_element: HashMap<NodeId, ElementId>,
}

impl PresentationRuntime {
    pub(crate) fn new() -> Self {
        Self {
            presentation: Some(PresentationStore::new()),
            tree: Tree::new(),
            element_to_nodes: HashMap::new(),
            node_to_element: HashMap::new(),
        }
    }

    pub(crate) fn insert_element(&mut self, id: ElementId, parent: Option<ElementId>) {
        let parent_node = parent
            .and_then(|p| self.element_to_nodes.get(&p))
            .and_then(|nodes| nodes.first())
            .copied();
        let tree_node = self.tree.insert(parent_node, LocalNode::default());
        let mut nodes = SmallVec::<[NodeId; 1]>::new();
        nodes.push(tree_node);
        self.element_to_nodes.insert(id, nodes);
        self.node_to_element.insert(tree_node, id);
        let _ = self.store_mut().insert(tree_node, id);
    }

    pub(crate) fn remove_element(&mut self, id: ElementId) {
        if let Some(nodes) = self.element_to_nodes.remove(&id) {
            for node in nodes {
                let _ = self.store_mut().remove(node);
                self.node_to_element.remove(&node);
                self.tree.remove(node);
            }
        }
    }

    pub(crate) fn box_for(&self, id: ElementId) -> Option<NodeId> {
        self.element_to_nodes
            .get(&id)
            .and_then(|nodes| nodes.first())
            .copied()
    }

    pub(crate) fn node_to_element(&self, node: NodeId) -> Option<ElementId> {
        self.node_to_element.get(&node).copied()
    }

    pub(crate) fn hit_test(&self, point: Point) -> Option<Hit> {
        self.tree.hit_test_point(point, QueryFilter::new())
    }

    pub(crate) fn commit(&mut self) {
        self.tree.commit();
    }

    pub(crate) fn flags(&self, node: NodeId) -> Option<NodeFlags> {
        self.tree.flags(node)
    }

    pub(crate) fn set_flags(&mut self, node: NodeId, flags: NodeFlags) {
        self.tree.set_flags(node, flags);
    }

    pub(crate) fn set_local_bounds(&mut self, node: NodeId, bounds: Rect) {
        self.tree.set_local_bounds(node, bounds);
    }

    pub(crate) fn set_local_clip(&mut self, node: NodeId, clip: Option<RoundedRect>) {
        self.tree.set_local_clip(node, clip);
    }

    pub(crate) fn local_clip(&self, node: NodeId) -> Option<Option<RoundedRect>> {
        self.tree.local_clip(node)
    }

    pub(crate) fn world_transform(&self, node: NodeId) -> Option<Affine> {
        self.tree.world_transform(node)
    }

    pub(crate) fn world_bounds(&self, node: NodeId) -> Option<Rect> {
        self.tree.world_bounds(node)
    }

    pub(crate) fn children_of(&self, node: NodeId) -> &[NodeId] {
        self.tree.children_of(node)
    }

    #[cfg(test)]
    pub(crate) fn parent_of(&self, node: NodeId) -> Option<NodeId> {
        self.tree.parent_of(node)
    }

    pub(crate) fn tree(&self) -> &Tree {
        &self.tree
    }

    pub(crate) fn take_presentation(&mut self) -> PresentationStore<NodeId, ElementId> {
        self.presentation
            .take()
            .expect("presentation always Some outside present_element")
    }

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

    #[cfg(test)]
    pub(crate) fn nodes_for(&self, id: ElementId) -> Option<&[NodeId]> {
        self.element_to_nodes.get(&id).map(SmallVec::as_slice)
    }
}
