// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Slider`: continuous value control with a draggable thumb.
//!
//! Roadmap:
//! - Keep range math, keyboard increments, drag capture, and value clamping in
//!   the control; keep track, thumb, tick, and label visuals in templates.
//! - Use `understory_axis` for tick generation and label placement so sliders,
//!   rulers, and graph axes share the same scale vocabulary.
//! - Add orientation, reversed direction, snapping, value formatting, major /
//!   minor tick styling, and live-vs-committed drag values without hard-coding a
//!   single visual layout.
//! - Make pointer, touch, wheel, and keyboard behavior line up with native
//!   slider expectations, including Shift-based larger steps.
//! - Add range accessibility metadata and host actions for increment, decrement,
//!   and set-value operations.

use alloc::{boxed::Box, format, string::String};

use understory_axis::{
    AxisMapping1D, AxisRuler1D, AxisRulerMark, AxisRulerOptions, AxisScale1D, AxisScaleOptions,
    AxisSubdivisionPolicy, AxisTickKind,
};
use understory_style::{ClassId, PartTag, TypeTag};

use crate::Phase;
use crate::controls::support::{f32_from_logical, is_initial_drag_move, read_local_or};
use crate::controls::text::TextWidget;
use crate::element::ElementId;
use crate::event_runtime::{ClickEvent, DragEvent, KeyEvent};
use crate::layouts::Layout;
use crate::template::{Template, TemplateChild, TemplateContext, TemplateItemKey, TemplateSlot};
use crate::timeline_runtime::{
    ChangeCause, TimelineTarget, TransitionPolicy, TransitionSpec, VisualOutput,
};
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, SurfacePartWidget, TemplateOverrideWidget, Widget};

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
/// Semantic slot for the slider track.
pub const SLOT_SLIDER_TRACK: TemplateSlot =
    TemplateSlot::new("slider-track").with_part_tag(PART_SLIDER_TRACK);
/// Semantic slot for the slider thumb.
pub const SLOT_SLIDER_THUMB: TemplateSlot =
    TemplateSlot::new("slider-thumb").with_part_tag(PART_SLIDER_THUMB);
/// Semantic slot for the slider axis surface.
pub const SLOT_SLIDER_AXIS: TemplateSlot =
    TemplateSlot::new("slider-axis").with_part_tag(PART_SLIDER_AXIS);
/// Semantic repeated slot for slider tick marks.
pub const SLOT_SLIDER_TICK: TemplateSlot =
    TemplateSlot::new("slider-tick").with_part_tag(PART_SLIDER_TICK);
/// Semantic repeated slot for slider tick labels.
pub const SLOT_SLIDER_TICK_LABEL: TemplateSlot =
    TemplateSlot::new("slider-tick-label").with_part_tag(PART_SLIDER_TICK_LABEL);
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
const DEFAULT_SMALL_CHANGE_FRACTION: f64 = 0.05;
const DEFAULT_LARGE_CHANGE_FRACTION: f64 = 0.20;
const THUMB_TRANSITION: TransitionSpec = TransitionSpec::linear(90_000_000);

/// Builder + spec for a `Slider`.
pub struct Slider {
    initial_value: f64,
    min: f64,
    max: f64,
    small_change: Option<f64>,
    large_change: Option<f64>,
    state: ControlState,
    template: Option<&'static dyn Template>,
}

impl core::fmt::Debug for Slider {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Slider")
            .field("initial_value", &self.initial_value)
            .field("min", &self.min)
            .field("max", &self.max)
            .field("small_change", &self.small_change)
            .field("large_change", &self.large_change)
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
            small_change: None,
            large_change: None,
            state: ControlState::interactive(),
            template: None,
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

    /// Sets the arrow-key increment. Non-positive or non-finite values are
    /// ignored and the control derives a range-relative default instead.
    #[must_use]
    pub fn small_change(mut self, value: f64) -> Self {
        self.small_change = Some(value);
        self
    }

    /// Sets the `PageUp` / `PageDown` and Shift+arrow increment. Non-positive or
    /// non-finite values are ignored and the control derives a range-relative
    /// default instead.
    #[must_use]
    pub fn large_change(mut self, value: f64) -> Self {
        self.large_change = Some(value);
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

    /// Convenience for [`Self::enabled`] with `false`.
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

    /// Overrides the visual template used when this slider is inserted.
    ///
    /// Custom templates must create [`SLOT_SLIDER_THUMB`] so slider value
    /// updates have a visual part to move. They can also create keyed
    /// [`SLOT_SLIDER_TICK`] and [`SLOT_SLIDER_TICK_LABEL`] entries when they
    /// want the built-in tick styling and label synchronization.
    #[must_use]
    pub fn template(mut self, template: &'static dyn Template) -> Self {
        self.template = Some(template);
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
        let widget: Box<dyn Widget> = match self.template {
            Some(template) => Box::new(TemplateOverrideWidget::new(
                SliderWidget::default(),
                template,
            )),
            None => Box::new(SliderWidget::default()),
        };
        let id = ui.append_widget_deferred_template(parent, widget);
        let layout_prop = ui.properties().layout;
        let min_prop = ui.properties().minimum;
        let max_prop = ui.properties().maximum;
        let small_change_prop = ui.properties().small_change;
        let large_change_prop = ui.properties().large_change;
        ui.set_local(id, min_prop, self.min);
        ui.set_local(id, max_prop, self.max);
        if let Some(small_change) = self.small_change {
            ui.set_local(id, small_change_prop, small_change);
        }
        if let Some(large_change) = self.large_change {
            ui.set_local(id, large_change_prop, large_change);
        }
        ui.set_local(
            id,
            layout_prop,
            Layout::new()
                .relative()
                .logical_size(SLIDER_WIDTH, SLIDER_HEIGHT),
        );
        ui.set_control_state(id, self.state);
        ui.expand_default_template(id);
        sync_slider_tick_labels(ui, id);
        // Initial value: drive through the same setter the runtime
        // path uses so the thumb's Layout matches. The template has
        // already expanded by this point, so the thumb child exists.
        let clamped = self.initial_value.clamp(self.min, self.max);
        let thumb = ui
            .slot(id, SLOT_SLIDER_THUMB)
            .expect("slider template should create SLOT_SLIDER_THUMB");
        set_value(ui, id, thumb, clamped, ThumbMotion::Snap);
        id
    }
}

/// Per-element widget for `Slider`. Drives the default template
/// (track + thumb) and selector identity. Value / Min / Max live
/// on the property store; the thumb's position is recomputed
/// imperatively by the template's event handlers.
#[derive(Debug, Default)]
pub(crate) struct SliderWidget {
    drag: SliderDragState,
}

impl Widget for SliderWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_SLIDER)
    }

    fn default_template(&self) -> Option<&'static dyn Template> {
        Some(&SLIDER_TEMPLATE)
    }

    fn handle_click(&mut self, slider: ElementId, ui: &mut Ui, event: ClickEvent<'_>) {
        let Some(thumb) = ui.slot(slider, SLOT_SLIDER_THUMB) else {
            return;
        };
        if !matches!(event.phase, Phase::Target | Phase::Bubble) {
            return;
        }
        if !ui.is_enabled(slider) {
            return;
        }
        if event.source == thumb {
            return;
        }
        set_value_from_position(ui, slider, thumb, event.position, ThumbMotion::Animate);
        event.mark_handled();
    }

    fn handle_drag(&mut self, slider: ElementId, ui: &mut Ui, event: DragEvent<'_>) {
        let Some(thumb) = ui.slot(slider, SLOT_SLIDER_THUMB) else {
            return;
        };
        if !matches!(event.phase, Phase::Target | Phase::Bubble) {
            return;
        }
        if !ui.is_enabled(slider) {
            return;
        }
        self.drag.update(
            ui,
            slider,
            thumb,
            event.press_position,
            event.position,
            event.delta,
        );
        event.mark_handled();
    }

    fn handle_key(&mut self, slider: ElementId, ui: &mut Ui, event: KeyEvent<'_>) {
        use ui_events::keyboard::{Key, KeyState, NamedKey};
        let Some(thumb) = ui.slot(slider, SLOT_SLIDER_THUMB) else {
            return;
        };
        if !matches!(event.phase, Phase::Target) {
            return;
        }
        if event.key.state != KeyState::Down {
            return;
        }
        if !ui.is_enabled(slider) {
            return;
        }
        let min = read_local_or(ui, slider, ui.properties().minimum, 0.0);
        let max = read_local_or(ui, slider, ui.properties().maximum, 1.0);
        let small = read_change_or(
            ui,
            slider,
            ui.properties().small_change,
            min,
            max,
            DEFAULT_SMALL_CHANGE_FRACTION,
        );
        let large = read_change_or(
            ui,
            slider,
            ui.properties().large_change,
            min,
            max,
            DEFAULT_LARGE_CHANGE_FRACTION,
        );
        let current = read_local_or(ui, slider, ui.properties().value, min);
        let next = match &event.key.key {
            Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowDown) => {
                current - keyboard_step(event.key.modifiers.shift(), small, large)
            }
            Key::Named(NamedKey::ArrowRight | NamedKey::ArrowUp) => {
                current + keyboard_step(event.key.modifiers.shift(), small, large)
            }
            Key::Named(NamedKey::PageDown) => current - large,
            Key::Named(NamedKey::PageUp) => current + large,
            Key::Named(NamedKey::Home) => min,
            Key::Named(NamedKey::End) => max,
            _ => return,
        };
        set_value(
            ui,
            slider,
            thumb,
            next.clamp(min, max),
            ThumbMotion::Animate,
        );
        event.mark_handled();
    }

    fn post_layout(&mut self, slider: ElementId, ui: &mut Ui) -> bool {
        sync_slider_thumb_offset_target(ui, slider)
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
        let track = cx.insert(TemplateChild::slot(
            SLOT_SLIDER_TRACK,
            Box::new(SurfacePartWidget),
        ));
        let track_inset_y = (SLIDER_HEIGHT - TRACK_HEIGHT) / 2.0;
        cx.set_local(
            track,
            layout_prop,
            Layout::new()
                .absolute_at(0.0, track_inset_y)
                .logical_size(SLIDER_WIDTH, TRACK_HEIGHT),
        );

        let axis = cx.insert(TemplateChild::slot(
            SLOT_SLIDER_AXIS,
            Box::new(SurfacePartWidget),
        ));
        cx.set_local(axis, layout_prop, axis_layout());
        let min = cx.read_local_or(owner, min_prop, 0.0);
        let max = cx.read_local_or(owner, max_prop, 1.0);
        let ruler = slider_ruler(min, max);
        for (index, mark) in ruler
            .marks()
            .iter()
            .copied()
            .filter(|mark| ruler_mark_in_view(&ruler, *mark))
            .enumerate()
        {
            let key = tick_key(index);
            let class = tick_class(mark.kind);
            let tick = cx.insert(
                TemplateChild::slot(SLOT_SLIDER_TICK, Box::new(SurfacePartWidget)).with_key(key),
            );
            cx.add_class(tick, class);
            cx.set_local(tick, layout_prop, tick_layout(mark));

            if mark.labeled {
                let label = cx.insert(
                    TemplateChild::slot(SLOT_SLIDER_TICK_LABEL, Box::new(TextWidget)).with_key(key),
                );
                cx.add_class(label, class);
                let content = format_tick_value(mark.value);
                cx.set_local(label, layout_prop, tick_label_layout(mark, &content));
                cx.set_local(label, content_prop, content);
            }
        }

        // Thumb: absolute, recomputed left whenever Value changes.
        let thumb = cx.insert(TemplateChild::slot(
            SLOT_SLIDER_THUMB,
            Box::new(SurfacePartWidget),
        ));
        let thumb_top = (SLIDER_HEIGHT - THUMB_DIAMETER) / 2.0;
        let initial_left = thumb_left_for_value(
            cx.read_local_or(owner, value_prop, 0.0),
            cx.read_local_or(owner, min_prop, 0.0),
            cx.read_local_or(owner, max_prop, 1.0),
        );
        cx.set_local(thumb, layout_prop, thumb_layout(initial_left, thumb_top));
    }
}

#[derive(Copy, Clone, Debug, Default)]
struct SliderDragState {
    offset_from_thumb_center: f64,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum ThumbMotion {
    Snap,
    Animate,
}

impl SliderDragState {
    fn update(
        &mut self,
        ui: &mut Ui,
        slider: ElementId,
        thumb: ElementId,
        press_position: kurbo::Point,
        position: kurbo::Point,
        delta: kurbo::Vec2,
    ) {
        if is_initial_drag_move(press_position, position, delta) {
            let thumb_bounds = ui.effective_bounds(thumb);
            self.offset_from_thumb_center = if thumb_bounds.contains(press_position) {
                press_position.x - thumb_bounds.center().x
            } else {
                0.0
            };
        }
        set_value_from_center_position(
            ui,
            slider,
            thumb,
            position.x - self.offset_from_thumb_center,
            ThumbMotion::Snap,
        );
    }
}

fn keyboard_step(large_requested: bool, small: f64, large: f64) -> f64 {
    if large_requested { large } else { small }
}

fn read_change_or(
    ui: &Ui,
    slider: ElementId,
    prop: understory_property::Property<f64>,
    min: f64,
    max: f64,
    default_fraction: f64,
) -> f64 {
    let configured = read_local_or(ui, slider, prop, 0.0);
    if configured.is_finite() && configured > 0.0 {
        configured
    } else {
        (max - min).abs() * default_fraction
    }
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

fn thumb_layout(left: f32, top: f32) -> Layout {
    Layout::new()
        .absolute_at(left, top)
        .logical_size(THUMB_DIAMETER, THUMB_DIAMETER)
}

fn axis_layout() -> Layout {
    Layout::new()
        .absolute()
        .logical_size(SLIDER_WIDTH, SLIDER_HEIGHT)
}

fn slider_ruler(min: f64, max: f64) -> AxisRuler1D {
    let mapping = AxisMapping1D::linear(0.0..f64::from(SLIDER_WIDTH), min..max);
    let scale = AxisScale1D::from_mapping(&mapping, slider_scale_options());
    AxisRuler1D::from_mapping(&mapping, &scale, slider_ruler_options())
}

fn slider_scale_options() -> AxisScaleOptions {
    AxisScaleOptions {
        target_major_spacing_px: f64::from(SLIDER_WIDTH) / 2.0,
        medium_label_min_spacing_px: 0.0,
        subdivision_policy: AxisSubdivisionPolicy::Fixed(2),
        ..AxisScaleOptions::default()
    }
}

fn slider_ruler_options() -> AxisRulerOptions {
    AxisRulerOptions {
        major_mark_extent: f64::from(MAJOR_TICK_HEIGHT),
        medium_mark_extent: f64::from(MINOR_TICK_HEIGHT),
        minor_mark_extent: f64::from(MINOR_TICK_HEIGHT),
    }
}

fn ruler_mark_in_view(ruler: &AxisRuler1D, mark: AxisRulerMark) -> bool {
    let span = ruler.view_span();
    const EPSILON: f64 = 1e-6;
    mark.view_position >= span.start - EPSILON && mark.view_position <= span.end + EPSILON
}

fn tick_layout(mark: AxisRulerMark) -> Layout {
    Layout::new()
        .absolute_at(tick_left(mark.view_position, TICK_WIDTH), TICK_TOP)
        .logical_size(TICK_WIDTH, f32_from_logical(mark.mark_extent))
}

fn tick_label_layout(mark: AxisRulerMark, label: &str) -> Layout {
    let width = label_width(label);
    Layout::new()
        .absolute_at(tick_left(mark.view_position, width), LABEL_TOP)
        .logical_size(width, SLIDER_HEIGHT - LABEL_TOP)
}

fn label_width(label: &str) -> f32 {
    let chars = u16::try_from(label.chars().count()).unwrap_or(u16::MAX);
    (f32::from(chars) * LABEL_CHAR_WIDTH).max(LABEL_MIN_WIDTH)
}

fn tick_left(view_position: f64, width: f32) -> f32 {
    (f32_from_logical(view_position) - width / 2.0).clamp(0.0, SLIDER_WIDTH - width)
}

fn tick_class(kind: AxisTickKind) -> ClassId {
    match kind {
        AxisTickKind::Major => CLASS_SLIDER_MAJOR_TICK,
        AxisTickKind::Medium | AxisTickKind::Minor => CLASS_SLIDER_MINOR_TICK,
    }
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
    let min = read_local_or(ui, slider, ui.properties().minimum, 0.0);
    let max = read_local_or(ui, slider, ui.properties().maximum, 1.0);
    let content_prop = ui.properties().content;
    let layout_prop = ui.properties().layout;
    let ruler = slider_ruler(min, max);
    for (index, mark) in ruler
        .marks()
        .iter()
        .copied()
        .filter(|mark| ruler_mark_in_view(&ruler, *mark))
        .enumerate()
    {
        if let Some(tick) = ui
            .template_instance(slider)
            .and_then(|instance| instance.keyed_slot(SLOT_SLIDER_TICK, tick_key(index)))
        {
            ui.remove_class(tick, CLASS_SLIDER_MAJOR_TICK);
            ui.remove_class(tick, CLASS_SLIDER_MINOR_TICK);
            ui.add_class(tick, tick_class(mark.kind));
            ui.set_local(tick, layout_prop, tick_layout(mark));
        }
        let Some(label) = ui
            .template_instance(slider)
            .and_then(|instance| instance.keyed_slot(SLOT_SLIDER_TICK_LABEL, tick_key(index)))
        else {
            continue;
        };
        ui.remove_class(label, CLASS_SLIDER_MAJOR_TICK);
        ui.remove_class(label, CLASS_SLIDER_MINOR_TICK);
        ui.add_class(label, tick_class(mark.kind));
        let content = format_tick_value(mark.value);
        ui.set_local(label, layout_prop, tick_label_layout(mark, &content));
        ui.set_local(label, content_prop, content);
    }
}

fn tick_key(index: usize) -> TemplateItemKey {
    let value = u64::try_from(index).expect("slider tick index should fit u64");
    TemplateItemKey::new(value)
}

fn set_value(ui: &mut Ui, slider: ElementId, thumb: ElementId, value: f64, motion: ThumbMotion) {
    let min = read_local_or(ui, slider, ui.properties().minimum, 0.0);
    let max = read_local_or(ui, slider, ui.properties().maximum, 1.0);
    let clamped = value.clamp(min, max);
    let value_prop = ui.properties().value;
    let layout_prop = ui.properties().layout;
    ui.set_local(slider, value_prop, clamped);
    if motion == ThumbMotion::Animate {
        ui.request_presentation_transition(
            thumb,
            TimelineTarget::VisualOutput(VisualOutput::LayerOffset),
            TransitionPolicy::interaction(THUMB_TRANSITION),
        );
    }
    let left = thumb_left_for_value(clamped, min, max);
    let top = (SLIDER_HEIGHT - THUMB_DIAMETER) / 2.0;
    ui.set_local(thumb, layout_prop, thumb_layout(left, top));
}

fn set_value_from_position(
    ui: &mut Ui,
    slider: ElementId,
    thumb: ElementId,
    pointer: kurbo::Point,
    motion: ThumbMotion,
) {
    set_value_from_center_position(ui, slider, thumb, pointer.x, motion);
}

fn set_value_from_center_position(
    ui: &mut Ui,
    slider: ElementId,
    thumb: ElementId,
    center_x: f64,
    motion: ThumbMotion,
) {
    let bounds = ui.effective_bounds(slider);
    if bounds.width() <= 0.0 {
        return;
    }
    let track_width = SLIDER_WIDTH - THUMB_DIAMETER;
    let usable_origin = bounds.origin().x + f64::from(THUMB_DIAMETER) / 2.0;
    let t = ((center_x - usable_origin) / f64::from(track_width)).clamp(0.0, 1.0);
    let min = read_local_or(ui, slider, ui.properties().minimum, 0.0);
    let max = read_local_or(ui, slider, ui.properties().maximum, 1.0);
    let value = min + t * (max - min);
    set_value(ui, slider, thumb, value, motion);
}

fn sync_slider_thumb_offset_target(ui: &mut Ui, slider: ElementId) -> bool {
    let Some(thumb) = ui.slot(slider, SLOT_SLIDER_THUMB) else {
        return false;
    };
    let slider_bounds = ui.effective_bounds(slider);
    let thumb_bounds = ui.effective_bounds(thumb);
    let target = kurbo::Vec2::new(
        thumb_bounds.x0 - slider_bounds.x0,
        thumb_bounds.y0 - slider_bounds.y0,
    );
    let property = TimelineTarget::VisualOutput(VisualOutput::LayerOffset);
    let cause = if ui.has_pending_presentation_transition(thumb, property) {
        ChangeCause::Interaction
    } else {
        ChangeCause::Other
    };
    ui.set_presentation_target(thumb, property, target, cause)
}

/// Programmatically set a slider's value (and move its thumb).
///
/// # Panics
///
/// Panics if `slider` is stale or doesn't have a thumb child.
pub fn set_slider_value(ui: &mut Ui, slider: ElementId, value: f64) {
    let thumb = ui
        .slot(slider, SLOT_SLIDER_THUMB)
        .expect("slider must have SLOT_SLIDER_THUMB");
    set_value(ui, slider, thumb, value, ThumbMotion::Snap);
}
