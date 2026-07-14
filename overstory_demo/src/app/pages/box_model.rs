use super::super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct BoxModelPage;

impl View for BoxModelPage {
    type Output = ();

    fn build(self, stage: &mut ChildUi<'_>) -> Self::Output {
        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            |row| {
                row.demo_responsive_card("CORNER CONTOURS", PAGE_CARD_WIDTH, |card| {
                    card.add(
                        body_text("Each specimen is an ordinary panel. Overstory resolves its surface longhands into presentation geometry before lowering."),
                    );
                    card.with_child(Panel::new().layout(Layout::row(12.0).wrap()), |samples| {
                        add_corner_sample(samples, "Round", CornerShapeKind::Round);
                        add_corner_sample(samples, "Squircle", CornerShapeKind::Squircle);
                        add_corner_sample(samples, "Bevel", CornerShapeKind::Bevel);
                        add_corner_sample(samples, "Scoop", CornerShapeKind::Scoop);
                    });
                });

                row.demo_responsive_card("BORDER SIDES", PAGE_CARD_WIDTH, |card| {
                    card.add(body_text(
                        "Physical sides can carry different widths and brushes. The lowerer fills border side geometry instead of stroking outside the box.",
                    ));
                    card.add(
                        asymmetric_border_specimen()
                            .layout(Layout::new().logical_size(PAGE_CONTROL_WIDTH, 104.0)),
                    );
                    card.with_child(Panel::row(8.0).layout(Layout::row(8.0).wrap()), |legend| {
                        legend.add(border_pill("top 6", 112, 143, 255));
                        legend.add(border_pill("right 12", 116, 214, 156));
                        legend.add(border_pill("bottom 8", 190, 142, 255));
                        legend.add(border_pill("left 3", 255, 205, 113));
                    });
                });
            },
        );

        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            |row| {
                row.demo_responsive_card("BOX PADDING", PAGE_CARD_WIDTH, |card| {
                    card.add(body_text(
                        "Surface padding and border widths now define the same content inset used by layout and decoration geometry.",
                    ));
                    card.with_child(
                        Panel::column(8.0)
                            .background(solid(26, 31, 43, 255))
                            .border_brush(opt_solid(126, 155, 255, 255))
                            .border_width(2.0)
                            .padding_xy(24.0, 18.0)
                            .corner_radius(18.0)
                            .layout(
                                Layout::column(8.0)
                                    .logical_size(PAGE_CONTROL_WIDTH, 132.0),
                            ),
                        |box_content| {
                            box_content.add(control_label("content box"));
                            box_content.add(
                                Text::new("This label sits inside Surface.Padding plus border thickness, matching the resolved content contour.")
                                    .class(CLASS_DEMO_BODY)
                                    .wrap(),
                            );
                        },
                    );
                });

                row.demo_responsive_card("BORDER STYLES", PAGE_CARD_WIDTH, |card| {
                    card.add(body_text(
                        "Border style is now a resolved surface longhand, so each side can use CSS-style visibility and stroke semantics.",
                    ));
                    card.with_child(Panel::new().layout(Layout::row(10.0).wrap()), |samples| {
                        add_border_style_sample(samples, "Solid", BorderStyle::Solid);
                        add_border_style_sample(samples, "Dashed", BorderStyle::Dashed);
                        add_border_style_sample(samples, "Dotted", BorderStyle::Dotted);
                        add_border_style_sample(samples, "Double", BorderStyle::Double);
                        add_border_style_sample(samples, "Groove", BorderStyle::Groove);
                        add_border_style_sample(samples, "Ridge", BorderStyle::Ridge);
                        add_border_style_sample(samples, "Inset", BorderStyle::Inset);
                        add_border_style_sample(samples, "Outset", BorderStyle::Outset);
                        add_border_style_sample(samples, "None", BorderStyle::None);
                        add_border_style_sample(samples, "Hidden", BorderStyle::Hidden);
                    });
                });
            },
        );
    }
}

#[derive(Clone, Copy, Debug)]
enum CornerShapeKind {
    Round,
    Squircle,
    Bevel,
    Scoop,
}

fn add_corner_sample(parent: &mut ChildUi<'_>, label: &str, kind: CornerShapeKind) {
    parent.with_child(
        Panel::column(6.0).layout(Layout::column(6.0).logical_size(150.0, 118.0)),
        |sample| {
            sample.add(corner_specimen(kind).layout(Layout::new().logical_size(150.0, 82.0)));
            sample.add(caption(label));
        },
    );
}

fn border_pill(label: &str, r: u8, g: u8, b: u8) -> Text {
    Text::new(label)
        .font_size(13.0)
        .foreground(Color::from_rgb8(r, g, b))
}

fn add_border_style_sample(parent: &mut ChildUi<'_>, label: &str, style: BorderStyle) {
    parent.with_child(
        Panel::column(6.0).layout(Layout::column(6.0).logical_size(96.0, 90.0)),
        |sample| {
            sample.add(border_style_specimen(style).layout(Layout::new().logical_size(96.0, 54.0)));
            sample.add(caption(label));
        },
    );
}

fn corner_specimen(kind: CornerShapeKind) -> Panel {
    Panel::new()
        .background(solid(38, 46, 65, 255))
        .border_brush(opt_solid(126, 155, 255, 255))
        .border_width(3.0)
        .padding_widths(Edges::vertical_horizontal(12.0, 18.0))
        .corner_radii(Corners::all(CornerRadius::new(28.0, 20.0)))
        .corner_shape(kind.shape())
}

fn asymmetric_border_specimen() -> Panel {
    Panel::new()
        .background(solid(30, 39, 54, 255))
        .border_brushes(
            opt_solid(112, 143, 255, 255),
            opt_solid(116, 214, 156, 255),
            opt_solid(190, 142, 255, 255),
            opt_solid(255, 205, 113, 255),
        )
        .border_widths(Edges::new(6.0, 12.0, 8.0, 3.0))
        .padding_widths(Edges::new(14.0, 22.0, 16.0, 12.0))
        .corner_radius(0.0)
        .corner_shape(CornerShape::Round)
}

fn border_style_specimen(style: BorderStyle) -> Panel {
    Panel::new()
        .background(solid(30, 39, 54, 255))
        .border_brush(opt_solid(126, 155, 255, 255))
        .border_width(7.0)
        .border_style(style)
        .corner_radius(0.0)
}

impl CornerShapeKind {
    fn shape(self) -> CornerShape {
        match self {
            Self::Round => CornerShape::Round,
            Self::Squircle => CornerShape::squircle(),
            Self::Bevel => CornerShape::Bevel,
            Self::Scoop => CornerShape::scoop(),
        }
    }
}
