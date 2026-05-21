// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Slider`: continuous value control with a draggable thumb.

use alloc::boxed::Box;

use understory_property::DependencyObjectExt;
use understory_style::TypeTag;

use crate::Phase;
use crate::element::ElementId;
use crate::template::Template;
use crate::ui::Ui;
use crate::widget::{AppendSpec, Widget};

/// Selector identity for `Slider`.
pub const TYPE_SLIDER: TypeTag = TypeTag(6);
/// Selector identity for the track part.
pub const TYPE_SLIDER_TRACK: TypeTag = TypeTag(7);
/// Selector identity for the thumb part.
pub const TYPE_SLIDER_THUMB: TypeTag = TypeTag(8);

const SLIDER_WIDTH: f32 = 220.0;
const SLIDER_HEIGHT: f32 = 32.0;
const TRACK_HEIGHT: f32 = 6.0;
const THUMB_DIAMETER: f32 = 22.0;
const KEY_STEP_FRACTION: f64 = 0.05;

/// Builder + spec for a `Slider`.
pub struct Slider {
    initial_value: f64,
    min: f64,
    max: f64,
    focusable: bool,
}

impl core::fmt::Debug for Slider {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Slider")
            .field("initial_value", &self.initial_value)
            .field("min", &self.min)
            .field("max", &self.max)
            .finish_non_exhaustive()
    }
}

impl Slider {
    /// Starts a new `Slider` builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            initial_value: 0.0,
            min: 0.0,
            max: 1.0,
            focusable: true,
        }
    }

    /// Sets the initial value.
    #[must_use]
    pub fn value(mut self, value: f64) -> Self {
        self.initial_value = value;
        self
    }

    /// Sets the lower bound. Default `0.0`.
    #[must_use]
    pub fn min(mut self, min: f64) -> Self {
        self.min = min;
        self
    }

    /// Sets the upper bound. Default `1.0`.
    #[must_use]
    pub fn max(mut self, max: f64) -> Self {
        self.max = max;
        self
    }

    /// Sets whether the slider participates in focus traversal.
    #[must_use]
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }
}

impl Default for Slider {
    fn default() -> Self {
        Self::new()
    }
}

impl AppendSpec for Slider {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget(parent, Box::new(SliderWidget));
        let layout_prop = ui.properties().layout;
        let min_prop = ui.properties().minimum;
        let max_prop = ui.properties().maximum;
        ui.set_local(id, min_prop, self.min);
        ui.set_local(id, max_prop, self.max);
        ui.set_local(
            id,
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
        if self.focusable {
            ui.set_focusable(id, true);
        }
        // Initial value: drive through the same setter the runtime
        // path uses so the thumb's Layout matches. The template has
        // already expanded by this point, so the thumb child exists.
        let clamped = self.initial_value.clamp(self.min, self.max);
        let thumb = ui
            .get(id)
            .and_then(|el| el.children().get(1).copied())
            .expect("slider template should create a thumb child");
        set_value(ui, id, thumb, clamped);
        id
    }
}

/// Per-element widget for `Slider`. Drives the default template
/// (track + thumb) and selector identity. Value / Min / Max live
/// on the property store; the thumb's position is recomputed
/// imperatively by the template's event handlers.
#[derive(Debug, Default)]
pub struct SliderWidget;

impl Widget for SliderWidget {
    fn type_tag(&self) -> TypeTag {
        TYPE_SLIDER
    }

    fn default_template(&self) -> Option<&'static dyn Template> {
        Some(&SLIDER_TEMPLATE)
    }
}

/// Per-element widget for the slider's track.
#[derive(Debug, Default)]
pub struct SliderTrackWidget;

impl Widget for SliderTrackWidget {
    fn type_tag(&self) -> TypeTag {
        TYPE_SLIDER_TRACK
    }
}

/// Per-element widget for the slider's thumb.
#[derive(Debug, Default)]
pub struct SliderThumbWidget;

impl Widget for SliderThumbWidget {
    fn type_tag(&self) -> TypeTag {
        TYPE_SLIDER_THUMB
    }
}

static SLIDER_TEMPLATE: SliderTemplate = SliderTemplate;

#[derive(Debug, Default)]
struct SliderTemplate;

impl Template for SliderTemplate {
    fn instantiate(&self, ui: &mut Ui, owner: ElementId) {
        let layout_prop = ui.properties().layout;

        // Track: full width, vertically centered.
        let track = ui.append_widget(owner, Box::new(SliderTrackWidget));
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

        // Thumb: absolute, recomputed left whenever Value changes.
        let thumb = ui.append_widget(owner, Box::new(SliderThumbWidget));
        let thumb_top = (SLIDER_HEIGHT - THUMB_DIAMETER) / 2.0;
        let initial_left = thumb_left_for_value(
            read_or(ui, owner, ui.properties().value, 0.0),
            read_or(ui, owner, ui.properties().minimum, 0.0),
            read_or(ui, owner, ui.properties().maximum, 1.0),
        );
        ui.set_local(thumb, layout_prop, thumb_layout(initial_left, thumb_top));

        ui.on_click(owner, move |ui, event| {
            if !matches!(event.phase, Phase::Target | Phase::Bubble) {
                return;
            }
            set_value_from_position(ui, owner, thumb, event.position);
        });
        ui.on_drag(owner, move |ui, event| {
            if !matches!(event.phase, Phase::Target | Phase::Bubble) {
                return;
            }
            set_value_from_position(ui, owner, thumb, event.position);
        });
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

/// Programmatically set a slider's value (and move its thumb).
///
/// # Panics
///
/// Panics if `slider` is stale or doesn't have a thumb child.
pub fn set_slider_value(ui: &mut Ui, slider: ElementId, value: f64) {
    let thumb = ui
        .get(slider)
        .and_then(|el| el.children().get(1).copied())
        .expect("slider must have a thumb child");
    set_value(ui, slider, thumb, value);
}
