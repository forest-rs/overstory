// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Slider`: continuous value control with a draggable thumb.
//!
//! Structure:
//!
//! ```text
//! Slider (focusable, holds `Value` / `Minimum` / `Maximum`)
//!   ├─ SliderTrack (full-width pill, vertically centered)
//!   └─ SliderThumb (circle, absolutely positioned by Value)
//! ```
//!
//! Interaction:
//!
//! * Click anywhere on the slider — `Value` snaps to the
//!   click-position-on-track ratio.
//! * Drag — `Value` follows the pointer continuously.
//! * Left / Down arrow — `Value -= step`.
//! * Right / Up arrow — `Value += step`.
//! * Home — `Value = Minimum`. End — `Value = Maximum`.
//!
//! The thumb's horizontal position is recomputed by writing its
//! `Layout` (absolute `inset.left`) whenever `Value` changes. The
//! cascade carries no per-value state; the slider's behavior owns
//! the value→position mapping imperatively.

use understory_box_tree::NodeId;
use understory_presentation::PresentationStore;
use understory_property::DependencyObjectExt;

use crate::Phase;
use crate::behavior::{ElementBehavior, PresentContext};
use crate::element::{Element, ElementId, ElementKind};
use crate::generic::present_surface;
use crate::template::Template;
use crate::ui::Ui;

/// Element kind for the slider container.
pub const ELEMENT_KIND_SLIDER: ElementKind = ElementKind::new(6);

/// Element kind for the slider's track (the colored bar behind the
/// thumb).
pub const ELEMENT_KIND_SLIDER_TRACK: ElementKind = ElementKind::new(7);

/// Element kind for the slider's thumb (the round handle).
pub const ELEMENT_KIND_SLIDER_THUMB: ElementKind = ElementKind::new(8);

/// Slider dimensions. Chosen to match the `ToggleSwitch`'s overall
/// height so a row of controls reads as a cohesive group.
const SLIDER_WIDTH: f32 = 220.0;
const SLIDER_HEIGHT: f32 = 32.0;
const TRACK_HEIGHT: f32 = 6.0;
const THUMB_DIAMETER: f32 = 22.0;
/// Per-arrow-key step as a fraction of the full range.
const KEY_STEP_FRACTION: f64 = 0.05;

/// Stateless behavior for [`ELEMENT_KIND_SLIDER`].
#[derive(Debug, Default)]
pub struct SliderBehavior;

/// Stateless behavior for the slider's track child.
#[derive(Debug, Default)]
pub struct SliderTrackBehavior;

/// Stateless behavior for the slider's thumb child.
#[derive(Debug, Default)]
pub struct SliderThumbBehavior;

pub(crate) static SLIDER_BEHAVIOR: SliderBehavior = SliderBehavior;
pub(crate) static SLIDER_TRACK_BEHAVIOR: SliderTrackBehavior = SliderTrackBehavior;
pub(crate) static SLIDER_THUMB_BEHAVIOR: SliderThumbBehavior = SliderThumbBehavior;

static SLIDER_TEMPLATE: SliderTemplate = SliderTemplate;

#[derive(Debug, Default)]
struct SliderTemplate;

impl Template for SliderTemplate {
    fn instantiate(&self, ui: &mut Ui, owner: ElementId) {
        let layout_prop = ui.properties().layout;

        // Slider: fixed-size relative container.
        ui.set_local(
            owner,
            layout_prop,
            taffy::Style {
                position: taffy::Position::Relative,
                size: taffy::Size {
                    width: taffy::Dimension::length(SLIDER_WIDTH),
                    height: taffy::Dimension::length(SLIDER_HEIGHT),
                },
                ..taffy::Style::default()
            },
        );

        // Track: spans the full width, vertically centered, with
        // padding so the rounded ends stay clear of the thumb edges.
        let track = ui.insert_child(owner, ELEMENT_KIND_SLIDER_TRACK);
        let track_inset_y = (SLIDER_HEIGHT - TRACK_HEIGHT) / 2.0;
        ui.set_local(
            track,
            layout_prop,
            taffy::Style {
                position: taffy::Position::Absolute,
                inset: taffy::Rect {
                    left: taffy::LengthPercentageAuto::length(0.0),
                    top: taffy::LengthPercentageAuto::length(track_inset_y),
                    right: taffy::LengthPercentageAuto::auto(),
                    bottom: taffy::LengthPercentageAuto::auto(),
                },
                size: taffy::Size {
                    width: taffy::Dimension::length(SLIDER_WIDTH),
                    height: taffy::Dimension::length(TRACK_HEIGHT),
                },
                ..taffy::Style::default()
            },
        );

        // Thumb: absolutely positioned. Initial left is computed
        // from the slider's initial Value (default = `Minimum`).
        let thumb = ui.insert_child(owner, ELEMENT_KIND_SLIDER_THUMB);
        let thumb_top = (SLIDER_HEIGHT - THUMB_DIAMETER) / 2.0;
        let initial_left = thumb_left_for_value(0.0, 0.0, 1.0);
        ui.set_local(thumb, layout_prop, thumb_layout(initial_left, thumb_top));

        // Click anywhere on the slider snaps the value.
        ui.on_click(owner, move |ui, event| {
            if !matches!(event.phase, Phase::Target | Phase::Bubble) {
                return;
            }
            set_value_from_position(ui, owner, thumb, event.position);
        });
        // Drag updates value continuously.
        ui.on_drag(owner, move |ui, event| {
            if !matches!(event.phase, Phase::Target | Phase::Bubble) {
                return;
            }
            set_value_from_position(ui, owner, thumb, event.position);
        });
        // Arrow keys + Home/End nudge the value.
        ui.on_key(owner, move |ui, event| {
            use ui_events::keyboard::{Key, KeyState, NamedKey};
            if !matches!(event.phase, Phase::Target) {
                return;
            }
            if event.key.state != KeyState::Down {
                return;
            }
            let min = read_or(ui, owner, ui.properties().minimum, 0.0);
            let max = read_or(ui, owner, ui.properties().maximum, 1.0);
            let step = (max - min) * KEY_STEP_FRACTION;
            let current = read_or(ui, owner, ui.properties().value, min);
            let next = match &event.key.key {
                Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowDown) => current - step,
                Key::Named(NamedKey::ArrowRight | NamedKey::ArrowUp) => current + step,
                Key::Named(NamedKey::Home) => min,
                Key::Named(NamedKey::End) => max,
                _ => return,
            };
            set_value(ui, owner, thumb, next.clamp(min, max));
        });
    }
}

impl ElementBehavior for SliderBehavior {
    fn present(
        &self,
        element: &Element,
        ctx: &PresentContext<'_>,
        store: &mut PresentationStore<NodeId, ElementId>,
        node: NodeId,
    ) {
        // Slider's own surface (focus ring lives here, body usually
        // transparent).
        present_surface(element, ctx, store, node);
    }

    fn default_template(&self) -> Option<&'static dyn Template> {
        Some(&SLIDER_TEMPLATE)
    }
}

impl ElementBehavior for SliderTrackBehavior {
    fn present(
        &self,
        element: &Element,
        ctx: &PresentContext<'_>,
        store: &mut PresentationStore<NodeId, ElementId>,
        node: NodeId,
    ) {
        present_surface(element, ctx, store, node);
    }
}

impl ElementBehavior for SliderThumbBehavior {
    fn present(
        &self,
        element: &Element,
        ctx: &PresentContext<'_>,
        store: &mut PresentationStore<NodeId, ElementId>,
        node: NodeId,
    ) {
        present_surface(element, ctx, store, node);
    }
}

/// Reads `prop`'s local value on `id`, falling back to `default` if
/// the element is gone or has no local value.
fn read_or<T: Clone + 'static>(
    ui: &Ui,
    id: ElementId,
    prop: understory_property::Property<T>,
    default: T,
) -> T {
    ui.get(id)
        .and_then(|el| el.get_local_value(prop).cloned())
        .unwrap_or(default)
}

/// Computes the thumb's `inset.left` for a given value, taking the
/// slider's `min`/`max` range into account.
fn thumb_left_for_value(value: f64, min: f64, max: f64) -> f32 {
    let span = (max - min).max(f64::EPSILON);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "ratio is clamped to 0..=1, fits f32 with room to spare"
    )]
    let t = ((value - min) / span).clamp(0.0, 1.0) as f32;
    let travel = SLIDER_WIDTH - THUMB_DIAMETER;
    t * travel
}

/// Builds the thumb's `Layout` style for a given `inset.left` and
/// vertical inset.
fn thumb_layout(left: f32, top: f32) -> taffy::Style {
    taffy::Style {
        position: taffy::Position::Absolute,
        inset: taffy::Rect {
            left: taffy::LengthPercentageAuto::length(left),
            top: taffy::LengthPercentageAuto::length(top),
            right: taffy::LengthPercentageAuto::auto(),
            bottom: taffy::LengthPercentageAuto::auto(),
        },
        size: taffy::Size {
            width: taffy::Dimension::length(THUMB_DIAMETER),
            height: taffy::Dimension::length(THUMB_DIAMETER),
        },
        ..taffy::Style::default()
    }
}

/// Updates `slider`'s `Value` and repositions its `thumb` to match.
/// Single source of truth for value writes — every interaction path
/// funnels through here.
fn set_value(ui: &mut Ui, slider: ElementId, thumb: ElementId, value: f64) {
    let min = read_or(ui, slider, ui.properties().minimum, 0.0);
    let max = read_or(ui, slider, ui.properties().maximum, 1.0);
    let clamped = value.clamp(min, max);
    let value_prop = ui.properties().value;
    let layout_prop = ui.properties().layout;
    ui.set_local(slider, value_prop, clamped);
    let left = thumb_left_for_value(clamped, min, max);
    let top = (SLIDER_HEIGHT - THUMB_DIAMETER) / 2.0;
    ui.set_local(thumb, layout_prop, thumb_layout(left, top));
}

/// Maps a pointer position (in physical pixels, in viewport space)
/// to a slider value, then writes it. The mapping reads the
/// slider's world bounds so click + drag work regardless of where
/// the slider is on the screen.
fn set_value_from_position(
    ui: &mut Ui,
    slider: ElementId,
    thumb: ElementId,
    pointer: kurbo::Point,
) {
    let bounds = ui.effective_bounds(slider);
    if bounds.width() <= 0.0 {
        return;
    }
    let track_width = SLIDER_WIDTH - THUMB_DIAMETER;
    let usable_origin = bounds.origin().x + f64::from(THUMB_DIAMETER) / 2.0;
    let t = ((pointer.x - usable_origin) / f64::from(track_width)).clamp(0.0, 1.0);
    let min = read_or(ui, slider, ui.properties().minimum, 0.0);
    let max = read_or(ui, slider, ui.properties().maximum, 1.0);
    let value = min + t * (max - min);
    set_value(ui, slider, thumb, value);
}

/// Public helper: programmatically set a slider's value (and move
/// its thumb). Useful for tests, undo/redo, or external control.
///
/// # Panics
///
/// Panics if `slider` is stale or doesn't have a thumb child.
pub fn set_slider_value(ui: &mut Ui, slider: ElementId, value: f64) {
    // The slider's template always inserts a Track then a Thumb,
    // so the thumb is the second child. We re-derive it instead of
    // requiring callers to thread a Thumb id through.
    let thumb = ui
        .get(slider)
        .and_then(|el| el.children().get(1).copied())
        .expect("slider must have a thumb child");
    set_value(ui, slider, thumb, value);
}
