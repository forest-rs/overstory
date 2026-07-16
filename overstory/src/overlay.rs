// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained bridge from Overstory elements to Understory overlay mechanisms.
//!
//! This module owns element identity mapping, anchor continuity, overlay stack
//! mutation, and the latest coherent derived frame; it explicitly does not own
//! control semantics, retained-tree mutation, layout, rendering, or platform
//! focus movement.

use alloc::vec::Vec;

use hashbrown::HashMap;
use kurbo::{Insets, Point, Rect, Size, Vec2};
pub use understory_anchor::Placement;
use understory_anchor::{
    Anchor, AnchorConstraint, AnchorFrame, AnchorInput, AnchorPolicy, AnchorPositionOption,
    PreviousAnchorFrame, ScoringPolicy, Side, resolve_anchor,
};
use understory_overlay::{
    AnchorId, FocusPolicy, OverlayBehavior, OverlayEntry, OverlayEvent, OverlayEventResult,
    OverlayFrame, OverlayFrameInput, OverlayGeometry, OverlayId, OverlayLayer, OverlayOp,
    OverlayStack, build_overlay_frame, point_in_grace_shape, resolve_event,
};

use crate::controls::tooltip::Tooltip;
use crate::element::ElementId;

fn resolve_anchor_frame(
    placement: Placement,
    anchor: Anchor<'_>,
    overlay: Size,
    viewport: Size,
    offset: Vec2,
    previous: Option<&PreviousAnchorFrame>,
) -> AnchorFrame {
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
    policy.scoring = ScoringPolicy::collision_first();
    resolve_anchor(
        AnchorInput {
            anchor,
            floating_size: overlay,
            viewport,
            boundary: viewport,
            previous,
        },
        policy,
    )
}

/// Geometry used to place an overlay while retaining a semantic element
/// target for lifecycle and accessibility relationships.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum OverlayAnchor {
    /// Follow the current bounds of the overlay target.
    TargetBounds,
    /// Stay at the scene-space point captured when the overlay opened.
    Point(Point),
}

impl OverlayAnchor {
    pub(crate) fn understory(self, target_bounds: Rect) -> Anchor<'static> {
        match self {
            Self::TargetBounds => Anchor::Rect(target_bounds),
            Self::Point(point) => Anchor::Point(point),
        }
    }

    pub(crate) fn reference_rect(self, target_bounds: Rect) -> Rect {
        match self {
            Self::TargetBounds => target_bounds,
            Self::Point(point) => Rect::new(point.x, point.y, point.x, point.y),
        }
    }
}

fn anchor_offsets(side: Side, offset: Vec2) -> (f64, f64) {
    match side {
        Side::Bottom | Side::Top => (offset.y, offset.x),
        Side::Right | Side::Left => (offset.x, offset.y),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct OverlayHost {
    /// Semantic interaction policy governing this overlay.
    pub(crate) behavior: OverlayBehavior,
    /// Element the overlay is anchored to.
    pub(crate) target: ElementId,
    /// Overlay element inserted into the retained UI tree.
    pub(crate) element: ElementId,
    /// Semantic content represented by the overlay host.
    pub(crate) content: ElementId,
    /// Geometry source used by the shared anchor resolver.
    pub(crate) anchor: OverlayAnchor,
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

#[derive(Debug)]
struct OverlayInstance {
    host: OverlayHost,
    anchor_frame: Option<AnchorFrame>,
}

#[derive(Debug, Default)]
pub(crate) struct OverlayRuntime {
    /// Tooltip declarations keyed by the element they describe.
    tooltips: HashMap<ElementId, Tooltip>,
    /// Headless overlay lifecycle, ordering, and dismissal policy.
    stack: OverlayStack,
    /// Overstory-owned host mapping and anchor continuity for each entry.
    instances: HashMap<OverlayId, OverlayInstance>,
    /// Latest coherent derived frame for event routing and future rendering.
    frame: OverlayFrame,
    /// Latest scene-space pointer position used to test submenu grace.
    pointer: Option<Point>,
    /// Stable pointer origin for the current submenu-travel grace shape.
    grace_pointer: Option<Point>,
    /// Whether stack or geometry changes require a replacement frame.
    frame_dirty: bool,
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
            .and_then(|entry| self.instances.get(&entry.id))
            .map(|instance| instance.host)
    }

    pub(crate) fn active_entries(&self) -> impl Iterator<Item = OverlayHost> + '_ {
        self.stack
            .entries()
            .iter()
            .filter_map(|entry| self.instances.get(&entry.id))
            .map(|instance| instance.host)
    }

    pub(crate) fn entries(&self) -> impl Iterator<Item = (&OverlayEntry, OverlayHost)> + '_ {
        self.stack.entries().iter().filter_map(|entry| {
            self.instances
                .get(&entry.id)
                .map(|instance| (entry, instance.host))
        })
    }

    pub(crate) fn frame(&self) -> &OverlayFrame {
        &self.frame
    }

    pub(crate) fn host_for(&self, overlay: OverlayId) -> Option<OverlayHost> {
        self.instances.get(&overlay).map(|instance| instance.host)
    }

    pub(crate) fn active_for_target(
        &self,
        target: ElementId,
        behavior: OverlayBehavior,
    ) -> Option<OverlayHost> {
        self.stack
            .entries()
            .iter()
            .rev()
            .filter(|entry| entry.behavior == behavior)
            .filter_map(|entry| self.instances.get(&entry.id))
            .find(|instance| instance.host.target == target)
            .map(|instance| instance.host)
    }

    pub(crate) fn active_for_behavior(&self, behavior: OverlayBehavior) -> Option<OverlayHost> {
        self.stack
            .entries()
            .iter()
            .rev()
            .find(|entry| entry.behavior == behavior)
            .and_then(|entry| self.instances.get(&entry.id))
            .map(|instance| instance.host)
    }

    pub(crate) fn topmost_non_tooltip(&self) -> Option<OverlayHost> {
        self.stack
            .entries()
            .iter()
            .rev()
            .find(|entry| entry.behavior != OverlayBehavior::Tooltip)
            .and_then(|entry| self.instances.get(&entry.id))
            .map(|instance| instance.host)
    }

    pub(crate) fn active_for_element(&self, element: ElementId) -> Option<OverlayHost> {
        self.instances
            .get(&overlay_id(element))
            .map(|instance| instance.host)
    }

    pub(crate) fn active_for_content(&self, content: ElementId) -> Option<OverlayHost> {
        self.instances
            .values()
            .find(|instance| instance.host.content == content)
            .map(|instance| instance.host)
    }

    pub(crate) fn child_anchor_towards(
        &self,
        ancestor: ElementId,
        descendant: ElementId,
    ) -> Option<ElementId> {
        let ancestor = overlay_id(ancestor);
        let mut child = overlay_id(descendant);
        self.stack.entry(ancestor)?;
        self.stack.entry(child)?;
        while let Some(parent) = self.stack.parent_of(child) {
            if parent == ancestor {
                return self
                    .instances
                    .get(&child)
                    .map(|instance| instance.host.target);
            }
            child = parent;
        }
        None
    }

    pub(crate) fn root_of(&self, element: ElementId) -> Option<OverlayHost> {
        let mut root = overlay_id(element);
        self.stack.entry(root)?;
        while let Some(parent) = self.stack.parent_of(root) {
            root = parent;
        }
        self.instances.get(&root).map(|instance| instance.host)
    }

    pub(crate) fn targets_to_root(&self, element: ElementId) -> Vec<ElementId> {
        let mut overlay = overlay_id(element);
        let mut targets = Vec::new();
        while let Some(instance) = self.instances.get(&overlay) {
            targets.push(instance.host.target);
            let Some(parent) = self.stack.parent_of(overlay) else {
                break;
            };
            overlay = parent;
        }
        targets
    }

    pub(crate) fn topmost_containing_focus_scope(&self) -> Option<OverlayHost> {
        self.frame
            .focus_scopes
            .iter()
            .rev()
            .find(|scope| scope.contain)
            .and_then(|scope| self.instances.get(&scope.overlay))
            .map(|instance| instance.host)
    }

    pub(crate) fn set_pointer(&mut self, pointer: Point) {
        self.pointer = Some(pointer);
    }

    pub(crate) fn begin_submenu_grace(&mut self, target: ElementId) -> bool {
        if self.pointer.is_none()
            || self
                .active_for_target(target, OverlayBehavior::Submenu)
                .is_none()
        {
            return false;
        }
        self.grace_pointer = self.pointer;
        self.frame_dirty = true;
        true
    }

    pub(crate) fn pointer_keeps_submenu_open(&self, target: ElementId) -> bool {
        let Some(pointer) = self.pointer else {
            return false;
        };
        let Some(child) = self.stack.entries().iter().rev().find(|entry| {
            entry.behavior == OverlayBehavior::Submenu
                && self
                    .instances
                    .get(&entry.id)
                    .is_some_and(|instance| instance.host.target == target)
        }) else {
            return false;
        };
        if self.frame.overlays.iter().any(|entry| {
            (entry.id == child.id || self.stack.is_descendant_of(entry.id, child.id))
                && entry.rect.contains(pointer)
        }) {
            return true;
        }
        self.frame.grace_regions.iter().any(|region| {
            (region.child == child.id || self.stack.is_descendant_of(region.child, child.id))
                && point_in_grace_shape(pointer, region.shape)
        })
    }

    fn anchor_frame(&self, overlay: OverlayId) -> Option<&AnchorFrame> {
        self.instances.get(&overlay)?.anchor_frame.as_ref()
    }

    pub(crate) fn resolve_anchor_rect(
        &mut self,
        overlay: OverlayId,
        target: Rect,
        size: Size,
        viewport: Size,
    ) -> Rect {
        let instance = self
            .instances
            .get_mut(&overlay)
            .expect("anchor resolution requires an active overlay");
        let previous = instance
            .anchor_frame
            .as_ref()
            .map(PreviousAnchorFrame::from);
        let frame = resolve_anchor_frame(
            instance.host.placement,
            instance.host.anchor.understory(target),
            size,
            viewport,
            instance.host.offset,
            previous.as_ref(),
        );
        let rect = frame.rect;
        instance.anchor_frame = Some(frame);
        self.frame_dirty = true;
        rect
    }

    pub(crate) fn frame_needs_refresh(&self) -> bool {
        self.frame_dirty
    }

    pub(crate) fn rebuild_frame(&mut self, input: OverlayFrameInput<'_>) {
        let frame = {
            let geometries = input
                .geometries
                .iter()
                .map(|geometry| {
                    self.anchor_frame(geometry.overlay)
                        .map_or(*geometry, |frame| {
                            OverlayGeometry::from_anchor_frame(geometry.overlay, frame)
                        })
                })
                .collect::<Vec<_>>();
            build_overlay_frame(
                &self.stack,
                OverlayFrameInput {
                    viewport: input.viewport,
                    geometries: &geometries,
                    pointer: self.grace_pointer.or(input.pointer),
                    grace_policy: input.grace_policy,
                },
            )
        };
        self.frame = frame;
        self.frame_dirty = false;
    }

    pub(crate) fn resolve_event(&self, event: OverlayEvent) -> OverlayEventResult {
        assert!(
            !self.frame_dirty,
            "overlay events require a frame coherent with the current stack and geometry"
        );
        resolve_event(&self.stack, &self.frame, event)
    }

    pub(crate) fn open_tooltip(&mut self, host: OverlayHost) {
        let entry = OverlayEntry::new(
            overlay_id(host.element),
            OverlayLayer::Tooltip,
            OverlayBehavior::Tooltip,
        )
        .with_anchor(anchor_id(host.target));
        self.open(entry, host);
    }

    pub(crate) fn open_popup(
        &mut self,
        host: OverlayHost,
        parent: Option<ElementId>,
        outside_pointer: bool,
        escape: bool,
        focus: FocusPolicy,
    ) {
        let behavior = host.behavior;
        if behavior == OverlayBehavior::Submenu {
            self.grace_pointer = self.pointer;
        }
        let mut dismiss = behavior.dismiss_policy();
        dismiss.escape_key = escape;
        dismiss.pointer_down_outside = outside_pointer;
        let mut entry =
            OverlayEntry::new(overlay_id(host.element), overlay_layer(behavior), behavior)
                .with_anchor(anchor_id(host.target))
                .with_dismiss(dismiss)
                .with_focus(focus);
        if let Some(parent) = parent {
            entry = entry.with_parent(overlay_id(parent));
        }
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
        self.instances.insert(
            id,
            OverlayInstance {
                host,
                anchor_frame: None,
            },
        );
        self.frame_dirty = true;
    }

    pub(crate) fn close_popup(&mut self, element: ElementId) -> Vec<OverlayHost> {
        self.take_element(element, |entry| entry.layer != OverlayLayer::Tooltip)
    }

    pub(crate) fn close_descendants(&mut self, element: ElementId) -> Vec<OverlayHost> {
        let overlay = overlay_id(element);
        if self.stack.entry(overlay).is_none() {
            return Vec::new();
        }
        self.apply_operation(OverlayOp::CloseDescendants { overlay })
    }

    pub(crate) fn close_tree(&mut self, element: ElementId) -> Vec<OverlayHost> {
        let Some(root) = self.root_of(element).map(OverlayHost::overlay_id) else {
            return Vec::new();
        };
        self.apply_operation(OverlayOp::CloseSubtree { overlay: root })
    }

    pub(crate) fn take_tooltip(&mut self) -> Vec<OverlayHost> {
        let Some(element) = self.active_tooltip().map(|host| host.element) else {
            return Vec::new();
        };
        self.take_element(element, |entry| entry.behavior == OverlayBehavior::Tooltip)
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
    pub(crate) fn remove_element(&mut self, id: ElementId) -> Vec<OverlayHost> {
        self.tooltips.remove(&id);
        let overlays = self
            .stack
            .entries()
            .iter()
            .filter_map(|entry| {
                let instance = self.instances.get(&entry.id)?;
                (instance.host.target == id || instance.host.element == id).then_some(entry.id)
            })
            .collect::<Vec<_>>();
        overlays
            .into_iter()
            .flat_map(|overlay| self.close(overlay))
            .collect()
    }

    pub(crate) fn apply_operation(&mut self, operation: OverlayOp) -> Vec<OverlayHost> {
        assert!(
            !matches!(operation, OverlayOp::Open { .. }),
            "opening an overlay also requires an Overstory host"
        );
        let removed = self
            .stack
            .apply(operation)
            .expect("resolved overlay operation references the current stack")
            .into_removed();
        if !self
            .stack
            .entries()
            .iter()
            .any(|entry| entry.behavior == OverlayBehavior::Submenu)
        {
            self.grace_pointer = None;
        }
        self.frame_dirty = true;
        removed
            .into_iter()
            .filter_map(|overlay| self.instances.remove(&overlay))
            .map(|instance| instance.host)
            .collect()
    }

    fn take_element(
        &mut self,
        element: ElementId,
        matches: impl FnOnce(&OverlayEntry) -> bool,
    ) -> Vec<OverlayHost> {
        let overlay = overlay_id(element);
        if self
            .stack
            .entry(overlay)
            .is_none_or(|entry| !matches(entry))
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

fn overlay_layer(behavior: OverlayBehavior) -> OverlayLayer {
    match behavior {
        OverlayBehavior::Tooltip => OverlayLayer::Tooltip,
        OverlayBehavior::HoverCard | OverlayBehavior::Popover => OverlayLayer::Popover,
        OverlayBehavior::Menu
        | OverlayBehavior::Submenu
        | OverlayBehavior::ContextMenu
        | OverlayBehavior::ComboboxPopup => OverlayLayer::Menu,
        OverlayBehavior::Dialog => OverlayLayer::Modal,
    }
}

fn overlay_id(element: ElementId) -> OverlayId {
    OverlayId(element_token(element))
}

pub(crate) fn anchor_id(element: ElementId) -> AnchorId {
    AnchorId(element_token(element))
}

pub(crate) fn element_token(element: ElementId) -> u64 {
    (u64::from(element.generation()) << 32) | u64::from(element.slot())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(target: u32, element: u32, behavior: OverlayBehavior) -> OverlayHost {
        OverlayHost {
            behavior,
            target: ElementId::new(target, 0),
            element: ElementId::new(element, 0),
            content: ElementId::new(element, 0),
            anchor: OverlayAnchor::TargetBounds,
            placement: Placement::BOTTOM,
            offset: Vec2::ZERO,
            restore_focus: None,
        }
    }

    #[test]
    fn mutation_report_removes_every_host_closed_by_an_operation() {
        let mut runtime = OverlayRuntime::default();
        let popup = host(1, 2, OverlayBehavior::Popover);
        let tooltip = host(3, 4, OverlayBehavior::Tooltip);
        runtime.open_popup(popup, None, true, true, FocusPolicy::None);
        runtime.open_tooltip(tooltip);

        let removed = runtime.apply_operation(OverlayOp::CloseAll);

        assert_eq!(removed, [popup, tooltip]);
        assert!(runtime.stack.entries().is_empty());
        assert!(runtime.instances.is_empty());
    }

    #[test]
    fn retained_anchor_and_overlay_frames_drive_event_resolution() {
        let mut runtime = OverlayRuntime::default();
        let popup = host(1, 2, OverlayBehavior::Popover);
        runtime.open_popup(popup, None, true, true, FocusPolicy::None);
        assert!(runtime.frame_needs_refresh());

        let overlay_rect = runtime.resolve_anchor_rect(
            popup.overlay_id(),
            Rect::new(20.0, 20.0, 80.0, 40.0),
            Size::new(80.0, 30.0),
            Size::new(240.0, 120.0),
        );
        let fallback_geometry = [OverlayGeometry::new(popup.overlay_id(), Rect::ZERO)];
        runtime.rebuild_frame(OverlayFrameInput::new(
            Rect::new(0.0, 0.0, 240.0, 120.0),
            &fallback_geometry,
        ));

        assert!(!runtime.frame_needs_refresh());
        assert_eq!(runtime.frame.overlays[0].rect, overlay_rect);
        let result = runtime.resolve_event(OverlayEvent::PointerDown {
            point: overlay_rect.center(),
        });
        assert!(result.ops.is_empty());
        assert!(!runtime.frame_needs_refresh());

        let _ = runtime.apply_operation(OverlayOp::Close {
            overlay: popup.overlay_id(),
        });
        assert!(runtime.frame_needs_refresh());
    }
}
