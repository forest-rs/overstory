// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Sightline theme tokens and stylesheet.
//!
//! This module owns Sightline's app-specific visual vocabulary. It explicitly
//! does not own Overstory's built-in control theme contract.

use overstory::{BuiltInProperties, StyleTokenSet, StyleVocabulary, SurfaceStyleExt, theme};
use peniko::{Brush, Color};
use understory_style::{
    ClassId, ResourceKey, Selector, SelectorStep, StyleBuilder, StyleCascade, StyleCascadeBuilder,
    StyleOrigin, Theme, ThemeBuilder,
};

pub(crate) const COLOR_ACTIVITY: Color = Color::from_rgba8(0x45, 0xc7, 0xf8, 0xff);
pub(crate) const COLOR_SUCCESS: Color = Color::from_rgba8(0x41, 0xc2, 0x7a, 0xff);
pub(crate) const COLOR_WARNING: Color = Color::from_rgba8(0xf2, 0xb3, 0x3d, 0xff);
pub(crate) const COLOR_TIMING: Color = Color::from_rgba8(0x8d, 0x6b, 0xff, 0xff);
pub(crate) const COLOR_METADATA: Color = Color::from_rgba8(0x4c, 0x6f, 0xae, 0xff);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SightlineThemeMode {
    Graphite,
    Blueprint,
}

impl SightlineThemeMode {
    pub(crate) const fn toggled(self) -> Self {
        match self {
            Self::Graphite => Self::Blueprint,
            Self::Blueprint => Self::Graphite,
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Graphite => "Graphite",
            Self::Blueprint => "Blueprint",
        }
    }

    fn palette(self) -> Palette {
        match self {
            Self::Graphite => Palette::GRAPHITE.clone(),
            Self::Blueprint => Palette::BLUEPRINT.clone(),
        }
    }
}

#[derive(Clone, Debug)]
struct Palette {
    canvas: Brush,
    elevated: Brush,
    panel: Brush,
    instrument: Brush,
    rule: Brush,
    border: Brush,
    text: Brush,
    secondary: Brush,
    muted: Brush,
    activity: Brush,
    success: Brush,
    warning: Brush,
    timing: Brush,
    metadata: Brush,
    on_activity: Brush,
}

impl Palette {
    const GRAPHITE: Self = Self {
        canvas: Brush::Solid(Color::from_rgba8(0x0f, 0x12, 0x17, 0xff)),
        elevated: Brush::Solid(Color::from_rgba8(0x15, 0x1a, 0x20, 0xff)),
        panel: Brush::Solid(Color::from_rgba8(0x1b, 0x21, 0x28, 0xff)),
        instrument: Brush::Solid(Color::from_rgba8(0x11, 0x15, 0x1c, 0xff)),
        rule: Brush::Solid(Color::from_rgba8(0x23, 0x2b, 0x35, 0xff)),
        border: Brush::Solid(Color::from_rgba8(0x2b, 0x34, 0x40, 0xff)),
        text: Brush::Solid(Color::from_rgba8(0xe7, 0xec, 0xf2, 0xff)),
        secondary: Brush::Solid(Color::from_rgba8(0xb5, 0xc0, 0xcc, 0xff)),
        muted: Brush::Solid(Color::from_rgba8(0x7e, 0x8a, 0x97, 0xff)),
        activity: Brush::Solid(COLOR_ACTIVITY),
        success: Brush::Solid(COLOR_SUCCESS),
        warning: Brush::Solid(COLOR_WARNING),
        timing: Brush::Solid(COLOR_TIMING),
        metadata: Brush::Solid(COLOR_METADATA),
        on_activity: Brush::Solid(Color::from_rgba8(0x0f, 0x12, 0x17, 0xff)),
    };

    const BLUEPRINT: Self = Self {
        canvas: Brush::Solid(Color::from_rgba8(0xf3, 0xf5, 0xf7, 0xff)),
        elevated: Brush::Solid(Color::from_rgba8(0xee, 0xf1, 0xf4, 0xff)),
        panel: Brush::Solid(Color::from_rgba8(0xe8, 0xec, 0xf0, 0xff)),
        instrument: Brush::Solid(Color::from_rgba8(0xfb, 0xfc, 0xfe, 0xff)),
        rule: Brush::Solid(Color::from_rgba8(0xd8, 0xe0, 0xe8, 0xff)),
        border: Brush::Solid(Color::from_rgba8(0xc8, 0xd3, 0xdf, 0xff)),
        text: Brush::Solid(Color::from_rgba8(0x1b, 0x23, 0x30, 0xff)),
        secondary: Brush::Solid(Color::from_rgba8(0x40, 0x50, 0x64, 0xff)),
        muted: Brush::Solid(Color::from_rgba8(0x70, 0x81, 0x94, 0xff)),
        activity: Brush::Solid(Color::from_rgba8(0x00, 0x78, 0xd4, 0xff)),
        success: Brush::Solid(Color::from_rgba8(0x25, 0x86, 0x5a, 0xff)),
        warning: Brush::Solid(Color::from_rgba8(0xb7, 0x6e, 0x00, 0xff)),
        timing: Brush::Solid(Color::from_rgba8(0x6d, 0x57, 0xc8, 0xff)),
        metadata: Brush::Solid(Color::from_rgba8(0x4a, 0x6e, 0xa8, 0xff)),
        on_activity: Brush::Solid(Color::from_rgba8(0xff, 0xff, 0xff, 0xff)),
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SightlineTokens {
    pub(crate) shell: ClassId,
    pub(crate) top_bar: ClassId,
    pub(crate) workspace: ClassId,
    pub(crate) panel: ClassId,
    pub(crate) instrument: ClassId,
    pub(crate) metric: ClassId,
    pub(crate) rule: ClassId,
    pub(crate) title_text: ClassId,
    pub(crate) secondary_text: ClassId,
    pub(crate) muted_text: ClassId,
    pub(crate) activity_text: ClassId,
    pub(crate) success_text: ClassId,
    pub(crate) warning_text: ClassId,
    pub(crate) timing_text: ClassId,
    canvas: ResourceKey,
    elevated_bg: ResourceKey,
    panel_bg: ResourceKey,
    instrument_bg: ResourceKey,
    rule_bg: ResourceKey,
    border: ResourceKey,
    text: ResourceKey,
    secondary_fg: ResourceKey,
    muted_fg: ResourceKey,
    activity: ResourceKey,
    success: ResourceKey,
    warning: ResourceKey,
    timing: ResourceKey,
    metadata: ResourceKey,
    on_activity: ResourceKey,
}

pub(crate) struct SightlineStyleTokens;

impl StyleTokenSet for SightlineStyleTokens {
    type Resolved = SightlineTokens;

    fn resolve(vocabulary: &mut StyleVocabulary) -> Self::Resolved {
        SightlineTokens {
            shell: vocabulary.class_id(".sightline-shell"),
            top_bar: vocabulary.class_id(".sightline-top-bar"),
            workspace: vocabulary.class_id(".sightline-workspace"),
            panel: vocabulary.class_id(".sightline-panel"),
            instrument: vocabulary.class_id(".sightline-instrument"),
            metric: vocabulary.class_id(".sightline-metric"),
            rule: vocabulary.class_id(".sightline-rule"),
            title_text: vocabulary.class_id(".sightline-title-text"),
            secondary_text: vocabulary.class_id(".sightline-secondary-text"),
            muted_text: vocabulary.class_id(".sightline-muted-text"),
            activity_text: vocabulary.class_id(".sightline-activity-text"),
            success_text: vocabulary.class_id(".sightline-success-text"),
            warning_text: vocabulary.class_id(".sightline-warning-text"),
            timing_text: vocabulary.class_id(".sightline-timing-text"),
            canvas: vocabulary.resource_key("sightline.canvas"),
            elevated_bg: vocabulary.resource_key("sightline.elevated.bg"),
            panel_bg: vocabulary.resource_key("sightline.panel.bg"),
            instrument_bg: vocabulary.resource_key("sightline.instrument.bg"),
            rule_bg: vocabulary.resource_key("sightline.rule.bg"),
            border: vocabulary.resource_key("sightline.border"),
            text: vocabulary.resource_key("sightline.text"),
            secondary_fg: vocabulary.resource_key("sightline.text.secondary"),
            muted_fg: vocabulary.resource_key("sightline.text.muted"),
            activity: vocabulary.resource_key("sightline.activity"),
            success: vocabulary.resource_key("sightline.success"),
            warning: vocabulary.resource_key("sightline.warning"),
            timing: vocabulary.resource_key("sightline.timing"),
            metadata: vocabulary.resource_key("sightline.metadata"),
            on_activity: vocabulary.resource_key("sightline.on-activity"),
        }
    }
}

pub(crate) fn sightline_theme(mode: SightlineThemeMode, tokens: SightlineTokens) -> Theme {
    let palette = mode.palette();
    let builder = match mode {
        SightlineThemeMode::Graphite => theme::dark_theme_builder(),
        SightlineThemeMode::Blueprint => theme::light_theme_builder(),
    };
    extend_theme(builder, tokens, palette).build()
}

fn extend_theme(builder: ThemeBuilder, tokens: SightlineTokens, palette: Palette) -> ThemeBuilder {
    builder
        .set(theme::SURFACE_BG, Some(palette.canvas.clone()))
        .set(theme::PANEL_BG, Some(palette.elevated.clone()))
        .set(theme::CONTROL_BG, Some(palette.panel.clone()))
        .set(theme::CONTROL_BORDER, Some(palette.border.clone()))
        .set(theme::CONTROL_HOVER_BG, Some(palette.instrument.clone()))
        .set(theme::CONTROL_HOVER_BORDER, Some(palette.activity.clone()))
        .set(theme::ACCENT_BG, Some(palette.activity.clone()))
        .set(theme::ACCENT_HOVER_BG, Some(palette.activity.clone()))
        .set(theme::ACCENT_PRESSED_BG, Some(palette.metadata.clone()))
        .set(theme::SUCCESS_BG, Some(palette.success.clone()))
        .set(theme::TRACK_BG, Some(palette.rule.clone()))
        .set(theme::ON_ACCENT_FG, palette.on_activity.clone())
        .set(theme::ON_SURFACE_FG, palette.text.clone())
        .set(theme::ON_SURFACE_MUTED_FG, palette.muted.clone())
        .set(theme::FOCUS_RING_BORDER, Some(palette.activity.clone()))
        .set(tokens.canvas, Some(palette.canvas))
        .set(tokens.elevated_bg, Some(palette.elevated))
        .set(tokens.panel_bg, Some(palette.panel))
        .set(tokens.instrument_bg, Some(palette.instrument))
        .set(tokens.rule_bg, Some(palette.rule))
        .set(tokens.border, Some(palette.border))
        .set(tokens.text, palette.text)
        .set(tokens.secondary_fg, palette.secondary)
        .set(tokens.muted_fg, palette.muted)
        .set(tokens.activity, palette.activity)
        .set(tokens.success, palette.success)
        .set(tokens.warning, palette.warning)
        .set(tokens.timing, palette.timing)
        .set(tokens.metadata, palette.metadata)
        .set(tokens.on_activity, palette.on_activity)
}

pub(crate) fn sightline_cascade(
    properties: &BuiltInProperties,
    tokens: SightlineTokens,
) -> StyleCascade {
    StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Sheet,
            class_selector(tokens.shell),
            StyleBuilder::new()
                .background_resource(properties, tokens.canvas)
                .set_resource(properties.foreground, tokens.text)
                .build(),
        )
        .push_rule(
            StyleOrigin::Sheet,
            class_selector(tokens.top_bar),
            StyleBuilder::new()
                .background_resource(properties, tokens.elevated_bg)
                .border_brush_resource(properties, tokens.border)
                .build(),
        )
        .push_rule(
            StyleOrigin::Sheet,
            class_selector(tokens.workspace),
            StyleBuilder::new()
                .background_resource(properties, tokens.canvas)
                .build(),
        )
        .push_rule(
            StyleOrigin::Sheet,
            class_selector(tokens.panel),
            StyleBuilder::new()
                .background_resource(properties, tokens.panel_bg)
                .border_brush_resource(properties, tokens.border)
                .build(),
        )
        .push_rule(
            StyleOrigin::Sheet,
            class_selector(tokens.instrument),
            StyleBuilder::new()
                .background_resource(properties, tokens.instrument_bg)
                .border_brush_resource(properties, tokens.border)
                .build(),
        )
        .push_rule(
            StyleOrigin::Sheet,
            class_selector(tokens.metric),
            StyleBuilder::new()
                .background_resource(properties, tokens.panel_bg)
                .border_brush_resource(properties, tokens.border)
                .build(),
        )
        .push_rule(
            StyleOrigin::Sheet,
            class_selector(tokens.rule),
            StyleBuilder::new()
                .background_resource(properties, tokens.rule_bg)
                .build(),
        )
        .push_rule(
            StyleOrigin::Sheet,
            class_selector(tokens.title_text),
            StyleBuilder::new()
                .set_resource(properties.foreground, tokens.text)
                .build(),
        )
        .push_rule(
            StyleOrigin::Sheet,
            class_selector(tokens.secondary_text),
            StyleBuilder::new()
                .set_resource(properties.foreground, tokens.secondary_fg)
                .build(),
        )
        .push_rule(
            StyleOrigin::Sheet,
            class_selector(tokens.muted_text),
            StyleBuilder::new()
                .set_resource(properties.foreground, tokens.muted_fg)
                .build(),
        )
        .push_rule(
            StyleOrigin::Sheet,
            class_selector(tokens.activity_text),
            StyleBuilder::new()
                .set_resource(properties.foreground, tokens.activity)
                .build(),
        )
        .push_rule(
            StyleOrigin::Sheet,
            class_selector(tokens.success_text),
            StyleBuilder::new()
                .set_resource(properties.foreground, tokens.success)
                .build(),
        )
        .push_rule(
            StyleOrigin::Sheet,
            class_selector(tokens.warning_text),
            StyleBuilder::new()
                .set_resource(properties.foreground, tokens.warning)
                .build(),
        )
        .push_rule(
            StyleOrigin::Sheet,
            class_selector(tokens.timing_text),
            StyleBuilder::new()
                .set_resource(properties.foreground, tokens.timing)
                .build(),
        )
        .build()
}

fn class_selector(class: ClassId) -> Selector {
    Selector::single(SelectorStep::class(class))
}
