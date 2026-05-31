// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Element tree mutation, template expansion, and element/runtime mirroring.

use alloc::boxed::Box;
use alloc::vec::Vec;

use understory_box_tree::NodeFlags;
use understory_style::PartTag;

use crate::channels::{COMPOSE, LAYOUT};
use crate::controls::panel::Panel;
use crate::element::ElementId;
use crate::template::{TemplateContext, TemplateInstance, TemplateSlot};
use crate::view::View;
use crate::widget::{AppendSpec, ContentSlotSpec, Widget};

use super::Ui;

/// Scoped authoring helper for adding children under one parent.
///
/// `ChildUi` does not own a separate tree or state model. It only remembers a
/// parent element and forwards to the same [`Ui::append`] primitive app code
/// can call directly. Custom controls remain first-class because [`Self::add`]
/// and [`Self::children`] accept any [`AppendSpec`](crate::AppendSpec).
#[derive(Debug)]
pub struct ChildUi<'a> {
    ui: &'a mut Ui,
    parent: ElementId,
}

impl<'a> ChildUi<'a> {
    pub(in crate::ui) fn new(ui: &'a mut Ui, parent: ElementId) -> Self {
        Self { ui, parent }
    }

    /// Returns the element new children are appended under.
    #[must_use]
    pub const fn parent(&self) -> ElementId {
        self.parent
    }

    /// Returns a shared reference to the underlying [`Ui`].
    #[must_use]
    pub fn ui(&self) -> &Ui {
        self.ui
    }

    /// Returns a mutable reference to the underlying [`Ui`].
    pub fn ui_mut(&mut self) -> &mut Ui {
        self.ui
    }

    /// Adds `spec` under this builder's parent.
    pub fn add<S>(&mut self, spec: S) -> ElementId
    where
        S: AppendSpec,
    {
        self.ui.append(self.parent, spec)
    }

    /// Adds `spec` under this builder's parent, then runs `f` with the
    /// inserted element id.
    ///
    /// Use this for element-level setup that is not part of the spec's builder
    /// surface yet, such as registering shared runtime state. Prefer ordinary
    /// builder methods for values that belong to the spec itself.
    pub fn add_with<S>(&mut self, spec: S, f: impl FnOnce(&mut Ui, ElementId)) -> ElementId
    where
        S: AppendSpec,
    {
        let id = self.add(spec);
        f(&mut *self.ui, id);
        id
    }

    /// Adds `spec`, scopes `f` to the inserted element's children, and returns
    /// the inserted element id with the closure's result.
    pub fn children<S, R>(
        &mut self,
        spec: S,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> (ElementId, R)
    where
        S: AppendSpec,
    {
        let child = self.add(spec);
        let result = self.ui.with_children(child, f);
        (child, result)
    }

    /// Scopes `f` to a template slot exposed by `owner`.
    ///
    /// This is the generic content-control escape hatch: controls can expose
    /// named slots without adding one-off `ChildUi` methods for every built-in
    /// or downstream widget.
    pub fn slot_children<R>(
        &mut self,
        owner: ElementId,
        slot: TemplateSlot,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> Option<R> {
        self.ui.slot_children(owner, slot, f)
    }

    /// Adds `spec`, scopes `f` to one of its template slots, and returns the
    /// inserted owner id with the optional closure result.
    ///
    /// The result is `None` when the inserted control's template does not
    /// expose `slot`, which can happen with custom templates.
    pub fn children_in_slot<S, R>(
        &mut self,
        spec: S,
        slot: TemplateSlot,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> (ElementId, Option<R>)
    where
        S: AppendSpec,
    {
        let child = self.add(spec);
        let result = self.slot_children(child, slot, f);
        (child, result)
    }

    /// Adds a content control, scopes `f` to its declared content slot, and
    /// returns the owner id with the optional closure result.
    ///
    /// The result is `None` when a custom template omits the declared content
    /// slot. Use [`Self::children_in_slot`] for controls with multiple named
    /// app-facing slots.
    pub fn content<S, R>(
        &mut self,
        spec: S,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> (ElementId, Option<R>)
    where
        S: ContentSlotSpec,
    {
        let child = self.add(spec);
        let result = self.slot_children(child, S::CONTENT_SLOT, f);
        (child, result)
    }

    /// Adds `panel`, scopes `f` to the inserted panel's children, and returns
    /// the panel id with the closure's result.
    pub fn panel<R>(
        &mut self,
        panel: Panel,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> (ElementId, R) {
        self.children(panel, f)
    }

    /// Adds a horizontal flex row with `gap` and scopes `f` to its children.
    pub fn row<R>(&mut self, gap: f32, f: impl FnOnce(&mut ChildUi<'_>) -> R) -> (ElementId, R) {
        self.panel(Panel::row(gap), f)
    }

    /// Adds a vertical flex column with `gap` and scopes `f` to its children.
    pub fn column<R>(&mut self, gap: f32, f: impl FnOnce(&mut ChildUi<'_>) -> R) -> (ElementId, R) {
        self.panel(Panel::column(gap), f)
    }

    /// Builds `view` under this builder's parent.
    ///
    /// Views are reusable authoring units, not runtime widgets. They receive
    /// the same scoped builder and append ordinary controls into the retained
    /// tree.
    pub fn view<V>(&mut self, view: V) -> V::Output
    where
        V: View,
    {
        view.build(self)
    }
}

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
    pub fn append<S: AppendSpec>(&mut self, parent: ElementId, spec: S) -> ElementId {
        spec.append_to(self, parent)
    }

    /// Runs `f` with a scoped builder that adds children under `parent`.
    ///
    /// This is an authoring convenience over [`Self::append`]. It preserves the
    /// same retained element ids, template expansion, style matching, and
    /// runtime mirroring as direct append calls.
    pub fn with_children<R>(
        &mut self,
        parent: ElementId,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> R {
        assert!(
            self.get(parent).is_some(),
            "with_children: parent id is stale"
        );
        let mut children = ChildUi::new(self, parent);
        f(&mut children)
    }

    /// Builds `view` under `parent` and returns the view's output.
    ///
    /// This is a convenience over [`Self::with_children`] for reusable
    /// app-authoring components.
    pub fn view<V>(&mut self, parent: ElementId, view: V) -> V::Output
    where
        V: View,
    {
        self.with_children(parent, |children| children.view(view))
    }

    /// Adds `spec` under `parent`, scopes `f` to the inserted element's
    /// children, and returns the inserted element id with the closure's result.
    pub fn children<S, R>(
        &mut self,
        parent: ElementId,
        spec: S,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> (ElementId, R)
    where
        S: AppendSpec,
    {
        let child = self.append(parent, spec);
        let result = self.with_children(child, f);
        (child, result)
    }

    /// Scopes `f` to a template slot exposed by `owner`.
    ///
    /// Returns `None` when `owner` is stale or the current template does not
    /// expose `slot`.
    pub fn slot_children<R>(
        &mut self,
        owner: ElementId,
        slot: TemplateSlot,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> Option<R> {
        let slot_child = self.slot(owner, slot)?;
        Some(self.with_children(slot_child, f))
    }

    /// Adds `spec` under `parent`, scopes `f` to one of its template slots, and
    /// returns the owner id with the optional closure result.
    ///
    /// The result is `None` when the inserted control's template does not
    /// expose `slot`, which can happen with custom templates.
    pub fn children_in_slot<S, R>(
        &mut self,
        parent: ElementId,
        spec: S,
        slot: TemplateSlot,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> (ElementId, Option<R>)
    where
        S: AppendSpec,
    {
        let child = self.append(parent, spec);
        let result = self.slot_children(child, slot, f);
        (child, result)
    }

    /// Adds a content control under `parent`, scopes `f` to its declared
    /// content slot, and returns the owner id with the optional closure result.
    ///
    /// The result is `None` when a custom template omits the declared content
    /// slot. Use [`Self::children_in_slot`] for controls with multiple named
    /// app-facing slots.
    pub fn content<S, R>(
        &mut self,
        parent: ElementId,
        spec: S,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> (ElementId, Option<R>)
    where
        S: ContentSlotSpec,
    {
        let child = self.append(parent, spec);
        let result = self.slot_children(child, S::CONTENT_SLOT, f);
        (child, result)
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
        self.remove_overlay_state_for(id);
        let _ = self.elements.remove_slot(id);
        self.style.remove(id);
        self.invalidation.remove_key(id);
        self.bindings.clear_owner(id);
        self.templates.remove_element(id);
        self.events.remove_target(id);
        self.timeline.remove_element(id);
        self.timers.remove_target(id);
        self.scrolling.remove_target(id);
        self.focus.remove_target(id);
        self.layout.remove_element(id);
        for node in self.geometry.remove_element(id) {
            self.presentation.remove_node(node);
        }
        self.composition.remove_element(id);
    }

    pub(in crate::ui) fn is_descendant_or_self(
        &self,
        descendant: ElementId,
        ancestor: ElementId,
    ) -> bool {
        let mut current = Some(descendant);
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            current = self.get(id).and_then(|element| element.parent());
        }
        false
    }

    pub(in crate::ui) fn mirror_allocated_element(
        &mut self,
        id: ElementId,
        parent: Option<ElementId>,
    ) {
        self.layout.insert_element(id, parent);
        let node = self.geometry.insert_element(id, parent);
        if let Some(element) = self.get(id)
            && let Some(widget) = element.widget()
            && !widget.is_hit_test_visible(element)
        {
            let mut flags = self.geometry.flags(node).unwrap_or_default();
            flags.remove(NodeFlags::PICKABLE);
            self.geometry.set_flags(node, flags);
        }
        self.presentation.insert_node(node, id);
        let policy = self
            .get(id)
            .and_then(|element| element.widget())
            .map_or_else(Default::default, |widget| widget.composition_policy());
        self.composition.insert_element(id, parent, policy);
    }
}
