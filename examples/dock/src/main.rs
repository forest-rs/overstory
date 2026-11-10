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
    Button, ChildUi, ElementId, ElementSpecExt, Layout, Panel, PointerEventKind, SurfaceSpecExt,
    Text, Ui, theme,
};
use overstory_app::WindowOptions;
#[cfg(test)]
use overstory_docking::tiling::{Placement, TileId};
use overstory_docking::{
    DockGeometryError, DockInteractionKind, DockOutcome, DockPaint, DockState, DockSurface,
    DockSurfaceStyle,
    tiling::{
        Axis, DockTarget, InteractionState, LayoutInput, PaneBookmark, PaneId, Point, Rect, Size,
        TileError, TileOp, TileSpec, TileTree,
    },
};
use peniko::{Brush, Color};

const INITIAL_WIDTH: u32 = 1112;
const INITIAL_HEIGHT: u32 = 710;
const CANVAS_WIDTH: f64 = 1064.0;
const CANVAS_HEIGHT: f64 = 484.0;
fn main() -> overstory_app::HostResult<()> {
    overstory_app::run(
        WindowOptions::new("Overstory | Dock workbench")
            .initial_size(INITIAL_WIDTH, INITIAL_HEIGHT),
        build_ui,
    )
}

#[derive(Debug)]
struct DockModel {
    dock: DockState,
    next_pane: u32,
    status: String,
    closed: Option<PaneBookmark>,
}

impl DockModel {
    fn new() -> Self {
        Self {
            dock: DockState::new(sample_tree(), layout_input(), PaneId(1)),
            next_pane: 6,
            status: String::from("Ready. Try dragging Preview onto the center of Search."),
            closed: None,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
        self.status = String::from("Reset sample layout");
    }

    #[cfg(test)]
    fn display_frame(&self) -> &overstory_docking::tiling::LayoutFrame {
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
        let outcome = self.dock.begin_pointer(point);
        self.record_outcome(outcome);
    }

    fn update_pointer(&mut self, point: Point) {
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
        self.status = match outcome {
            DockOutcome::None
            | DockOutcome::NoHit
            | DockOutcome::ClickMiss(_)
            | DockOutcome::InteractionStarted(DockInteractionKind::PendingDrag)
            | DockOutcome::NoProposal(DockInteractionKind::PendingDrag) => return,
            DockOutcome::InteractionStarted(DockInteractionKind::Resize)
            | DockOutcome::NoProposal(DockInteractionKind::Resize) => "Resizing split".into(),
            DockOutcome::InteractionStarted(DockInteractionKind::Drag)
            | DockOutcome::NoProposal(DockInteractionKind::Drag) => {
                "Move to a pane center to make tabs, or to an edge to split.".into()
            }
            DockOutcome::ProposalValid(op) => placement_label(&op, false),
            DockOutcome::PaneSelected(pane) => format!("Selected {}.", pane_title(pane)),
            DockOutcome::Committed(op) => placement_label(&op, true),
            DockOutcome::ProposalRejected(error) | DockOutcome::Rejected(error) => {
                if error == TileError::PolicyRejected {
                    "Docking is locked.".into()
                } else {
                    format!("Can't place the pane here ({error}).")
                }
            }
            DockOutcome::Canceled => "Drag canceled.".into(),
        };
    }
}

#[derive(Clone, Debug)]
struct DockView {
    surface: DockSurface,
    status: ElementId,
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
        Panel::column(12.0)
            .layout(Layout::column(12.0).fill_size())
            .padding(24.0)
            .background(Some(brush(color(0x10, 0x16, 0x20, 0xff)))),
        |shell| {
            shell.add(Text::new("Dock workbench").font_size(30.0)
                .foreground(brush(color(0xf4, 0xf7, 0xfb, 0xff))));
            shell.add(Text::new("Drag to a pane center to make tabs. Drag to an edge to split. Drag along a tab strip to insert.")
                .font_size(14.0).foreground(brush(color(0x9d, 0xb2, 0xca, 0xff))));
            let [split_h, split_v, collapse, close, reopen, zoom, policy, reset] = shell.with_child(
                Panel::row(8.0).layout(Layout::row(8.0).fill_width()),
                |toolbar| {
                    ["Split across", "Split below", "Collapse", "Close", "Reopen", "Zoom / restore", "Toggle lock", "Reset"].map(|label| {
                        toolbar.add(Button::new(label).font_size(13.0)
                            .layout(Layout::new().logical_size(120.0, 34.0)))
                    })
                },
            );

            let surface = DockSurface::mount_with_style(
                shell,
                Layout::new().logical_size(CANVAS_WIDTH, CANVAS_HEIGHT).flex_shrink(0.0),
                workbench_style(),
            );
            let status = shell.add(Text::new("").font_size(14.0)
                .foreground(brush(color(0x62, 0xe6, 0xd3, 0xff)))
                .layout(Layout::new().flex_shrink(0.0)));
            DockView { surface, status, split_h, split_v, close, reopen, collapse, zoom, reset, policy }
        },
    )
}

fn workbench_style() -> DockSurfaceStyle {
    let mut style = DockSurfaceStyle::default();
    let accent = color(0x62, 0xe6, 0xd3, 0xff);
    let none = color(0, 0, 0, 0);
    let quiet = DockPaint::new(none, none, 0.0);
    let pane = color(0x19, 0x23, 0x32, 0xff);
    let border = color(0x34, 0x45, 0x5a, 0xff);
    style.pane = DockPaint::new(pane, border, 1.0);
    style.selected_pane = DockPaint::new(pane, border, 1.0);
    style.tab = DockPaint::new(color(0x10, 0x16, 0x20, 0xff), border, 1.0);
    style.active_tab = DockPaint::new(color(0x23, 0x37, 0x46, 0xff), accent, 1.0);
    style.split_handle = DockPaint::new(border, border, 0.0);
    style.preview = DockPaint::new(color(0x62, 0xe6, 0xd3, 0x32), accent, 2.0);
    style.accepted_target = quiet;
    style.rejected_target = quiet;
    style.active_target_border = none;
    style.dragged = quiet;
    style.pane_padding = 16.0;
    style
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
            let Some(position) = event.position.filter(|_| !event.phase().is_bubble()) else {
                return;
            };
            let mut model = down_state.borrow_mut();
            let mut geometry = down_view.surface.apply_tab_geometry(ui, &mut model.dock);
            if matches!(geometry, Err(DockGeometryError::PendingLayout)) {
                // A hover style can dirty layout in the same input batch as
                // pointer-down. Refresh the arranged strip before picking a tab.
                let viewport = ui.effective_bounds(ui.root()).size();
                let _ = ui.compute_layout(viewport);
                geometry = down_view.surface.apply_tab_geometry(ui, &mut model.dock);
            }
            if geometry.is_err() {
                model.status = String::from("The layout is updating. Try the drag again.");
                set_text(ui, down_view.status, model.status.clone());
                return;
            }
            let point = down_view.surface.local_point(ui, position);
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
            if ui.pointer_capture() != Some(move_view.surface.root()) || event.phase().is_bubble() {
                return;
            }
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
            if ui.pointer_capture() != Some(up_view.surface.root()) || event.phase().is_bubble() {
                return;
            }
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
            if ui.pointer_capture() != Some(cancel_view.surface.root()) || event.phase().is_bubble()
            {
                return;
            }
            cancel_state.borrow_mut().cancel_pointer();
            let _ = ui.release_pointer_capture(cancel_view.surface.root());
            render(ui, &cancel_view, &cancel_state.borrow());
            event.mark_handled();
        });
}

fn render(ui: &mut Ui, view: &DockView, model: &DockModel) {
    if matches!(model.dock.interaction(), InteractionState::Drag(_)) {
        // Draw prospective placement over the current panes. The actual
        // contents and hit geometry stay steady until the user releases.
        view.surface.sync_overlay(ui, model.dock.overlay());
    } else {
        view.surface
            .sync(ui, &model.dock, build_pane_content, pane_title);
    }
    set_text(ui, view.status, model.status.clone());
}

fn build_pane_content(pane: PaneId, ui: &mut ChildUi<'_>) {
    ui.add(
        Text::new(pane_title(pane))
            .font_size(20.0)
            .foreground(brush(color(0xf2, 0xf6, 0xfa, 0xff)))
            .layout(Layout::new().flex_shrink(0.0)),
    );
    let description = match pane.0 {
        1 => "Workspace document",
        2 => "Rendered output",
        3 => "Document structure",
        4 => "Selection properties",
        5 => "Drag a pane here to make tabs",
        _ => "Workspace pane",
    };
    ui.add(
        Text::new(description)
            .font_size(13.0)
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

fn placement_label(op: &TileOp, committed: bool) -> String {
    let text = match (op, committed) {
        (
            TileOp::MovePane {
                target: DockTarget::TabWith { .. },
                ..
            },
            false,
        ) => "Release to create a tab group.",
        (
            TileOp::MovePane {
                target: DockTarget::TabWith { .. },
                ..
            },
            true,
        ) => "Created a tab group.",
        (
            TileOp::MovePane {
                target: DockTarget::TabInto { index: Some(_), .. },
                ..
            }
            | TileOp::ReorderTab { .. },
            false,
        ) => "Release to insert at the marker.",
        (
            TileOp::MovePane {
                target: DockTarget::TabInto { .. },
                ..
            },
            false,
        ) => "Release to add a tab.",
        (
            TileOp::MovePane {
                target: DockTarget::TabInto { .. },
                ..
            }
            | TileOp::ReorderTab { .. },
            true,
        ) => "Tab placed.",
        (TileOp::SetSplitShares { .. }, false) => "Resizing split",
        (TileOp::SetSplitShares { .. }, true) => "Split resized.",
        (TileOp::SetCollapsed { extent: None, .. }, true) => "Tile expanded.",
        (TileOp::ActivatePane { pane }, true) => return format!("Selected {}.", pane_title(*pane)),
        (_, false) => "Release to dock in the highlighted area.",
        (_, true) => "Pane docked.",
    };
    String::from(text)
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
    use overstory_docking::tiling::{DockProposal, LayoutFrame, SplitHandleFrame};

    #[test]
    fn routed_tab_drag_creates_a_group_and_retains_layout_during_hover() {
        use ui_events::pointer::{
            PointerButton, PointerButtonEvent, PointerButtons, PointerEvent, PointerId,
            PointerInfo, PointerState, PointerType, PointerUpdate,
        };

        fn pointer_state(point: Point, time: u64, pressed: bool) -> PointerState {
            let mut buttons = PointerButtons::new();
            if pressed {
                buttons.insert(PointerButton::Primary);
            }
            let mut state = PointerState {
                time,
                buttons,
                count: 1,
                ..PointerState::default()
            };
            state.position.x = point.x;
            state.position.y = point.y;
            state
        }

        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).fill_size());
        let view = build_shell(&mut ui, root);
        let model = Rc::new(RefCell::new(DockModel::new()));
        render(&mut ui, &view, &model.borrow());
        wire(&mut ui, view.clone(), Rc::clone(&model));
        let viewport = Size::new(f64::from(INITIAL_WIDTH), f64::from(INITIAL_HEIGHT));
        ui.compute_layout(viewport);
        let geometry = view
            .surface
            .tab_group_geometry(&ui, &model.borrow().dock)
            .unwrap();
        let tab = geometry
            .iter()
            .flat_map(|group| &group.tabs)
            .find(|tab| tab.pane == PaneId(2))
            .unwrap();
        let search = model
            .borrow()
            .dock
            .frame()
            .panes()
            .iter()
            .find(|pane| pane.pane == PaneId(5))
            .copied()
            .unwrap();
        let bounds = ui.effective_bounds(view.surface.pane_element(PaneId(5)).unwrap());
        let offset = -view.surface.local_point(&ui, Point::ORIGIN).to_vec2();
        let from = tab.interaction_regions[0].center() + offset;
        let to = search.rect.center() + offset;
        let pointer = PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        };
        // Exercise pointer-down while a preceding style/layout change is pending.
        ui.set_layout(view.status, Layout::new().logical_size(600.0, 24.0));
        assert!(ui.has_pending_layout());
        ui.dispatch_pointer_event(&PointerEvent::Down(PointerButtonEvent {
            pointer,
            button: Some(PointerButton::Primary),
            state: pointer_state(from, 1, true),
        }));
        assert_eq!(ui.pointer_capture(), Some(view.surface.root()));
        ui.dispatch_pointer_event(&PointerEvent::Move(PointerUpdate {
            pointer,
            current: pointer_state(to, 2, true),
            coalesced: vec![],
            predicted: vec![],
        }));
        ui.compute_layout(viewport);
        assert_eq!(
            ui.effective_bounds(view.surface.pane_element(PaneId(5)).unwrap()),
            bounds
        );
        assert_eq!(model.borrow().status, "Release to create a tab group.");
        ui.dispatch_pointer_event(&PointerEvent::Up(PointerButtonEvent {
            pointer,
            button: Some(PointerButton::Primary),
            state: pointer_state(to, 3, false),
        }));
        assert_eq!(ui.pointer_capture(), None);
        let model = model.borrow();
        assert_eq!(model.status, "Created a tab group.");
        let tabs = model
            .dock
            .frame()
            .tabs()
            .iter()
            .filter(|tab| tab.group == search.tile)
            .map(|tab| tab.pane)
            .collect::<Vec<_>>();
        assert_eq!(tabs, [PaneId(5), PaneId(2)]);
    }

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
        assert_eq!(model.status, "Resizing split");
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
