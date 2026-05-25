// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use hashbrown::HashMap;

use crate::element::ElementId;

/// Taffy layout tree plus the element-to-layout-node map.
pub(crate) struct LayoutRuntime {
    /// Internal Taffy tree. Overstory keeps Taffy behind this runtime so most
    /// callers deal in `ElementId` rather than layout-node ids.
    taffy: taffy::TaffyTree<ElementId>,
    /// Mapping from retained elements to their Taffy layout nodes.
    element_to_taffy: HashMap<ElementId, taffy::NodeId>,
}

impl LayoutRuntime {
    pub(crate) fn new() -> Self {
        Self {
            taffy: taffy::TaffyTree::new(),
            element_to_taffy: HashMap::new(),
        }
    }

    pub(crate) fn insert_element(&mut self, id: ElementId, parent: Option<ElementId>) {
        let taffy_node = self
            .taffy
            .new_leaf_with_context(taffy::Style::default(), id)
            .expect("taffy leaf allocation should succeed");
        self.element_to_taffy.insert(id, taffy_node);
        if let Some(parent_id) = parent
            && let Some(parent_node) = self.element_to_taffy.get(&parent_id).copied()
        {
            self.taffy
                .add_child(parent_node, taffy_node)
                .expect("taffy add_child should succeed for fresh node");
        }
    }

    pub(crate) fn remove_element(&mut self, id: ElementId) {
        if let Some(taffy_node) = self.element_to_taffy.remove(&id) {
            let _ = self.taffy.remove(taffy_node);
        }
    }

    pub(crate) fn node_for(&self, id: ElementId) -> Option<taffy::NodeId> {
        self.element_to_taffy.get(&id).copied()
    }

    pub(crate) fn set_style(&mut self, node: taffy::NodeId, style: taffy::Style) {
        let _ = self.taffy.set_style(node, style);
    }

    pub(crate) fn layout(&self, node: taffy::NodeId) -> Option<taffy::Layout> {
        self.taffy.layout(node).copied().ok()
    }

    pub(crate) fn taffy_mut(&mut self) -> &mut taffy::TaffyTree<ElementId> {
        &mut self.taffy
    }
}
