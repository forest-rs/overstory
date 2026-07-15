// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Expander`: a disclosure header with a templated content slot.
//!
//! Roadmap:
//! - Keep `Expander` as the first small headered-content control: the widget
//!   owns disclosure state, focus, accessibility, and keyboard/pointer
//!   activation; the template owns header, disclosure, label, and content
//!   slots.
//! - Use [`crate::Ui::content`] / [`crate::ChildUi::content`] as the typed
//!   authoring surface for the primary content slot. Keep
//!   [`crate::Ui::children_in_slot`] / [`crate::ChildUi::children_in_slot`] as
//!   the named-slot escape hatch for controls with multiple app-facing slots.
//! - Grow toward animation, retained content transitions, and richer header
//!   templates once content controls have a more complete authoring story.

use alloc::{boxed::Box, string::String, vec::Vec};

use understory_style::{PartTag, TypeTag};

use crate::behavior::{ArrangeContext, ArrangeResult, WidgetEventContext};
use crate::controls::disclosure;
use crate::controls::support::{CallbackSlot, EventHandler};
use crate::element::{Element, ElementId};
use crate::event_runtime::{ClickEvent, KeyEvent};
use crate::template::{
    TemplateContract, TemplateNode, TemplateSlot, TemplateSpec, TemplateSpecRecipe,
};
use crate::ui::{ControlState, Ui};
use crate::widget::{
    AppendSpec, ContentSlotSpec, ControlTemplateSpec, ElementConfig, ElementSpec, Widget,
};

/// Selector identity for `Expander`.
pub const TYPE_EXPANDER: TypeTag = TypeTag(25);
/// Owner-local part identity for the clickable header row.
pub const PART_EXPANDER_HEADER: PartTag = PartTag(1);
/// Owner-local part identity for the disclosure glyph.
pub const PART_EXPANDER_DISCLOSURE: PartTag = PartTag(2);
/// Owner-local part identity for the visible header label.
pub const PART_EXPANDER_LABEL: PartTag = PartTag(3);
/// Owner-local part identity for the content host.
pub const PART_EXPANDER_CONTENT: PartTag = PartTag(4);
/// Semantic slot for the clickable header row.
pub const SLOT_EXPANDER_HEADER: TemplateSlot =
    TemplateSlot::new("expander-header").with_part_tag(PART_EXPANDER_HEADER);
/// Semantic slot for the disclosure glyph.
pub const SLOT_EXPANDER_DISCLOSURE: TemplateSlot =
    TemplateSlot::new("expander-disclosure").with_part_tag(PART_EXPANDER_DISCLOSURE);
/// Semantic slot for the visible header label.
pub const SLOT_EXPANDER_LABEL: TemplateSlot =
    TemplateSlot::new("expander-label").with_part_tag(PART_EXPANDER_LABEL);
/// Semantic slot for the content host.
pub const SLOT_EXPANDER_CONTENT: TemplateSlot =
    TemplateSlot::new("expander-content").with_part_tag(PART_EXPANDER_CONTENT);

type ExpandedChangedHandler = EventHandler<ExpandedChangedEvent>;
type ExpandedChangedSlot = CallbackSlot<ExpandedChangedEvent>;

/// Event delivered after an [`Expander`] changes expansion state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpandedChangedEvent {
    /// The expander whose state changed.
    pub expander: ElementId,
    /// Whether the expander is open after the change.
    pub expanded: bool,
}

/// Builder + spec for a headered disclosure container.
pub struct Expander {
    header: String,
    expanded: bool,
    font_size: Option<f64>,
    element: ElementConfig,
    on_expanded_changed: Option<ExpandedChangedHandler>,
}

impl core::fmt::Debug for Expander {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Expander")
            .field("header", &self.header)
            .field("expanded", &self.expanded)
            .finish_non_exhaustive()
    }
}

impl Expander {
    /// Starts a new expander with the given header text.
    #[must_use]
    pub fn new(header: impl Into<String>) -> Self {
        let mut element = ElementConfig::default();
        element.set_control_state(ControlState::interactive());
        Self {
            header: header.into(),
            expanded: false,
            font_size: None,
            element,
            on_expanded_changed: None,
        }
    }

    /// Sets the initial expansion state.
    #[must_use]
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Overrides the header label font size.
    #[must_use]
    pub fn font_size(mut self, font_size: f64) -> Self {
        self.font_size = Some(font_size);
        self
    }

    /// Registers a handler fired after the expansion state changes.
    #[must_use]
    pub fn on_expanded_changed<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, ExpandedChangedEvent) + 'static,
    {
        self.on_expanded_changed = Some(Box::new(handler));
        self
    }
}

impl AppendSpec for Expander {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let widget = ExpanderWidget {
            expanded: self.expanded,
            expanded_changed: ExpandedChangedSlot::new(self.on_expanded_changed),
        };
        let id = ui.append_widget_deferred_template(parent, Box::new(widget));
        let content_prop = ui.properties().content;
        let font_size_prop = ui.properties().font_size;
        ui.set_local(id, content_prop, self.header);
        if let Some(font_size) = self.font_size {
            ui.set_local(id, font_size_prop, font_size);
        }
        self.element.apply_to(ui, id);
        ui.expand_resolved_template(id);
        sync_expander_state(ui, id, self.expanded);
        id
    }
}

impl ControlTemplateSpec for Expander {}

impl ContentSlotSpec for Expander {
    const CONTENT_SLOT: TemplateSlot = SLOT_EXPANDER_CONTENT;
}

impl ElementSpec for Expander {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

pub(crate) struct ExpanderWidget {
    expanded: bool,
    expanded_changed: ExpandedChangedSlot,
}

impl core::fmt::Debug for ExpanderWidget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ExpanderWidget")
            .field("expanded", &self.expanded)
            .finish_non_exhaustive()
    }
}

impl Widget for ExpanderWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_EXPANDER)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::DisclosureTriangle)
    }

    fn configure_accessibility(
        &self,
        element: &Element,
        ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        disclosure::configure_accessibility(element, ctx, node, self.expanded);
    }

    fn append_accessibility_children(
        &self,
        element: &Element,
        ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
        nodes: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        disclosure::append_expanded_slot_accessibility(
            element,
            ctx,
            node,
            nodes,
            SLOT_EXPANDER_CONTENT,
            self.expanded,
        );
    }

    fn template_contract(&self) -> TemplateContract {
        TemplateContract::new()
            .with_required_slot(SLOT_EXPANDER_HEADER)
            .with_required_slot(SLOT_EXPANDER_CONTENT)
            .with_recommended_slot(SLOT_EXPANDER_DISCLOSURE)
            .with_recommended_slot(SLOT_EXPANDER_LABEL)
    }

    fn handle_click(&mut self, ctx: &mut WidgetEventContext<'_>, event: ClickEvent<'_>) {
        let expander = ctx.id();
        if !event.is_semantic_target() || !ctx.is_enabled(expander) {
            return;
        }
        if !click_source_is_header(ctx, expander, event.source()) {
            return;
        }
        let _ = self.set_expanded(ctx, expander, !self.expanded);
        event.mark_handled();
    }

    fn handle_key(&mut self, ctx: &mut WidgetEventContext<'_>, event: KeyEvent<'_>) {
        let expander = ctx.id();
        if !event.is_target() {
            return;
        }
        if !ctx.is_enabled(expander) {
            return;
        }
        let next = disclosure::key_target(&event, self.expanded);
        if let Some(expanded) = next
            && self.set_expanded(ctx, expander, expanded)
        {
            event.mark_handled();
        }
    }

    fn handle_accessibility_action(
        &mut self,
        ctx: &mut WidgetEventContext<'_>,
        request: &accesskit::ActionRequest,
    ) -> bool {
        let expander = ctx.id();
        if !ctx.is_enabled(expander) {
            return false;
        }
        disclosure::action_target(request.action, self.expanded)
            .is_some_and(|expanded| self.set_expanded(ctx, expander, expanded))
    }

    fn arrange(&mut self, ctx: &mut ArrangeContext<'_>) -> ArrangeResult {
        let expander = ctx.id();
        let expanded = disclosure::resolved_expanded(ctx.ui(), expander);
        ArrangeResult::from_layout_changed(self.set_expanded(ctx.ui_mut(), expander, expanded))
    }
}

impl ExpanderWidget {
    fn set_expanded(&mut self, ui: &mut Ui, expander: ElementId, expanded: bool) -> bool {
        if self.expanded == expanded {
            return false;
        }
        self.expanded = expanded;
        sync_expander_state(ui, expander, expanded);
        self.expanded_changed
            .dispatch(ui, expander, ExpandedChangedEvent { expander, expanded });
        true
    }
}

static EXPANDER_TEMPLATE: TemplateSpecRecipe = TemplateSpecRecipe::new(expander_template_spec);

pub(crate) fn default_expander_template() -> crate::ControlTemplate {
    crate::ControlTemplate::from_static(&EXPANDER_TEMPLATE)
}

fn expander_template_spec() -> TemplateSpec {
    TemplateSpec::from_nodes([
        TemplateNode::surface_slot(SLOT_EXPANDER_HEADER)
            .with_child(TemplateNode::text_slot(SLOT_EXPANDER_DISCLOSURE))
            .with_child(TemplateNode::content_presenter(SLOT_EXPANDER_LABEL)),
        TemplateNode::surface_slot(SLOT_EXPANDER_CONTENT),
    ])
}

/// Sets whether `expander` is expanded.
///
/// Returns `false` if `expander` is stale, not an [`Expander`], or already has
/// the requested state.
pub fn set_expander_expanded(ui: &mut Ui, expander: ElementId, expanded: bool) -> bool {
    with_expander_mut(ui, expander, |widget, ui| {
        widget.set_expanded(ui, expander, expanded)
    })
    .unwrap_or(false)
}

/// Toggles whether `expander` is expanded.
///
/// Returns `false` if `expander` is stale or not an [`Expander`].
pub fn toggle_expander(ui: &mut Ui, expander: ElementId) -> bool {
    with_expander_mut(ui, expander, |widget, ui| {
        widget.set_expanded(ui, expander, !widget.expanded)
    })
    .unwrap_or(false)
}

/// Returns whether `expander` is currently expanded.
#[must_use]
pub fn expander_expanded(ui: &Ui, expander: ElementId) -> Option<bool> {
    expander_widget(ui, expander).map(|widget| widget.expanded)
}

/// Returns the content host generated by `expander`'s current template.
#[must_use]
pub fn expander_content(ui: &Ui, expander: ElementId) -> Option<ElementId> {
    ui.slot(expander, SLOT_EXPANDER_CONTENT)
}

fn with_expander_mut<R>(
    ui: &mut Ui,
    expander: ElementId,
    f: impl FnOnce(&mut ExpanderWidget, &mut Ui) -> R,
) -> Option<R> {
    let mut output = None;
    ui.with_widget_mut(expander, |widget, ui| {
        if let Some(expander) = widget.downcast_mut::<ExpanderWidget>() {
            output = Some(f(expander, ui));
        }
    });
    output
}

fn expander_widget(ui: &Ui, expander: ElementId) -> Option<&ExpanderWidget> {
    ui.get(expander)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<ExpanderWidget>())
}

fn sync_expander_state(ui: &mut Ui, expander: ElementId, expanded: bool) {
    disclosure::sync_state(
        ui,
        expander,
        SLOT_EXPANDER_DISCLOSURE,
        SLOT_EXPANDER_CONTENT,
        expanded,
    );
}

fn click_source_is_header(
    ctx: &WidgetEventContext<'_>,
    expander: ElementId,
    source: ElementId,
) -> bool {
    disclosure::source_is_header(ctx, expander, source, SLOT_EXPANDER_HEADER)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accessibility::accesskit_node_id;
    use crate::pseudos::PSEUDO_EXPANDED;
    use accesskit::{Action, ActionRequest, Role, TreeId};
    use alloc::rc::Rc;
    use core::cell::Cell;
    use kurbo::{Point, Size};
    use ui_events::keyboard::{Code, Key, KeyboardEvent};

    #[test]
    fn expander_hides_and_shows_content_slot() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, crate::Layout::column(0.0).logical_size(240.0, 140.0));
        let (expander, _) = ui.content(root, Expander::new("Advanced"), |content| {
            content.add(crate::Text::new("Details"));
        });
        let content = expander_content(&ui, expander).unwrap();

        ui.compute_layout(Size::new(240.0, 140.0));
        assert_eq!(ui.effective_bounds(content).height(), 0.0);

        assert!(set_expander_expanded(&mut ui, expander, true));
        ui.compute_layout(Size::new(240.0, 140.0));
        assert!(ui.effective_bounds(content).height() > 0.0);
    }

    #[test]
    fn direct_is_expanded_write_updates_expander_state_and_content_slot() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, crate::Layout::column(0.0).logical_size(240.0, 140.0));
        let (expander, _) = ui.content(root, Expander::new("Advanced"), |content| {
            content.add(crate::Text::new("Details"));
        });
        let content = expander_content(&ui, expander).unwrap();

        ui.compute_layout(Size::new(240.0, 140.0));
        ui.set_local(expander, ui.properties().is_expanded, true);
        ui.compute_layout(Size::new(240.0, 140.0));

        assert_eq!(expander_expanded(&ui, expander), Some(true));
        assert!(
            ui.get(expander)
                .unwrap()
                .pseudos()
                .contains(&PSEUDO_EXPANDED)
        );
        assert!(ui.effective_bounds(content).height() > 0.0);
    }

    #[test]
    fn expander_clicking_header_toggles_and_reports_state() {
        let observed = Rc::new(Cell::new(None));
        let observed_for_handler = Rc::clone(&observed);
        let mut ui = Ui::new();
        let root = ui.root();
        let expander = ui.append(
            root,
            Expander::new("Advanced").on_expanded_changed(move |_ui, event| {
                observed_for_handler.set(Some((event.expander, event.expanded)));
            }),
        );
        let header = ui.slot(expander, SLOT_EXPANDER_HEADER).unwrap();

        ui.synthesize_click(header, Point::ZERO);

        assert_eq!(expander_expanded(&ui, expander), Some(true));
        assert_eq!(observed.get(), Some((expander, true)));
    }

    #[test]
    fn expander_keyboard_toggles_on_space() {
        let mut ui = Ui::new();
        let root = ui.root();
        let expander = ui.append(root, Expander::new("Advanced"));
        ui.set_focus(Some(expander));

        let key = KeyboardEvent::key_down(Key::Character(String::from(" ")), Code::Space);
        let result = ui.dispatch_key(&key);

        assert!(result.handled);
        assert_eq!(expander_expanded(&ui, expander), Some(true));
    }

    #[test]
    fn expander_accessibility_exposes_disclosure_state_and_actions() {
        let mut ui = Ui::new();
        let root = ui.root();
        let expander = ui.append(root, Expander::new("Advanced"));
        ui.compute_layout(Size::new(240.0, 120.0));
        let update = ui.accesskit_tree_update();
        let node = node_for(&update, expander);

        assert_eq!(node.role(), Role::DisclosureTriangle);
        assert_eq!(node.is_expanded(), Some(false));
        assert!(node.supports_action(Action::Expand));

        assert!(ui.handle_accesskit_action(&action(expander, Action::Expand)));
        let update = ui.accesskit_tree_update();
        let node = node_for(&update, expander);
        assert_eq!(node.is_expanded(), Some(true));
        assert!(node.supports_action(Action::Collapse));
    }

    #[test]
    fn expander_accessibility_exposes_expanded_content_slot() {
        let mut ui = Ui::new();
        let root = ui.root();
        let (expander, details) =
            ui.content(root, Expander::new("Advanced").expanded(true), |content| {
                content.add(crate::Text::new("Details"))
            });
        let details = details.expect("expander template should expose content slot");
        ui.compute_layout(Size::new(240.0, 120.0));

        let update = ui.accesskit_tree_update();
        let node = node_for(&update, expander);

        assert!(node.children().contains(&accesskit_node_id(details)));
        assert_eq!(node_for(&update, details).role(), Role::Label);

        assert!(set_expander_expanded(&mut ui, expander, false));
        let update = ui.accesskit_tree_update();
        let node = node_for(&update, expander);

        assert!(!node.children().contains(&accesskit_node_id(details)));
        assert!(!has_node(&update, details));
    }

    fn action(id: ElementId, action: Action) -> ActionRequest {
        ActionRequest {
            target_tree: TreeId::ROOT,
            target_node: accesskit_node_id(id),
            action,
            data: None,
        }
    }

    fn node_for(update: &accesskit::TreeUpdate, id: ElementId) -> &accesskit::Node {
        let node_id = accesskit_node_id(id);
        update
            .nodes
            .iter()
            .find(|(id, _)| *id == node_id)
            .map(|(_, node)| node)
            .expect("node should be emitted")
    }

    fn has_node(update: &accesskit::TreeUpdate, id: ElementId) -> bool {
        let node_id = accesskit_node_id(id);
        update.nodes.iter().any(|(id, _)| *id == node_id)
    }
}
