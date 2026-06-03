// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Spinner`: animated 8-dot busy indicator.
//!
//! Roadmap:
//! - Keep `Spinner` an indeterminate busy indicator; determinate progress
//!   belongs in `ProgressBar`.
//! - Move size, color, timing, and reduced-motion policy into style / timing
//!   hooks instead of baking them into the current default.
//! - Consider lowering to a single presentation primitive or keyed scene when
//!   the cost of many child panels matters.
//! - Coordinate with host frame pacing and accessibility so busy state is
//!   visible to assistive technologies without producing noisy announcements.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::f32::consts::FRAC_1_SQRT_2 as FRAC_1_SQRT_2_F32;

use peniko::{Brush, Color};
use understory_style::TypeTag;
use understory_timing::TimerDuration;

use crate::controls::panel::Panel;
use crate::element::{Element, ElementId};
use crate::layouts::Layout;
use crate::motion::millis;
use crate::ui::Ui;
use crate::widget::{AppendSpec, ElementConfig, ElementSpec, Widget};

/// Selector identity for `Spinner`.
pub const TYPE_SPINNER: TypeTag = TypeTag(5);

const SPINNER_SIZE: f32 = 36.0;
const DOT_DIAMETER: f32 = 7.0;
const RING_RADIUS: f32 = (SPINNER_SIZE - DOT_DIAMETER) / 2.0;
const DOT_INSET_CENTER: f32 = SPINNER_SIZE / 2.0 - DOT_DIAMETER / 2.0;
const STEP_INTERVAL: TimerDuration = millis(90);
const DOT_COUNT: usize = 8;
const DOT_VECTORS: [(f32, f32); DOT_COUNT] = [
    (0.0, -1.0),
    (FRAC_1_SQRT_2_F32, -FRAC_1_SQRT_2_F32),
    (1.0, 0.0),
    (FRAC_1_SQRT_2_F32, FRAC_1_SQRT_2_F32),
    (0.0, 1.0),
    (-FRAC_1_SQRT_2_F32, FRAC_1_SQRT_2_F32),
    (-1.0, 0.0),
    (-FRAC_1_SQRT_2_F32, -FRAC_1_SQRT_2_F32),
];
const DOT_ALPHAS: [u8; DOT_COUNT] = [255, 208, 168, 132, 100, 72, 48, 28];

/// Builder + spec for a `Spinner`. Currently config-free; the size
/// and tick interval are baked in. Future builder methods will hang
/// here when there's a real need.
#[derive(Debug, Default)]
pub struct Spinner {
    element: ElementConfig,
}

impl Spinner {
    /// Starts a new `Spinner` builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl AppendSpec for Spinner {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget(parent, Box::new(SpinnerWidget));

        // Pin the spinner to a fixed square. Position relative so the
        // dots' absolute inset is anchored here.
        ui.set_layout(
            id,
            Layout::new()
                .relative()
                .logical_size(SPINNER_SIZE, SPINNER_SIZE),
        );
        self.element.apply_to(ui, id);

        // Build dot children.
        let mut dots: Vec<ElementId> = Vec::with_capacity(DOT_COUNT);
        let bg_prop = ui.properties().background;
        let corner_prop = ui.properties().corner_radius;
        for (i, &(ux, uy)) in DOT_VECTORS.iter().enumerate() {
            let dot = ui.append(
                id,
                Panel::new().layout(
                    Layout::new()
                        .absolute_at(
                            DOT_INSET_CENTER + RING_RADIUS * ux,
                            DOT_INSET_CENTER + RING_RADIUS * uy,
                        )
                        .logical_size(DOT_DIAMETER, DOT_DIAMETER),
                ),
            );
            ui.set_local(dot, corner_prop, f64::from(DOT_DIAMETER * 0.5));
            ui.set_local(dot, bg_prop, Some(dot_brush(i, 0)));
            dots.push(dot);
        }

        // Schedule the repeating animation timer and register the
        // phase-rotating handler. Both the dots list and the running
        // phase counter are captured by the closure.
        let _ = ui.schedule_timer_repeating(id, STEP_INTERVAL);
        let mut phase: usize = 0;
        ui.on_timer(id, move |ui, _event| {
            phase = (phase + 1) % DOT_COUNT;
            let bg = ui.properties().background;
            for (i, &dot_id) in dots.iter().enumerate() {
                ui.set_local(dot_id, bg, Some(dot_brush(i, phase)));
            }
        });

        id
    }
}

impl ElementSpec for Spinner {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

/// Per-element widget for `Spinner`. Stateless — the running phase
/// lives in the timer-handler closure that was registered during
/// `append_to`.
#[derive(Debug, Default)]
pub(crate) struct SpinnerWidget;

impl Widget for SpinnerWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_SPINNER)
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
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        node.set_busy();
    }
}

fn dot_brush(dot_index: usize, phase: usize) -> Brush {
    let table_index = (DOT_COUNT + dot_index - phase) % DOT_COUNT;
    let alpha = DOT_ALPHAS[table_index];
    Brush::Solid(Color::from_rgba8(0xff, 0xff, 0xff, alpha))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accessibility::accesskit_node_id;
    use accesskit::{Node, Role, TreeUpdate};
    use kurbo::Size;

    #[test]
    fn spinner_accessibility_marks_indeterminate_busy_state() {
        let mut ui = Ui::new();
        let root = ui.root();
        let spinner = ui.append(root, Spinner::new());
        ui.compute_layout(Size::new(80.0, 80.0));

        let update = ui.accesskit_tree_update();
        let node = accesskit_node_for(&update, spinner);

        assert_eq!(node.role(), Role::ProgressIndicator);
        assert!(node.is_busy());
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
