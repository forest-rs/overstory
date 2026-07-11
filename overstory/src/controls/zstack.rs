// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained overlapping layout.
//!
//! `ZStack` owns overlap and two-axis alignment. It deliberately does not own
//! visibility selection, overlay-window policy, portals, or absolute
//! positioning. Later children paint and hit-test above earlier children by
//! ordinary retained-tree order.

use alloc::boxed::Box;

use understory_style::TypeTag;

use crate::behavior::{ArrangeContext, ArrangeResult};
use crate::element::{Element, ElementId};
use crate::layouts::{Layout, LayoutAlign, LayoutAlignment, LayoutContainer, LayoutPlacement};
use crate::ui::Ui;
use crate::widget::{AppendSpec, ElementConfig, ElementSpec, Widget};

/// Selector identity for [`ZStack`].
pub const TYPE_ZSTACK: TypeTag = TypeTag(31);

/// Two-axis alignment for children in a [`ZStack`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StackAlignment {
    /// Align children to the top-left corner.
    TopLeft,
    /// Align children to the top edge and horizontal center.
    Top,
    /// Align children to the top-right corner.
    TopRight,
    /// Align children to the left edge and vertical center.
    Left,
    /// Center children in both axes.
    #[default]
    Center,
    /// Align children to the right edge and vertical center.
    Right,
    /// Align children to the bottom-left corner.
    BottomLeft,
    /// Align children to the bottom edge and horizontal center.
    Bottom,
    /// Align children to the bottom-right corner.
    BottomRight,
    /// Stretch children across both axes of the stack's grid cell.
    Stretch,
}

impl StackAlignment {
    const fn block(self) -> LayoutAlign {
        match self {
            Self::TopLeft | Self::Top | Self::TopRight => LayoutAlign::Start,
            Self::Left | Self::Center | Self::Right => LayoutAlign::Center,
            Self::BottomLeft | Self::Bottom | Self::BottomRight => LayoutAlign::End,
            Self::Stretch => LayoutAlign::Stretch,
        }
    }

    const fn inline(self) -> LayoutAlign {
        match self {
            Self::TopLeft | Self::Left | Self::BottomLeft => LayoutAlign::Start,
            Self::Top | Self::Center | Self::Bottom => LayoutAlign::Center,
            Self::TopRight | Self::Right | Self::BottomRight => LayoutAlign::End,
            Self::Stretch => LayoutAlign::Stretch,
        }
    }

    pub(crate) const fn container_alignment(self) -> LayoutAlignment {
        LayoutAlignment::new()
            .align_items(self.block())
            .justify_items(self.inline())
    }

    /// Returns an item-alignment contribution that overrides a stack's
    /// alignment for one child.
    #[must_use]
    pub const fn child_alignment(self) -> LayoutAlignment {
        LayoutAlignment::new()
            .align_self(self.block())
            .justify_self(self.inline())
    }
}

/// Builder for a retained container whose direct children overlap.
#[derive(Debug)]
pub struct ZStack {
    alignment: StackAlignment,
    element: ElementConfig,
}

impl ZStack {
    /// Creates a centered overlapping stack.
    #[must_use]
    pub fn new() -> Self {
        let alignment = StackAlignment::Center;
        let mut element = ElementConfig::default();
        element.set_layout(
            Layout::new()
                .container(LayoutContainer::grid())
                .alignment(alignment.container_alignment()),
        );
        Self { alignment, element }
    }

    /// Sets the default alignment for direct children.
    #[must_use]
    pub fn alignment(mut self, alignment: StackAlignment) -> Self {
        self.alignment = alignment;
        self.sync_layout();
        self
    }

    fn sync_layout(&mut self) {
        self.element.set_layout(
            Layout::new()
                .container(LayoutContainer::grid())
                .alignment(self.alignment.container_alignment()),
        );
    }
}

impl Default for ZStack {
    fn default() -> Self {
        Self::new()
    }
}

impl AppendSpec for ZStack {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget(parent, Box::new(ZStackWidget));
        self.element.apply_to(ui, id);
        id
    }
}

impl ElementSpec for ZStack {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

#[derive(Debug)]
struct ZStackWidget;

impl Widget for ZStackWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_ZSTACK)
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
        let children = ctx
            .ui()
            .get(stack)
            .map(|element| element.children().to_vec())
            .unwrap_or_default();
        let target = LayoutPlacement::grid_cell(1, 1);
        let mut changed = false;
        for child in children {
            if ctx.ui().effective_layout_placement(child) != target {
                ctx.ui_mut()
                    .set_layout_placement_without_motion(child, target);
                changed = true;
            }
        }
        ArrangeResult::from_layout_changed(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Panel, Rect, Size};

    #[test]
    fn children_overlap_and_stack_sizes_to_the_largest_child() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(
            root,
            Layout::row(0.0)
                .logical_size(200.0, 100.0)
                .alignment(LayoutAlignment::new().align_items(LayoutAlign::Start)),
        );
        let stack = ui.append(root, ZStack::new().alignment(StackAlignment::TopLeft));
        let first = ui.append(
            stack,
            Panel::new().layout(Layout::new().logical_size(80.0, 40.0)),
        );
        let second = ui.append(
            stack,
            Panel::new().layout(Layout::new().logical_size(30.0, 60.0)),
        );

        ui.compute_layout(Size::new(200.0, 100.0));

        assert_eq!(ui.effective_bounds(stack), Rect::new(0.0, 0.0, 80.0, 60.0));
        assert_eq!(ui.effective_bounds(first), Rect::new(0.0, 0.0, 80.0, 40.0));
        assert_eq!(ui.effective_bounds(second), Rect::new(0.0, 0.0, 30.0, 60.0));
    }

    #[test]
    fn child_alignment_can_override_the_stack_default() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(
            root,
            Layout::row(0.0)
                .logical_size(200.0, 100.0)
                .alignment(LayoutAlignment::new().align_items(LayoutAlign::Start)),
        );
        let stack = ui.append(root, ZStack::new().alignment(StackAlignment::TopLeft));
        let background = ui.append(
            stack,
            Panel::new().layout(Layout::new().logical_size(100.0, 80.0)),
        );
        let foreground = ui.append(
            stack,
            Panel::new().layout(
                Layout::new()
                    .logical_size(20.0, 10.0)
                    .alignment(StackAlignment::BottomRight.child_alignment()),
            ),
        );

        ui.compute_layout(Size::new(200.0, 100.0));

        assert_eq!(
            ui.effective_bounds(background),
            Rect::new(0.0, 0.0, 100.0, 80.0)
        );
        assert_eq!(
            ui.effective_bounds(foreground),
            Rect::new(80.0, 70.0, 100.0, 80.0)
        );
    }
}
