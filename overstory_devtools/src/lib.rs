// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![no_std]

//! Snapshot adapters and model helpers for Overstory devtools.
//!
//! ```text
//! Ui -> UiInspectionSnapshot -> DevtoolsSnapshot -> inspector UI / transport
//! ```
//!
//! This crate owns the inspection adapter layer. It explicitly does not own
//! live UI mutation, rendering, host integration, or time-series telemetry.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use overstory::inspection::{
    CompositionLayerInspectionRow, ElementInspectionRow, LayerInspectionId, SurfaceInspectionId,
    UiInspectionSnapshot,
};
use overstory::{ElementId, Layout, OutlineNodeKey, OutlineView, OutlineViewNode, Panel, Text, Ui};

/// A source that can produce an owned Overstory inspection snapshot.
///
/// Implement this for host-side targets that want to expose a snapshot without
/// exposing their live UI internals.
pub trait DevtoolsTarget {
    /// Captures the target's current inspectable state.
    fn capture_inspection_snapshot(&self) -> UiInspectionSnapshot;
}

impl DevtoolsTarget for Ui {
    fn capture_inspection_snapshot(&self) -> UiInspectionSnapshot {
        Self::inspection_snapshot(self)
    }
}

/// Owned devtools snapshot plus lightweight lookup tables.
#[derive(Clone, Debug, PartialEq)]
pub struct DevtoolsSnapshot {
    inspection: UiInspectionSnapshot,
    element_index: Vec<(ElementId, usize)>,
    layer_index: Vec<(LayerInspectionId, usize)>,
    surface_index: Vec<(SurfaceInspectionId, usize)>,
}

impl DevtoolsSnapshot {
    /// Captures a snapshot from `target`.
    #[must_use]
    pub fn capture(target: &impl DevtoolsTarget) -> Self {
        Self::new(target.capture_inspection_snapshot())
    }

    /// Wraps an existing inspection snapshot with lookup helpers.
    #[must_use]
    pub fn new(inspection: UiInspectionSnapshot) -> Self {
        let mut element_index: Vec<_> = inspection
            .elements
            .iter()
            .enumerate()
            .map(|(index, row)| (row.id, index))
            .collect();
        element_index.sort_by_key(|(id, _)| *id);

        let mut layer_index: Vec<_> = inspection
            .composition
            .layers
            .iter()
            .enumerate()
            .map(|(index, row)| (row.id, index))
            .collect();
        layer_index.sort_by_key(|(id, _)| *id);

        let mut surface_index: Vec<_> = inspection
            .composition
            .surfaces
            .iter()
            .copied()
            .enumerate()
            .map(|(index, surface)| (surface, index))
            .collect();
        surface_index.sort_by_key(|(id, _)| *id);

        Self {
            inspection,
            element_index,
            layer_index,
            surface_index,
        }
    }

    /// Returns the wrapped inspection snapshot.
    #[must_use]
    pub const fn inspection(&self) -> &UiInspectionSnapshot {
        &self.inspection
    }

    /// Returns the element row for `id`.
    #[must_use]
    pub fn element(&self, id: ElementId) -> Option<&ElementInspectionRow> {
        let index = self.element_offset(id)?;
        self.inspection.elements.get(index)
    }

    /// Returns the composition-layer row for `id`.
    #[must_use]
    pub fn layer(&self, id: LayerInspectionId) -> Option<&CompositionLayerInspectionRow> {
        let index = self
            .layer_index
            .binary_search_by_key(&id, |(candidate, _)| *candidate)
            .ok()
            .map(|index| self.layer_index[index].1)?;
        self.inspection.composition.layers.get(index)
    }

    /// Returns whether `id` is a live Overstory-owned surface in the snapshot.
    #[must_use]
    pub fn has_surface(&self, id: SurfaceInspectionId) -> bool {
        self.surface_index
            .binary_search_by_key(&id, |(candidate, _)| *candidate)
            .is_ok()
    }

    /// Projects the retained element tree into outline rows.
    #[must_use]
    pub fn element_outline(&self) -> Vec<ElementOutlineRow> {
        let mut rows = Vec::new();
        self.push_element_outline(self.inspection.root, 0, &mut rows);
        rows
    }

    /// Projects the retained element tree into string-backed outline nodes.
    ///
    /// The returned nodes use element snapshot indices as
    /// [`OutlineNodeKey`] values, so an outline selection can be mapped back
    /// to `inspection.elements[key.index()].id`.
    #[must_use]
    pub fn element_outline_nodes(&self) -> Vec<OutlineViewNode> {
        self.inspection
            .elements
            .iter()
            .map(|row| {
                let mut node = OutlineViewNode::new(element_label(row));
                if let Some(first_child) = row
                    .children
                    .first()
                    .and_then(|child| self.element_offset(*child))
                {
                    node = node.with_first_child(Some(first_child));
                }
                if let Some(next_sibling) = self.next_sibling_offset(row) {
                    node = node.with_next_sibling(Some(next_sibling));
                }
                node
            })
            .collect()
    }

    /// Builds an [`OutlineView`] for the retained element tree.
    ///
    /// Rows with children start expanded so the first devtools view shows the
    /// whole retained tree without requiring interaction.
    #[must_use]
    pub fn element_outline_view(&self) -> OutlineView {
        let mut view = OutlineView::new().nodes(self.element_outline_nodes(), Some(0));
        for (index, row) in self.inspection.elements.iter().enumerate() {
            if !row.children.is_empty() {
                view = view.expanded(OutlineNodeKey::from(index));
            }
        }
        view
    }

    fn push_element_outline(
        &self,
        element: ElementId,
        depth: usize,
        rows: &mut Vec<ElementOutlineRow>,
    ) {
        let Some(row) = self.element(element) else {
            return;
        };
        rows.push(ElementOutlineRow {
            element,
            parent: row.parent,
            depth,
            child_count: row.children.len(),
            has_widget_identity: row.has_widget_identity,
            layer: row.layer,
            surface: row.surface,
        });
        for child in &row.children {
            self.push_element_outline(*child, depth + 1, rows);
        }
    }

    fn element_offset(&self, id: ElementId) -> Option<usize> {
        self.element_index
            .binary_search_by_key(&id, |(candidate, _)| *candidate)
            .ok()
            .map(|index| self.element_index[index].1)
    }

    fn next_sibling_offset(&self, row: &ElementInspectionRow) -> Option<usize> {
        let siblings = row
            .parent
            .and_then(|parent| self.element(parent))
            .map(|parent| parent.children.as_slice())?;
        let sibling = siblings
            .iter()
            .position(|candidate| *candidate == row.id)
            .and_then(|index| siblings.get(index + 1))?;
        self.element_offset(*sibling)
    }
}

/// One projected retained-element outline row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ElementOutlineRow {
    /// Element represented by this row.
    pub element: ElementId,
    /// Parent element, or `None` for the root.
    pub parent: Option<ElementId>,
    /// Tree depth from the root.
    pub depth: usize,
    /// Number of retained children.
    pub child_count: usize,
    /// Whether the element has widget selector identity.
    pub has_widget_identity: bool,
    /// Composition layer corresponding to this element, if one exists.
    pub layer: Option<LayerInspectionId>,
    /// Surface attached to the element's layer, if one exists.
    pub surface: Option<SurfaceInspectionId>,
}

/// Element ids created by [`append_inspection_view`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DevtoolsViewElements {
    /// Root panel for the devtools view.
    pub root: ElementId,
    /// Outline control showing the retained element tree.
    pub outline: ElementId,
    /// Details panel for the selected element.
    pub details: ElementId,
}

/// Appends a simple inspector view for `snapshot`.
///
/// This is a bootstrap UI for same-process devtools: it renders the retained
/// element tree in an [`OutlineView`] and shows selected element and
/// composition details beside it. Interactive selection wiring can build on the
/// same snapshot model without changing the target UI.
pub fn append_inspection_view(
    ui: &mut Ui,
    parent: ElementId,
    snapshot: &DevtoolsSnapshot,
    selected: Option<ElementId>,
) -> DevtoolsViewElements {
    let root = ui.append(
        parent,
        Panel::row(16.0).layout(Layout::row(16.0).padding_all(12.0)),
    );
    let outline = ui.append(root, snapshot.element_outline_view().size(280.0, 420.0));
    let details = ui.append(
        root,
        Panel::column(6.0).layout(Layout::column(6.0).flex_grow(1.0)),
    );

    append_selected_element_details(ui, details, snapshot, selected);

    DevtoolsViewElements {
        root,
        outline,
        details,
    }
}

fn element_label(row: &ElementInspectionRow) -> String {
    let prefix = if row.has_widget_identity {
        "widget"
    } else {
        "element"
    };
    format!("{prefix} {}:{}", row.id.slot(), row.id.generation())
}

fn append_selected_element_details(
    ui: &mut Ui,
    parent: ElementId,
    snapshot: &DevtoolsSnapshot,
    selected: Option<ElementId>,
) {
    let selected = selected.unwrap_or(snapshot.inspection.root);
    let Some(row) = snapshot.element(selected) else {
        ui.append(parent, Text::new("No selected element").font_size(13.0));
        return;
    };

    ui.append(
        parent,
        Text::new(format!("Element {}:{}", row.id.slot(), row.id.generation())).font_size(15.0),
    );
    ui.append(
        parent,
        Text::new(format!("parent: {}", optional_element_id(row.parent))).font_size(12.0),
    );
    ui.append(
        parent,
        Text::new(format!("children: {}", row.children.len())).font_size(12.0),
    );
    ui.append(
        parent,
        Text::new(format!("type: {:?}", row.type_tag)).font_size(12.0),
    );
    ui.append(
        parent,
        Text::new(format!("bounds: {}", rect_summary(row.bounds))).font_size(12.0),
    );
    ui.append(
        parent,
        Text::new(format!("layer: {}", optional_layer_id(row.layer))).font_size(12.0),
    );
    ui.append(
        parent,
        Text::new(format!("surface: {}", optional_surface_id(row.surface))).font_size(12.0),
    );
    ui.append(
        parent,
        Text::new(format!("composition: {:?}", row.composition_policy)).font_size(12.0),
    );

    if let Some(layer) = row.layer.and_then(|id| snapshot.layer(id)) {
        ui.append(
            parent,
            Text::new(format!(
                "opacity: {:.3} / {:.3}",
                layer.local_opacity, layer.effective_opacity
            ))
            .font_size(12.0),
        );
        ui.append(
            parent,
            Text::new(format!(
                "hidden: {} / {}",
                layer.hidden, layer.effectively_hidden
            ))
            .font_size(12.0),
        );
    }
}

fn optional_element_id(id: Option<ElementId>) -> String {
    id.map_or_else(
        || "none".into(),
        |id| format!("{}:{}", id.slot(), id.generation()),
    )
}

fn optional_layer_id(id: Option<LayerInspectionId>) -> String {
    id.map_or_else(
        || "none".into(),
        |id| format!("{}:{}", id.index, id.generation),
    )
}

fn optional_surface_id(id: Option<SurfaceInspectionId>) -> String {
    id.map_or_else(
        || "none".into(),
        |id| format!("{}:{}", id.index, id.generation),
    )
}

fn rect_summary(rect: overstory::Rect) -> String {
    format!(
        "{:.1},{:.1} {:.1}x{:.1}",
        rect.x0,
        rect.y0,
        rect.width(),
        rect.height()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_capture_indexes_elements_and_layers() {
        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.append(root, Panel::new());

        let snapshot = DevtoolsSnapshot::capture(&ui);
        assert_eq!(snapshot.inspection().root, root);
        assert_eq!(
            snapshot
                .element(child)
                .expect("child row should be indexed")
                .parent,
            Some(root)
        );

        let layer = snapshot
            .element(child)
            .expect("child row should be indexed")
            .layer
            .expect("child should have a composition layer");
        assert_eq!(
            snapshot
                .layer(layer)
                .expect("layer row should be indexed")
                .element,
            Some(child)
        );
    }

    #[test]
    fn element_outline_preserves_tree_depth() {
        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.append(root, Panel::new());
        let grandchild = ui.append(child, Panel::new());

        let snapshot = DevtoolsSnapshot::capture(&ui);
        let outline = snapshot.element_outline();

        assert_eq!(outline[0].element, root);
        assert_eq!(outline[0].depth, 0);
        assert_eq!(outline[1].element, child);
        assert_eq!(outline[1].depth, 1);
        assert_eq!(outline[2].element, grandchild);
        assert_eq!(outline[2].depth, 2);
    }

    #[test]
    fn element_outline_nodes_can_feed_outline_view() {
        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.append(root, Panel::new());

        let snapshot = DevtoolsSnapshot::capture(&ui);
        let nodes = snapshot.element_outline_nodes();

        assert_eq!(nodes.len(), 2);
        assert_eq!(
            nodes[0].label(),
            format!("widget {}:{}", root.slot(), root.generation())
        );
        assert_eq!(
            nodes[1].label(),
            format!("widget {}:{}", child.slot(), child.generation())
        );

        let view = snapshot.element_outline_view();
        assert!(!alloc::format!("{view:?}").is_empty());
    }

    #[test]
    fn append_inspection_view_builds_outline_and_details() {
        let mut target = Ui::new();
        let target_root = target.root();
        let selected = target.append(target_root, Panel::new());
        let snapshot = DevtoolsSnapshot::capture(&target);

        let mut inspector = Ui::new();
        let root = inspector.root();
        let elements = append_inspection_view(&mut inspector, root, &snapshot, Some(selected));

        assert_eq!(
            inspector
                .get(elements.root)
                .and_then(overstory::Element::parent),
            Some(root)
        );
        assert_eq!(
            inspector
                .get(elements.outline)
                .and_then(overstory::Element::parent),
            Some(elements.root)
        );
        assert_eq!(
            inspector
                .get(elements.details)
                .map(overstory::Element::children)
                .map(<[ElementId]>::len),
            Some(10)
        );
    }
}
