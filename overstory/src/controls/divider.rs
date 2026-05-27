// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Divider`: decorative separator for rows and columns.
//!
//! Roadmap:
//! - Keep `Divider` decorative and non-interactive; resizing behavior belongs
//!   in `Splitter`.
//! - Let style control thickness, color, insets, and stretching rather than
//!   growing a broad builder API.
//! - Add semantic separator metadata only when callers ask for it; many
//!   dividers should stay hidden from accessibility trees.
//! - Use the same axis vocabulary as scrollbars, splitters, and future layout
//!   helpers so horizontal and vertical behavior remains predictable.

use alloc::boxed::Box;

use understory_style::TypeTag;

use crate::element::{Element, ElementId};
use crate::layouts::Layout;
use crate::ui::Ui;
use crate::widget::{AppendSpec, Widget};

/// Selector identity for `Divider`.
pub const TYPE_DIVIDER: TypeTag = TypeTag(4);

/// Orientation for a decorative divider.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum DividerAxis {
    /// Horizontal rule separating stacked content.
    #[default]
    Horizontal,
    /// Vertical rule separating side-by-side content.
    Vertical,
}

/// Builder + spec for a simple separator.
#[derive(Clone, Debug)]
pub struct Divider {
    axis: DividerAxis,
    thickness: f32,
    length: Option<f32>,
}

impl Default for Divider {
    fn default() -> Self {
        Self {
            axis: DividerAxis::Horizontal,
            thickness: 1.0,
            length: None,
        }
    }
}

impl Divider {
    /// Creates a horizontal divider.
    #[must_use]
    pub fn horizontal() -> Self {
        Self::default()
    }

    /// Creates a vertical divider.
    #[must_use]
    pub fn vertical() -> Self {
        Self {
            axis: DividerAxis::Vertical,
            ..Self::default()
        }
    }

    /// Sets the divider thickness in logical pixels.
    #[must_use]
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness.max(1.0);
        self
    }

    /// Sets the divider's main-axis length in logical pixels.
    #[must_use]
    pub fn length(mut self, length: f32) -> Self {
        self.length = Some(length.max(1.0));
        self
    }

    fn layout(&self) -> Layout {
        let length = self.length.unwrap_or(1.0);
        match self.axis {
            DividerAxis::Horizontal => Layout::new().logical_size(length, self.thickness),
            DividerAxis::Vertical => Layout::new().logical_size(self.thickness, length),
        }
    }
}

impl AppendSpec for Divider {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget(parent, Box::new(DividerWidget));
        ui.set_layout(id, self.layout());
        id
    }
}

/// Per-element widget for `Divider`.
#[derive(Debug, Default)]
pub(crate) struct DividerWidget;

impl Widget for DividerWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_DIVIDER)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::Splitter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Size;

    #[test]
    fn vertical_divider_lays_out_to_thickness_and_length() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(0.0).logical_size(100.0, 100.0));
        let divider = ui.append(root, Divider::vertical().thickness(3.0).length(42.0));

        ui.compute_layout(Size::new(100.0, 100.0));

        let bounds = ui.effective_bounds(divider);
        assert_eq!(bounds.width(), 3.0);
        assert_eq!(bounds.height(), 42.0);
    }
}
