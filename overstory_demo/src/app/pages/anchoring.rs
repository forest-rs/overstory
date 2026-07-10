use super::super::*;
use alloc::vec::Vec;
use peniko::kurbo::{Insets, Size};
use understory_anchor::{
    Anchor, AnchorConstraint, AnchorFrame, AnchorInput, AnchorPolicy, AnchorPositionOption,
    AnchorRects, RectReference, ScoringPolicy, Side, resolve_anchor,
};

#[derive(Clone, Copy, Debug)]
pub(super) struct AnchoringPage;

const ANCHOR_SCENE_WIDTH: f64 = 340.0;
const ANCHOR_SCENE_HEIGHT: f64 = 268.0;
const FALLBACK_SCENE_HEIGHT: f64 = 620.0;
const ANCHOR_BUTTON_WIDTH: f64 = 112.0;
const ANCHOR_BUTTON_HEIGHT: f64 = 34.0;
const ANCHOR_VISUAL_HEIGHT: f64 = 308.0;
const ANCHOR_ANCHOR_BRUSH: (u8, u8, u8, u8) = (0x59, 0x75, 0xe6, 0x52);
const ANCHOR_RESOLVED_BRUSH: (u8, u8, u8, u8) = (0x9d, 0xf8, 0xb0, 0x74);
const ANCHOR_GHOST_BRUSH: (u8, u8, u8, u8) = (0x9d, 0xf8, 0xb0, 0x20);
const ANCHOR_ARROW_BRUSH: (u8, u8, u8, u8) = (0xf4, 0xc9, 0x5f, 0xd8);

impl View for AnchoringPage {
    type Output = ();

    fn build(self, stage: &mut ChildUi<'_>) -> Self::Output {
        let open_popup = Rc::new(Cell::new(None));

        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            |row| {
                row.demo_responsive_card("PREFERRED SIDES", PAGE_CARD_WIDTH, |card| {
                    card.add(control_label("Popup and tooltip targets"));
                    card.with_child(anchor_scene(ANCHOR_SCENE_HEIGHT), |scene| {
                        add_anchor_target(
                            scene,
                            Rc::clone(&open_popup),
                            "Top",
                            Placement::TOP_START,
                            114.0,
                            18.0,
                            CLASS_DEMO_ANCHOR_TARGET,
                        );
                        add_anchor_target(
                            scene,
                            Rc::clone(&open_popup),
                            "Left",
                            Placement::LEFT_START,
                            22.0,
                            116.0,
                            CLASS_DEMO_ANCHOR_TARGET,
                        );
                        add_anchor_target(
                            scene,
                            Rc::clone(&open_popup),
                            "Right",
                            Placement::RIGHT_START,
                            206.0,
                            116.0,
                            CLASS_DEMO_ANCHOR_TARGET,
                        );
                        add_anchor_target(
                            scene,
                            Rc::clone(&open_popup),
                            "Bottom",
                            Placement::BOTTOM_START,
                            114.0,
                            214.0,
                            CLASS_DEMO_ANCHOR_TARGET,
                        );
                    });
                });

                row.demo_responsive_card("VIEWPORT FALLBACK", PAGE_CARD_WIDTH, |card| {
                    card.add(control_label("Bottom preference near the viewport edge"));
                    card.with_child(anchor_scene(FALLBACK_SCENE_HEIGHT), |scene| {
                        scene.add(
                            Panel::new()
                                .layout(
                                    Layout::new()
                                        .absolute_at(156.0, 62.0)
                                        .logical_size(28.0, 460.0),
                                )
                                .background(solid(0x59, 0x75, 0xe6, 0x2a))
                                .corner_radius(14.0),
                        );
                        add_anchor_target(
                            scene,
                            Rc::clone(&open_popup),
                            "Bottom edge",
                            Placement::BOTTOM_START,
                            108.0,
                            540.0,
                            CLASS_DEMO_ANCHOR_FLIP_TARGET,
                        );
                    });
                });
            },
        );

        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            |row| {
                row.demo_responsive_card("ALIGNMENT & ARROWS", PAGE_CARD_WIDTH, |card| {
                    card.add(control_label("Start, center, and end placement"));
                    card.with_child(anchor_scene(ANCHOR_VISUAL_HEIGHT), |scene| {
                        add_alignment_sample(scene, "Start", Placement::BOTTOM_START, 24.0);
                        add_alignment_sample(scene, "Center", Placement::BOTTOM, 108.0);
                        add_alignment_sample(scene, "End", Placement::BOTTOM_END, 192.0);
                    });
                });

                row.demo_responsive_card("ANCHORS & CONSTRAINTS", PAGE_CARD_WIDTH, |card| {
                    card.add(control_label("Point, multi-rect, and bounded size"));
                    card.with_child(anchor_scene(ANCHOR_VISUAL_HEIGHT), |scene| {
                        add_point_anchor_sample(scene);
                        add_multi_rect_sample(scene);
                        add_size_constraint_sample(scene);
                    });
                });
            },
        );
    }
}

fn anchor_scene(height: f64) -> Panel {
    Panel::new()
        .class(CLASS_DEMO_ANCHOR_SCENE)
        .layout(
            Layout::new()
                .relative()
                .logical_size(ANCHOR_SCENE_WIDTH, height),
        )
        .background(solid(0x59, 0x75, 0xe6, 0x12))
        .border_brush(solid(0x8f, 0x98, 0xaa, 0x66))
        .border_width(1.0)
        .corner_radius(10.0)
}

fn add_anchor_target(
    parent: &mut ChildUi<'_>,
    open_popup: Rc<Cell<Option<(ElementId, ElementId)>>>,
    label: &'static str,
    placement: Placement,
    x: f64,
    y: f64,
    class: &'static str,
) -> ElementId {
    parent.add(
        Button::new(label)
            .class(class)
            .font_size(13.0)
            .layout(
                Layout::new()
                    .absolute_at(x, y)
                    .logical_size(ANCHOR_BUTTON_WIDTH, ANCHOR_BUTTON_HEIGHT),
            )
            .on_activate(anchor_popup_handler(
                open_popup,
                placement,
                popup_title(label, placement),
            ))
            .with_tooltip(
                Tooltip::new(format!("{label} tooltip"))
                    .placement(placement)
                    .offset(0.0, 8.0),
            ),
    )
}

fn add_alignment_sample(
    parent: &mut ChildUi<'_>,
    label: &'static str,
    placement: Placement,
    y: f64,
) {
    let anchor = Rect::new(126.0, y + 12.0, 214.0, y + 38.0);
    let frame = resolve_demo_anchor(
        Anchor::Rect(anchor),
        Size::new(92.0, 30.0),
        placement,
        &[placement.opposite()],
        &[
            AnchorConstraint::Offset {
                main_axis: 8.0,
                cross_axis: 0.0,
            },
            AnchorConstraint::Shift {
                padding: Insets::uniform(8.0),
            },
            AnchorConstraint::Arrow {
                size: Size::new(14.0, 7.0),
                padding: 8.0,
            },
        ],
        ANCHOR_SCENE_WIDTH,
        ANCHOR_VISUAL_HEIGHT,
    );
    add_sample_label(parent, label, 16.0, y + 14.0);
    add_visual_rect(
        parent,
        anchor,
        CLASS_DEMO_ANCHOR_REFERENCE,
        ANCHOR_ANCHOR_BRUSH,
    );
    add_frame_visuals(parent, &frame);
}

fn add_point_anchor_sample(parent: &mut ChildUi<'_>) {
    let point = Point::new(112.0, 58.0);
    let frame = resolve_demo_anchor(
        Anchor::Point(point),
        Size::new(88.0, 30.0),
        Placement::RIGHT,
        &[Placement::LEFT],
        &[
            AnchorConstraint::Offset {
                main_axis: 10.0,
                cross_axis: 0.0,
            },
            AnchorConstraint::Arrow {
                size: Size::new(12.0, 8.0),
                padding: 6.0,
            },
        ],
        ANCHOR_SCENE_WIDTH,
        ANCHOR_VISUAL_HEIGHT,
    );
    add_sample_label(parent, "Point", 16.0, 18.0);
    add_point_marker(parent, point);
    add_frame_visuals(parent, &frame);
}

fn add_multi_rect_sample(parent: &mut ChildUi<'_>) {
    let rects = [
        Rect::new(118.0, 122.0, 206.0, 136.0),
        Rect::new(102.0, 142.0, 230.0, 156.0),
        Rect::new(102.0, 162.0, 154.0, 176.0),
    ];
    let frame = resolve_demo_anchor(
        Anchor::Rects {
            rects: AnchorRects {
                rects: &rects,
                primary: Some(0),
                focus: Some(2),
            },
            reference: RectReference::Focus,
        },
        Size::new(98.0, 28.0),
        Placement::BOTTOM,
        &[Placement::TOP],
        &[
            AnchorConstraint::Offset {
                main_axis: 8.0,
                cross_axis: 0.0,
            },
            AnchorConstraint::Shift {
                padding: Insets::uniform(8.0),
            },
            AnchorConstraint::Arrow {
                size: Size::new(12.0, 7.0),
                padding: 6.0,
            },
        ],
        ANCHOR_SCENE_WIDTH,
        ANCHOR_VISUAL_HEIGHT,
    );
    add_sample_label(parent, "Focus rect", 16.0, 118.0);
    for rect in rects {
        add_visual_rect(
            parent,
            rect,
            CLASS_DEMO_ANCHOR_REFERENCE,
            ANCHOR_ANCHOR_BRUSH,
        );
    }
    add_visual_rect(
        parent,
        frame.reference_rect,
        CLASS_DEMO_ANCHOR_REFERENCE,
        (0x59, 0x75, 0xe6, 0x92),
    );
    add_frame_visuals(parent, &frame);
}

fn add_size_constraint_sample(parent: &mut ChildUi<'_>) {
    let anchor = Rect::new(96.0, 250.0, 152.0, 272.0);
    let desired = Size::new(128.0, 42.0);
    let frame = resolve_demo_anchor(
        Anchor::Rect(anchor),
        desired,
        Placement::RIGHT,
        &[Placement::TOP],
        &[
            AnchorConstraint::Offset {
                main_axis: 10.0,
                cross_axis: 0.0,
            },
            AnchorConstraint::Size {
                min: Size::new(64.0, 24.0),
                max: Some(Size::new(78.0, 28.0)),
                allow_shrink: true,
            },
            AnchorConstraint::KeepInBounds {
                padding: Insets::uniform(8.0),
            },
        ],
        ANCHOR_SCENE_WIDTH,
        ANCHOR_VISUAL_HEIGHT,
    );
    add_sample_label(parent, "Max size", 16.0, 246.0);
    add_visual_rect(
        parent,
        anchor,
        CLASS_DEMO_ANCHOR_REFERENCE,
        ANCHOR_ANCHOR_BRUSH,
    );
    add_visual_rect(
        parent,
        Rect::from_origin_size((anchor.x1 + 10.0, anchor.y0), desired),
        CLASS_DEMO_ANCHOR_RESOLVED,
        ANCHOR_GHOST_BRUSH,
    );
    add_frame_visuals(parent, &frame);
}

fn resolve_demo_anchor(
    anchor: Anchor<'_>,
    floating_size: Size,
    preferred: Placement,
    fallbacks: &[Placement],
    constraints: &[AnchorConstraint],
    width: f64,
    height: f64,
) -> AnchorFrame {
    let bounds = Rect::new(0.0, 0.0, width, height);
    let preferred = AnchorPositionOption::new(preferred).with_constraints(constraints);
    let fallback_options: Vec<_> = fallbacks
        .iter()
        .copied()
        .map(|placement| AnchorPositionOption::new(placement).with_constraints(constraints))
        .collect();
    let mut policy = AnchorPolicy::new(preferred, &fallback_options);
    policy.scoring = ScoringPolicy::collision_first();
    resolve_anchor(
        AnchorInput {
            anchor,
            floating_size,
            viewport: bounds,
            boundary: bounds,
            previous: None,
        },
        policy,
    )
}

fn add_frame_visuals(parent: &mut ChildUi<'_>, frame: &AnchorFrame) {
    if let Some(arrow) = frame.arrow {
        add_visual_rect(
            parent,
            arrow.rect,
            CLASS_DEMO_ANCHOR_ARROW,
            ANCHOR_ARROW_BRUSH,
        );
    }
    add_visual_rect(
        parent,
        frame.rect,
        CLASS_DEMO_ANCHOR_RESOLVED,
        ANCHOR_RESOLVED_BRUSH,
    );
}

fn add_visual_rect(
    parent: &mut ChildUi<'_>,
    rect: Rect,
    class: &'static str,
    color: (u8, u8, u8, u8),
) -> ElementId {
    parent.add(
        Panel::new()
            .class(class)
            .layout(
                Layout::new()
                    .absolute_at(rect.x0, rect.y0)
                    .logical_size(rect.width().max(1.0), rect.height().max(1.0)),
            )
            .background(solid(color.0, color.1, color.2, color.3))
            .border_brush(solid(color.0, color.1, color.2, 0xe0))
            .border_width(1.0)
            .corner_radius(4.0),
    )
}

fn add_point_marker(parent: &mut ChildUi<'_>, point: Point) -> ElementId {
    parent.add(
        Panel::new()
            .class(CLASS_DEMO_ANCHOR_REFERENCE)
            .layout(
                Layout::new()
                    .absolute_at(point.x - 4.0, point.y - 4.0)
                    .logical_size(8.0, 8.0),
            )
            .background(solid(
                ANCHOR_ANCHOR_BRUSH.0,
                ANCHOR_ANCHOR_BRUSH.1,
                ANCHOR_ANCHOR_BRUSH.2,
                0xff,
            ))
            .corner_radius(4.0),
    )
}

fn add_sample_label(parent: &mut ChildUi<'_>, label: &'static str, x: f64, y: f64) -> ElementId {
    parent.add(
        caption(label)
            .layout(Layout::new().absolute_at(x, y).logical_size(78.0, 18.0))
            .font_size(11.0),
    )
}

fn anchor_popup_handler(
    open_popup: Rc<Cell<Option<(ElementId, ElementId)>>>,
    placement: Placement,
    title: &'static str,
) -> impl FnMut(&mut Ui, overstory::ButtonActivatedEvent) + 'static {
    move |ui, event| {
        if let Some((target, popup)) = open_popup.take()
            && ui.close_popup(popup)
            && target == event.element
        {
            return;
        }

        let popup = ui.open_popup(
            event.element,
            Popup::new()
                .class(CLASS_DEMO_ANCHOR_POPUP)
                .placement(placement)
                .offset(0.0, 8.0)
                .padding(12.0, 10.0),
            |ui, popup| {
                ui.with_children(popup, |popup| {
                    popup.add(control_label(title));
                    popup.add(caption(placement_description(placement)));
                    popup.add(
                        Panel::new()
                            .layout(Layout::new().logical_size(172.0, 8.0))
                            .background(solid(0x9d, 0xf8, 0xb0, 0xcc))
                            .corner_radius(4.0),
                    );
                });
            },
        );
        if let Some(popup) = popup {
            open_popup.set(Some((event.element, popup)));
        }
    }
}

fn popup_title(label: &str, placement: Placement) -> &'static str {
    match placement.side {
        Side::Bottom if label == "Bottom edge" => "Flipped fallback",
        Side::Bottom => "Bottom popup",
        Side::Top => "Top popup",
        Side::Right => "Right popup",
        Side::Left => "Left popup",
    }
}

fn placement_description(placement: Placement) -> &'static str {
    match placement.side {
        Side::Bottom => "Preferred bottom, opposite fallback",
        Side::Top => "Preferred top, opposite fallback",
        Side::Right => "Preferred right, opposite fallback",
        Side::Left => "Preferred left, opposite fallback",
    }
}
