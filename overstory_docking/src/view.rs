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
use understory_tiling::{PaneId, Point, Rect, TabGroupGeometry};

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
    pub fn sync_overlay(&self, ui: &mut Ui, overlay: &understory_tiling::OverlayFrame) {
        self.inner.borrow_mut().sync_overlay(ui, overlay);
    }

    /// Reconciles retained elements with the state's display frame and overlay.
    ///
    /// `build_pane` runs only when a pane id first appears (or reappears after
    /// removal), with a scoped builder rooted inside that pane's retained
    /// wrapper. `pane_title` supplies the current title for tab chrome.
    pub fn sync<B, T>(&self, ui: &mut Ui, state: &DockState, mut build_pane: B, mut pane_title: T)
    where
        B: FnMut(PaneId, &mut ChildUi<'_>),
        T: FnMut(PaneId) -> String,
    {
        let mut inner = self.inner.borrow_mut();
        let frame = state.display_frame();
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
        inner.sync_overlay(ui, state.overlay());
        inner.synchronized_frame_epoch = Some(state.frame_epoch());
    }
}

#[cfg(test)]
mod tests;
