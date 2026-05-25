// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `ProgressBar`: determinate value indicator with a templated fill.

use alloc::{boxed::Box, format, string::String};

use understory_style::{PartTag, TypeTag};

use crate::controls::support::read_local_or;
use crate::controls::text::TextWidget;
use crate::element::ElementId;
use crate::layouts::Layout;
use crate::template::{Template, TemplateContext, TemplateSlot};
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, SurfacePartWidget, TemplateOverrideWidget, Widget};

/// Selector identity for `ProgressBar`.
pub const TYPE_PROGRESS_BAR: TypeTag = TypeTag(7);
/// Owner-local part identity for the filled value surface.
pub const PART_PROGRESS_BAR_FILL: PartTag = PartTag(1);
/// Owner-local part identity for the optional text label.
pub const PART_PROGRESS_BAR_LABEL: PartTag = PartTag(2);
/// Semantic slot for the filled value surface.
pub const SLOT_PROGRESS_BAR_FILL: TemplateSlot =
    TemplateSlot::new("progress-fill").with_part_tag(PART_PROGRESS_BAR_FILL);
/// Semantic slot for the optional text label.
pub const SLOT_PROGRESS_BAR_LABEL: TemplateSlot =
    TemplateSlot::new("progress-label").with_part_tag(PART_PROGRESS_BAR_LABEL);

const DEFAULT_WIDTH: f32 = 260.0;
const DEFAULT_HEIGHT: f32 = 18.0;
const LABEL_FONT_SIZE: f64 = 11.0;

/// Builder + spec for a determinate progress bar.
pub struct ProgressBar {
    value: f64,
    min: f64,
    max: f64,
    width: f32,
    height: f32,
    show_label: bool,
    state: ControlState,
    template: Option<&'static dyn Template>,
}

impl core::fmt::Debug for ProgressBar {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ProgressBar")
            .field("value", &self.value)
            .field("min", &self.min)
            .field("max", &self.max)
            .field("show_label", &self.show_label)
            .finish_non_exhaustive()
    }
}

impl ProgressBar {
    /// Starts a new determinate progress bar.
    #[must_use]
    pub fn new() -> Self {
        Self {
            value: 0.0,
            min: 0.0,
            max: 1.0,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            show_label: true,
            state: ControlState::interactive().focusable(false),
            template: None,
        }
    }

    /// Sets the current value.
    #[must_use]
    pub fn value(mut self, value: f64) -> Self {
        self.value = value;
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

    /// Sets the fixed logical size.
    #[must_use]
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width.max(1.0);
        self.height = height.max(1.0);
        self
    }

    /// Sets whether the percent label is shown.
    #[must_use]
    pub fn label(mut self, show_label: bool) -> Self {
        self.show_label = show_label;
        self
    }

    /// Sets whether the progress bar starts enabled. Disabled progress bars
    /// are still visible, but style rules can use `:disabled` to dim them.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.state = self.state.enabled(enabled);
        self
    }

    /// Overrides the visual template used when this progress bar is inserted.
    ///
    /// Custom templates should create [`SLOT_PROGRESS_BAR_FILL`] when they want
    /// [`set_progress_value`] to update the filled extent.
    #[must_use]
    pub fn template(mut self, template: &'static dyn Template) -> Self {
        self.template = Some(template);
        self
    }
}

impl Default for ProgressBar {
    fn default() -> Self {
        Self::new()
    }
}

impl AppendSpec for ProgressBar {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let widget: Box<dyn Widget> = match self.template {
            Some(template) => Box::new(TemplateOverrideWidget::new(ProgressBarWidget, template)),
            None => Box::new(ProgressBarWidget),
        };
        let id = ui.append_widget_deferred_template(parent, widget);
        let min_prop = ui.properties().minimum;
        let max_prop = ui.properties().maximum;
        let layout_prop = ui.properties().layout;
        ui.set_local(id, min_prop, self.min);
        ui.set_local(id, max_prop, self.max);
        ui.set_local(
            id,
            layout_prop,
            Layout::row(0.0)
                .relative()
                .justify_center()
                .align_center()
                .logical_size(self.width, self.height),
        );
        ui.set_control_state(id, self.state);
        ui.expand_default_template(id);
        set_progress_label_visible(ui, id, self.show_label);
        set_progress_value(ui, id, self.value);
        id
    }
}

/// Per-element widget for `ProgressBar`.
#[derive(Debug, Default)]
pub(crate) struct ProgressBarWidget;

impl Widget for ProgressBarWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_PROGRESS_BAR)
    }

    fn default_template(&self) -> Option<&'static dyn Template> {
        Some(&PROGRESS_TEMPLATE)
    }
}

static PROGRESS_TEMPLATE: ProgressTemplate = ProgressTemplate;

#[derive(Debug, Default)]
struct ProgressTemplate;

impl Template for ProgressTemplate {
    fn instantiate(&self, cx: &mut TemplateContext<'_>) {
        let fill = cx.slot_widget(SLOT_PROGRESS_BAR_FILL, Box::new(SurfacePartWidget));
        cx.set_local(
            fill,
            cx.properties().layout,
            Layout::new().absolute_at(0.0, 0.0).percent_size(0.0, 1.0),
        );

        let label = cx.slot_widget(SLOT_PROGRESS_BAR_LABEL, Box::new(TextWidget));
        cx.set_local(label, cx.properties().font_size, LABEL_FONT_SIZE);
    }
}

/// Sets a progress bar's value and synchronizes its default fill and label
/// parts if they exist.
pub fn set_progress_value(ui: &mut Ui, progress: ElementId, value: f64) {
    let min = read_local_or(ui, progress, ui.properties().minimum, 0.0);
    let max = read_local_or(ui, progress, ui.properties().maximum, 1.0);
    let clamped = value.clamp(min.min(max), min.max(max));
    ui.set_local(progress, ui.properties().value, clamped);

    let fraction = progress_fraction(clamped, min, max);
    if let Some(fill) = ui.slot(progress, SLOT_PROGRESS_BAR_FILL) {
        ui.set_local(
            fill,
            ui.properties().layout,
            Layout::new()
                .absolute_at(0.0, 0.0)
                .percent_size(f32_from_unit_fraction(fraction), 1.0),
        );
    }
    if let Some(label) = ui.slot(progress, SLOT_PROGRESS_BAR_LABEL) {
        let content = label_for_fraction(fraction);
        ui.set_local(label, ui.properties().content, content);
    }
}

fn set_progress_label_visible(ui: &mut Ui, progress: ElementId, visible: bool) {
    let Some(label) = ui.slot(progress, SLOT_PROGRESS_BAR_LABEL) else {
        return;
    };
    if !visible {
        ui.set_local(label, ui.properties().content, String::new());
    }
}

fn progress_fraction(value: f64, min: f64, max: f64) -> f64 {
    let range = max - min;
    if !range.is_finite() || range.abs() <= f64::EPSILON {
        return 0.0;
    }
    ((value - min) / range).clamp(0.0, 1.0)
}

fn label_for_fraction(fraction: f64) -> String {
    format!("{:.0}%", fraction * 100.0)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "unit fractions fit f32 precisely enough for layout percentages"
)]
fn f32_from_unit_fraction(fraction: f64) -> f32 {
    fraction as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Size;
    use understory_property::DependencyObjectExt;

    #[test]
    fn progress_value_sizes_fill_and_updates_label() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(300.0, 40.0));
        let progress = ui.append(root, ProgressBar::new().value(0.25).size(200.0, 20.0));

        ui.compute_layout(Size::new(300.0, 40.0));

        let fill = ui.slot(progress, SLOT_PROGRESS_BAR_FILL).unwrap();
        let label = ui.slot(progress, SLOT_PROGRESS_BAR_LABEL).unwrap();
        assert_eq!(ui.effective_bounds(fill).width(), 50.0);
        assert_eq!(
            ui.get(label)
                .and_then(|el| el.get_local_value(ui.properties().content))
                .map(String::as_str),
            Some("25%")
        );
    }
}
