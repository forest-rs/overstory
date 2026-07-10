// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn theme_variant_switches_resource_lookup_without_rebuilding_theme() {
    use understory_style::{ResourceKey, ThemeBuilder, ThemeVariant, VariantThemeBuilder};

    const TOKEN: ResourceKey = ResourceKey::new(500);
    let variants = VariantThemeBuilder::new()
        .base(ThemeBuilder::new().set(TOKEN, 1_u32).build())
        .variant(
            ThemeVariant::DARK,
            ThemeBuilder::new().set(TOKEN, 2_u32).build(),
        )
        .build();
    let mut ui = Ui::new();
    ui.set_theme(variants);

    assert_eq!(ui.theme().get::<u32>(TOKEN), Some(&1));
    ui.set_theme_variant(Some(ThemeVariant::DARK));
    assert_eq!(ui.theme().get::<u32>(TOKEN), Some(&2));
}

#[test]
fn styled_background_resolves_through_cascade() {
    // A style for `Type(TYPE_ROOT)` should supply the
    // background value for elements that don't have a local override.
    let mut ui = Ui::new();
    let root = ui.root();
    let layout_prop = ui.properties().layout;
    let background = ui.properties().surface.background;

    // Build a cascade with one rule.
    let red = Some(Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff)));
    let style = StyleBuilder::new().set(background, red).build();
    let selector = SelectorBuilder::new(SelectorStep::type_tag(TYPE_ROOT)).build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(StyleOrigin::Base, selector, style)
        .build();
    ui.set_cascade(cascade);

    ui.set_local(root, layout_prop, Layout::new().logical_size(10.0, 10.0));
    ui.compute_layout(Size::new(100.0, 100.0));

    let mut output = Scene::new();
    ui.paint(&mut output);
    // Resolution should have surfaced the styled red brush, so the scene
    // is non-empty.
    assert!(
        !output.is_empty(),
        "cascade should supply the background and produce paint output"
    );
}

#[test]
fn three_rule_cascade_picks_class_over_type() {
    // Mirror the demo: rules at Base origin keyed on TYPE_GENERIC.
    use understory_style::{ClassId, ExpressionLayer, SelectorBuilder, SelectorStep};

    const CLASS_FG: ClassId = ClassId(1);

    let mut ui = Ui::new();
    let root = ui.root();
    // Two generic children: `plain` exercises the type-only rule, `fg` the
    // class-over-type specificity. The root itself is `TYPE_ROOT`, so it is not
    // a generic styling subject.
    let plain = ui.insert_child(root);
    let fg = ui.insert_child(root);
    ui.add_class(fg, CLASS_FG);

    let dark = Some(Brush::Solid(Color::from_rgba8(0x1e, 0x1e, 0x24, 0xff)));
    let blue = Some(Brush::Solid(Color::from_rgba8(0x6b, 0x9d, 0xff, 0xff)));
    let bg_prop = ui.properties().surface.background;

    let generic = StyleBuilder::new().set(bg_prop, dark.clone()).build();
    let foreground = StyleBuilder::new().set(bg_prop, blue.clone()).build();

    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC)).build(),
            generic,
        )
        .push_rule(
            StyleOrigin::Base,
            SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC).with_class(CLASS_FG)).build(),
            foreground,
        )
        .build();
    ui.set_cascade(cascade);

    // Confirm the class survived set_cascade (which clears match_states).
    assert_eq!(
        ui.get(fg).unwrap().classes(),
        &[CLASS_FG],
        "class should survive set_cascade"
    );

    let lookup = UiResolveLookup::new(&ui);
    let expressions = ExpressionLayer::new();
    let cx = ResolveCx::new(ui.registry(), ui.theme(), &expressions, lookup);

    // Plain generic child: only the generic rule matches → dark.
    let plain_el = ui.get(plain).unwrap();
    let resolved_plain: Option<Brush> = cx.get_value(
        plain_el,
        bg_prop,
        Some((ui.cascade(), ui.match_state(plain))),
    );
    assert_eq!(
        resolved_plain, dark,
        "plain generic child should resolve to dark"
    );

    // Foreground: both rules match; the class-bearing rule has higher
    // specificity → blue.
    let fg_el = ui.get(fg).unwrap();
    let resolved_fg: Option<Brush> =
        cx.get_value(fg_el, bg_prop, Some((ui.cascade(), ui.match_state(fg))));
    assert_eq!(resolved_fg, blue, "foreground should resolve to blue");

    // The class survived set_cascade (which clears match_states).
    assert_eq!(
        ui.get(fg).unwrap().classes(),
        &[CLASS_FG],
        "class should survive set_cascade"
    );
}

#[test]
fn styled_button_foreground_reaches_template_label() {
    use crate::{Button, PART_BUTTON_LABEL, TYPE_BUTTON, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    let root = ui.root();
    let button = ui.append(root, Button::new("Run"));
    let label = ui.part(button, PART_BUTTON_LABEL).unwrap();

    let white = Brush::Solid(Color::from_rgba8(0xff, 0xff, 0xff, 0xff));
    let style = StyleBuilder::new()
        .set_resource(ui.properties().foreground, theme::ON_ACCENT_FG)
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            SelectorBuilder::new(SelectorStep::type_tag(TYPE_BUTTON)).build(),
            style,
        )
        .build();
    ui.set_cascade(cascade);

    let button_fg = ui.resolve_value_for(ui.get(button).unwrap(), ui.properties().foreground);
    assert_eq!(
        button_fg, white,
        "button itself should resolve styled foreground"
    );
    let label_fg = ui.resolve_value_for(ui.get(label).unwrap(), ui.properties().foreground);
    assert_eq!(
        label_fg, white,
        "label should inherit the button's styled foreground"
    );

    let mut output = Scene::new();
    ui.paint(&mut output);

    let label_node = ui.box_for(label).expect("label should have a box node");
    let foreground = ui
        .presentation
        .presentation(label_node)
        .expect("label should have presentation")
        .plain_text()
        .expect("label should present text")
        .foreground
        .clone();
    assert_eq!(foreground, Some(white));
}

#[test]
fn default_cascade_supplies_control_font_sizes() {
    use crate::controls::list::realized_list_item;
    use crate::{
        Button, CheckBox, ListBox, PART_BUTTON_LABEL, PART_CHECK_BOX_LABEL, SLOT_LIST_ITEM_CONTENT,
        SLOT_TEXT_INPUT_TEXT, SLOT_TOOLTIP_CONTENT, TextInput, Tooltip, theme,
    };

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    let font_size = ui.properties().font_size;

    let button = ui.append(root, Button::new("Run"));
    let button_label = ui.part(button, PART_BUTTON_LABEL).unwrap();
    let checkbox = ui.append(root, CheckBox::new("Enabled"));
    let checkbox_label = ui.part(checkbox, PART_CHECK_BOX_LABEL).unwrap();
    let listbox = ui.append(root, ListBox::new().item("Row").size(120.0, 40.0));
    ui.compute_layout(Size::new(400.0, 200.0));
    let list_item = realized_list_item(&ui, listbox, 0).unwrap();
    let list_item_label = ui.slot(list_item, SLOT_LIST_ITEM_CONTENT).unwrap();
    let text_input = ui.append(root, TextInput::new());
    let text_input_label = ui.slot(text_input, SLOT_TEXT_INPUT_TEXT).unwrap();
    let tooltip = ui.append(root, Tooltip::new("Tip"));
    let tooltip_label = ui.slot(tooltip, SLOT_TOOLTIP_CONTENT).unwrap();

    for id in [button, checkbox, listbox, list_item, text_input, tooltip] {
        assert!(
            ui.get(id).unwrap().get_local_value(font_size).is_none(),
            "default builder should leave font size to styling"
        );
    }

    assert_eq!(
        ui.resolve_value_for(ui.get(button_label).unwrap(), font_size),
        22.0
    );
    assert_eq!(
        ui.resolve_value_for(ui.get(checkbox_label).unwrap(), font_size),
        15.0
    );
    assert_eq!(
        ui.resolve_value_for(ui.get(list_item_label).unwrap(), font_size),
        14.0
    );
    assert_eq!(
        ui.resolve_value_for(ui.get(text_input_label).unwrap(), font_size),
        15.0
    );
    assert_eq!(
        ui.resolve_value_for(ui.get(tooltip_label).unwrap(), font_size),
        13.0
    );

    let explicit = ui.append(root, Button::new("Small").font_size(12.0));
    assert_eq!(
        ui.get(explicit)
            .unwrap()
            .get_local_value(font_size)
            .copied(),
        Some(12.0)
    );
}

#[test]
fn text_measure_uses_styled_text_and_font_size() {
    use crate::{TYPE_TEXT, Text};
    use alloc::string::String;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_local(
        root,
        ui.properties().layout,
        Layout::flex().logical_size(200.0, 100.0),
    );
    let text = ui.append(root, Text::new(""));

    let style = StyleBuilder::new()
        .set(ui.properties().text, String::from("Styled text"))
        .set(ui.properties().font_size, 40.0_f64)
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            SelectorBuilder::new(SelectorStep::type_tag(TYPE_TEXT)).build(),
            style,
        )
        .build();
    ui.set_cascade(cascade);

    ui.compute_layout(Size::new(200.0, 100.0));

    let bounds = ui.effective_bounds(text);
    assert!(
        bounds.width() > 0.0 && bounds.height() > 30.0,
        "styled text and font size should participate in measure; got {bounds:?}"
    );
}

#[test]
fn text_measure_uses_inherited_styled_font_size() {
    use crate::Text;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_local(
        root,
        ui.properties().layout,
        Layout::flex().logical_size(200.0, 100.0),
    );
    let text = ui.append(root, Text::new("Inherited font"));

    let style = StyleBuilder::new()
        .set(ui.properties().font_size, 40.0_f64)
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC)).build(),
            style,
        )
        .build();
    ui.set_cascade(cascade);

    ui.compute_layout(Size::new(200.0, 100.0));

    let bounds = ui.effective_bounds(text);
    assert!(
        bounds.height() > 30.0,
        "inherited styled font size should participate in measure; got {bounds:?}"
    );
}

#[test]
fn repeated_text_widgets_share_shaped_layout_cache() {
    use crate::Text;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(4.0).logical_size(240.0, 120.0));

    ui.append(root, Text::new("Shared label").font_size(14.0));
    ui.append(root, Text::new("Shared label").font_size(14.0));
    ui.append(root, Text::new("Shared label").font_size(14.0));

    ui.compute_layout(Size::new(240.0, 120.0));

    let stats = ui.text_layout_cache_stats();
    assert_eq!(stats.misses, 1);
    assert!(
        stats.hits >= 2,
        "expected repeated labels to hit cache: {stats:?}"
    );
    assert_eq!(ui.text_layout_cache_len(), 1);
}

#[test]
fn text_foreground_repaint_reuses_shaped_layout() {
    use crate::Text;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(4.0).logical_size(240.0, 120.0));
    let text = ui.append(root, Text::new("Shared label").font_size(14.0));

    ui.compute_layout(Size::new(240.0, 120.0));
    let mut first = Scene::new();
    ui.paint(&mut first);
    let before = ui.text_layout_cache_stats();

    ui.set_local(
        text,
        ui.properties().foreground,
        Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff)),
    );
    assert!(!ui.invalidation().has_invalidated(LAYOUT));
    assert!(ui.invalidation().has_invalidated(PAINT));

    let mut second = Scene::new();
    ui.paint(&mut second);
    let after = ui.text_layout_cache_stats();

    assert_eq!(after.misses, before.misses);
    assert!(after.hits > before.hits);
}

#[test]
fn paint_keeps_default_text_on_one_line() {
    use crate::Text;
    use imaging::record::{Command, Draw};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_local(
        root,
        ui.properties().layout,
        Layout::flex().logical_size(80.0, 200.0),
    );
    let text = ui.append(
        root,
        Text::new("alpha beta gamma delta epsilon").font_size(20.0),
    );
    ui.set_layout(text, Layout::new().logical_size(80.0, 200.0));

    ui.compute_layout(Size::new(80.0, 200.0));

    let mut output = Scene::new();
    ui.paint(&mut output);

    let mut saw_glyph_run = false;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for command in output.commands() {
        let Command::Draw(id) = command else {
            continue;
        };
        let Draw::GlyphRun(run) = output.draw_op(*id) else {
            continue;
        };
        saw_glyph_run = true;
        assert!(run.hint, "UI text glyph runs should enable hinting");
        for glyph in &run.glyphs {
            min_y = min_y.min(glyph.y);
            max_y = max_y.max(glyph.y);
        }
    }

    assert!(saw_glyph_run, "text should paint at least one glyph run");
    assert!(
        max_y - min_y < 20.0,
        "default text should paint on one line; glyph y span was {}",
        max_y - min_y
    );
}

#[test]
fn paint_wraps_opted_in_text_to_element_bounds() {
    use crate::Text;
    use imaging::record::{Command, Draw};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_local(
        root,
        ui.properties().layout,
        Layout::flex().logical_size(80.0, 200.0),
    );
    let text = ui.append(
        root,
        Text::new("alpha beta gamma delta epsilon")
            .font_size(20.0)
            .wrap(),
    );
    ui.set_layout(text, Layout::new().logical_size(80.0, 200.0));

    ui.compute_layout(Size::new(80.0, 200.0));

    let bounds = ui.effective_bounds(text);
    assert!(
        bounds.width() <= 80.0,
        "text should be laid out within the constrained width; got {bounds:?}"
    );

    let mut output = Scene::new();
    ui.paint(&mut output);

    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for command in output.commands() {
        let Command::Draw(id) = command else {
            continue;
        };
        let Draw::GlyphRun(run) = output.draw_op(*id) else {
            continue;
        };
        for glyph in &run.glyphs {
            min_y = min_y.min(glyph.y);
            max_y = max_y.max(glyph.y);
        }
    }

    assert!(
        max_y - min_y > 20.0,
        "paint should render wrapped text lines; glyph y span was {}",
        max_y - min_y
    );
}

#[test]
fn row_text_uses_natural_width_until_width_is_chosen() {
    use crate::{Panel, Text};
    use imaging::record::{Command, Draw};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::row(12.0).logical_size(284.0, 80.0));

    let spacer = ui.append(root, Panel::new());
    ui.set_layout(spacer, Layout::new().logical_size(64.0, 32.0));
    let label = ui.append(root, Text::new("Live rendering").font_size(15.0));

    ui.compute_layout(Size::new(284.0, 80.0));

    let label_bounds = ui.effective_bounds(label);
    assert!(
        label_bounds.width() > 75.0,
        "row label should keep its natural width instead of measuring wrapped; got {label_bounds:?}"
    );

    let mut output = Scene::new();
    ui.paint(&mut output);

    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for command in output.commands() {
        let Command::Draw(id) = command else {
            continue;
        };
        let Draw::GlyphRun(run) = output.draw_op(*id) else {
            continue;
        };
        for glyph in &run.glyphs {
            min_y = min_y.min(glyph.y);
            max_y = max_y.max(glyph.y);
        }
    }

    assert!(
        max_y - min_y < 20.0,
        "row label should paint on one line; glyph y span was {}",
        max_y - min_y
    );
}

#[test]
fn control_row_labels_keep_single_line_widths() {
    use crate::{Spinner, Text, ToggleSwitch, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    let root = ui.root();
    ui.set_layout(root, Layout::column(14.0).logical_size(320.0, 350.0));
    ui.set_box_padding(root, understory_presentation::Edges::all(18.0));

    let toggle_row = ui.append(root, crate::Panel::row(12.0).align_center());
    ui.append(toggle_row, ToggleSwitch::new().checked(true));
    let toggle_label = ui.append(toggle_row, Text::new("Live rendering").font_size(15.0));

    let spinner_row = ui.append(root, crate::Panel::row(12.0).align_center());
    ui.append(spinner_row, Spinner::new());
    let spinner_label = ui.append(spinner_row, Text::new("Streaming updates").font_size(13.0));

    ui.compute_layout(Size::new(320.0, 350.0));

    let toggle_style = understory_presentation::TextStyle {
        font_size: 15.0,
        ..understory_presentation::TextStyle::default()
    };
    let spinner_style = understory_presentation::TextStyle {
        font_size: 13.0,
        ..understory_presentation::TextStyle::default()
    };
    let toggle_natural_width = f64::from(
        ui.text
            .borrow_mut()
            .shape("Live rendering", &toggle_style, None)
            .width(),
    );
    let spinner_natural_width = f64::from(
        ui.text
            .borrow_mut()
            .shape("Streaming updates", &spinner_style, None)
            .width(),
    );

    let toggle_bounds = ui.effective_bounds(toggle_label);
    let spinner_bounds = ui.effective_bounds(spinner_label);
    assert!(
        toggle_bounds.width() >= toggle_natural_width - 1.0,
        "toggle label should retain natural width; natural={toggle_natural_width}, bounds={toggle_bounds:?}"
    );
    assert!(
        spinner_bounds.width() >= spinner_natural_width - 1.0,
        "spinner label should retain natural width; natural={spinner_natural_width}, bounds={spinner_bounds:?}"
    );

    let mut output = Scene::new();
    ui.paint(&mut output);

    let mut max_span = 0.0_f32;
    for command in output.commands() {
        let Command::Draw(id) = command else {
            continue;
        };
        let imaging::record::Draw::GlyphRun(run) = output.draw_op(*id) else {
            continue;
        };
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for glyph in &run.glyphs {
            min_y = min_y.min(glyph.y);
            max_y = max_y.max(glyph.y);
        }
        if min_y.is_finite() && max_y.is_finite() {
            max_span = max_span.max(max_y - min_y);
        }
    }
    assert!(
        max_span < 20.0,
        "control row labels should paint on one line; max glyph y span was {max_span}"
    );
}

#[test]
fn default_cascade_styles_root_surface_and_text_foreground() {
    use crate::{Text, theme};

    // The default cascade gives the root the themed window background and seeds
    // the inherited default foreground; a bare text element should pick up that
    // foreground without any per-element styling. Values are the dark theme's
    // `SURFACE_BG` / `ON_SURFACE_FG`.
    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    let root = ui.root();
    let text = ui.append(root, Text::new("hello"));

    let surface_bg = Some(Brush::Solid(Color::from_rgba8(0x1e, 0x1e, 0x24, 0xff)));
    let on_surface_fg = Brush::Solid(Color::from_rgba8(0xf4, 0xf4, 0xf7, 0xff));

    let background = ui.properties().surface.background;
    let foreground = ui.properties().foreground;

    assert_eq!(
        ui.resolve_value_for(ui.get(root).unwrap(), background),
        surface_bg,
        "root should resolve the themed surface background by default"
    );
    assert_eq!(
        ui.resolve_value_for(ui.get(text).unwrap(), foreground),
        on_surface_fg,
        "a bare text element should inherit the default on-surface foreground"
    );
}

#[test]
fn text_style_resolution_carries_font_feature_settings() {
    let mut ui = Ui::new();
    let root = ui.root();
    let text = ui.append(root, crate::Text::new("Variable"));
    let variations: Box<[parley::FontVariation]> = Box::from([parley::FontVariation::new(
        parley::setting::Tag::new(b"opsz"),
        12.0,
    )]);
    let features: Box<[parley::FontFeature]> = Box::from([parley::FontFeature::new(
        parley::setting::Tag::new(b"kern"),
        0,
    )]);

    ui.set_local(text, ui.properties().font_variations, variations.clone());
    ui.set_local(text, ui.properties().font_features, features.clone());

    let element = ui.get(text).unwrap();
    let style = crate::controls::text::text_style(element, &ui);

    assert_eq!(style.font_variations.as_ref(), variations.as_ref());
    assert_eq!(style.font_features.as_ref(), features.as_ref());
}
