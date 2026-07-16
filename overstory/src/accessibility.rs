// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! AccessKit tree lowering for the retained Overstory element tree.
//!
//! This module owns the deterministic mapping from Overstory elements and
//! dependency properties to AccessKit nodes. It does not own platform adapter
//! activation, OS event plumbing, or assistive-technology lifecycle; host
//! crates bridge these snapshots into the native AccessKit adapter.
//!
//! First slice:
//! - Emit full-tree [`accesskit::TreeUpdate`] snapshots.
//! - Use AccessKit's own role/action/value vocabulary directly.
//! - Support focus, click, and coarse range actions for built-in controls.

use accesskit::{
    Action, ActionRequest, Affine, Node, NodeId, Rect as AccessRect, Role, Toggled, Tree, TreeId,
    TreeUpdate,
};
use alloc::{string::String, vec::Vec};
use understory_overlay::OverlayBehavior;
use understory_property::Property;

use crate::behavior::WidgetEventContext;
use crate::element::{Element, ElementId};
use crate::properties::BuiltInProperties;
use crate::pseudos::PSEUDO_SELECTED;
use crate::template::TemplateSlot;
use crate::ui::Ui;

/// Read-only context handed to widget accessibility hooks.
///
/// This context exposes resolved semantic state without requiring widgets to
/// know where Overstory stores built-in dependency properties or pseudoclasses.
#[derive(Copy, Clone)]
pub struct AccessibilityContext<'a> {
    ui: &'a Ui,
    scale_factor: f64,
}

impl<'a> AccessibilityContext<'a> {
    fn new(ui: &'a Ui, scale_factor: f64) -> Self {
        Self {
            ui,
            scale_factor: sane_scale_factor(scale_factor),
        }
    }

    /// Returns the device scale factor used for this tree snapshot.
    #[must_use]
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    /// Returns the built-in property handles for resolved semantic reads.
    #[must_use]
    pub fn properties(&self) -> &BuiltInProperties {
        self.ui.properties()
    }

    /// Resolves `property` for `element` through the active cascade.
    #[must_use]
    pub fn get_value<T>(&self, element: &Element, property: Property<T>) -> T
    where
        T: Clone + 'static,
    {
        self.ui.resolve_value_for(element, property)
    }

    /// Returns a live element by id.
    #[must_use]
    pub fn element(&self, id: ElementId) -> Option<&Element> {
        self.ui.get(id)
    }

    pub(crate) fn has_active_overlay_for(
        &self,
        target: ElementId,
        behavior: OverlayBehavior,
    ) -> bool {
        self.ui.active_overlay_for(target, behavior).is_some()
    }

    pub(crate) fn active_overlay_for(
        &self,
        target: ElementId,
        behavior: OverlayBehavior,
    ) -> Option<ElementId> {
        self.ui.active_overlay_for(target, behavior)
    }

    pub(crate) fn active_overlay_content_for(
        &self,
        target: ElementId,
        behavior: OverlayBehavior,
    ) -> Option<ElementId> {
        self.ui.active_overlay_content_for(target, behavior)
    }

    pub(crate) fn active_descendant_for_overlay_content(
        &self,
        content: ElementId,
    ) -> Option<ElementId> {
        self.ui.active_descendant_for_overlay_content(content)
    }

    /// Returns the latest computed bounds for `id`.
    #[must_use]
    pub fn bounds(&self, id: ElementId) -> kurbo::Rect {
        self.ui.effective_bounds(id)
    }

    /// Returns the AccessKit node id used for `id` in this tree.
    #[must_use]
    pub fn node_id(&self, id: ElementId) -> NodeId {
        accesskit_node_id(id)
    }

    /// Resolves the accessible text associated with `element`.
    ///
    /// Controls expose semantic labels through `Content`; text-rendering
    /// elements expose drawn strings through `Text`. Accessibility uses content
    /// first because it is the control owner's semantic label, then falls back
    /// to rendered text for leaf text widgets and template-owned text parts.
    #[must_use]
    pub fn content(&self, element: &Element) -> String {
        let content = self.get_value(element, self.ui.properties().content);
        if content.is_empty() {
            self.get_value(element, self.ui.properties().text)
        } else {
            content
        }
    }

    /// Returns whether `id` is currently eligible for focus.
    #[must_use]
    pub fn is_focusable(&self, id: ElementId) -> bool {
        self.ui.is_focusable(id)
    }

    /// Returns whether `id` is currently enabled.
    #[must_use]
    pub fn is_enabled(&self, id: ElementId) -> bool {
        self.ui.is_enabled(id)
    }

    /// Resolves the checked state associated with `element`.
    #[must_use]
    pub fn is_checked(&self, element: &Element) -> bool {
        self.get_value(element, self.ui.properties().is_checked)
    }

    /// Resolves the numeric value associated with `element`.
    #[must_use]
    pub fn numeric_value(&self, element: &Element) -> f64 {
        self.get_value(element, self.ui.properties().value)
    }

    /// Resolves the minimum numeric value associated with `element`.
    #[must_use]
    pub fn minimum_value(&self, element: &Element) -> f64 {
        self.get_value(element, self.ui.properties().minimum)
    }

    /// Resolves the maximum numeric value associated with `element`.
    #[must_use]
    pub fn maximum_value(&self, element: &Element) -> f64 {
        self.get_value(element, self.ui.properties().maximum)
    }

    /// Resolves the small-change step associated with `element`.
    #[must_use]
    pub fn small_change(&self, element: &Element) -> f64 {
        self.get_value(element, self.ui.properties().small_change)
    }

    /// Resolves the large-change step associated with `element`.
    #[must_use]
    pub fn large_change(&self, element: &Element) -> f64 {
        self.get_value(element, self.ui.properties().large_change)
    }

    /// Returns whether `element` currently carries selected state.
    #[must_use]
    pub fn is_selected(&self, element: &Element) -> bool {
        element.pseudos().contains(&PSEUDO_SELECTED)
    }

    /// Resolves read-only state associated with `element`.
    #[must_use]
    pub fn is_read_only(&self, element: &Element) -> bool {
        self.get_value(element, self.ui.properties().is_read_only)
    }

    /// Adds the standard click action to `node`.
    pub fn add_click_action(&self, node: &mut Node) {
        let _ = self;
        node.add_action(Action::Click);
    }

    /// Adds the standard click action to `node` when `element` is enabled.
    pub fn add_click_action_if_enabled(&self, element: &Element, node: &mut Node) {
        if self.is_enabled(element.id()) {
            self.add_click_action(node);
        }
    }

    /// Applies checked state from Overstory's built-in checked property.
    pub fn apply_checked_state(&self, element: &Element, node: &mut Node) {
        node.set_toggled(Toggled::from(self.is_checked(element)));
    }

    /// Applies selected state from Overstory's selected pseudoclass.
    pub fn apply_selected_state(&self, element: &Element, node: &mut Node) {
        if self.is_selected(element) {
            node.set_selected(true);
        }
    }

    /// Applies numeric range state from Overstory's built-in range properties.
    pub fn apply_numeric_range(&self, element: &Element, node: &mut Node) {
        node.set_numeric_value(self.numeric_value(element));
        node.set_min_numeric_value(self.minimum_value(element));
        node.set_max_numeric_value(self.maximum_value(element));
        let small = self.small_change(element);
        if small > 0.0 {
            node.set_numeric_value_step(small);
        }
        let large = self.large_change(element);
        if large > 0.0 {
            node.set_numeric_value_jump(large);
        }
    }

    /// Appends authored children from `owner`'s template `slot` to `node`.
    ///
    /// The core accessibility traversal skips template-owned visual parts.
    /// Templated controls use this helper when a slot is semantically part of
    /// the owner, such as expanded disclosure content.
    pub fn append_slot_children(
        &self,
        owner: ElementId,
        slot: TemplateSlot,
        node: &mut Node,
        nodes: &mut Vec<(NodeId, Node)>,
    ) {
        let Some(slot_host) = self.ui.slot(owner, slot) else {
            return;
        };
        let Some(slot_element) = self.ui.get(slot_host) else {
            return;
        };
        for child in self.ui.accessibility_children(slot_element) {
            node.push_child(accesskit_node_id(child));
            self.ui.push_accesskit_node(child, self, nodes);
        }
    }
}

impl core::fmt::Debug for AccessibilityContext<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AccessibilityContext")
            .field("scale_factor", &self.scale_factor)
            .finish_non_exhaustive()
    }
}

impl Ui {
    /// Builds a full AccessKit tree update for the current retained UI tree.
    ///
    /// This first slice intentionally emits a complete snapshot every time.
    /// Host adapters can call it lazily when platform accessibility activates
    /// and after frames that may have changed semantics, focus, or geometry.
    #[must_use]
    pub fn accesskit_tree_update(&self) -> TreeUpdate {
        self.accesskit_tree_update_with_scale_factor(1.0)
    }

    /// Builds a full AccessKit tree update for a host scale factor.
    ///
    /// Overstory layout coordinates are logical pixels. AccessKit expects final
    /// transformed coordinates to be physical pixels, so this records
    /// `scale_factor` as the root node transform instead of rewriting every
    /// element's logical bounds.
    #[must_use]
    pub fn accesskit_tree_update_with_scale_factor(&self, scale_factor: f64) -> TreeUpdate {
        let root = self.root();
        let ctx = AccessibilityContext::new(self, scale_factor);
        let mut nodes = Vec::new();
        self.push_accesskit_node(root, &ctx, &mut nodes);
        TreeUpdate {
            nodes,
            tree: Some(accesskit_tree(root)),
            tree_id: TreeId::ROOT,
            focus: self
                .focused()
                .filter(|id| self.get(*id).is_some())
                .map(accesskit_node_id)
                .unwrap_or_else(|| accesskit_node_id(root)),
        }
    }

    /// Handles an AccessKit action request against the current UI tree.
    ///
    /// Returns `true` when the request mapped to an Overstory action. Unknown
    /// actions, stale nodes, and non-root AccessKit subtrees are ignored.
    pub fn handle_accesskit_action(&mut self, request: &ActionRequest) -> bool {
        if request.target_tree != TreeId::ROOT {
            return false;
        }
        let Some(target) = element_id_from_accesskit_node_id(request.target_node) else {
            return false;
        };
        if self.get(target).is_none() {
            return false;
        }

        match request.action {
            Action::Focus => self.set_focus(Some(target)),
            Action::Blur if self.focused() == Some(target) => self.set_focus(None),
            Action::Blur => false,
            _ => self.perform_accesskit_action(target, request),
        }
    }

    fn push_accesskit_node(
        &self,
        id: ElementId,
        ctx: &AccessibilityContext<'_>,
        nodes: &mut Vec<(NodeId, Node)>,
    ) {
        let Some(element) = self.get(id) else {
            return;
        };
        let children = self.accessibility_children(element);
        let mut node = self.accesskit_node_for(element, ctx);
        node.set_children(
            children
                .iter()
                .copied()
                .map(accesskit_node_id)
                .collect::<Vec<_>>(),
        );
        if let Some(widget) = element.widget() {
            widget.append_accessibility_children(element, ctx, &mut node, nodes);
        }
        nodes.push((accesskit_node_id(id), node));
        for child in children {
            self.push_accesskit_node(child, ctx, nodes);
        }
    }

    fn accesskit_node_for(&self, element: &Element, ctx: &AccessibilityContext<'_>) -> Node {
        let id = element.id();
        let role = accesskit_role(element, ctx, id == self.root());
        let mut node = Node::new(role);
        if id == self.root() && ctx.scale_factor() != 1.0 {
            node.set_transform(Affine::scale(ctx.scale_factor()));
        }
        node.set_bounds(accesskit_bounds(self.effective_bounds(id)));

        if self.is_focusable(id) {
            node.add_action(Action::Focus);
        }
        if !self.is_enabled(id) {
            node.set_disabled();
        }

        let content = ctx.content(element);
        apply_accesskit_label_or_value(&mut node, role, content);
        if let Some(widget) = element.widget() {
            widget.configure_accessibility(element, ctx, &mut node);
        }
        node
    }

    fn accessibility_children(&self, element: &Element) -> Vec<ElementId> {
        element
            .children()
            .iter()
            .copied()
            .filter(|child| {
                self.is_effectively_visible(*child)
                    && self
                        .get(*child)
                        .is_some_and(|child| child.templated_parent().is_none())
            })
            .collect()
    }

    fn dispatch_accesskit_widget_action(
        &mut self,
        target: ElementId,
        request: &ActionRequest,
    ) -> bool {
        let mut handled = false;
        self.with_widget_mut(target, |widget, ui| {
            let mut ctx = WidgetEventContext::new(target, ui);
            handled = widget.handle_accessibility_action(&mut ctx, request);
        });
        handled
    }

    fn perform_accesskit_click(&mut self, target: ElementId) -> bool {
        let clickable = self
            .get(target)
            .map(|element| {
                let ctx = AccessibilityContext::new(self, 1.0);
                is_clickable_role(accesskit_role(element, &ctx, target == self.root()))
            })
            .unwrap_or(false);
        if !clickable || !self.is_enabled(target) {
            return false;
        }
        let point = self.effective_bounds(target).center();
        let _ = self.synthesize_click(target, point);
        true
    }

    fn perform_accesskit_action(&mut self, target: ElementId, request: &ActionRequest) -> bool {
        if self.dispatch_accesskit_widget_action(target, request) {
            return true;
        }
        match request.action {
            Action::Click => self.perform_accesskit_click(target),
            _ => false,
        }
    }
}

fn accesskit_tree(root: ElementId) -> Tree {
    let mut tree = Tree::new(accesskit_node_id(root));
    tree.toolkit_name = Some(String::from("Overstory"));
    tree.toolkit_version = Some(String::from(env!("CARGO_PKG_VERSION")));
    tree
}

fn accesskit_role(element: &Element, ctx: &AccessibilityContext<'_>, is_root: bool) -> Role {
    if is_root {
        return Role::Window;
    }
    element
        .widget()
        .and_then(|widget| widget.accessibility_role(element, ctx))
        .unwrap_or(Role::GenericContainer)
}

fn apply_accesskit_label_or_value(node: &mut Node, role: Role, content: String) {
    if content.is_empty() {
        return;
    }
    if role == Role::Label || role == Role::TextRun {
        node.set_value(content);
    } else {
        node.set_label(content);
    }
}

fn is_clickable_role(role: Role) -> bool {
    matches!(
        role,
        Role::Button
            | Role::CheckBox
            | Role::ComboBox
            | Role::ListBoxOption
            | Role::MenuItem
            | Role::Switch
    )
}

pub(crate) fn accesskit_node_id(id: ElementId) -> NodeId {
    NodeId((u64::from(id.generation()) << 32) | u64::from(id.slot()))
}

fn sane_scale_factor(scale_factor: f64) -> f64 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}

fn element_id_from_accesskit_node_id(id: NodeId) -> Option<ElementId> {
    let raw = id.0;
    let slot = u32::try_from(raw & u64::from(u32::MAX)).ok()?;
    let generation = u32::try_from(raw >> 32).ok()?;
    Some(ElementId::new(slot, generation))
}

fn accesskit_bounds(bounds: kurbo::Rect) -> AccessRect {
    AccessRect {
        x0: bounds.x0,
        y0: bounds.y0,
        x1: bounds.x1,
        y1: bounds.y1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Button, CheckBox, Layout, Slider, Text, ToggleSwitch, Widget};
    use accesskit::{ActionData, Orientation};
    use alloc::boxed::Box;
    use kurbo::Size;
    use understory_property::DependencyObjectExt;

    #[test]
    fn accesskit_tree_includes_roles_labels_bounds_and_focus() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(240.0, 120.0));
        let button = ui.append(root, Button::new("Deploy"));
        ui.append(root, Text::new("Status"));
        assert!(ui.set_focus(Some(button)));
        ui.compute_layout(Size::new(240.0, 120.0));

        let update = ui.accesskit_tree_update();
        let button_node = update
            .nodes
            .iter()
            .find(|(id, _)| *id == accesskit_node_id(button))
            .map(|(_, node)| node)
            .expect("button node should be emitted");

        assert_eq!(
            update.tree.as_ref().map(|tree| tree.root),
            Some(accesskit_node_id(root))
        );
        assert_eq!(update.focus, accesskit_node_id(button));
        assert_eq!(button_node.role(), Role::Button);
        assert_eq!(button_node.label(), Some("Deploy"));
        assert!(button_node.supports_action(Action::Click));
        assert!(button_node.bounds().is_some());
    }

    #[test]
    fn accesskit_tree_skips_presentational_content_presenters() {
        let mut ui = Ui::new();
        let root = ui.root();
        let button = ui.append(root, Button::new("Deploy"));
        let label = ui
            .slot(button, crate::SLOT_BUTTON_CONTENT)
            .expect("button template should expose label slot");
        ui.compute_layout(Size::new(240.0, 120.0));

        let update = ui.accesskit_tree_update();

        assert_eq!(node_for(&update, button).label(), Some("Deploy"));
        assert!(
            update
                .nodes
                .iter()
                .all(|(id, _node)| *id != accesskit_node_id(label)),
            "content presenter should remain presentational in the AccessKit tree"
        );
    }

    #[test]
    fn accesskit_tree_exposes_checked_and_range_state() {
        let mut ui = Ui::new();
        let root = ui.root();
        let checkbox = ui.append(root, CheckBox::new("Trace").checked(true));
        let slider = ui.append(root, Slider::new().value(0.4).min(0.0).max(2.0));
        ui.compute_layout(Size::new(240.0, 120.0));

        let update = ui.accesskit_tree_update();
        let checkbox_node = node_for(&update, checkbox);
        let slider_node = node_for(&update, slider);

        assert_eq!(checkbox_node.role(), Role::CheckBox);
        assert_eq!(checkbox_node.toggled(), Some(Toggled::True));
        assert_eq!(slider_node.role(), Role::Slider);
        assert_eq!(slider_node.orientation(), Some(Orientation::Horizontal));
        assert_eq!(slider_node.value(), Some("0.4"));
        assert_eq!(slider_node.numeric_value(), Some(0.4));
        assert_eq!(slider_node.min_numeric_value(), Some(0.0));
        assert_eq!(slider_node.max_numeric_value(), Some(2.0));
        assert!(slider_node.supports_action(Action::Increment));
        assert!(slider_node.supports_action(Action::Decrement));
        assert!(slider_node.supports_action(Action::SetValue));
    }

    #[test]
    fn slider_accessibility_value_text_matches_axis_labels() {
        let mut ui = Ui::new();
        let root = ui.root();
        let unit_slider = ui.append(root, Slider::new().value(0.42).min(0.0).max(1.0));
        let axis_slider = ui.append(root, Slider::new().value(32.0).min(-16.0).max(64.0));
        ui.compute_layout(Size::new(240.0, 120.0));

        let update = ui.accesskit_tree_update();

        assert_eq!(node_for(&update, unit_slider).value(), Some("0.42"));
        assert_eq!(node_for(&update, axis_slider).value(), Some("32"));
    }

    #[test]
    fn disabled_controls_do_not_advertise_click_actions() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(240.0, 160.0));
        let button = ui.append(root, Button::new("Disabled button").disabled());
        let checkbox = ui.append(root, CheckBox::new("Disabled check").disabled());
        let toggle = ui.append(root, ToggleSwitch::new().disabled());
        ui.compute_layout(Size::new(240.0, 160.0));

        let update = ui.accesskit_tree_update();

        assert!(!node_for(&update, button).supports_action(Action::Click));
        assert!(!node_for(&update, checkbox).supports_action(Action::Click));
        assert!(!node_for(&update, toggle).supports_action(Action::Click));
        assert!(!ui.handle_accesskit_action(&ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: accesskit_node_id(button),
            data: None,
        }));
    }

    #[test]
    fn accesskit_slider_set_value_action_updates_range_value() {
        let mut ui = Ui::new();
        let root = ui.root();
        let slider = ui.append(root, Slider::new().value(0.4).min(0.0).max(2.0));

        let handled =
            ui.handle_accesskit_action(&action_with_numeric_value(slider, Action::SetValue, 1.5));

        assert!(handled);
        assert_eq!(
            ui.get(slider)
                .and_then(|el| el.get_local_value(ui.properties().value))
                .copied(),
            Some(1.5)
        );
    }

    #[test]
    fn accesskit_role_comes_from_widget_hook_not_type_tag() {
        let mut ui = Ui::new();
        let root = ui.root();
        let custom = ui.append_widget(root, Box::new(SemanticOnlyWidget));
        ui.set_local(
            custom,
            ui.properties().content,
            String::from("Custom action"),
        );
        ui.compute_layout(Size::new(100.0, 40.0));

        let update = ui.accesskit_tree_update();
        let custom_node = node_for(&update, custom);

        assert_eq!(custom_node.role(), Role::Button);
        assert_eq!(custom_node.label(), Some("Custom action"));
        assert!(custom_node.supports_action(Action::Click));
    }

    #[test]
    fn accesskit_tree_records_device_scale_as_root_transform() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(100.0, 40.0));
        ui.compute_layout(Size::new(100.0, 40.0));

        let update = ui.accesskit_tree_update_with_scale_factor(2.0);
        let root_node = node_for(&update, root);

        assert_eq!(root_node.transform(), Some(&Affine::scale(2.0)));
        assert_eq!(
            root_node.bounds(),
            Some(AccessRect {
                x0: 0.0,
                y0: 0.0,
                x1: 100.0,
                y1: 40.0
            })
        );
    }

    fn node_for(update: &TreeUpdate, element: ElementId) -> &Node {
        update
            .nodes
            .iter()
            .find(|(id, _)| *id == accesskit_node_id(element))
            .map(|(_, node)| node)
            .expect("node should exist")
    }

    fn action_with_numeric_value(element: ElementId, action: Action, value: f64) -> ActionRequest {
        ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: accesskit_node_id(element),
            data: Some(ActionData::NumericValue(value)),
        }
    }

    #[derive(Debug)]
    struct SemanticOnlyWidget;

    impl Widget for SemanticOnlyWidget {
        fn accessibility_role(
            &self,
            _element: &Element,
            _ctx: &AccessibilityContext<'_>,
        ) -> Option<Role> {
            Some(Role::Button)
        }

        fn configure_accessibility(
            &self,
            _element: &Element,
            ctx: &AccessibilityContext<'_>,
            node: &mut Node,
        ) {
            ctx.add_click_action(node);
        }
    }
}
