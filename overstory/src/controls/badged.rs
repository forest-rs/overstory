// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Content decorated by an optional overflowing badge.
//!
//! `Badged` owns two retained roles: content that determines intrinsic size,
//! and an optional adornment anchored over a content corner. It deliberately
//! does not impose badge visuals, interaction, notification state, or clipping.

use alloc::boxed::Box;

use kurbo::Vec2;
use understory_style::TypeTag;

use crate::behavior::{ArrangeContext, ArrangeResult};
use crate::element::{Element, ElementId};
use crate::layouts::{Layout, LayoutPlacement};
use crate::ui::Ui;
use crate::widget::{AppendSpec, ElementConfig, ElementSpec, Widget};

/// Selector identity for [`Badged`].
pub const TYPE_BADGED: TypeTag = TypeTag(34);

/// Corner of the content used to anchor a badge's center.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BadgePlacement {
    /// Anchor at the content's top-left corner.
    TopLeft,
    /// Anchor at the content's top-right corner.
    #[default]
    TopRight,
    /// Anchor at the content's bottom-left corner.
    BottomLeft,
    /// Anchor at the content's bottom-right corner.
    BottomRight,
}

/// Builder for content with an optional overflowing badge.
pub struct Badged<Content, Badge> {
    content: Content,
    badge: Option<Badge>,
    placement: BadgePlacement,
    offset: Vec2,
    element: ElementConfig,
}

impl<Content: core::fmt::Debug, Badge: core::fmt::Debug> core::fmt::Debug
    for Badged<Content, Badge>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Badged")
            .field("content", &self.content)
            .field("badge", &self.badge)
            .field("placement", &self.placement)
            .field("offset", &self.offset)
            .finish_non_exhaustive()
    }
}

impl<Content, Badge> Badged<Content, Badge> {
    /// Creates content with a badge anchored at its top-right corner.
    #[must_use]
    pub fn new(content: Content, badge: Badge) -> Self {
        Self::new_optional(content, Some(badge))
    }

    /// Creates content whose badge may initially be absent.
    #[must_use]
    pub fn new_optional(content: Content, badge: Option<Badge>) -> Self {
        let mut element = ElementConfig::default();
        element.set_layout(Layout::row(0.0).relative());
        Self {
            content,
            badge,
            placement: BadgePlacement::TopRight,
            offset: Vec2::ZERO,
            element,
        }
    }

    /// Sets the content corner used to anchor the badge's center.
    #[must_use]
    pub fn badge_placement(mut self, placement: BadgePlacement) -> Self {
        self.placement = placement;
        self
    }

    /// Adds a logical-pixel offset after corner anchoring.
    #[must_use]
    pub fn badge_offset(mut self, offset: Vec2) -> Self {
        self.offset = offset;
        self
    }
}

impl<Content, Badge> AppendSpec for Badged<Content, Badge>
where
    Content: AppendSpec,
    Badge: AppendSpec,
{
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget(
            parent,
            Box::new(BadgedWidget {
                content: None,
                badge: None,
                placement: self.placement,
                offset: self.offset,
            }),
        );
        self.element.apply_to(ui, id);
        let content = ui.append(id, self.content);
        let badge = self.badge.map(|badge| {
            let badge = ui.append(id, badge);
            ui.set_layout_placement_without_motion(badge, LayoutPlacement::absolute_at(0.0, 0.0));
            badge
        });
        ui.with_widget_mut(id, |widget, _ui| {
            let widget = widget
                .downcast_mut::<BadgedWidget>()
                .expect("Badged keeps its widget type");
            widget.content = Some(content);
            widget.badge = badge;
        });
        id
    }
}

impl<Content, Badge> ElementSpec for Badged<Content, Badge>
where
    Content: AppendSpec,
    Badge: AppendSpec,
{
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

#[derive(Debug)]
struct BadgedWidget {
    content: Option<ElementId>,
    badge: Option<ElementId>,
    placement: BadgePlacement,
    offset: Vec2,
}

impl Widget for BadgedWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_BADGED)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::GenericContainer)
    }

    fn is_hit_test_visible(&self, _element: &Element) -> bool {
        false
    }

    fn arrange(&mut self, ctx: &mut ArrangeContext<'_>) -> ArrangeResult {
        let Some(content) = self.content else {
            return ArrangeResult::UNCHANGED;
        };
        let Some(badge) = self.badge else {
            return ArrangeResult::UNCHANGED;
        };
        let root_bounds = ctx.ui().effective_bounds(ctx.id());
        let content_bounds = ctx.ui().effective_bounds(content);
        let badge_bounds = ctx.ui().effective_bounds(badge);
        let (anchor_x, anchor_y) = match self.placement {
            BadgePlacement::TopLeft => (content_bounds.x0, content_bounds.y0),
            BadgePlacement::TopRight => (content_bounds.x1, content_bounds.y0),
            BadgePlacement::BottomLeft => (content_bounds.x0, content_bounds.y1),
            BadgePlacement::BottomRight => (content_bounds.x1, content_bounds.y1),
        };
        let left = anchor_x - root_bounds.x0 - badge_bounds.width() * 0.5 + self.offset.x;
        let top = anchor_y - root_bounds.y0 - badge_bounds.height() * 0.5 + self.offset.y;
        let target = LayoutPlacement::absolute_at(left, top);
        if placement_close(ctx.ui().effective_layout_placement(badge), target) {
            return ArrangeResult::UNCHANGED;
        }
        ctx.ui_mut()
            .set_layout_placement_without_motion(badge, target);
        ArrangeResult::LAYOUT_CHANGED
    }
}

fn placement_close(current: LayoutPlacement, target: LayoutPlacement) -> bool {
    match (current, target) {
        (
            LayoutPlacement::AbsoluteAt {
                left: current_left,
                top: current_top,
            },
            LayoutPlacement::AbsoluteAt {
                left: target_left,
                top: target_top,
            },
        ) => {
            (current_left - target_left).abs() <= f64::EPSILON
                && (current_top - target_top).abs() <= f64::EPSILON
        }
        _ => false,
    }
}

/// Returns the content element owned by a `Badged` wrapper.
#[must_use]
pub fn badged_content(ui: &Ui, badged: ElementId) -> Option<ElementId> {
    badged_widget(ui, badged).and_then(|widget| widget.content)
}

/// Returns the current badge element, if present.
#[must_use]
pub fn badged_badge(ui: &Ui, badged: ElementId) -> Option<ElementId> {
    badged_widget(ui, badged).and_then(|widget| widget.badge)
}

/// Replaces or installs a badge and returns its retained element id.
pub fn replace_badged_badge(
    ui: &mut Ui,
    badged: ElementId,
    badge: impl AppendSpec,
) -> Option<ElementId> {
    badged_widget(ui, badged)?;
    if let Some(previous) = badged_badge(ui, badged) {
        ui.remove(previous);
    }
    let badge = ui.append(badged, badge);
    ui.set_layout_placement_without_motion(badge, LayoutPlacement::absolute_at(0.0, 0.0));
    ui.with_widget_mut(badged, |widget, _ui| {
        let widget = widget
            .downcast_mut::<BadgedWidget>()
            .expect("validated Badged widget type");
        widget.badge = Some(badge);
    });
    Some(badge)
}

/// Removes a `Badged` wrapper's badge while preserving its content.
pub fn clear_badged_badge(ui: &mut Ui, badged: ElementId) -> bool {
    let Some(badge) = badged_badge(ui, badged) else {
        return false;
    };
    ui.remove(badge);
    ui.with_widget_mut(badged, |widget, _ui| {
        let widget = widget
            .downcast_mut::<BadgedWidget>()
            .expect("validated Badged widget type");
        widget.badge = None;
    });
    true
}

fn badged_widget(ui: &Ui, badged: ElementId) -> Option<&BadgedWidget> {
    ui.get(badged)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<BadgedWidget>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Badge, LayoutAlign, LayoutAlignment, Panel, Rect, Size};

    #[test]
    fn content_alone_determines_wrapper_size() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(
            root,
            Layout::row(0.0)
                .logical_size(200.0, 120.0)
                .alignment(LayoutAlignment::new().align_items(LayoutAlign::Start)),
        );
        let badged = ui.append(
            root,
            Badged::new(
                Panel::new().layout(Layout::new().logical_size(80.0, 40.0)),
                Panel::new().layout(Layout::new().logical_size(120.0, 70.0)),
            ),
        );

        ui.compute_layout(Size::new(200.0, 120.0));

        assert_eq!(ui.effective_bounds(badged), Rect::new(0.0, 0.0, 80.0, 40.0));
        let badge = badged_badge(&ui, badged).unwrap();
        assert_eq!(
            ui.effective_bounds(badge),
            Rect::new(20.0, -35.0, 140.0, 35.0)
        );
    }

    #[test]
    fn badge_can_be_removed_and_reinstalled() {
        let mut ui = Ui::new();
        let root = ui.root();
        let badged = ui.append(
            root,
            Badged::new_optional(
                Panel::new().layout(Layout::new().logical_size(80.0, 40.0)),
                None::<Badge<crate::Text>>,
            ),
        );
        assert!(badged_badge(&ui, badged).is_none());

        let badge = replace_badged_badge(&mut ui, badged, Badge::count(3)).unwrap();
        assert_eq!(badged_badge(&ui, badged), Some(badge));
        assert!(clear_badged_badge(&mut ui, badged));
        assert!(badged_badge(&ui, badged).is_none());
        assert!(badged_content(&ui, badged).is_some());
    }
}
