// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Runtime behavior for `Slider`.

use crate::behavior::{ArrangeContext, ArrangeResult, WidgetEventContext};
use crate::controls::range::ScalarRange;
use crate::controls::support::{action_numeric_value, is_initial_drag_move, read_local_or};
use crate::element::{Element, ElementId};
use crate::event_runtime::{ClickEvent, DragEvent, KeyEvent, PointerRoutedEvent, RoutedEventKind};
use crate::layouts::LayoutPlacement;
use crate::template::TemplateContract;
use crate::ui::Ui;
use crate::widget::Widget;

use super::events::{
    SliderValueChange, SliderValueChangePhase, SliderValueChangedEvent, SliderValueChangedHandler,
    SliderValueChangedSlot, SliderValueDelta,
};
use super::ids::{
    DEFAULT_LARGE_CHANGE_FRACTION, DEFAULT_SMALL_CHANGE_FRACTION, SLIDER_HEIGHT, SLIDER_WIDTH,
    SLOT_SLIDER_AXIS, SLOT_SLIDER_THUMB, SLOT_SLIDER_TICK, SLOT_SLIDER_TICK_LABEL,
    SLOT_SLIDER_TRACK, THUMB_DIAMETER, TYPE_SLIDER,
};
use super::template::format_tick_value;

/// Per-element widget for `Slider`.
///
/// Value, min, and max live in the property store. The widget owns interaction
/// state and keeps the default thumb slot synchronized to the resolved value.
pub(crate) struct SliderWidget {
    drag: SliderDragState,
    value_changed: SliderValueChangedSlot,
}

impl SliderWidget {
    pub(super) fn new(value_changed: Option<SliderValueChangedHandler>) -> Self {
        Self {
            drag: SliderDragState::default(),
            value_changed: SliderValueChangedSlot::new(value_changed),
        }
    }

    fn dispatch_value_changed(
        &self,
        ui: &mut Ui,
        slider: ElementId,
        change: Option<SliderValueChange>,
    ) {
        let Some(change) = change else {
            return;
        };
        self.value_changed.dispatch(
            ui,
            slider,
            SliderValueChangedEvent {
                element: slider,
                previous_value: change.previous_value,
                value: change.value,
                phase: change.phase,
            },
        );
    }
}

impl Default for SliderWidget {
    fn default() -> Self {
        Self::new(None)
    }
}

impl core::fmt::Debug for SliderWidget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SliderWidget").finish_non_exhaustive()
    }
}

impl Widget for SliderWidget {
    fn type_tag(&self) -> Option<understory_style::TypeTag> {
        Some(TYPE_SLIDER)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::Slider)
    }

    fn configure_accessibility(
        &self,
        element: &Element,
        ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        ctx.apply_numeric_range(element, node);
        node.set_orientation(accesskit::Orientation::Horizontal);
        node.set_value(format_tick_value(ctx.numeric_value(element)));
        if ctx.is_enabled(element.id()) {
            node.add_action(accesskit::Action::Increment);
            node.add_action(accesskit::Action::Decrement);
            node.add_action(accesskit::Action::SetValue);
        }
    }

    fn template_contract(&self) -> TemplateContract {
        TemplateContract::new()
            .with_required_slot(SLOT_SLIDER_THUMB)
            .with_recommended_slot(SLOT_SLIDER_TRACK)
            .with_optional_slot(SLOT_SLIDER_AXIS)
            .with_optional_slot(SLOT_SLIDER_TICK)
            .with_optional_slot(SLOT_SLIDER_TICK_LABEL)
    }

    fn handle_click(&mut self, ctx: &mut WidgetEventContext<'_>, event: ClickEvent<'_>) {
        let slider = ctx.id();
        let Some(thumb) = ctx.slot(slider, SLOT_SLIDER_THUMB) else {
            return;
        };
        if !event.is_semantic_target() {
            return;
        }
        if !ctx.is_enabled(slider) {
            return;
        }
        if event.source() == thumb {
            return;
        }
        let value =
            set_value_from_position(ctx, slider, thumb, event.position, ThumbMotion::Animate)
                .map(|delta| delta.into_change(SliderValueChangePhase::Committed));
        self.dispatch_value_changed(ctx, slider, value);
        event.mark_handled();
    }

    fn handle_drag(&mut self, ctx: &mut WidgetEventContext<'_>, event: DragEvent<'_>) {
        let slider = ctx.id();
        let Some(thumb) = ctx.slot(slider, SLOT_SLIDER_THUMB) else {
            return;
        };
        if !event.is_semantic_target() {
            return;
        }
        if !ctx.is_enabled(slider) {
            return;
        }
        let value = self.drag.update(
            ctx,
            slider,
            thumb,
            event.press_position,
            event.position,
            event.delta,
        );
        self.dispatch_value_changed(ctx, slider, value);
        event.mark_handled();
    }

    fn handle_pointer(&mut self, ctx: &mut WidgetEventContext<'_>, event: PointerRoutedEvent<'_>) {
        let slider = ctx.id();
        if !event.is_semantic_target() {
            return;
        }
        match event.kind {
            RoutedEventKind::PointerUp => {
                let value = self.drag.finish(ctx, slider);
                self.dispatch_value_changed(ctx, slider, value);
            }
            RoutedEventKind::PointerCancel => {
                self.drag.cancel();
            }
            _ => {}
        }
    }

    fn handle_key(&mut self, ctx: &mut WidgetEventContext<'_>, event: KeyEvent<'_>) {
        use ui_events::keyboard::{Key, KeyState, NamedKey};

        let slider = ctx.id();
        let Some(thumb) = ctx.slot(slider, SLOT_SLIDER_THUMB) else {
            return;
        };
        if !event.is_target() {
            return;
        }
        if event.key.state != KeyState::Down {
            return;
        }
        if !ctx.is_enabled(slider) {
            return;
        }
        let min = read_local_or(ctx, slider, ctx.properties().minimum, 0.0);
        let max = read_local_or(ctx, slider, ctx.properties().maximum, 1.0);
        let range = ScalarRange::new(min, max);
        let small = read_change_or(
            ctx,
            slider,
            ctx.properties().small_change,
            min,
            max,
            DEFAULT_SMALL_CHANGE_FRACTION,
        );
        let large = read_change_or(
            ctx,
            slider,
            ctx.properties().large_change,
            min,
            max,
            DEFAULT_LARGE_CHANGE_FRACTION,
        );
        let current = read_local_or(ctx, slider, ctx.properties().value, min);
        let key_step = if event.key.modifiers.shift() {
            large
        } else {
            small
        };
        let next = match &event.key.key {
            Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowDown) => current - key_step,
            Key::Named(NamedKey::ArrowRight | NamedKey::ArrowUp) => current + key_step,
            Key::Named(NamedKey::PageDown) => current - large,
            Key::Named(NamedKey::PageUp) => current + large,
            Key::Named(NamedKey::Home) => min,
            Key::Named(NamedKey::End) => max,
            _ => return,
        };
        let value = set_value(ctx, slider, thumb, range.clamp(next), ThumbMotion::Animate)
            .map(|delta| delta.into_change(SliderValueChangePhase::Committed));
        self.dispatch_value_changed(ctx, slider, value);
        event.mark_handled();
    }

    fn arrange(&mut self, ctx: &mut ArrangeContext<'_>) -> ArrangeResult {
        let slider = ctx.id();
        if sync_slider_thumb_layout(ctx.ui_mut(), slider) {
            return ArrangeResult::LAYOUT_CHANGED;
        }
        ArrangeResult::UNCHANGED
    }

    fn handle_accessibility_action(
        &mut self,
        ctx: &mut WidgetEventContext<'_>,
        request: &accesskit::ActionRequest,
    ) -> bool {
        let slider = ctx.id();
        if !ctx.is_enabled(slider) {
            return false;
        }
        let Some(element) = ctx.element() else {
            return false;
        };
        let min = ctx.get_value(element, ctx.properties().minimum);
        let max = ctx.get_value(element, ctx.properties().maximum);
        let current = ctx.get_value(element, ctx.properties().value);
        let next = match request.action {
            accesskit::Action::Increment => current + accessibility_step(ctx, element, min, max),
            accesskit::Action::Decrement => current - accessibility_step(ctx, element, min, max),
            accesskit::Action::SetValue => {
                let Some(value) = action_numeric_value(request) else {
                    return false;
                };
                value
            }
            _ => return false,
        };
        let Some(thumb) = ctx.slot(slider, SLOT_SLIDER_THUMB) else {
            return false;
        };
        let value = set_value(ctx, slider, thumb, next, ThumbMotion::Snap)
            .map(|delta| delta.into_change(SliderValueChangePhase::Committed));
        self.dispatch_value_changed(ctx, slider, value);
        true
    }
}

#[derive(Copy, Clone, Debug, Default)]
struct SliderDragState {
    offset_from_thumb_center: f64,
    start_value: f64,
    active: bool,
    changed: bool,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum ThumbMotion {
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
    ) -> Option<SliderValueChange> {
        if is_initial_drag_move(press_position, position, delta) {
            let thumb_bounds = ui.effective_bounds(thumb);
            self.offset_from_thumb_center = if thumb_bounds.contains(press_position) {
                press_position.x - thumb_bounds.center().x
            } else {
                0.0
            };
            self.start_value = slider_value(ui, slider);
            self.active = true;
            self.changed = false;
        }
        let change = set_value_from_center_position(
            ui,
            slider,
            thumb,
            position.x - self.offset_from_thumb_center,
            ThumbMotion::Snap,
        )
        .map(|delta| delta.into_change(SliderValueChangePhase::Changing));
        self.changed |= change.is_some();
        change
    }

    fn finish(&mut self, ui: &Ui, slider: ElementId) -> Option<SliderValueChange> {
        let should_commit = self.active && self.changed;
        let previous_value = self.start_value;
        let value = slider_value(ui, slider);
        self.cancel();
        (should_commit && previous_value != value).then_some(SliderValueChange {
            previous_value,
            value,
            phase: SliderValueChangePhase::Committed,
        })
    }

    fn cancel(&mut self) {
        self.active = false;
        self.changed = false;
    }
}

fn slider_value(ui: &Ui, slider: ElementId) -> f64 {
    let min = read_local_or(ui, slider, ui.properties().minimum, 0.0);
    let value_prop = ui.properties().value;
    ui.get(slider)
        .map_or(min, |element| ui.resolve_value_for(element, value_prop))
}

fn read_change_or(
    ui: &Ui,
    slider: ElementId,
    prop: understory_property::Property<f64>,
    min: f64,
    max: f64,
    default_fraction: f64,
) -> f64 {
    let range = ScalarRange::new(min, max);
    let configured = read_local_or(ui, slider, prop, 0.0);
    range.change_or_fraction(configured, default_fraction)
}

pub(super) fn thumb_left_for_value(value: f64, min: f64, max: f64) -> f64 {
    let t = ScalarRange::new(min, max).fraction(value);
    let travel = SLIDER_WIDTH - THUMB_DIAMETER;
    t * travel
}

fn accessibility_step(ui: &WidgetEventContext<'_>, element: &Element, min: f64, max: f64) -> f64 {
    let explicit = ui.get_value(element, ui.properties().small_change);
    ScalarRange::new(min, max).positive_change_or_fraction(
        explicit,
        DEFAULT_SMALL_CHANGE_FRACTION,
        0.01,
    )
}

pub(super) fn set_value(
    ui: &mut Ui,
    slider: ElementId,
    thumb: ElementId,
    value: f64,
    motion: ThumbMotion,
) -> Option<SliderValueDelta> {
    let min = read_local_or(ui, slider, ui.properties().minimum, 0.0);
    let max = read_local_or(ui, slider, ui.properties().maximum, 1.0);
    let value_prop = ui.properties().value;
    let previous = ui
        .get(slider)
        .map_or(min, |element| ui.resolve_value_for(element, value_prop));
    ui.set_local(slider, value_prop, value);
    let settled = slider_value(ui, slider);
    let left = thumb_left_for_value(settled, min, max);
    let top = (SLIDER_HEIGHT - THUMB_DIAMETER) / 2.0;
    let placement = LayoutPlacement::absolute_at(left, top);
    match motion {
        ThumbMotion::Animate => ui.set_layout_placement(thumb, placement),
        ThumbMotion::Snap => ui.set_layout_placement_without_motion(thumb, placement),
    }
    (previous != settled).then_some(SliderValueDelta {
        previous_value: previous,
        value: settled,
    })
}

fn set_value_from_position(
    ui: &mut Ui,
    slider: ElementId,
    thumb: ElementId,
    pointer: kurbo::Point,
    motion: ThumbMotion,
) -> Option<SliderValueDelta> {
    set_value_from_center_position(ui, slider, thumb, pointer.x, motion)
}

fn set_value_from_center_position(
    ui: &mut Ui,
    slider: ElementId,
    thumb: ElementId,
    center_x: f64,
    motion: ThumbMotion,
) -> Option<SliderValueDelta> {
    let bounds = ui.effective_bounds(slider);
    if bounds.width() <= 0.0 {
        return None;
    }
    let track_width = SLIDER_WIDTH - THUMB_DIAMETER;
    let usable_origin = bounds.origin().x + THUMB_DIAMETER / 2.0;
    let t = ((center_x - usable_origin) / track_width).clamp(0.0, 1.0);
    let min = read_local_or(ui, slider, ui.properties().minimum, 0.0);
    let max = read_local_or(ui, slider, ui.properties().maximum, 1.0);
    let value = ScalarRange::new(min, max).value_at_fraction(t);
    set_value(ui, slider, thumb, value, motion)
}

fn sync_slider_thumb_layout(ui: &mut Ui, slider: ElementId) -> bool {
    let Some(thumb) = ui.slot(slider, SLOT_SLIDER_THUMB) else {
        return false;
    };
    let (value, min, max) = {
        let Some(element) = ui.get(slider) else {
            return false;
        };
        (
            ui.resolve_value_for(element, ui.properties().value),
            ui.resolve_value_for(element, ui.properties().minimum),
            ui.resolve_value_for(element, ui.properties().maximum),
        )
    };
    let left = thumb_left_for_value(value, min, max);
    let top = (SLIDER_HEIGHT - THUMB_DIAMETER) / 2.0;
    let placement = LayoutPlacement::absolute_at(left, top);
    if ui.effective_layout_placement(thumb) == placement {
        return false;
    }
    ui.set_layout_placement_without_motion(thumb, placement);
    true
}

/// Programmatically set a slider's value (and move its thumb).
///
/// Returns `true` when the effective slider value changed. This does not
/// dispatch [`SliderValueChangedEvent`]; use a binding when other retained
/// properties should follow programmatic value writes.
///
/// # Panics
///
/// Panics if `slider` is stale or doesn't have a thumb child.
pub fn set_slider_value(ui: &mut Ui, slider: ElementId, value: f64) -> bool {
    let thumb = ui
        .slot(slider, SLOT_SLIDER_THUMB)
        .expect("slider must have SLOT_SLIDER_THUMB");
    set_value(ui, slider, thumb, value, ThumbMotion::Snap).is_some()
}
