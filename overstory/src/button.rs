// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Button: the first templated control.
//!
//! The Button's surface presentation is identical to a generic element
//! (fill background, round corners) — the templated label child handles
//! text. What makes Button distinct is its default template, which
//! creates the text child + sets up one-way template bindings so
//! `Content` / `Foreground` / `FontSize` on the Button flow through to
//! the label automatically.

use understory_box_tree::NodeId;
use understory_presentation::PresentationStore;
use understory_property_binding::PropertyEndpoint;

use crate::behavior::{ElementBehavior, PresentContext};
use crate::element::{Element, ElementId, ElementKind};
use crate::generic::present_surface;
use crate::template::Template;
use crate::text_behavior::ELEMENT_KIND_TEXT;
use crate::ui::Ui;

/// Element kind for a templated button. Default behavior presents a
/// rounded surface and hosts a `Text` label sourced from `Content`.
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
    fn present(
        &self,
        element: &Element,
        ctx: &PresentContext<'_>,
        store: &mut PresentationStore<NodeId, ElementId>,
        node: NodeId,
    ) {
        // Button's own draw contribution is the same as Generic's
        // (rounded fill); the label child contributes the text.
        present_surface(element, ctx, store, node);
    }

    fn default_template(&self) -> Option<&'static dyn Template> {
        Some(&BUTTON_TEMPLATE)
    }
}
