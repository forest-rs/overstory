// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared mechanics for disclosure-style controls.

use accesskit::{Action, Node, NodeId};
use alloc::{string::String, vec::Vec};
use ui_events::keyboard::{Key, KeyState, NamedKey};

use crate::AccessibilityContext;
use crate::behavior::WidgetEventContext;
use crate::controls::support::is_descendant_or_self;
use crate::element::{Element, ElementId};
use crate::event_runtime::KeyEvent;
use crate::layouts::{LayoutContainer, LayoutDisplay};
use crate::pseudos::PSEUDO_EXPANDED;
use crate::template::TemplateSlot;
use crate::ui::Ui;

pub(crate) fn resolved_expanded(ui: &Ui, owner: ElementId) -> bool {
    ui.get(owner)
        .map(|element| ui.resolve_value_for(element, ui.properties().is_expanded))
        .unwrap_or(false)
}

pub(crate) fn configure_accessibility(
    element: &Element,
    ctx: &AccessibilityContext<'_>,
    node: &mut Node,
    expanded: bool,
) {
    ctx.add_click_action_if_enabled(element, node);
    node.set_expanded(expanded);
    if expanded {
        node.add_action(Action::Collapse);
    } else {
        node.add_action(Action::Expand);
    }
}

pub(crate) fn append_expanded_slot_accessibility(
    element: &Element,
    ctx: &AccessibilityContext<'_>,
    node: &mut Node,
    nodes: &mut Vec<(NodeId, Node)>,
    slot: TemplateSlot,
    expanded: bool,
) {
    if expanded {
        ctx.append_slot_children(element.id(), slot, node, nodes);
    }
}

pub(crate) fn key_target(event: &KeyEvent<'_>, expanded: bool) -> Option<bool> {
    if event.key.state != KeyState::Down {
        return None;
    }
    match &event.key.key {
        Key::Named(NamedKey::Enter) => Some(!expanded),
        Key::Character(s) if s == " " => Some(!expanded),
        Key::Named(NamedKey::ArrowRight) => Some(true),
        Key::Named(NamedKey::ArrowLeft) => Some(false),
        _ => None,
    }
}

pub(crate) fn action_target(action: Action, expanded: bool) -> Option<bool> {
    match action {
        Action::Click => Some(!expanded),
        Action::Expand => Some(true),
        Action::Collapse => Some(false),
        _ => None,
    }
}

pub(crate) fn sync_state(
    ui: &mut Ui,
    owner: ElementId,
    disclosure_slot: TemplateSlot,
    content_slot: TemplateSlot,
    expanded: bool,
) {
    ui.set_local(owner, ui.properties().is_expanded, expanded);
    if expanded {
        ui.add_pseudo(owner, PSEUDO_EXPANDED);
    } else {
        ui.remove_pseudo(owner, PSEUDO_EXPANDED);
    }
    if let Some(disclosure) = ui.slot(owner, disclosure_slot) {
        ui.set_local(
            disclosure,
            ui.properties().text,
            String::from(glyph(expanded)),
        );
    }
    if let Some(content) = ui.slot(owner, content_slot) {
        let display = if expanded {
            LayoutContainer::column()
        } else {
            LayoutContainer::new().display(LayoutDisplay::None)
        };
        ui.set_layout_container(content, display);
    }
}

pub(crate) fn source_is_header(
    ctx: &WidgetEventContext<'_>,
    owner: ElementId,
    source: ElementId,
    header_slot: TemplateSlot,
) -> bool {
    let Some(header) = ctx.slot(owner, header_slot) else {
        return source == owner;
    };
    is_descendant_or_self(ctx, source, header)
}

fn glyph(expanded: bool) -> &'static str {
    if expanded { "▾" } else { "▸" }
}
