// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `ProgressBar`: determinate value indicator with a templated fill.
//!
//! Roadmap:
//! - Keep `ProgressBar` a non-interactive status control; focus and pointer
//!   behavior belong elsewhere unless a future variant explicitly changes that.
//! - Model determinate, indeterminate, paused, and error states deliberately
//!   instead of overloading the current value range.
//! - Let templates own track, fill, and label shape while the control owns value
//!   normalization, formatting hooks, and state synchronization.
//! - Add accessibility metadata for range value, busy / indeterminate state, and
//!   label association.
//! - Share animation and reduced-motion policy with `Spinner` and the timing
//!   runtime rather than embedding one-off animation loops.

use alloc::{boxed::Box, format, string::String};

use understory_property::DependencyObjectExt;
use understory_style::{PartTag, TypeTag};

use crate::behavior::{ArrangeContext, ArrangeResult};
use crate::controls::range::ScalarRange;
use crate::controls::support::control_size;
use crate::controls::support::read_local_or;
use crate::element::{Element, ElementId};
use crate::layouts::{Layout, LayoutSize};
use crate::template::{
    TemplateContract, TemplateNode, TemplateSlot, TemplateSpec, TemplateSpecRecipe,
};
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, ControlTemplateSpec, ElementConfig, ElementSpec, Widget};

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

const DEFAULT_WIDTH: f64 = 260.0;
const DEFAULT_HEIGHT: f64 = 18.0;

/// Builder + spec for a determinate progress bar.
pub struct ProgressBar {
    value: f64,
    min: f64,
    max: f64,
    show_label: bool,
    element: ElementConfig,
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
        let mut element = ElementConfig::default();
        element.set_layout(progress_layout(DEFAULT_WIDTH, DEFAULT_HEIGHT));
        element.set_control_state(ControlState::interactive().focusable(false));
        Self {
            value: 0.0,
            min: 0.0,
            max: 1.0,
            show_label: true,
            element,
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
    pub fn size(mut self, width: f64, height: f64) -> Self {
        self.element.set_layout(progress_layout(width, height));
        self
    }

    /// Stretches the bar to fill its container width, with a fixed logical
    /// height, instead of the fixed pixel size set by [`Self::size`].
    #[must_use]
    pub fn fill_width(mut self, height: f64) -> Self {
        self.element.set_layout(progress_fill_layout(height));
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
        self.element
            .update_control_state(|state| state.enabled(enabled));
        self
    }

    /// Sets the shared construction-time control state.
    #[must_use]
    pub fn control_state(mut self, state: ControlState) -> Self {
        self.element.set_control_state(state);
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
        let id = ui.append_widget_deferred_template(
            parent,
            Box::new(ProgressBarWidget::new(self.show_label)),
        );
        let min_prop = ui.properties().minimum;
        let max_prop = ui.properties().maximum;
        ui.set_local(id, min_prop, self.min);
        ui.set_local(id, max_prop, self.max);
        self.element.apply_to(ui, id);
        ui.expand_resolved_template(id);
        set_progress_value(ui, id, self.value);
        id
    }
}

impl ControlTemplateSpec for ProgressBar {}

impl ElementSpec for ProgressBar {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

fn progress_layout(width: f64, height: f64) -> Layout {
    Layout::row(0.0)
        .relative()
        .justify_center()
        .align_center()
        .logical_size(width.max(1.0), height.max(1.0))
}

fn progress_fill_layout(height: f64) -> Layout {
    Layout::row(0.0)
        .relative()
        .justify_center()
        .align_center()
        .size(control_size(true, DEFAULT_WIDTH, height.max(1.0)))
}

/// Per-element widget for `ProgressBar`.
#[derive(Debug)]
pub(crate) struct ProgressBarWidget {
    /// Whether the default label projection writes percentage text.
    show_label: bool,
}

impl ProgressBarWidget {
    /// Creates a progress bar widget with label projection policy.
    const fn new(show_label: bool) -> Self {
        Self { show_label }
    }
}

impl Default for ProgressBarWidget {
    fn default() -> Self {
        Self::new(true)
    }
}

impl Widget for ProgressBarWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_PROGRESS_BAR)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::ProgressIndicator)
    }

    fn configure_accessibility(
        &self,
        element: &Element,
        ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        ctx.apply_numeric_range(element, node);
    }

    fn template_contract(&self) -> TemplateContract {
        TemplateContract::new()
            .with_required_slot(SLOT_PROGRESS_BAR_FILL)
            .with_optional_slot(SLOT_PROGRESS_BAR_LABEL)
    }

    fn arrange(&mut self, ctx: &mut ArrangeContext<'_>) -> ArrangeResult {
        let progress = ctx.id();
        ArrangeResult::from_layout_changed(sync_progress_parts(
            ctx.ui_mut(),
            progress,
            Some(self.show_label),
        ))
    }
}

static PROGRESS_TEMPLATE: TemplateSpecRecipe = TemplateSpecRecipe::new(progress_template_spec);

pub(crate) fn default_progress_template() -> crate::ControlTemplate {
    crate::ControlTemplate::from_static(&PROGRESS_TEMPLATE)
}

fn progress_template_spec() -> TemplateSpec {
    TemplateSpec::from_nodes([
        TemplateNode::surface_slot(SLOT_PROGRESS_BAR_FILL).with_layout(
            Layout::new()
                .absolute_at(0.0, 0.0)
                .size(LayoutSize::percent(0.0, 1.0)),
        ),
        TemplateNode::text_slot(SLOT_PROGRESS_BAR_LABEL),
    ])
}

/// Sets a progress bar's value and synchronizes its default fill and label
/// parts if they exist.
pub fn set_progress_value(ui: &mut Ui, progress: ElementId, value: f64) {
    let min = read_local_or(ui, progress, ui.properties().minimum, 0.0);
    let max = read_local_or(ui, progress, ui.properties().maximum, 1.0);
    let range = ScalarRange::new(min, max);
    let clamped = range.clamp(value);
    ui.set_local(progress, ui.properties().value, clamped);
    sync_progress_parts(ui, progress, None);
}

fn sync_progress_parts(ui: &mut Ui, progress: ElementId, show_label: Option<bool>) -> bool {
    let min = read_local_or(ui, progress, ui.properties().minimum, 0.0);
    let max = read_local_or(ui, progress, ui.properties().maximum, 1.0);
    let range = ScalarRange::new(min, max);
    let value_prop = ui.properties().value;
    let value = read_local_or(ui, progress, value_prop, 0.0);
    let clamped = range.clamp(value);
    let mut changed = false;

    if ui
        .get(progress)
        .and_then(|element| element.get_local_value(value_prop))
        .copied()
        != Some(clamped)
    {
        ui.set_local(progress, value_prop, clamped);
        changed = true;
    }

    let fraction = range.fraction(clamped);
    if let Some(fill) = ui.slot(progress, SLOT_PROGRESS_BAR_FILL) {
        let size = LayoutSize::percent(fraction.clamp(0.0, 1.0), 1.0);
        let size_changed = ui
            .get(fill)
            .and_then(|element| element.get_local_value(ui.properties().layout_size))
            != Some(&size);
        if size_changed {
            ui.set_layout_size(fill, size);
            changed = true;
        }
    }
    if let Some(label) = ui.slot(progress, SLOT_PROGRESS_BAR_LABEL) {
        let content = if show_label.unwrap_or_else(|| progress_label_visible(ui, progress)) {
            label_for_fraction(fraction)
        } else {
            String::new()
        };
        let text_changed = ui
            .get(label)
            .and_then(|element| element.get_local_value(ui.properties().text))
            != Some(&content);
        if text_changed {
            ui.set_local(label, ui.properties().text, content);
            changed = true;
        }
    }

    changed
}

fn progress_label_visible(ui: &Ui, progress: ElementId) -> bool {
    ui.get(progress)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<ProgressBarWidget>())
        .is_none_or(|widget| widget.show_label)
}

fn label_for_fraction(fraction: f64) -> String {
    format!("{:.0}%", fraction * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accessibility::accesskit_node_id;
    use accesskit::{Node, Role, TreeUpdate};
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
                .and_then(|el| el.get_local_value(ui.properties().text))
                .map(String::as_str),
            Some("25%")
        );
    }

    #[test]
    fn direct_value_write_sizes_fill_and_updates_label() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(300.0, 40.0));
        let progress = ui.append(root, ProgressBar::new().value(0.25).size(200.0, 20.0));
        ui.compute_layout(Size::new(300.0, 40.0));

        ui.set_local(progress, ui.properties().value, 0.75);
        ui.compute_layout(Size::new(300.0, 40.0));

        let fill = ui.slot(progress, SLOT_PROGRESS_BAR_FILL).unwrap();
        let label = ui.slot(progress, SLOT_PROGRESS_BAR_LABEL).unwrap();
        assert_eq!(ui.effective_bounds(fill).width(), 150.0);
        assert_eq!(
            ui.get(label)
                .and_then(|el| el.get_local_value(ui.properties().text))
                .map(String::as_str),
            Some("75%")
        );
    }

    #[test]
    fn hidden_progress_label_stays_empty_after_value_changes() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).logical_size(300.0, 40.0));
        let progress = ui.append(
            root,
            ProgressBar::new()
                .value(0.25)
                .size(200.0, 20.0)
                .label(false),
        );
        ui.compute_layout(Size::new(300.0, 40.0));

        ui.set_local(progress, ui.properties().value, 0.75);
        ui.compute_layout(Size::new(300.0, 40.0));

        let fill = ui.slot(progress, SLOT_PROGRESS_BAR_FILL).unwrap();
        let label = ui.slot(progress, SLOT_PROGRESS_BAR_LABEL).unwrap();
        assert_eq!(ui.effective_bounds(fill).width(), 150.0);
        assert_eq!(
            ui.get(label)
                .and_then(|el| el.get_local_value(ui.properties().text))
                .map(String::as_str),
            Some("")
        );
    }

    #[test]
    fn progress_accessibility_exposes_determinate_range() {
        let mut ui = Ui::new();
        let root = ui.root();
        let progress = ui.append(root, ProgressBar::new().min(10.0).max(20.0).value(15.0));
        ui.compute_layout(Size::new(300.0, 40.0));

        let update = ui.accesskit_tree_update();
        let node = accesskit_node_for(&update, progress);

        assert_eq!(node.role(), Role::ProgressIndicator);
        assert_eq!(node.numeric_value(), Some(15.0));
        assert_eq!(node.min_numeric_value(), Some(10.0));
        assert_eq!(node.max_numeric_value(), Some(20.0));
        assert!(!node.is_busy());
    }

    fn accesskit_node_for(update: &TreeUpdate, element: ElementId) -> &Node {
        update
            .nodes
            .iter()
            .find(|(id, _)| *id == accesskit_node_id(element))
            .map(|(_, node)| node)
            .expect("node should exist")
    }
}
