// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Box-tree geometry and spatial queries for retained elements.
//!
//! Roadmap:
//! - Keep `understory_box_tree` as the geometry, clip, spatial index, hit-test,
//!   and focus-flag substrate; Overstory maps that back to `ElementId`.
//! - Support multiple boxes or fragments per element through the side map when
//!   text, inline content, or complex widgets need it.
//! - Coordinate transforms, clipping, and hit regions with composition so
//!   compositor-backed layers and software presentation agree.
//! - Add query and invalidation diagnostics for hit testing, overlay placement,
//!   focus navigation, and virtualized controls.

use alloc::vec::Vec;

use hashbrown::HashMap;
use kurbo::{Point, Rect, RoundedRect};
use smallvec::SmallVec;
use understory_box_tree::{Hit, LocalNode, NodeFlags, NodeId, QueryFilter, Tree};

use crate::element::ElementId;
use crate::element_side_map::ElementSideMap;

/// Box-tree geometry, spatial indexing, hit testing, and focus flags.
pub(crate) struct GeometryRuntime {
    /// Understory box tree that owns committed local/world geometry.
    tree: Tree,
    /// Mapping from retained elements to their primary box-tree nodes.
    ///
    /// Stored as a small vector so future widgets can own multiple boxes
    /// without changing the external lookup shape.
    element_to_nodes: ElementSideMap<SmallVec<[NodeId; 1]>>,
    /// Reverse lookup used to turn spatial hits and focus results back into
    /// retained element ids.
    node_to_element: HashMap<NodeId, ElementId>,
}

impl GeometryRuntime {
    pub(crate) fn new() -> Self {
        Self {
            tree: Tree::new(),
            element_to_nodes: ElementSideMap::new(),
            node_to_element: HashMap::new(),
        }
    }

    pub(crate) fn insert_element(&mut self, id: ElementId, parent: Option<ElementId>) -> NodeId {
        // The first node is the element's primary layout/hit/focus box.
        let parent_node = parent
            .and_then(|p| self.element_to_nodes.get(p))
            .and_then(|nodes| nodes.first())
            .copied();
        let tree_node = self.tree.insert(parent_node, LocalNode::default());
        let mut nodes = SmallVec::<[NodeId; 1]>::new();
        nodes.push(tree_node);
        self.element_to_nodes.insert(id, nodes);
        self.node_to_element.insert(tree_node, id);
        tree_node
    }

    pub(crate) fn remove_element(&mut self, id: ElementId) -> Vec<NodeId> {
        let Some(nodes) = self.element_to_nodes.remove(id) else {
            return Vec::new();
        };
        let mut removed = Vec::with_capacity(nodes.len());
        for node in nodes {
            self.node_to_element.remove(&node);
            self.tree.remove(node);
            removed.push(node);
        }
        removed
    }

    pub(crate) fn box_for(&self, id: ElementId) -> Option<NodeId> {
        self.element_to_nodes
            .get(id)
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

    pub(crate) fn set_z_index(&mut self, id: ElementId, z_index: i32) {
        if let Some(nodes) = self.element_to_nodes.get(id) {
            for node in nodes {
                self.tree.set_z_index(*node, z_index);
            }
        }
    }

    pub(crate) fn set_local_bounds(&mut self, node: NodeId, bounds: Rect) {
        self.tree.set_local_bounds(node, bounds);
    }

    pub(crate) fn set_local_clip(&mut self, node: NodeId, clip: Option<RoundedRect>) {
        self.tree.set_local_clip(node, clip);
    }

    #[cfg(test)]
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

    #[cfg(test)]
    pub(crate) fn nodes_for(&self, id: ElementId) -> Option<&[NodeId]> {
        self.element_to_nodes.get(id).map(SmallVec::as_slice)
    }
}
