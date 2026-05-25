// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Viewport`: clipped container for canvas-like and scrollable regions.

use alloc::boxed::Box;

use cursor_icon::CursorIcon;
use understory_style::TypeTag;

use crate::element::{Element, ElementId};
use crate::layouts::Layout;
use crate::ui::Ui;
use crate::widget::{AppendSpec, Widget};

/// Selector identity for `Viewport`.
pub const TYPE_VIEWPORT: TypeTag = TypeTag(17);

/// Builder + spec for a clipped viewport container.
///
/// `Viewport` is intentionally small: it clips descendants to its arranged
/// bounds and can advertise a cursor. Higher-level controls can layer
/// scrolling, panning, zooming, or compositor-backed behavior on top.
#[derive(Clone, Debug, Default)]
pub struct Viewport {
    layout: Option<Layout>,
    cursor_icon: Option<CursorIcon>,
}

impl Viewport {
    /// Starts a viewport with default layout and no cursor override.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the viewport's layout style.
    #[must_use]
    pub fn layout(mut self, layout: impl Into<Layout>) -> Self {
        self.layout = Some(layout.into());
        self
    }

    /// Sets the cursor shown while the viewport is the active hover or press
    /// target.
    #[must_use]
    pub fn cursor_icon(mut self, cursor_icon: CursorIcon) -> Self {
        self.cursor_icon = Some(cursor_icon);
        self
    }
}

impl AppendSpec for Viewport {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget(
            parent,
            Box::new(ViewportWidget {
                cursor_icon: self.cursor_icon,
            }),
        );
        if let Some(layout) = self.layout {
            ui.set_local(id, ui.properties().layout, layout);
        }
        id
    }
}

/// Per-element widget for `Viewport`.
#[derive(Debug, Default)]
pub(crate) struct ViewportWidget {
    cursor_icon: Option<CursorIcon>,
}

impl Widget for ViewportWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_VIEWPORT)
    }

    fn cursor_icon(&self, _element: &Element) -> Option<CursorIcon> {
        self.cursor_icon
    }

    fn clips_subtree(&self, _element: &Element) -> bool {
        true
    }
}
