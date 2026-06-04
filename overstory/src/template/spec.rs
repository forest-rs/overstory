// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Data-driven template specs.

use alloc::boxed::Box;

use crate::controls::text::ContentPresenterWidget;
use crate::element::ElementId;
use crate::widget::{SurfacePartWidget, Widget};
use understory_style::{ClassId, PartTag};

use super::{Template, TemplateChild, TemplateContext, TemplateItemKey, TemplateSlot};

/// A data-driven default template.
///
/// `TemplateSpec` is intentionally only a compact description of normal
/// element-tree construction. It does not own layout, styling, or presentation;
/// those remain ordinary properties and cascade output on the generated
/// elements.
#[derive(Copy, Clone, Debug)]
pub struct TemplateSpec {
    nodes: &'static [TemplateNode],
}

impl TemplateSpec {
    /// Creates a spec from top-level template nodes.
    #[must_use]
    #[inline]
    pub const fn new(nodes: &'static [TemplateNode]) -> Self {
        Self { nodes }
    }

    /// Returns the top-level nodes in this spec.
    #[must_use]
    #[inline]
    pub const fn nodes(&self) -> &'static [TemplateNode] {
        self.nodes
    }
}

impl Template for TemplateSpec {
    fn instantiate(&self, cx: &mut TemplateContext<'_>) {
        for node in self.nodes {
            instantiate_spec_node(cx, cx.owner(), node);
        }
    }
}

/// Factory for a widget node in a [`TemplateSpec`].
#[derive(Copy, Clone)]
pub struct TemplateWidgetFactory {
    create: fn() -> Box<dyn Widget>,
}

impl core::fmt::Debug for TemplateWidgetFactory {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TemplateWidgetFactory")
            .finish_non_exhaustive()
    }
}

impl TemplateWidgetFactory {
    /// Creates a factory from a zero-argument widget constructor.
    #[must_use]
    #[inline]
    pub const fn new(create: fn() -> Box<dyn Widget>) -> Self {
        Self { create }
    }

    /// Creates a surface-only template part widget.
    #[must_use]
    #[inline]
    pub const fn surface() -> Self {
        Self::new(surface_template_widget)
    }

    /// Creates a content presenter widget.
    #[must_use]
    #[inline]
    pub const fn content_presenter() -> Self {
        Self::new(content_presenter_widget)
    }

    fn create(self) -> Box<dyn Widget> {
        (self.create)()
    }
}

fn surface_template_widget() -> Box<dyn Widget> {
    Box::new(SurfacePartWidget)
}

fn content_presenter_widget() -> Box<dyn Widget> {
    Box::new(ContentPresenterWidget)
}

/// One node in a [`TemplateSpec`].
#[derive(Copy, Clone, Debug)]
pub struct TemplateNode {
    widget: TemplateWidgetFactory,
    slot: Option<TemplateSlot>,
    part: Option<PartTag>,
    key: Option<TemplateItemKey>,
    classes: &'static [ClassId],
    children: &'static [Self],
}

impl TemplateNode {
    /// Creates a node with the supplied widget factory.
    #[must_use]
    #[inline]
    pub const fn widget(widget: TemplateWidgetFactory) -> Self {
        Self {
            widget,
            slot: None,
            part: None,
            key: None,
            classes: &[],
            children: &[],
        }
    }

    /// Creates a node that fills a semantic template slot.
    #[must_use]
    #[inline]
    pub const fn for_slot(slot: TemplateSlot, widget: TemplateWidgetFactory) -> Self {
        Self::widget(widget).with_slot(slot)
    }

    /// Creates a node with an owner-local part tag.
    #[must_use]
    #[inline]
    pub const fn for_part(part: PartTag, widget: TemplateWidgetFactory) -> Self {
        Self::widget(widget).with_part(part)
    }

    /// Creates a surface-only node.
    #[must_use]
    #[inline]
    pub const fn surface() -> Self {
        Self::widget(TemplateWidgetFactory::surface())
    }

    /// Creates a surface-only node that fills a semantic template slot.
    #[must_use]
    #[inline]
    pub const fn surface_slot(slot: TemplateSlot) -> Self {
        Self::for_slot(slot, TemplateWidgetFactory::surface())
    }

    /// Creates a surface-only node with an owner-local part tag.
    #[must_use]
    #[inline]
    pub const fn surface_part(part: PartTag) -> Self {
        Self::for_part(part, TemplateWidgetFactory::surface())
    }

    /// Creates a content presenter node for a content-style semantic slot.
    ///
    /// The generated widget reads `Content` from the template owner and
    /// resolves text styling on the generated element.
    #[must_use]
    #[inline]
    pub const fn content_presenter(slot: TemplateSlot) -> Self {
        Self::for_slot(slot, TemplateWidgetFactory::content_presenter())
    }

    /// Assigns this node to a semantic template slot.
    #[must_use]
    #[inline]
    pub const fn with_slot(mut self, slot: TemplateSlot) -> Self {
        self.slot = Some(slot);
        self
    }

    /// Marks the node as an owner-local named part.
    #[must_use]
    #[inline]
    pub const fn with_part(mut self, part: PartTag) -> Self {
        self.part = Some(part);
        self
    }

    /// Assigns a stable key for a repeated part.
    #[must_use]
    #[inline]
    pub const fn with_key(mut self, key: TemplateItemKey) -> Self {
        self.key = Some(key);
        self
    }

    /// Adds classes to the generated element.
    #[must_use]
    #[inline]
    pub const fn with_classes(mut self, classes: &'static [ClassId]) -> Self {
        self.classes = classes;
        self
    }

    /// Adds child nodes under this generated element.
    #[must_use]
    #[inline]
    pub const fn with_children(mut self, children: &'static [Self]) -> Self {
        self.children = children;
        self
    }

    /// Returns this node's widget factory.
    #[must_use]
    #[inline]
    pub const fn widget_factory(&self) -> TemplateWidgetFactory {
        self.widget
    }

    /// Returns this node's semantic slot, if any.
    #[must_use]
    #[inline]
    pub const fn slot(&self) -> Option<TemplateSlot> {
        self.slot
    }

    /// Returns this node's part tag, if any.
    #[must_use]
    #[inline]
    pub const fn part(&self) -> Option<PartTag> {
        self.part
    }

    /// Returns this node's repeated-item key, if any.
    #[must_use]
    #[inline]
    pub const fn key(&self) -> Option<TemplateItemKey> {
        self.key
    }

    /// Returns this node's classes.
    #[must_use]
    #[inline]
    pub const fn classes(&self) -> &'static [ClassId] {
        self.classes
    }

    /// Returns this node's children.
    #[must_use]
    #[inline]
    pub const fn children(&self) -> &'static [Self] {
        self.children
    }
}

fn instantiate_spec_node(
    cx: &mut TemplateContext<'_>,
    parent: ElementId,
    node: &TemplateNode,
) -> ElementId {
    let mut child = TemplateChild::widget(node.widget.create());
    if let Some(slot) = node.slot {
        child = child.with_slot(slot);
    }
    if let Some(part) = node.part {
        child = child.with_part(part);
    }
    if let Some(key) = node.key {
        child = child.with_key(key);
    }
    let id = cx.insert_under(parent, child);

    for class in node.classes {
        cx.add_class(id, *class);
    }
    for child in node.children {
        instantiate_spec_node(cx, id, child);
    }
    id
}
