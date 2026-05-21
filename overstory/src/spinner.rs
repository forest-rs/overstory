// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Spinner`: animated 8-dot busy indicator.
//!
//! Structure:
//!
//! ```text
//! Spinner (square layout container, fixed size)
//!   ├─ dot 0 (top, generic)
//!   ├─ dot 1 (top-right, generic)
//!   ├─ … 6 more …
//!   └─ dot 7 (top-left, generic)
//! ```
//!
//! The dots themselves never move. Instead, a repeating timer
//! rotates a `phase` index 0..8; each frame the spinner's timer
//! handler rewrites each dot's `Background` to a foreground brush
//! at a phase-dependent alpha, which makes the bright "head" travel
//! around the ring. Eight discrete alpha levels give the typical
//! "comet tail" look.
//!
//! The animation lives entirely on the timer + property update —
//! no special compositor / transform plumbing. Each property write
//! marks `PAINT` (which cascades to `COMPOSE`), so the lowerer
//! re-emits the spinner's draws on the next frame.

use alloc::vec::Vec;
use core::f32::consts::FRAC_1_SQRT_2 as FRAC_1_SQRT_2_F32;

use peniko::{Brush, Color};
use understory_box_tree::NodeId;
use understory_presentation::PresentationStore;

use crate::behavior::{ElementBehavior, PresentContext};
use crate::element::{ELEMENT_KIND_GENERIC, Element, ElementId, ElementKind};
use crate::generic::present_surface;
use crate::template::Template;
use crate::ui::Ui;

/// Element kind for the spinner container.
pub const ELEMENT_KIND_SPINNER: ElementKind = ElementKind::new(5);

/// Pixel size of the spinner's square bounding box. Dots are sized
/// proportionally.
const SPINNER_SIZE: f32 = 36.0;
/// Diameter of each dot. The spacing is chosen so eight dots fit
/// snugly around the rim.
const DOT_DIAMETER: f32 = 7.0;
/// Distance from the spinner center to each dot's center.
const RING_RADIUS: f32 = (SPINNER_SIZE - DOT_DIAMETER) / 2.0;
/// Offset of the (top-left corner of the) dot bounding box so that
/// the dot's center sits on the ring.
const DOT_INSET_CENTER: f32 = SPINNER_SIZE / 2.0 - DOT_DIAMETER / 2.0;
/// Repeat interval — one phase step every 90 ms gives an 8-step
/// rotation that completes in ~720 ms, which reads as "busy"
/// without being frantic.
const STEP_INTERVAL_NANOS: u64 = 90_000_000;
/// Number of dots. Keeping this in sync with [`DOT_VECTORS`] and
/// [`DOT_ALPHAS`] is a manual job.
const DOT_COUNT: usize = 8;
/// Unit vectors from the spinner center to each dot, ordered
/// clockwise starting from "12 o'clock."
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
/// Alpha values for each "comet tail" position, brightest first.
/// At any given phase, dot `i` reads alpha `DOT_ALPHAS[(i - phase) mod 8]`,
/// so the brightest dot rotates around the ring as phase increments.
const DOT_ALPHAS: [u8; DOT_COUNT] = [255, 208, 168, 132, 100, 72, 48, 28];

/// Stateless behavior for [`ELEMENT_KIND_SPINNER`]. The spinner's
/// own surface is invisible (no background, no border); the dots
/// are children that own their own presentation.
#[derive(Debug, Default)]
pub struct SpinnerBehavior;

/// `'static` instance registered in [`crate::Ui::new`].
pub(crate) static SPINNER_BEHAVIOR: SpinnerBehavior = SpinnerBehavior;

/// `'static` template dispatched by [`SpinnerBehavior::default_template`].
static SPINNER_TEMPLATE: SpinnerTemplate = SpinnerTemplate;

#[derive(Debug, Default)]
struct SpinnerTemplate;

impl Template for SpinnerTemplate {
    fn instantiate(&self, ui: &mut Ui, owner: ElementId) {
        let layout_prop = ui.properties().layout;
        let bg_prop = ui.properties().background;
        let corner_prop = ui.properties().corner_radius;

        // Pin the spinner to a fixed square. Position is relative
        // so the dots' absolute inset is anchored here.
        ui.set_local(
            owner,
            layout_prop,
            taffy::Style {
                position: taffy::Position::Relative,
                size: taffy::Size {
                    width: taffy::Dimension::length(SPINNER_SIZE),
                    height: taffy::Dimension::length(SPINNER_SIZE),
                },
                ..taffy::Style::default()
            },
        );

        // Create the eight dots and record their ids on the
        // spinner element so the timer handler can update them
        // without re-walking the tree each tick.
        let mut dots: Vec<ElementId> = Vec::with_capacity(DOT_COUNT);
        for (i, &(ux, uy)) in DOT_VECTORS.iter().enumerate() {
            let dot = ui.insert_child(owner, ELEMENT_KIND_GENERIC);
            let left = DOT_INSET_CENTER + RING_RADIUS * ux;
            let top = DOT_INSET_CENTER + RING_RADIUS * uy;
            ui.set_local(
                dot,
                layout_prop,
                taffy::Style {
                    position: taffy::Position::Absolute,
                    inset: taffy::Rect {
                        left: taffy::LengthPercentageAuto::length(left),
                        top: taffy::LengthPercentageAuto::length(top),
                        right: taffy::LengthPercentageAuto::auto(),
                        bottom: taffy::LengthPercentageAuto::auto(),
                    },
                    size: taffy::Size {
                        width: taffy::Dimension::length(DOT_DIAMETER),
                        height: taffy::Dimension::length(DOT_DIAMETER),
                    },
                    ..taffy::Style::default()
                },
            );
            ui.set_local(dot, corner_prop, f64::from(DOT_DIAMETER * 0.5));
            ui.set_local(dot, bg_prop, Some(dot_brush(i, 0)));
            dots.push(dot);
        }

        // Repeating timer that advances the phase index. The handler
        // closure owns the dot list and the running phase counter.
        let _ = ui.schedule_timer_repeating(owner, STEP_INTERVAL_NANOS);
        let mut phase: usize = 0;
        ui.on_timer(owner, move |ui, _event| {
            phase = (phase + 1) % DOT_COUNT;
            let bg = ui.properties().background;
            for (i, &dot_id) in dots.iter().enumerate() {
                ui.set_local(dot_id, bg, Some(dot_brush(i, phase)));
            }
        });
    }
}

impl ElementBehavior for SpinnerBehavior {
    fn present(
        &self,
        element: &Element,
        ctx: &PresentContext<'_>,
        store: &mut PresentationStore<NodeId, ElementId>,
        node: NodeId,
    ) {
        // The spinner's own surface honors `Background` / `Border`
        // like any generic, so a focus ring or backdrop fill would
        // still render. By default it's invisible — only the dot
        // children draw.
        present_surface(element, ctx, store, node);
    }

    fn default_template(&self) -> Option<&'static dyn Template> {
        Some(&SPINNER_TEMPLATE)
    }
}

/// Computes the brush a given dot should paint with at a given
/// phase. The "head" dot (phase index modulo 8) is fully opaque
/// white; the rest fade through the alpha table going backwards
/// around the ring.
fn dot_brush(dot_index: usize, phase: usize) -> Brush {
    let table_index = (DOT_COUNT + dot_index - phase) % DOT_COUNT;
    let alpha = DOT_ALPHAS[table_index];
    Brush::Solid(Color::from_rgba8(0xff, 0xff, 0xff, alpha))
}
