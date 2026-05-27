// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Popup`: explicit anchored overlay container.
//!
//! ```ignore
//! let button = ui.append(parent, Button::new("Open"));
//! let popup = ui.open_popup(button, Popup::new(), |ui, popup| {
//!     ui.append(popup, Text::new("Overlay content"));
//! });
//! if let Some(popup) = popup {
//!     ui.close_popup(popup);
//! }
//! ```
//!
//! Roadmap:
//! - Keep `Popup` as explicit anchored overlay infrastructure, not as a menu,
//!   tooltip, combo box, or flyout policy bundle.
//! - Make placement robust: collision handling, fallback placements, viewport
//!   constraints, transforms, and nested overlay coordinates should be handled
//!   in one overlay system.
//! - Route input, focus scopes, dismiss reasons, and focus restoration through
//!   first-class event / focus systems instead of ad hoc host code.
//! - Prepare for compositor-backed overlay layers while keeping the builder API
//!   independent of a specific renderer or windowing backend.

use alloc::boxed::Box;

use kurbo::{Point, Vec2};
use understory_style::TypeTag;

use crate::composition::CompositionPolicy;
use crate::element::{Element, ElementId};
use crate::layouts::Layout;
use crate::overlay::Placement;
use crate::ui::Ui;
use crate::widget::{AppendSpec, Widget};

/// Selector identity for `Popup`.
pub const TYPE_POPUP: TypeTag = TypeTag(16);

/// Builder + retained configuration for an anchored popup.
#[derive(Clone, Debug, PartialEq)]
pub struct Popup {
    placement: Placement,
    offset: Vec2,
    padding_x: f32,
    padding_y: f32,
    dismiss_on_outside_pointer: bool,
    dismiss_on_escape: bool,
    restore_focus_on_close: bool,
}

impl Popup {
    /// Starts a new popup configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            placement: Placement::Bottom,
            offset: Vec2::new(0.0, 6.0),
            padding_x: 10.0,
            padding_y: 8.0,
            dismiss_on_outside_pointer: true,
            dismiss_on_escape: true,
            restore_focus_on_close: true,
        }
    }

    /// Sets the popup's preferred placement around its target.
    #[must_use]
    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    /// Offsets the popup from the placed edge of the target element.
    #[must_use]
    pub fn offset(mut self, x: f32, y: f32) -> Self {
        self.offset = Vec2::new(f64::from(x), f64::from(y));
        self
    }

    /// Sets the popup's horizontal and vertical padding.
    #[must_use]
    pub fn padding(mut self, horizontal: f32, vertical: f32) -> Self {
        self.padding_x = horizontal;
        self.padding_y = vertical;
        self
    }

    /// Sets whether a primary pointer press outside this popup closes it.
    #[must_use]
    pub fn dismiss_on_outside_pointer(mut self, dismiss: bool) -> Self {
        self.dismiss_on_outside_pointer = dismiss;
        self
    }

    /// Sets whether pressing Escape closes this popup.
    #[must_use]
    pub fn dismiss_on_escape(mut self, dismiss: bool) -> Self {
        self.dismiss_on_escape = dismiss;
        self
    }

    /// Sets whether closing the popup restores focus to the opening focus.
    #[must_use]
    pub fn restore_focus_on_close(mut self, restore: bool) -> Self {
        self.restore_focus_on_close = restore;
        self
    }

    pub(crate) fn placement_value(&self) -> Placement {
        self.placement
    }

    pub(crate) fn offset_value(&self) -> Vec2 {
        self.offset
    }

    pub(crate) fn dismiss_on_outside_pointer_value(&self) -> bool {
        self.dismiss_on_outside_pointer
    }

    pub(crate) fn dismiss_on_escape_value(&self) -> bool {
        self.dismiss_on_escape
    }

    pub(crate) fn restore_focus_on_close_value(&self) -> bool {
        self.restore_focus_on_close
    }

    pub(crate) fn layout_for(&self, origin: Point) -> Layout {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "popup coordinates fit the layout engine"
        )]
        let left = origin.x as f32;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "popup coordinates fit the layout engine"
        )]
        let top = origin.y as f32;
        Layout::column(0.0)
            .absolute_at(left, top)
            .padding_xy(self.padding_x, self.padding_y)
    }
}

impl Default for Popup {
    fn default() -> Self {
        Self::new()
    }
}

impl AppendSpec for Popup {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        append_popup(ui, parent, &self, Point::ZERO)
    }
}

/// Per-element widget for `Popup`.
#[derive(Debug, Default)]
pub(crate) struct PopupWidget;

impl Widget for PopupWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_POPUP)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::Pane)
    }

    fn composition_policy(&self) -> CompositionPolicy {
        CompositionPolicy::OverlayScene
    }
}

pub(crate) fn append_overlay_popup(ui: &mut Ui, popup: &Popup, origin: Point) -> ElementId {
    append_popup(ui, ui.root(), popup, origin)
}

fn append_popup(ui: &mut Ui, parent: ElementId, popup: &Popup, origin: Point) -> ElementId {
    let id = ui.append_widget(parent, Box::new(PopupWidget));
    ui.set_layout(id, popup.layout_for(origin));
    id
}
