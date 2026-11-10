// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Keyed retained realization of docking frame elements.
//!
//! This module owns retained element identity and frame-to-element updates; it
//! explicitly does not own semantic tiling mutations or application content.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;

use overstory::{
    AppendSpec, Brush, ChildUi, CursorIcon, Edges, Element, ElementId, ElementSpecExt, Layout,
    LayoutPlacement, LayoutSize, Panel, SurfaceSpecExt, TabBar, TabKey, TabOption, TabOrientation,
    Text, Ui, Viewport, Visibility, Widget, replace_tab_options, select_tab_key,
    tab_bar_layout_snapshot, tab_bar_options,
};
use understory_tiling::{
    Axis, CollapsedTileFrame, DropTargetFrame, DropTargetId, GhostFrame, GhostKind, OverlayFrame,
    PaneFrame, PaneId, Rect, SplitHandleFrame, TabBarFrame, TabBarPlacement, TabFrame, TileId,
};

use super::events::DockTabEvents;
use super::style::{DockPaint, DockSurfaceStyle, apply_paint};

#[derive(Debug)]
pub(super) struct DockSurfaceInner {
    pub(super) root: ElementId,
    pub(super) pane_layer: ElementId,
    pub(super) chrome_layer: ElementId,
    pub(super) overlay_layer: ElementId,
    pub(super) panes: Vec<PaneView>,
    pub(super) tab_bars: Vec<TabBarView>,
    handles: Vec<HandleView>,
    collapsed: Vec<(TileId, ElementId)>,
    targets: Vec<TargetView>,
    ghosts: Vec<ElementId>,
    pub(super) dragged: ElementId,
    pub(super) style: DockSurfaceStyle,
    pub(super) synchronized_frame_epoch: Option<u64>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PaneView {
    pub(super) pane: PaneId,
    pub(super) body: ElementId,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct TabBarView {
    pub(super) group: TileId,
    pub(super) body: ElementId,
    pub(super) orientation: TabOrientation,
}

#[derive(Clone, Copy, Debug)]
struct HandleView {
    split: TileId,
    handle: usize,
    axis: Axis,
    body: ElementId,
}

#[derive(Clone, Copy, Debug)]
struct TargetView {
    target: DropTargetId,
    body: ElementId,
}

#[derive(Debug)]
pub(super) struct DockLayer {
    layout: Layout,
}

impl DockLayer {
    pub(super) fn new(layout: Layout) -> Self {
        Self { layout }
    }
}

impl AppendSpec for DockLayer {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget(parent, Box::new(DockLayerWidget));
        ui.set_layout(id, self.layout);
        id
    }
}

#[derive(Debug)]
struct DockLayerWidget;

impl Widget for DockLayerWidget {
    fn is_hit_test_visible(&self, _element: &Element) -> bool {
        false
    }
}

impl DockSurfaceInner {
    pub(super) fn new(
        root: ElementId,
        pane_layer: ElementId,
        chrome_layer: ElementId,
        overlay_layer: ElementId,
        dragged: ElementId,
        style: DockSurfaceStyle,
    ) -> Self {
        Self {
            root,
            pane_layer,
            chrome_layer,
            overlay_layer,
            panes: Vec::new(),
            tab_bars: Vec::new(),
            handles: Vec::new(),
            collapsed: Vec::new(),
            targets: Vec::new(),
            ghosts: Vec::new(),
            dragged,
            style,
            synchronized_frame_epoch: None,
        }
    }

    pub(super) fn sync_panes<B>(
        &mut self,
        ui: &mut Ui,
        panes: &[PaneId],
        frames: &[PaneFrame],
        selected: PaneId,
        build_pane: &mut B,
    ) where
        B: FnMut(PaneId, &mut ChildUi<'_>),
    {
        remove_missing(
            ui,
            &mut self.panes,
            |view| panes.contains(&view.pane),
            |view| view.body,
        );

        for pane in panes {
            let body = if let Some(view) = self.panes.iter().find(|view| view.pane == *pane) {
                view.body
            } else {
                let body = ui.add_with_children(
                    self.pane_layer,
                    Panel::column(6.0)
                        .layout(absolute_column_layout())
                        .padding(self.style.pane_padding)
                        .corner_radius(4.0),
                    |ui| build_pane(*pane, ui),
                );
                self.panes.push(PaneView { pane: *pane, body });
                body
            };
            if let Some(frame) = frames.iter().find(|frame| frame.pane == *pane) {
                set_rect(ui, body, frame.rect);
                apply_paint(
                    ui,
                    body,
                    if *pane == selected {
                        self.style.selected_pane
                    } else {
                        self.style.pane
                    },
                );
            } else {
                hide(ui, body);
            }
        }
    }

    pub(super) fn sync_tab_groups<T>(
        &mut self,
        ui: &mut Ui,
        bars: &[TabBarFrame],
        tabs: &[TabFrame],
        pane_title: &mut T,
        tab_events: &Rc<DockTabEvents>,
    ) where
        T: FnMut(PaneId) -> String,
    {
        remove_missing(
            ui,
            &mut self.tab_bars,
            |view| {
                bars.iter().any(|frame| {
                    frame.group == view.group
                        && tab_orientation(frame.placement) == view.orientation
                })
            },
            |view| view.body,
        );

        for frame in bars {
            let orientation = tab_orientation(frame.placement);
            let mut group_tabs: Vec<_> = tabs
                .iter()
                .filter(|tab| tab.group == frame.group)
                .copied()
                .collect();
            group_tabs.sort_by_key(|tab| tab.index);
            let options: Vec<_> = group_tabs
                .iter()
                .map(|tab| TabOption::new(tab_key(tab.pane), pane_title(tab.pane)))
                .collect();
            let selected = frame.active_pane.map(tab_key);
            let body = keyed_element(
                &mut self.tab_bars,
                |view| view.group == frame.group && view.orientation == orientation,
                |view| view.body,
                || {
                    let events = Rc::clone(tab_events);
                    let mut bar = TabBar::new()
                        .orientation(orientation)
                        .tabs(options.clone())
                        .on_selection_changed(move |ui, event| {
                            let Some(key) = event.selected_key else {
                                return;
                            };
                            let Ok(pane) = u32::try_from(key.value()) else {
                                return;
                            };
                            events.dispatch_activated(ui, PaneId(pane));
                        })
                        .layout(absolute_layout());
                    if let Some(selected) = selected {
                        bar = bar.selected_key(selected);
                    }
                    let body = ui.append(self.chrome_layer, bar);
                    TabBarView {
                        group: frame.group,
                        body,
                        orientation,
                    }
                },
            );
            if tab_bar_options(ui, body) != Some(options.as_slice()) {
                replace_tab_options(ui, body, options);
            }
            if let Some(selected) = selected {
                select_tab_key(ui, body, Some(selected));
            }
            set_rect(ui, body, frame.rect);
            apply_paint(ui, body, self.style.tab_bar);
            ui.set_box_padding(body, Edges::all(self.style.tab_inset));
            if let Some(snapshot) = tab_bar_layout_snapshot(ui, body) {
                for item in snapshot.items {
                    let Some(tab) = group_tabs.iter().find(|tab| tab_key(tab.pane) == item.key)
                    else {
                        continue;
                    };
                    apply_paint(
                        ui,
                        item.element,
                        if tab.active {
                            self.style.active_tab
                        } else {
                            self.style.tab
                        },
                    );
                    ui.set_local(
                        item.element,
                        ui.properties().foreground,
                        Brush::Solid(self.style.tab_foreground),
                    );
                    ui.set_local(item.element, ui.properties().font_size, 11.0);
                    ui.set_box_padding(
                        item.element,
                        Edges::vertical_horizontal(
                            self.style.tab_inset,
                            self.style.tab_inset + 2.0,
                        ),
                    );
                }
            }
        }
    }

    pub(super) fn sync_handles(&mut self, ui: &mut Ui, frames: &[SplitHandleFrame]) {
        remove_missing(
            ui,
            &mut self.handles,
            |view| {
                frames.iter().any(|frame| {
                    frame.split == view.split
                        && frame.handle == view.handle
                        && frame.axis == view.axis
                })
            },
            |view| view.body,
        );

        for frame in frames {
            let body = keyed_element(
                &mut self.handles,
                |view| {
                    view.split == frame.split
                        && view.handle == frame.handle
                        && view.axis == frame.axis
                },
                |view| view.body,
                || {
                    let body = ui.append(
                        self.chrome_layer,
                        Viewport::new()
                            .cursor_icon(match frame.axis {
                                Axis::Horizontal => CursorIcon::ColResize,
                                Axis::Vertical => CursorIcon::RowResize,
                            })
                            .layout(absolute_layout()),
                    );
                    HandleView {
                        split: frame.split,
                        handle: frame.handle,
                        axis: frame.axis,
                        body,
                    }
                },
            );
            set_rect_snap(ui, body, frame.rect);
            apply_paint(ui, body, self.style.split_handle);
        }
    }

    pub(super) fn sync_collapsed_tiles(&mut self, ui: &mut Ui, frames: &[CollapsedTileFrame]) {
        remove_missing(
            ui,
            &mut self.collapsed,
            |(tile, _)| frames.iter().any(|frame| frame.tile == *tile),
            |(_, body)| *body,
        );
        for frame in frames {
            let body = keyed_element(
                &mut self.collapsed,
                |(tile, _)| *tile == frame.tile,
                |(_, body)| *body,
                || {
                    let body = ui.append(
                        self.chrome_layer,
                        Panel::column(0.0).align_center().layout(
                            Layout::column(0.0)
                                .align_center()
                                .justify_center()
                                .absolute(),
                        ),
                    );
                    ui.append(body, Text::new("+").font_size(20.0));
                    (frame.tile, body)
                },
            );
            set_rect(ui, body, frame.rect);
            apply_paint(ui, body, self.style.split_handle);
        }
    }

    pub(super) fn sync_overlay(&mut self, ui: &mut Ui, overlay: &OverlayFrame) {
        remove_missing(
            ui,
            &mut self.targets,
            |view| {
                overlay
                    .drop_targets
                    .iter()
                    .any(|frame| frame.id == view.target)
            },
            |view| view.body,
        );

        for frame in &overlay.drop_targets {
            self.sync_target(ui, *frame, overlay);
        }

        while self.ghosts.len() > overlay.ghost_rects.len() {
            if let Some(ghost) = self.ghosts.pop() {
                ui.remove(ghost);
            }
        }
        while self.ghosts.len() < overlay.ghost_rects.len() {
            let ghost = ui.append(self.overlay_layer, Panel::new().layout(absolute_layout()));
            self.ghosts.push(ghost);
        }
        for (element, frame) in self.ghosts.iter().zip(&overlay.ghost_rects) {
            set_rect(ui, *element, frame.rect);
            apply_paint(ui, *element, self.ghost_paint(frame));
        }

        if let Some(dragged) = overlay.dragged {
            set_rect(ui, self.dragged, dragged.rect);
        } else {
            hide(ui, self.dragged);
        }
    }

    fn sync_target(&mut self, ui: &mut Ui, frame: DropTargetFrame, overlay: &OverlayFrame) {
        let body = keyed_element(
            &mut self.targets,
            |view| view.target == frame.id,
            |view| view.body,
            || {
                let body = ui.append(self.overlay_layer, Panel::new().layout(absolute_layout()));
                TargetView {
                    target: frame.id,
                    body,
                }
            },
        );
        set_rect(ui, body, frame.rect);
        let mut paint = if frame.accepts {
            self.style.accepted_target
        } else {
            self.style.rejected_target
        };
        if Some(frame.id) == overlay.active_target {
            paint.border = self.style.active_target_border;
            paint.border_width = 2.0;
        }
        apply_paint(ui, body, paint);
    }

    fn ghost_paint(&self, frame: &GhostFrame) -> DockPaint {
        match frame.kind {
            GhostKind::PreviewPane | GhostKind::PreviewGroup
                if frame.rect.width() <= 2.0 || frame.rect.height() <= 2.0 =>
            {
                DockPaint::new(self.style.preview.border, self.style.preview.border, 0.0)
            }
            GhostKind::PreviewPane | GhostKind::PreviewGroup => self.style.preview,
            GhostKind::Invalid => self.style.invalid_preview,
        }
    }
}

fn keyed_element<T>(
    views: &mut Vec<T>,
    matches: impl Fn(&T) -> bool,
    element: impl Fn(&T) -> ElementId,
    create: impl FnOnce() -> T,
) -> ElementId {
    if let Some(view) = views.iter().find(|view| matches(view)) {
        return element(view);
    }
    let view = create();
    let id = element(&view);
    views.push(view);
    id
}

fn remove_missing<T>(
    ui: &mut Ui,
    views: &mut Vec<T>,
    keep: impl Fn(&T) -> bool,
    element: impl Fn(&T) -> ElementId,
) {
    let mut index = views.len();
    while index > 0 {
        index -= 1;
        if !keep(&views[index]) {
            let view = views.swap_remove(index);
            ui.remove(element(&view));
        }
    }
}

fn tab_orientation(placement: TabBarPlacement) -> TabOrientation {
    match placement {
        TabBarPlacement::Left | TabBarPlacement::Right => TabOrientation::Vertical,
        TabBarPlacement::Top | TabBarPlacement::Bottom | TabBarPlacement::Hidden => {
            TabOrientation::Horizontal
        }
    }
}

const fn tab_key(pane: PaneId) -> TabKey {
    TabKey::new(pane.0 as u64)
}

fn set_rect(ui: &mut Ui, id: ElementId, rect: Rect) {
    ui.set_visibility(id, Visibility::Visible);
    ui.set_layout(
        id,
        Layout::new()
            .absolute_at(rect.x0, rect.y0)
            .logical_size(rect.width().max(0.0), rect.height().max(0.0)),
    );
}

fn set_rect_snap(ui: &mut Ui, id: ElementId, rect: Rect) {
    ui.set_visibility(id, Visibility::Visible);
    let layout_placement = ui.properties().layout_placement;
    let layout_size = ui.properties().layout_size;
    ui.set_local(
        id,
        layout_placement,
        LayoutPlacement::absolute_at(rect.x0, rect.y0),
    );
    ui.set_local(
        id,
        layout_size,
        LayoutSize::logical(rect.width().max(0.0), rect.height().max(0.0)),
    );
}

pub(super) fn hide(ui: &mut Ui, id: ElementId) {
    ui.set_visibility(id, Visibility::Collapsed);
}

pub(super) fn absolute_layout() -> Layout {
    Layout::new().absolute()
}

fn absolute_column_layout() -> Layout {
    Layout::column(6.0).absolute()
}
