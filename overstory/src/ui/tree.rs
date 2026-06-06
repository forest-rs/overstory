// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Element tree mutation, template expansion, and element/runtime mirroring.

use alloc::boxed::Box;
use alloc::vec::Vec;

use understory_box_tree::NodeFlags;
use understory_style::PartTag;

use crate::binding::{BindingOwner, LocalState};
use crate::channels::{COMPOSE, LAYOUT, PAINT};
use crate::controls::panel::Panel;
use crate::element::ElementId;
use crate::template::{
    ControlTemplate, TemplateContext, TemplateExpansionReport, TemplateInstance, TemplateItemKey,
    TemplateSlot,
};
use crate::view::View;
use crate::widget::{AppendSpec, ContentSlotSpec, Widget};

use super::{Ui, collect_preorder};

struct PreservedSlotChildren {
    slot: TemplateSlot,
    key: Option<TemplateItemKey>,
    children: Vec<ElementId>,
}

/// Scoped authoring helper for adding children under one parent.
///
/// `ChildUi` does not own a separate tree or state model. It only remembers a
/// parent element and forwards to the same [`Ui::append`] primitive app code
/// can call directly. Custom controls remain first-class because [`Self::add`],
/// [`Self::with_child`], [`Self::add_with_children`], and [`Self::children`]
/// accept any [`AppendSpec`](crate::AppendSpec).
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

    /// Creates a local state value owned by this builder's underlying [`Ui`].
    ///
    /// "Local" means local to the retained UI tree, not to this builder
    /// closure. This forwards to [`Ui::local_state`] so view builders can
    /// create bindable local values without escaping from the scoped authoring
    /// API.
    pub fn local_state<T>(&mut self, initial: T) -> LocalState<T>
    where
        T: Clone + PartialEq + 'static,
    {
        self.ui.local_state(initial)
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

    /// Adds `spec`, scopes `f` to the inserted element's children, and returns
    /// only the closure's result.
    ///
    /// Use this when lexical parentage matters but the inserted container id
    /// does not. Use [`Self::children`] when the caller needs both the owner id
    /// and the nested result.
    pub fn with_child<S, R>(&mut self, spec: S, f: impl FnOnce(&mut ChildUi<'_>) -> R) -> R
    where
        S: AppendSpec,
    {
        self.children(spec, f).1
    }

    /// Adds `spec`, scopes `f` to the inserted element's children, and returns
    /// the inserted element id.
    ///
    /// Use this when the nested body only populates the child. Use
    /// [`Self::children`] when the caller needs both the owner id and a value
    /// produced by the nested body.
    pub fn add_with_children<S>(&mut self, spec: S, f: impl FnOnce(&mut ChildUi<'_>)) -> ElementId
    where
        S: AppendSpec,
    {
        self.children(spec, f).0
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

    /// Returns the control contract report for `owner`'s expanded template.
    ///
    /// Non-templated elements, stale ids, and widgets with empty template
    /// contracts return `None`. The report combines the widget-declared
    /// contract, the retained template instance, and expansion diagnostics.
    #[must_use]
    pub fn template_expansion_report(&self, owner: ElementId) -> Option<TemplateExpansionReport> {
        let contract = self.get(owner)?.widget()?.template_contract();
        if contract.is_empty() {
            return None;
        }
        Some(contract.validate_expansion(self.template_instance(owner)))
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

    /// Adds `spec` under `parent`, scopes `f` to the inserted element's
    /// children, and returns only the closure's result.
    ///
    /// Use this when the inserted container id is not part of the caller's
    /// result. Use [`Self::children`] when the caller needs both the owner id
    /// and the nested result.
    pub fn with_child<S, R>(
        &mut self,
        parent: ElementId,
        spec: S,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> R
    where
        S: AppendSpec,
    {
        self.children(parent, spec, f).1
    }

    /// Adds `spec` under `parent`, scopes `f` to the inserted element's
    /// children, and returns the inserted element id.
    ///
    /// Use this when the nested body only populates the child. Use
    /// [`Self::children`] when the caller needs both the owner id and a value
    /// produced by the nested body.
    pub fn add_with_children<S>(
        &mut self,
        parent: ElementId,
        spec: S,
        f: impl FnOnce(&mut ChildUi<'_>),
    ) -> ElementId
    where
        S: AppendSpec,
    {
        self.children(parent, spec, f).0
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
    /// widget (or `None` for anonymous containers), expands its resolved
    /// `ControlTemplate` if it has one, and returns the new id. Most callers
    /// should use [`Self::append`] instead.
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
        let previous_sibling = self
            .get(parent)
            .and_then(|element| element.children().iter().rev().nth(1).copied());
        self.mirror_allocated_element(child, Some(parent));
        self.style_one_for_insert(child);
        // LAYOUT cascades to GEOMETRY, PAINT, and COMPOSE via the tracker's
        // channel cascade. `style_one`'s diff path only marks
        // channels on state transitions — first-time styling skips
        // the diff, so without this explicit mark a cascade-supplied
        // `Layout` would sit in the cascade but never reach the layout engine.
        self.invalidation.mark(child, LAYOUT);
        self.invalidation.mark(parent, COMPOSE);
        self.mark_child_dependent_presentation(parent);
        if expand_template {
            self.expand_resolved_template(child);
        }
        self.record_element_tree_upsert(parent);
        if let Some(previous_sibling) = previous_sibling {
            self.record_element_tree_upsert(previous_sibling);
        }
        self.record_element_tree_upsert(child);
        child
    }

    /// Expands the currently resolved control template under `owner`, if any.
    ///
    /// The template may come from the `ControlTemplate` property via local
    /// value, style, or theme default. During the migration to fully
    /// property-driven templates, widgets can still provide a fallback default
    /// template when no property value resolves.
    pub(crate) fn expand_resolved_template(&mut self, owner: ElementId) {
        let Some(template) = self.resolved_control_template(owner) else {
            return;
        };
        self.expand_template(owner, template);
    }

    pub(crate) fn retemplate_if_needed(&mut self, owner: ElementId) {
        let current_template_id = self.templates.get(owner).map(TemplateInstance::template_id);
        let template = self.resolved_control_template(owner);

        match (current_template_id, template) {
            (None, None) => {}
            (None, Some(template)) => {
                self.expand_template(owner, template);
                self.with_widget_mut(owner, |widget, ui| widget.template_expanded(owner, ui));
            }
            (Some(_), None) => {
                if self.get(owner).is_some_and(|element| {
                    element.has_widget_identity() && element.widget().is_none()
                }) {
                    return;
                }
                self.remove_template_instance(owner);
            }
            (Some(current_template_id), Some(template)) => {
                if current_template_id != template.id() {
                    self.replace_template(owner, template);
                }
            }
        }
    }

    fn resolved_control_template(&self, owner: ElementId) -> Option<ControlTemplate> {
        let element = self.get(owner)?;
        self.resolve_value_for(element, self.properties.control_template)
    }

    fn expand_template(&mut self, owner: ElementId, template: ControlTemplate) {
        let template_id = template.id();
        let mut cx = TemplateContext::new(self, owner, template_id);
        template.instantiate(&mut cx);
        let instance = cx.finish();
        self.templates.insert(owner, instance);
    }

    fn replace_template(&mut self, owner: ElementId, template: ControlTemplate) {
        let preserved = self.detach_authored_slot_children(owner);
        self.remove_template_instance(owner);
        self.expand_template(owner, template);
        self.reattach_authored_slot_children(owner, preserved);
        self.with_widget_mut(owner, |widget, ui| widget.template_expanded(owner, ui));
    }

    fn remove_template_instance(&mut self, owner: ElementId) {
        let roots = self.template_root_children(owner);
        for root in roots {
            self.remove(root);
        }
        self.templates.remove_instance(owner);
    }

    fn template_root_children(&self, owner: ElementId) -> Vec<ElementId> {
        self.get(owner)
            .map(|element| {
                element
                    .children()
                    .iter()
                    .copied()
                    .filter(|child| {
                        self.get(*child)
                            .is_some_and(|child| child.templated_parent() == Some(owner))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn detach_authored_slot_children(&mut self, owner: ElementId) -> Vec<PreservedSlotChildren> {
        let parts = self
            .templates
            .get(owner)
            .map(|instance| instance.part_records().to_vec())
            .unwrap_or_default();
        let mut preserved = Vec::new();
        for part in parts {
            let Some(slot) = part.slot() else {
                continue;
            };
            let children = self
                .get(part.element())
                .map(|slot_element| {
                    slot_element
                        .children()
                        .iter()
                        .copied()
                        .filter(|child| {
                            self.get(*child)
                                .is_some_and(|child| child.templated_parent() != Some(owner))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            for child in &children {
                let _ = self.elements.detach_from_parent(*child);
            }
            if !children.is_empty() {
                preserved.push(PreservedSlotChildren {
                    slot,
                    key: part.key(),
                    children,
                });
            }
        }
        preserved
    }

    fn reattach_authored_slot_children(
        &mut self,
        owner: ElementId,
        preserved: Vec<PreservedSlotChildren>,
    ) {
        for preserved in preserved {
            let target = self.templates.get(owner).and_then(|instance| {
                preserved
                    .key
                    .and_then(|key| instance.keyed_slot(preserved.slot, key))
                    .or_else(|| instance.slot(preserved.slot))
            });
            let target = target.unwrap_or(owner);
            for child in preserved.children {
                let _ = self.elements.move_child(child, target);
                self.invalidation.mark(target, COMPOSE);
                self.mark_child_dependent_presentation(target);
                self.record_element_tree_upsert(child);
            }
            self.record_element_tree_upsert(target);
        }
    }

    /// Removes `id` and all of its descendants from the tree.
    ///
    /// # Panics
    ///
    /// Panics if `id` is the root or stale.
    pub fn remove(&mut self, id: ElementId) {
        assert!(id != self.root(), "remove: cannot remove the tree root");
        assert!(self.get(id).is_some(), "remove: id is stale");

        let parent = self.get(id).and_then(crate::element::Element::parent);
        let previous_sibling = parent.and_then(|parent| {
            self.get(parent)
                .and_then(|parent| parent.children().iter().position(|child| *child == id))
                .and_then(|index| index.checked_sub(1))
                .and_then(|index| {
                    self.get(parent)
                        .and_then(|parent| parent.children().get(index))
                })
                .copied()
        });
        let removed = collect_preorder(self, id)
            .into_iter()
            .filter_map(|id| self.get(id).map(|element| (id, element.parent())))
            .collect::<Vec<_>>();

        if let Some(parent) = self.elements.detach_from_parent(id) {
            self.invalidation.mark(parent, COMPOSE);
            self.mark_child_dependent_presentation(parent);
        }
        self.remove_subtree(id);
        for (id, parent) in removed {
            self.record_element_tree_removal(id, parent);
        }
        if let Some(parent) = parent {
            self.record_element_tree_upsert(parent);
        }
        if let Some(previous_sibling) = previous_sibling {
            self.record_element_tree_upsert(previous_sibling);
        }
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
        self.bindings.clear_owner(BindingOwner::element(id));
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

    fn mark_child_dependent_presentation(&mut self, id: ElementId) {
        let depends_on_children = self
            .get(id)
            .and_then(|element| {
                element
                    .widget()
                    .map(|widget| widget.presentation_depends_on_children(element))
            })
            .unwrap_or(false);
        if depends_on_children {
            self.invalidation.mark(id, PAINT);
        }
    }

    pub(in crate::ui) fn template_owner_for_presented_descendant(
        &self,
        descendant: ElementId,
    ) -> Option<ElementId> {
        let mut current = self
            .get(descendant)
            .and_then(crate::element::Element::parent);
        while let Some(id) = current {
            let element = self.get(id)?;
            if let Some(owner) = element.templated_parent()
                && element
                    .widget()
                    .is_some_and(|widget| widget.routes_descendants_to_template_owner(element))
            {
                return Some(owner);
            }
            current = element.parent();
        }
        None
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
