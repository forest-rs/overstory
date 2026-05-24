// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared overlay placement and lifecycle state.

use alloc::vec::Vec;

use hashbrown::HashMap;
use kurbo::{Point, Rect, Size, Vec2};

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
        let placement = self.flip_to_fit(target, overlay, viewport, offset);
        let origin = placement.raw_origin(target, overlay, offset);
        clamp_to_viewport(origin, overlay, viewport)
    }

    fn flip_to_fit(self, target: Rect, overlay: Size, viewport: Size, offset: Vec2) -> Self {
        if viewport.width <= 0.0 || viewport.height <= 0.0 {
            return self;
        }
        match self {
            Self::Bottom
                if target.y1 + offset.y + overlay.height > viewport.height
                    && target.y0 - offset.y - overlay.height >= 0.0 =>
            {
                Self::Top
            }
            Self::Top
                if target.y0 - offset.y - overlay.height < 0.0
                    && target.y1 + offset.y + overlay.height <= viewport.height =>
            {
                Self::Bottom
            }
            Self::Right
                if target.x1 + offset.x + overlay.width > viewport.width
                    && target.x0 - offset.x - overlay.width >= 0.0 =>
            {
                Self::Left
            }
            Self::Left
                if target.x0 - offset.x - overlay.width < 0.0
                    && target.x1 + offset.x + overlay.width <= viewport.width =>
            {
                Self::Right
            }
            _ => self,
        }
    }

    fn raw_origin(self, target: Rect, overlay: Size, offset: Vec2) -> Point {
        match self {
            Self::Bottom => Point::new(target.x0 + offset.x, target.y1 + offset.y),
            Self::Top => Point::new(target.x0 + offset.x, target.y0 - overlay.height - offset.y),
            Self::Right => Point::new(target.x1 + offset.x, target.y0 + offset.y),
            Self::Left => Point::new(target.x0 - overlay.width - offset.x, target.y0 + offset.y),
        }
    }
}

fn clamp_to_viewport(origin: Point, overlay: Size, viewport: Size) -> Point {
    if viewport.width <= 0.0 || viewport.height <= 0.0 {
        return origin;
    }
    Point::new(
        clamp_axis(origin.x, overlay.width, viewport.width),
        clamp_axis(origin.y, overlay.height, viewport.height),
    )
}

fn clamp_axis(origin: f64, extent: f64, viewport: f64) -> f64 {
    if extent >= viewport {
        0.0
    } else {
        origin.clamp(0.0, viewport - extent)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OverlayKind {
    Tooltip,
    Popup,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ActiveOverlay {
    pub(crate) kind: OverlayKind,
    pub(crate) target: ElementId,
    pub(crate) element: ElementId,
    pub(crate) placement: Placement,
    pub(crate) offset: Vec2,
    pub(crate) dismissal: OverlayDismissal,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct OverlayDismissal {
    pub(crate) outside_pointer: bool,
    pub(crate) escape: bool,
    pub(crate) restore_focus: Option<ElementId>,
}

impl OverlayDismissal {
    pub(crate) fn none() -> Self {
        Self::default()
    }
}

#[derive(Debug, Default)]
pub(crate) struct OverlayRuntime {
    tooltips: HashMap<ElementId, Tooltip>,
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

    pub(crate) fn take_kind(&mut self, kind: OverlayKind) -> Option<ActiveOverlay> {
        let index = self.active.iter().position(|entry| entry.kind == kind)?;
        Some(self.active.remove(index))
    }

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
