// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Panel`: anonymous container with configurable layout.
//!
//! Panels carry no per-instance behavior beyond surface painting +
//! layout. They're what apps use for rows, columns, decoration
//! wrappers, etc.
//!
//! ```ignore
//! ui.append(parent, Panel::row(24.0).padding(50.0));
//! ui.append(parent, Panel::column(6.0));
//! ```

use alloc::boxed::Box;
use smallvec::SmallVec;
use understory_style::{ClassId, TypeTag};

use crate::element::ElementId;
use crate::layouts;
use crate::ui::Ui;
use crate::widget::{AppendSpec, TYPE_GENERIC, Widget};

/// Builder + spec for an anonymous container element.
#[derive(Debug, Default)]
pub struct Panel {
    layout: Option<taffy::Style>,
    classes: SmallVec<[ClassId; 1]>,
}

impl Panel {
    /// Starts a new `Panel` builder with no layout configured.
    /// Width / height / gap / padding default to whatever the
    /// taffy default style provides (no fixed size, no flex).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Shortcut for a horizontal flex row with the given gap.
    #[must_use]
    pub fn row(gap: f32) -> Self {
        Self {
            layout: Some(layouts::row_with_gap(gap)),
            classes: SmallVec::new(),
        }
    }

    /// Shortcut for a vertical flex column with the given gap.
    #[must_use]
    pub fn column(gap: f32) -> Self {
        Self {
            layout: Some(layouts::column_with_gap(gap)),
            classes: SmallVec::new(),
        }
    }

    /// Replaces the panel's layout style. Use this when the row /
    /// column shortcuts aren't enough.
    #[must_use]
    pub fn layout(mut self, layout: taffy::Style) -> Self {
        self.layout = Some(layout);
        self
    }

    /// Adds uniform padding to the panel's existing layout. If no
    /// layout has been configured yet, the panel becomes a flex
    /// container with the padding applied.
    #[must_use]
    pub fn padding(mut self, value: f32) -> Self {
        let mut layout = self.layout.unwrap_or_else(|| taffy::Style {
            display: taffy::Display::Flex,
            ..taffy::Style::default()
        });
        layout.padding = layouts::uniform_padding(value);
        self.layout = Some(layout);
        self
    }

    /// Sets the cross-axis alignment to centered.
    #[must_use]
    pub fn align_center(mut self) -> Self {
        let mut layout = self.layout.unwrap_or_default();
        layout.align_items = Some(taffy::AlignItems::Center);
        self.layout = Some(layout);
        self
    }

    /// Sets the main-axis justification to centered.
    #[must_use]
    pub fn justify_center(mut self) -> Self {
        let mut layout = self.layout.unwrap_or_default();
        layout.justify_content = Some(taffy::JustifyContent::Center);
        self.layout = Some(layout);
        self
    }

    /// Attaches a class id to the resulting element. Style rules
    /// keyed on this class will match.
    #[must_use]
    pub fn class(mut self, class: ClassId) -> Self {
        self.classes.push(class);
        self
    }
}

impl AppendSpec for Panel {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget(parent, Box::new(PanelWidget));
        if let Some(layout) = self.layout {
            let layout_prop = ui.properties().layout;
            ui.set_local(id, layout_prop, layout);
        }
        for class in self.classes {
            ui.add_class(id, class);
        }
        id
    }
}

/// Per-element widget for `Panel`. Stateless — selector identity
/// is the only thing it provides. Panels resolve their surface
/// via the standard `present_surface` default in `Widget::present`.
#[derive(Debug, Default)]
pub struct PanelWidget;

impl Widget for PanelWidget {
    fn type_tag(&self) -> TypeTag {
        TYPE_GENERIC
    }
}
