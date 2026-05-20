// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Button: the first templated control.
//!
//! Phase 1f-5 ships a `Button` kind whose default template creates a `Text`
//! child for its content. The Button itself paints its background (rounded
//! by `CornerRadius`) and reacts to `:hover` / `:pressed` via stylesheet
//! rules. Phase 1f-6 will hook up `TemplateBinding` so `Button.Content`
//! flows through `understory_property_binding`; for 1f-5 the template just
//! copies the value at instantiation time.

use imaging::Painter;
use imaging::record::Scene;
use kurbo::{Rect, RoundedRect};
use peniko::Brush;
use understory_property_binding::PropertyEndpoint;

use crate::behavior::{ElementBehavior, PaintContext};
use crate::element::{Element, ElementId, ElementKind};
use crate::template::Template;
use crate::text_behavior::ELEMENT_KIND_TEXT;
use crate::ui::Ui;

/// Element kind for a templated button. Default behavior paints its
/// background (rounded by `CornerRadius`) and hosts a `Text` label sourced
/// from the `Content` property.
pub const ELEMENT_KIND_BUTTON: ElementKind = ElementKind::new(2);

/// Stateless behavior for [`ELEMENT_KIND_BUTTON`].
#[derive(Debug, Default)]
pub struct ButtonBehavior;

/// `'static` instance registered in [`crate::Ui::new`].
pub(crate) static BUTTON_BEHAVIOR: ButtonBehavior = ButtonBehavior;

/// `'static` template instance dispatched by [`ButtonBehavior::default_template`].
static BUTTON_TEMPLATE: ButtonTemplate = ButtonTemplate;

#[derive(Debug, Default)]
struct ButtonTemplate;

impl Template for ButtonTemplate {
    fn instantiate(&self, ui: &mut Ui, owner: ElementId) {
        let content_prop = ui.properties().content;
        let foreground_prop = ui.properties().foreground;
        let font_size_prop = ui.properties().font_size;

        let label = ui.insert_child(owner, ELEMENT_KIND_TEXT);

        // One-way template bindings: as the owner's Content / Foreground /
        // FontSize change at runtime, the label tracks them. The initial
        // values propagate on the next `flush_bindings` (which the frame
        // loop calls at the start of `compute_layout`).
        let _ = ui.bind(
            PropertyEndpoint::new(owner, content_prop),
            PropertyEndpoint::new(label, content_prop),
        );
        let _ = ui.bind(
            PropertyEndpoint::new(owner, foreground_prop),
            PropertyEndpoint::new(label, foreground_prop),
        );
        let _ = ui.bind(
            PropertyEndpoint::new(owner, font_size_prop),
            PropertyEndpoint::new(label, font_size_prop),
        );
    }
}

impl ElementBehavior for ButtonBehavior {
    fn paint(&self, element: &Element, ctx: &PaintContext<'_>, scene: &mut Scene) {
        // Same paint shape as GenericBehavior — fills the button's bounds
        // with Background, rounded by CornerRadius. The label child paints
        // on top via the normal tree walk.
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

    fn default_template(&self) -> Option<&'static dyn Template> {
        Some(&BUTTON_TEMPLATE)
    }
}
