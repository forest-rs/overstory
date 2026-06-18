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
//!   minor tick styling, and richer drag commit policy without hard-coding a
//!   single visual layout.
//! - Make pointer, touch, and wheel behavior line up with native slider
//!   expectations.
//! - Keep keyboard and accessibility adjustments on the same value path as
//!   pointer input, with range semantics exposed to accessibility clients.

mod behavior;
mod events;
mod ids;
mod template;

use alloc::boxed::Box;

use crate::controls::range::ScalarRange;
use crate::element::ElementId;
use crate::layouts::Layout;
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, ControlTemplateSpec, ElementConfig, ElementSpec};

use behavior::{SliderWidget, ThumbMotion, set_value};
use events::SliderValueChangedHandler;
use ids::{SLIDER_HEIGHT, SLIDER_WIDTH};
use template::sync_slider_tick_labels;

pub use behavior::set_slider_value;
pub use events::{SliderValueChangePhase, SliderValueChangedEvent};
pub use ids::{
    CLASS_SLIDER_MAJOR_TICK, CLASS_SLIDER_MINOR_TICK, PART_SLIDER_AXIS, PART_SLIDER_THUMB,
    PART_SLIDER_TICK, PART_SLIDER_TICK_LABEL, PART_SLIDER_TRACK, SLOT_SLIDER_AXIS,
    SLOT_SLIDER_THUMB, SLOT_SLIDER_TICK, SLOT_SLIDER_TICK_LABEL, SLOT_SLIDER_TRACK, TYPE_SLIDER,
};
pub(crate) use template::default_slider_template;

/// Builder + spec for a `Slider`.
pub struct Slider {
    initial_value: f64,
    min: f64,
    max: f64,
    small_change: Option<f64>,
    large_change: Option<f64>,
    on_value_changed: Option<SliderValueChangedHandler>,
    element: ElementConfig,
}

impl core::fmt::Debug for Slider {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Slider")
            .field("initial_value", &self.initial_value)
            .field("min", &self.min)
            .field("max", &self.max)
            .field("small_change", &self.small_change)
            .field("large_change", &self.large_change)
            .field(
                "has_value_changed_handler",
                &self.on_value_changed.is_some(),
            )
            .finish_non_exhaustive()
    }
}

impl Slider {
    /// Starts a new `Slider` builder.
    #[must_use]
    pub fn new() -> Self {
        let mut element = ElementConfig::default();
        element.set_layout(
            Layout::new()
                .relative()
                .logical_size(SLIDER_WIDTH, SLIDER_HEIGHT),
        );
        element.set_control_state(ControlState::interactive());
        Self {
            initial_value: 0.0,
            min: 0.0,
            max: 1.0,
            small_change: None,
            large_change: None,
            on_value_changed: None,
            element,
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

    /// Sets the inclusive value range.
    ///
    /// The lower and upper values may be reversed; slider math preserves that
    /// direction while clamping to the numeric span between them.
    #[must_use]
    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.min = min;
        self.max = max;
        self
    }

    /// Sets the small interaction step.
    ///
    /// This is the increment used by arrow keys and accessibility increment /
    /// decrement actions. Non-positive or non-finite values are ignored and the
    /// control derives a range-relative default instead.
    #[must_use]
    pub fn step(mut self, value: f64) -> Self {
        self.small_change = Some(value);
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

    /// Registers a handler fired after user interaction changes the value.
    ///
    /// Programmatic property writes and bindings should observe the value
    /// property through the binding system. This callback is for immediate
    /// side effects from slider input such as updating app state or previewing
    /// a setting.
    #[must_use]
    pub fn on_value_changed<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, SliderValueChangedEvent) + 'static,
    {
        self.on_value_changed = Some(Box::new(handler));
        self
    }

    /// Sets whether the slider participates in focus traversal.
    #[must_use]
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.element
            .update_control_state(|state| state.focusable(focusable));
        self
    }

    /// Sets whether the slider starts enabled. Disabled sliders are removed
    /// from focus traversal, ignore user activation, and carry `:disabled` for
    /// styling.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.element
            .update_control_state(|state| state.enabled(enabled));
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
        self.element.set_control_state(state);
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
        let id = ui.append_widget_deferred_template(
            parent,
            Box::new(SliderWidget::new(self.on_value_changed)),
        );
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
        self.element.apply_to(ui, id);
        ui.expand_resolved_template(id);
        sync_slider_tick_labels(ui, id);
        // Initial value: drive through the same setter the runtime
        // path uses so the thumb's Layout matches. The template has
        // already expanded by this point, so the thumb child exists.
        let clamped = ScalarRange::new(self.min, self.max).clamp(self.initial_value);
        let thumb = ui
            .slot(id, SLOT_SLIDER_THUMB)
            .expect("slider template should create SLOT_SLIDER_THUMB");
        let _ = set_value(ui, id, thumb, clamped, ThumbMotion::Snap);
        id
    }
}

impl ControlTemplateSpec for Slider {}

impl ElementSpec for Slider {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

#[cfg(test)]
mod tests;
