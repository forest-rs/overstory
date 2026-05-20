// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Behavior implementation for [`crate::ELEMENT_KIND_GENERIC`].
//!
//! Fills the element's `Bounds` with the `Background` brush, rounding to
//! `CornerRadius`. If either bounds is zero-area or background is `None`, the
//! element contributes nothing to the frame.

use imaging::Painter;
use imaging::record::Scene;
use kurbo::{Rect, RoundedRect};
use peniko::Brush;

use crate::behavior::{ElementBehavior, PaintContext};
use crate::element::Element;

/// The generic kind's behavior. Stateless; instantiated as a `'static`.
#[derive(Debug, Default)]
pub struct GenericBehavior;

/// `'static` instance registered in [`crate::Ui::new`].
pub(crate) static GENERIC_BEHAVIOR: GenericBehavior = GenericBehavior;

impl ElementBehavior for GenericBehavior {
    fn paint(&self, element: &Element, ctx: &PaintContext<'_>, scene: &mut Scene) {
        let cx = ctx.resolve_cx();
        let style = Some((ctx.cascade, ctx.match_state));

        let bounds: Rect = cx.get_value(element, ctx.properties.bounds, style);
        if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
            return;
        }
        let background: Option<Brush> = cx.get_value(element, ctx.properties.background, style);
        let Some(brush) = background else {
            return;
        };
        let corner_radius: f64 = cx.get_value(element, ctx.properties.corner_radius, style);

        let mut painter = Painter::new(scene);
        if corner_radius > 0.0 {
            let rounded = RoundedRect::from_rect(bounds, corner_radius);
            painter.fill(rounded, &brush).draw();
        } else {
            painter.fill(bounds, &brush).draw();
        }
    }
}
