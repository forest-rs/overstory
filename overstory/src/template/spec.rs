// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Data-driven template specs.

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::controls::text::{ContentPresenterWidget, TextWidget};
use crate::element::ElementId;
use crate::layouts::Layout;
use crate::widget::{SurfacePartWidget, Widget};
use understory_style::{ClassId, PartTag};

use super::{Template, TemplateChild, TemplateContext, TemplateItemKey, TemplateSlot};

type TemplateSpecFactory = fn() -> TemplateSpec;

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
    SpecFactory(TemplateSpecFactory),
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

    /// Creates a stable template value from a data-spec factory.
    ///
    /// This is for built-in default templates that should be authored as
    /// [`TemplateSpec`] data but still need stable identity across repeated
    /// [`crate::Widget::default_template`] queries. App-authored templates
    /// should generally use [`Self::new`] or `ControlTemplate::from(spec)`.
    #[must_use]
    pub(crate) fn from_spec_factory(factory: TemplateSpecFactory) -> Self {
        #[expect(
            clippy::fn_to_numeric_cast_any,
            reason = "built-in template factories use the function address as a stable identity, not as an arithmetic value"
        )]
        let id = TemplateId::from_usize(factory as usize);
        Self {
            id,
            storage: ControlTemplateStorage::SpecFactory(factory),
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
            ControlTemplateStorage::SpecFactory(factory) => factory().instantiate(cx),
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

/// A compact data-driven template.
///
/// `TemplateSpec` is intentionally only a description of normal element-tree
/// construction. It does not own layout, styling, or presentation; those remain
/// ordinary properties and cascade output on the generated elements.
///
/// Prefer the narrow node constructors such as [`TemplateNode::surface`],
/// [`TemplateNode::surface_slot`], [`TemplateNode::content_presenter`], and
/// [`TemplateNode::text_slot`]. Reach for [`TemplateWidgetFactory`] only when a
/// generated template node truly needs custom widget behavior.
#[derive(Clone, Debug)]
pub struct TemplateSpec {
    nodes: Vec<TemplateNode>,
}

impl TemplateSpec {
    /// Creates a spec from owned top-level template nodes.
    #[must_use]
    pub fn from_vec(nodes: Vec<TemplateNode>) -> Self {
        Self { nodes }
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
        &self.nodes
    }
}

impl Template for TemplateSpec {
    fn instantiate(&self, cx: &mut TemplateContext<'_>) {
        for node in self.nodes() {
            instantiate_spec_node(cx, cx.owner(), node);
        }
    }
}

/// Factory for an arbitrary widget node in a [`TemplateSpec`].
///
/// This is the escape hatch for generated nodes with custom widget behavior.
/// Most visual template structure should use [`TemplateNode`]'s convenience
/// constructors instead, which keep the authored template focused on semantic
/// slots, owner-local parts, classes, keys, and children.
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

    /// Creates a text widget for template-owned glyph or label parts.
    #[must_use]
    #[inline]
    pub const fn text() -> Self {
        Self::new(text_template_widget)
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

fn text_template_widget() -> Box<dyn Widget> {
    Box::new(TextWidget)
}

/// One node in a [`TemplateSpec`].
#[derive(Clone, Debug)]
pub struct TemplateNode {
    widget: TemplateWidgetFactory,
    slot: Option<TemplateSlot>,
    part: Option<PartTag>,
    key: Option<TemplateItemKey>,
    layout: Option<Layout>,
    classes: Vec<ClassId>,
    children: Vec<Self>,
}

impl TemplateNode {
    /// Creates a node with the supplied widget factory.
    #[must_use]
    #[inline]
    pub fn widget(widget: TemplateWidgetFactory) -> Self {
        Self {
            widget,
            slot: None,
            part: None,
            key: None,
            layout: None,
            classes: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Creates a node that fills a semantic template slot.
    #[must_use]
    #[inline]
    pub fn for_slot(slot: TemplateSlot, widget: TemplateWidgetFactory) -> Self {
        Self::widget(widget).with_slot(slot)
    }

    /// Creates a node with an owner-local part tag.
    #[must_use]
    #[inline]
    pub fn for_part(part: PartTag, widget: TemplateWidgetFactory) -> Self {
        Self::widget(widget).with_part(part)
    }

    /// Creates a surface-only node.
    #[must_use]
    #[inline]
    pub fn surface() -> Self {
        Self::widget(TemplateWidgetFactory::surface())
    }

    /// Creates a surface-only node that fills a semantic template slot.
    #[must_use]
    #[inline]
    pub fn surface_slot(slot: TemplateSlot) -> Self {
        Self::for_slot(slot, TemplateWidgetFactory::surface())
    }

    /// Creates a surface-only node with an owner-local part tag.
    #[must_use]
    #[inline]
    pub fn surface_part(part: PartTag) -> Self {
        Self::for_part(part, TemplateWidgetFactory::surface())
    }

    /// Creates a content presenter node for a content-style semantic slot.
    ///
    /// The generated widget presents the template owner's `Content` string
    /// when it has no authored children. If app code adds retained children to
    /// the slot, the presenter becomes a transparent host for that subtree.
    #[must_use]
    #[inline]
    pub fn content_presenter(slot: TemplateSlot) -> Self {
        Self::for_slot(slot, TemplateWidgetFactory::content_presenter())
    }

    /// Creates a text node for a template-owned semantic slot.
    ///
    /// Use this for control-owned glyph or label parts whose text is supplied
    /// by the owning control after expansion. Use
    /// [`Self::content_presenter`] for user-authored content slots.
    #[must_use]
    #[inline]
    pub fn text_slot(slot: TemplateSlot) -> Self {
        Self::for_slot(slot, TemplateWidgetFactory::text())
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

    /// Adds one class to the generated element.
    #[must_use]
    #[inline]
    pub fn with_class(mut self, class: ClassId) -> Self {
        self.classes.push(class);
        self
    }

    /// Adds classes to the generated element.
    #[must_use]
    #[inline]
    pub fn with_classes<I>(mut self, classes: I) -> Self
    where
        I: IntoIterator<Item = ClassId>,
    {
        self.classes.extend(classes);
        self
    }

    /// Writes a layout contribution to the generated element.
    ///
    /// Only the longhands carried by `layout` are written during template
    /// expansion, matching [`crate::Ui::set_layout`]. This is for stable
    /// template-owned defaults such as absolute placement, fixed part sizes,
    /// or content-host flow behavior.
    #[must_use]
    #[inline]
    pub fn with_layout(mut self, layout: impl Into<Layout>) -> Self {
        self.layout = Some(layout.into());
        self
    }

    /// Adds child nodes under this generated element.
    #[must_use]
    #[inline]
    pub fn with_children<I>(mut self, children: I) -> Self
    where
        I: IntoIterator<Item = Self>,
    {
        self.children.extend(children);
        self
    }

    /// Adds one child node under this generated element.
    #[must_use]
    #[inline]
    pub fn with_child(mut self, child: Self) -> Self {
        self.children.push(child);
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
        &self.classes
    }

    /// Returns this node's children.
    #[must_use]
    #[inline]
    pub fn children(&self) -> &[Self] {
        &self.children
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

    if let Some(layout) = node.layout.clone() {
        cx.set_layout(id, layout);
    }
    for class in node.classes() {
        cx.add_class(id, *class);
    }
    for child in node.children() {
        instantiate_spec_node(cx, id, child);
    }
    id
}
