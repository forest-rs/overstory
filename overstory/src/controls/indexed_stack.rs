// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained single-visible-child layout.
//!
//! `IndexedStack` owns which direct child participates in layout,
//! presentation, hit testing, and accessibility. It deliberately does not own
//! selection interaction, stable application keys, headers, navigation, or
//! transition policy. Controls such as `TabView` map their stable keys onto
//! this ordered layout primitive.

use alloc::{boxed::Box, vec::Vec};

use understory_style::TypeTag;

use crate::StackAlignment;
use crate::behavior::{ArrangeContext, ArrangeResult};
use crate::element::{Element, ElementId};
use crate::layouts::{Layout, LayoutContainer, LayoutPlacement};
use crate::properties::Visibility;
use crate::ui::Ui;
use crate::widget::{AppendSpec, BuiltInPropertyKey, ElementConfig, ElementSpec, Widget};

/// Selector identity for [`IndexedStack`].
pub const TYPE_INDEXED_STACK: TypeTag = TypeTag(32);

/// Builder for a retained stack that displays at most one direct child.
#[derive(Debug)]
pub struct IndexedStack {
    visible_index: Option<usize>,
    alignment: StackAlignment,
    element: ElementConfig,
}

impl IndexedStack {
    /// Creates a stack with no visible child.
    #[must_use]
    pub fn new() -> Self {
        let alignment = StackAlignment::Center;
        let mut element = ElementConfig::default();
        element.set_layout(
            Layout::new()
                .container(LayoutContainer::grid())
                .alignment(alignment.container_alignment()),
        );
        Self {
            visible_index: None,
            alignment,
            element,
        }
    }

    /// Sets the initially visible child index.
    ///
    /// An out-of-range index settles to no visible child after the direct
    /// child collection has been authored.
    #[must_use]
    pub fn visible_index(mut self, index: usize) -> Self {
        self.visible_index = Some(index);
        self
    }

    /// Sets the alignment of the visible child within the stack.
    #[must_use]
    pub fn alignment(mut self, alignment: StackAlignment) -> Self {
        self.alignment = alignment;
        self.element.set_layout(
            Layout::new()
                .container(LayoutContainer::grid())
                .alignment(alignment.container_alignment()),
        );
        self
    }
}

impl Default for IndexedStack {
    fn default() -> Self {
        Self::new()
    }
}

impl AppendSpec for IndexedStack {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let mut element = self.element;
        element.set_built_in_local(BuiltInPropertyKey::SelectedIndex, self.visible_index);
        let id = ui.append_widget(
            parent,
            Box::new(IndexedStackWidget {
                visible_index: self.visible_index,
                visible_child: None,
            }),
        );
        element.apply_to(ui, id);
        id
    }
}

impl ElementSpec for IndexedStack {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

#[derive(Debug)]
struct IndexedStackWidget {
    visible_index: Option<usize>,
    visible_child: Option<ElementId>,
}

impl Widget for IndexedStackWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_INDEXED_STACK)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::GenericContainer)
    }

    fn arrange(&mut self, ctx: &mut ArrangeContext<'_>) -> ArrangeResult {
        let stack = ctx.id();
        let children = direct_children(ctx.ui(), stack);
        let requested = resolved_index(ctx.ui(), stack);
        let mut changed = self.reconcile_selection(ctx.ui_mut(), stack, &children, requested);
        changed |= sync_children(ctx.ui_mut(), &children, self.visible_index);
        ArrangeResult::from_layout_changed(changed)
    }
}

impl IndexedStackWidget {
    fn reconcile_selection(
        &mut self,
        ui: &mut Ui,
        stack: ElementId,
        children: &[ElementId],
        requested: Option<usize>,
    ) -> bool {
        let retained_index = self
            .visible_child
            .and_then(|visible| children.iter().position(|child| *child == visible));

        let target = if requested == self.visible_index {
            retained_index.or_else(|| requested.filter(|index| *index < children.len()))
        } else {
            requested.filter(|index| *index < children.len())
        };
        let target_child = target.and_then(|index| children.get(index).copied());
        let changed = target != self.visible_index || target_child != self.visible_child;
        self.visible_index = target;
        self.visible_child = target_child;

        if resolved_index(ui, stack) != target {
            ui.set_local(stack, ui.properties().selected_index, target);
            return true;
        }
        changed
    }
}

fn direct_children(ui: &Ui, stack: ElementId) -> Vec<ElementId> {
    ui.get(stack)
        .map(|element| element.children().to_vec())
        .unwrap_or_default()
}

fn resolved_index(ui: &Ui, stack: ElementId) -> Option<usize> {
    ui.get(stack)
        .and_then(|element| ui.resolve_value_for(element, ui.properties().selected_index))
}

fn sync_children(ui: &mut Ui, children: &[ElementId], visible_index: Option<usize>) -> bool {
    let placement = LayoutPlacement::grid_cell(1, 1);
    let mut changed = false;
    for (index, child) in children.iter().copied().enumerate() {
        if ui.effective_layout_placement(child) != placement {
            ui.set_layout_placement_without_motion(child, placement);
            changed = true;
        }
        let target = if Some(index) == visible_index {
            Visibility::Visible
        } else {
            Visibility::Collapsed
        };
        if ui.visibility(child) != Some(target) {
            ui.set_visibility(child, target);
            changed = true;
        }
    }
    changed
}

/// Sets the visible child of an indexed stack.
///
/// Passing `None` hides every child. An out-of-range index is rejected without
/// changing the current visible child.
pub fn set_indexed_stack_visible_index(
    ui: &mut Ui,
    stack: ElementId,
    index: Option<usize>,
) -> bool {
    let children = direct_children(ui, stack);
    if index.is_some_and(|index| index >= children.len()) {
        return false;
    }
    let mut changed = false;
    ui.with_widget_mut(stack, |widget, ui| {
        let Some(widget) = widget.downcast_mut::<IndexedStackWidget>() else {
            return;
        };
        let target_child = index.and_then(|index| children.get(index).copied());
        changed = widget.visible_index != index || widget.visible_child != target_child;
        widget.visible_index = index;
        widget.visible_child = target_child;
        if resolved_index(ui, stack) != index {
            ui.set_local(stack, ui.properties().selected_index, index);
            changed = true;
        }
        changed |= sync_children(ui, &children, index);
    });
    changed
}

/// Returns the visible child index of an indexed stack.
#[must_use]
pub fn indexed_stack_visible_index(ui: &Ui, stack: ElementId) -> Option<usize> {
    ui.get(stack)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<IndexedStackWidget>())
        .and_then(|widget| widget.visible_index)
}

/// Returns the currently visible child of an indexed stack.
#[must_use]
pub fn indexed_stack_visible_child(ui: &Ui, stack: ElementId) -> Option<ElementId> {
    ui.get(stack)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<IndexedStackWidget>())
        .and_then(|widget| widget.visible_child)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LayoutAlign, LayoutAlignment, Panel, Rect, Size};
    use kurbo::Point;

    fn root_layout() -> Layout {
        Layout::row(0.0)
            .logical_size(200.0, 120.0)
            .alignment(LayoutAlignment::new().align_items(LayoutAlign::Start))
    }

    #[test]
    fn only_visible_child_contributes_layout() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, root_layout());
        let stack = ui.append(
            root,
            IndexedStack::new()
                .visible_index(1)
                .alignment(StackAlignment::TopLeft),
        );
        let first = ui.append(
            stack,
            Panel::new().layout(Layout::new().logical_size(100.0, 80.0)),
        );
        let second = ui.append(
            stack,
            Panel::new().layout(Layout::new().logical_size(40.0, 30.0)),
        );

        ui.compute_layout(Size::new(200.0, 120.0));

        assert_eq!(ui.effective_bounds(stack), Rect::new(0.0, 0.0, 40.0, 30.0));
        assert_eq!(ui.visibility(first), Some(Visibility::Collapsed));
        assert_eq!(ui.visibility(second), Some(Visibility::Visible));
        assert_eq!(indexed_stack_visible_child(&ui, stack), Some(second));
    }

    #[test]
    fn switching_keeps_both_children_retained() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, root_layout());
        let stack = ui.append(root, IndexedStack::new().visible_index(0));
        let first = ui.append(
            stack,
            Panel::new().layout(Layout::new().logical_size(70.0, 50.0)),
        );
        let second = ui.append(
            stack,
            Panel::new().layout(Layout::new().logical_size(30.0, 20.0)),
        );
        ui.compute_layout(Size::new(200.0, 120.0));

        assert!(set_indexed_stack_visible_index(&mut ui, stack, Some(1)));
        ui.compute_layout(Size::new(200.0, 120.0));

        assert!(ui.get(first).is_some());
        assert!(ui.get(second).is_some());
        assert_eq!(ui.visibility(first), Some(Visibility::Collapsed));
        assert_eq!(ui.visibility(second), Some(Visibility::Visible));
        assert_eq!(indexed_stack_visible_index(&ui, stack), Some(1));
    }

    #[test]
    fn removing_an_earlier_child_preserves_visible_identity() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, root_layout());
        let stack = ui.append(root, IndexedStack::new().visible_index(1));
        let first = ui.append(stack, Panel::new());
        let second = ui.append(stack, Panel::new());
        ui.compute_layout(Size::new(200.0, 120.0));

        ui.remove(first);
        ui.compute_layout(Size::new(200.0, 120.0));

        assert_eq!(indexed_stack_visible_index(&ui, stack), Some(0));
        assert_eq!(indexed_stack_visible_child(&ui, stack), Some(second));
    }

    #[test]
    fn collapsed_children_are_not_accessible_or_hit_testable() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, root_layout());
        let stack = ui.append(root, IndexedStack::new().visible_index(1));
        let hidden = ui.append(
            stack,
            Panel::new().layout(Layout::new().logical_size(100.0, 100.0)),
        );
        let visible = ui.append(
            stack,
            Panel::new().layout(Layout::new().logical_size(30.0, 30.0)),
        );
        ui.compute_layout(Size::new(200.0, 120.0));

        assert_ne!(ui.hit_test(Point::new(10.0, 10.0)), Some(hidden));
        assert_eq!(ui.hit_test(Point::new(10.0, 10.0)), Some(visible));
        let update = ui.accesskit_tree_update();
        assert!(
            !update
                .nodes
                .iter()
                .any(|(id, _)| { *id == crate::accessibility::accesskit_node_id(hidden) })
        );
    }
}
