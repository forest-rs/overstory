// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Docking and tiling example.
//!
//! The example renders and wires the application-facing `overstory_docking`
//! adapter. It leaves the semantic tiling model, layout solving, hit testing,
//! drop-target ranking, proposal validation, and layout mutation rules to
//! `understory_tiling` through that adapter.

use std::cell::RefCell;
use std::rc::Rc;

use overstory::{
    Button, ChildUi, ElementId, ElementSpecExt, Layout, LayoutDimension, LayoutSize, Panel,
    PointerEventKind, SurfaceSpecExt, Text, Ui, theme,
};
use overstory_app::WindowOptions;
#[cfg(test)]
use overstory_docking::tiling::{DockTarget, Placement, TileId};
use overstory_docking::{
    DockInteractionKind, DockOutcome, DockState, DockSurface,
    tiling::{
        Axis, FrameItemId, HitKind, LayoutFrame, LayoutInput, PaneBookmark, PaneId, Point, Rect,
        Size, TileError, TileOp, TileSpec, TileTree, hit_test,
    },
};
use peniko::{Brush, Color};

const INITIAL_WIDTH: u32 = 1360;
const INITIAL_HEIGHT: u32 = 860;
const CANVAS_WIDTH: f64 = 920.0;
const CANVAS_HEIGHT: f64 = 620.0;
const SIDE_WIDTH: f64 = 332.0;
fn main() -> overstory_app::HostResult<()> {
    overstory_app::run(
        WindowOptions::new("Dock").initial_size(INITIAL_WIDTH, INITIAL_HEIGHT),
        build_ui,
    )
}

#[derive(Debug)]
struct DockModel {
    dock: DockState,
    next_pane: u32,
    hit: Option<HitKind>,
    status: String,
    last_op: String,
    closed: Option<PaneBookmark>,
}

impl DockModel {
    fn new() -> Self {
        Self {
            dock: DockState::new(sample_tree(), layout_input(), PaneId(1)),
            next_pane: 6,
            hit: None,
            status: String::from("Ready"),
            last_op: String::from("seed sample layout"),
            closed: None,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
        self.status = String::from("Reset sample layout");
    }

    fn display_frame(&self) -> &LayoutFrame {
        self.dock.display_frame()
    }

    fn next_pane(&mut self) -> PaneId {
        let pane = PaneId(self.next_pane);
        self.next_pane = self.next_pane.saturating_add(1);
        pane
    }

    fn record_command_result(&mut self, label: &str, result: Result<TileOp, TileError>) {
        match result {
            Ok(_op) => {
                self.last_op = String::from(label);
                self.status = format!("{label} committed");
            }
            Err(error) => {
                self.status = format!("{label} rejected: {error}");
            }
        }
    }

    fn split_selected(&mut self, axis: Axis) {
        let new_pane = self.next_pane();
        let result = self.dock.split_selected(axis, new_pane, 0.38);
        self.record_command_result("split selected pane", result);
    }

    fn close_selected(&mut self) {
        match self.dock.close_selected_with_bookmark() {
            Ok(bookmark) => {
                self.last_op = format!("close pane {}", bookmark.pane().0);
                self.status = String::from("Pane closed. Reopen it after editing the layout.");
                self.closed = Some(bookmark);
            }
            Err(error) => self.status = format!("Close rejected: {error}"),
        }
    }

    fn reopen_closed(&mut self) {
        let Some(bookmark) = self.closed.as_ref() else {
            self.status = String::from("Close a pane to record its placement.");
            return;
        };
        match self.dock.reopen_pane(bookmark) {
            Ok(placement) => {
                self.last_op = format!("reopen pane {}", bookmark.pane().0);
                self.status = format!("Pane reopened ({placement:?} placement)");
                self.closed = None;
            }
            Err(error) => self.status = format!("Reopen rejected: {error}"),
        }
    }

    fn collapse_selected(&mut self) {
        let result = self.dock.collapse_selected(44.0);
        let collapsed = result.is_ok();
        self.record_command_result("collapse selected tile", result);
        if collapsed {
            self.status = String::from("Tile collapsed. Click its + strip to expand.");
        }
    }

    fn toggle_policy_lock(&mut self) {
        let locked = self.dock.toggle_policy_lock();
        self.status = if locked {
            String::from("Policy lock enabled: proposals validate as rejected")
        } else {
            String::from("Policy lock disabled")
        };
    }

    fn toggle_zoom_selected(&mut self) {
        self.status = match self.dock.toggle_zoom_selected() {
            Some(pane) => format!("Zoomed pane {}", pane.0),
            None => String::from("Restored tiled layout"),
        };
    }

    fn activate_pane(&mut self, pane: PaneId) {
        let result = self.dock.activate_pane(pane);
        self.record_command_result("activate tab", result);
    }

    fn begin_pointer(&mut self, point: Point) {
        self.hit = hit_test(self.dock.frame(), point);
        let outcome = self.dock.begin_pointer(point);
        self.record_outcome(outcome);
    }

    fn update_pointer(&mut self, point: Point) {
        self.hit = hit_test(self.dock.frame(), point);
        let outcome = self.dock.update_pointer(point);
        self.record_outcome(outcome);
    }

    fn end_pointer(&mut self, point: Point) {
        let outcome = self.dock.end_pointer(point);
        self.record_outcome(outcome);
    }

    fn cancel_pointer(&mut self) {
        let outcome = self.dock.cancel_pointer();
        self.record_outcome(outcome);
    }

    fn record_outcome(&mut self, outcome: DockOutcome) {
        match outcome {
            DockOutcome::None => {}
            DockOutcome::NoHit => self.status = String::from("No tiling hit at pointer down"),
            DockOutcome::InteractionStarted(kind) => {
                self.status = interaction_started_label(kind);
            }
            DockOutcome::NoProposal(kind) => {
                self.status = no_proposal_label(kind);
            }
            DockOutcome::ProposalValid(op) => {
                self.status = format!("valid proposal: {}", op_label(&op));
            }
            DockOutcome::ProposalRejected(error) => {
                self.status = format!("proposal rejected: {error}");
            }
            DockOutcome::PaneSelected(pane) => {
                self.status = format!("Selected pane {}", pane.0);
            }
            DockOutcome::ClickMiss(hit) => {
                self.hit = hit;
                self.status = String::from("Click did not hit an activatable tile");
            }
            DockOutcome::Committed(op) => {
                self.last_op = op_label(&op);
                self.status = String::from("Validated interaction proposal committed");
            }
            DockOutcome::Rejected(error) => {
                self.status = format!("validated interaction rejected: {error}");
            }
            DockOutcome::Canceled => {
                self.status = String::from("Pointer interaction canceled");
            }
        }
    }
}

#[derive(Clone, Debug)]
struct DockView {
    surface: DockSurface,
    status: ElementId,
    hit: ElementId,
    frame: ElementId,
    operation: ElementId,
    split_h: ElementId,
    split_v: ElementId,
    close: ElementId,
    reopen: ElementId,
    collapse: ElementId,
    zoom: ElementId,
    reset: ElementId,
    policy: ElementId,
}

fn build_ui() -> Ui {
    let mut ui = Ui::new();
    ui.set_theme(theme::dark_theme());
    let state = Rc::new(RefCell::new(DockModel::new()));

    let root = ui.root();
    ui.set_layout(root, Layout::column(0.0).fill_size());
    ui.set_local(
        root,
        ui.properties().surface.background,
        Some(brush(color(0x0d, 0x11, 0x16, 0xff))),
    );

    let view = build_shell(&mut ui, root);
    render(&mut ui, &view, &state.borrow());
    wire(&mut ui, view, state);
    ui
}

fn build_shell(ui: &mut Ui, root: ElementId) -> DockView {
    ui.with_child(
        root,
        Panel::column(18.0)
            .layout(Layout::column(18.0).fill_size())
            .padding(20.0)
            .background(Some(brush(color(0x0d, 0x11, 0x16, 0xff)))),
        |shell| {
            shell.add(
                Text::new("Dock")
                    .font_size(34.0)
                    .foreground(brush(color(0xf4, 0xf7, 0xfb, 0xff)))
                    .layout(Layout::new().flex_shrink(0.0)),
            );
            shell.add(
                Text::new("A vertical slice over understory_tiling: frames, hits, proposals, validation, and commits.")
                    .font_size(14.0)
                    .foreground(brush(color(0xb6, 0xc0, 0xcb, 0xff)))
                    .layout(Layout::new().flex_shrink(0.0)),
            );

            let (workbench, side) = shell.with_child(
                Panel::row(18.0)
                    .layout(Layout::row(18.0).fill_size())
                    .background(Some(brush(color(0x12, 0x18, 0x20, 0xff))))
                    .border_brush(Some(brush(color(0x2a, 0x35, 0x43, 0xff))))
                    .border_width(1.0)
                    .corner_radius(8.0)
                    .padding(14.0),
                |row| {
                    let workbench = build_workbench(row);
                    let side = build_side(row);
                    (workbench, side)
                },
            );

            DockView {
                surface: workbench,
                status: side.status,
                hit: side.hit,
                frame: side.frame,
                operation: side.operation,
                split_h: side.split_h,
                split_v: side.split_v,
                close: side.close,
                reopen: side.reopen,
                collapse: side.collapse,
                zoom: side.zoom,
                reset: side.reset,
                policy: side.policy,
            }
        },
    )
}

fn build_workbench(row: &mut ChildUi<'_>) -> DockSurface {
    DockSurface::mount(
        row,
        Layout::new()
            .logical_size(CANVAS_WIDTH, CANVAS_HEIGHT)
            .flex_shrink(0.0),
    )
}

#[derive(Debug)]
struct SideView {
    status: ElementId,
    hit: ElementId,
    frame: ElementId,
    operation: ElementId,
    split_h: ElementId,
    split_v: ElementId,
    close: ElementId,
    reopen: ElementId,
    collapse: ElementId,
    zoom: ElementId,
    reset: ElementId,
    policy: ElementId,
}

fn build_side(row: &mut ChildUi<'_>) -> SideView {
    row.with_child(
        Panel::column(12.0)
            .layout(
                Layout::column(12.0)
                    .size(LayoutSize::new().width(LayoutDimension::Logical(SIDE_WIDTH)))
                    .flex_shrink(0.0),
            )
            .background(Some(brush(color(0x15, 0x1c, 0x25, 0xff))))
            .border_brush(Some(brush(color(0x2d, 0x3b, 0x4b, 0xff))))
            .border_width(1.0)
            .corner_radius(6.0)
            .padding(12.0),
        |side| {
            side.add(
                Text::new("Controls")
                    .font_size(18.0)
                    .foreground(brush(color(0xf3, 0xf7, 0xfa, 0xff)))
                    .layout(Layout::new().flex_shrink(0.0)),
            );
            let split_h = side.add(Button::new("Split H").font_size(12.0));
            let split_v = side.add(Button::new("Split V").font_size(12.0));
            let close = side.add(Button::new("Close selected").font_size(12.0));
            let reopen = side.add(Button::new("Reopen last closed pane").font_size(12.0));
            let collapse = side.add(Button::new("Collapse selected tile").font_size(12.0));
            let zoom = side.add(Button::new("Zoom / restore selected").font_size(12.0));
            let policy = side.add(Button::new("Toggle policy lock").font_size(12.0));
            let reset = side.add(Button::new("Reset").font_size(12.0));
            let status = readout(side, "Status");
            let hit = readout(side, "Hit");
            let frame = readout(side, "Frame");
            let operation = readout(side, "Last op");
            SideView {
                status,
                hit,
                frame,
                operation,
                split_h,
                split_v,
                close,
                reopen,
                collapse,
                zoom,
                reset,
                policy,
            }
        },
    )
}

fn readout(side: &mut ChildUi<'_>, label: &str) -> ElementId {
    side.with_child(
        Panel::column(4.0)
            .layout(Layout::column(4.0).fill_width().flex_shrink(0.0))
            .background(Some(brush(color(0x10, 0x16, 0x1e, 0xff))))
            .border_brush(Some(brush(color(0x2a, 0x36, 0x45, 0xff))))
            .border_width(1.0)
            .corner_radius(4.0)
            .padding(8.0),
        |card| {
            card.add(
                Text::new(label)
                    .font_size(11.0)
                    .foreground(brush(color(0x7f, 0x92, 0xa7, 0xff)))
                    .layout(Layout::new().flex_shrink(0.0)),
            );
            card.add(
                Text::new("")
                    .font_size(12.0)
                    .wrap()
                    .foreground(brush(color(0xe7, 0xed, 0xf5, 0xff)))
                    .layout(
                        Layout::new()
                            .size(
                                LayoutSize::new()
                                    .width(LayoutDimension::Logical(SIDE_WIDTH - 48.0)),
                            )
                            .flex_shrink(0.0),
                    ),
            )
        },
    )
}

fn wire(ui: &mut Ui, view: DockView, state: Rc<RefCell<DockModel>>) {
    let tab_state = Rc::clone(&state);
    let tab_view = view.clone();
    view.surface.set_tab_activated_handler(move |ui, pane| {
        tab_state.borrow_mut().activate_pane(pane);
        render(ui, &tab_view, &tab_state.borrow());
    });

    let split_h_state = Rc::clone(&state);
    let split_h_view = view.clone();
    ui.events(view.split_h).click(move |ui, event| {
        split_h_state.borrow_mut().split_selected(Axis::Horizontal);
        render(ui, &split_h_view, &split_h_state.borrow());
        event.mark_handled();
    });

    let split_v_state = Rc::clone(&state);
    let split_v_view = view.clone();
    ui.events(view.split_v).click(move |ui, event| {
        split_v_state.borrow_mut().split_selected(Axis::Vertical);
        render(ui, &split_v_view, &split_v_state.borrow());
        event.mark_handled();
    });

    let close_state = Rc::clone(&state);
    let close_view = view.clone();
    ui.events(view.close).click(move |ui, event| {
        close_state.borrow_mut().close_selected();
        render(ui, &close_view, &close_state.borrow());
        event.mark_handled();
    });

    let reopen_state = Rc::clone(&state);
    let reopen_view = view.clone();
    ui.events(view.reopen).click(move |ui, event| {
        reopen_state.borrow_mut().reopen_closed();
        render(ui, &reopen_view, &reopen_state.borrow());
        event.mark_handled();
    });

    let collapse_state = Rc::clone(&state);
    let collapse_view = view.clone();
    ui.events(view.collapse).click(move |ui, event| {
        collapse_state.borrow_mut().collapse_selected();
        render(ui, &collapse_view, &collapse_state.borrow());
        event.mark_handled();
    });

    let zoom_state = Rc::clone(&state);
    let zoom_view = view.clone();
    ui.events(view.zoom).click(move |ui, event| {
        zoom_state.borrow_mut().toggle_zoom_selected();
        render(ui, &zoom_view, &zoom_state.borrow());
        event.mark_handled();
    });

    let policy_state = Rc::clone(&state);
    let policy_view = view.clone();
    ui.events(view.policy).click(move |ui, event| {
        policy_state.borrow_mut().toggle_policy_lock();
        render(ui, &policy_view, &policy_state.borrow());
        event.mark_handled();
    });

    let reset_state = Rc::clone(&state);
    let reset_view = view.clone();
    ui.events(view.reset).click(move |ui, event| {
        reset_state.borrow_mut().reset();
        render(ui, &reset_view, &reset_state.borrow());
        event.mark_handled();
    });

    let down_state = Rc::clone(&state);
    let down_view = view.clone();
    ui.events(view.surface.root())
        .pointer(PointerEventKind::Down, move |ui, event| {
            let Some(position) = event.position else {
                return;
            };
            let point = down_view.surface.local_point(ui, position);
            let mut model = down_state.borrow_mut();
            if let Err(error) = down_view.surface.apply_tab_geometry(ui, &mut model.dock) {
                model.status = format!("tab geometry rejected: {error}");
                return;
            }
            model.begin_pointer(point);
            drop(model);
            let _ = ui.capture_pointer(down_view.surface.root());
            render(ui, &down_view, &down_state.borrow());
            event.mark_handled();
        });

    let move_state = Rc::clone(&state);
    let move_view = view.clone();
    ui.events(view.surface.root())
        .pointer(PointerEventKind::Move, move |ui, event| {
            let Some(position) = event.position else {
                return;
            };
            let point = move_view.surface.local_point(ui, position);
            move_state.borrow_mut().update_pointer(point);
            render(ui, &move_view, &move_state.borrow());
            event.mark_handled();
        });

    let up_state = Rc::clone(&state);
    let up_view = view.clone();
    ui.events(view.surface.root())
        .pointer(PointerEventKind::Up, move |ui, event| {
            let Some(position) = event.position else {
                return;
            };
            let point = up_view.surface.local_point(ui, position);
            up_state.borrow_mut().end_pointer(point);
            let _ = ui.release_pointer_capture(up_view.surface.root());
            render(ui, &up_view, &up_state.borrow());
            event.mark_handled();
        });

    let cancel_state = Rc::clone(&state);
    let cancel_view = view.clone();
    ui.events(view.surface.root())
        .pointer(PointerEventKind::Cancel, move |ui, event| {
            cancel_state.borrow_mut().cancel_pointer();
            let _ = ui.release_pointer_capture(cancel_view.surface.root());
            render(ui, &cancel_view, &cancel_state.borrow());
            event.mark_handled();
        });
}

fn render(ui: &mut Ui, view: &DockView, model: &DockModel) {
    let frame = model.display_frame();
    view.surface
        .sync(ui, &model.dock, build_pane_content, pane_title);
    set_text(ui, view.status, model.status.clone());
    set_text(ui, view.hit, hit_summary(model.hit));
    set_text(
        ui,
        view.frame,
        frame_summary(frame, model.dock.is_policy_locked()),
    );
    set_text(ui, view.operation, model.last_op.clone());
}

fn build_pane_content(pane: PaneId, ui: &mut ChildUi<'_>) {
    ui.add(
        Text::new(pane_title(pane))
            .font_size(15.0)
            .foreground(brush(color(0xf2, 0xf6, 0xfa, 0xff)))
            .layout(Layout::new().flex_shrink(0.0)),
    );
    ui.add(
        Text::new(format!("Application content for pane {}", pane.0))
            .font_size(12.0)
            .foreground(brush(color(0x96, 0xa6, 0xb7, 0xff)))
            .layout(Layout::new().flex_shrink(0.0)),
    );
}

fn sample_tree() -> TileTree {
    TileTree::from_spec(TileSpec::split(
        Axis::Horizontal,
        [
            TileSpec::tabs([PaneId(1), PaneId(2)]),
            TileSpec::split(
                Axis::Vertical,
                [
                    TileSpec::tabs([PaneId(3), PaneId(4)]),
                    TileSpec::pane(PaneId(5)),
                ],
                [0.54, 0.46],
            ),
        ],
        [0.58, 0.42],
    ))
    .expect("sample docking layout should be structurally valid")
}

fn layout_input() -> LayoutInput {
    LayoutInput {
        bounds: Rect::new(0.0, 0.0, CANVAS_WIDTH, CANVAS_HEIGHT),
        tab_bar_thickness: 32.0,
        split_handle_thickness: 7.0,
        min_pane_size: Size::new(96.0, 84.0),
    }
}

#[cfg(test)]
fn axis_position(point: Point, axis: Axis) -> f64 {
    match axis {
        Axis::Horizontal => point.x,
        Axis::Vertical => point.y,
    }
}

fn interaction_started_label(kind: DockInteractionKind) -> String {
    match kind {
        DockInteractionKind::PendingDrag => String::from("Pointer gesture pending click or drag"),
        DockInteractionKind::Drag => String::from("Drag session started"),
        DockInteractionKind::Resize => String::from("Resize session started"),
    }
}

fn no_proposal_label(kind: DockInteractionKind) -> String {
    match kind {
        DockInteractionKind::PendingDrag => String::from("Pointer gesture pending click or drag"),
        DockInteractionKind::Drag => String::from("Drag session has no accepting proposal"),
        DockInteractionKind::Resize => String::from("Resize session has no proposal"),
    }
}

fn pane_title(pane: PaneId) -> String {
    match pane.0 {
        1 => String::from("Editor.rs"),
        2 => String::from("Preview"),
        3 => String::from("Outline"),
        4 => String::from("Inspector"),
        5 => String::from("Search"),
        id => format!("Pane {id}"),
    }
}

fn hit_summary(hit: Option<HitKind>) -> String {
    match hit {
        Some(HitKind::Pane { pane }) => format!("pane {}", pane.0),
        Some(HitKind::Tab { group, pane }) => format!("tab pane {} in group {}", pane.0, group.0),
        Some(HitKind::TabBar { group }) => format!("tab bar group {}", group.0),
        Some(HitKind::SplitHandle { split, handle }) => {
            format!("split {} handle {}", split.0, handle)
        }
        Some(HitKind::CollapsedTile { tile }) => format!("collapsed tile {}", tile.0),
        Some(HitKind::Empty) => String::from("empty"),
        None => String::from("none"),
    }
}

fn frame_summary(frame: &LayoutFrame, policy_locked: bool) -> String {
    format!(
        "revision {}\npanes {} tabs {} bars {} handles {}\nfocus {}\npaint {}\nprojection {}\npolicy {}",
        frame.revision().0,
        frame.panes().len(),
        frame.tabs().len(),
        frame.tab_bars().len(),
        frame.split_handles().len(),
        pane_list(frame.focus_order()),
        paint_list(frame.paint_order()),
        frame.projection().map_or_else(
            || String::from("none"),
            |projection| { format!("zoom pane {}", projection.focus.0) }
        ),
        if policy_locked { "locked" } else { "open" }
    )
}

fn pane_list(panes: &[PaneId]) -> String {
    panes
        .iter()
        .map(|pane| pane.0.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn paint_list(items: &[FrameItemId]) -> String {
    items
        .iter()
        .take(8)
        .map(|item| match item {
            FrameItemId::Pane(pane) => format!("P{}", pane.0),
            FrameItemId::CollapsedTile(tile) => format!("R{}", tile.0),
            FrameItemId::Tab { pane, .. } => format!("T{}", pane.0),
            FrameItemId::TabBar(group) => format!("B{}", group.0),
            FrameItemId::SplitChild { split, child } => format!("C{}:{}", split.0, child.0),
            FrameItemId::SplitHandle { split, handle } => format!("H{}:{}", split.0, handle),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn op_label(op: &TileOp) -> String {
    match op {
        TileOp::ActivatePane { pane } => format!("activate pane {}", pane.0),
        TileOp::ClosePane { pane } => format!("close pane {}", pane.0),
        TileOp::SplitPane {
            pane,
            axis,
            new_pane,
            ..
        } => format!("split pane {} {:?} -> {}", pane.0, axis, new_pane.0),
        TileOp::MovePane { pane, target } => format!("move pane {} to {:?}", pane.0, target),
        TileOp::MoveTabGroup { group, target } => {
            format!("move tab group {} to {:?}", group.0, target)
        }
        TileOp::ReorderTab { group, pane, index } => {
            format!("reorder pane {} in group {} to {}", pane.0, group.0, index)
        }
        TileOp::SetCollapsed { tile, extent } => {
            format!("set tile {} collapsed extent {:?}", tile.0, extent)
        }
        TileOp::SetSplitShares { split, shares } => {
            format!("set split {} shares {:?}", split.0, shares)
        }
    }
}

fn set_text(ui: &mut Ui, id: ElementId, text: String) {
    ui.set_local(id, ui.properties().text, text);
}

fn brush(color: Color) -> Brush {
    Brush::Solid(color)
}

fn color(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_rgba8(r, g, b, a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use overstory_docking::tiling::{DockProposal, InteractionState, SplitHandleFrame};

    #[test]
    fn arranged_demo_geometry_allows_resize_and_tab_click() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).fill_size());
        let view = build_shell(&mut ui, root);
        let mut model = DockModel::new();
        render(&mut ui, &view, &model);
        ui.compute_layout(Size::new(
            f64::from(INITIAL_WIDTH),
            f64::from(INITIAL_HEIGHT),
        ));

        let geometry = view.surface.tab_group_geometry(&ui, &model.dock).unwrap();
        let result = view.surface.apply_tab_geometry(&ui, &mut model.dock);
        assert!(
            result.is_ok(),
            "{result:?}\ngeometry={geometry:#?}\nbars={:#?}",
            model.dock.frame().tab_bars()
        );

        let handle = model.dock.frame().split_handles()[0];
        model.begin_pointer(handle.rect.center());
        assert_eq!(model.status, "Resize session started");
        model.cancel_pointer();

        let pane_two = geometry
            .iter()
            .flat_map(|group| &group.tabs)
            .find(|tab| tab.pane == PaneId(2))
            .unwrap();
        let point = pane_two.interaction_regions[0].center();
        model.begin_pointer(point);
        model.end_pointer(point);
        assert_eq!(model.dock.selected(), PaneId(2));
    }

    #[test]
    fn clicking_tab_activates_without_committing_drag() {
        let mut model = DockModel::new();
        let tab = model
            .dock
            .frame()
            .tabs()
            .iter()
            .find(|tab| tab.pane == PaneId(2))
            .copied()
            .expect("sample layout should expose pane 2 as a tab");
        let revision = model.dock.tree().revision();
        let point = tab.rect.center();

        model.begin_pointer(point);
        model.end_pointer(point);

        assert_eq!(model.dock.tree().revision(), revision.saturating_next());
        assert_eq!(model.dock.selected(), PaneId(2));
        assert!(
            model
                .dock
                .frame()
                .panes()
                .iter()
                .any(|pane| pane.pane == PaneId(2)),
            "activating the tab should make pane 2 the visible body",
        );
    }

    #[test]
    fn resize_drag_updates_preview_before_commit() {
        let mut model = DockModel::new();
        let handle = model
            .dock
            .frame()
            .split_handles()
            .first()
            .copied()
            .expect("sample layout should include a split handle");
        let start = handle.rect.center();
        let end = match handle.axis {
            Axis::Horizontal => Point::new(start.x + 60.0, start.y),
            Axis::Vertical => Point::new(start.x, start.y + 60.0),
        };
        let committed = model.dock.frame().clone();
        let revision = model.dock.tree().revision();

        model.begin_pointer(start);
        model.update_pointer(end);

        let preview = model
            .dock
            .preview_frame()
            .expect("resize move should populate a preview frame");
        assert_eq!(model.dock.tree().revision(), revision);
        assert_ne!(
            pane_rects(&committed),
            pane_rects(preview),
            "preview geometry should change before the committed tree changes",
        );

        model.end_pointer(end);

        assert!(model.dock.preview_frame().is_none());
        assert_ne!(model.dock.tree().revision(), revision);
        assert_eq!(
            pane_rects(model.dock.frame()),
            pane_rects(model.display_frame())
        );
    }

    #[test]
    fn resize_preview_handle_tracks_pointer_axis() {
        let mut model = DockModel::new();
        let handle = model
            .dock
            .frame()
            .split_handles()
            .first()
            .copied()
            .expect("sample layout should include a split handle");
        let start = handle.rect.center();
        let end = match handle.axis {
            Axis::Horizontal => Point::new(start.x + 60.0, start.y),
            Axis::Vertical => Point::new(start.x, start.y + 60.0),
        };

        model.begin_pointer(start);
        model.update_pointer(end);

        let preview = model
            .dock
            .preview_frame()
            .expect("resize move should populate a preview frame");
        let preview_handle = split_handle(preview, handle.split, handle.handle)
            .expect("preview should retain the dragged split handle");
        assert_close(
            axis_position(preview_handle.rect.center(), handle.axis),
            axis_position(end, handle.axis),
        );
    }

    #[test]
    fn local_top_edge_targets_the_pane_below_its_tab_bar() {
        let mut model = DockModel::new();
        let source = model
            .dock
            .frame()
            .panes()
            .first()
            .copied()
            .expect("sample layout should include panes");
        let target = model
            .dock
            .frame()
            .panes()
            .iter()
            .copied()
            .find(|pane| pane.pane != source.pane && pane.rect.height() > 120.0)
            .expect("sample layout should include a tall target pane");
        let start = source.rect.center();
        let hover = Point::new(
            target.rect.center().x,
            // The edge is measured from the full tile, including its tab bar.
            // Stay below the ancestor container's outer docking strip.
            target.rect.y0 + 24.0,
        );

        model.begin_pointer(start);
        model.update_pointer(hover);

        let InteractionState::Drag(drag) = model.dock.interaction() else {
            panic!("pane press should start a drag session");
        };
        let Some(DockProposal::MovePane {
            target:
                DockTarget::Split {
                    tile,
                    axis,
                    placement,
                    ..
                },
            ..
        }) = &drag.proposal
        else {
            panic!("pane-edge hover should produce a split move proposal");
        };
        assert_eq!(*tile, target.tile);
        assert_eq!(*axis, Axis::Vertical);
        assert_eq!(*placement, Placement::Before);
    }

    #[test]
    fn left_workspace_edge_previews_the_committed_split() {
        let mut model = DockModel::new();
        let source = model
            .dock
            .frame()
            .panes()
            .iter()
            .find(|pane| pane.pane == PaneId(5))
            .unwrap()
            .rect
            .center();
        let left = Point::new(2.0, CANVAS_HEIGHT / 2.0);
        let root = model.dock.tree().root();

        model.begin_pointer(source);
        model.update_pointer(left);
        assert!(matches!(
            model.dock.overlay().active_target,
            Some(overstory_docking::tiling::DropTargetId::Split {
                tile, axis: Axis::Horizontal, placement: Placement::Before,
            }) if tile == root
        ));
        let ghost = model.dock.overlay().ghost_rects[0].rect;
        assert_eq!(model.display_frame().panes()[0].rect, ghost);

        model.end_pointer(left);
        assert_eq!(model.display_frame().panes()[0].pane, PaneId(5));
        assert_eq!(model.display_frame().panes()[0].rect, ghost);
    }

    #[test]
    fn overlay_renders_only_the_chosen_drop_target() {
        let mut model = DockModel::new();
        let source = model.dock.frame().panes()[0];
        let target = model.dock.frame().panes()[1];

        model.begin_pointer(source.rect.center());
        model.update_pointer(target.rect.center());

        assert_eq!(
            model.dock.overlay().drop_targets.len(),
            1,
            "core should expose a focused overlay for normal rendering",
        );
    }

    #[test]
    fn zoom_toggle_projects_and_restores_selected_pane() {
        let mut model = DockModel::new();
        let revision = model.dock.tree().revision();
        model.dock.set_selected(PaneId(3));

        model.toggle_zoom_selected();

        assert_eq!(model.dock.zoomed_pane(), Some(PaneId(3)));
        assert_eq!(model.display_frame().panes().len(), 1);
        assert_eq!(model.dock.tree().revision(), revision);

        model.toggle_zoom_selected();

        assert_eq!(model.dock.zoomed_pane(), None);
        assert!(model.display_frame().panes().len() > 1);
        assert_eq!(model.dock.tree().revision(), revision);
    }

    fn pane_rects(frame: &LayoutFrame) -> Vec<(PaneId, Rect)> {
        frame
            .panes()
            .iter()
            .map(|pane| (pane.pane, pane.rect))
            .collect()
    }

    fn split_handle(frame: &LayoutFrame, split: TileId, handle: usize) -> Option<SplitHandleFrame> {
        frame
            .split_handles()
            .iter()
            .copied()
            .find(|candidate| candidate.split == split && candidate.handle == handle)
    }

    fn assert_close(left: f64, right: f64) {
        assert!(
            (left - right).abs() <= 0.5,
            "expected {left:.3} to be within 0.5px of {right:.3}",
        );
    }

    trait RevisionExt {
        fn saturating_next(self) -> Self;
    }

    impl RevisionExt for overstory_docking::tiling::Revision {
        fn saturating_next(self) -> Self {
            Self(self.0.saturating_add(1))
        }
    }
}
