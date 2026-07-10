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
pub use understory_anchor::Placement;
use understory_anchor::{
    Anchor, AnchorConstraint, AnchorInput, AnchorPolicy, AnchorPositionOption, ScoringPolicy, Side,
    resolve_anchor,
};
use understory_overlay::{
    AnchorId, DismissPolicy, FocusPolicy, OverlayBehavior, OverlayEntry, OverlayId, OverlayLayer,
    OverlayOp, OverlayStack,
};

use crate::controls::tooltip::Tooltip;
use crate::element::ElementId;

pub(crate) fn resolve_placement(
    placement: Placement,
    target: Rect,
    overlay: Size,
    viewport: Size,
    offset: Vec2,
) -> Point {
    let viewport = Rect::from_origin_size((0.0, 0.0), viewport);
    let (main_axis, cross_axis) = anchor_offsets(placement.side, offset);
    let constraints = [
        AnchorConstraint::Offset {
            main_axis,
            cross_axis,
        },
        AnchorConstraint::Shift {
            padding: Insets::ZERO,
        },
    ];
    let preferred = AnchorPositionOption::new(placement).with_constraints(&constraints);
    let fallbacks =
        [AnchorPositionOption::new(placement.opposite()).with_constraints(&constraints)];
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

fn anchor_offsets(side: Side, offset: Vec2) -> (f64, f64) {
    match side {
        Side::Bottom | Side::Top => (offset.y, offset.x),
        Side::Right | Side::Left => (offset.x, offset.y),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct OverlayHost {
    /// Element the overlay is anchored to.
    pub(crate) target: ElementId,
    /// Overlay element inserted into the retained UI tree.
    pub(crate) element: ElementId,
    /// Preferred placement relative to `target`.
    pub(crate) placement: Placement,
    /// Placement offset in logical pixels.
    pub(crate) offset: Vec2,
    /// Focus target captured when the overlay opened.
    pub(crate) restore_focus: Option<ElementId>,
}

impl OverlayHost {
    pub(crate) fn overlay_id(self) -> OverlayId {
        overlay_id(self.element)
    }
}

#[derive(Debug, Default)]
pub(crate) struct OverlayRuntime {
    /// Tooltip declarations keyed by the element they describe.
    tooltips: HashMap<ElementId, Tooltip>,
    /// Headless overlay lifecycle, ordering, and dismissal policy.
    stack: OverlayStack,
    /// Overstory-owned element and placement state for each headless entry.
    hosts: HashMap<OverlayId, OverlayHost>,
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

    pub(crate) fn active_tooltip(&self) -> Option<OverlayHost> {
        self.stack
            .entries()
            .iter()
            .find(|entry| entry.behavior == OverlayBehavior::Tooltip)
            .and_then(|entry| self.hosts.get(&entry.id))
            .copied()
    }

    pub(crate) fn active_entries(&self) -> impl Iterator<Item = OverlayHost> + '_ {
        self.stack
            .entries()
            .iter()
            .filter_map(|entry| self.hosts.get(&entry.id).copied())
    }

    pub(crate) fn stack(&self) -> &OverlayStack {
        &self.stack
    }

    pub(crate) fn open_tooltip(&mut self, host: OverlayHost) {
        let entry = OverlayEntry::new(
            overlay_id(host.element),
            OverlayLayer::Tooltip,
            OverlayBehavior::Tooltip,
        )
        .with_anchor(anchor_id(host.target))
        .with_dismiss(DismissPolicy::default());
        self.open(entry, host);
    }

    pub(crate) fn open_popup(&mut self, host: OverlayHost, outside_pointer: bool, escape: bool) {
        let entry = OverlayEntry::new(
            overlay_id(host.element),
            OverlayLayer::Popover,
            OverlayBehavior::Popover,
        )
        .with_anchor(anchor_id(host.target))
        .with_dismiss(DismissPolicy {
            escape_key: escape,
            pointer_down_outside: outside_pointer,
            ..DismissPolicy::default()
        })
        .with_focus(if host.restore_focus.is_some() {
            FocusPolicy::RestoreOnClose
        } else {
            FocusPolicy::None
        });
        self.open(entry, host);
    }

    fn open(&mut self, entry: OverlayEntry, host: OverlayHost) {
        let id = entry.id;
        let report = self
            .stack
            .apply(OverlayOp::Open { entry })
            .expect("fresh retained overlay element has a unique valid id");
        debug_assert!(
            report.is_empty(),
            "opening an overlay must not remove entries"
        );
        self.hosts.insert(id, host);
    }

    pub(crate) fn close_popup(&mut self, element: ElementId) -> Vec<OverlayHost> {
        self.take_element_behavior(element, OverlayBehavior::Popover)
    }

    pub(crate) fn take_tooltip(&mut self) -> Vec<OverlayHost> {
        let Some(element) = self.active_tooltip().map(|host| host.element) else {
            return Vec::new();
        };
        self.take_element_behavior(element, OverlayBehavior::Tooltip)
    }

    /// Removes a tooltip declaration and closes it if it is active.
    pub(crate) fn remove_tooltip(&mut self, target: ElementId) -> Vec<OverlayHost> {
        self.tooltips.remove(&target);
        let Some(active) = self.active_tooltip() else {
            return Vec::new();
        };
        if active.target == target {
            self.take_tooltip()
        } else {
            Vec::new()
        }
    }

    /// Removes overlay declarations and active entries involving `id`.
    ///
    /// Returns overlay elements that must also be removed from the retained UI
    /// tree by the caller.
    pub(crate) fn remove_element(&mut self, id: ElementId) -> Vec<ElementId> {
        self.tooltips.remove(&id);
        let overlays = self
            .stack
            .entries()
            .iter()
            .filter_map(|entry| {
                let host = self.hosts.get(&entry.id)?;
                (host.target == id || host.element == id).then_some(entry.id)
            })
            .collect::<Vec<_>>();
        overlays
            .into_iter()
            .flat_map(|overlay| self.close(overlay))
            .map(|host| host.element)
            .collect()
    }

    pub(crate) fn apply_operation(&mut self, operation: OverlayOp) -> Vec<OverlayHost> {
        assert!(
            !matches!(operation, OverlayOp::Open { .. }),
            "opening an overlay also requires an Overstory host"
        );
        self.stack
            .apply(operation)
            .expect("resolved overlay operation references the current stack")
            .into_removed()
            .into_iter()
            .filter_map(|overlay| self.hosts.remove(&overlay))
            .collect()
    }

    fn take_element_behavior(
        &mut self,
        element: ElementId,
        behavior: OverlayBehavior,
    ) -> Vec<OverlayHost> {
        let overlay = overlay_id(element);
        if self
            .stack
            .entry(overlay)
            .is_none_or(|entry| entry.behavior != behavior)
        {
            return Vec::new();
        }
        self.close(overlay)
    }

    fn close(&mut self, overlay: OverlayId) -> Vec<OverlayHost> {
        if self.stack.entry(overlay).is_none() {
            return Vec::new();
        }
        self.apply_operation(OverlayOp::Close { overlay })
    }
}

fn overlay_id(element: ElementId) -> OverlayId {
    OverlayId(element_token(element))
}

fn anchor_id(element: ElementId) -> AnchorId {
    AnchorId(element_token(element))
}

fn element_token(element: ElementId) -> u64 {
    (u64::from(element.generation()) << 32) | u64::from(element.slot())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(target: u32, element: u32) -> OverlayHost {
        OverlayHost {
            target: ElementId::new(target, 0),
            element: ElementId::new(element, 0),
            placement: Placement::BOTTOM,
            offset: Vec2::ZERO,
            restore_focus: None,
        }
    }

    #[test]
    fn mutation_report_removes_every_host_closed_by_an_operation() {
        let mut runtime = OverlayRuntime::default();
        let popup = host(1, 2);
        let tooltip = host(3, 4);
        runtime.open_popup(popup, true, true);
        runtime.open_tooltip(tooltip);

        let removed = runtime.apply_operation(OverlayOp::CloseAll);

        assert_eq!(removed, [popup, tooltip]);
        assert!(runtime.stack().entries().is_empty());
        assert!(runtime.hosts.is_empty());
    }
}
