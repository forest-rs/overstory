use super::{LAYOUT, PAINT};
use crate::{Panel, TYPE_GENERIC, Text, Ui};
use peniko::{Brush, Color};
use understory_style::{
    ClassId, SelectorBuilder, SelectorStep, StyleBuilder, StyleCascadeBuilder, StyleOrigin,
};

#[test]
fn inherited_paint_property_write_invalidates_descendants() {
    let mut ui = Ui::new();
    let root = ui.root();
    let (panel, label) = ui.children(root, Panel::column(0.0), |children| {
        children.add(Text::new("Inherited"))
    });

    ui.invalidation.clear(PAINT);

    ui.set_local(
        panel,
        ui.properties().foreground,
        Brush::Solid(Color::from_rgba8(0x20, 0x80, 0xff, 0xff)),
    );

    assert!(ui.invalidation().is_invalidated(panel, PAINT));
    assert!(ui.invalidation().is_invalidated(label, PAINT));
}

#[test]
fn inherited_layout_property_write_invalidates_descendants() {
    let mut ui = Ui::new();
    let root = ui.root();
    let (panel, label) = ui.children(root, Panel::column(0.0), |children| {
        children.add(Text::new("Inherited"))
    });

    ui.invalidation.clear(LAYOUT);

    ui.set_local(panel, ui.properties().font_size, 22.0);

    assert!(ui.invalidation().is_invalidated(panel, LAYOUT));
    assert!(ui.invalidation().is_invalidated(label, LAYOUT));
}

#[test]
fn inherited_styled_property_change_invalidates_descendants() {
    const EMPHASIS: ClassId = ClassId(0x1A);

    let mut ui = Ui::new();
    let foreground = ui.properties().foreground;
    let style = StyleBuilder::new()
        .set(
            foreground,
            Brush::Solid(Color::from_rgba8(0xff, 0x40, 0x70, 0xff)),
        )
        .build();
    let selector =
        SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC).with_class(EMPHASIS)).build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(StyleOrigin::Base, selector, style)
        .build();
    ui.set_cascade(cascade);

    let root = ui.root();
    let (panel, label) = ui.children(root, Panel::column(0.0), |children| {
        children.add(Text::new("Inherited"))
    });

    ui.invalidation.clear(PAINT);

    ui.add_class(panel, EMPHASIS);

    assert!(ui.invalidation().is_invalidated(panel, PAINT));
    assert!(ui.invalidation().is_invalidated(label, PAINT));
}
