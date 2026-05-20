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
use understory_property::DependencyObjectExt;

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
        // Capture the button's initial content / foreground so the label
        // shows them. Phase 1f-6 will replace this snapshot with a
        // TemplateBinding subscription so later writes propagate.
        let content_prop = ui.properties().content;
        let foreground_prop = ui.properties().foreground;
        let font_size_prop = ui.properties().font_size;

        let initial_content = ui
            .get(owner)
            .and_then(|e| e.get_local_value(content_prop).cloned())
            .unwrap_or_default();
        let initial_foreground = ui
            .get(owner)
            .and_then(|e| e.get_local_value(foreground_prop).cloned());
        let initial_font_size = ui
            .get(owner)
            .and_then(|e| e.get_local_value(font_size_prop).copied());

        // The button's layout is whatever its containing flex sets up;
        // inside, the label sits in the center.
        let label = ui.insert_child(owner, ELEMENT_KIND_TEXT);
        ui.set_local(label, content_prop, initial_content);
        if let Some(brush) = initial_foreground {
            ui.set_local(label, foreground_prop, brush);
        }
        if let Some(font_size) = initial_font_size {
            ui.set_local(label, font_size_prop, font_size);
        }
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
