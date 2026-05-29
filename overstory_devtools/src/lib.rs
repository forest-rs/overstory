// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![no_std]

//! Snapshot adapters and model helpers for Overstory devtools.
//!
//! ```text
//! Ui -> UiInspectionSnapshot -> DevtoolsSnapshot -> DevtoolsSession -> inspector UI / transport
//! ```
//!
//! This crate owns the inspection adapter layer. It explicitly does not own
//! live UI mutation, rendering, host integration, or time-series telemetry.

extern crate alloc;

use core::cell::Cell;

use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;

use overstory::inspection::{
    CompositionLayerInspectionRow, ElementInspectionRow, LayerInspectionId, SurfaceInspectionId,
    UiInspectionSnapshot,
};
use overstory::{
    ElementId, Layout, OutlineNodeKey, OutlineView, OutlineViewNode, Panel, TYPE_BUTTON,
    TYPE_CHECK_BOX, TYPE_DIVIDER, TYPE_GENERIC, TYPE_LIST_BOX, TYPE_OUTLINE_VIEW,
    TYPE_PROGRESS_BAR, TYPE_SCROLL_AREA, TYPE_SCROLL_BAR, TYPE_SCROLL_VIEW, TYPE_SELECT,
    TYPE_SLIDER, TYPE_SPINNER, TYPE_SPLITTER, TYPE_TEXT, TYPE_TEXT_INPUT, TYPE_TOGGLE,
    TYPE_TOOLTIP, TYPE_VECTOR_PATH, TYPE_VIEWPORT, Text, Ui, theme,
};

mod session;

#[cfg(feature = "spoor-session")]
pub use session::DevtoolsSpoorSummary;
pub use session::{DevtoolsEvent, DevtoolsSession, DevtoolsSessionHandle};

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

    /// Returns the outline key that represents `id` in the element tree.
    #[must_use]
    pub fn outline_key_for_element(&self, id: ElementId) -> Option<OutlineNodeKey> {
        self.element_offset(id).map(OutlineNodeKey::from)
    }

    /// Returns the element represented by `key` in the element outline.
    #[must_use]
    pub fn element_for_outline_key(&self, key: OutlineNodeKey) -> Option<ElementId> {
        self.inspection.elements.get(key.index()).map(|row| row.id)
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
/// composition details beside it. Selecting an outline row rebuilds the details
/// panel from the same snapshot.
pub fn append_inspection_view(
    ui: &mut Ui,
    parent: ElementId,
    snapshot: &DevtoolsSnapshot,
    selected: Option<ElementId>,
) -> DevtoolsViewElements {
    append_session_view(
        ui,
        parent,
        DevtoolsSessionHandle::new(
            DevtoolsSession::new(snapshot.clone()).with_selected_element(selected),
        ),
    )
}

/// Appends an inspector view backed by `session`.
///
/// Selection changes update both the rendered details panel and the supplied
/// session handle.
pub fn append_session_view(
    ui: &mut Ui,
    parent: ElementId,
    session: DevtoolsSessionHandle,
) -> DevtoolsViewElements {
    let root = ui.append(
        parent,
        Panel::row(16.0).layout(Layout::row(16.0).padding_all(12.0)),
    );
    if let Some(foreground) = ui.theme().get::<overstory::Brush>(theme::ON_SURFACE_FG) {
        ui.set_local(root, ui.properties().foreground, foreground.clone());
    }

    let details_slot: Rc<Cell<Option<ElementId>>> = Rc::new(Cell::new(None));
    let details_for_selection = Rc::clone(&details_slot);
    let session_for_selection = session.clone();
    let mut outline_view =
        session.with(|session| session.snapshot().element_outline_view().size(280.0, 420.0));
    if let Some(key) = session.with(DevtoolsSession::selected_outline_key) {
        outline_view = outline_view.selected_key(key);
    }
    let outline = ui.append(
        root,
        outline_view.on_selection_changed(move |ui, event| {
            let Some(details) = details_for_selection.get() else {
                return;
            };
            clear_children(ui, details);
            session_for_selection.with_mut(|session| {
                let _ = session.select_outline_key(event.selected_key);
                append_selected_element_details(
                    ui,
                    details,
                    session.snapshot(),
                    session.selected_element(),
                );
            });
        }),
    );
    let details = ui.append(
        root,
        Panel::column(6.0).layout(Layout::column(6.0).flex_grow(1.0)),
    );
    details_slot.set(Some(details));

    session.with(|session| {
        append_selected_element_details(
            ui,
            details,
            session.snapshot(),
            session.selected_element(),
        );
    });

    DevtoolsViewElements {
        root,
        outline,
        details,
    }
}

fn clear_children(ui: &mut Ui, parent: ElementId) {
    let children = ui
        .get(parent)
        .map(|element| element.children().to_vec())
        .unwrap_or_default();
    for child in children {
        ui.remove(child);
    }
}

fn element_label(row: &ElementInspectionRow) -> String {
    let prefix = element_type_label(row);
    format!("{prefix} {}:{}", row.id.slot(), row.id.generation())
}

fn element_type_label(row: &ElementInspectionRow) -> &'static str {
    let Some(tag) = row.type_tag else {
        return "Element";
    };
    if tag == TYPE_BUTTON {
        "Button"
    } else if tag == TYPE_CHECK_BOX {
        "CheckBox"
    } else if tag == TYPE_DIVIDER {
        "Divider"
    } else if tag == TYPE_GENERIC {
        "Panel"
    } else if tag == TYPE_LIST_BOX {
        "ListBox"
    } else if tag == TYPE_OUTLINE_VIEW {
        "OutlineView"
    } else if tag == TYPE_PROGRESS_BAR {
        "ProgressBar"
    } else if tag == TYPE_SCROLL_AREA {
        "ScrollArea"
    } else if tag == TYPE_SCROLL_BAR {
        "ScrollBar"
    } else if tag == TYPE_SCROLL_VIEW {
        "ScrollView"
    } else if tag == TYPE_SELECT {
        "Select"
    } else if tag == TYPE_SLIDER {
        "Slider"
    } else if tag == TYPE_SPINNER {
        "Spinner"
    } else if tag == TYPE_SPLITTER {
        "Splitter"
    } else if tag == TYPE_TEXT {
        "Text"
    } else if tag == TYPE_TEXT_INPUT {
        "TextInput"
    } else if tag == TYPE_TOGGLE {
        "ToggleSwitch"
    } else if tag == TYPE_TOOLTIP {
        "Tooltip"
    } else if tag == TYPE_VECTOR_PATH {
        "VectorPath"
    } else if tag == TYPE_VIEWPORT {
        "Viewport"
    } else if row.has_widget_identity {
        "Widget"
    } else {
        "Element"
    }
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
        Text::new(format!("type: {}", element_type_label(row))).font_size(12.0),
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
    #[cfg(feature = "spoor-session")]
    use alloc::vec;

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
            format!("Panel {}:{}", root.slot(), root.generation())
        );
        assert_eq!(
            nodes[1].label(),
            format!("Panel {}:{}", child.slot(), child.generation())
        );

        let view = snapshot.element_outline_view();
        assert!(!alloc::format!("{view:?}").is_empty());
    }

    #[test]
    fn element_outline_keys_map_back_to_elements() {
        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.append(root, Panel::new());

        let snapshot = DevtoolsSnapshot::capture(&ui);
        let key = snapshot
            .outline_key_for_element(child)
            .expect("child should have an outline key");

        assert_eq!(snapshot.element_for_outline_key(key), Some(child));
    }

    #[test]
    fn devtools_session_selects_elements_by_outline_key() {
        let mut ui = Ui::new();
        let root = ui.root();
        let child = ui.append(root, Panel::new());
        let grandchild = ui.append(child, Panel::new());
        let snapshot = DevtoolsSnapshot::capture(&ui);
        let grandchild_key = snapshot
            .outline_key_for_element(grandchild)
            .expect("grandchild should have an outline key");

        let mut session = DevtoolsSession::new(snapshot).with_selected_element(Some(child));
        let event = session.select_outline_key(Some(grandchild_key));

        assert_eq!(
            event,
            Some(DevtoolsEvent::SelectionChanged {
                selected: Some(grandchild)
            })
        );
        assert_eq!(session.selected_element(), Some(grandchild));
        assert_eq!(session.selected_outline_key(), Some(grandchild_key));
    }

    #[test]
    fn devtools_session_replaces_snapshot_and_clears_stale_selection() {
        let mut first = Ui::new();
        let first_root = first.root();
        let selected = first.append(first_root, Panel::new());
        let mut session = DevtoolsSession::capture(&first).with_selected_element(Some(selected));

        let second = Ui::new();
        let event = session.replace_snapshot(DevtoolsSnapshot::capture(&second));

        assert_eq!(event, DevtoolsEvent::SnapshotReplaced { selected: None });
        assert_eq!(session.selected_element(), None);
    }

    #[cfg(feature = "spoor-session")]
    #[test]
    fn devtools_session_summarizes_spoor_envelopes() {
        use spoor_session::{
            DroppedSummary, EnvelopeHeader, OwnedSessionEnvelope, OwnedSessionPayload,
            OwnedSnapshotMetadata, OwnedSnapshotSummary, PROTOCOL_VERSION,
        };

        let mut session = DevtoolsSession::new(DevtoolsSnapshot::capture(&Ui::new()));
        session.replace_spoor_envelopes(vec![
            OwnedSessionEnvelope {
                header: EnvelopeHeader {
                    protocol_version: PROTOCOL_VERSION,
                    session_sequence: 7,
                    chunk_index: 0,
                },
                payload: OwnedSessionPayload::SnapshotStart(OwnedSnapshotSummary {
                    metadata: OwnedSnapshotMetadata {
                        process_name: String::from("overstory_demo"),
                        session_name: Some(String::from("devtools")),
                        session_sequence: 7,
                    },
                    capture_start_micros: 0,
                    captured_at_micros: 10,
                    context_count: 3,
                    counter_count: 2,
                    event_count: 1,
                }),
            },
            OwnedSessionEnvelope {
                header: EnvelopeHeader {
                    protocol_version: PROTOCOL_VERSION,
                    session_sequence: 7,
                    chunk_index: 1,
                },
                payload: OwnedSessionPayload::SnapshotEnd(DroppedSummary {
                    contexts: 1,
                    counters: 2,
                    events: 3,
                }),
            },
        ]);

        assert_eq!(
            session.spoor_summary(),
            DevtoolsSpoorSummary {
                envelope_count: 2,
                session_sequence: Some(7),
                context_count: 3,
                counter_count: 2,
                event_count: 1,
                dropped_contexts: 1,
                dropped_counters: 2,
                dropped_events: 3,
            }
        );
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

    #[test]
    fn append_inspection_view_updates_details_when_outline_selection_changes() {
        let mut target = Ui::new();
        let target_root = target.root();
        let child = target.append(target_root, Panel::new());
        let grandchild = target.append(child, Panel::new());
        let snapshot = DevtoolsSnapshot::capture(&target);

        let mut inspector = Ui::new();
        let root = inspector.root();
        let elements = append_inspection_view(&mut inspector, root, &snapshot, Some(child));
        let child_key = snapshot
            .outline_key_for_element(child)
            .expect("child should have an outline key");
        let grandchild_key = snapshot
            .outline_key_for_element(grandchild)
            .expect("grandchild should have an outline key");
        let first_detail_before = inspector
            .get(elements.details)
            .and_then(|element| element.children().first().copied())
            .expect("details should contain selected element text");

        assert_eq!(
            overstory::selected_outline_key(&inspector, elements.outline),
            Some(child_key)
        );
        assert!(overstory::select_outline_key(
            &mut inspector,
            elements.outline,
            grandchild_key
        ));

        let detail_children = inspector
            .get(elements.details)
            .map(overstory::Element::children)
            .expect("details panel should still exist");
        assert_eq!(detail_children.len(), 10);
        assert_ne!(detail_children[0], first_detail_before);
        assert_eq!(
            overstory::selected_outline_key(&inspector, elements.outline),
            Some(grandchild_key)
        );
    }

    #[test]
    fn append_session_view_updates_session_selection() {
        let mut target = Ui::new();
        let target_root = target.root();
        let child = target.append(target_root, Panel::new());
        let grandchild = target.append(child, Panel::new());
        let snapshot = DevtoolsSnapshot::capture(&target);
        let grandchild_key = snapshot
            .outline_key_for_element(grandchild)
            .expect("grandchild should have an outline key");
        let session = DevtoolsSessionHandle::new(
            DevtoolsSession::new(snapshot).with_selected_element(Some(child)),
        );

        let mut inspector = Ui::new();
        let root = inspector.root();
        let elements = append_session_view(&mut inspector, root, session.clone());

        assert!(overstory::select_outline_key(
            &mut inspector,
            elements.outline,
            grandchild_key
        ));
        assert_eq!(
            session.with(DevtoolsSession::selected_element),
            Some(grandchild)
        );
    }
}
