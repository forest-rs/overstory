// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Element tree mutation, template expansion, and element/runtime mirroring.

use alloc::boxed::Box;
use alloc::vec::Vec;

use understory_style::PartTag;

use crate::channels::{COMPOSE, LAYOUT};
use crate::element::ElementId;
use crate::template::{TemplateContext, TemplateInstance, TemplateSlot};
use crate::widget::Widget;

use super::Ui;

impl Ui {
    /// Returns the retained template expansion for `owner`, if it has one.
    #[must_use]
    pub fn template_instance(&self, owner: ElementId) -> Option<&TemplateInstance> {
        self.templates.get(owner)
    }

    /// Returns the first template part registered for `owner + tag`.
    #[must_use]
    pub fn part(&self, owner: ElementId, tag: PartTag) -> Option<ElementId> {
        self.templates.part(owner, tag)
    }

    /// Returns the first template part registered for `owner + slot`.
    #[must_use]
    pub fn slot(&self, owner: ElementId, slot: TemplateSlot) -> Option<ElementId> {
        self.templates.slot(owner, slot)
    }

    /// Returns all template parts registered for `owner + tag`.
    pub fn parts(&self, owner: ElementId, tag: PartTag) -> impl Iterator<Item = ElementId> + '_ {
        self.templates.parts(owner, tag)
    }

    /// Returns all template parts registered for `owner + slot`.
    pub fn slots(
        &self,
        owner: ElementId,
        slot: TemplateSlot,
    ) -> impl Iterator<Item = ElementId> + '_ {
        self.templates.slots(owner, slot)
    }

    /// Inserts a new control under `parent`. The spec's builder
    /// methods produce its configuration; `append_to` reads those,
    /// creates the element + widget, registers handlers, and
    /// returns the new element id.
    ///
    /// This is the primary way for application code to add controls
    /// to the tree. See [`crate::Button`], [`crate::ToggleSwitch`],
    /// [`crate::Slider`], [`crate::Spinner`], [`crate::Panel`],
    /// [`crate::Text`].
    pub fn append<S: crate::widget::AppendSpec>(
        &mut self,
        parent: ElementId,
        spec: S,
    ) -> ElementId {
        spec.append_to(self, parent)
    }

    /// Lower-level entry point used by [`AppendSpec`](crate::widget::AppendSpec)
    /// implementations.
    ///
    /// Allocates a new element under `parent` carrying the supplied
    /// widget (or `None` for anonymous containers), runs the
    /// widget's default template if it has one, and returns the new
    /// id. Most callers should use [`Self::append`] instead.
    pub fn append_widget(&mut self, parent: ElementId, widget: Box<dyn Widget>) -> ElementId {
        self.insert_child_inner(parent, None, None, Some(widget), true)
    }

    pub(crate) fn append_widget_deferred_template(
        &mut self,
        parent: ElementId,
        widget: Box<dyn Widget>,
    ) -> ElementId {
        self.insert_child_inner(parent, None, None, Some(widget), false)
    }

    /// Inserts an anonymous container child. Used by tests + the
    /// occasional template that needs a no-widget slot; app code
    /// should prefer [`append(parent, Panel::new())`](Self::append).
    #[cfg(test)]
    pub(crate) fn insert_child(&mut self, parent: ElementId) -> ElementId {
        self.insert_child_inner(parent, None, None, None, true)
    }

    pub(crate) fn insert_template_child(
        &mut self,
        owner: ElementId,
        parent: ElementId,
        part_tag: Option<PartTag>,
        widget: Option<Box<dyn Widget>>,
    ) -> ElementId {
        assert!(
            self.get(owner).is_some(),
            "insert_template_child: owner id is stale"
        );
        self.insert_child_inner(parent, Some(owner), part_tag, widget, true)
    }

    fn insert_child_inner(
        &mut self,
        parent: ElementId,
        templated_parent: Option<ElementId>,
        part_tag: Option<PartTag>,
        widget: Option<Box<dyn Widget>>,
        expand_template: bool,
    ) -> ElementId {
        assert!(
            self.get(parent).is_some(),
            "insert_child: parent id is stale"
        );
        let child = self
            .elements
            .insert_child(parent, templated_parent, part_tag, widget);
        self.mirror_allocated_element(child, Some(parent));
        self.style_one(child);
        // LAYOUT cascades to PAINT and COMPOSE via the tracker's
        // channel cascade. `style_one`'s diff path only marks
        // channels on state transitions — first-time styling skips
        // the diff, so without this explicit mark a cascade-supplied
        // `Layout` would sit in the cascade but never reach the layout engine.
        self.invalidation.mark(child, LAYOUT);
        self.invalidation.mark(parent, COMPOSE);
        if expand_template {
            self.expand_default_template(child);
        }
        child
    }

    /// Expands the widget's default template under `owner`, if any.
    ///
    /// Template-generated elements are inserted through [`TemplateContext`],
    /// which records named parts and applies the owner back-reference
    /// explicitly.
    pub(crate) fn expand_default_template(&mut self, owner: ElementId) {
        let Some(element) = self.get(owner) else {
            return;
        };
        let Some(template) = element.widget().and_then(|w| w.default_template()) else {
            return;
        };
        let mut cx = TemplateContext::new(self, owner);
        template.instantiate(&mut cx);
        let instance = cx.finish();
        self.templates.insert(owner, instance);
    }

    /// Removes `id` and all of its descendants from the tree.
    ///
    /// # Panics
    ///
    /// Panics if `id` is the root or stale.
    pub fn remove(&mut self, id: ElementId) {
        assert!(id != self.root(), "remove: cannot remove the tree root");
        assert!(self.get(id).is_some(), "remove: id is stale");

        if let Some(parent) = self.elements.detach_from_parent(id) {
            self.invalidation.mark(parent, COMPOSE);
        }
        self.remove_subtree(id);
    }

    fn remove_subtree(&mut self, id: ElementId) {
        let children: Vec<ElementId> = self
            .get(id)
            .map(|e| e.children().to_vec())
            .unwrap_or_default();
        for child in children {
            self.remove_subtree(child);
        }
        let _ = self.elements.remove_slot(id);
        self.style.remove(id);
        self.invalidation.remove_key(id);
        self.bindings.clear_owner(id);
        self.templates.remove_element(id);
        self.events.remove_target(id);
        self.timers.remove_target(id);
        self.focus.remove_target(id);
        self.layout.remove_element(id);
        for node in self.geometry.remove_element(id) {
            self.presentation.remove_node(node);
        }
        self.composition.remove_element(id);
    }

    pub(in crate::ui) fn mirror_allocated_element(
        &mut self,
        id: ElementId,
        parent: Option<ElementId>,
    ) {
        self.layout.insert_element(id, parent);
        let node = self.geometry.insert_element(id, parent);
        self.presentation.insert_node(node, id);
        self.composition.insert_element(id, parent);
    }
}
