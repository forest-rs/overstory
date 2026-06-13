// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Expansion context passed to template factories.

use alloc::boxed::Box;

use crate::binding::BindingOwner;
use crate::controls::text::TextWidget;
use crate::element::ElementId;
use crate::event_runtime::{ClickEvent, DragEvent, KeyEvent};
use crate::layouts::{
    Layout, LayoutAlignment, LayoutContainer, LayoutOverflow, LayoutPlacement, LayoutSize,
    LayoutSpacing,
};
use crate::properties::BuiltInProperties;
use crate::style_class::StyleClass;
use crate::ui::Ui;
use crate::widget::Widget;
use understory_property::{DependencyObjectExt, Property};
use understory_property_binding::{BindingError, BindingId, PropertyEndpoint};
use understory_style::PartTag;

use super::{TemplateChild, TemplateId, TemplateInstance, TemplateItemKey, TemplateSlot};

/// Expansion context passed to a [`super::Template`].
///
/// The context is the canonical way for templates to create generated parts,
/// bind owner properties into those parts, and install template-owned
/// behavior. It keeps template bookkeeping local to expansion instead of
/// relying on ambient `Ui` insertion state.
#[derive(Debug)]
pub struct TemplateContext<'a> {
    ui: &'a mut Ui,
    owner: ElementId,
    instance: TemplateInstance,
}

impl<'a> TemplateContext<'a> {
    pub(crate) fn new(ui: &'a mut Ui, owner: ElementId, template_id: TemplateId) -> Self {
        Self {
            ui,
            owner,
            instance: TemplateInstance::new(owner, template_id),
        }
    }

    pub(crate) fn finish(self) -> TemplateInstance {
        self.instance
    }

    /// Returns the element whose template is currently expanding.
    #[must_use]
    #[inline]
    pub fn owner(&self) -> ElementId {
        self.owner
    }

    /// Returns Overstory's built-in property handles.
    #[must_use]
    #[inline]
    pub fn properties(&self) -> &BuiltInProperties {
        self.ui.properties()
    }

    /// Inserts a generated child directly under the template owner.
    pub fn insert(&mut self, child: TemplateChild) -> ElementId {
        self.insert_under(self.owner, child)
    }

    /// Inserts a generated child under an existing template-generated parent.
    pub fn insert_under(&mut self, parent: ElementId, child: TemplateChild) -> ElementId {
        let TemplateChild {
            widget,
            slot,
            part_tag,
            key,
            owner_property_dependencies,
        } = child;
        let part_tag = part_tag.or_else(|| slot.and_then(TemplateSlot::part_tag));
        let id = self
            .ui
            .insert_template_child(self.owner, parent, part_tag, Some(widget));
        if slot.is_some() || part_tag.is_some() {
            self.instance.push_part(slot, part_tag, key, id);
        }
        for dependency in owner_property_dependencies {
            self.instance
                .push_owner_property_dependent(dependency.property_id(self.properties()), id);
        }
        id
    }

    /// Inserts an arbitrary widget child under the template owner.
    pub fn widget(&mut self, widget: Box<dyn Widget>) -> ElementId {
        self.insert(TemplateChild::widget(widget))
    }

    /// Inserts an arbitrary widget child under an existing template-generated
    /// parent.
    pub fn widget_under(&mut self, parent: ElementId, widget: Box<dyn Widget>) -> ElementId {
        self.insert_under(parent, TemplateChild::widget(widget))
    }

    /// Inserts an unnamed surface-only child under the template owner.
    pub fn surface(&mut self) -> ElementId {
        self.insert(TemplateChild::surface())
    }

    /// Inserts an unnamed surface-only child under an existing
    /// template-generated parent.
    pub fn surface_under(&mut self, parent: ElementId) -> ElementId {
        self.insert_under(parent, TemplateChild::surface())
    }

    /// Inserts a surface-only child with an owner-local part tag under the
    /// template owner.
    pub fn surface_part(&mut self, part: PartTag) -> ElementId {
        self.insert(TemplateChild::surface_part(part))
    }

    /// Inserts a surface-only child with an owner-local part tag under an
    /// existing template-generated parent.
    pub fn surface_part_under(&mut self, parent: ElementId, part: PartTag) -> ElementId {
        self.insert_under(parent, TemplateChild::surface_part(part))
    }

    /// Inserts a surface-only child that fills a semantic slot under the
    /// template owner.
    pub fn surface_slot(&mut self, slot: TemplateSlot) -> ElementId {
        self.insert(TemplateChild::surface_slot(slot))
    }

    /// Inserts a keyed surface-only child that fills a repeated semantic slot
    /// under the template owner.
    pub fn keyed_surface_slot(&mut self, slot: TemplateSlot, key: TemplateItemKey) -> ElementId {
        self.insert(TemplateChild::surface_slot(slot).with_key(key))
    }

    /// Inserts a surface-only child that fills a semantic slot under an
    /// existing template-generated parent.
    pub fn surface_slot_under(&mut self, parent: ElementId, slot: TemplateSlot) -> ElementId {
        self.insert_under(parent, TemplateChild::surface_slot(slot))
    }

    /// Inserts a content presenter for a content-style semantic slot under the
    /// template owner.
    ///
    /// The generated widget presents the templated owner's `Content` property
    /// when the slot has no authored children. If app code adds retained
    /// children to the slot, the presenter becomes a transparent host for that
    /// subtree.
    pub fn content_presenter(&mut self, slot: TemplateSlot) -> ElementId {
        self.insert(TemplateChild::content_presenter(slot))
    }

    /// Inserts a content presenter for a content-style semantic slot under an
    /// existing template-generated parent.
    pub fn content_presenter_under(&mut self, parent: ElementId, slot: TemplateSlot) -> ElementId {
        self.insert_under(parent, TemplateChild::content_presenter(slot))
    }

    /// Inserts a text element that fills a semantic slot under the template
    /// owner.
    ///
    /// Use this for template-owned glyph or label parts whose text is set by
    /// the owning control after expansion. Use [`Self::content_presenter`] for
    /// user-authored content slots.
    pub fn text_slot(&mut self, slot: TemplateSlot) -> ElementId {
        self.insert(TemplateChild::slot(slot, Box::new(TextWidget)))
    }

    /// Inserts a keyed text element that fills a repeated semantic slot under
    /// the template owner.
    pub fn keyed_text_slot(&mut self, slot: TemplateSlot, key: TemplateItemKey) -> ElementId {
        self.insert(TemplateChild::slot(slot, Box::new(TextWidget)).with_key(key))
    }

    /// Inserts a text element that fills a semantic slot under an existing
    /// template-generated parent.
    pub fn text_slot_under(&mut self, parent: ElementId, slot: TemplateSlot) -> ElementId {
        self.insert_under(parent, TemplateChild::slot(slot, Box::new(TextWidget)))
    }

    /// Adds a class to a generated element.
    pub fn add_class(&mut self, id: ElementId, class: impl Into<StyleClass>) {
        let class = class.into().resolve(self.ui.style_vocabulary_mut());
        self.ui.add_class(id, class);
    }

    /// Writes a local property value through the owning [`Ui`].
    pub fn set_local<T: Clone + PartialEq + 'static>(
        &mut self,
        id: ElementId,
        property: Property<T>,
        value: T,
    ) {
        self.ui.set_local(id, property, value);
    }

    /// Writes a generated element's layout contribution.
    ///
    /// Only longhand contributions explicitly carried by the layout are
    /// written, so size/placement updates do not shadow styled spacing,
    /// alignment, or overflow.
    pub fn set_layout(&mut self, id: ElementId, layout: impl Into<Layout>) {
        let parts = layout.into().into_parts();
        if let Some(container) = parts.container {
            self.set_layout_container(id, container);
        }
        if let Some(size) = parts.size {
            self.set_layout_size(id, size);
        }
        if let Some(spacing) = parts.spacing {
            self.set_layout_spacing(id, spacing);
        }
        if let Some(alignment) = parts.alignment {
            self.set_layout_alignment(id, alignment);
        }
        if let Some(overflow) = parts.overflow {
            self.set_layout_overflow(id, overflow);
        }
        if let Some(placement) = parts.placement {
            self.set_layout_placement(id, placement);
        }
    }

    /// Writes a generated element's container-layout contribution.
    pub fn set_layout_container(&mut self, id: ElementId, container: impl Into<LayoutContainer>) {
        let property = self.properties().layout_container;
        self.set_local(id, property, container.into());
    }

    /// Writes a generated element's size contribution.
    pub fn set_layout_size(&mut self, id: ElementId, size: impl Into<LayoutSize>) {
        let property = self.properties().layout_size;
        self.set_local(id, property, size.into());
    }

    /// Writes a generated element's margin and child-gap contribution.
    pub fn set_layout_spacing(&mut self, id: ElementId, spacing: impl Into<LayoutSpacing>) {
        let property = self.properties().layout_spacing;
        self.set_local(id, property, spacing.into());
    }

    /// Writes a generated element's alignment contribution.
    pub fn set_layout_alignment(&mut self, id: ElementId, alignment: impl Into<LayoutAlignment>) {
        let property = self.properties().layout_alignment;
        self.set_local(id, property, alignment.into());
    }

    /// Writes a generated element's overflow contribution.
    pub fn set_layout_overflow(&mut self, id: ElementId, overflow: impl Into<LayoutOverflow>) {
        let property = self.properties().layout_overflow;
        self.set_local(id, property, overflow.into());
    }

    /// Writes a generated element's placement contribution.
    pub fn set_layout_placement(&mut self, id: ElementId, placement: impl Into<LayoutPlacement>) {
        let property = self.properties().layout_placement;
        self.set_local(id, property, placement.into());
    }

    /// Reads a local property value or returns `default`.
    ///
    /// This is intentionally local-only; template setup should not
    /// accidentally couple itself to cascade resolution.
    #[must_use]
    pub fn read_local_or<T: Clone + 'static>(
        &self,
        id: ElementId,
        property: Property<T>,
        default: T,
    ) -> T {
        self.ui
            .get(id)
            .and_then(|el| el.get_local_value(property).cloned())
            .unwrap_or(default)
    }

    /// Registers a one-way binding and records it on the retained instance.
    pub fn bind<T>(
        &mut self,
        source: PropertyEndpoint<BindingOwner, T>,
        target: PropertyEndpoint<BindingOwner, T>,
    ) -> Result<BindingId, BindingError<BindingOwner>>
    where
        T: Clone + 'static,
    {
        let id = self.ui.bind(source, target)?;
        self.instance.push_binding(id);
        Ok(id)
    }

    /// Binds an owner property into a generated part property.
    pub fn bind_owner_to_part<T>(
        &mut self,
        owner_property: Property<T>,
        part: ElementId,
        part_property: Property<T>,
    ) -> Result<BindingId, BindingError<BindingOwner>>
    where
        T: Clone + 'static,
    {
        self.bind(
            PropertyEndpoint::new(BindingOwner::element(self.owner), owner_property),
            PropertyEndpoint::new(BindingOwner::element(part), part_property),
        )
    }

    /// Registers a click handler on `id`.
    pub fn on_click<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Ui, ClickEvent<'_>) + 'static,
    {
        self.ui.on_control_click(id, handler);
    }

    /// Registers a drag handler on `id`.
    pub fn on_drag<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Ui, DragEvent<'_>) + 'static,
    {
        self.ui.on_control_drag(id, handler);
    }

    /// Registers a key handler on `id`.
    pub fn on_key<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Ui, KeyEvent<'_>) + 'static,
    {
        self.ui.on_control_key(id, handler);
    }
}
