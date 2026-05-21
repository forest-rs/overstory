// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Slider`: continuous value control with a draggable thumb.

use alloc::{boxed::Box, format, string::String};

use understory_property::DependencyObjectExt;
use understory_style::{ClassId, PartTag, TypeTag};

use crate::Phase;
use crate::controls::text::TextWidget;
use crate::element::ElementId;
use crate::template::{Template, TemplateContext, TemplateItemKey};
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, SurfacePartWidget, Widget};

/// Selector identity for `Slider`.
pub const TYPE_SLIDER: TypeTag = TypeTag(6);
/// Owner-local part identity for the slider track.
pub const PART_SLIDER_TRACK: PartTag = PartTag(1);
/// Owner-local part identity for the slider thumb.
pub const PART_SLIDER_THUMB: PartTag = PartTag(2);
/// Owner-local part identity for the slider axis surface.
pub const PART_SLIDER_AXIS: PartTag = PartTag(3);
/// Owner-local repeated part identity for slider tick marks.
pub const PART_SLIDER_TICK: PartTag = PartTag(4);
/// Owner-local repeated part identity for slider tick labels.
pub const PART_SLIDER_TICK_LABEL: PartTag = PartTag(5);
/// Class applied to major slider tick marks and labels.
pub const CLASS_SLIDER_MAJOR_TICK: ClassId = ClassId(1);
/// Class applied to minor slider tick marks and labels.
pub const CLASS_SLIDER_MINOR_TICK: ClassId = ClassId(2);

const SLIDER_WIDTH: f32 = 220.0;
const SLIDER_HEIGHT: f32 = 52.0;
const TRACK_HEIGHT: f32 = 6.0;
const THUMB_DIAMETER: f32 = 22.0;
const TICK_WIDTH: f32 = 2.0;
const MAJOR_TICK_HEIGHT: f32 = 10.0;
const MINOR_TICK_HEIGHT: f32 = 6.0;
const TICK_TOP: f32 = 28.0;
const LABEL_TOP: f32 = 38.0;
const LABEL_CHAR_WIDTH: f32 = 6.0;
const LABEL_MIN_WIDTH: f32 = 6.0;
const SLIDER_TICK_STEPS: usize = 4;
const SLIDER_MAJOR_TICK_EVERY: usize = 2;
const KEY_STEP_FRACTION: f64 = 0.05;

/// Builder + spec for a `Slider`.
pub struct Slider {
    initial_value: f64,
    min: f64,
    max: f64,
    state: ControlState,
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
            state: ControlState::interactive(),
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
        self.state = self.state.focusable(focusable);
        self
    }

    /// Sets whether the slider starts enabled. Disabled sliders are removed
    /// from focus traversal, ignore user activation, and carry `:disabled` for
    /// styling.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.state = self.state.enabled(enabled);
        self
    }

    /// Convenience for [`Self::enabled(false)`].
    #[must_use]
    pub fn disabled(self) -> Self {
        self.enabled(false)
    }

    /// Sets the shared construction-time control state.
    #[must_use]
    pub fn control_state(mut self, state: ControlState) -> Self {
        self.state = state;
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
        ui.set_control_state(id, self.state);
        sync_slider_tick_labels(ui, id);
        // Initial value: drive through the same setter the runtime
        // path uses so the thumb's Layout matches. The template has
        // already expanded by this point, so the thumb child exists.
        let clamped = self.initial_value.clamp(self.min, self.max);
        let thumb = ui
            .part(id, PART_SLIDER_THUMB)
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
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_SLIDER)
    }

    fn default_template(&self) -> Option<&'static dyn Template> {
        Some(&SLIDER_TEMPLATE)
    }
}

static SLIDER_TEMPLATE: SliderTemplate = SliderTemplate;

#[derive(Debug, Default)]
struct SliderTemplate;

impl Template for SliderTemplate {
    fn instantiate(&self, cx: &mut TemplateContext<'_>) {
        let owner = cx.owner();
        let layout_prop = cx.properties().layout;
        let value_prop = cx.properties().value;
        let min_prop = cx.properties().minimum;
        let max_prop = cx.properties().maximum;
        let content_prop = cx.properties().content;

        // Track: full width, vertically centered.
        let track = cx.part_widget(PART_SLIDER_TRACK, Box::new(SurfacePartWidget));
        let track_inset_y = (SLIDER_HEIGHT - TRACK_HEIGHT) / 2.0;
        cx.set_local(
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

        let axis = cx.part_widget(PART_SLIDER_AXIS, Box::new(SurfacePartWidget));
        cx.set_local(axis, layout_prop, axis_layout());
        let min = cx.read_local_or(owner, min_prop, 0.0);
        let max = cx.read_local_or(owner, max_prop, 1.0);
        for index in 0..=SLIDER_TICK_STEPS {
            let key = tick_key(index);
            let major = is_major_tick(index);
            let class = tick_class(major);
            let tick = cx.keyed_part_widget(PART_SLIDER_TICK, key, Box::new(SurfacePartWidget));
            cx.add_class(tick, class);
            cx.set_local(tick, layout_prop, tick_layout(index, major));

            let label = cx.keyed_part_widget(PART_SLIDER_TICK_LABEL, key, Box::new(TextWidget));
            cx.add_class(label, class);
            let content = tick_label_for_index(index, min, max);
            cx.set_local(label, layout_prop, tick_label_layout(index, &content));
            cx.set_local(label, content_prop, content);
        }

        // Thumb: absolute, recomputed left whenever Value changes.
        let thumb = cx.part_widget(PART_SLIDER_THUMB, Box::new(SurfacePartWidget));
        let thumb_top = (SLIDER_HEIGHT - THUMB_DIAMETER) / 2.0;
        let initial_left = thumb_left_for_value(
            cx.read_local_or(owner, value_prop, 0.0),
            cx.read_local_or(owner, min_prop, 0.0),
            cx.read_local_or(owner, max_prop, 1.0),
        );
        cx.set_local(thumb, layout_prop, thumb_layout(initial_left, thumb_top));

        cx.on_click(owner, move |ui, event| {
            if !matches!(event.phase, Phase::Target | Phase::Bubble) {
                return;
            }
            if !ui.is_enabled(owner) {
                return;
            }
            set_value_from_position(ui, owner, thumb, event.position);
        });
        cx.on_drag(owner, move |ui, event| {
            if !matches!(event.phase, Phase::Target | Phase::Bubble) {
                return;
            }
            if !ui.is_enabled(owner) {
                return;
            }
            set_value_from_position(ui, owner, thumb, event.position);
        });
        cx.on_key(owner, move |ui, event| {
            use ui_events::keyboard::{Key, KeyState, NamedKey};
            if !matches!(event.phase, Phase::Target) {
                return;
            }
            if event.key.state != KeyState::Down {
                return;
            }
            if !ui.is_enabled(owner) {
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

fn axis_layout() -> taffy::Style {
    taffy::Style {
        position: taffy::Position::Absolute,
        size: taffy::Size {
            width: taffy::Dimension::length(SLIDER_WIDTH),
            height: taffy::Dimension::length(SLIDER_HEIGHT),
        },
        ..taffy::Style::default()
    }
}

fn tick_layout(index: usize, major: bool) -> taffy::Style {
    let height = if major {
        MAJOR_TICK_HEIGHT
    } else {
        MINOR_TICK_HEIGHT
    };
    taffy::Style {
        position: taffy::Position::Absolute,
        inset: taffy::Rect {
            left: taffy::LengthPercentageAuto::length(tick_left(index, TICK_WIDTH)),
            top: taffy::LengthPercentageAuto::length(TICK_TOP),
            right: taffy::LengthPercentageAuto::auto(),
            bottom: taffy::LengthPercentageAuto::auto(),
        },
        size: taffy::Size {
            width: taffy::Dimension::length(TICK_WIDTH),
            height: taffy::Dimension::length(height),
        },
        ..taffy::Style::default()
    }
}

fn tick_label_layout(index: usize, label: &str) -> taffy::Style {
    let width = label_width(label);
    taffy::Style {
        position: taffy::Position::Absolute,
        inset: taffy::Rect {
            left: taffy::LengthPercentageAuto::length(tick_left(index, width)),
            top: taffy::LengthPercentageAuto::length(LABEL_TOP),
            right: taffy::LengthPercentageAuto::auto(),
            bottom: taffy::LengthPercentageAuto::auto(),
        },
        size: taffy::Size {
            width: taffy::Dimension::length(width),
            height: taffy::Dimension::length(SLIDER_HEIGHT - LABEL_TOP),
        },
        ..taffy::Style::default()
    }
}

fn label_width(label: &str) -> f32 {
    let chars = u16::try_from(label.chars().count()).unwrap_or(u16::MAX);
    (f32::from(chars) * LABEL_CHAR_WIDTH).max(LABEL_MIN_WIDTH)
}

fn tick_left(index: usize, width: f32) -> f32 {
    let t = tick_fraction_f32(index);
    (t * SLIDER_WIDTH - width / 2.0).clamp(0.0, SLIDER_WIDTH - width)
}

#[expect(
    clippy::cast_precision_loss,
    reason = "slider tick indices are small constants"
)]
fn tick_fraction_f32(index: usize) -> f32 {
    index as f32 / SLIDER_TICK_STEPS as f32
}

#[expect(
    clippy::cast_precision_loss,
    reason = "slider tick indices are small constants"
)]
fn tick_fraction(index: usize) -> f64 {
    index as f64 / SLIDER_TICK_STEPS as f64
}

fn is_major_tick(index: usize) -> bool {
    index.is_multiple_of(SLIDER_MAJOR_TICK_EVERY)
}

fn tick_class(major: bool) -> ClassId {
    if major {
        CLASS_SLIDER_MAJOR_TICK
    } else {
        CLASS_SLIDER_MINOR_TICK
    }
}

fn tick_label_for_index(index: usize, min: f64, max: f64) -> String {
    let value = min + tick_fraction(index) * (max - min);
    format_tick_value(value)
}

fn format_tick_value(value: f64) -> String {
    let mut label = format!("{value:.2}");
    while label.contains('.') && label.ends_with('0') {
        label.pop();
    }
    if label.ends_with('.') {
        label.pop();
    }
    label
}

fn sync_slider_tick_labels(ui: &mut Ui, slider: ElementId) {
    let min = read_or(ui, slider, ui.properties().minimum, 0.0);
    let max = read_or(ui, slider, ui.properties().maximum, 1.0);
    let content_prop = ui.properties().content;
    let layout_prop = ui.properties().layout;
    for index in 0..=SLIDER_TICK_STEPS {
        let Some(label) = ui
            .template_instance(slider)
            .and_then(|instance| instance.keyed_part(PART_SLIDER_TICK_LABEL, tick_key(index)))
        else {
            continue;
        };
        let content = tick_label_for_index(index, min, max);
        ui.set_local(label, layout_prop, tick_label_layout(index, &content));
        ui.set_local(label, content_prop, content);
    }
}

fn tick_key(index: usize) -> TemplateItemKey {
    let value = u64::try_from(index).expect("slider tick index should fit u64");
    TemplateItemKey::new(value)
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
        .part(slider, PART_SLIDER_THUMB)
        .expect("slider must have a thumb child");
    set_value(ui, slider, thumb, value);
}
