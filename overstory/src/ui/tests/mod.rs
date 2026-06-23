// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT
use super::*;
use crate::channels::{ARRANGE, COMPOSE, GEOMETRY, LAYOUT, PAINT};
use crate::pseudos::{
    PSEUDO_CHECKED, PSEUDO_DISABLED, PSEUDO_FOCUS_VISIBLE, PSEUDO_FOCUSED, PSEUDO_HOVER,
    PSEUDO_PRESSED, PSEUDO_SELECTED,
};
use crate::test_support::{
    pointer_down, pointer_leave, pointer_move, pointer_scroll, pointer_up, touch_down, touch_up,
};
use crate::{
    BindingOwner, CompositionPolicy, ControlTemplateSpecExt, ElementRef, ElementRefSpecExt,
    ElementSpecExt, FocusChangeReason, KeyDispatchResult, Layout, LayoutPlacement, Navigation,
    Phase, PointerEventKind, RoutedEventKind, RoutedHandlerOptions, StyleTokenSet, StyleVocabulary,
    SurfaceSpecExt, SurfaceStyleExt, TYPE_GENERIC, TemplateContext, TemplateSlot, TooltipSpecExt,
    UiResolveLookup,
};
use alloc::boxed::Box;
use alloc::string::String;
use core::time::Duration;
use frameclock::FrameDemand;
use imaging::record::{Command, Scene};
use kurbo::{Affine, Point, Rect, Size, Vec2};
use peniko::color::{ColorSpaceTag, DynamicColor, HueDirection, Srgb};
use peniko::{Brush, Color};
use understory_property::{DependencyObject, DependencyObjectExt, PropertyMetadataBuilder};
use understory_style::{
    ClassId, PartTag, ResolveCx, ResourceKey, SelectorBuilder, SelectorStep, StyleBuilder,
    StyleCascadeBuilder, StyleOrigin, ThemeBuilder,
};

use crate::motion::{
    Animatable, AnimationTarget, ColorInterpolation, EffectClip, EffectKeyframe, EffectRule,
    EffectRules, EffectTriggerId, Interpolate, PresentationSlot, PresentationTarget,
    PresentationValue, PresentationValueKind, TimingFunction, TransitionRule, TransitionRules,
    TransitionSpec, millis,
};

mod content;
mod content_presenter;
mod events;
mod invalidation;
mod layout_composition;
mod layout_motion;
mod overlays;
mod presentation_motion;
mod properties;
mod runtime;
mod state_authoring;
mod style_text;
mod templates;
mod tree;

fn presented_element_text(ui: &mut Ui, element: ElementId) -> Option<String> {
    ui.compute_layout(Size::new(400.0, 120.0));
    let mut output = Scene::new();
    ui.paint(&mut output);

    let node = ui.box_for(element)?;
    ui.presentation
        .presentation(node)?
        .plain_text()
        .map(|text| String::from(text.content.as_str()))
}

fn presented_button_label_text(ui: &mut Ui, button: ElementId) -> Option<String> {
    let label = ui.part(button, crate::PART_BUTTON_LABEL)?;
    presented_element_text(ui, label)
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
        .expect("element should have a composition layer");
    ui.composition_layers()
        .world_transform(layer)
        .transform_point(Point::ZERO)
        .expect("element layer transform should be affine")
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.001,
        "expected {expected}, got {actual}"
    );
}

fn assert_surface_padding(
    ui: &Ui,
    element: ElementId,
    top: f64,
    right: f64,
    bottom: f64,
    left: f64,
) {
    let element = ui.get(element).expect("element should be live");
    let padding = ui.properties().surface.padding_widths;
    assert_eq!(element.get_local_value(padding.top), Some(&top));
    assert_eq!(element.get_local_value(padding.right), Some(&right));
    assert_eq!(element.get_local_value(padding.bottom), Some(&bottom));
    assert_eq!(element.get_local_value(padding.left), Some(&left));
}

fn assert_layout_spacing_has_no_padding(ui: &Ui, element: ElementId) {
    let element = ui.get(element).expect("element should be live");
    if let Some(spacing) = element.get_local_value(ui.properties().layout_spacing) {
        assert!(
            !spacing.inspection_summary().contains("padding"),
            "layout spacing should not carry box padding"
        );
    }
}

fn slider_value(ui: &Ui, slider: ElementId) -> Option<f64> {
    ui.get(slider)
        .and_then(|el| el.get_local_value(ui.properties().value).copied())
}
