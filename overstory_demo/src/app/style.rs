// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

pub(super) const RESOURCE_DEMO_ROOT_BG: &str = "demo.root.bg";
pub(super) const RESOURCE_DEMO_NAV_BG: &str = "demo.nav.bg";
pub(super) const RESOURCE_DEMO_NAV_BORDER: &str = "demo.nav.border";
pub(super) const RESOURCE_DEMO_NAV_SELECTED_BG: &str = "demo.nav.selected.bg";
pub(super) const RESOURCE_DEMO_STAGE_BG: &str = "demo.stage.bg";
pub(super) const RESOURCE_DEMO_STAGE_BORDER: &str = "demo.stage.border";
pub(super) const RESOURCE_DEMO_HEADER_BG: &str = "demo.header.bg";
pub(super) const RESOURCE_DEMO_HEADER_BORDER: &str = "demo.header.border";
pub(super) const RESOURCE_DEMO_CARD_BG: &str = "demo.card.bg";
pub(super) const RESOURCE_DEMO_CARD_BORDER: &str = "demo.card.border";
pub(super) const RESOURCE_DEMO_SPLIT_BORDER: &str = "demo.split.border";
pub(super) const RESOURCE_DEMO_SPLIT_SCENE_BG: &str = "demo.split.scene.bg";
pub(super) const RESOURCE_DEMO_SPLIT_INSPECTOR_BG: &str = "demo.split.inspector.bg";
pub(super) const RESOURCE_DEMO_MOTION_STAGE_BG: &str = "demo.motion.stage.bg";
pub(super) const RESOURCE_DEMO_ACCENT_BG: &str = "demo.accent.bg";
pub(super) const RESOURCE_DEMO_TEXT_FG: &str = "demo.text.fg";
pub(super) const RESOURCE_DEMO_BODY_FG: &str = "demo.body.fg";
pub(super) const RESOURCE_DEMO_MUTED_FG: &str = "demo.muted.fg";
pub(super) const RESOURCE_DEMO_TITLE_FG: &str = "demo.title.fg";
pub(super) const RESOURCE_DEMO_FOCUS_BORDER: &str = "demo.focus.border";

pub(super) const CLASS_DEMO_ROOT: &str = ".demo-root";
pub(super) const CLASS_DEMO_NAV: &str = ".demo-nav";
pub(super) const CLASS_DEMO_NAV_MARKER: &str = ".demo-nav-marker";
pub(super) const CLASS_DEMO_STAGE: &str = ".demo-stage";
pub(super) const CLASS_DEMO_HEADER: &str = ".demo-header";
pub(super) const CLASS_DEMO_HEADER_ACCENT: &str = ".demo-header-accent";
pub(super) const CLASS_DEMO_CARD: &str = ".demo-card";
pub(super) const CLASS_DEMO_NAV_ROW: &str = ".demo-nav-row";
pub(super) const CLASS_DEMO_NAV_LABEL: &str = ".demo-nav-label";
pub(super) const CLASS_DEMO_HEADING: &str = ".demo-heading";
pub(super) const CLASS_DEMO_PAGE_HEADING: &str = ".demo-page-heading";
pub(super) const CLASS_DEMO_CAPTION: &str = ".demo-caption";
pub(super) const CLASS_DEMO_CARD_TITLE: &str = ".demo-card-title";
pub(super) const CLASS_DEMO_CONTROL_LABEL: &str = ".demo-control-label";
pub(super) const CLASS_DEMO_NUMBER: &str = ".demo-number";
pub(super) const CLASS_DEMO_LARGE_NUMBER: &str = ".demo-large-number";
pub(super) const CLASS_DEMO_BODY: &str = ".demo-body";
pub(super) const CLASS_DEMO_SPLIT_SCENE: &str = ".demo-split-scene";
pub(super) const CLASS_DEMO_SPLIT_INSPECTOR: &str = ".demo-split-inspector";
pub(super) const CLASS_DEMO_MOTION_STAGE: &str = ".demo-motion-stage";
pub(super) const CLASS_SVG_GRID_LINE: &str = ".svg-grid-line";
pub(super) const CLASS_SVG_INFO_CARD: &str = ".svg-info-card";
pub(super) const CLASS_SVG_TIGER_CARD: &str = ".svg-tiger-card";
pub(super) const CLASS_SVG_INSPECTOR_CARD: &str = ".svg-inspector-card";
pub(super) const CLASS_DEVTOOLS_INSPECTOR_CARD: &str = ".devtools-inspector-card";
pub(super) const CLASS_DEVTOOLS_STATUS: &str = ".devtools-status";
pub(super) const CLASS_DEVTOOLS_BUTTON: &str = ".devtools-button";
pub(super) const CLASS_DEMO_TEMPLATE_BUTTON_OUTER: &str = ".demo-template-button-outer";
pub(super) const CLASS_DEMO_TEMPLATE_BUTTON_INNER: &str = ".demo-template-button-inner";
pub(super) const CLASS_DEMO_TEMPLATE_CHECK_BOX_BOX: &str = ".demo-template-check-box-box";
pub(super) const CLASS_DEMO_TEMPLATE_SLIDER_TRACK: &str = ".demo-template-slider-track";
pub(super) const CLASS_DEMO_TEMPLATE_SLIDER_THUMB: &str = ".demo-template-slider-thumb";
pub(super) const CLASS_DEMO_TRANSCRIPT_VIEWPORT: &str = ".demo-transcript-viewport";
pub(super) const CLASS_DEMO_TRANSCRIPT_COMPOSER_INPUT: &str = ".demo-transcript-composer-input";
pub(super) const CLASS_DEVTOOLS_EVENT_LOG: &str = ".devtools-event-log";
pub(super) const CLASS_FOREGROUND_SWATCH: &str = ".foreground-swatch";
pub(super) const CLASS_OKLAB_SWATCH: &str = ".oklab-swatch";
pub(super) const CLASS_OPACITY_SWATCH: &str = ".opacity-swatch";
pub(super) const CLASS_BORDER_SWATCH: &str = ".border-swatch";
pub(super) const CLASS_RETARGET_SWATCH: &str = ".retarget-swatch";
pub(super) const CLASS_MOVER_SWATCH: &str = ".mover-swatch";
pub(super) const CLASS_SRGB_SWATCH: &str = ".srgb-swatch";

#[derive(Clone, Copy, Debug)]
pub(super) struct DemoTokens {
    pub(super) classes: DemoClasses,
    pub(super) resources: DemoResources,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct DemoClasses {
    pub(super) demo_root: ClassId,
    pub(super) demo_nav: ClassId,
    pub(super) demo_nav_marker: ClassId,
    pub(super) demo_stage: ClassId,
    pub(super) demo_header: ClassId,
    pub(super) demo_header_accent: ClassId,
    pub(super) demo_card: ClassId,
    pub(super) demo_nav_row: ClassId,
    pub(super) demo_nav_label: ClassId,
    pub(super) demo_heading: ClassId,
    pub(super) demo_page_heading: ClassId,
    pub(super) demo_caption: ClassId,
    pub(super) demo_card_title: ClassId,
    pub(super) demo_control_label: ClassId,
    pub(super) demo_number: ClassId,
    pub(super) demo_large_number: ClassId,
    pub(super) demo_body: ClassId,
    pub(super) demo_split_scene: ClassId,
    pub(super) demo_split_inspector: ClassId,
    pub(super) demo_motion_stage: ClassId,
    pub(super) devtools_button: ClassId,
    pub(super) demo_template_button_outer: ClassId,
    pub(super) demo_template_button_inner: ClassId,
    pub(super) demo_template_check_box_box: ClassId,
    pub(super) demo_template_slider_track: ClassId,
    pub(super) demo_template_slider_thumb: ClassId,
    pub(super) foreground_swatch: ClassId,
    pub(super) oklab_swatch: ClassId,
    pub(super) opacity_swatch: ClassId,
    pub(super) border_swatch: ClassId,
    pub(super) retarget_swatch: ClassId,
    pub(super) mover_swatch: ClassId,
    pub(super) srgb_swatch: ClassId,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct DemoResources {
    pub(super) root_bg: ResourceKey,
    pub(super) nav_bg: ResourceKey,
    pub(super) nav_border: ResourceKey,
    pub(super) nav_selected_bg: ResourceKey,
    pub(super) stage_bg: ResourceKey,
    pub(super) stage_border: ResourceKey,
    pub(super) header_bg: ResourceKey,
    pub(super) header_border: ResourceKey,
    pub(super) card_bg: ResourceKey,
    pub(super) card_border: ResourceKey,
    pub(super) split_border: ResourceKey,
    pub(super) split_scene_bg: ResourceKey,
    pub(super) split_inspector_bg: ResourceKey,
    pub(super) motion_stage_bg: ResourceKey,
    pub(super) accent_bg: ResourceKey,
    pub(super) text_fg: ResourceKey,
    pub(super) body_fg: ResourceKey,
    pub(super) muted_fg: ResourceKey,
    pub(super) title_fg: ResourceKey,
    pub(super) focus_border: ResourceKey,
}

pub(super) struct DemoStyleTokens;

impl overstory::StyleTokenSet for DemoStyleTokens {
    type Resolved = DemoTokens;

    fn resolve(vocabulary: &mut overstory::StyleVocabulary) -> Self::Resolved {
        DemoTokens {
            classes: DemoClasses {
                demo_root: vocabulary.class_id(CLASS_DEMO_ROOT),
                demo_nav: vocabulary.class_id(CLASS_DEMO_NAV),
                demo_nav_marker: vocabulary.class_id(CLASS_DEMO_NAV_MARKER),
                demo_stage: vocabulary.class_id(CLASS_DEMO_STAGE),
                demo_header: vocabulary.class_id(CLASS_DEMO_HEADER),
                demo_header_accent: vocabulary.class_id(CLASS_DEMO_HEADER_ACCENT),
                demo_card: vocabulary.class_id(CLASS_DEMO_CARD),
                demo_nav_row: vocabulary.class_id(CLASS_DEMO_NAV_ROW),
                demo_nav_label: vocabulary.class_id(CLASS_DEMO_NAV_LABEL),
                demo_heading: vocabulary.class_id(CLASS_DEMO_HEADING),
                demo_page_heading: vocabulary.class_id(CLASS_DEMO_PAGE_HEADING),
                demo_caption: vocabulary.class_id(CLASS_DEMO_CAPTION),
                demo_card_title: vocabulary.class_id(CLASS_DEMO_CARD_TITLE),
                demo_control_label: vocabulary.class_id(CLASS_DEMO_CONTROL_LABEL),
                demo_number: vocabulary.class_id(CLASS_DEMO_NUMBER),
                demo_large_number: vocabulary.class_id(CLASS_DEMO_LARGE_NUMBER),
                demo_body: vocabulary.class_id(CLASS_DEMO_BODY),
                demo_split_scene: vocabulary.class_id(CLASS_DEMO_SPLIT_SCENE),
                demo_split_inspector: vocabulary.class_id(CLASS_DEMO_SPLIT_INSPECTOR),
                demo_motion_stage: vocabulary.class_id(CLASS_DEMO_MOTION_STAGE),
                devtools_button: vocabulary.class_id(CLASS_DEVTOOLS_BUTTON),
                demo_template_button_outer: vocabulary.class_id(CLASS_DEMO_TEMPLATE_BUTTON_OUTER),
                demo_template_button_inner: vocabulary.class_id(CLASS_DEMO_TEMPLATE_BUTTON_INNER),
                demo_template_check_box_box: vocabulary.class_id(CLASS_DEMO_TEMPLATE_CHECK_BOX_BOX),
                demo_template_slider_track: vocabulary.class_id(CLASS_DEMO_TEMPLATE_SLIDER_TRACK),
                demo_template_slider_thumb: vocabulary.class_id(CLASS_DEMO_TEMPLATE_SLIDER_THUMB),
                foreground_swatch: vocabulary.class_id(CLASS_FOREGROUND_SWATCH),
                oklab_swatch: vocabulary.class_id(CLASS_OKLAB_SWATCH),
                opacity_swatch: vocabulary.class_id(CLASS_OPACITY_SWATCH),
                border_swatch: vocabulary.class_id(CLASS_BORDER_SWATCH),
                retarget_swatch: vocabulary.class_id(CLASS_RETARGET_SWATCH),
                mover_swatch: vocabulary.class_id(CLASS_MOVER_SWATCH),
                srgb_swatch: vocabulary.class_id(CLASS_SRGB_SWATCH),
            },
            resources: DemoResources {
                root_bg: vocabulary.resource_key(RESOURCE_DEMO_ROOT_BG),
                nav_bg: vocabulary.resource_key(RESOURCE_DEMO_NAV_BG),
                nav_border: vocabulary.resource_key(RESOURCE_DEMO_NAV_BORDER),
                nav_selected_bg: vocabulary.resource_key(RESOURCE_DEMO_NAV_SELECTED_BG),
                stage_bg: vocabulary.resource_key(RESOURCE_DEMO_STAGE_BG),
                stage_border: vocabulary.resource_key(RESOURCE_DEMO_STAGE_BORDER),
                header_bg: vocabulary.resource_key(RESOURCE_DEMO_HEADER_BG),
                header_border: vocabulary.resource_key(RESOURCE_DEMO_HEADER_BORDER),
                card_bg: vocabulary.resource_key(RESOURCE_DEMO_CARD_BG),
                card_border: vocabulary.resource_key(RESOURCE_DEMO_CARD_BORDER),
                split_border: vocabulary.resource_key(RESOURCE_DEMO_SPLIT_BORDER),
                split_scene_bg: vocabulary.resource_key(RESOURCE_DEMO_SPLIT_SCENE_BG),
                split_inspector_bg: vocabulary.resource_key(RESOURCE_DEMO_SPLIT_INSPECTOR_BG),
                motion_stage_bg: vocabulary.resource_key(RESOURCE_DEMO_MOTION_STAGE_BG),
                accent_bg: vocabulary.resource_key(RESOURCE_DEMO_ACCENT_BG),
                text_fg: vocabulary.resource_key(RESOURCE_DEMO_TEXT_FG),
                body_fg: vocabulary.resource_key(RESOURCE_DEMO_BODY_FG),
                muted_fg: vocabulary.resource_key(RESOURCE_DEMO_MUTED_FG),
                title_fg: vocabulary.resource_key(RESOURCE_DEMO_TITLE_FG),
                focus_border: vocabulary.resource_key(RESOURCE_DEMO_FOCUS_BORDER),
            },
        }
    }
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

    pub(super) fn theme(self, resources: DemoResources) -> understory_style::Theme {
        match self {
            Self::Dark => theme::dark_theme_builder()
                .set(resources.root_bg, opt_solid(0x1e, 0x1e, 0x24, 0xff))
                .set(resources.nav_bg, opt_solid(0x22, 0x23, 0x2b, 0xff))
                .set(resources.nav_border, opt_solid(0x3a, 0x3d, 0x4b, 0xff))
                .set(resources.nav_selected_bg, opt_solid(0x4d, 0x6f, 0xdc, 0xff))
                .set(resources.stage_bg, opt_solid(0x1b, 0x1c, 0x23, 0xff))
                .set(resources.stage_border, opt_solid(0x2f, 0x32, 0x3d, 0xff))
                .set(resources.header_bg, opt_solid(0x24, 0x25, 0x2e, 0xff))
                .set(resources.header_border, opt_solid(0x3b, 0x3e, 0x4c, 0xff))
                .set(resources.card_bg, opt_solid(0x27, 0x28, 0x31, 0xff))
                .set(resources.card_border, opt_solid(0x3b, 0x3d, 0x49, 0xff))
                .set(resources.split_border, opt_solid(0x48, 0x4c, 0x58, 0xff))
                .set(resources.split_scene_bg, opt_solid(0x31, 0x35, 0x43, 0xff))
                .set(
                    resources.split_inspector_bg,
                    opt_solid(0x2e, 0x3a, 0x35, 0xff),
                )
                .set(resources.motion_stage_bg, opt_solid(0x27, 0x28, 0x31, 0xff))
                .set(resources.accent_bg, opt_solid(0x59, 0x75, 0xe6, 0xff))
                .set(resources.text_fg, solid(0xf4, 0xf4, 0xf7, 0xff))
                .set(resources.body_fg, solid(0xd8, 0xd9, 0xe2, 0xff))
                .set(resources.muted_fg, solid(0xc8, 0xc8, 0xd2, 0xff))
                .set(resources.title_fg, solid(0x96, 0x9a, 0xa8, 0xff))
                .set(resources.focus_border, opt_solid(0xee, 0xef, 0xf8, 0xff))
                .build(),
            Self::Light => theme::light_theme_builder()
                .set(resources.root_bg, opt_solid(0xf0, 0xf3, 0xf8, 0xff))
                .set(resources.nav_bg, opt_solid(0xff, 0xff, 0xff, 0xff))
                .set(resources.nav_border, opt_solid(0xd2, 0xd8, 0xe5, 0xff))
                .set(resources.nav_selected_bg, opt_solid(0x2f, 0x62, 0xd6, 0xff))
                .set(resources.stage_bg, opt_solid(0xf7, 0xf9, 0xfc, 0xff))
                .set(resources.stage_border, opt_solid(0xd8, 0xde, 0xea, 0xff))
                .set(resources.header_bg, opt_solid(0xff, 0xff, 0xff, 0xff))
                .set(resources.header_border, opt_solid(0xda, 0xe0, 0xeb, 0xff))
                .set(resources.card_bg, opt_solid(0xff, 0xff, 0xff, 0xff))
                .set(resources.card_border, opt_solid(0xd8, 0xde, 0xea, 0xff))
                .set(resources.split_border, opt_solid(0xbf, 0xc7, 0xd5, 0xff))
                .set(resources.split_scene_bg, opt_solid(0xec, 0xf1, 0xfb, 0xff))
                .set(
                    resources.split_inspector_bg,
                    opt_solid(0xea, 0xf5, 0xef, 0xff),
                )
                .set(resources.motion_stage_bg, opt_solid(0xf2, 0xf4, 0xf8, 0xff))
                .set(resources.accent_bg, opt_solid(0x2f, 0x62, 0xd6, 0xff))
                .set(resources.text_fg, solid(0x16, 0x1a, 0x24, 0xff))
                .set(resources.body_fg, solid(0x3a, 0x40, 0x4e, 0xff))
                .set(resources.muted_fg, solid(0x61, 0x68, 0x78, 0xff))
                .set(resources.title_fg, solid(0x6a, 0x72, 0x83, 0xff))
                .set(resources.focus_border, opt_solid(0x1e, 0x53, 0xc8, 0xff))
                .build(),
        }
    }
}
pub(super) fn demo_cascade(properties: &BuiltInProperties, tokens: DemoTokens) -> StyleCascade {
    let classes = tokens.classes;
    let resources = tokens.resources;
    StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            class_selector(classes.demo_root),
            StyleBuilder::new()
                .background_resource(properties, resources.root_bg)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(classes.demo_nav),
            StyleBuilder::new()
                .background_resource(properties, resources.nav_bg)
                .border_brush_resource(properties, resources.nav_border)
                .border_width(properties, 1.0_f64)
                .corner_radius(properties, 16.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(classes.demo_stage),
            StyleBuilder::new()
                .background_resource(properties, resources.stage_bg)
                .border_brush_resource(properties, resources.stage_border)
                .border_width(properties, 1.0_f64)
                .corner_radius(properties, 18.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(classes.demo_header),
            StyleBuilder::new()
                .background_resource(properties, resources.header_bg)
                .border_brush_resource(properties, resources.header_border)
                .border_width(properties, 1.0_f64)
                .corner_radius(properties, 16.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(classes.demo_card),
            StyleBuilder::new()
                .background_resource(properties, resources.card_bg)
                .border_brush_resource(properties, resources.card_border)
                .border_width(properties, 1.0_f64)
                .corner_radius(properties, 12.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(classes.demo_nav_marker),
            StyleBuilder::new()
                .background_resource(properties, resources.accent_bg)
                .corner_radius(properties, 2.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(classes.demo_header_accent),
            StyleBuilder::new()
                .background_resource(properties, resources.accent_bg)
                .corner_radius(properties, 8.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(classes.demo_nav_row),
            StyleBuilder::new()
                .background_resource(properties, resources.nav_bg)
                .border_brush_resource(properties, resources.nav_bg)
                .border_width(properties, 1.0_f64)
                .corner_radius(properties, 8.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(classes.demo_nav_row, PSEUDO_SELECTED),
            StyleBuilder::new()
                .background_resource(properties, resources.nav_selected_bg)
                .border_brush_resource(properties, resources.nav_selected_bg)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(classes.demo_nav_row, PSEUDO_FOCUS_VISIBLE),
            StyleBuilder::new()
                .border_brush_resource(properties, resources.focus_border)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(classes.demo_split_scene),
            StyleBuilder::new()
                .background_resource(properties, resources.split_scene_bg)
                .border_brush_resource(properties, resources.split_border)
                .border_width(properties, 1.0_f64)
                .corner_radius(properties, 8.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(classes.demo_split_inspector),
            StyleBuilder::new()
                .background_resource(properties, resources.split_inspector_bg)
                .border_brush_resource(properties, resources.split_border)
                .border_width(properties, 1.0_f64)
                .corner_radius(properties, 8.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(classes.demo_motion_stage),
            StyleBuilder::new()
                .background_resource(properties, resources.motion_stage_bg)
                .border_brush_resource(properties, resources.motion_stage_bg)
                .border_width(properties, 1.0_f64)
                .corner_radius(properties, 14.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(classes.demo_heading),
            StyleBuilder::new()
                .set_resource(properties.foreground, resources.text_fg)
                .set(properties.font_size, 26.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(classes.demo_page_heading),
            StyleBuilder::new()
                .set_resource(properties.foreground, resources.text_fg)
                .set(properties.font_size, 22.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(classes.demo_caption),
            StyleBuilder::new()
                .set_resource(properties.foreground, resources.muted_fg)
                .set(properties.font_size, 13.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(classes.demo_card_title),
            StyleBuilder::new()
                .set_resource(properties.foreground, resources.title_fg)
                .set(properties.font_size, 12.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(classes.demo_control_label),
            StyleBuilder::new()
                .set_resource(properties.foreground, resources.text_fg)
                .set(properties.font_size, 15.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(classes.demo_nav_label),
            StyleBuilder::new()
                .set_resource(properties.foreground, resources.text_fg)
                .set(properties.font_size, 15.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            selected_nav_label_selector(classes),
            StyleBuilder::new()
                .set_resource(properties.foreground, theme::ON_ACCENT_FG)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            button_class_state_selector(classes.devtools_button, PSEUDO_SELECTED),
            StyleBuilder::new()
                .background_resource(properties, resources.accent_bg)
                .border_brush_resource(properties, resources.accent_bg)
                .set_resource(properties.foreground, theme::ON_ACCENT_FG)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            untyped_class_selector(classes.demo_template_button_outer),
            StyleBuilder::new()
                .background(properties, opt_solid(0x27, 0x3b, 0x6f, 0xff))
                .border_brush(properties, opt_solid(0x90, 0xa7, 0xff, 0xff))
                .border_width(properties, 1.0_f64)
                .corner_radius(properties, 12.0_f64)
                .set(properties.layout_container, LayoutContainer::row())
                .padding_all(properties, 3.0)
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
            untyped_class_selector(classes.demo_template_button_inner),
            StyleBuilder::new()
                .background_resource(properties, resources.accent_bg)
                .border_brush(properties, opt_solid(0xff, 0xff, 0xff, 0x80))
                .border_width(properties, 1.0_f64)
                .corner_radius(properties, 9.0_f64)
                .set(properties.layout_container, LayoutContainer::row())
                .padding_xy(properties, 12.0, 8.0)
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
            untyped_class_selector(classes.demo_template_check_box_box),
            StyleBuilder::new()
                .background_resource(properties, resources.accent_bg)
                .border_brush_resource(properties, resources.accent_bg)
                .border_width(properties, 1.0_f64)
                .corner_radius(properties, 12.0_f64)
                .set(properties.layout_size, LayoutSize::logical(24.0, 24.0))
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            untyped_class_selector(classes.demo_template_slider_track),
            StyleBuilder::new()
                .background_resource(properties, resources.split_border)
                .border_brush(properties, opt_solid(0xff, 0xff, 0xff, 0x40))
                .border_width(properties, 1.0_f64)
                .corner_radius(properties, 5.0_f64)
                .set(
                    properties.layout_placement,
                    LayoutPlacement::absolute_at(0.0, 22.0),
                )
                .set(properties.layout_size, LayoutSize::logical(220.0, 10.0))
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            untyped_class_selector(classes.demo_template_slider_thumb),
            StyleBuilder::new()
                .background_resource(properties, resources.accent_bg)
                .border_brush(properties, opt_solid(0xff, 0xff, 0xff, 0xd0))
                .border_width(properties, 2.0_f64)
                .corner_radius(properties, 11.0_f64)
                .set(properties.layout_size, LayoutSize::logical(22.0, 22.0))
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(classes.demo_number),
            StyleBuilder::new()
                .set_resource(properties.foreground, resources.text_fg)
                .set(properties.font_size, 22.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(classes.demo_large_number),
            StyleBuilder::new()
                .set_resource(properties.foreground, resources.text_fg)
                .set(properties.font_size, 34.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(classes.demo_body),
            StyleBuilder::new()
                .set_resource(properties.foreground, resources.body_fg)
                .set(properties.font_size, 14.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_selector(classes.foreground_swatch),
            StyleBuilder::new()
                .set_resource(properties.foreground, resources.muted_fg)
                .set(properties.font_size, 22.0_f64)
                .set(
                    properties.transitions,
                    scalar_transition_rules(
                        PresentationTarget::property(properties.foreground.id()),
                        TransitionSpec::timed(millis(820), EASE_OUT)
                            .with_color_interpolation(ColorInterpolation::OKLAB),
                    ),
                )
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            text_class_state_selector(classes.foreground_swatch, PSEUDO_SELECTED),
            StyleBuilder::new()
                .set(properties.foreground, solid(0x2f, 0x62, 0xd6, 0xff))
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(classes.oklab_swatch),
            StyleBuilder::new()
                .background(properties, opt_solid(0xff, 0x2a, 0x78, 0xff))
                .corner_radius(properties, 18.0_f64)
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
            class_state_selector(classes.oklab_swatch, PSEUDO_SELECTED),
            StyleBuilder::new()
                .background(properties, opt_solid(0x26, 0xd9, 0x8e, 0xff))
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(classes.srgb_swatch),
            StyleBuilder::new()
                .background(properties, opt_solid(0xff, 0x2a, 0x78, 0xff))
                .corner_radius(properties, 18.0_f64)
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
            class_state_selector(classes.srgb_swatch, PSEUDO_SELECTED),
            StyleBuilder::new()
                .background(properties, opt_solid(0x26, 0xd9, 0x8e, 0xff))
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(classes.opacity_swatch),
            StyleBuilder::new()
                .background(properties, opt_solid(0x6d, 0xf2, 0x96, 0xff))
                .corner_radius(properties, 18.0_f64)
                .set(properties.opacity, 1.0_f32)
                .set(
                    properties.transitions,
                    scalar_transition_rules(
                        PresentationTarget::property(properties.opacity.id()),
                        TransitionSpec::timed(millis(780), EASE_IN_OUT),
                    ),
                )
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(classes.opacity_swatch, PSEUDO_PRESSED),
            StyleBuilder::new()
                .set(properties.opacity, 0.16_f32)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(classes.border_swatch),
            StyleBuilder::new()
                .background(properties, opt_solid(0x25, 0x29, 0x38, 0xff))
                .border_brush(properties, opt_solid(0x79, 0x86, 0xff, 0xff))
                .border_width(properties, 2.0_f64)
                .corner_radius(properties, 18.0_f64)
                .set(properties.transitions, border_transition_rules(properties))
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_state_selector(classes.border_swatch, PSEUDO_FOCUS_VISIBLE),
            StyleBuilder::new()
                .border_brush(properties, opt_solid(0xff, 0xff, 0xff, 0xff))
                .border_width(properties, 12.0_f64)
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(classes.mover_swatch),
            StyleBuilder::new()
                .background(properties, opt_solid(0xff, 0x9f, 0x1c, 0xff))
                .corner_radius(properties, 21.0_f64)
                .set(
                    properties.transitions,
                    scalar_transition_rules(
                        PresentationTarget::layer_offset(),
                        TransitionSpec::timed(millis(920), EASE_IN_OUT),
                    ),
                )
                .build(),
        )
        .push_rule(
            StyleOrigin::Base,
            class_selector(classes.retarget_swatch),
            StyleBuilder::new()
                .background(properties, opt_solid(0xd8, 0xb4, 0xfe, 0xff))
                .corner_radius(properties, 18.0_f64)
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
            class_state_selector(classes.retarget_swatch, PSEUDO_SELECTED),
            StyleBuilder::new()
                .background(properties, opt_solid(0xf9, 0xf8, 0x71, 0xff))
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

pub(super) fn text_class_state_selector(class: ClassId, pseudo: PseudoClassId) -> Selector {
    Selector::single(
        SelectorStep::type_tag(TYPE_TEXT)
            .with_class(class)
            .with_pseudo(pseudo),
    )
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

pub(super) fn selected_nav_label_selector(classes: DemoClasses) -> Selector {
    Selector::from([
        SelectorStep::type_tag(TYPE_GENERIC)
            .with_class(classes.demo_nav_row)
            .with_pseudo(PSEUDO_SELECTED),
        SelectorStep::type_tag(TYPE_TEXT).with_class(classes.demo_nav_label),
    ])
}

pub(super) fn color_transition_rules(
    properties: &BuiltInProperties,
    duration: TimerDuration,
    timing: TimingFunction,
    interpolation: ColorInterpolation,
) -> TransitionRules {
    TransitionRules::single(TransitionRule::interaction(
        PresentationTarget::property(properties.surface.background.id()),
        TransitionSpec::timed(duration, timing).with_color_interpolation(interpolation),
    ))
}

pub(super) fn border_transition_rules(properties: &BuiltInProperties) -> TransitionRules {
    let brushes = properties.surface.border_brushes;
    let widths = properties.surface.border_widths;
    let mut rules = TransitionRules::new();
    for property in [brushes.top, brushes.right, brushes.bottom, brushes.left] {
        rules.push(TransitionRule::interaction(
            PresentationTarget::property(property.id()),
            TransitionSpec::timed(millis(720), EASE_OUT)
                .with_color_interpolation(ColorInterpolation::OKLAB),
        ));
    }
    for property in [widths.top, widths.right, widths.bottom, widths.left] {
        rules.push(TransitionRule::interaction(
            PresentationTarget::property(property.id()),
            TransitionSpec::timed(millis(520), EASE_IN_OUT),
        ));
    }
    rules
}

pub(super) fn scalar_transition_rules(
    target: PresentationTarget,
    spec: TransitionSpec,
) -> TransitionRules {
    TransitionRules::single(TransitionRule::interaction(target, spec))
}
