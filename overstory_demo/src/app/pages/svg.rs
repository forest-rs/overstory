use super::super::*;
use super::*;

#[derive(Clone, Debug)]
struct SvgViewportDemo {
    path_count: usize,
    selected_path: Option<usize>,
    viewport: Viewport2D,
    world_bounds: Rect,
}

impl SvgViewportDemo {
    fn new(svg: &SvgElementTree, view_width: f32, view_height: f32) -> Self {
        let world_bounds = Rect::new(0.0, 0.0, f64::from(svg.width()), f64::from(svg.height()));
        let mut viewport = Viewport2D::new(Rect::new(
            0.0,
            0.0,
            f64::from(view_width),
            f64::from(view_height),
        ));
        viewport.set_fit_mode(FitMode::Center);
        viewport.set_clamp_mode(ClampMode::KeepSomeVisible);
        viewport.set_zoom_limits(0.12, 18.0);
        viewport.set_world_bounds(Some(padded_svg_bounds(world_bounds)));
        viewport.fit_world();
        Self {
            path_count: svg.len(),
            selected_path: None,
            viewport,
            world_bounds,
        }
    }

    fn reset_to_fit(&mut self) {
        self.viewport.fit_rect(padded_svg_bounds(self.world_bounds));
    }

    fn zoom_about(&mut self, anchor: Point, factor: f64) {
        self.viewport.zoom_about_view_point(anchor, factor);
    }

    fn zoom_center(&mut self, factor: f64) {
        self.zoom_about(self.viewport.view_rect().center(), factor);
    }

    fn pan_by_view(&mut self, delta: Vec2) {
        self.viewport.pan_by_view(delta);
    }

    fn select_path(&mut self, path: Option<usize>) {
        self.selected_path = path;
    }

    fn status(&self) -> String {
        let zoom = self.viewport.zoom() * 100.0;
        format!("{} paths · {zoom:.0}%", self.path_count)
    }

    fn path_count_readout(&self) -> String {
        format!("{}", self.path_count)
    }

    fn zoom_readout(&self) -> String {
        format!("{:.0}%", self.viewport.zoom() * 100.0)
    }

    fn visible_rect_readout(&self) -> String {
        let visible = self.viewport.visible_world_rect();
        format!(
            "{:.0},{:.0} / {:.0}x{:.0}",
            visible.x0,
            visible.y0,
            visible.width(),
            visible.height(),
        )
    }

    fn transform_readout(&self) -> String {
        let origin = self.viewport.world_to_view_point(Point::ZERO);
        format!(
            "scale {:.2}\noffset {:.0},{:.0}",
            self.viewport.zoom(),
            origin.x,
            origin.y,
        )
    }

    fn selected_path_readout(&self, tree: &SvgElementTree) -> String {
        let Some(path_index) = self.selected_path else {
            return String::from("None");
        };
        let label = tree
            .path(path_index)
            .and_then(overstory_svg::SvgPathElement::source_id)
            .unwrap_or("path");
        format!("#{path_index} {label}")
    }

    fn selected_bounds_readout(&self, tree: &SvgElementTree) -> String {
        let Some(path) = self.selected_path.and_then(|index| tree.path(index)) else {
            return String::from("-");
        };
        let bounds = path.bounds();
        format!(
            "{:.0},{:.0} / {:.0}x{:.0}",
            bounds.x0,
            bounds.y0,
            bounds.width(),
            bounds.height(),
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub(in crate::app) struct SvgPage;

impl View for SvgPage {
    type Output = ();

    fn build(self, stage: &mut ChildUi<'_>) -> Self::Output {
        stage.with_child(
            Panel::new().layout(Layout::column(18.0).fill_width()),
            |page| {
                page.demo_card("IMPORT SURFACE", SVG_PAGE_WIDTH, SVG_INFO_HEIGHT, |notes| {
                    let notes_id = notes.parent();
                    notes.ui_mut().add_class(notes_id, CLASS_SVG_INFO_CARD);
                    append_svg_import_summary_to(notes);
                });

                page.with_child(
                    Panel::row(18.0).layout(
                        Layout::row(18.0).size(
                            LayoutSize::new()
                                .width(LayoutDimension::Logical(SVG_PAGE_WIDTH))
                                .height(LayoutDimension::Logical(SVG_TIGER_CARD_HEIGHT)),
                        ),
                    ),
                    |main| {
                        let viewer = main.demo_card(
                            "GHOSTSCRIPT TIGER",
                            SVG_TIGER_CARD_WIDTH,
                            SVG_TIGER_CARD_HEIGHT,
                            |specimen| {
                                let specimen_id = specimen.parent();
                                specimen
                                    .ui_mut()
                                    .add_class(specimen_id, CLASS_SVG_TIGER_CARD);
                                append_svg_specimen(specimen)
                            },
                        );

                        main.demo_card(
                            "INSPECTOR",
                            SVG_INSPECTOR_CARD_WIDTH,
                            SVG_TIGER_CARD_HEIGHT,
                            |inspector| {
                                let inspector_id = inspector.parent();
                                inspector
                                    .ui_mut()
                                    .add_class(inspector_id, CLASS_SVG_INSPECTOR_CARD);
                                if let Some(viewer) = viewer {
                                    let inspector_state =
                                        append_svg_inspector_to(inspector, &viewer);
                                    viewer.runtime.inspector.set(Some(inspector_state));
                                    refresh_svg_viewer(inspector.ui_mut(), &viewer);
                                } else {
                                    append_svg_static_inspector_to(inspector);
                                }
                            },
                        );
                    },
                );
            },
        );
    }
}

fn append_svg_specimen(specimen: &mut ChildUi<'_>) -> Option<SvgViewer> {
    let specimen_id = specimen.parent();
    let viewer = match append_svg_viewer(specimen.ui_mut(), specimen_id, TIGER_SVG) {
        Ok(viewer) => viewer,
        Err(()) => {
            specimen.add(body_text(
                "The SVG stress asset did not import any supported vector paths.",
            ));
            return None;
        }
    };
    specimen.with_child(Panel::row(8.0).align_center(), |controls| {
        append_svg_view_button(controls, "Fit", viewer.clone(), |state| {
            state.reset_to_fit();
        });
        append_svg_view_button(controls, "-", viewer.clone(), |state| {
            state.zoom_center(0.82);
        });
        append_svg_view_button(controls, "+", viewer.clone(), |state| {
            state.zoom_center(1.22);
        });
    });
    Some(viewer)
}

fn append_svg_import_summary_to(parent: &mut ChildUi<'_>) {
    parent.with_child(
        Panel::new().layout(Layout::row(18.0).fill_width()),
        |row| {
            row.with_child(
                Panel::column(6.0).layout(
                    Layout::column(6.0).size(
                        LayoutSize::new()
                            .width(LayoutDimension::Logical(414.0))
                            .min_width(LayoutDimension::Logical(0.0)),
                    ),
                ),
                |lowering| {
                    lowering.add(page_heading("Path-first lowering"));
                    lowering.add(
                        body_text("The importer keeps SVG parsing outside the UI core, then appends ordinary elements that paint through the same presentation primitive path as hand-authored vector controls."),
                    );
                },
            );
            row.with_child(
                Panel::column(6.0).layout(
                    Layout::column(6.0).size(
                        LayoutSize::new()
                            .width(LayoutDimension::Logical(322.0))
                            .min_width(LayoutDimension::Logical(0.0)),
                    ),
                ),
                |support| {
                    support.add(caption("SUPPORTED NOW"));
                    support.add(
                        body_text("Current support is intentionally static: solid fills, strokes, transforms, opacity, fill rules, caps, joins, dashes, and paint order."),
                    );
                },
            );
        },
    );
}

fn append_svg_static_inspector_to(parent: &mut ChildUi<'_>) {
    parent.add(caption("DOCUMENT"));
    append_svg_inspector_row_to(parent, "Asset", "Ghostscript Tiger");
    append_svg_inspector_row_to(parent, "Lowering", "VectorPath tree");
    append_svg_inspector_row_to(parent, "Viewport", "Pan + zoom layer");
    parent.add(Divider::horizontal().length(200.0));
    parent.add(caption("NEXT"));
    append_svg_inspector_row_to(parent, "Outline", "SVG element tree");
    append_svg_inspector_row_to(parent, "Selection", "Path metadata");
    append_svg_inspector_row_to(parent, "Properties", "Fill, stroke, transform");
}

fn append_svg_inspector_to(parent: &mut ChildUi<'_>, viewer: &SvgViewer) -> SvgInspector {
    let state = viewer.runtime.state.borrow();
    parent.add(caption("DOCUMENT"));
    append_svg_inspector_row_to(parent, "Asset", "Ghostscript Tiger");
    let paths = append_svg_inspector_row_to(parent, "Paths", &state.path_count_readout());
    let selected = append_svg_inspector_row_to(
        parent,
        "Selected",
        &state.selected_path_readout(&viewer.tree),
    );
    let bounds = append_svg_inspector_row_to(
        parent,
        "Bounds",
        &state.selected_bounds_readout(&viewer.tree),
    );
    let zoom = append_svg_inspector_row_to(parent, "Zoom", &state.zoom_readout());
    let visible = append_svg_inspector_row_to(parent, "Visible", &state.visible_rect_readout());
    let transform =
        append_svg_inspector_multiline_row_to(parent, "Layer", &state.transform_readout());
    parent.add(Divider::horizontal().length(200.0));
    parent.add(caption("PIPELINE"));
    append_svg_inspector_row_to(parent, "Lowering", "VectorPath tree");
    append_svg_inspector_row_to(parent, "Grid", "Viewport overlay");
    append_svg_inspector_row_to(parent, "Next", "Outline + properties");
    SvgInspector {
        paths,
        selected,
        bounds,
        zoom,
        visible,
        transform,
    }
}

#[cfg(test)]
fn append_svg_inspector(ui: &mut Ui, parent: ElementId, viewer: &SvgViewer) -> SvgInspector {
    ui.with_children(parent, |parent| append_svg_inspector_to(parent, viewer))
}

fn update_svg_inspector(
    ui: &mut Ui,
    inspector: SvgInspector,
    state: &SvgViewportDemo,
    tree: &SvgElementTree,
) {
    ui.set_local(
        inspector.paths,
        ui.properties().content,
        state.path_count_readout(),
    );
    ui.set_local(
        inspector.selected,
        ui.properties().content,
        state.selected_path_readout(tree),
    );
    ui.set_local(
        inspector.bounds,
        ui.properties().content,
        state.selected_bounds_readout(tree),
    );
    ui.set_local(
        inspector.zoom,
        ui.properties().content,
        state.zoom_readout(),
    );
    ui.set_local(
        inspector.visible,
        ui.properties().content,
        state.visible_rect_readout(),
    );
    ui.set_local(
        inspector.transform,
        ui.properties().content,
        state.transform_readout(),
    );
}

pub(in crate::app) fn append_svg_inspector_row_to(
    parent: &mut ChildUi<'_>,
    label: &str,
    value: &str,
) -> ElementId {
    append_svg_inspector_row_with_height_to(parent, label, value, 18.0)
}

fn append_svg_inspector_multiline_row_to(
    parent: &mut ChildUi<'_>,
    label: &str,
    value: &str,
) -> ElementId {
    append_svg_inspector_row_with_height_to(parent, label, value, 38.0)
}

fn append_svg_inspector_row_with_height_to(
    parent: &mut ChildUi<'_>,
    label: &str,
    value: &str,
    height: f32,
) -> ElementId {
    parent.with_child(
        Panel::row(8.0).layout(
            Layout::row(8.0).size(
                LayoutSize::new()
                    .width(LayoutDimension::Percent(1.0))
                    .height(LayoutDimension::Logical(height))
                    .min_width(LayoutDimension::Logical(0.0)),
            ),
        ),
        |row| {
            let label = row.add(caption(label));
            row.ui_mut().set_layout(
                label,
                Layout::new()
                    .logical_size(78.0, height)
                    .flex_grow(0.0)
                    .flex_shrink(0.0),
            );
            let value = row.add(body_text(value));
            row.ui_mut().set_layout(
                value,
                Layout::new()
                    .size(
                        LayoutSize::new()
                            .height(LayoutDimension::Logical(height))
                            .min_width(LayoutDimension::Logical(0.0)),
                    )
                    .flex_grow(1.0)
                    .flex_shrink(1.0),
            );
            value
        },
    )
}

#[derive(Clone)]
struct SvgViewer {
    tree: Rc<SvgElementTree>,
    host: ElementId,
    grid: ElementId,
    content: ElementId,
    path_elements: Rc<[ElementId]>,
    selection: ElementId,
    status: ElementId,
    runtime: Rc<SvgViewerRuntime>,
}

#[derive(Debug)]
struct SvgViewerRuntime {
    state: RefCell<SvgViewportDemo>,
    inspector: Cell<Option<SvgInspector>>,
    wobble_generation: Cell<u32>,
    wobble_timers: RefCell<Vec<TimerId>>,
}

impl SvgViewerRuntime {
    fn new(state: SvgViewportDemo) -> Self {
        Self {
            state: RefCell::new(state),
            inspector: Cell::new(None),
            wobble_generation: Cell::new(0),
            wobble_timers: RefCell::new(Vec::new()),
        }
    }

    fn next_wobble_generation(&self) -> u32 {
        let next = self.wobble_generation.get().wrapping_add(1);
        self.wobble_generation.set(next);
        next
    }
}

#[derive(Clone, Copy, Debug)]
struct SvgInspector {
    paths: ElementId,
    selected: ElementId,
    bounds: ElementId,
    zoom: ElementId,
    visible: ElementId,
    transform: ElementId,
}

fn append_svg_viewer(ui: &mut Ui, parent: ElementId, bytes: &[u8]) -> Result<SvgViewer, ()> {
    let svg = SvgElementTree::from_bytes(bytes, SvgImportOptions::default()).map_err(|_| ())?;
    if svg.is_empty() {
        return Err(());
    }
    let tree = Rc::new(svg);
    let state = SvgViewportDemo::new(&tree, SVG_VIEWPORT_WIDTH, SVG_VIEWPORT_HEIGHT);
    let host = ui.append(
        parent,
        Viewport::new()
            .cursor_icon(CursorIcon::Grab)
            .layout(
                Layout::new()
                    .relative()
                    .logical_size(SVG_VIEWPORT_WIDTH, SVG_VIEWPORT_HEIGHT),
            )
            .background(opt_solid(0x13, 0x13, 0x1b, 0xff))
            .border_brush(opt_solid(0x44, 0x46, 0x54, 0xff))
            .border_width(1.0)
            .corner_radius(12.0),
    );

    let grid = ui.append(
        host,
        Panel::new().layout(
            Layout::new()
                .absolute_at(0.0, 0.0)
                .logical_size(SVG_VIEWPORT_WIDTH, SVG_VIEWPORT_HEIGHT),
        ),
    );
    let world_bounds = state.world_bounds;
    let content = ui.append(
        host,
        Panel::new().layout(Layout::new().absolute_at(0.0, 0.0).logical_size(
            f32_coord(world_bounds.width()),
            f32_coord(world_bounds.height()),
        )),
    );
    let svg_root = ui.append(content, (*tree).clone());
    let path_elements: Rc<[ElementId]> = Rc::from(
        ui.get(svg_root)
            .map(|element| element.children().to_vec())
            .unwrap_or_default()
            .into_boxed_slice(),
    );
    let selection = ui.append(
        host,
        Panel::new()
            .layout(Layout::new().absolute_at(0.0, 0.0).logical_size(0.0, 0.0))
            .border_brush(opt_solid(0x6f, 0x8f, 0xff, 0xff))
            .border_width(1.5)
            .corner_radius(4.0),
    );

    let status = ui.append(parent, caption(&state.status()));
    let viewer = SvgViewer {
        tree,
        host,
        grid,
        content,
        path_elements,
        selection,
        status,
        runtime: Rc::new(SvgViewerRuntime::new(state)),
    };
    install_svg_viewer_interaction(ui, &viewer);
    refresh_svg_viewer(ui, &viewer);
    Ok(viewer)
}

fn install_svg_viewer_interaction(ui: &mut Ui, viewer: &SvgViewer) {
    let pick_viewer = viewer.clone();
    ui.on_pointer(
        viewer.host,
        overstory::PointerEventKind::Down,
        move |ui, event| {
            if !matches!(
                event.phase,
                overstory::Phase::Capture | overstory::Phase::Target
            ) {
                return;
            }
            let Some(position) = event.position else {
                return;
            };
            let view_point = local_point(ui, pick_viewer.host, position);
            select_svg_path_at_view_point(ui, &pick_viewer, view_point);
        },
    );

    let drag_viewer = viewer.clone();
    ui.on_drag(viewer.host, move |ui, event| {
        if !matches!(
            event.phase,
            overstory::Phase::Capture | overstory::Phase::Target
        ) || event.delta.hypot() <= 0.0
        {
            return;
        }
        let _ = ui.capture_pointer(drag_viewer.host);
        drag_viewer
            .runtime
            .state
            .borrow_mut()
            .pan_by_view(event.delta);
        refresh_svg_viewer(ui, &drag_viewer);
        event.mark_handled();
        event.stop_propagation();
    });

    let scroll_viewer = viewer.clone();
    ui.on_scroll(viewer.host, move |ui, event| {
        if !matches!(
            event.phase,
            overstory::Phase::Capture | overstory::Phase::Target
        ) {
            return;
        }
        let delta = event.logical_delta(Vec2::new(48.0, 48.0), Vec2::new(260.0, 260.0));
        let amount = if delta.y.abs() >= delta.x.abs() {
            delta.y
        } else {
            delta.x
        };
        if amount.abs() <= 0.01 {
            return;
        }
        let anchor = local_point(ui, scroll_viewer.host, event.position);
        let factor = (-amount * SVG_ZOOM_SENSITIVITY).exp().clamp(0.2, 5.0);
        scroll_viewer
            .runtime
            .state
            .borrow_mut()
            .zoom_about(anchor, factor);
        refresh_svg_viewer(ui, &scroll_viewer);
        event.mark_handled();
        event.stop_propagation();
    });
}

fn select_svg_path_at_view_point(ui: &mut Ui, viewer: &SvgViewer, view_point: Point) {
    let (previous, selected) = {
        let mut state = viewer.runtime.state.borrow_mut();
        let previous = state.selected_path;
        let world_point = state.viewport.view_to_world_point(view_point);
        let params = svg_hit_params();
        let selected = viewer
            .tree
            .hit_test_path(world_point, &params)
            .map(|hit| hit.path_index());
        state.select_path(selected);
        (previous, selected)
    };
    if previous != selected {
        reset_svg_path_wobble(ui, viewer, previous);
    }
    if let Some(path_index) = selected {
        start_svg_path_wobble(ui, viewer, path_index);
    }
    refresh_svg_viewer(ui, viewer);
}

fn start_svg_path_wobble(ui: &mut Ui, viewer: &SvgViewer, path_index: usize) {
    let Some(element) = viewer.path_elements.get(path_index).copied() else {
        return;
    };
    let Some(path) = viewer.tree.path(path_index) else {
        return;
    };
    cancel_svg_wobble_timers(ui, viewer);
    let generation = viewer.runtime.next_wobble_generation();
    let path_bounds = path.bounds();
    let center = local_svg_path_center(path_bounds);
    let selection = viewer.selection;
    let selection_center = local_svg_path_center(
        viewer
            .runtime
            .state
            .borrow()
            .viewport
            .world_to_view_rect(path_bounds),
    );
    set_svg_selection_wobble(ui, element, center, selection, selection_center, 1.1);
    let first_timer = ui.schedule_timer_once(element, millis(SVG_SELECTION_WOBBLE_UP_MS));
    viewer.runtime.wobble_timers.borrow_mut().push(first_timer);

    let runtime = Rc::clone(&viewer.runtime);
    let phase = Cell::new(0_u8);
    ui.on_timer(element, move |ui, _event| {
        if runtime.wobble_generation.get() != generation {
            ui.remove_timer_handler(element);
            return;
        }
        match phase.get() {
            0 => {
                phase.set(1);
                set_svg_selection_wobble(ui, element, center, selection, selection_center, 0.985);
                runtime.wobble_timers.borrow_mut().clear();
                let second_timer =
                    ui.schedule_timer_once(element, millis(SVG_SELECTION_WOBBLE_DOWN_MS));
                runtime.wobble_timers.borrow_mut().push(second_timer);
            }
            _ => {
                runtime.wobble_timers.borrow_mut().clear();
                ui.set_visual_transform(element, Affine::IDENTITY);
                ui.set_visual_transform(selection, Affine::IDENTITY);
                ui.remove_timer_handler(element);
            }
        }
    });
}

fn reset_svg_path_wobble(ui: &mut Ui, viewer: &SvgViewer, path_index: Option<usize>) {
    cancel_svg_wobble_timers(ui, viewer);
    viewer.runtime.next_wobble_generation();
    ui.set_visual_transform(viewer.selection, Affine::IDENTITY);
    let Some(element) = path_index.and_then(|index| viewer.path_elements.get(index).copied())
    else {
        return;
    };
    ui.set_visual_transform(element, Affine::IDENTITY);
    ui.remove_timer_handler(element);
}

fn set_svg_selection_wobble(
    ui: &mut Ui,
    path: ElementId,
    path_center: Point,
    selection: ElementId,
    selection_center: Point,
    scale: f64,
) {
    ui.set_visual_transform(path, scale_about(path_center, scale));
    ui.set_visual_transform(selection, scale_about(selection_center, scale));
}

fn cancel_svg_wobble_timers(ui: &mut Ui, viewer: &SvgViewer) {
    for timer in viewer.runtime.wobble_timers.borrow_mut().drain(..) {
        let _ = ui.cancel_timer(timer);
    }
}

fn local_svg_path_center(bounds: Rect) -> Point {
    Point::new(bounds.width() * 0.5, bounds.height() * 0.5)
}

fn scale_about(center: Point, scale: f64) -> Affine {
    Affine::translate((center.x, center.y))
        * Affine::scale(scale)
        * Affine::translate((-center.x, -center.y))
}

fn svg_hit_params() -> HitParams {
    // Keep fill tolerance at zero until path tolerance is true near-edge
    // geometry rather than a bounding-box fallback.
    HitParams {
        fill_tolerance: 0.0,
        stroke_tolerance: 0.0,
        prefer_fill: true,
    }
}

fn append_svg_view_button(
    parent: &mut ChildUi<'_>,
    label: &str,
    viewer: SvgViewer,
    action: impl Fn(&mut SvgViewportDemo) + 'static,
) {
    parent.add(
        Button::new(label)
            .font_size(13.0)
            .on_click(move |ui, event| {
                if !matches!(
                    event.phase,
                    overstory::Phase::Target | overstory::Phase::Bubble
                ) {
                    return;
                }
                action(&mut viewer.runtime.state.borrow_mut());
                refresh_svg_viewer(ui, &viewer);
                event.mark_handled();
            })
            .tooltip("Adjust the SVG viewport"),
    );
}

fn refresh_svg_viewer(ui: &mut Ui, viewer: &SvgViewer) {
    let state = viewer.runtime.state.borrow();
    clear_children(ui, viewer.grid);
    append_svg_grid(ui, viewer.grid, &state);
    ui.set_visual_transform(viewer.content, viewport_transform(&state.viewport));
    update_svg_selection_overlay(ui, viewer, &state);
    ui.set_local(viewer.status, ui.properties().content, state.status());
    if let Some(inspector) = viewer.runtime.inspector.get() {
        update_svg_inspector(ui, inspector, &state, &viewer.tree);
    }
}

fn update_svg_selection_overlay(ui: &mut Ui, viewer: &SvgViewer, state: &SvgViewportDemo) {
    let view_bounds = state
        .selected_path
        .and_then(|index| viewer.tree.path(index))
        .map(|path| state.viewport.world_to_view_rect(path.bounds()))
        .unwrap_or(Rect::ZERO);
    ui.set_layout(
        viewer.selection,
        Layout::new()
            .absolute_at(f32_coord(view_bounds.x0), f32_coord(view_bounds.y0))
            .logical_size(
                f32_coord(view_bounds.width().max(0.0)),
                f32_coord(view_bounds.height().max(0.0)),
            ),
    );
}

fn viewport_transform(viewport: &Viewport2D) -> Affine {
    let origin = viewport.world_to_view_point(Point::ZERO);
    let x = viewport.world_to_view_point(Point::new(1.0, 0.0));
    let y = viewport.world_to_view_point(Point::new(0.0, 1.0));
    Affine::new([
        x.x - origin.x,
        x.y - origin.y,
        y.x - origin.x,
        y.y - origin.y,
        origin.x,
        origin.y,
    ])
}

fn append_svg_grid(ui: &mut Ui, parent: ElementId, state: &SvgViewportDemo) {
    let viewport = &state.viewport;
    let visible = viewport.visible_world_rect();
    let spacing = viewport.suggest_grid_spacing(12.0).max(1.0);
    append_svg_grid_axis(ui, parent, viewport, visible.x0, visible.x1, spacing, true);
    append_svg_grid_axis(ui, parent, viewport, visible.y0, visible.y1, spacing, false);
}

fn append_svg_grid_axis(
    ui: &mut Ui,
    parent: ElementId,
    viewport: &Viewport2D,
    start: f64,
    end: f64,
    spacing: f64,
    vertical: bool,
) {
    let view = viewport.view_rect();
    let mut world = (start / spacing).floor() * spacing;
    let mut count = 0;
    while world <= end && count < SVG_GRID_LIMIT {
        let index = (world / spacing).round();
        let major = ((index / 5.0) - (index / 5.0).round()).abs() <= 0.001;
        let color = if major {
            Color::from_rgba8(0x40, 0x42, 0x50, 0x86)
        } else {
            Color::from_rgba8(0x2a, 0x2b, 0x35, 0x72)
        };
        let coordinate = if vertical {
            viewport.world_to_view_point(Point::new(world, 0.0)).x
        } else {
            viewport.world_to_view_point(Point::new(0.0, world)).y
        };
        let rect = if vertical {
            Rect::new(coordinate, view.y0, coordinate + 1.0, view.y1)
        } else {
            Rect::new(view.x0, coordinate, view.x1, coordinate + 1.0)
        };
        append_svg_grid_rect(ui, parent, rect, color);
        world += spacing;
        count += 1;
    }
}

fn append_svg_grid_rect(ui: &mut Ui, parent: ElementId, rect: Rect, color: Color) -> ElementId {
    let element = ui.append(
        parent,
        Panel::new().class(CLASS_SVG_GRID_LINE).layout(
            Layout::new()
                .absolute_at(f32_coord(rect.x0), f32_coord(rect.y0))
                .logical_size(f32_coord(rect.width()), f32_coord(rect.height())),
        ),
    );
    ui.set_local(element, ui.properties().background, Some(color.into()));
    element
}

fn padded_svg_bounds(bounds: Rect) -> Rect {
    let pad_x = (bounds.width() * 0.08).max(8.0);
    let pad_y = (bounds.height() * 0.08).max(8.0);
    bounds.inflate(pad_x, pad_y)
}

fn local_point(ui: &Ui, parent: ElementId, position: Point) -> Point {
    let bounds = ui.effective_bounds(parent);
    Point::new(position.x - bounds.x0, position.y - bounds.y0)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "demo logical coordinates fit f32 layout values"
)]
fn f32_coord(value: f64) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::pages::show_demo_page;
    use peniko::kurbo::Size;

    #[test]
    fn tiger_svg_asset_stresses_path_importer() {
        let svg = SvgElementTree::from_bytes(TIGER_SVG, SvgImportOptions::default())
            .expect("Tiger SVG asset should parse");

        assert!(
            svg.len() > 100,
            "Tiger SVG should stress the path importer, got {} paths",
            svg.len(),
        );
    }

    #[test]
    fn svg_viewer_zoom_updates_content_transform_without_rebuilding_paths() {
        let svg = br##"
            <svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
              <rect x="25" y="25" width="50" height="50" fill="#ff8a00"/>
            </svg>
        "##;
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(360.0, 280.0));
        let viewer = append_svg_viewer(&mut ui, root, svg).expect("test SVG should import");
        let inspector = append_svg_inspector(&mut ui, root, &viewer);
        viewer.runtime.inspector.set(Some(inspector));
        refresh_svg_viewer(&mut ui, &viewer);
        ui.compute_layout(Size::new(360.0, 280.0));

        let before_count = count_type(&ui, viewer.host, overstory::TYPE_VECTOR_PATH);
        let before_bounds = ui.effective_bounds(viewer.content);
        let before_scale = transform_scale(ui.effective_visual_transform(viewer.content));
        let before_grid_thickness =
            first_grid_line_thickness(&ui, viewer.grid).expect("viewer should contain grid lines");

        viewer.runtime.state.borrow_mut().zoom_center(2.0);
        refresh_svg_viewer(&mut ui, &viewer);
        ui.compute_layout(Size::new(360.0, 280.0));

        let after_count = count_type(&ui, viewer.host, overstory::TYPE_VECTOR_PATH);
        let after_bounds = ui.effective_bounds(viewer.content);
        let after_scale = transform_scale(ui.effective_visual_transform(viewer.content));
        let after_grid_thickness =
            first_grid_line_thickness(&ui, viewer.grid).expect("viewer should contain grid lines");
        assert_eq!(after_count, before_count);
        assert_eq!(after_bounds, before_bounds);
        assert_close(before_grid_thickness, 1.0, "grid starts at 1px");
        assert_close(after_grid_thickness, 1.0, "grid stays at 1px after zoom");
        assert!(
            after_scale > before_scale * 1.5,
            "zoom should enlarge the content presentation transform: {before_scale} -> {after_scale}",
        );
    }

    #[test]
    fn svg_viewer_precisely_picks_path_content() {
        let svg = br##"
            <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20">
              <path id="triangle" d="M0 0 L12 0 L0 12 Z" fill="#00ff00"/>
            </svg>
        "##;
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(360.0, 300.0));
        let viewer = append_svg_viewer(&mut ui, root, svg).expect("test SVG should import");
        let inspector = append_svg_inspector(&mut ui, root, &viewer);
        viewer.runtime.inspector.set(Some(inspector));
        refresh_svg_viewer(&mut ui, &viewer);
        ui.compute_layout(Size::new(360.0, 300.0));

        let hit = viewer
            .runtime
            .state
            .borrow()
            .viewport
            .world_to_view_point(Point::new(2.0, 2.0));
        select_svg_path_at_view_point(&mut ui, &viewer, hit);
        ui.compute_layout(Size::new(360.0, 300.0));
        assert_eq!(viewer.runtime.state.borrow().selected_path, Some(0));
        let selected_path = viewer.path_elements[0];
        assert!(
            transform_scale(ui.effective_visual_transform(selected_path)) > 1.05,
            "selecting a path should start a direct path wobble",
        );
        assert!(
            transform_scale(ui.effective_visual_transform(viewer.selection)) > 1.05,
            "selection overlay should follow the path wobble scale",
        );
        ui.pump_timers(millis(SVG_SELECTION_WOBBLE_UP_MS));
        assert!(
            transform_scale(ui.effective_visual_transform(selected_path)) < 1.0,
            "path wobble should overshoot back below the final scale",
        );
        assert!(
            transform_scale(ui.effective_visual_transform(viewer.selection)) < 1.0,
            "selection overlay should follow the wobble overshoot",
        );
        ui.pump_timers(millis(
            SVG_SELECTION_WOBBLE_UP_MS + SVG_SELECTION_WOBBLE_DOWN_MS,
        ));
        assert_close(
            transform_scale(ui.effective_visual_transform(selected_path)),
            1.0,
            "path wobble returns to identity scale",
        );
        assert_close(
            transform_scale(ui.effective_visual_transform(viewer.selection)),
            1.0,
            "selection overlay wobble returns to identity scale",
        );
        assert!(
            ui.effective_bounds(viewer.selection).width() > 0.0,
            "selection overlay should track the selected path bounds",
        );

        let miss_inside_bounds = viewer
            .runtime
            .state
            .borrow()
            .viewport
            .world_to_view_point(Point::new(10.0, 10.0));
        select_svg_path_at_view_point(&mut ui, &viewer, miss_inside_bounds);
        ui.compute_layout(Size::new(360.0, 300.0));
        assert_eq!(viewer.runtime.state.borrow().selected_path, None);
        assert_close(
            ui.effective_bounds(viewer.selection).width(),
            0.0,
            "selection overlay clears after a precise miss",
        );
    }

    #[test]
    fn svg_viewer_selection_pointer_down_still_starts_pan_drag() {
        let svg = br##"
            <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20">
              <path id="triangle" d="M0 0 L12 0 L0 12 Z" fill="#00ff00"/>
            </svg>
        "##;
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(360.0, 300.0));
        let viewer = append_svg_viewer(&mut ui, root, svg).expect("test SVG should import");
        ui.compute_layout(Size::new(360.0, 300.0));

        let local_hit = viewer
            .runtime
            .state
            .borrow()
            .viewport
            .world_to_view_point(Point::new(2.0, 2.0));
        let host_bounds = ui.effective_bounds(viewer.host);
        let hit = Point::new(host_bounds.x0 + local_hit.x, host_bounds.y0 + local_hit.y);
        let visible_before = viewer.runtime.state.borrow().viewport.visible_world_rect();

        let down = ui.dispatch_pointer_event(&pointer_down(hit, 1));
        assert!(
            down.capture_changed,
            "selection pointer down should still start pointer capture for drag"
        );
        assert_eq!(viewer.runtime.state.borrow().selected_path, Some(0));

        let moved = ui.dispatch_pointer_event(&pointer_move(
            Point::new(hit.x + 28.0, hit.y + 12.0),
            2,
            true,
        ));
        let visible_after = viewer.runtime.state.borrow().viewport.visible_world_rect();
        assert!(
            moved.handled,
            "pan drag should be handled by the SVG viewport"
        );
        assert_ne!(
            visible_after, visible_before,
            "dragging after selection should pan the SVG viewport",
        );
    }

    #[test]
    fn svg_page_puts_import_summary_above_large_tiger_and_inspector() {
        let mut ui = build_ui();
        let stage = stage_element(&ui);
        show_demo_page(&mut ui, stage, DemoPage::Svg);
        ui.compute_layout(Size::new(
            f64::from(INITIAL_SIZE.0),
            f64::from(INITIAL_SIZE.1),
        ));

        let info = find_by_class(&ui, stage, CLASS_SVG_INFO_CARD)
            .expect("SVG page should contain an import summary card");
        let tiger = find_by_class(&ui, stage, CLASS_SVG_TIGER_CARD)
            .expect("SVG page should contain the Tiger card");
        let inspector = find_by_class(&ui, stage, CLASS_SVG_INSPECTOR_CARD)
            .expect("SVG page should contain an inspector card");
        let info_bounds = ui.effective_bounds(info);
        let tiger_bounds = ui.effective_bounds(tiger);
        let inspector_bounds = ui.effective_bounds(inspector);

        assert!(
            info_bounds.y1 <= tiger_bounds.y0 + 0.01,
            "import summary should sit above Tiger: {info_bounds:?} then {tiger_bounds:?}",
        );
        assert!(
            tiger_bounds.width() > inspector_bounds.width() * 1.8,
            "Tiger should get most horizontal space: {tiger_bounds:?} vs {inspector_bounds:?}",
        );
        assert!(
            tiger_bounds.height() > info_bounds.height() * 3.0,
            "Tiger card should dominate vertical space: {tiger_bounds:?} vs {info_bounds:?}",
        );
    }

    fn assert_close(actual: f64, expected: f64, label: &str) {
        assert!(
            (actual - expected).abs() < 0.01,
            "{label}: expected {expected}, got {actual}",
        );
    }

    fn stage_element(ui: &Ui) -> ElementId {
        let root_children = ui
            .get(ui.root())
            .expect("root should exist")
            .children()
            .to_vec();
        let [_, stage] = *root_children else {
            panic!("gallery root should contain nav and stage");
        };
        stage
    }

    fn find_by_class(ui: &Ui, element: ElementId, class: ClassId) -> Option<ElementId> {
        let current = ui.get(element)?;
        if current.classes().contains(&class) {
            return Some(element);
        }
        current
            .children()
            .iter()
            .find_map(|child| find_by_class(ui, *child, class))
    }

    fn count_type(ui: &Ui, element: ElementId, type_tag: understory_style::TypeTag) -> usize {
        let Some(current) = ui.get(element) else {
            return 0;
        };
        let current_count = usize::from(current.type_tag() == Some(type_tag));
        current_count
            + current
                .children()
                .iter()
                .map(|child| count_type(ui, *child, type_tag))
                .sum::<usize>()
    }

    fn transform_scale(transform: Affine) -> f64 {
        let origin = transform * Point::ZERO;
        let unit_x = transform * Point::new(1.0, 0.0);
        (unit_x - origin).hypot()
    }

    fn first_grid_line_thickness(ui: &Ui, element: ElementId) -> Option<f64> {
        let current = ui.get(element)?;
        if current.classes().contains(&CLASS_SVG_GRID_LINE) {
            let bounds = ui.effective_bounds(element);
            return Some(bounds.width().min(bounds.height()));
        }
        current
            .children()
            .iter()
            .find_map(|child| first_grid_line_thickness(ui, *child))
    }

    fn pointer_down(point: Point, time: u64) -> ui_events::pointer::PointerEvent {
        use ui_events::pointer::{PointerButton, PointerButtonEvent, PointerEvent};

        PointerEvent::Down(PointerButtonEvent {
            pointer: pointer_info(),
            button: Some(PointerButton::Primary),
            state: pointer_state(point, time, true),
        })
    }

    fn pointer_move(
        point: Point,
        time: u64,
        primary_down: bool,
    ) -> ui_events::pointer::PointerEvent {
        use ui_events::pointer::{PointerEvent, PointerUpdate};

        PointerEvent::Move(PointerUpdate {
            pointer: pointer_info(),
            current: pointer_state(point, time, primary_down),
            coalesced: Vec::new(),
            predicted: Vec::new(),
        })
    }

    fn pointer_info() -> ui_events::pointer::PointerInfo {
        use ui_events::pointer::{PointerId, PointerInfo, PointerType};

        PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        }
    }

    fn pointer_state(
        point: Point,
        time: u64,
        primary_down: bool,
    ) -> ui_events::pointer::PointerState {
        use ui_events::pointer::{PointerButton, PointerButtons, PointerState};

        let mut buttons = PointerButtons::new();
        if primary_down {
            buttons.insert(PointerButton::Primary);
        }
        let mut state = PointerState {
            time,
            buttons,
            count: 1,
            pressure: if primary_down { 0.5 } else { 0.0 },
            ..PointerState::default()
        };
        state.position.x = point.x;
        state.position.y = point.y;
        state
    }
}
