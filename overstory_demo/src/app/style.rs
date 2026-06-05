// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

pub(super) const DEMO_ROOT_BG: ResourceKey = ResourceKey::new(1_000);
pub(super) const DEMO_NAV_BG: ResourceKey = ResourceKey::new(1_001);
pub(super) const DEMO_NAV_BORDER: ResourceKey = ResourceKey::new(1_002);
pub(super) const DEMO_NAV_SELECTED_BG: ResourceKey = ResourceKey::new(1_003);
pub(super) const DEMO_STAGE_BG: ResourceKey = ResourceKey::new(1_004);
pub(super) const DEMO_STAGE_BORDER: ResourceKey = ResourceKey::new(1_005);
pub(super) const DEMO_HEADER_BG: ResourceKey = ResourceKey::new(1_006);
pub(super) const DEMO_HEADER_BORDER: ResourceKey = ResourceKey::new(1_007);
pub(super) const DEMO_CARD_BG: ResourceKey = ResourceKey::new(1_008);
pub(super) const DEMO_CARD_BORDER: ResourceKey = ResourceKey::new(1_009);
pub(super) const DEMO_SPLIT_BORDER: ResourceKey = ResourceKey::new(1_010);
pub(super) const DEMO_SPLIT_SCENE_BG: ResourceKey = ResourceKey::new(1_011);
pub(super) const DEMO_SPLIT_INSPECTOR_BG: ResourceKey = ResourceKey::new(1_012);
pub(super) const DEMO_MOTION_STAGE_BG: ResourceKey = ResourceKey::new(1_013);
pub(super) const DEMO_ACCENT_BG: ResourceKey = ResourceKey::new(1_014);
pub(super) const DEMO_TEXT_FG: ResourceKey = ResourceKey::new(1_015);
pub(super) const DEMO_BODY_FG: ResourceKey = ResourceKey::new(1_016);
pub(super) const DEMO_MUTED_FG: ResourceKey = ResourceKey::new(1_017);
pub(super) const DEMO_TITLE_FG: ResourceKey = ResourceKey::new(1_018);
pub(super) const DEMO_FOCUS_BORDER: ResourceKey = ResourceKey::new(1_019);

pub(super) const CLASS_DEMO_ROOT: ClassId = ClassId(1);
pub(super) const CLASS_DEMO_NAV: ClassId = ClassId(2);
pub(super) const CLASS_DEMO_NAV_MARKER: ClassId = ClassId(3);
pub(super) const CLASS_DEMO_STAGE: ClassId = ClassId(4);
pub(super) const CLASS_DEMO_HEADER: ClassId = ClassId(5);
pub(super) const CLASS_DEMO_HEADER_ACCENT: ClassId = ClassId(6);
pub(super) const CLASS_DEMO_CARD: ClassId = ClassId(7);
pub(super) const CLASS_DEMO_NAV_ROW: ClassId = ClassId(8);
pub(super) const CLASS_DEMO_NAV_LABEL: ClassId = ClassId(9);
pub(super) const CLASS_DEMO_HEADING: ClassId = ClassId(10);
pub(super) const CLASS_DEMO_PAGE_HEADING: ClassId = ClassId(11);
pub(super) const CLASS_DEMO_CAPTION: ClassId = ClassId(12);
pub(super) const CLASS_DEMO_CARD_TITLE: ClassId = ClassId(13);
pub(super) const CLASS_DEMO_CONTROL_LABEL: ClassId = ClassId(14);
pub(super) const CLASS_DEMO_NUMBER: ClassId = ClassId(15);
pub(super) const CLASS_DEMO_LARGE_NUMBER: ClassId = ClassId(16);
pub(super) const CLASS_DEMO_BODY: ClassId = ClassId(17);
pub(super) const CLASS_DEMO_SPLIT_SCENE: ClassId = ClassId(18);
pub(super) const CLASS_DEMO_SPLIT_INSPECTOR: ClassId = ClassId(19);
pub(super) const CLASS_DEMO_MOTION_STAGE: ClassId = ClassId(20);
pub(super) const CLASS_SVG_GRID_LINE: ClassId = ClassId(21);
pub(super) const CLASS_SVG_INFO_CARD: ClassId = ClassId(22);
pub(super) const CLASS_SVG_TIGER_CARD: ClassId = ClassId(23);
pub(super) const CLASS_SVG_INSPECTOR_CARD: ClassId = ClassId(24);
pub(super) const CLASS_DEVTOOLS_INSPECTOR_CARD: ClassId = ClassId(25);
pub(super) const CLASS_DEVTOOLS_STATUS: ClassId = ClassId(26);
pub(super) const CLASS_DEVTOOLS_BUTTON: ClassId = ClassId(27);
pub(super) const CLASS_DEMO_TEMPLATE_BUTTON_OUTER: ClassId = ClassId(28);
pub(super) const CLASS_DEMO_TEMPLATE_BUTTON_INNER: ClassId = ClassId(29);
pub(super) const CLASS_DEMO_TEMPLATE_CHECK_BOX_BOX: ClassId = ClassId(30);
pub(super) const CLASS_DEMO_TEMPLATE_SLIDER_TRACK: ClassId = ClassId(31);
pub(super) const CLASS_DEMO_TEMPLATE_SLIDER_THUMB: ClassId = ClassId(32);
pub(super) const CLASS_OKLAB_SWATCH: ClassId = ClassId(100);
pub(super) const CLASS_OPACITY_SWATCH: ClassId = ClassId(101);
pub(super) const CLASS_BORDER_SWATCH: ClassId = ClassId(102);
pub(super) const CLASS_RETARGET_SWATCH: ClassId = ClassId(103);
pub(super) const CLASS_MOVER_SWATCH: ClassId = ClassId(104);
pub(super) const CLASS_SRGB_SWATCH: ClassId = ClassId(105);

pub(super) fn register_demo_style_debug_names(ui: &mut Ui) {
    ui.set_class_debug_name(CLASS_DEMO_ROOT, ".demo-root");
    ui.set_class_debug_name(CLASS_DEMO_NAV, ".demo-nav");
    ui.set_class_debug_name(CLASS_DEMO_NAV_MARKER, ".demo-nav-marker");
    ui.set_class_debug_name(CLASS_DEMO_STAGE, ".demo-stage");
    ui.set_class_debug_name(CLASS_DEMO_HEADER, ".demo-header");
    ui.set_class_debug_name(CLASS_DEMO_HEADER_ACCENT, ".demo-header-accent");
    ui.set_class_debug_name(CLASS_DEMO_CARD, ".demo-card");
    ui.set_class_debug_name(CLASS_DEMO_NAV_ROW, ".demo-nav-row");
    ui.set_class_debug_name(CLASS_DEMO_NAV_LABEL, ".demo-nav-label");
    ui.set_class_debug_name(CLASS_DEMO_HEADING, ".demo-heading");
    ui.set_class_debug_name(CLASS_DEMO_PAGE_HEADING, ".demo-page-heading");
    ui.set_class_debug_name(CLASS_DEMO_CAPTION, ".demo-caption");
    ui.set_class_debug_name(CLASS_DEMO_CARD_TITLE, ".demo-card-title");
    ui.set_class_debug_name(CLASS_DEMO_CONTROL_LABEL, ".demo-control-label");
    ui.set_class_debug_name(CLASS_DEMO_NUMBER, ".demo-number");
    ui.set_class_debug_name(CLASS_DEMO_LARGE_NUMBER, ".demo-large-number");
    ui.set_class_debug_name(CLASS_DEMO_BODY, ".demo-body");
    ui.set_class_debug_name(CLASS_DEMO_SPLIT_SCENE, ".demo-split-scene");
    ui.set_class_debug_name(CLASS_DEMO_SPLIT_INSPECTOR, ".demo-split-inspector");
    ui.set_class_debug_name(CLASS_DEMO_MOTION_STAGE, ".demo-motion-stage");
    ui.set_class_debug_name(CLASS_SVG_GRID_LINE, ".svg-grid-line");
    ui.set_class_debug_name(CLASS_SVG_INFO_CARD, ".svg-info-card");
    ui.set_class_debug_name(CLASS_SVG_TIGER_CARD, ".svg-tiger-card");
    ui.set_class_debug_name(CLASS_SVG_INSPECTOR_CARD, ".svg-inspector-card");
    ui.set_class_debug_name(CLASS_DEVTOOLS_INSPECTOR_CARD, ".devtools-inspector-card");
    ui.set_class_debug_name(CLASS_DEVTOOLS_STATUS, ".devtools-status");
    ui.set_class_debug_name(CLASS_DEVTOOLS_BUTTON, ".devtools-button");
    ui.set_class_debug_name(
        CLASS_DEMO_TEMPLATE_BUTTON_OUTER,
        ".demo-template-button-outer",
    );
    ui.set_class_debug_name(
        CLASS_DEMO_TEMPLATE_BUTTON_INNER,
        ".demo-template-button-inner",
    );
    ui.set_class_debug_name(
        CLASS_DEMO_TEMPLATE_CHECK_BOX_BOX,
        ".demo-template-check-box-box",
    );
    ui.set_class_debug_name(
        CLASS_DEMO_TEMPLATE_SLIDER_TRACK,
        ".demo-template-slider-track",
    );
    ui.set_class_debug_name(
        CLASS_DEMO_TEMPLATE_SLIDER_THUMB,
        ".demo-template-slider-thumb",
    );
    ui.set_class_debug_name(CLASS_OKLAB_SWATCH, ".oklab-swatch");
    ui.set_class_debug_name(CLASS_OPACITY_SWATCH, ".opacity-swatch");
    ui.set_class_debug_name(CLASS_BORDER_SWATCH, ".border-swatch");
    ui.set_class_debug_name(CLASS_RETARGET_SWATCH, ".retarget-swatch");
    ui.set_class_debug_name(CLASS_MOVER_SWATCH, ".mover-swatch");
    ui.set_class_debug_name(CLASS_SRGB_SWATCH, ".srgb-swatch");
}

pub(super) const EASE_OUT: TimingFunction = TimingFunction::cubic_bezier(0.16, 1.0, 0.3, 1.0);
pub(super) const EASE_IN_OUT: TimingFunction = TimingFunction::cubic_bezier(0.65, 0.0, 0.35, 1.0);
pub(super) const MID_SLOW: TimingFunction = TimingFunction::cubic_bezier(0.2, 0.48, 0.8, 0.52);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DemoThemeMode {
    Dark,
    Light,
}

impl DemoThemeMode {
    pub(super) fn toggled(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }

    pub(super) fn theme(self) -> understory_style::Theme {
        match self {
            Self::Dark => theme::dark_theme_builder()
                .set(DEMO_ROOT_BG, opt_solid(0x1e, 0x1e, 0x24, 0xff))
                .set(DEMO_NAV_BG, opt_solid(0x22, 0x23, 0x2b, 0xff))
                .set(DEMO_NAV_BORDER, opt_solid(0x3a, 0x3d, 0x4b, 0xff))
                .set(DEMO_NAV_SELECTED_BG, opt_solid(0x4d, 0x6f, 0xdc, 0xff))
                .set(DEMO_STAGE_BG, opt_solid(0x1b, 0x1c, 0x23, 0xff))
                .set(DEMO_STAGE_BORDER, opt_solid(0x2f, 0x32, 0x3d, 0xff))
                .set(DEMO_HEADER_BG, opt_solid(0x24, 0x25, 0x2e, 0xff))
                .set(DEMO_HEADER_BORDER, opt_solid(0x3b, 0x3e, 0x4c, 0xff))
                .set(DEMO_CARD_BG, opt_solid(0x27, 0x28, 0x31, 0xff))
                .set(DEMO_CARD_BORDER, opt_solid(0x3b, 0x3d, 0x49, 0xff))
                .set(DEMO_SPLIT_BORDER, opt_solid(0x48, 0x4c, 0x58, 0xff))
                .set(DEMO_SPLIT_SCENE_BG, opt_solid(0x31, 0x35, 0x43, 0xff))
                .set(DEMO_SPLIT_INSPECTOR_BG, opt_solid(0x2e, 0x3a, 0x35, 0xff))
                .set(DEMO_MOTION_STAGE_BG, opt_solid(0x27, 0x28, 0x31, 0xff))
                .set(DEMO_ACCENT_BG, opt_solid(0x59, 0x75, 0xe6, 0xff))
                .set(DEMO_TEXT_FG, solid(0xf4, 0xf4, 0xf7, 0xff))
                .set(DEMO_BODY_FG, solid(0xd8, 0xd9, 0xe2, 0xff))
                .set(DEMO_MUTED_FG, solid(0xc8, 0xc8, 0xd2, 0xff))
                .set(DEMO_TITLE_FG, solid(0x96, 0x9a, 0xa8, 0xff))
                .set(DEMO_FOCUS_BORDER, opt_solid(0xee, 0xef, 0xf8, 0xff))
                .build(),
            Self::Light => theme::light_theme_builder()
                .set(DEMO_ROOT_BG, opt_solid(0xf0, 0xf3, 0xf8, 0xff))
                .set(DEMO_NAV_BG, opt_solid(0xff, 0xff, 0xff, 0xff))
                .set(DEMO_NAV_BORDER, opt_solid(0xd2, 0xd8, 0xe5, 0xff))
                .set(DEMO_NAV_SELECTED_BG, opt_solid(0x2f, 0x62, 0xd6, 0xff))
                .set(DEMO_STAGE_BG, opt_solid(0xf7, 0xf9, 0xfc, 0xff))
                .set(DEMO_STAGE_BORDER, opt_solid(0xd8, 0xde, 0xea, 0xff))
                .set(DEMO_HEADER_BG, opt_solid(0xff, 0xff, 0xff, 0xff))
                .set(DEMO_HEADER_BORDER, opt_solid(0xda, 0xe0, 0xeb, 0xff))
                .set(DEMO_CARD_BG, opt_solid(0xff, 0xff, 0xff, 0xff))
                .set(DEMO_CARD_BORDER, opt_solid(0xd8, 0xde, 0xea, 0xff))
                .set(DEMO_SPLIT_BORDER, opt_solid(0xbf, 0xc7, 0xd5, 0xff))
                .set(DEMO_SPLIT_SCENE_BG, opt_solid(0xec, 0xf1, 0xfb, 0xff))
                .set(DEMO_SPLIT_INSPECTOR_BG, opt_solid(0xea, 0xf5, 0xef, 0xff))
                .set(DEMO_MOTION_STAGE_BG, opt_solid(0xf2, 0xf4, 0xf8, 0xff))
                .set(DEMO_ACCENT_BG, opt_solid(0x2f, 0x62, 0xd6, 0xff))
                .set(DEMO_TEXT_FG, solid(0x16, 0x1a, 0x24, 0xff))
                .set(DEMO_BODY_FG, solid(0x3a, 0x40, 0x4e, 0xff))
                .set(DEMO_MUTED_FG, solid(0x61, 0x68, 0x78, 0xff))
                .set(DEMO_TITLE_FG, solid(0x6a, 0x72, 0x83, 0xff))
                .set(DEMO_FOCUS_BORDER, opt_solid(0x1e, 0x53, 0xc8, 0xff))
                .build(),
        }
    }
}
pub(super) fn demo_cascade(properties: &BuiltInProperties) -> StyleCascade {
    theme::default_cascade_builder(properties)
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_DEMO_ROOT),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_ROOT_BG)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_DEMO_NAV),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_NAV_BG)
                .set_resource(properties.border_brush, DEMO_NAV_BORDER)
                .set(properties.border_width, 1.0_f64)
                .set(properties.corner_radius, 16.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_DEMO_STAGE),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_STAGE_BG)
                .set_resource(properties.border_brush, DEMO_STAGE_BORDER)
                .set(properties.border_width, 1.0_f64)
                .set(properties.corner_radius, 18.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_DEMO_HEADER),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_HEADER_BG)
                .set_resource(properties.border_brush, DEMO_HEADER_BORDER)
                .set(properties.border_width, 1.0_f64)
                .set(properties.corner_radius, 16.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_DEMO_CARD),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_CARD_BG)
                .set_resource(properties.border_brush, DEMO_CARD_BORDER)
                .set(properties.border_width, 1.0_f64)
                .set(properties.corner_radius, 12.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_DEMO_NAV_MARKER),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_ACCENT_BG)
                .set(properties.corner_radius, 2.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_DEMO_HEADER_ACCENT),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_ACCENT_BG)
                .set(properties.corner_radius, 8.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_DEMO_NAV_ROW),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_NAV_BG)
                .set_resource(properties.border_brush, DEMO_NAV_BG)
                .set(properties.border_width, 1.0_f64)
                .set(properties.corner_radius, 8.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(CLASS_DEMO_NAV_ROW, PSEUDO_SELECTED),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_NAV_SELECTED_BG)
                .set_resource(properties.border_brush, DEMO_NAV_SELECTED_BG)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(CLASS_DEMO_NAV_ROW, PSEUDO_FOCUS_VISIBLE),
            StyleBuilder::new()
                .set_resource(properties.border_brush, DEMO_FOCUS_BORDER)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_DEMO_SPLIT_SCENE),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_SPLIT_SCENE_BG)
                .set_resource(properties.border_brush, DEMO_SPLIT_BORDER)
                .set(properties.border_width, 1.0_f64)
                .set(properties.corner_radius, 8.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_DEMO_SPLIT_INSPECTOR),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_SPLIT_INSPECTOR_BG)
                .set_resource(properties.border_brush, DEMO_SPLIT_BORDER)
                .set(properties.border_width, 1.0_f64)
                .set(properties.corner_radius, 8.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_DEMO_MOTION_STAGE),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_MOTION_STAGE_BG)
                .set_resource(properties.border_brush, DEMO_MOTION_STAGE_BG)
                .set(properties.border_width, 1.0_f64)
                .set(properties.corner_radius, 14.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(CLASS_DEMO_HEADING),
            StyleBuilder::new()
                .set_resource(properties.foreground, DEMO_TEXT_FG)
                .set(properties.font_size, 26.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(CLASS_DEMO_PAGE_HEADING),
            StyleBuilder::new()
                .set_resource(properties.foreground, DEMO_TEXT_FG)
                .set(properties.font_size, 22.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(CLASS_DEMO_CAPTION),
            StyleBuilder::new()
                .set_resource(properties.foreground, DEMO_MUTED_FG)
                .set(properties.font_size, 13.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(CLASS_DEMO_CARD_TITLE),
            StyleBuilder::new()
                .set_resource(properties.foreground, DEMO_TITLE_FG)
                .set(properties.font_size, 12.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(CLASS_DEMO_CONTROL_LABEL),
            StyleBuilder::new()
                .set_resource(properties.foreground, DEMO_TEXT_FG)
                .set(properties.font_size, 15.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(CLASS_DEMO_NAV_LABEL),
            StyleBuilder::new()
                .set_resource(properties.foreground, DEMO_TEXT_FG)
                .set(properties.font_size, 15.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            selected_nav_label_selector(),
            StyleBuilder::new()
                .set_resource(properties.foreground, theme::ON_ACCENT_FG)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            button_class_state_selector(CLASS_DEVTOOLS_BUTTON, PSEUDO_SELECTED),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_ACCENT_BG)
                .set_resource(properties.border_brush, DEMO_ACCENT_BG)
                .set_resource(properties.foreground, theme::ON_ACCENT_FG)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            untyped_class_selector(CLASS_DEMO_TEMPLATE_BUTTON_OUTER),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0x27, 0x3b, 0x6f, 0xff))
                .set(properties.border_brush, opt_solid(0x90, 0xa7, 0xff, 0xff))
                .set(properties.border_width, 1.0_f64)
                .set(properties.corner_radius, 12.0_f64)
                .set(properties.layout_container, LayoutContainer::row())
                .set(properties.layout_spacing, LayoutSpacing::padding_all(3.0))
                .set(
                    properties.layout_alignment,
                    LayoutAlignment::new()
                        .justify_content(LayoutJustify::Center)
                        .align_items(LayoutAlign::Center),
                )
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            untyped_class_selector(CLASS_DEMO_TEMPLATE_BUTTON_INNER),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_ACCENT_BG)
                .set(properties.border_brush, opt_solid(0xff, 0xff, 0xff, 0x80))
                .set(properties.border_width, 1.0_f64)
                .set(properties.corner_radius, 9.0_f64)
                .set(properties.layout_container, LayoutContainer::row())
                .set(
                    properties.layout_spacing,
                    LayoutSpacing::padding_xy(12.0, 8.0),
                )
                .set(
                    properties.layout_alignment,
                    LayoutAlignment::new()
                        .justify_content(LayoutJustify::Center)
                        .align_items(LayoutAlign::Center),
                )
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            untyped_class_selector(CLASS_DEMO_TEMPLATE_CHECK_BOX_BOX),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_ACCENT_BG)
                .set_resource(properties.border_brush, DEMO_ACCENT_BG)
                .set(properties.border_width, 1.0_f64)
                .set(properties.corner_radius, 12.0_f64)
                .set(properties.layout_size, LayoutSize::logical(24.0, 24.0))
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            untyped_class_selector(CLASS_DEMO_TEMPLATE_SLIDER_TRACK),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_SPLIT_BORDER)
                .set(properties.border_brush, opt_solid(0xff, 0xff, 0xff, 0x40))
                .set(properties.border_width, 1.0_f64)
                .set(properties.corner_radius, 5.0_f64)
                .set(
                    properties.layout_placement,
                    LayoutPlacement::absolute_at(0.0, 22.0),
                )
                .set(properties.layout_size, LayoutSize::logical(220.0, 10.0))
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            untyped_class_selector(CLASS_DEMO_TEMPLATE_SLIDER_THUMB),
            StyleBuilder::new()
                .set_resource(properties.background, DEMO_ACCENT_BG)
                .set(properties.border_brush, opt_solid(0xff, 0xff, 0xff, 0xd0))
                .set(properties.border_width, 2.0_f64)
                .set(properties.corner_radius, 11.0_f64)
                .set(properties.layout_size, LayoutSize::logical(22.0, 22.0))
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(CLASS_DEMO_NUMBER),
            StyleBuilder::new()
                .set_resource(properties.foreground, DEMO_TEXT_FG)
                .set(properties.font_size, 22.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(CLASS_DEMO_LARGE_NUMBER),
            StyleBuilder::new()
                .set_resource(properties.foreground, DEMO_TEXT_FG)
                .set(properties.font_size, 34.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(CLASS_DEMO_BODY),
            StyleBuilder::new()
                .set_resource(properties.foreground, DEMO_BODY_FG)
                .set(properties.font_size, 14.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_OKLAB_SWATCH),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0xff, 0x2a, 0x78, 0xff))
                .set(properties.corner_radius, 18.0_f64)
                .set(
                    properties.transitions,
                    color_transition_rules(
                        properties,
                        millis(5_500),
                        MID_SLOW,
                        ColorInterpolation::OKLAB,
                    ),
                )
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(CLASS_OKLAB_SWATCH, PSEUDO_SELECTED),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0x26, 0xd9, 0x8e, 0xff))
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_SRGB_SWATCH),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0xff, 0x2a, 0x78, 0xff))
                .set(properties.corner_radius, 18.0_f64)
                .set(
                    properties.transitions,
                    color_transition_rules(
                        properties,
                        millis(5_500),
                        MID_SLOW,
                        ColorInterpolation::SRGB,
                    ),
                )
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(CLASS_SRGB_SWATCH, PSEUDO_SELECTED),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0x26, 0xd9, 0x8e, 0xff))
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_OPACITY_SWATCH),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0x6d, 0xf2, 0x96, 0xff))
                .set(properties.corner_radius, 18.0_f64)
                .set(properties.opacity, 1.0_f32)
                .set(
                    properties.transitions,
                    scalar_transition_rules(
                        TransitionTarget::property(properties.opacity.id()),
                        TransitionSpec::timed(millis(780), EASE_IN_OUT),
                    ),
                )
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(CLASS_OPACITY_SWATCH, PSEUDO_PRESSED),
            StyleBuilder::new()
                .set(properties.opacity, 0.16_f32)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_BORDER_SWATCH),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0x25, 0x29, 0x38, 0xff))
                .set(properties.border_brush, opt_solid(0x79, 0x86, 0xff, 0xff))
                .set(properties.border_width, 2.0_f64)
                .set(properties.corner_radius, 18.0_f64)
                .set(properties.transitions, border_transition_rules(properties))
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(CLASS_BORDER_SWATCH, PSEUDO_FOCUS_VISIBLE),
            StyleBuilder::new()
                .set(properties.border_brush, opt_solid(0xff, 0xff, 0xff, 0xff))
                .set(properties.border_width, 12.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_MOVER_SWATCH),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0xff, 0x9f, 0x1c, 0xff))
                .set(properties.corner_radius, 21.0_f64)
                .set(
                    properties.transitions,
                    scalar_transition_rules(
                        TransitionTarget::layer_offset(),
                        TransitionSpec::timed(millis(920), EASE_IN_OUT),
                    ),
                )
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(CLASS_RETARGET_SWATCH),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0xd8, 0xb4, 0xfe, 0xff))
                .set(properties.corner_radius, 18.0_f64)
                .set(
                    properties.transitions,
                    color_transition_rules(
                        properties,
                        millis(780),
                        EASE_OUT,
                        ColorInterpolation::OKLAB,
                    ),
                )
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(CLASS_RETARGET_SWATCH, PSEUDO_SELECTED),
            StyleBuilder::new()
                .set(properties.background, opt_solid(0xf9, 0xf8, 0x71, 0xff))
                .build(),
        )
        .build()
}
pub(super) fn class_selector(class: ClassId) -> Selector {
    Selector::single(SelectorStep::type_tag(TYPE_GENERIC).with_class(class))
}

pub(super) fn untyped_class_selector(class: ClassId) -> Selector {
    Selector::single(SelectorStep::class(class))
}

pub(super) fn text_class_selector(class: ClassId) -> Selector {
    Selector::single(SelectorStep::type_tag(TYPE_TEXT).with_class(class))
}

pub(super) fn button_class_state_selector(class: ClassId, pseudo: PseudoClassId) -> Selector {
    Selector::single(
        SelectorStep::type_tag(TYPE_BUTTON)
            .with_class(class)
            .with_pseudo(pseudo),
    )
}

pub(super) fn class_state_selector(class: ClassId, pseudo: PseudoClassId) -> Selector {
    Selector::single(
        SelectorStep::type_tag(TYPE_GENERIC)
            .with_class(class)
            .with_pseudo(pseudo),
    )
}

pub(super) fn selected_nav_label_selector() -> Selector {
    Selector::from([
        SelectorStep::type_tag(TYPE_GENERIC)
            .with_class(CLASS_DEMO_NAV_ROW)
            .with_pseudo(PSEUDO_SELECTED),
        SelectorStep::type_tag(TYPE_TEXT).with_class(CLASS_DEMO_NAV_LABEL),
    ])
}

pub(super) fn color_transition_rules(
    properties: &BuiltInProperties,
    duration: TimerDuration,
    timing: TimingFunction,
    interpolation: ColorInterpolation,
) -> TransitionRules {
    TransitionRules::single(TransitionRule::interaction(
        TransitionTarget::property(properties.background.id()),
        TransitionSpec::timed(duration, timing).with_color_interpolation(interpolation),
    ))
}

pub(super) fn border_transition_rules(properties: &BuiltInProperties) -> TransitionRules {
    TransitionRules::new()
        .with_rule(TransitionRule::interaction(
            TransitionTarget::property(properties.border_brush.id()),
            TransitionSpec::timed(millis(720), EASE_OUT)
                .with_color_interpolation(ColorInterpolation::OKLAB),
        ))
        .with_rule(TransitionRule::interaction(
            TransitionTarget::property(properties.border_width.id()),
            TransitionSpec::timed(millis(520), EASE_IN_OUT),
        ))
}

pub(super) fn scalar_transition_rules(
    target: TransitionTarget,
    spec: TransitionSpec,
) -> TransitionRules {
    TransitionRules::single(TransitionRule::interaction(target, spec))
}
