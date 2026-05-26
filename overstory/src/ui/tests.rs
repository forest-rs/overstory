// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use crate::channels::{COMPOSE, LAYOUT, PAINT};
use crate::pseudos::{
    PSEUDO_DISABLED, PSEUDO_FOCUS_VISIBLE, PSEUDO_FOCUSED, PSEUDO_HOVER, PSEUDO_PRESSED,
};
use crate::test_support::{pointer_down, pointer_leave, pointer_move, pointer_scroll, pointer_up};
use crate::{
    CompositionPolicy, FocusChangeReason, KeyDispatchResult, Layout, Navigation, Phase,
    PointerEventKind, RoutedEventKind, RoutedHandlerOptions, TYPE_GENERIC, TemplateContext,
    TemplateSlot, UiResolveLookup,
};
use alloc::string::String;
use imaging::record::Scene;
use kurbo::{Point, Rect, Size, Vec2};
use peniko::{Brush, Color};
use understory_property::{DependencyObject, DependencyObjectExt};
use understory_style::{
    ClassId, PartTag, ResolveCx, SelectorBuilder, SelectorStep, StyleBuilder, StyleCascadeBuilder,
    StyleOrigin,
};

mod events;

#[test]
fn new_ui_has_root() {
    let ui = Ui::new();
    let root = ui.root();
    assert!(ui.get(root).is_some());
    assert_eq!(ui.len(), 1);
    assert_eq!(ui.get(root).unwrap().parent(), None);
    assert!(ui.get(root).unwrap().children().is_empty());
}

#[test]
fn insert_child_appends_to_parent_and_marks_dirty() {
    let mut ui = Ui::new();
    let root = ui.root();
    let a = ui.insert_child(root);
    let b = ui.insert_child(root);

    assert_eq!(ui.len(), 3);
    let root_children = ui.get(root).unwrap().children();
    assert_eq!(root_children, &[a, b]);
    assert_eq!(ui.get(a).unwrap().parent(), Some(root));
    assert_eq!(ui.get(b).unwrap().parent(), Some(root));

    assert!(ui.invalidation().is_invalidated(a, PAINT));
    assert!(ui.invalidation().is_invalidated(b, PAINT));
    assert!(ui.invalidation().is_invalidated(root, COMPOSE));
}

#[test]
fn remove_drops_subtree_and_invalidates_handles() {
    let mut ui = Ui::new();
    let root = ui.root();
    let a = ui.insert_child(root);
    let grandchild = ui.insert_child(a);
    let b = ui.insert_child(root);

    ui.remove(a);

    assert!(ui.get(a).is_none());
    assert!(ui.get(grandchild).is_none());
    assert!(ui.get(b).is_some());
    assert_eq!(ui.get(root).unwrap().children(), &[b]);
    assert_eq!(ui.len(), 2);
}

#[test]
fn slot_reuse_invalidates_old_handle() {
    let mut ui = Ui::new();
    let root = ui.root();
    let a = ui.insert_child(root);
    ui.remove(a);

    let c = ui.insert_child(root);
    assert_eq!(c.slot(), a.slot());
    assert_ne!(c.generation(), a.generation());
    assert!(ui.get(a).is_none());
    assert!(ui.get(c).is_some());
}

#[test]
#[should_panic(expected = "cannot remove the tree root")]
fn remove_root_panics() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.remove(root);
}

#[test]
#[should_panic(expected = "parent id is stale")]
fn insert_child_with_stale_parent_panics() {
    let mut ui = Ui::new();
    let root = ui.root();
    let a = ui.insert_child(root);
    ui.remove(a);
    let _ = ui.insert_child(a);
}

#[test]
fn set_local_marks_paint_dirty() {
    let mut ui = Ui::new();
    let root = ui.root();
    let bounds = ui.properties().bounds;
    let background = ui.properties().background;

    let _ = ui
        .invalidation
        .drain_sorted(PAINT)
        .collect::<Vec<ElementId>>();

    ui.set_local(root, bounds, Rect::new(0.0, 0.0, 100.0, 50.0));
    assert!(ui.invalidation().is_invalidated(root, PAINT));

    ui.invalidation.clear(PAINT);
    ui.set_local(root, bounds, Rect::new(0.0, 0.0, 100.0, 50.0));
    assert!(!ui.invalidation().is_invalidated(root, PAINT));

    ui.set_local(
        root,
        background,
        Some(Brush::Solid(Color::from_rgba8(0, 0, 0, 0xff))),
    );
    assert!(ui.invalidation().is_invalidated(root, PAINT));
}

#[test]
fn paint_drains_dirty_and_emits_output() {
    let mut ui = Ui::new();
    let root = ui.root();
    let layout_prop = ui.properties().layout;
    let background = ui.properties().background;

    ui.set_local(root, layout_prop, Layout::new().logical_size(10.0, 10.0));
    ui.set_local(
        root,
        background,
        Some(Brush::Solid(Color::from_rgba8(0xff, 0, 0, 0xff))),
    );
    ui.compute_layout(Size::new(100.0, 100.0));

    let mut output = Scene::new();
    ui.paint(&mut output);

    assert!(!ui.invalidation().has_invalidated(PAINT));
    assert!(!output.is_empty());
}

#[test]
fn paint_skips_zero_area_elements() {
    let mut ui = Ui::new();
    let root = ui.root();
    let background = ui.properties().background;

    ui.set_local(
        root,
        background,
        Some(Brush::Solid(Color::from_rgba8(0xff, 0, 0, 0xff))),
    );

    let mut output = Scene::new();
    ui.paint(&mut output);
    assert!(output.is_empty(), "zero-area element should record nothing");
}

#[test]
fn styled_background_resolves_through_cascade() {
    // A style for `Type(TYPE_GENERIC)` should supply the
    // background value for elements that don't have a local override.
    let mut ui = Ui::new();
    let root = ui.root();
    let layout_prop = ui.properties().layout;
    let background = ui.properties().background;

    // Build a cascade with one rule.
    let red = Some(Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff)));
    let style = StyleBuilder::new().set(background, red).build();
    let selector = SelectorBuilder::new(SelectorStep::type_tag(TYPE_GENERIC)).build();
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
    use understory_style::{ClassId, SelectorBuilder, SelectorStep};

    const CLASS_FG: ClassId = ClassId(1);

    let mut ui = Ui::new();
    let root = ui.root();
    let fg = ui.insert_child(root);
    ui.add_class(fg, CLASS_FG);

    let dark = Some(Brush::Solid(Color::from_rgba8(0x1e, 0x1e, 0x24, 0xff)));
    let blue = Some(Brush::Solid(Color::from_rgba8(0x6b, 0x9d, 0xff, 0xff)));
    let bg_prop = ui.properties().background;

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
    let cx = ResolveCx::new(ui.registry(), ui.theme(), lookup);

    // Root: only the generic rule matches → dark.
    let root_el = ui.get(root).unwrap();
    let resolved_root: Option<Brush> =
        cx.get_value(root_el, bg_prop, Some((ui.cascade(), ui.match_state(root))));
    assert_eq!(resolved_root, dark, "root should resolve to dark");

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

    let button_fg = ui.effective_value_for(ui.get(button).unwrap(), ui.properties().foreground);
    assert_eq!(
        button_fg, white,
        "button itself should resolve styled foreground"
    );
    let label_fg = ui.effective_value_for(ui.get(label).unwrap(), ui.properties().foreground);
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
        ui.effective_value_for(ui.get(button_label).unwrap(), font_size),
        22.0
    );
    assert_eq!(
        ui.effective_value_for(ui.get(checkbox_label).unwrap(), font_size),
        15.0
    );
    assert_eq!(
        ui.effective_value_for(ui.get(list_item_label).unwrap(), font_size),
        14.0
    );
    assert_eq!(
        ui.effective_value_for(ui.get(text_input_label).unwrap(), font_size),
        15.0
    );
    assert_eq!(
        ui.effective_value_for(ui.get(tooltip_label).unwrap(), font_size),
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
fn text_measure_uses_styled_content_and_font_size() {
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
        .set(ui.properties().content, String::from("Styled text"))
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
        "styled text content and font size should participate in measure; got {bounds:?}"
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
        label_bounds.width() > 90.0,
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
    ui.set_layout(
        root,
        Layout::column(14.0)
            .logical_size(320.0, 350.0)
            .padding_all(18.0),
    );

    let toggle_row = ui.append(root, crate::Panel::row(12.0).align_center());
    ui.append(toggle_row, ToggleSwitch::new().checked(true));
    let toggle_label = ui.append(toggle_row, Text::new("Live rendering").font_size(15.0));

    let spinner_row = ui.append(root, crate::Panel::row(12.0).align_center());
    ui.append(spinner_row, Spinner::new());
    let spinner_label = ui.append(spinner_row, Text::new("Streaming updates").font_size(13.0));

    ui.compute_layout(Size::new(320.0, 350.0));

    let toggle_natural_width = f64::from(
        ui.text
            .borrow_mut()
            .shape("Live rendering", 15.0, None)
            .width(),
    );
    let spinner_natural_width = f64::from(
        ui.text
            .borrow_mut()
            .shape("Streaming updates", 13.0, None)
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
        let imaging::record::Command::Draw(id) = command else {
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
fn slider_initial_value_positions_thumb() {
    use crate::Slider;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_local(
        root,
        ui.properties().layout,
        Layout::flex().logical_size(300.0, 80.0),
    );

    let slider = ui.append(root, Slider::new().value(0.4));
    let thumb = ui.part(slider, crate::PART_SLIDER_THUMB).unwrap();

    ui.compute_layout(Size::new(300.0, 80.0));

    let slider_bounds = ui.effective_bounds(slider);
    let thumb_bounds = ui.effective_bounds(thumb);
    let thumb_offset = thumb_bounds.x0 - slider_bounds.x0;
    assert!(
        thumb_offset > 70.0,
        "initial value should move thumb along the track; got x offset {thumb_offset}"
    );
}

#[test]
fn slider_snap_value_change_updates_layer_without_waiting_for_second_layout() {
    use crate::{SLOT_SLIDER_THUMB, Slider, set_slider_value};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(300.0, 80.0));
    let slider = ui.append(root, Slider::new().value(0.0));
    ui.compute_layout(Size::new(300.0, 80.0));
    let thumb = ui.slot(slider, SLOT_SLIDER_THUMB).unwrap();

    set_slider_value(&mut ui, slider, 1.0);
    ui.compute_layout(Size::new(300.0, 80.0));

    assert_close(layer_x(&ui, thumb), ui.effective_bounds(thumb).x0);
    assert_close(layer_y(&ui, thumb), ui.effective_bounds(thumb).y0);
}

#[test]
fn default_templates_label_named_parts() {
    use crate::{
        Button, PART_BUTTON_LABEL, PART_SLIDER_THUMB, PART_SLIDER_TRACK, PART_TOGGLE_THUMB,
        SLOT_BUTTON_CONTENT, SLOT_SLIDER_THUMB, SLOT_SLIDER_TRACK, SLOT_TOGGLE_THUMB, Slider,
        ToggleSwitch,
    };

    let mut ui = Ui::new();
    let root = ui.root();

    let button = ui.append(root, Button::new("Run"));
    let button_label = ui.part(button, PART_BUTTON_LABEL).unwrap();
    assert_eq!(
        ui.get(button_label).unwrap().part_tag(),
        Some(PART_BUTTON_LABEL)
    );
    assert_eq!(
        ui.template_instance(button)
            .unwrap()
            .part(PART_BUTTON_LABEL),
        Some(button_label)
    );
    assert_eq!(ui.slot(button, SLOT_BUTTON_CONTENT), Some(button_label));

    let slider = ui.append(root, Slider::new());
    let slider_track = ui.part(slider, PART_SLIDER_TRACK).unwrap();
    let slider_thumb = ui.part(slider, PART_SLIDER_THUMB).unwrap();
    assert_eq!(ui.slot(slider, SLOT_SLIDER_TRACK), Some(slider_track));
    assert_eq!(ui.slot(slider, SLOT_SLIDER_THUMB), Some(slider_thumb));
    assert_eq!(
        ui.get(slider_track).unwrap().part_tag(),
        Some(PART_SLIDER_TRACK)
    );
    assert_eq!(
        ui.get(slider_thumb).unwrap().part_tag(),
        Some(PART_SLIDER_THUMB)
    );

    let toggle = ui.append(root, ToggleSwitch::new());
    let toggle_thumb = ui.part(toggle, PART_TOGGLE_THUMB).unwrap();
    assert_eq!(ui.slot(toggle, SLOT_TOGGLE_THUMB), Some(toggle_thumb));
    assert_eq!(
        ui.get(toggle_thumb).unwrap().part_tag(),
        Some(PART_TOGGLE_THUMB)
    );
}

#[test]
fn toggle_thumb_transition_samples_composition_offset() {
    use crate::SLOT_TOGGLE_THUMB;

    let (mut ui, toggle, thumb) = animated_toggle_fixture();
    let start_x = layer_x(&ui, thumb);
    let start_y = layer_y(&ui, thumb);

    ui.set_now(0);
    ui.synthesize_click(toggle, ui.effective_bounds(toggle).center());
    ui.compute_layout(Size::new(200.0, 80.0));

    let target_x = ui.effective_bounds(thumb).x0;
    assert!(
        target_x > start_x,
        "checked layout target should move thumb to the right"
    );
    assert_close(layer_x(&ui, thumb), start_x);
    assert_close(layer_y(&ui, thumb), start_y);

    ui.set_now(60_000_000);
    ui.compute_layout(Size::new(200.0, 80.0));
    assert_close(layer_x(&ui, thumb), (start_x + target_x) * 0.5);
    assert_close(layer_y(&ui, thumb), start_y);

    ui.set_now(120_000_000);
    ui.compute_layout(Size::new(200.0, 80.0));
    assert_close(layer_x(&ui, thumb), target_x);
    assert_close(layer_y(&ui, thumb), start_y);

    assert_eq!(ui.slot(toggle, SLOT_TOGGLE_THUMB), Some(thumb));
}

#[test]
fn toggle_thumb_transition_retargets_from_sampled_offset() {
    let (mut ui, toggle, thumb) = animated_toggle_fixture();
    let start_x = layer_x(&ui, thumb);

    ui.set_now(0);
    ui.synthesize_click(toggle, ui.effective_bounds(toggle).center());
    ui.compute_layout(Size::new(200.0, 80.0));
    let checked_target = ui.effective_bounds(thumb).x0;

    ui.set_now(60_000_000);
    ui.compute_layout(Size::new(200.0, 80.0));
    let midpoint = layer_x(&ui, thumb);
    assert_close(midpoint, (start_x + checked_target) * 0.5);

    ui.synthesize_click(toggle, ui.effective_bounds(toggle).center());
    ui.compute_layout(Size::new(200.0, 80.0));
    assert_close(layer_x(&ui, thumb), midpoint);

    ui.set_now(120_000_000);
    ui.compute_layout(Size::new(200.0, 80.0));
    assert_close(layer_x(&ui, thumb), (midpoint + start_x) * 0.5);

    ui.set_now(180_000_000);
    ui.compute_layout(Size::new(200.0, 80.0));
    assert_close(layer_x(&ui, thumb), start_x);
}

#[test]
fn button_hover_transitions_background_color() {
    use crate::{Button, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(240.0, 80.0));
    let button = ui.append(root, Button::new("Hover"));
    ui.compute_layout(Size::new(240.0, 80.0));

    let idle = Color::from_rgba8(0x3a, 0x69, 0xd6, 0xff);
    let hover = Color::from_rgba8(0x4d, 0x81, 0xf2, 0xff);
    ui.set_now(0);
    let _ = ui.dispatch_pointer_event(&pointer_move(
        ui.effective_bounds(button).center(),
        0,
        false,
    ));
    ui.compute_layout(Size::new(240.0, 80.0));

    let mut scene = Scene::new();
    ui.paint(&mut scene);
    assert_eq!(element_background_color(&ui, button), Some(idle));

    ui.set_now(45_000_000);
    ui.compute_layout(Size::new(240.0, 80.0));
    ui.paint(&mut scene);
    assert_eq!(
        element_background_color(&ui, button),
        Some(idle.lerp_rect(hover, 0.5))
    );

    ui.set_now(90_000_000);
    ui.compute_layout(Size::new(240.0, 80.0));
    ui.paint(&mut scene);
    assert_eq!(element_background_color(&ui, button), Some(hover));
}

#[test]
fn local_background_change_after_transition_paints_new_target() {
    use crate::{Button, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(240.0, 80.0));
    let button = ui.append(root, Button::new("Hover"));
    ui.compute_layout(Size::new(240.0, 80.0));

    ui.set_now(0);
    let _ = ui.dispatch_pointer_event(&pointer_move(
        ui.effective_bounds(button).center(),
        0,
        false,
    ));
    ui.set_now(90_000_000);
    let mut scene = Scene::new();
    ui.paint(&mut scene);

    let red = Color::from_rgba8(0xff, 0x00, 0x00, 0xff);
    ui.set_local(button, ui.properties().background, Some(Brush::Solid(red)));
    ui.paint(&mut scene);

    assert_eq!(element_background_color(&ui, button), Some(red));
}

#[test]
fn button_press_transitions_layer_opacity() {
    use crate::{Button, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(240.0, 80.0));
    let button = ui.append(root, Button::new("Press"));
    ui.compute_layout(Size::new(240.0, 80.0));

    let center = ui.effective_bounds(button).center();
    let _ = ui.dispatch_pointer_event(&pointer_down(center, 0));
    ui.compute_layout(Size::new(240.0, 80.0));
    assert_close(layer_opacity(&ui, button), 1.0);

    ui.set_now(35_000_000);
    ui.compute_layout(Size::new(240.0, 80.0));
    assert_close(layer_opacity(&ui, button), 0.96);

    ui.set_now(70_000_000);
    ui.compute_layout(Size::new(240.0, 80.0));
    assert_close(layer_opacity(&ui, button), 0.92);
}

#[test]
fn slider_keyboard_transitions_thumb_offset_but_drag_snaps() {
    use crate::{SLOT_SLIDER_THUMB, Slider};
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(300.0, 80.0));
    let slider = ui.append(root, Slider::new().value(0.0).small_change(0.5));
    assert!(ui.set_focus(Some(slider)));
    ui.compute_layout(Size::new(300.0, 80.0));
    let thumb = ui.slot(slider, SLOT_SLIDER_THUMB).unwrap();
    let start_x = layer_x(&ui, thumb);
    let start_y = layer_y(&ui, thumb);

    ui.set_now(0);
    let right = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
    assert!(ui.dispatch_key(&right).handled);
    ui.compute_layout(Size::new(300.0, 80.0));
    let target_x = ui.effective_bounds(thumb).x0;
    assert!(target_x > start_x);
    assert_close(layer_x(&ui, thumb), start_x);
    assert_close(layer_y(&ui, thumb), start_y);

    ui.set_now(45_000_000);
    ui.compute_layout(Size::new(300.0, 80.0));
    assert_close(layer_x(&ui, thumb), (start_x + target_x) * 0.5);
    assert_close(layer_y(&ui, thumb), start_y);

    let drag_target = Point::new(target_x + 30.0, ui.effective_bounds(thumb).center().y);
    let _ = ui.dispatch_pointer_event(&pointer_down(ui.effective_bounds(thumb).center(), 100));
    let _ = ui.dispatch_pointer_event(&pointer_move(drag_target, 101, true));
    ui.compute_layout(Size::new(300.0, 80.0));
    assert_close(layer_x(&ui, thumb), ui.effective_bounds(thumb).x0);
    assert_close(layer_y(&ui, thumb), start_y);
}

fn animated_toggle_fixture() -> (Ui, ElementId, ElementId) {
    use crate::{SLOT_TOGGLE_THUMB, ToggleSwitch, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(200.0, 80.0));
    let toggle = ui.append(root, ToggleSwitch::new());
    ui.compute_layout(Size::new(200.0, 80.0));
    let thumb = ui
        .slot(toggle, SLOT_TOGGLE_THUMB)
        .expect("toggle template should create a thumb slot");
    (ui, toggle, thumb)
}

fn layer_x(ui: &Ui, thumb: ElementId) -> f64 {
    layer_origin(ui, thumb).x
}

fn layer_y(ui: &Ui, thumb: ElementId) -> f64 {
    layer_origin(ui, thumb).y
}

fn layer_origin(ui: &Ui, thumb: ElementId) -> Point {
    let layer = ui
        .composition
        .layer_for(thumb)
        .expect("thumb should have a composition layer");
    ui.composition_layers()
        .world_transform(layer)
        .transform_point(Point::ZERO)
        .expect("thumb layer transform should be affine")
}

fn layer_opacity(ui: &Ui, id: ElementId) -> f64 {
    let layer = ui
        .composition
        .layer_for(id)
        .expect("element should have a composition layer");
    f64::from(ui.composition_layers().local_opacity(layer))
}

fn element_background_color(ui: &Ui, id: ElementId) -> Option<Color> {
    let node = ui.box_for(id)?;
    let surface = ui.presentation.presentation(node)?.surface()?;
    let brush = &surface.backgrounds.first()?.brush;
    match brush {
        Brush::Solid(color) => Some(*color),
        _ => None,
    }
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.001,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn template_overrides_replace_visuals_not_control_behavior() {
    use crate::widget::SurfacePartWidget;
    use crate::{
        Button, SLOT_BUTTON_CONTENT, SLOT_SLIDER_THUMB, SLOT_TOGGLE_THUMB, Slider, Template,
        TemplateChild, TextWidget, ToggleSwitch, set_slider_value,
    };

    const OUTER_FRAME: PartTag = PartTag(0x40);
    const INNER_FRAME: PartTag = PartTag(0x41);

    struct DoubleFrameButtonTemplate;
    impl Template for DoubleFrameButtonTemplate {
        fn instantiate(&self, cx: &mut TemplateContext<'_>) {
            let props = *cx.properties();
            let outer = cx.insert(TemplateChild::part(
                OUTER_FRAME,
                Box::new(SurfacePartWidget),
            ));
            let inner = cx.insert_under(
                outer,
                TemplateChild::part(INNER_FRAME, Box::new(SurfacePartWidget)),
            );
            let content = cx.insert_under(
                inner,
                TemplateChild::slot(SLOT_BUTTON_CONTENT, Box::new(TextWidget)),
            );
            cx.bind_owner_to_part(props.content, content, props.content)
                .expect("content binding should install");
            cx.bind_owner_to_part(props.font_size, content, props.font_size)
                .expect("font size binding should install");
        }
    }

    struct SingleSlotTemplate(TemplateSlot);
    impl Template for SingleSlotTemplate {
        fn instantiate(&self, cx: &mut TemplateContext<'_>) {
            let _part = cx.insert(TemplateChild::slot(self.0, Box::new(SurfacePartWidget)));
        }
    }

    static BUTTON_TEMPLATE: DoubleFrameButtonTemplate = DoubleFrameButtonTemplate;
    static TOGGLE_TEMPLATE: SingleSlotTemplate = SingleSlotTemplate(SLOT_TOGGLE_THUMB);
    static SLIDER_TEMPLATE: SingleSlotTemplate = SingleSlotTemplate(SLOT_SLIDER_THUMB);

    let mut ui = Ui::new();
    let root = ui.root();

    let button = ui.append(root, Button::new("Run").template(&BUTTON_TEMPLATE));
    let outer = ui.part(button, OUTER_FRAME).unwrap();
    let inner = ui.part(button, INNER_FRAME).unwrap();
    let content = ui.slot(button, SLOT_BUTTON_CONTENT).unwrap();
    assert_eq!(ui.get(outer).unwrap().parent(), Some(button));
    assert_eq!(ui.get(inner).unwrap().parent(), Some(outer));
    assert_eq!(ui.get(content).unwrap().parent(), Some(inner));
    ui.flush_bindings();
    assert_eq!(
        ui.get(content)
            .unwrap()
            .property_store()
            .effective_local_erased(ui.properties().content.id())
            .and_then(|value| value.downcast_ref::<String>())
            .map(String::as_str),
        Some("Run")
    );

    let toggle = ui.append(root, ToggleSwitch::new().template(&TOGGLE_TEMPLATE));
    ui.synthesize_click(toggle, Point::ZERO);
    let checked = ui
        .get(toggle)
        .and_then(|el| el.get_local_value(ui.properties().is_checked).copied())
        .unwrap_or(false);
    assert!(
        checked,
        "custom toggle template should keep toggle behavior"
    );

    let slider = ui.append(root, Slider::new().template(&SLIDER_TEMPLATE).value(0.75));
    assert!(ui.slot(slider, SLOT_SLIDER_THUMB).is_some());
    assert_eq!(
        ui.get(slider)
            .unwrap()
            .get_local_value(ui.properties().value)
            .copied(),
        Some(0.75)
    );
    set_slider_value(&mut ui, slider, 0.25);
    assert_eq!(
        ui.get(slider)
            .unwrap()
            .get_local_value(ui.properties().value)
            .copied(),
        Some(0.25)
    );
}

#[test]
fn surface_template_parts_do_not_match_generic_type_selector() {
    use crate::Slider;

    let mut ui = Ui::new();
    let root = ui.root();
    let slider = ui.append(root, Slider::new());
    let track = ui.part(slider, crate::PART_SLIDER_TRACK).unwrap();

    let red = Some(Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff)));
    let style = StyleBuilder::new()
        .set(ui.properties().background, red)
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            crate::selectors::type_tag(TYPE_GENERIC),
            style,
        )
        .build();
    ui.set_cascade(cascade);

    let background = ui.effective_value_for(ui.get(track).unwrap(), ui.properties().background);
    assert_eq!(
        background, None,
        "surface-only template parts should not inherit TYPE_GENERIC as a fake type"
    );
}

#[test]
fn slider_template_parts_match_owner_local_part_selectors() {
    use crate::{PART_SLIDER_TRACK, Slider, TYPE_SLIDER};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_local(
        root,
        ui.properties().layout,
        Layout::flex().logical_size(300.0, 80.0),
    );

    let slider = ui.append(root, Slider::new());
    let track = ui.part(slider, PART_SLIDER_TRACK).unwrap();
    let red = Some(Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff)));
    let style = StyleBuilder::new()
        .set(ui.properties().background, red.clone())
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            SelectorBuilder::new(SelectorStep::type_tag(TYPE_SLIDER))
                .child(SelectorStep::part_tag(PART_SLIDER_TRACK))
                .build(),
            style,
        )
        .build();
    ui.set_cascade(cascade);

    let background = ui.effective_value_for(ui.get(track).unwrap(), ui.properties().background);
    assert_eq!(
        background, red,
        "owner-local part rule should match the nested slider track"
    );

    ui.compute_layout(Size::new(300.0, 80.0));
    let mut output = Scene::new();
    ui.paint(&mut output);
    assert!(
        !output.is_empty(),
        "styled slider track should contribute painted output"
    );
}

#[test]
fn slider_template_creates_keyed_major_and_minor_tick_parts() {
    use crate::{
        CLASS_SLIDER_MAJOR_TICK, CLASS_SLIDER_MINOR_TICK, PART_SLIDER_TICK, PART_SLIDER_TICK_LABEL,
        SLOT_SLIDER_TICK, SLOT_SLIDER_TICK_LABEL, Slider, TemplateItemKey,
    };

    let mut ui = Ui::new();
    let root = ui.root();
    let slider = ui.append(root, Slider::new().min(10.0).max(20.0));
    let ticks: Vec<_> = ui.parts(slider, PART_SLIDER_TICK).collect();
    let labels: Vec<_> = ui.parts(slider, PART_SLIDER_TICK_LABEL).collect();

    assert_eq!(ticks.len(), 5);
    assert_eq!(labels.len(), 5);

    let instance = ui.template_instance(slider).unwrap();
    let first_tick = instance
        .keyed_part(PART_SLIDER_TICK, TemplateItemKey::new(0))
        .unwrap();
    assert_eq!(
        instance.keyed_slot(SLOT_SLIDER_TICK, TemplateItemKey::new(0)),
        Some(first_tick)
    );
    let second_tick = instance
        .keyed_part(PART_SLIDER_TICK, TemplateItemKey::new(1))
        .unwrap();
    assert!(
        ui.get(first_tick)
            .unwrap()
            .classes()
            .contains(&CLASS_SLIDER_MAJOR_TICK)
    );
    assert!(
        ui.get(second_tick)
            .unwrap()
            .classes()
            .contains(&CLASS_SLIDER_MINOR_TICK)
    );

    let middle_label = instance
        .keyed_part(PART_SLIDER_TICK_LABEL, TemplateItemKey::new(2))
        .unwrap();
    assert_eq!(
        instance.keyed_slot(SLOT_SLIDER_TICK_LABEL, TemplateItemKey::new(2)),
        Some(middle_label)
    );
    let content = ui.properties().content;
    let label = ui
        .get(middle_label)
        .unwrap()
        .get_local_value(content)
        .cloned();
    assert_eq!(label.as_deref(), Some("15"));
}

#[test]
fn slider_keyboard_uses_small_and_large_changes() {
    use crate::Slider;
    use ui_events::keyboard::{Code, Key, KeyboardEvent, Modifiers, NamedKey};

    let mut ui = Ui::new();
    let root = ui.root();
    let slider = ui.append(
        root,
        Slider::new()
            .value(0.5)
            .small_change(0.1)
            .large_change(0.25),
    );
    assert!(ui.set_focus(Some(slider)));

    let right = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
    assert!(ui.dispatch_key(&right).handled);
    assert_eq!(slider_value(&ui, slider), Some(0.6));

    let mut shift_right =
        KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
    shift_right.modifiers = Modifiers::SHIFT;
    assert!(ui.dispatch_key(&shift_right).handled);
    assert_eq!(slider_value(&ui, slider), Some(0.85));

    let page_down = KeyboardEvent::key_down(Key::Named(NamedKey::PageDown), Code::PageDown);
    assert!(ui.dispatch_key(&page_down).handled);
    assert_eq!(slider_value(&ui, slider), Some(0.6));

    let home = KeyboardEvent::key_down(Key::Named(NamedKey::Home), Code::Home);
    assert!(ui.dispatch_key(&home).handled);
    assert_eq!(slider_value(&ui, slider), Some(0.0));

    let end = KeyboardEvent::key_down(Key::Named(NamedKey::End), Code::End);
    assert!(ui.dispatch_key(&end).handled);
    assert_eq!(slider_value(&ui, slider), Some(1.0));
}

#[test]
fn slider_thumb_click_and_drag_preserve_thumb_grab_offset() {
    use crate::{SLOT_SLIDER_THUMB, Slider};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_local(
        root,
        ui.properties().layout,
        Layout::flex().logical_size(300.0, 80.0),
    );
    let slider = ui.append(root, Slider::new().value(0.5));
    ui.compute_layout(Size::new(300.0, 80.0));

    let thumb = ui.slot(slider, SLOT_SLIDER_THUMB).unwrap();
    let before = ui.effective_bounds(thumb);
    let press = Point::new(before.x0 + 3.0, before.center().y);

    assert!(ui.dispatch_pointer_event(&pointer_down(press, 1)).changed());
    assert!(ui.dispatch_pointer_event(&pointer_up(press, 2)).changed());
    assert_eq!(
        slider_value(&ui, slider),
        Some(0.5),
        "clicking the thumb itself should not recenter it under the pointer"
    );

    assert!(ui.dispatch_pointer_event(&pointer_down(press, 3)).changed());
    assert!(
        ui.dispatch_pointer_event(&pointer_move(Point::new(press.x + 20.0, press.y), 4, true,))
            .changed()
    );
    ui.compute_layout(Size::new(300.0, 80.0));

    let after = ui.effective_bounds(thumb);
    assert!(
        ((after.center().x - before.center().x) - 20.0).abs() < 0.001,
        "thumb should move by the drag delta instead of snapping its center to the pointer"
    );
}

#[test]
fn default_cascade_styles_template_parts_by_owner_part() {
    use crate::{PART_SLIDER_TICK, Slider, TemplateItemKey, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    let cascade = theme::default_cascade(ui.properties());
    ui.set_cascade(cascade);

    let root = ui.root();
    let slider = ui.append(root, Slider::new());
    let track = ui.part(slider, crate::PART_SLIDER_TRACK).unwrap();
    let major_tick = ui
        .template_instance(slider)
        .unwrap()
        .keyed_part(PART_SLIDER_TICK, TemplateItemKey::new(0))
        .unwrap();
    let expected = Some(Brush::Solid(Color::from_rgba8(0x44, 0x44, 0x4a, 0xff)));
    let background = ui.effective_value_for(ui.get(track).unwrap(), ui.properties().background);
    let tick_background =
        ui.effective_value_for(ui.get(major_tick).unwrap(), ui.properties().background);

    assert_eq!(background, expected);
    assert_eq!(
        tick_background,
        Some(Brush::Solid(Color::from_rgba8(0xc8, 0xc8, 0xd2, 0xff)))
    );
}

#[test]
fn owner_pseudo_selector_can_style_template_descendant() {
    use crate::{Button, PART_BUTTON_LABEL, TYPE_BUTTON};

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, Button::new("Run"));
    let label = ui.part(button, PART_BUTTON_LABEL).unwrap();

    let red = Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff));
    let style = StyleBuilder::new()
        .set(ui.properties().foreground, red.clone())
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            SelectorBuilder::new(SelectorStep::type_tag(TYPE_BUTTON).with_pseudo(PSEUDO_PRESSED))
                .child(SelectorStep::part_tag(PART_BUTTON_LABEL))
                .build(),
            style,
        )
        .build();
    ui.set_cascade(cascade);
    ui.add_pseudo(button, PSEUDO_PRESSED);

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
    assert_eq!(foreground, Some(red));
}

#[test]
fn hit_test_picks_topmost_child() {
    // Hit testing now goes through `understory_box_tree`'s
    // spatially-indexed box-tree hit testing, which reads the
    // tree's `local_bounds` populated by `compute_layout`. Drive
    // the test through the layout pipeline: root 200×200 with
    // 50px padding all around; child fills the content area,
    // giving it 50..150 × 50..150.
    let mut ui = Ui::new();
    let root = ui.root();
    let layout_prop = ui.properties().layout;
    ui.set_local(
        root,
        layout_prop,
        Layout::flex().logical_size(200.0, 200.0).padding_all(50.0),
    );
    let child = ui.insert_child(root);
    ui.set_local(
        child,
        layout_prop,
        Layout::new().flex_grow(1.0).align_self_stretch(),
    );
    ui.compute_layout(Size::new(200.0, 200.0));

    // Inside child's bounds → child wins (deepest, topmost).
    assert_eq!(ui.hit_test(Point::new(100.0, 100.0)), Some(child));
    // Inside root only.
    assert_eq!(ui.hit_test(Point::new(10.0, 10.0)), Some(root));
    // Outside root entirely.
    assert_eq!(ui.hit_test(Point::new(300.0, 300.0)), None);
}

#[test]
fn viewport_clips_subtree_and_sets_cursor() {
    use crate::{CursorIcon, Viewport};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_local(
        root,
        ui.properties().layout,
        Layout::flex().logical_size(100.0, 100.0),
    );
    let viewport = ui.append(
        root,
        Viewport::new()
            .layout(Layout::new().logical_size(100.0, 100.0))
            .cursor_icon(CursorIcon::Grab),
    );
    let element = ui.get(viewport).unwrap();
    assert!(element.widget().unwrap().clips_subtree(element));

    ui.compute_layout(Size::new(100.0, 100.0));
    assert!(
        ui.dispatch_pointer_event(&pointer_move(Point::new(10.0, 10.0), 1, false))
            .hover_changed
    );
    assert_eq!(ui.cursor_icon(), Some(CursorIcon::Grab));
}

#[test]
fn repeating_timer_rearms_while_handler_remains() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    let mut ui = Ui::new();
    let target = ui.root();
    let count = Rc::new(Cell::new(0_u32));
    let count_for_handler = Rc::clone(&count);

    ui.on_timer(target, move |_ui, _event| {
        count_for_handler.set(count_for_handler.get() + 1);
    });
    ui.schedule_timer_repeating(target, 10);

    ui.pump_timers(10);
    assert_eq!(count.get(), 1, "first timer deadline should fire");
    assert_eq!(
        ui.next_timer_deadline(),
        Some(20),
        "live repeating timer should re-arm"
    );

    ui.pump_timers(20);
    assert_eq!(count.get(), 2, "re-armed timer should fire again");
}

#[test]
fn repeating_timer_stops_when_handler_removes_itself() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    let mut ui = Ui::new();
    let target = ui.root();
    let count = Rc::new(Cell::new(0_u32));
    let count_for_handler = Rc::clone(&count);

    ui.on_timer(target, move |ui, _event| {
        count_for_handler.set(count_for_handler.get() + 1);
        ui.remove_timer_handler(target);
    });
    ui.schedule_timer_repeating(target, 10);

    ui.pump_timers(10);
    assert_eq!(count.get(), 1, "first timer deadline should fire");
    assert_eq!(
        ui.next_timer_deadline(),
        None,
        "repeating timer should stop after handler self-removal"
    );

    ui.pump_timers(20);
    assert_eq!(count.get(), 1, "removed handler should not fire again");
}

#[test]
fn repeating_timer_stops_when_target_is_removed() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    let mut ui = Ui::new();
    let root = ui.root();
    let target = ui.insert_child(root);
    let count = Rc::new(Cell::new(0_u32));
    let count_for_handler = Rc::clone(&count);

    ui.on_timer(target, move |ui, _event| {
        count_for_handler.set(count_for_handler.get() + 1);
        ui.remove(target);
    });
    ui.schedule_timer_repeating(target, 10);

    ui.pump_timers(10);
    assert_eq!(count.get(), 1, "first timer deadline should fire");
    assert!(
        ui.get(target).is_none(),
        "timer target should have been removed"
    );
    assert_eq!(
        ui.next_timer_deadline(),
        None,
        "repeating timer should not re-arm for a removed target"
    );

    ui.pump_timers(20);
    assert_eq!(count.get(), 1, "removed target should not receive timers");
}

#[test]
fn hover_path_marks_ancestor_pseudoclasses() {
    // A child hover should mark every element along the
    // root→target path as `:hover` — the more correct CSS/WPF
    // behavior we get from hover path tracking. Sanity-check
    // by laying out a parent with a smaller padded child, hovering
    // over the child, and verifying both elements have :hover.
    let mut ui = Ui::new();
    let root = ui.root();
    let layout_prop = ui.properties().layout;
    ui.set_local(
        root,
        layout_prop,
        Layout::flex().logical_size(200.0, 200.0).padding_all(50.0),
    );
    let child = ui.insert_child(root);
    ui.set_local(
        child,
        layout_prop,
        Layout::new().flex_grow(1.0).align_self_stretch(),
    );
    ui.compute_layout(Size::new(200.0, 200.0));

    // Hover via update_hover_at-equivalent path.
    let changed = ui.update_hover_at(Point::new(100.0, 100.0));
    assert!(changed);

    // Both root and child have :hover; the child is the deepest.
    assert!(ui.get(root).unwrap().pseudos().contains(&PSEUDO_HOVER));
    assert!(ui.get(child).unwrap().pseudos().contains(&PSEUDO_HOVER));
    assert_eq!(ui.hovered(), Some(child));

    // Move pointer outside the child but still in the root's
    // padding band: leaves the child, keeps the root hovered.
    let changed = ui.update_hover_at(Point::new(10.0, 10.0));
    assert!(changed);
    assert!(ui.get(root).unwrap().pseudos().contains(&PSEUDO_HOVER));
    assert!(!ui.get(child).unwrap().pseudos().contains(&PSEUDO_HOVER));
    assert_eq!(ui.hovered(), Some(root));

    // Out of root entirely.
    let changed = ui.update_hover_at(Point::new(500.0, 500.0));
    assert!(changed);
    assert!(!ui.get(root).unwrap().pseudos().contains(&PSEUDO_HOVER));
    assert_eq!(ui.hovered(), None);
}

#[test]
fn element_to_tree_node_round_trips() {
    let mut ui = Ui::new();
    let root = ui.root();
    let a = ui.insert_child(root);
    let b = ui.insert_child(root);

    // Every element has at least one node; `node_to_element`
    // recovers it.
    for elem in [root, a, b] {
        let nodes = ui
            .geometry
            .nodes_for(elem)
            .expect("element should have tree nodes");
        assert!(!nodes.is_empty(), "element should have a tree node");
        for &node in nodes {
            assert_eq!(ui.node_to_element(node), Some(elem));
        }
    }

    // Tree structure mirrors element-tree structure: `a` and `b`
    // are children of `root`'s tree node.
    let root_node = ui.geometry.nodes_for(root).unwrap()[0];
    let a_node = ui.geometry.nodes_for(a).unwrap()[0];
    let b_node = ui.geometry.nodes_for(b).unwrap()[0];
    let children = ui.geometry.children_of(root_node);
    assert_eq!(children, &[a_node, b_node]);
    assert_eq!(ui.geometry.parent_of(a_node), Some(root_node));
}

#[test]
fn flush_bindings_propagates_template_binding() {
    // Two generic elements; binding from source.content → target.content.
    // Writing source.content then flushing should propagate the value
    // into target's `TemplateBinding` slot so its effective value reads
    // the new content.
    use alloc::string::String;
    use understory_property_binding::PropertyEndpoint;

    let mut ui = Ui::new();
    let root = ui.root();
    let source = ui.insert_child(root);
    let target = ui.insert_child(root);

    let content_prop = ui.properties().content;
    ui.bind(
        PropertyEndpoint::new(source, content_prop),
        PropertyEndpoint::new(target, content_prop),
    )
    .expect("binding registers");

    ui.set_local(source, content_prop, String::from("hello"));

    // Before flush, the binding is dirty; target still has its default.
    assert_eq!(ui.get(target).unwrap().get_local_value(content_prop), None);

    ui.flush_bindings();

    // After flush, target's effective local content is "hello".
    let stored = ui
        .get(target)
        .unwrap()
        .property_store()
        .effective_local_erased(content_prop.id())
        .and_then(|v| v.downcast_ref::<String>().cloned());
    assert_eq!(stored.as_deref(), Some("hello"));
}

#[test]
fn flush_bindings_respects_local_precedence() {
    // If the target has a user `Local` value, the binding's
    // `TemplateBinding` write should be shadowed — `Local` outranks
    // `TemplateBinding`.
    use alloc::string::String;
    use understory_property_binding::PropertyEndpoint;

    let mut ui = Ui::new();
    let root = ui.root();
    let source = ui.insert_child(root);
    let target = ui.insert_child(root);

    let content_prop = ui.properties().content;
    // User sets a Local first.
    ui.set_local(target, content_prop, String::from("user"));
    ui.bind(
        PropertyEndpoint::new(source, content_prop),
        PropertyEndpoint::new(target, content_prop),
    )
    .expect("binding registers");
    ui.set_local(source, content_prop, String::from("from-binding"));
    ui.flush_bindings();

    // Target's effective value is still the user's "user" because Local
    // wins over TemplateBinding.
    let stored = ui
        .get(target)
        .unwrap()
        .property_store()
        .effective_local_erased(content_prop.id())
        .and_then(|v| v.downcast_ref::<String>().cloned());
    assert_eq!(stored.as_deref(), Some("user"));
}

#[test]
fn try_flush_bindings_reports_type_mismatch() {
    use alloc::string::String;
    use understory_property::Property;
    use understory_property_binding::{BindingError, PropertyEndpoint};

    let mut ui = Ui::new();
    let root = ui.root();
    let source = ui.insert_child(root);
    let target = ui.insert_child(root);

    let content_prop = ui.properties().content;
    let content_as_u32: Property<u32> = Property::from_id(content_prop.id());
    ui.bind(
        PropertyEndpoint::new(source, content_as_u32),
        PropertyEndpoint::new(target, content_as_u32),
    )
    .expect("binding registers because typed handles carry the same property id");

    ui.set_local(source, content_prop, String::from("not a u32"));

    let error = ui
        .try_flush_bindings()
        .expect_err("wrong erased source type should be reported");
    assert!(matches!(
        error.error(),
        BindingError::SourceTypeMismatch { .. }
    ));
}

#[test]
fn remove_clears_bindings_for_subtree() {
    use alloc::string::String;
    use understory_property_binding::PropertyEndpoint;

    let mut ui = Ui::new();
    let root = ui.root();
    let source = ui.insert_child(root);
    let target = ui.insert_child(root);

    let content_prop = ui.properties().content;
    ui.bind(
        PropertyEndpoint::new(source, content_prop),
        PropertyEndpoint::new(target, content_prop),
    )
    .expect("binding registers");

    ui.remove(target);
    ui.set_local(source, content_prop, String::from("after-remove"));

    let report = ui
        .try_flush_bindings()
        .expect("removed target binding should have been cleared");
    assert_eq!(report.evaluated_bindings(), 0);
}

#[test]
fn template_expansion_inserts_parts_under_owner() {
    use crate::widget::{AppendSpec, SurfacePartWidget, Widget};
    use crate::{Template, TemplateChild, TemplateContext};
    use understory_style::TypeTag;

    // A unit-test widget whose default_template inserts two
    // child panels. Both should have templated_parent = the
    // owner.
    const TEST_OWNER_TAG: TypeTag = TypeTag(0xFEED_BEEF);
    const TEST_PART_A: PartTag = PartTag(0xA);
    const TEST_PART_B: PartTag = PartTag(0xB);

    struct TestTemplate;
    impl Template for TestTemplate {
        fn instantiate(&self, cx: &mut TemplateContext<'_>) {
            let _a = cx.insert(TemplateChild::part(
                TEST_PART_A,
                Box::new(SurfacePartWidget),
            ));
            let _b = cx.insert(TemplateChild::part(
                TEST_PART_B,
                Box::new(SurfacePartWidget),
            ));
        }
    }
    static TEST_TEMPLATE: TestTemplate = TestTemplate;

    #[derive(Debug)]
    struct TestWidget;
    impl Widget for TestWidget {
        fn type_tag(&self) -> Option<TypeTag> {
            Some(TEST_OWNER_TAG)
        }
        fn default_template(&self) -> Option<&'static dyn Template> {
            Some(&TEST_TEMPLATE)
        }
    }
    struct TestSpec;
    impl AppendSpec for TestSpec {
        fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
            ui.append_widget(parent, Box::new(TestWidget))
        }
    }

    let mut ui = Ui::new();
    let root = ui.root();
    let owner = ui.append(root, TestSpec);

    // The template should have created two children under owner, each
    // tagged with templated_parent = Some(owner).
    let owner_children = ui.get(owner).unwrap().children().to_vec();
    assert_eq!(owner_children.len(), 2, "template should insert 2 parts");
    let instance = ui
        .template_instance(owner)
        .expect("template instance should be retained");
    assert_eq!(instance.owner(), owner);
    assert_eq!(instance.part(TEST_PART_A), Some(owner_children[0]));
    assert_eq!(instance.part(TEST_PART_B), Some(owner_children[1]));
    for part_id in owner_children {
        let part = ui.get(part_id).unwrap();
        assert_eq!(
            part.templated_parent(),
            Some(owner),
            "part should track owner"
        );
        assert_eq!(part.parent(), Some(owner), "part is a child of owner");
    }

    // The owner itself has no templated parent (it's a logical child of root).
    assert_eq!(ui.get(owner).unwrap().templated_parent(), None);
}

#[test]
fn template_spec_builds_nested_parts_classes_and_keys() {
    use crate::widget::{AppendSpec, Widget};
    use crate::{Template, TemplateItemKey, TemplateNode, TemplateSlot, TemplateSpec};
    use understory_style::TypeTag;

    const TEST_OWNER_TAG: TypeTag = TypeTag(0xFEED_CAFE);
    const OUTER_PART: PartTag = PartTag(0x10);
    const INNER_PART: PartTag = PartTag(0x11);
    const OUTER_SLOT: TemplateSlot = TemplateSlot::new("outer").with_part_tag(OUTER_PART);
    const OUTER_KEY: TemplateItemKey = TemplateItemKey::new(7);
    const OUTER_CLASS: ClassId = ClassId(0x12);
    const OUTER_CLASSES: &[ClassId] = &[OUTER_CLASS];
    const CHILDREN: &[TemplateNode] = &[TemplateNode::surface().with_part(INNER_PART)];
    const NODES: &[TemplateNode] = &[TemplateNode::surface()
        .with_slot(OUTER_SLOT)
        .with_key(OUTER_KEY)
        .with_classes(OUTER_CLASSES)
        .with_children(CHILDREN)];
    static SPEC: TemplateSpec = TemplateSpec::new(NODES);

    #[derive(Debug)]
    struct TestWidget;
    impl Widget for TestWidget {
        fn type_tag(&self) -> Option<TypeTag> {
            Some(TEST_OWNER_TAG)
        }
        fn default_template(&self) -> Option<&'static dyn Template> {
            Some(&SPEC)
        }
    }
    struct TestSpec;
    impl AppendSpec for TestSpec {
        fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
            ui.append_widget(parent, Box::new(TestWidget))
        }
    }

    let mut ui = Ui::new();
    let owner = ui.append(ui.root(), TestSpec);
    let instance = ui.template_instance(owner).unwrap();
    let outer = instance.keyed_part(OUTER_PART, OUTER_KEY).unwrap();
    let inner = instance.part(INNER_PART).unwrap();

    assert_eq!(instance.keyed_slot(OUTER_SLOT, OUTER_KEY), Some(outer));
    assert_eq!(ui.get(outer).unwrap().parent(), Some(owner));
    assert_eq!(ui.get(inner).unwrap().parent(), Some(outer));
    assert_eq!(ui.get(outer).unwrap().templated_parent(), Some(owner));
    assert_eq!(ui.get(inner).unwrap().templated_parent(), Some(owner));
    assert!(ui.get(outer).unwrap().classes().contains(&OUTER_CLASS));
}

#[test]
fn two_independent_bindings_both_propagate() {
    // Pure-binding analogue of the two-button scenario: two
    // source/target pairs registered + written in sequence.
    // Both targets should receive their bound value on flush.
    use alloc::string::String;
    use understory_property_binding::PropertyEndpoint;

    let mut ui = Ui::new();
    let root = ui.root();
    let s1 = ui.insert_child(root);
    let t1 = ui.insert_child(root);
    let s2 = ui.insert_child(root);
    let t2 = ui.insert_child(root);

    let content_prop = ui.properties().content;
    ui.bind(
        PropertyEndpoint::new(s1, content_prop),
        PropertyEndpoint::new(t1, content_prop),
    )
    .unwrap();
    ui.set_local(s1, content_prop, String::from("one"));

    ui.bind(
        PropertyEndpoint::new(s2, content_prop),
        PropertyEndpoint::new(t2, content_prop),
    )
    .unwrap();
    ui.set_local(s2, content_prop, String::from("two"));

    ui.flush_bindings();

    let read = |ui: &Ui, id: ElementId| -> Option<String> {
        ui.get(id)
            .unwrap()
            .property_store()
            .effective_local_erased(content_prop.id())
            .and_then(|v| v.downcast_ref::<String>().cloned())
    };
    assert_eq!(read(&ui, t1).as_deref(), Some("one"));
    assert_eq!(read(&ui, t2).as_deref(), Some("two"));
}

#[test]
fn button_label_content_propagates_through_template_binding() {
    // The button's template creates a `Text` label, then binds
    // owner.content → label.content. If `Text::append_to` writes
    // `Local=""` on the label, that Local would shadow the
    // binding (Local outranks TemplateBinding) and the label
    // would never display the button's text. Regression guard
    // for two sibling buttons (each must propagate independently).
    use crate::Button;

    let mut ui = Ui::new();
    let root = ui.root();
    let b1 = ui.append(root, Button::new("Click me"));
    let b2 = ui.append(root, Button::new("Or me"));

    ui.flush_bindings();

    let content_prop = ui.properties().content;
    let read_label = |ui: &Ui, button: ElementId| -> Option<String> {
        let label = ui.part(button, crate::PART_BUTTON_LABEL).unwrap();
        ui.get(label)
            .unwrap()
            .property_store()
            .effective_local_erased(content_prop.id())
            .and_then(|v| v.downcast_ref::<String>().cloned())
    };
    assert_eq!(read_label(&ui, b1).as_deref(), Some("Click me"));
    assert_eq!(read_label(&ui, b2).as_deref(), Some("Or me"));
}

#[test]
fn compute_layout_writes_bounds() {
    let mut ui = Ui::new();
    let root = ui.root();
    let child = ui.insert_child(root);

    let layout_prop = ui.properties().layout;
    // Root fills the viewport with 10px padding.
    ui.set_local(
        root,
        layout_prop,
        Layout::flex().percent_size(1.0, 1.0).padding_all(10.0),
    );
    // Child fills the padded content.
    ui.set_local(
        child,
        layout_prop,
        Layout::new().flex_grow(1.0).align_self_stretch(),
    );

    ui.compute_layout(Size::new(200.0, 100.0));
    // Root is the full viewport.
    assert_eq!(ui.effective_bounds(root), Rect::new(0.0, 0.0, 200.0, 100.0));
    // Child fills (10, 10) → (190, 90).
    assert_eq!(
        ui.effective_bounds(child),
        Rect::new(10.0, 10.0, 190.0, 90.0)
    );
}

#[test]
fn composition_layers_track_layout_geometry() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(
        root,
        Layout::flex().logical_size(200.0, 100.0).padding_all(10.0),
    );
    let child = ui.insert_child(root);
    ui.set_layout(child, Layout::new().logical_size(30.0, 20.0));

    ui.compute_layout(Size::new(200.0, 100.0));

    let root_layer = ui.composition.layer_for(root).unwrap();
    let child_layer = ui.composition.layer_for(child).unwrap();
    let layers = ui.composition_layers();

    assert_eq!(layers.parent(child_layer), Some(root_layer));
    assert_eq!(layers.bounds(root_layer), Size::new(200.0, 100.0));
    assert_eq!(layers.bounds(child_layer), Size::new(30.0, 20.0));
    assert_eq!(
        layers
            .world_transform(child_layer)
            .transform_point(Point::ZERO),
        Some(Point::new(10.0, 10.0))
    );
    assert_eq!(ui.composition.element_for_layer(child_layer), Some(child));
}

#[test]
fn composition_layer_is_destroyed_with_element() {
    let mut ui = Ui::new();
    let root = ui.root();
    let child = ui.insert_child(root);
    let layer = ui.composition.layer_for(child).unwrap();

    ui.remove(child);
    ui.flush_composition();

    assert!(ui.composition.layer_for(child).is_none());
    assert!(!ui.composition_layers().is_alive(layer));
    assert!(ui.composition_changes().removed.contains(&layer.index()));
}

#[test]
fn paint_rebuilds_root_composition_surface() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::flex().logical_size(40.0, 20.0));
    ui.set_local(
        root,
        ui.properties().background,
        Some(Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff))),
    );
    ui.compute_layout(Size::new(40.0, 20.0));

    let mut output = Scene::new();
    ui.paint(&mut output);

    let root_layer = ui.composition.layer_for(root).unwrap();
    let root_surface = ui.composition.surface_for(root).unwrap();
    assert_eq!(
        ui.composition_layers().content(root_layer),
        Some(root_surface)
    );
    assert!(!ui.composition.surface(root_surface).unwrap().is_empty());
    assert!(!output.is_empty());

    let updates: Vec<_> = ui
        .composition_surface_updates()
        .map(|surface| surface.surface())
        .collect();
    assert_eq!(updates, vec![root_surface]);

    assert!(
        ui.composition_surfaces()
            .any(|surface| surface.surface() == root_surface && !surface.scene().is_empty())
    );
}

#[test]
fn paint_rebuilds_child_composition_surfaces() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(0.0).logical_size(40.0, 40.0));
    let child = ui.insert_child(root);
    ui.set_layout(child, Layout::new().logical_size(20.0, 20.0));
    ui.set_local(
        child,
        ui.properties().background,
        Some(Brush::Solid(Color::from_rgba8(0x00, 0xff, 0x00, 0xff))),
    );
    ui.compute_layout(Size::new(40.0, 40.0));

    let mut output = Scene::new();
    ui.paint(&mut output);

    let child_layer = ui.composition.layer_for(child).unwrap();
    let child_surface = ui.composition.surface_for(child).unwrap();
    assert_eq!(
        ui.composition_layers().content(child_layer),
        Some(child_surface)
    );
    assert!(!ui.composition.surface(child_surface).unwrap().is_empty());
    assert!(
        ui.composition_surface_updates()
            .any(|surface| surface.surface() == child_surface)
    );
    assert!(!output.is_empty());

    ui.set_local(
        child,
        ui.properties().background,
        Some(Brush::Solid(Color::from_rgba8(0xff, 0xff, 0x00, 0xff))),
    );
    ui.paint(&mut output);
    let updates: Vec<_> = ui
        .composition_surface_updates()
        .map(|surface| surface.surface())
        .collect();
    assert_eq!(updates, vec![child_surface]);
}

#[test]
fn composition_policy_can_make_container_only_layer() {
    use crate::widget::{AppendSpec, Widget};

    #[derive(Debug)]
    struct ContainerOnlyWidget;

    impl Widget for ContainerOnlyWidget {
        fn type_tag(&self) -> Option<understory_style::TypeTag> {
            None
        }

        fn composition_policy(&self) -> CompositionPolicy {
            CompositionPolicy::Container
        }
    }

    struct ContainerOnly;

    impl AppendSpec for ContainerOnly {
        fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
            ui.append_widget(parent, Box::new(ContainerOnlyWidget))
        }
    }

    let mut ui = Ui::new();
    let root = ui.root();
    let container = ui.append(root, ContainerOnly);

    let layer = ui.composition.layer_for(container).unwrap();
    assert_eq!(ui.composition_layers().content(layer), None);
    assert_eq!(ui.composition.surface_for(container), None);
}

#[test]
fn overlay_composition_policy_parents_layer_under_overlay_root() {
    use crate::widget::{AppendSpec, Widget};

    #[derive(Debug)]
    struct OverlayWidget;

    impl Widget for OverlayWidget {
        fn type_tag(&self) -> Option<understory_style::TypeTag> {
            None
        }

        fn composition_policy(&self) -> CompositionPolicy {
            CompositionPolicy::OverlayScene
        }
    }

    struct OverlayOnly;

    impl AppendSpec for OverlayOnly {
        fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
            ui.append_widget(parent, Box::new(OverlayWidget))
        }
    }

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(
        root,
        Layout::column(0.0)
            .logical_size(100.0, 100.0)
            .padding_all(10.0),
    );
    let parent = ui.insert_child(root);
    ui.set_layout(
        parent,
        Layout::column(0.0)
            .logical_size(50.0, 50.0)
            .padding_all(7.0),
    );
    let overlay = ui.append(parent, OverlayOnly);
    ui.set_layout(overlay, Layout::new().logical_size(10.0, 10.0));

    ui.compute_layout(Size::new(100.0, 100.0));

    let overlay_root = ui
        .composition
        .overlay_root()
        .expect("composition should create an overlay root");
    let overlay_layer = ui.composition.layer_for(overlay).unwrap();
    let layers = ui.composition_layers();
    assert_eq!(layers.parent(overlay_layer), Some(overlay_root));
    assert_eq!(layers.content(overlay_root), None);
    assert_eq!(layers.roots().last(), Some(&overlay_root));
    assert_eq!(
        layers
            .world_transform(overlay_layer)
            .transform_point(Point::ZERO),
        Some(ui.effective_bounds(overlay).origin())
    );
}

#[test]
fn tooltip_shows_as_overlay_for_hovered_target() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().logical_size(220.0, 120.0));
    let target = ui.insert_child(root);
    ui.set_layout(target, Layout::new().logical_size(80.0, 30.0));
    ui.set_tooltip(target, crate::Tooltip::new("Deploys the service"));

    ui.compute_layout(Size::new(220.0, 120.0));
    assert_eq!(ui.overlays.active_tooltip(), None);

    let result = ui.dispatch_pointer_event(&pointer_move(Point::new(20.0, 12.0), 1, false));
    assert!(result.changed());
    let active = ui
        .overlays
        .active_tooltip()
        .expect("hovering a tooltip target should create an overlay tooltip");
    assert_eq!(active.target, target);
    assert_eq!(ui.get(active.element).unwrap().parent(), Some(root));

    ui.compute_layout(Size::new(220.0, 120.0));
    let target_bounds = ui.effective_bounds(target);
    let tooltip_bounds = ui.effective_bounds(active.element);
    assert_eq!(
        tooltip_bounds.origin(),
        Point::new(target_bounds.x0, target_bounds.y1 + 8.0)
    );
    assert!(tooltip_bounds.width() > 0.0);
    assert!(tooltip_bounds.height() > 0.0);

    let overlay_root = ui
        .composition
        .overlay_root()
        .expect("tooltip should create an overlay composition root");
    let tooltip_layer = ui.composition.layer_for(active.element).unwrap();
    assert_eq!(
        ui.composition_layers().parent(tooltip_layer),
        Some(overlay_root)
    );
    assert_ne!(ui.hit_test(tooltip_bounds.center()), Some(active.element));

    let leave = ui.dispatch_pointer_event(&pointer_move(Point::new(180.0, 100.0), 2, false));
    assert!(leave.changed());
    assert_eq!(ui.overlays.active_tooltip(), None);
    assert!(ui.get(active.element).is_none());
}

#[test]
fn default_cascade_styles_tooltip_surface_and_label() {
    use crate::{SLOT_TOOLTIP_CONTENT, Tooltip, theme};

    let mut ui = Ui::new();
    ui.set_theme(theme::default_theme());
    ui.set_cascade(theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::new().logical_size(220.0, 120.0));
    let target = ui.insert_child(root);
    ui.set_layout(target, Layout::new().logical_size(80.0, 30.0));
    ui.set_tooltip(target, Tooltip::new("Deploys the service"));

    ui.compute_layout(Size::new(220.0, 120.0));
    let result = ui.dispatch_pointer_event(&pointer_move(Point::new(20.0, 12.0), 1, false));
    assert!(result.changed());
    let active = ui
        .overlays
        .active_tooltip()
        .expect("hovering a tooltip target should create an overlay tooltip");

    ui.compute_layout(Size::new(220.0, 120.0));
    let mut output = Scene::new();
    ui.paint(&mut output);

    let tooltip_node = ui
        .box_for(active.element)
        .expect("tooltip should have a box node");
    let surface = ui
        .presentation
        .presentation(tooltip_node)
        .expect("tooltip should have presentation")
        .surface()
        .expect("tooltip should present a surface");
    assert!(
        !surface.backgrounds.is_empty(),
        "default cascade should give tooltips an opaque background"
    );
    assert!(
        !surface.border.is_empty(),
        "default cascade should give tooltips a border"
    );

    let label = ui
        .slot(active.element, SLOT_TOOLTIP_CONTENT)
        .expect("tooltip should create a content label");
    let label_node = ui.box_for(label).expect("label should have a box node");
    let foreground = ui
        .presentation
        .presentation(label_node)
        .expect("label should have presentation")
        .plain_text()
        .expect("label should present text")
        .foreground
        .clone();
    assert_eq!(
        foreground,
        Some(Brush::Solid(Color::from_rgba8(0xf4, 0xf4, 0xf7, 0xff))),
        "tooltip label should inherit the styled tooltip foreground"
    );
}

#[test]
fn tooltip_attached_to_owner_activates_from_template_part_hover() {
    let mut ui = Ui::new();
    ui.set_cascade(crate::theme::default_cascade(ui.properties()));
    let root = ui.root();
    ui.set_layout(root, Layout::new().logical_size(220.0, 120.0));
    let button = ui.append(root, crate::Button::new("Deploy").font_size(12.0));
    ui.set_layout(button, Layout::new().logical_size(92.0, 36.0));
    ui.set_tooltip(button, crate::Tooltip::new("Runs the deploy workflow"));

    ui.compute_layout(Size::new(220.0, 120.0));
    let center = ui.effective_bounds(button).center();
    let _ = ui.dispatch_pointer_event(&pointer_move(center, 1, false));

    assert_eq!(
        ui.overlays.active_tooltip().map(|active| active.target),
        Some(button),
        "tooltip lookup should walk from hovered template parts to the owning control"
    );
}

#[test]
fn tooltip_follows_keyboard_focus_and_clears_with_focus() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().logical_size(220.0, 120.0));
    let target = ui.insert_child(root);
    ui.set_layout(target, Layout::new().logical_size(80.0, 30.0));
    ui.set_focusable(target, true);
    ui.set_tooltip(target, crate::Tooltip::new("Focusable helper"));

    ui.compute_layout(Size::new(220.0, 120.0));
    assert!(ui.set_focus(Some(target)));
    let active = ui
        .overlays
        .active_tooltip()
        .expect("focused tooltip target should create an overlay tooltip");
    assert_eq!(active.target, target);

    assert!(ui.set_focus(None));
    assert_eq!(ui.overlays.active_tooltip(), None);
    assert!(ui.get(active.element).is_none());
}

#[test]
fn pointer_leave_does_not_restore_previous_focused_tooltip() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::row(20.0).logical_size(240.0, 120.0));
    let first = ui.insert_child(root);
    ui.set_layout(first, Layout::new().logical_size(70.0, 30.0));
    ui.set_focusable(first, true);
    ui.set_tooltip(first, crate::Tooltip::new("First"));
    let second = ui.insert_child(root);
    ui.set_layout(second, Layout::new().logical_size(70.0, 30.0));
    ui.set_tooltip(second, crate::Tooltip::new("Second"));

    ui.compute_layout(Size::new(240.0, 120.0));
    assert!(ui.set_focus(Some(first)));
    assert_eq!(
        ui.overlays.active_tooltip().map(|active| active.target),
        Some(first)
    );

    let second_center = ui.effective_bounds(second).center();
    let _ = ui.dispatch_pointer_event(&pointer_move(second_center, 1, false));
    assert_eq!(
        ui.overlays.active_tooltip().map(|active| active.target),
        Some(second)
    );

    let _ = ui.dispatch_pointer_event(&pointer_move(Point::new(220.0, 90.0), 2, false));
    assert_eq!(
        ui.overlays.active_tooltip(),
        None,
        "leaving a hover tooltip should hide it instead of restoring a focused target"
    );
}

#[test]
fn tooltip_placement_flips_when_bottom_would_overflow() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(220.0, 90.0));
    let target = ui.insert_child(root);
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(10.0, 50.0)
            .logical_size(70.0, 30.0),
    );
    ui.set_tooltip(target, crate::Tooltip::new("Near the bottom edge"));

    ui.compute_layout(Size::new(220.0, 90.0));
    let _ = ui.dispatch_pointer_event(&pointer_move(
        ui.effective_bounds(target).center(),
        1,
        false,
    ));
    ui.compute_layout(Size::new(220.0, 90.0));

    let active = ui.overlays.active_tooltip().unwrap();
    let target_bounds = ui.effective_bounds(target);
    let tooltip_bounds = ui.effective_bounds(active.element);
    assert!(
        tooltip_bounds.y1 <= target_bounds.y0,
        "bottom placement should flip above the target when it would overflow"
    );
}

#[test]
fn tooltip_can_be_placed_to_the_right_of_target() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(260.0, 100.0));
    let target = ui.insert_child(root);
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(20.0, 20.0)
            .logical_size(60.0, 24.0),
    );
    ui.set_tooltip(
        target,
        crate::Tooltip::new("Side")
            .placement(crate::Placement::Right)
            .offset(6.0, 0.0),
    );

    ui.compute_layout(Size::new(260.0, 100.0));
    let _ = ui.dispatch_pointer_event(&pointer_move(
        ui.effective_bounds(target).center(),
        1,
        false,
    ));
    ui.compute_layout(Size::new(260.0, 100.0));

    let active = ui.overlays.active_tooltip().unwrap();
    let target_bounds = ui.effective_bounds(target);
    let tooltip_bounds = ui.effective_bounds(active.element);
    assert_eq!(
        tooltip_bounds.origin(),
        Point::new(target_bounds.x1 + 6.0, target_bounds.y0)
    );
}

#[test]
fn popup_opens_as_overlay_container_with_content() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(260.0, 120.0));
    let target = ui.insert_child(root);
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(20.0, 20.0)
            .logical_size(80.0, 28.0),
    );
    ui.compute_layout(Size::new(260.0, 120.0));

    let popup = ui
        .open_popup(target, crate::Popup::new(), |ui, popup| {
            ui.append(popup, crate::Text::new("Popup content").font_size(12.0));
        })
        .expect("live target should open popup");
    ui.compute_layout(Size::new(260.0, 120.0));

    assert_eq!(ui.get(popup).unwrap().parent(), Some(root));
    assert_eq!(ui.get(popup).unwrap().children().len(), 1);
    let target_bounds = ui.effective_bounds(target);
    let popup_bounds = ui.effective_bounds(popup);
    assert_eq!(
        popup_bounds.origin(),
        Point::new(target_bounds.x0, target_bounds.y1 + 6.0)
    );

    let overlay_root = ui.composition.overlay_root().unwrap();
    let popup_layer = ui.composition.layer_for(popup).unwrap();
    assert_eq!(
        ui.composition_layers().parent(popup_layer),
        Some(overlay_root)
    );
}

#[test]
fn popup_repositions_with_target_and_closes_by_root_element() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(260.0, 140.0));
    let target = ui.insert_child(root);
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(20.0, 20.0)
            .logical_size(70.0, 24.0),
    );
    ui.compute_layout(Size::new(260.0, 140.0));

    let popup = ui
        .open_popup(target, crate::Popup::new(), |ui, popup| {
            ui.append(popup, crate::Text::new("Popup").font_size(12.0));
        })
        .unwrap();
    ui.compute_layout(Size::new(260.0, 140.0));
    let original = ui.effective_bounds(popup).origin();

    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(120.0, 40.0)
            .logical_size(70.0, 24.0),
    );
    ui.compute_layout(Size::new(260.0, 140.0));
    let moved = ui.effective_bounds(popup).origin();
    assert_ne!(moved, original);
    assert_eq!(moved, Point::new(120.0, 70.0));

    assert!(ui.close_popup(popup));
    assert!(ui.get(popup).is_none());
    assert!(!ui.close_popup(popup));
}

#[test]
fn escape_closes_topmost_popup_and_restores_focus() {
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(260.0, 140.0));
    let target = ui.append(root, crate::Button::new("Open"));
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(20.0, 20.0)
            .logical_size(70.0, 28.0),
    );
    ui.compute_layout(Size::new(260.0, 140.0));
    assert!(ui.set_focus(Some(target)));

    let mut focused_inside = None;
    let popup = ui
        .open_popup(target, crate::Popup::new(), |ui, popup| {
            let child = ui.insert_child(popup);
            ui.set_layout(child, Layout::new().logical_size(80.0, 24.0));
            ui.set_focusable(child, true);
            focused_inside = Some(child);
        })
        .unwrap();
    ui.compute_layout(Size::new(260.0, 140.0));
    assert!(ui.set_focus(focused_inside));

    let escape = KeyboardEvent::key_down(Key::Named(NamedKey::Escape), Code::Escape);
    let result = ui.dispatch_key(&escape);

    assert!(result.handled);
    assert!(result.focus_changed);
    assert_eq!(result.focused, Some(target));
    assert!(ui.get(popup).is_none());
}

#[test]
fn outside_pointer_down_closes_popup_without_consuming_click() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::new().relative().logical_size(320.0, 120.0));
    let target = ui.append(root, crate::Button::new("Open"));
    ui.set_layout(
        target,
        Layout::new()
            .absolute_at(20.0, 20.0)
            .logical_size(70.0, 28.0),
    );
    let outside = ui.append(root, crate::Button::new("Other"));
    ui.set_layout(
        outside,
        Layout::new()
            .absolute_at(210.0, 20.0)
            .logical_size(80.0, 28.0),
    );
    let clicked = Rc::new(Cell::new(false));
    let clicked_for_handler = Rc::clone(&clicked);
    ui.on_click(outside, move |_ui, _event| {
        clicked_for_handler.set(true);
    });
    ui.compute_layout(Size::new(320.0, 120.0));

    let popup = ui
        .open_popup(target, crate::Popup::new(), |ui, popup| {
            ui.append(popup, crate::Text::new("Popup").font_size(12.0));
        })
        .unwrap();
    ui.compute_layout(Size::new(320.0, 120.0));
    assert!(ui.get(popup).is_some());

    let outside_center = ui.effective_bounds(outside).center();
    let down = ui.dispatch_pointer_event(&pointer_down(outside_center, 1));
    assert!(down.changed());
    assert!(ui.get(popup).is_none());

    let up = ui.dispatch_pointer_event(&pointer_up(outside_center, 2));
    assert!(up.changed());
    assert!(
        clicked.get(),
        "outside click should continue after dismissal"
    );
}

#[test]
fn scroll_view_clip_applies_to_child_surfaces() {
    use imaging::record::Command;

    let mut ui = Ui::new();
    let root = ui.root();
    ui.set_layout(root, Layout::column(0.0).logical_size(120.0, 80.0));
    let scroll = ui.append(root, crate::ScrollView::new().size(80.0, 32.0));
    let child = ui.insert_child(scroll);
    ui.set_layout(
        child,
        Layout::new().logical_size(80.0, 64.0).flex_shrink(0.0),
    );
    ui.set_local(
        child,
        ui.properties().background,
        Some(Brush::Solid(Color::from_rgba8(0x00, 0x00, 0xff, 0xff))),
    );

    ui.compute_layout(Size::new(120.0, 80.0));
    let mut output = Scene::new();
    ui.paint(&mut output);

    let scroll_layer = ui.composition.layer_for(scroll).unwrap();
    let child_layer = ui.composition.layer_for(child).unwrap();
    assert!(ui.composition_layers().clip(scroll_layer).is_some());
    assert!(ui.composition_layers().content(child_layer).is_some());
    assert!(
        output
            .commands()
            .iter()
            .any(|command| matches!(command, Command::PushClip(_)))
    );
}

#[test]
fn press_marks_templated_control_not_inner_part() {
    // Pressing on a templated control's part (Button's label
    // `Text`) should put `:pressed` on the Button itself, not
    // on the label. Without this, the cascade rule keyed on
    // `TYPE_BUTTON:pressed` never matches and the button shows
    // no press feedback. Use `press_visual_target` directly so
    // the test doesn't depend on layout + hit-test plumbing.
    use crate::Button;

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, Button::new("Click me"));
    let label = ui.part(button, crate::PART_BUTTON_LABEL).unwrap();

    assert_eq!(ui.press_visual_target(label), button);
    assert_eq!(ui.press_visual_target(button), button);
    assert_eq!(ui.press_visual_target(root), root);
}

#[test]
fn disabled_button_carries_state_and_ignores_builder_click() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    use crate::Button;

    let mut ui = Ui::new();
    let root = ui.root();
    let fired = Rc::new(Cell::new(false));
    let fired_for_handler = Rc::clone(&fired);
    let button = ui.append(
        root,
        Button::new("No")
            .disabled()
            .on_click(move |_ui, _event| fired_for_handler.set(true)),
    );

    assert!(!ui.is_enabled(button));
    assert!(ui.get(button).unwrap().pseudos().contains(&PSEUDO_DISABLED));

    ui.synthesize_click(button, Point::ZERO);
    assert!(
        !fired.get(),
        "disabled button click handler should not fire"
    );

    assert!(!ui.set_focus(Some(button)));
    assert_eq!(ui.focused(), None, "disabled button should reject focus");
}

#[test]
fn set_enabled_false_clears_focus_and_pressed_state() {
    use crate::Button;

    let mut ui = Ui::new();
    let root = ui.root();
    let button = ui.append(root, Button::new("Run"));

    assert!(ui.set_focus(Some(button)));
    ui.set_press_target(Some(button));
    ui.set_pointer_capture(Some(button));
    ui.set_enabled(button, false);

    assert!(!ui.is_enabled(button));
    assert_eq!(ui.focused(), None);
    assert_eq!(ui.events.press_target(), None);
    assert_eq!(ui.pointer_capture(), None);
    let pseudos = ui.get(button).unwrap().pseudos();
    assert!(pseudos.contains(&PSEUDO_DISABLED));
    assert!(!pseudos.contains(&PSEUDO_FOCUSED));
    assert!(!pseudos.contains(&PSEUDO_FOCUS_VISIBLE));
    assert!(!pseudos.contains(&PSEUDO_PRESSED));

    ui.set_enabled(button, true);
    assert!(ui.is_enabled(button));
    assert!(
        ui.is_focusable(button),
        "re-enabling should restore the stored focusable preference"
    );
    assert!(ui.set_focus(Some(button)));
}

#[test]
fn disabled_toggle_and_slider_share_control_state_policy() {
    use alloc::rc::Rc;
    use core::cell::Cell;

    use crate::{Slider, ToggleSwitch};

    let mut ui = Ui::new();
    let root = ui.root();

    let toggle = ui.append(root, ToggleSwitch::new().disabled());
    assert!(!ui.is_enabled(toggle));
    assert!(!ui.is_focusable(toggle));
    assert!(ui.get(toggle).unwrap().pseudos().contains(&PSEUDO_DISABLED));
    ui.synthesize_click(toggle, Point::ZERO);
    let checked = ui
        .get(toggle)
        .and_then(|el| el.get_local_value(ui.properties().is_checked).copied())
        .unwrap_or(false);
    assert!(!checked, "disabled toggle should ignore user activation");

    let fired = Rc::new(Cell::new(false));
    let fired_for_handler = Rc::clone(&fired);
    let slider = ui.append(root, Slider::new().disabled());
    ui.on_click(slider, move |_ui, _event| fired_for_handler.set(true));
    assert!(!ui.is_enabled(slider));
    assert!(!ui.is_focusable(slider));
    assert!(ui.get(slider).unwrap().pseudos().contains(&PSEUDO_DISABLED));
    ui.synthesize_click(slider, Point::ZERO);
    assert!(
        !fired.get(),
        "disabled slider should suppress click dispatch"
    );
}

#[test]
fn pointer_leave_clears_hover() {
    // Move events require constructing a PointerUpdate with platform
    // position types; we exercise that end-to-end via the demo. Here we
    // verify the dispatch shape: a Leave event clears whatever was
    // currently hovered.

    let mut ui = Ui::new();
    let root = ui.root();
    // Manually mark something hovered (this is what dispatch_pointer_event
    // does internally via add_pseudo on a Move).
    ui.add_pseudo(root, PSEUDO_HOVER);
    // Match the internal state. (Phase 1d's dispatch is the only writer
    // of `hovered` in real code, so this is a deliberate test-only nudge.)
    // We test the Leave path which clears via the public API.
    // Even if hovered is None (we never set it via dispatch), Leave
    // should be a no-op rather than panic.
    let _ = ui.dispatch_pointer_event(&pointer_leave());
    assert_eq!(ui.hovered(), None);
}

#[test]
fn add_class_restyles_subtree() {
    let mut ui = Ui::new();
    let root = ui.root();
    ui.add_class(root, ClassId(1));
    // `add_class` restyles synchronously — the class is on the
    // element afterward and any cascade-driven channel changes
    // are already marked via `restyle_subtree`. With an empty
    // cascade (this test's case) no channels move.
    assert!(ui.get(root).unwrap().classes().contains(&ClassId(1)));
}

fn slider_value(ui: &Ui, slider: ElementId) -> Option<f64> {
    ui.get(slider)
        .and_then(|el| el.get_local_value(ui.properties().value).copied())
}
