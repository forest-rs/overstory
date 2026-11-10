// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Keyed retained realization of docking frames as Overstory elements.
//!
//! This module owns docking chrome and the retained identity of realized frame
//! items; it explicitly does not own pane content, application commands, or
//! the semantic tile tree.

use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use overstory::{ChildUi, ElementId, Layout, Panel, SurfaceSpecExt, Ui};
use understory_tiling::{
    DropTargetId, InteractionState, LayoutFrame, OverlayFrame, PaneId, Point, Rect,
    TabGroupGeometry,
};

use crate::DockState;

mod events;
mod geometry;
mod realization;
mod style;

use events::DockTabEvents;
pub use geometry::DockGeometryError;
use realization::{DockLayer, DockSurfaceInner, absolute_layout, hide};
use style::apply_paint;
pub use style::{DockPaint, DockSurfaceStyle};

/// How a surface presents a proposed docking layout during a drag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockDragPreview {
    /// Keep the committed panes in place and show the placement outline or insertion marker.
    Outline,
    /// Arrange the panes and tabs in their proposed positions.
    ///
    /// The proposed tab order replaces the marker drawn on the committed strip.
    FullLayout,
}

/// Reusable retained presentation of one [`DockState`].
///
/// `DockSurface` owns the Overstory elements used for panes, tabs, tab bars,
/// split handles, and interaction overlays. [`Self::sync`] reconciles those
/// elements by stable tiling identifiers, preserving element identity across
/// layout passes and removing elements whose semantic items disappeared.
/// Applications provide pane contents and pane titles; the surface owns the
/// docking chrome around them.
#[derive(Clone, Debug)]
pub struct DockSurface {
    inner: Rc<RefCell<DockSurfaceInner>>,
    tab_events: Rc<DockTabEvents>,
}

impl DockSurface {
    /// Mounts a surface with [`DockSurfaceStyle::default`].
    #[must_use]
    pub fn mount(ui: &mut ChildUi<'_>, layout: Layout) -> Self {
        Self::mount_with_style(ui, layout, DockSurfaceStyle::default())
    }

    /// Mounts a surface with explicit visual styling.
    #[must_use]
    pub fn mount_with_style(ui: &mut ChildUi<'_>, layout: Layout, style: DockSurfaceStyle) -> Self {
        let tab_events = Rc::new(DockTabEvents::default());
        let root = ui.add(Panel::new().layout(layout.relative()));
        let (pane_layer, chrome_layer, overlay_layer) = ui.ui_mut().with_children(root, |root| {
            let layer_layout = || Layout::new().absolute_at(0.0, 0.0).fill_size();
            (
                root.add(DockLayer::new(layer_layout())),
                root.add(DockLayer::new(layer_layout())),
                root.add(DockLayer::new(layer_layout())),
            )
        });
        let dragged = ui.ui_mut().append(
            overlay_layer,
            Panel::new().layout(absolute_layout()).corner_radius(3.0),
        );
        hide(ui.ui_mut(), dragged);
        apply_paint(ui.ui_mut(), root, style.surface);
        apply_paint(ui.ui_mut(), dragged, style.dragged);

        Self {
            inner: Rc::new(RefCell::new(DockSurfaceInner::new(
                root,
                pane_layer,
                chrome_layer,
                overlay_layer,
                dragged,
                style,
            ))),
            tab_events,
        }
    }

    /// Returns the surface root used for pointer routing and layout queries.
    #[must_use]
    pub fn root(&self) -> ElementId {
        self.inner.borrow().root
    }

    /// Returns the frame-local point for a window-local pointer position.
    ///
    /// The frame origin is the retained pane layer's content origin. This
    /// keeps pointer coordinates aligned with absolutely positioned frame
    /// items when the surface itself has a border or padding.
    #[must_use]
    pub fn local_point(&self, ui: &Ui, position: Point) -> Point {
        let pane_layer = self.inner.borrow().pane_layer;
        let bounds = ui.effective_bounds(pane_layer);
        Point::new(position.x - bounds.x0, position.y - bounds.y0)
    }

    /// Returns the surface's current frame-local bounds.
    #[must_use]
    pub fn local_bounds(&self, ui: &Ui) -> Rect {
        let pane_layer = self.inner.borrow().pane_layer;
        let bounds = ui.effective_bounds(pane_layer);
        Rect::new(0.0, 0.0, bounds.width(), bounds.height())
    }

    /// Returns the pane wrapper for `pane` when it is currently realized.
    #[must_use]
    pub fn pane_element(&self, pane: PaneId) -> Option<ElementId> {
        self.inner
            .borrow()
            .panes
            .iter()
            .find(|view| view.pane == pane)
            .map(|view| view.body)
    }

    /// Returns the number of currently realized pane wrappers.
    #[must_use]
    pub fn realized_pane_count(&self) -> usize {
        self.inner.borrow().panes.len()
    }

    /// Installs the application handler for semantic tab activation.
    ///
    /// Pointer, keyboard, and accessibility selection from retained tab bars
    /// all arrive here as application pane ids. Installing another handler
    /// replaces the previous one.
    pub fn set_tab_activated_handler<F>(&self, handler: F)
    where
        F: FnMut(&mut Ui, PaneId) + 'static,
    {
        self.tab_events.set_activated_handler(handler);
    }

    /// Applies the currently arranged tab geometry to `state`.
    ///
    /// Call this after [`Self::sync`] and UI layout, before beginning a pointer
    /// interaction, so Understory hit testing and reorder insertion use the
    /// same rectangles as the retained tab controls. The call fails rather
    /// than mixing provisional and measured geometry when the surface, layout,
    /// or semantic pane set is incomplete or stale.
    pub fn apply_tab_geometry(
        &self,
        ui: &Ui,
        state: &mut DockState,
    ) -> Result<(), DockGeometryError> {
        let groups = self.tab_group_geometry(ui, state)?;
        state.apply_tab_group_geometry(&groups)?;
        Ok(())
    }

    /// Captures complete presentation-resolved geometry for the current frame.
    ///
    /// The surface must have been synchronized with `state`, and pending UI
    /// layout must already have been drained. Every semantic tab group and pane
    /// must have a corresponding retained realization.
    pub fn tab_group_geometry(
        &self,
        ui: &Ui,
        state: &DockState,
    ) -> Result<Vec<TabGroupGeometry>, DockGeometryError> {
        let inner = self.inner.borrow();
        geometry::capture(ui, state, &inner)
    }

    /// Replaces the visual style used by future synchronization passes.
    ///
    /// The surface and dragged-subject paint update immediately. Call
    /// [`Self::sync`] afterward to repaint currently realized frame items.
    pub fn set_style(&self, ui: &mut Ui, style: DockSurfaceStyle) {
        let mut inner = self.inner.borrow_mut();
        inner.style = style;
        apply_paint(ui, inner.root, style.surface);
        apply_paint(ui, inner.dragged, style.dragged);
    }

    /// Updates only interaction feedback, retaining the currently realized panes and tabs.
    ///
    /// Use this while dragging when the host wants to draw a proposed
    /// placement over the committed layout. Call [`Self::sync`] when the drag ends.
    pub fn sync_overlay(&self, ui: &mut Ui, overlay: &OverlayFrame) {
        self.inner.borrow_mut().sync_overlay(ui, overlay);
    }

    /// Reconciles retained elements with the state's display frame and overlay.
    ///
    /// `build_pane` runs only when a pane id first appears (or reappears after
    /// removal), with a scoped builder rooted inside that pane's retained
    /// wrapper. `pane_title` supplies the current title for tab chrome.
    pub fn sync<B, T>(&self, ui: &mut Ui, state: &DockState, build_pane: B, pane_title: T)
    where
        B: FnMut(PaneId, &mut ChildUi<'_>),
        T: FnMut(PaneId) -> String,
    {
        self.sync_frame(
            ui,
            state,
            state.display_frame(),
            state.overlay(),
            build_pane,
            pane_title,
        );
    }

    /// Reconciles the surface using the host's drag preview preference.
    ///
    /// This affects presentation only. Both modes use the same committed hit
    /// geometry and proposal. Split resize continues to show its live layout.
    /// Pane realization follows the same rules as [`Self::sync`].
    pub fn sync_with_drag_preview<B, T>(
        &self,
        ui: &mut Ui,
        state: &DockState,
        mode: DockDragPreview,
        build_pane: B,
        pane_title: T,
    ) where
        B: FnMut(PaneId, &mut ChildUi<'_>),
        T: FnMut(PaneId) -> String,
    {
        if !matches!(state.interaction(), InteractionState::Drag(_)) {
            self.sync(ui, state, build_pane, pane_title);
            return;
        }

        let overlay = state.overlay();
        let preview = match mode {
            DockDragPreview::Outline => None,
            DockDragPreview::FullLayout => state.preview_frame(),
        };
        let tab_preview_overlay;
        let overlay = if preview.is_some()
            && matches!(overlay.active_target, Some(DropTargetId::TabBar(_)))
        {
            // This marker refers to the committed strip's gap. In a full
            // preview the moved tab itself shows the resulting insertion.
            tab_preview_overlay = OverlayFrame {
                dragged: overlay.dragged,
                ..OverlayFrame::default()
            };
            &tab_preview_overlay
        } else {
            overlay
        };
        let frame = preview.unwrap_or_else(|| state.frame());
        self.sync_frame(ui, state, frame, overlay, build_pane, pane_title);
    }

    fn sync_frame<B, T>(
        &self,
        ui: &mut Ui,
        state: &DockState,
        frame: &LayoutFrame,
        overlay: &OverlayFrame,
        mut build_pane: B,
        mut pane_title: T,
    ) where
        B: FnMut(PaneId, &mut ChildUi<'_>),
        T: FnMut(PaneId) -> String,
    {
        let mut inner = self.inner.borrow_mut();
        let panes = state.pane_ids();
        let _suppression = self.tab_events.suppress();
        inner.sync_panes(ui, &panes, frame.panes(), state.selected(), &mut build_pane);
        inner.sync_tab_groups(
            ui,
            frame.tab_bars(),
            frame.tabs(),
            &mut pane_title,
            &self.tab_events,
        );
        inner.sync_handles(ui, frame.split_handles());
        inner.sync_collapsed_tiles(ui, frame.collapsed_tiles());
        inner.sync_overlay(ui, overlay);
        // Preview geometry is never authoritative for the next gesture.
        inner.synchronized_frame_epoch =
            core::ptr::eq(frame, state.frame()).then_some(state.frame_epoch());
    }
}

#[cfg(test)]
mod tests;
