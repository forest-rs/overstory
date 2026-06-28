// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared overlay placement and lifecycle state.
//!
//! Roadmap:
//! - Keep placement and lifecycle shared by tooltips, popups, menus, combo
//!   boxes, flyouts, and future teaching UI. See ticket `ove-hfm1`.
//! - Expand placement beyond simple flipping: fallback chains, alignment,
//!   viewport padding, transforms, nested overlays, and anchor movement should
//!   be handled in one service.
//! - Integrate dismiss routing, focus scopes, pointer capture, Escape handling,
//!   and restore-focus semantics with routed events and focus runtime.
//! - Use composition overlay layers for ordering and host handoff without
//!   making individual overlay controls manage layer stacks.

use alloc::vec::Vec;

use hashbrown::HashMap;
use kurbo::{Insets, Point, Rect, Size, Vec2};
use understory_anchor::{
    Anchor, AnchorConstraint, AnchorInput, AnchorPolicy, AnchorPositionOption,
    Placement as AnchorPlacement, ScoringPolicy, resolve_anchor,
};

use crate::controls::tooltip::Tooltip;
use crate::element::ElementId;

/// Preferred placement for an anchored overlay.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Placement {
    /// Place the overlay below the target.
    #[default]
    Bottom,
    /// Place the overlay above the target.
    Top,
    /// Place the overlay to the target's right.
    Right,
    /// Place the overlay to the target's left.
    Left,
}

impl Placement {
    pub(crate) fn resolve(
        self,
        target: Rect,
        overlay: Size,
        viewport: Size,
        offset: Vec2,
    ) -> Point {
        if viewport.width <= 0.0 || viewport.height <= 0.0 {
            return self.raw_origin(target, overlay, offset);
        }

        let viewport = Rect::from_origin_size((0.0, 0.0), viewport);
        let (main_axis, cross_axis) = self.anchor_offsets(offset);
        let constraints = [
            AnchorConstraint::Offset {
                main_axis,
                cross_axis,
            },
            AnchorConstraint::Shift {
                padding: Insets::ZERO,
            },
        ];
        let preferred =
            AnchorPositionOption::new(self.anchor_placement()).with_constraints(&constraints);
        let fallbacks = [AnchorPositionOption::new(preferred.placement.opposite())
            .with_constraints(&constraints)];
        let mut policy = AnchorPolicy::new(preferred, &fallbacks);
        policy.scoring = ScoringPolicy {
            preferred_bonus: 0.0,
            incumbent_bonus: 0.0,
            ..ScoringPolicy::default()
        };
        resolve_anchor(
            AnchorInput {
                anchor: Anchor::Rect(target),
                floating_size: overlay,
                viewport,
                boundary: viewport,
                previous: None,
            },
            policy,
        )
        .rect
        .origin()
    }

    fn raw_origin(self, target: Rect, overlay: Size, offset: Vec2) -> Point {
        match self {
            Self::Bottom => Point::new(target.x0 + offset.x, target.y1 + offset.y),
            Self::Top => Point::new(target.x0 + offset.x, target.y0 - overlay.height - offset.y),
            Self::Right => Point::new(target.x1 + offset.x, target.y0 + offset.y),
            Self::Left => Point::new(target.x0 - overlay.width - offset.x, target.y0 + offset.y),
        }
    }

    fn opposite(self) -> Self {
        match self {
            Self::Bottom => Self::Top,
            Self::Top => Self::Bottom,
            Self::Right => Self::Left,
            Self::Left => Self::Right,
        }
    }

    fn anchor_placement(self) -> AnchorPlacement {
        match self {
            Self::Bottom => AnchorPlacement::BOTTOM_START,
            Self::Top => AnchorPlacement::TOP_START,
            Self::Right => AnchorPlacement::RIGHT_START,
            Self::Left => AnchorPlacement::LEFT_START,
        }
    }

    fn anchor_offsets(self, offset: Vec2) -> (f64, f64) {
        match self {
            Self::Bottom | Self::Top => (offset.y, offset.x),
            Self::Right | Self::Left => (offset.x, offset.y),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OverlayKind {
    /// Transient descriptive overlay anchored to an element.
    Tooltip,
    /// User-opened popup overlay that may participate in dismissal policy.
    Popup,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ActiveOverlay {
    /// Overlay category.
    pub(crate) kind: OverlayKind,
    /// Element the overlay is anchored to.
    pub(crate) target: ElementId,
    /// Overlay element inserted into the retained UI tree.
    pub(crate) element: ElementId,
    /// Preferred placement relative to `target`.
    pub(crate) placement: Placement,
    /// Placement offset in logical pixels.
    pub(crate) offset: Vec2,
    /// Dismissal behavior for this active overlay.
    pub(crate) dismissal: OverlayDismissal,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct OverlayDismissal {
    /// Close when a pointer down occurs outside the overlay.
    pub(crate) outside_pointer: bool,
    /// Close when Escape is pressed.
    pub(crate) escape: bool,
    /// Focus target to restore when the overlay is dismissed.
    pub(crate) restore_focus: Option<ElementId>,
}

impl OverlayDismissal {
    pub(crate) fn none() -> Self {
        Self::default()
    }
}

#[derive(Debug, Default)]
pub(crate) struct OverlayRuntime {
    /// Tooltip declarations keyed by the element they describe.
    tooltips: HashMap<ElementId, Tooltip>,
    /// Active overlay entries in opening order.
    ///
    /// Popups are treated as stack-like, so queries walk this vector from the
    /// end when they need the topmost popup.
    active: Vec<ActiveOverlay>,
}

impl OverlayRuntime {
    pub(crate) fn set_tooltip(&mut self, target: ElementId, tooltip: Tooltip) {
        self.tooltips.insert(target, tooltip);
    }

    pub(crate) fn tooltip_for(&self, target: ElementId) -> Option<&Tooltip> {
        self.tooltips.get(&target)
    }

    pub(crate) fn has_tooltip(&self, target: ElementId) -> bool {
        self.tooltips.contains_key(&target)
    }

    pub(crate) fn active_tooltip(&self) -> Option<ActiveOverlay> {
        self.active
            .iter()
            .find(|entry| entry.kind == OverlayKind::Tooltip)
            .copied()
    }

    pub(crate) fn active_popup(&self) -> Option<ActiveOverlay> {
        self.active
            .iter()
            .rev()
            .find(|entry| entry.kind == OverlayKind::Popup)
            .copied()
    }

    pub(crate) fn active_popups_top_to_bottom(&self) -> impl Iterator<Item = ActiveOverlay> + '_ {
        self.active
            .iter()
            .rev()
            .filter(|entry| entry.kind == OverlayKind::Popup)
            .copied()
    }

    pub(crate) fn active_entries(&self) -> impl Iterator<Item = ActiveOverlay> + '_ {
        self.active.iter().copied()
    }

    pub(crate) fn open(&mut self, entry: ActiveOverlay) {
        self.active.push(entry);
    }

    /// Removes an active overlay for the exact element/kind pair.
    pub(crate) fn take_element_kind(
        &mut self,
        element: ElementId,
        kind: OverlayKind,
    ) -> Option<ActiveOverlay> {
        let index = self
            .active
            .iter()
            .position(|entry| entry.element == element && entry.kind == kind)?;
        Some(self.active.remove(index))
    }

    /// Removes the oldest active overlay of `kind`.
    ///
    /// Tooltip activation uses this to keep only one live tooltip at a time.
    pub(crate) fn take_kind(&mut self, kind: OverlayKind) -> Option<ActiveOverlay> {
        let index = self.active.iter().position(|entry| entry.kind == kind)?;
        Some(self.active.remove(index))
    }

    /// Removes a tooltip declaration and closes it if it is active.
    pub(crate) fn remove_tooltip(&mut self, target: ElementId) -> Option<ElementId> {
        self.tooltips.remove(&target);
        let active = self.active_tooltip()?;
        if active.target == target {
            self.take_kind(OverlayKind::Tooltip)
                .map(|entry| entry.element)
        } else {
            None
        }
    }

    /// Removes overlay declarations and active entries involving `id`.
    ///
    /// Returns overlay elements that must also be removed from the retained UI
    /// tree by the caller.
    pub(crate) fn remove_element(&mut self, id: ElementId) -> Vec<ElementId> {
        self.tooltips.remove(&id);
        let mut removed = Vec::new();
        self.active.retain(|entry| {
            let keep = entry.target != id && entry.element != id;
            if !keep {
                removed.push(entry.element);
            }
            keep
        });
        removed
    }
}
