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
//! - Keep custom widget accessibility extensibility as a later widget-trait
//!   concern once more custom-control needs are visible.

use accesskit::{
    Action, ActionRequest, Node, NodeId, Rect as AccessRect, Role, Toggled, Tree, TreeId,
    TreeUpdate,
};
use alloc::{string::String, vec::Vec};

use crate::controls::button::TYPE_BUTTON;
use crate::controls::checkbox::TYPE_CHECK_BOX;
use crate::controls::divider::TYPE_DIVIDER;
use crate::controls::list::{TYPE_LIST_BOX, TYPE_LIST_ITEM};
use crate::controls::popup::TYPE_POPUP;
use crate::controls::progress::TYPE_PROGRESS_BAR;
use crate::controls::scroll::{TYPE_SCROLL_BAR, TYPE_SCROLL_VIEW};
use crate::controls::select::TYPE_SELECT;
use crate::controls::slider::{TYPE_SLIDER, set_slider_value};
use crate::controls::spinner::TYPE_SPINNER;
use crate::controls::splitter::TYPE_SPLITTER;
use crate::controls::text::TYPE_TEXT;
use crate::controls::text_input::TYPE_TEXT_INPUT;
use crate::controls::toggle::TYPE_TOGGLE;
use crate::controls::tooltip::TYPE_TOOLTIP;
use crate::controls::viewport::TYPE_VIEWPORT;
use crate::element::{Element, ElementId};
use crate::pseudos::PSEUDO_SELECTED;
use crate::ui::Ui;
use crate::widget::TYPE_GENERIC;

impl Ui {
    /// Builds a full AccessKit tree update for the current retained UI tree.
    ///
    /// This first slice intentionally emits a complete snapshot every time.
    /// Host adapters can call it lazily when platform accessibility activates
    /// and after frames that may have changed semantics, focus, or geometry.
    #[must_use]
    pub fn accesskit_tree_update(&self) -> TreeUpdate {
        let root = self.root();
        let mut nodes = Vec::new();
        self.push_accesskit_node(root, &mut nodes);
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
            Action::Click => self.perform_accesskit_click(target),
            Action::Increment => self.step_accesskit_range(target, 1.0),
            Action::Decrement => self.step_accesskit_range(target, -1.0),
            _ => false,
        }
    }

    fn push_accesskit_node(&self, id: ElementId, nodes: &mut Vec<(NodeId, Node)>) {
        let Some(element) = self.get(id) else {
            return;
        };
        let children = element.children().to_vec();
        let mut node = self.accesskit_node_for(element);
        node.set_children(
            children
                .iter()
                .copied()
                .map(accesskit_node_id)
                .collect::<Vec<_>>(),
        );
        nodes.push((accesskit_node_id(id), node));
        for child in children {
            self.push_accesskit_node(child, nodes);
        }
    }

    fn accesskit_node_for(&self, element: &Element) -> Node {
        let id = element.id();
        let role = accesskit_role(element, id == self.root());
        let mut node = Node::new(role);
        node.set_bounds(accesskit_bounds(self.effective_bounds(id)));

        if self.is_focusable(id) {
            node.add_action(Action::Focus);
        }
        if !self.is_enabled(id) {
            node.set_disabled();
        }

        let content = self.accesskit_content(element);
        apply_accesskit_label_or_value(&mut node, role, content);
        self.apply_accesskit_control_state(element, &mut node);
        node
    }

    fn accesskit_content(&self, element: &Element) -> String {
        self.resolve_value_for(element, self.properties().content)
    }

    fn apply_accesskit_control_state(&self, element: &Element, node: &mut Node) {
        let role = node.role();
        if is_clickable_role(role) {
            node.add_action(Action::Click);
        }
        if role == Role::CheckBox || role == Role::Switch {
            let checked = self.resolve_value_for(element, self.properties().is_checked);
            node.set_toggled(Toggled::from(checked));
        }
        if role == Role::ListBoxOption && element.pseudos().contains(&PSEUDO_SELECTED) {
            node.set_selected(true);
        }
        if is_range_role(role) {
            let value = self.resolve_value_for(element, self.properties().value);
            let min = self.resolve_value_for(element, self.properties().minimum);
            let max = self.resolve_value_for(element, self.properties().maximum);
            node.set_numeric_value(value);
            node.set_min_numeric_value(min);
            node.set_max_numeric_value(max);
            if role == Role::Slider {
                node.add_action(Action::Increment);
                node.add_action(Action::Decrement);
            }
        }
    }

    fn step_accesskit_range(&mut self, target: ElementId, direction: f64) -> bool {
        let Some(element) = self.get(target) else {
            return false;
        };
        if element.type_tag() != Some(TYPE_SLIDER) || !self.is_enabled(target) {
            return false;
        }

        let min = self.resolve_value_for(element, self.properties().minimum);
        let max = self.resolve_value_for(element, self.properties().maximum);
        let current = self.resolve_value_for(element, self.properties().value);
        let explicit_step = self.resolve_value_for(element, self.properties().small_change);
        let fallback_step = ((max - min).abs() * 0.01).max(0.01);
        let step = if explicit_step > 0.0 {
            explicit_step
        } else {
            fallback_step
        };
        set_slider_value(self, target, current + (direction * step));
        true
    }

    fn perform_accesskit_click(&mut self, target: ElementId) -> bool {
        let clickable = self
            .get(target)
            .map(|element| is_clickable_role(accesskit_role(element, target == self.root())))
            .unwrap_or(false);
        if !clickable || !self.is_enabled(target) {
            return false;
        }
        let point = self.effective_bounds(target).center();
        let _ = self.synthesize_click(target, point);
        true
    }
}

fn accesskit_tree(root: ElementId) -> Tree {
    let mut tree = Tree::new(accesskit_node_id(root));
    tree.toolkit_name = Some(String::from("Overstory"));
    tree.toolkit_version = Some(String::from(env!("CARGO_PKG_VERSION")));
    tree
}

fn accesskit_role(element: &Element, is_root: bool) -> Role {
    if is_root {
        return Role::Window;
    }
    match element.type_tag() {
        Some(tag) if tag == TYPE_TEXT => Role::Label,
        Some(tag) if tag == TYPE_BUTTON => Role::Button,
        Some(tag) if tag == TYPE_TOGGLE => Role::Switch,
        Some(tag) if tag == TYPE_CHECK_BOX => Role::CheckBox,
        Some(tag) if tag == TYPE_SLIDER => Role::Slider,
        Some(tag) if tag == TYPE_PROGRESS_BAR => Role::ProgressIndicator,
        Some(tag) if tag == TYPE_TEXT_INPUT => Role::TextInput,
        Some(tag) if tag == TYPE_LIST_BOX => Role::ListBox,
        Some(tag) if tag == TYPE_LIST_ITEM => Role::ListBoxOption,
        Some(tag) if tag == TYPE_SELECT => Role::ComboBox,
        Some(tag) if tag == TYPE_SCROLL_VIEW => Role::ScrollView,
        Some(tag) if tag == TYPE_SCROLL_BAR => Role::ScrollBar,
        Some(tag) if tag == TYPE_SPLITTER => Role::Splitter,
        Some(tag) if tag == TYPE_SPINNER => Role::ProgressIndicator,
        Some(tag) if tag == TYPE_DIVIDER => Role::Splitter,
        Some(tag) if tag == TYPE_TOOLTIP => Role::Tooltip,
        Some(tag) if tag == TYPE_POPUP => Role::Pane,
        Some(tag) if tag == TYPE_VIEWPORT => Role::Pane,
        Some(tag) if tag == TYPE_GENERIC => Role::GenericContainer,
        Some(_) | None => Role::GenericContainer,
    }
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

fn is_range_role(role: Role) -> bool {
    matches!(
        role,
        Role::Meter | Role::ProgressIndicator | Role::ScrollBar | Role::Slider | Role::SpinButton
    )
}

fn accesskit_node_id(id: ElementId) -> NodeId {
    NodeId((u64::from(id.generation()) << 32) | u64::from(id.slot()))
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
    use crate::{Button, CheckBox, Layout, Slider, Text};
    use kurbo::Size;

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
        assert_eq!(slider_node.numeric_value(), Some(0.4));
        assert_eq!(slider_node.min_numeric_value(), Some(0.0));
        assert_eq!(slider_node.max_numeric_value(), Some(2.0));
        assert!(slider_node.supports_action(Action::Increment));
    }

    fn node_for(update: &TreeUpdate, element: ElementId) -> &Node {
        update
            .nodes
            .iter()
            .find(|(id, _)| *id == accesskit_node_id(element))
            .map(|(_, node)| node)
            .expect("node should exist")
    }
}
