// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `ToggleSwitch`: the second templated control.
//!
//! Structure:
//!
//! ```text
//! Toggle (focusable, clickable, holds `IsChecked`)
//!   └─ ToggleThumb (template-created)
//! ```
//!
//! The slide-the-thumb visual comes from Toggle's flex layout: the
//! cascade carries `justify-content: start` for the unchecked state
//! and `justify-content: end` for `Toggle:checked`, so when
//! [`PSEUDO_CHECKED`] flips the next layout pass moves the thumb. No
//! descendant selectors or manual thumb-positioning required.
//!
//! Click / Space / Enter activate the toggle. The default template
//! attaches click and key handlers that flip `IsChecked` *and* the
//! `:checked` pseudoclass together, keeping the cascade in sync with
//! the property. Both phases on the click handler fire
//! (Target when the Toggle itself is the hit, Bubble when the thumb
//! is the hit) for the same WPF-style routed-event semantics as
//! Button.

use understory_box_tree::NodeId;
use understory_presentation::PresentationStore;
use understory_property::DependencyObjectExt;

use crate::Phase;
use crate::behavior::{ElementBehavior, PresentContext};
use crate::element::{Element, ElementId, ElementKind};
use crate::generic::present_surface;
use crate::pseudos::PSEUDO_CHECKED;
use crate::template::Template;
use crate::ui::Ui;

/// Element kind for a toggle switch — the focusable, clickable
/// container that holds `IsChecked`.
pub const ELEMENT_KIND_TOGGLE: ElementKind = ElementKind::new(3);

/// Element kind for a toggle switch's thumb — the inner circle
/// positioned by the parent's flex layout.
pub const ELEMENT_KIND_TOGGLE_THUMB: ElementKind = ElementKind::new(4);

/// Stateless behavior for [`ELEMENT_KIND_TOGGLE`].
#[derive(Debug, Default)]
pub struct ToggleBehavior;

/// Stateless behavior for [`ELEMENT_KIND_TOGGLE_THUMB`].
#[derive(Debug, Default)]
pub struct ToggleThumbBehavior;

/// `'static` instances registered in [`crate::Ui::new`].
pub(crate) static TOGGLE_BEHAVIOR: ToggleBehavior = ToggleBehavior;
pub(crate) static TOGGLE_THUMB_BEHAVIOR: ToggleThumbBehavior = ToggleThumbBehavior;

/// `'static` template dispatched by [`ToggleBehavior::default_template`].
static TOGGLE_TEMPLATE: ToggleTemplate = ToggleTemplate;

#[derive(Debug, Default)]
struct ToggleTemplate;

impl Template for ToggleTemplate {
    fn instantiate(&self, ui: &mut Ui, owner: ElementId) {
        // Thumb part. Sized + styled by the cascade.
        let _thumb = ui.insert_child(owner, ELEMENT_KIND_TOGGLE_THUMB);

        // Click handler: flip checked on Target or Bubble (the thumb
        // child becomes the hit target for clicks inside the toggle,
        // so we need both phases — same as Button).
        ui.on_click(owner, move |ui, event| {
            if !matches!(event.phase, Phase::Target | Phase::Bubble) {
                return;
            }
            toggle_checked(ui, owner);
        });

        // Key handler: Space or Enter on Down while focused.
        ui.on_key(owner, move |ui, event| {
            use ui_events::keyboard::{Key, KeyState, NamedKey};
            if !matches!(event.phase, Phase::Target) {
                return;
            }
            if event.key.state != KeyState::Down {
                return;
            }
            let activate = match &event.key.key {
                Key::Named(NamedKey::Enter) => true,
                Key::Character(s) if s == " " => true,
                _ => false,
            };
            if activate {
                toggle_checked(ui, owner);
            }
        });
    }
}

impl ElementBehavior for ToggleBehavior {
    fn present(
        &self,
        element: &Element,
        ctx: &PresentContext<'_>,
        store: &mut PresentationStore<NodeId, ElementId>,
        node: NodeId,
    ) {
        present_surface(element, ctx, store, node);
    }

    fn default_template(&self) -> Option<&'static dyn Template> {
        Some(&TOGGLE_TEMPLATE)
    }
}

impl ElementBehavior for ToggleThumbBehavior {
    fn present(
        &self,
        element: &Element,
        ctx: &PresentContext<'_>,
        store: &mut PresentationStore<NodeId, ElementId>,
        node: NodeId,
    ) {
        present_surface(element, ctx, store, node);
    }
}

/// Flips `IsChecked` on `toggle` and keeps the `:checked`
/// pseudoclass in sync. Available as `pub` so app code or other
/// behaviors can drive a toggle without going through user input.
pub fn toggle_checked(ui: &mut Ui, toggle: ElementId) {
    let is_checked = ui.properties().is_checked;
    let was_checked = ui
        .get(toggle)
        .and_then(|el| el.get_local_value(is_checked).copied())
        .unwrap_or(false);
    let next = !was_checked;
    ui.set_local(toggle, is_checked, next);
    if next {
        ui.add_pseudo(toggle, PSEUDO_CHECKED);
    } else {
        ui.remove_pseudo(toggle, PSEUDO_CHECKED);
    }
}
