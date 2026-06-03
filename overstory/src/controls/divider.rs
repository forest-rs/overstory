// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Divider`: decorative separator for rows and columns.
//!
//! Roadmap:
//! - Keep `Divider` decorative and non-interactive; resizing behavior belongs
//!   in `Splitter`.
//! - Let style control thickness, color, insets, and stretching rather than
//!   growing a broad builder API.
//! - Add semantic separator metadata only when callers ask for it; many
//!   dividers should stay hidden from accessibility trees.
//! - Use the same axis vocabulary as scrollbars, splitters, and future layout
//!   helpers so horizontal and vertical behavior remains predictable.

use alloc::boxed::Box;

use kurbo::Axis;
use understory_style::TypeTag;

use crate::element::{Element, ElementId};
use crate::layouts::Layout;
use crate::ui::Ui;
use crate::widget::{AppendSpec, ElementConfig, ElementSpec, Widget};

/// Selector identity for `Divider`.
pub const TYPE_DIVIDER: TypeTag = TypeTag(4);

/// Builder + spec for a simple separator.
#[derive(Clone, Debug)]
pub struct Divider {
    axis: Axis,
    thickness: f32,
    length: Option<f32>,
    element: ElementConfig,
}

impl Default for Divider {
    fn default() -> Self {
        Self {
            axis: Axis::Horizontal,
            thickness: 1.0,
            length: None,
            element: ElementConfig::default(),
        }
    }
}

impl Divider {
    /// Creates a divider along `axis`.
    #[must_use]
    pub fn new(axis: Axis) -> Self {
        Self {
            axis,
            ..Self::default()
        }
    }

    /// Creates a horizontal divider.
    #[must_use]
    pub fn horizontal() -> Self {
        Self::new(Axis::Horizontal)
    }

    /// Creates a vertical divider.
    #[must_use]
    pub fn vertical() -> Self {
        Self::new(Axis::Vertical)
    }

    /// Sets the divider axis.
    #[must_use]
    pub fn axis(mut self, axis: Axis) -> Self {
        self.axis = axis;
        self
    }

    /// Sets the divider thickness in logical pixels.
    #[must_use]
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness.max(1.0);
        self
    }

    /// Sets the divider's main-axis length in logical pixels.
    #[must_use]
    pub fn length(mut self, length: f32) -> Self {
        self.length = Some(length.max(1.0));
        self
    }

    fn default_layout(&self) -> Layout {
        let length = self.length.unwrap_or(1.0);
        match self.axis {
            Axis::Horizontal => Layout::new().logical_size(length, self.thickness),
            Axis::Vertical => Layout::new().logical_size(self.thickness, length),
        }
    }
}

impl AppendSpec for Divider {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget(parent, Box::new(DividerWidget));
        ui.set_layout(id, self.default_layout());
        self.element.apply_to(ui, id);
        id
    }
}

impl ElementSpec for Divider {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

/// Per-element widget for `Divider`.
#[derive(Debug, Default)]
pub(crate) struct DividerWidget;

impl Widget for DividerWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_DIVIDER)
    }

    fn configure_accessibility(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        node.set_hidden();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accessibility::accesskit_node_id;
    use accesskit::{Node, Role, TreeUpdate};
    use kurbo::Size;

    #[test]
    fn vertical_divider_lays_out_to_thickness_and_length() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(0.0).logical_size(100.0, 100.0));
        let divider = ui.append(root, Divider::vertical().thickness(3.0).length(42.0));

        ui.compute_layout(Size::new(100.0, 100.0));

        let bounds = ui.effective_bounds(divider);
        assert_eq!(bounds.width(), 3.0);
        assert_eq!(bounds.height(), 42.0);
    }

    #[test]
    fn divider_accessibility_is_decorative_not_splitter() {
        let mut ui = Ui::new();
        let root = ui.root();
        let divider = ui.append(root, Divider::horizontal().length(42.0));
        ui.compute_layout(Size::new(100.0, 100.0));

        let update = ui.accesskit_tree_update();
        let node = accesskit_node_for(&update, divider);

        assert_eq!(node.role(), Role::GenericContainer);
        assert!(node.is_hidden());
    }

    fn accesskit_node_for(update: &TreeUpdate, element: ElementId) -> &Node {
        update
            .nodes
            .iter()
            .find(|(id, _)| *id == accesskit_node_id(element))
            .map(|(_, node)| node)
            .expect("node should exist")
    }
}
