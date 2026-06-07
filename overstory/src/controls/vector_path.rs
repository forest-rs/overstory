// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `VectorPath`: retained vector path drawing.
//!
//! `VectorPath` is the small Overstory-side bridge for vector artwork. It
//! stores a resolved [`PathPrimitive`] and contributes it through the normal
//! presentation pass so import crates can create ordinary elements instead of
//! bypassing the UI tree.
//!
//! Roadmap:
//! - Keep this as a leaf drawing primitive, not a full SVG renderer.
//! - Let importer crates create many path elements while Overstory keeps the
//!   path presentation/lowering path shared and styleable.
//! - Add source metadata, path hit testing, and shape-specific accessibility
//!   only after repeated importer/control needs make those boundaries clear.

use alloc::boxed::Box;

use kurbo::Affine;
use understory_box_tree::NodeId;
use understory_presentation::{
    PathFill, PathPaintOrder, PathPrimitive, PathStroke, PresentationStore,
};
use understory_style::TypeTag;

use crate::accessibility::AccessibilityContext;
use crate::behavior::PresentContext;
use crate::element::{Element, ElementId};
use crate::layouts::Layout;
use crate::style_class::StyleClass;
use crate::ui::Ui;
use crate::widget::{AppendSpec, ElementConfig, ElementSpec, Widget};

/// Selector identity for vector path elements.
pub const TYPE_VECTOR_PATH: TypeTag = TypeTag(19);

/// Builder + spec for a retained vector path element.
#[derive(Clone, Debug)]
pub struct VectorPath {
    primitive: PathPrimitive,
    element: ElementConfig,
}

impl VectorPath {
    /// Creates a vector path from resolved presentation intent.
    #[must_use]
    pub fn from_primitive(primitive: PathPrimitive) -> Self {
        Self {
            primitive,
            element: ElementConfig::default(),
        }
    }

    /// Creates a vector path from path geometry.
    #[must_use]
    pub fn new(path: kurbo::BezPath) -> Self {
        Self::from_primitive(PathPrimitive::new(path))
    }

    /// Sets the fill pass.
    #[must_use]
    pub fn fill(mut self, fill: PathFill) -> Self {
        self.primitive.fill = Some(fill);
        self
    }

    /// Sets the stroke pass.
    #[must_use]
    pub fn stroke(mut self, stroke: PathStroke) -> Self {
        self.primitive.stroke = Some(stroke);
        self
    }

    /// Sets the path transform.
    #[must_use]
    pub fn transform(mut self, transform: Affine) -> Self {
        self.primitive.transform = transform;
        self
    }

    /// Sets the fill/stroke paint order.
    #[must_use]
    pub fn paint_order(mut self, paint_order: PathPaintOrder) -> Self {
        self.primitive.paint_order = paint_order;
        self
    }

    /// Replaces the element's layout contribution.
    #[must_use]
    pub fn layout(mut self, layout: impl Into<Layout>) -> Self {
        self.element.set_layout(layout);
        self
    }

    /// Attaches a style class to the resulting element.
    #[must_use]
    pub fn class(mut self, class: impl Into<StyleClass>) -> Self {
        self.element.add_class(class);
        self
    }
}

impl AppendSpec for VectorPath {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget(
            parent,
            Box::new(VectorPathWidget {
                primitive: self.primitive,
            }),
        );
        self.element.apply_to(ui, id);
        id
    }
}

impl ElementSpec for VectorPath {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

/// Per-element widget for [`VectorPath`].
#[derive(Clone, Debug)]
pub(crate) struct VectorPathWidget {
    primitive: PathPrimitive,
}

impl Widget for VectorPathWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_VECTOR_PATH)
    }

    fn configure_accessibility(
        &self,
        _element: &Element,
        _ctx: &AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        node.set_hidden();
    }

    fn present(
        &self,
        _element: &Element,
        _ctx: &PresentContext<'_>,
        store: &mut PresentationStore<NodeId, ElementId>,
        node: NodeId,
    ) {
        let _ = store.set_path(node, self.primitive.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use imaging::record::{Command, Draw, Scene};
    use kurbo::Size;
    use peniko::Color;

    #[test]
    fn vector_path_paints_path_primitive() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::new().logical_size(20.0, 20.0));

        let mut path = kurbo::BezPath::new();
        path.move_to((2.0, 2.0));
        path.line_to((18.0, 2.0));
        path.line_to((18.0, 18.0));
        path.close_path();
        ui.append(
            root,
            VectorPath::new(path)
                .fill(PathFill::new(Color::WHITE))
                .layout(Layout::new().logical_size(20.0, 20.0)),
        );

        ui.compute_layout(Size::new(20.0, 20.0));
        let mut scene = Scene::new();
        ui.paint(&mut scene);

        let saw_fill = scene.commands().iter().any(|command| {
            let Command::Draw(id) = command else {
                return false;
            };
            matches!(scene.draw_op(*id), Draw::Fill { .. })
        });
        assert!(saw_fill, "vector path should emit a fill draw");
    }
}
