// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Data-driven template specs.

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::controls::text::ContentPresenterWidget;
use crate::element::ElementId;
use crate::widget::{SurfacePartWidget, Widget};
use understory_style::{ClassId, PartTag};

use super::{Template, TemplateChild, TemplateContext, TemplateItemKey, TemplateSlot};

/// Stable identity for a [`ControlTemplate`] value.
///
/// Template equality is identity-based. Rebuilding an equivalent owned
/// template creates a new identity and therefore means "retemplate" to the
/// runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct TemplateId(usize);

impl TemplateId {
    /// Creates a template id from a thin pointer-sized identity.
    #[must_use]
    pub(crate) const fn from_usize(value: usize) -> Self {
        Self(value)
    }

    /// Returns the raw identity value.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

/// Cloneable control-template value for local properties, styles, and
/// authoring builders.
#[derive(Clone)]
pub struct ControlTemplate {
    id: TemplateId,
    storage: ControlTemplateStorage,
}

#[derive(Clone)]
enum ControlTemplateStorage {
    Static(&'static dyn Template),
    Owned(Arc<dyn Template>),
}

impl ControlTemplate {
    /// Creates a template value from a static template recipe.
    #[must_use]
    pub fn from_static(template: &'static dyn Template) -> Self {
        let ptr = template as *const dyn Template as *const ();
        Self {
            id: TemplateId::from_usize(ptr as usize),
            storage: ControlTemplateStorage::Static(template),
        }
    }

    /// Creates a template value from an owned template recipe.
    #[must_use]
    pub fn new<T>(template: T) -> Self
    where
        T: Template + 'static,
    {
        Self::from_arc(Arc::new(template))
    }

    /// Creates a template value from a shared owned template recipe.
    #[must_use]
    pub fn from_arc<T>(template: Arc<T>) -> Self
    where
        T: Template + 'static,
    {
        let template: Arc<dyn Template> = template;
        let ptr = Arc::as_ptr(&template) as *const ();
        Self {
            id: TemplateId::from_usize(ptr as usize),
            storage: ControlTemplateStorage::Owned(template),
        }
    }

    /// Returns this template value's identity.
    #[must_use]
    pub const fn id(&self) -> TemplateId {
        self.id
    }

    pub(crate) fn instantiate(&self, cx: &mut TemplateContext<'_>) {
        match &self.storage {
            ControlTemplateStorage::Static(template) => template.instantiate(cx),
            ControlTemplateStorage::Owned(template) => template.instantiate(cx),
        }
    }
}

impl core::fmt::Debug for ControlTemplate {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ControlTemplate")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl PartialEq for ControlTemplate {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for ControlTemplate {}

impl<T> From<&'static T> for ControlTemplate
where
    T: Template + 'static,
{
    fn from(template: &'static T) -> Self {
        Self::from_static(template)
    }
}

impl From<&'static dyn Template> for ControlTemplate {
    fn from(template: &'static dyn Template) -> Self {
        Self::from_static(template)
    }
}

impl From<TemplateSpec> for ControlTemplate {
    fn from(template: TemplateSpec) -> Self {
        Self::new(template)
    }
}

/// A data-driven default template.
///
/// `TemplateSpec` is intentionally only a compact description of normal
/// element-tree construction. It does not own layout, styling, or presentation;
/// those remain ordinary properties and cascade output on the generated
/// elements.
#[derive(Clone, Debug)]
pub struct TemplateSpec {
    nodes: &'static [TemplateNode],
    owned_nodes: Option<Box<[TemplateNode]>>,
}

impl TemplateSpec {
    /// Creates a spec from top-level template nodes.
    #[must_use]
    #[inline]
    pub const fn new(nodes: &'static [TemplateNode]) -> Self {
        Self {
            nodes,
            owned_nodes: None,
        }
    }

    /// Creates a spec from owned top-level template nodes.
    #[must_use]
    pub fn from_vec(nodes: Vec<TemplateNode>) -> Self {
        Self {
            nodes: &[],
            owned_nodes: Some(nodes.into_boxed_slice()),
        }
    }

    /// Creates a spec from an owned array of top-level template nodes.
    #[must_use]
    pub fn from_nodes<const N: usize>(nodes: [TemplateNode; N]) -> Self {
        Self::from_vec(Vec::from(nodes))
    }

    /// Returns the top-level nodes in this spec.
    #[must_use]
    #[inline]
    pub fn nodes(&self) -> &[TemplateNode] {
        self.owned_nodes.as_deref().unwrap_or(self.nodes)
    }
}

impl Template for TemplateSpec {
    fn instantiate(&self, cx: &mut TemplateContext<'_>) {
        for node in self.nodes() {
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

    /// Creates an owner-content presenter widget.
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
#[derive(Clone, Debug)]
pub struct TemplateNode {
    widget: TemplateWidgetFactory,
    slot: Option<TemplateSlot>,
    part: Option<PartTag>,
    key: Option<TemplateItemKey>,
    classes: &'static [ClassId],
    owned_classes: Option<Box<[ClassId]>>,
    children: &'static [Self],
    owned_children: Option<Box<[Self]>>,
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
            owned_classes: None,
            children: &[],
            owned_children: None,
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
    /// The generated widget presents the template owner's `Content` string
    /// when it has no authored children. If app code adds retained children to
    /// the slot, the presenter becomes a transparent host for that subtree.
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

    /// Adds one class to the generated element.
    #[must_use]
    #[inline]
    pub fn with_class(mut self, class: ClassId) -> Self {
        self.owned_classes = Some(Box::new([class]));
        self
    }

    /// Adds owned classes to the generated element.
    #[must_use]
    #[inline]
    pub fn with_owned_classes(mut self, classes: Vec<ClassId>) -> Self {
        self.owned_classes = Some(classes.into_boxed_slice());
        self
    }

    /// Adds child nodes under this generated element.
    #[must_use]
    #[inline]
    pub const fn with_children(mut self, children: &'static [Self]) -> Self {
        self.children = children;
        self
    }

    /// Adds one owned child node under this generated element.
    #[must_use]
    #[inline]
    pub fn with_child(mut self, child: Self) -> Self {
        self.owned_children = Some(Box::new([child]));
        self
    }

    /// Adds owned child nodes under this generated element.
    #[must_use]
    #[inline]
    pub fn with_owned_children(mut self, children: Vec<Self>) -> Self {
        self.owned_children = Some(children.into_boxed_slice());
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
    pub fn classes(&self) -> &[ClassId] {
        self.owned_classes.as_deref().unwrap_or(self.classes)
    }

    /// Returns this node's children.
    #[must_use]
    #[inline]
    pub fn children(&self) -> &[Self] {
        self.owned_children.as_deref().unwrap_or(self.children)
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

    for class in node.classes() {
        cx.add_class(id, *class);
    }
    for child in node.children() {
        instantiate_spec_node(cx, id, child);
    }
    id
}
