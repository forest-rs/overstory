// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Accordion`: coordinated expandable sections.
//!
//! Roadmap:
//! - Keep `Accordion` as a coordinator over ordinary section elements: it owns
//!   expansion policy, while each section owns its header, content slot, focus,
//!   input, and accessibility.
//! - Keep section authoring on [`crate::ChildUi`] through
//!   [`AccordionChildUiExt`] so downstream crates can add their own typed
//!   helpers without modifying Overstory.
//! - Grow toward item-backed accordions once item controls have typed keys,
//!   item templates, and section-level commands.

use alloc::{boxed::Box, string::String, vec::Vec};

use understory_style::{PartTag, TypeTag};

use crate::behavior::WidgetEventContext;
use crate::controls::disclosure;
use crate::controls::support::{CallbackSlot, EventHandler};
use crate::controls::text::TextWidget;
use crate::element::{Element, ElementId};
use crate::event_runtime::{ClickEvent, KeyEvent};
use crate::layouts::Layout;
use crate::template::{
    TemplateContract, TemplateNode, TemplateSlot, TemplateSpec, TemplateWidgetFactory,
};
use crate::ui::{ChildUi, ControlState, Ui};
use crate::widget::{
    AppendSpec, ContentSlotSpec, ControlTemplateSpec, ElementConfig, ElementSpec, Widget,
    widget_with_template_override,
};

/// Selector identity for `Accordion`.
pub const TYPE_ACCORDION: TypeTag = TypeTag(28);
/// Selector identity for a coordinated accordion section.
pub const TYPE_ACCORDION_SECTION: TypeTag = TypeTag(29);
/// Owner-local part identity for an accordion section's clickable header row.
pub const PART_ACCORDION_SECTION_HEADER: PartTag = PartTag(1);
/// Owner-local part identity for an accordion section's disclosure glyph.
pub const PART_ACCORDION_SECTION_DISCLOSURE: PartTag = PartTag(2);
/// Owner-local part identity for an accordion section's visible header label.
pub const PART_ACCORDION_SECTION_LABEL: PartTag = PartTag(3);
/// Owner-local part identity for an accordion section's content host.
pub const PART_ACCORDION_SECTION_CONTENT: PartTag = PartTag(4);
/// Semantic slot for an accordion section's clickable header row.
pub const SLOT_ACCORDION_SECTION_HEADER: TemplateSlot =
    TemplateSlot::new("accordion-section-header").with_part_tag(PART_ACCORDION_SECTION_HEADER);
/// Semantic slot for an accordion section's disclosure glyph.
pub const SLOT_ACCORDION_SECTION_DISCLOSURE: TemplateSlot =
    TemplateSlot::new("accordion-section-disclosure")
        .with_part_tag(PART_ACCORDION_SECTION_DISCLOSURE);
/// Semantic slot for an accordion section's visible header label.
pub const SLOT_ACCORDION_SECTION_LABEL: TemplateSlot =
    TemplateSlot::new("accordion-section-label").with_part_tag(PART_ACCORDION_SECTION_LABEL);
/// Semantic slot for an accordion section's content host.
pub const SLOT_ACCORDION_SECTION_CONTENT: TemplateSlot =
    TemplateSlot::new("accordion-section-content").with_part_tag(PART_ACCORDION_SECTION_CONTENT);

type AccordionExpandedChangedHandler = EventHandler<AccordionExpandedChangedEvent>;
type AccordionExpandedChangedSlot = CallbackSlot<AccordionExpandedChangedEvent>;

/// Expansion policy for sections inside an [`Accordion`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AccordionMode {
    /// At most one section may be open; every section may also be closed.
    #[default]
    Single,
    /// Exactly one section remains open after at least one section exists.
    SingleRequired,
    /// Sections expand and collapse independently.
    Multiple,
}

/// Event delivered after an [`AccordionSection`] changes expansion state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccordionExpandedChangedEvent {
    /// The parent accordion when the section is coordinated by one.
    pub accordion: Option<ElementId>,
    /// The section whose expansion state changed.
    pub section: ElementId,
    /// Whether the section is open after the change.
    pub expanded: bool,
}

/// Builder + spec for a coordinated collection of expandable sections.
pub struct Accordion {
    mode: AccordionMode,
    gap: f32,
    element: ElementConfig,
}

impl core::fmt::Debug for Accordion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Accordion")
            .field("mode", &self.mode)
            .field("gap", &self.gap)
            .finish_non_exhaustive()
    }
}

impl Accordion {
    /// Starts a new accordion with single-section expansion policy.
    #[must_use]
    pub fn new() -> Self {
        Self {
            mode: AccordionMode::Single,
            gap: 8.0,
            element: ElementConfig::default(),
        }
    }

    /// Sets the accordion's expansion policy.
    #[must_use]
    pub fn mode(mut self, mode: AccordionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Uses [`AccordionMode::Single`].
    #[must_use]
    pub fn single(self) -> Self {
        self.mode(AccordionMode::Single)
    }

    /// Uses [`AccordionMode::SingleRequired`].
    #[must_use]
    pub fn single_required(self) -> Self {
        self.mode(AccordionMode::SingleRequired)
    }

    /// Uses [`AccordionMode::Multiple`].
    #[must_use]
    pub fn multiple(self) -> Self {
        self.mode(AccordionMode::Multiple)
    }

    /// Sets the vertical gap between section roots.
    #[must_use]
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }
}

impl Default for Accordion {
    fn default() -> Self {
        Self::new()
    }
}

impl AppendSpec for Accordion {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget(parent, Box::new(AccordionWidget { mode: self.mode }));
        ui.set_layout(id, Layout::column(self.gap));
        self.element.apply_to(ui, id);
        id
    }
}

impl ElementSpec for Accordion {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

/// Builder + spec for one section inside an [`Accordion`].
pub struct AccordionSection {
    header: String,
    expanded: bool,
    font_size: Option<f64>,
    element: ElementConfig,
    template: Option<crate::ControlTemplate>,
    on_expanded_changed: Option<AccordionExpandedChangedHandler>,
}

impl core::fmt::Debug for AccordionSection {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AccordionSection")
            .field("header", &self.header)
            .field("expanded", &self.expanded)
            .finish_non_exhaustive()
    }
}

impl AccordionSection {
    /// Starts a new accordion section with the given header text.
    #[must_use]
    pub fn new(header: impl Into<String>) -> Self {
        let mut element = ElementConfig::default();
        element.set_control_state(ControlState::interactive());
        Self {
            header: header.into(),
            expanded: false,
            font_size: None,
            element,
            template: None,
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

    /// Registers a handler fired after this section's expansion state changes.
    #[must_use]
    pub fn on_expanded_changed<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, AccordionExpandedChangedEvent) + 'static,
    {
        self.on_expanded_changed = Some(Box::new(handler));
        self
    }
}

impl AppendSpec for AccordionSection {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let widget = AccordionSectionWidget {
            expanded: self.expanded,
            expanded_changed: AccordionExpandedChangedSlot::new(self.on_expanded_changed),
        };
        let widget = widget_with_template_override(widget, self.template);
        let id = ui.append_widget_deferred_template(parent, widget);
        let content_prop = ui.properties().content;
        let font_size_prop = ui.properties().font_size;
        ui.set_local(id, content_prop, self.header);
        if let Some(font_size) = self.font_size {
            ui.set_local(id, font_size_prop, font_size);
        }
        self.element.apply_to(ui, id);
        ui.expand_default_template(id);
        sync_accordion_section_state(ui, id, self.expanded);
        enforce_accordion_after_insert(ui, parent, id);
        id
    }
}

impl ControlTemplateSpec for AccordionSection {
    fn template_override_mut(&mut self) -> &mut Option<crate::ControlTemplate> {
        &mut self.template
    }
}

impl ContentSlotSpec for AccordionSection {
    const CONTENT_SLOT: TemplateSlot = SLOT_ACCORDION_SECTION_CONTENT;
}

impl ElementSpec for AccordionSection {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

/// Extension methods for authoring accordion sections inside a [`ChildUi`].
pub trait AccordionChildUiExt {
    /// Adds an [`AccordionSection`] with `header` and scopes `f` to its content.
    fn section<R>(
        &mut self,
        header: impl Into<String>,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> (ElementId, R);

    /// Adds the configured [`AccordionSection`] and scopes `f` to its content.
    fn section_with<R>(
        &mut self,
        section: AccordionSection,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> (ElementId, R);
}

impl AccordionChildUiExt for ChildUi<'_> {
    fn section<R>(
        &mut self,
        header: impl Into<String>,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> (ElementId, R) {
        self.section_with(AccordionSection::new(header), f)
    }

    fn section_with<R>(
        &mut self,
        section: AccordionSection,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> (ElementId, R) {
        let (id, result) = self.content(section, f);
        let result = result.expect("AccordionSection template must expose its content slot");
        (id, result)
    }
}

#[derive(Debug)]
pub(crate) struct AccordionWidget {
    mode: AccordionMode,
}

impl Widget for AccordionWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_ACCORDION)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::Group)
    }
}

pub(crate) struct AccordionSectionWidget {
    expanded: bool,
    expanded_changed: AccordionExpandedChangedSlot,
}

impl core::fmt::Debug for AccordionSectionWidget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AccordionSectionWidget")
            .field("expanded", &self.expanded)
            .finish_non_exhaustive()
    }
}

impl Widget for AccordionSectionWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_ACCORDION_SECTION)
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
            SLOT_ACCORDION_SECTION_CONTENT,
            self.expanded,
        );
    }

    fn default_template(&self) -> Option<crate::ControlTemplate> {
        Some(crate::ControlTemplate::from_static(
            &ACCORDION_SECTION_TEMPLATE,
        ))
    }

    fn template_contract(&self) -> TemplateContract {
        TemplateContract::new()
            .with_required_slot(SLOT_ACCORDION_SECTION_HEADER)
            .with_required_slot(SLOT_ACCORDION_SECTION_CONTENT)
            .with_recommended_slot(SLOT_ACCORDION_SECTION_DISCLOSURE)
            .with_recommended_slot(SLOT_ACCORDION_SECTION_LABEL)
    }

    fn handle_click(&mut self, ctx: &mut WidgetEventContext<'_>, event: ClickEvent<'_>) {
        let section = ctx.id();
        if !event.is_semantic_target() || !ctx.is_enabled(section) {
            return;
        }
        if !click_source_is_section_header(ctx, section, event.source) {
            return;
        }
        let _ = self.set_expanded(ctx, section, !self.expanded);
        event.mark_handled();
    }

    fn handle_key(&mut self, ctx: &mut WidgetEventContext<'_>, event: KeyEvent<'_>) {
        let section = ctx.id();
        if !event.is_target() {
            return;
        }
        if !ctx.is_enabled(section) {
            return;
        }
        let next = disclosure::key_target(&event, self.expanded);
        if let Some(expanded) = next
            && self.set_expanded(ctx, section, expanded)
        {
            event.mark_handled();
        }
    }

    fn handle_accessibility_action(
        &mut self,
        ctx: &mut WidgetEventContext<'_>,
        request: &accesskit::ActionRequest,
    ) -> bool {
        let section = ctx.id();
        if !ctx.is_enabled(section) {
            return false;
        }
        disclosure::action_target(request.action, self.expanded)
            .is_some_and(|expanded| self.set_expanded(ctx, section, expanded))
    }
}

impl AccordionSectionWidget {
    fn set_expanded(&mut self, ui: &mut Ui, section: ElementId, expanded: bool) -> bool {
        let Some(accordion) = accordion_parent(ui, section) else {
            return self.set_expanded_direct(ui, None, section, expanded, true);
        };
        apply_accordion_policy(ui, accordion, section, self, expanded)
    }

    fn set_expanded_direct(
        &mut self,
        ui: &mut Ui,
        accordion: Option<ElementId>,
        section: ElementId,
        expanded: bool,
        notify: bool,
    ) -> bool {
        if self.expanded == expanded {
            return false;
        }
        self.expanded = expanded;
        sync_accordion_section_state(ui, section, expanded);
        if notify {
            self.expanded_changed.dispatch(
                ui,
                AccordionExpandedChangedEvent {
                    accordion,
                    section,
                    expanded,
                },
            );
        }
        true
    }
}

type AccordionSectionTemplate = TemplateSpec;

static ACCORDION_SECTION_TEMPLATE: AccordionSectionTemplate =
    TemplateSpec::new(ACCORDION_SECTION_TEMPLATE_NODES);

const ACCORDION_SECTION_TEMPLATE_NODES: &[TemplateNode] = &[
    TemplateNode::surface_slot(SLOT_ACCORDION_SECTION_HEADER)
        .with_children(ACCORDION_SECTION_HEADER_CHILDREN),
    TemplateNode::surface_slot(SLOT_ACCORDION_SECTION_CONTENT),
];

const ACCORDION_SECTION_HEADER_CHILDREN: &[TemplateNode] = &[
    TemplateNode::for_slot(
        SLOT_ACCORDION_SECTION_DISCLOSURE,
        TemplateWidgetFactory::new(accordion_section_disclosure_widget),
    ),
    TemplateNode::content_presenter(SLOT_ACCORDION_SECTION_LABEL),
];

fn accordion_section_disclosure_widget() -> Box<dyn Widget> {
    Box::new(TextWidget)
}

/// Sets whether an accordion section is expanded.
///
/// If the section belongs to an [`Accordion`], the parent accordion's mode
/// decides whether sibling sections are collapsed or whether the requested
/// collapse is allowed.
pub fn set_accordion_section_expanded(ui: &mut Ui, section: ElementId, expanded: bool) -> bool {
    with_accordion_section_mut(ui, section, |widget, ui| {
        widget.set_expanded(ui, section, expanded)
    })
    .unwrap_or(false)
}

/// Toggles whether an accordion section is expanded.
pub fn toggle_accordion_section(ui: &mut Ui, section: ElementId) -> bool {
    with_accordion_section_mut(ui, section, |widget, ui| {
        widget.set_expanded(ui, section, !widget.expanded)
    })
    .unwrap_or(false)
}

/// Returns whether an accordion section is expanded.
#[must_use]
pub fn accordion_section_expanded(ui: &Ui, section: ElementId) -> Option<bool> {
    accordion_section_widget(ui, section).map(|widget| widget.expanded)
}

/// Returns the content host generated by an accordion section's current
/// template.
#[must_use]
pub fn accordion_section_content(ui: &Ui, section: ElementId) -> Option<ElementId> {
    ui.slot(section, SLOT_ACCORDION_SECTION_CONTENT)
}

fn enforce_accordion_after_insert(ui: &mut Ui, accordion: ElementId, section: ElementId) {
    let Some(mode) = accordion_widget(ui, accordion).map(|widget| widget.mode) else {
        return;
    };
    match mode {
        AccordionMode::Multiple => {}
        AccordionMode::Single => {
            if accordion_section_expanded(ui, section) == Some(true) {
                collapse_other_sections(ui, accordion, section, false);
            }
        }
        AccordionMode::SingleRequired => {
            if accordion_section_expanded(ui, section) == Some(true) {
                collapse_other_sections(ui, accordion, section, false);
            } else if !accordion_sections(ui, accordion)
                .into_iter()
                .any(|candidate| accordion_section_expanded(ui, candidate) == Some(true))
            {
                set_accordion_section_expanded_direct(ui, accordion, section, true, false);
            }
        }
    }
}

fn apply_accordion_policy(
    ui: &mut Ui,
    accordion: ElementId,
    section: ElementId,
    current: &mut AccordionSectionWidget,
    expanded: bool,
) -> bool {
    let Some(mode) = accordion_widget(ui, accordion).map(|widget| widget.mode) else {
        return current.set_expanded_direct(ui, None, section, expanded, true);
    };
    match (mode, expanded) {
        (AccordionMode::Multiple, _) => {
            current.set_expanded_direct(ui, Some(accordion), section, expanded, true)
        }
        (AccordionMode::Single, true) => {
            let siblings_changed = collapse_other_sections(ui, accordion, section, true);
            current.set_expanded_direct(ui, Some(accordion), section, true, true)
                || siblings_changed
        }
        (AccordionMode::Single, false) => {
            current.set_expanded_direct(ui, Some(accordion), section, false, true)
        }
        (AccordionMode::SingleRequired, true) => {
            let siblings_changed = collapse_other_sections(ui, accordion, section, true);
            current.set_expanded_direct(ui, Some(accordion), section, true, true)
                || siblings_changed
        }
        (AccordionMode::SingleRequired, false) => {
            if has_expanded_sibling(ui, accordion, section) {
                current.set_expanded_direct(ui, Some(accordion), section, false, true)
            } else {
                false
            }
        }
    }
}

fn collapse_other_sections(
    ui: &mut Ui,
    accordion: ElementId,
    section: ElementId,
    notify: bool,
) -> bool {
    let mut changed = false;
    for candidate in accordion_sections(ui, accordion) {
        if candidate != section {
            changed |=
                set_accordion_section_expanded_direct(ui, accordion, candidate, false, notify);
        }
    }
    changed
}

fn has_expanded_sibling(ui: &Ui, accordion: ElementId, section: ElementId) -> bool {
    accordion_sections(ui, accordion)
        .into_iter()
        .any(|candidate| {
            candidate != section && accordion_section_expanded(ui, candidate) == Some(true)
        })
}

fn accordion_sections(ui: &Ui, accordion: ElementId) -> Vec<ElementId> {
    ui.get(accordion)
        .map(|element| {
            element
                .children()
                .iter()
                .copied()
                .filter(|child| accordion_section_widget(ui, *child).is_some())
                .collect()
        })
        .unwrap_or_default()
}

fn set_accordion_section_expanded_direct(
    ui: &mut Ui,
    accordion: ElementId,
    section: ElementId,
    expanded: bool,
    notify: bool,
) -> bool {
    with_accordion_section_mut(ui, section, |widget, ui| {
        widget.set_expanded_direct(ui, Some(accordion), section, expanded, notify)
    })
    .unwrap_or(false)
}

fn with_accordion_section_mut<R>(
    ui: &mut Ui,
    section: ElementId,
    f: impl FnOnce(&mut AccordionSectionWidget, &mut Ui) -> R,
) -> Option<R> {
    let mut output = None;
    ui.with_widget_mut(section, |widget, ui| {
        if let Some(section_widget) = widget.downcast_mut::<AccordionSectionWidget>() {
            output = Some(f(section_widget, ui));
        }
    });
    output
}

fn accordion_widget(ui: &Ui, accordion: ElementId) -> Option<&AccordionWidget> {
    ui.get(accordion)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<AccordionWidget>())
}

fn accordion_section_widget(ui: &Ui, section: ElementId) -> Option<&AccordionSectionWidget> {
    ui.get(section)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<AccordionSectionWidget>())
}

fn accordion_parent(ui: &Ui, section: ElementId) -> Option<ElementId> {
    let parent = ui.get(section)?.parent()?;
    accordion_widget(ui, parent).is_some().then_some(parent)
}

fn sync_accordion_section_state(ui: &mut Ui, section: ElementId, expanded: bool) {
    disclosure::sync_state(
        ui,
        section,
        SLOT_ACCORDION_SECTION_DISCLOSURE,
        SLOT_ACCORDION_SECTION_CONTENT,
        expanded,
    );
}

fn click_source_is_section_header(
    ctx: &WidgetEventContext<'_>,
    section: ElementId,
    source: ElementId,
) -> bool {
    disclosure::source_is_header(ctx, section, source, SLOT_ACCORDION_SECTION_HEADER)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accessibility::accesskit_node_id;
    use accesskit::{Action, ActionRequest, Role, TreeId};
    use alloc::rc::Rc;
    use core::cell::Cell;
    use kurbo::{Point, Size};

    #[test]
    fn accordion_section_helper_scopes_children_to_content_slot() {
        let mut ui = Ui::new();
        let root = ui.root();
        let (accordion, (section, label)) = ui.children(root, Accordion::new(), |accordion| {
            accordion.section("Routing", |section| section.add(crate::Text::new("Nested")))
        });
        let content = accordion_section_content(&ui, section).unwrap();

        assert_eq!(ui.get(root).unwrap().children(), &[accordion]);
        assert_eq!(ui.get(accordion).unwrap().children(), &[section]);
        assert_eq!(ui.get(content).unwrap().children(), &[label]);
        assert_eq!(ui.get(label).unwrap().parent(), Some(content));
    }

    #[test]
    fn single_mode_expands_one_section_at_a_time() {
        let mut ui = Ui::new();
        let root = ui.root();
        let (_accordion, (first, second)) =
            ui.children(root, Accordion::new().single(), |accordion| {
                let (first, _) =
                    accordion.section_with(AccordionSection::new("One").expanded(true), |_| {});
                let (second, _) = accordion.section("Two", |_| {});
                (first, second)
            });

        assert_eq!(accordion_section_expanded(&ui, first), Some(true));
        assert_eq!(accordion_section_expanded(&ui, second), Some(false));

        assert!(set_accordion_section_expanded(&mut ui, second, true));
        assert_eq!(accordion_section_expanded(&ui, first), Some(false));
        assert_eq!(accordion_section_expanded(&ui, second), Some(true));

        assert!(set_accordion_section_expanded(&mut ui, second, false));
        assert_eq!(accordion_section_expanded(&ui, first), Some(false));
        assert_eq!(accordion_section_expanded(&ui, second), Some(false));
    }

    #[test]
    fn single_required_keeps_one_section_open() {
        let mut ui = Ui::new();
        let root = ui.root();
        let (_accordion, (first, second)) =
            ui.children(root, Accordion::new().single_required(), |accordion| {
                let (first, _) = accordion.section("One", |_| {});
                let (second, _) = accordion.section("Two", |_| {});
                (first, second)
            });

        assert_eq!(accordion_section_expanded(&ui, first), Some(true));
        assert_eq!(accordion_section_expanded(&ui, second), Some(false));

        assert!(!set_accordion_section_expanded(&mut ui, first, false));
        assert_eq!(accordion_section_expanded(&ui, first), Some(true));

        assert!(set_accordion_section_expanded(&mut ui, second, true));
        assert_eq!(accordion_section_expanded(&ui, first), Some(false));
        assert_eq!(accordion_section_expanded(&ui, second), Some(true));
    }

    #[test]
    fn multiple_mode_allows_independent_sections() {
        let mut ui = Ui::new();
        let root = ui.root();
        let (_accordion, (first, second)) =
            ui.children(root, Accordion::new().multiple(), |accordion| {
                let (first, _) = accordion.section("One", |_| {});
                let (second, _) = accordion.section("Two", |_| {});
                (first, second)
            });

        assert!(set_accordion_section_expanded(&mut ui, first, true));
        assert!(set_accordion_section_expanded(&mut ui, second, true));
        assert_eq!(accordion_section_expanded(&ui, first), Some(true));
        assert_eq!(accordion_section_expanded(&ui, second), Some(true));
    }

    #[test]
    fn clicking_section_header_toggles_and_reports_state() {
        let observed = Rc::new(Cell::new(None));
        let observed_for_handler = Rc::clone(&observed);
        let mut ui = Ui::new();
        let root = ui.root();
        let (_accordion, section) = ui.children(root, Accordion::new(), |accordion| {
            let (section, _) = accordion.section_with(
                AccordionSection::new("One").on_expanded_changed(move |_ui, event| {
                    observed_for_handler.set(Some((event.section, event.expanded)));
                }),
                |_| {},
            );
            section
        });
        let header = ui.slot(section, SLOT_ACCORDION_SECTION_HEADER).unwrap();

        ui.synthesize_click(header, Point::ZERO);

        assert_eq!(accordion_section_expanded(&ui, section), Some(true));
        assert_eq!(observed.get(), Some((section, true)));
    }

    #[test]
    fn accordion_section_accessibility_exposes_disclosure_state_and_actions() {
        let mut ui = Ui::new();
        let root = ui.root();
        let (_accordion, section) = ui.children(root, Accordion::new(), |accordion| {
            let (section, _) = accordion.section("One", |_| {});
            section
        });
        ui.compute_layout(Size::new(240.0, 120.0));
        let update = ui.accesskit_tree_update();
        let node = node_for(&update, section);

        assert_eq!(node.role(), Role::DisclosureTriangle);
        assert_eq!(node.is_expanded(), Some(false));
        assert!(node.supports_action(Action::Expand));

        assert!(ui.handle_accesskit_action(&action(section, Action::Expand)));
        let update = ui.accesskit_tree_update();
        let node = node_for(&update, section);
        assert_eq!(node.is_expanded(), Some(true));
        assert!(node.supports_action(Action::Collapse));
    }

    #[test]
    fn accordion_section_accessibility_exposes_expanded_content_slot() {
        let mut ui = Ui::new();
        let root = ui.root();
        let (_accordion, (section, details)) = ui.children(root, Accordion::new(), |accordion| {
            accordion.section_with(AccordionSection::new("One").expanded(true), |section| {
                section.add(crate::Text::new("Details"))
            })
        });
        ui.compute_layout(Size::new(240.0, 120.0));

        let update = ui.accesskit_tree_update();
        let node = node_for(&update, section);

        assert!(node.children().contains(&accesskit_node_id(details)));
        assert_eq!(node_for(&update, details).role(), Role::Label);

        assert!(set_accordion_section_expanded(&mut ui, section, false));
        let update = ui.accesskit_tree_update();
        let node = node_for(&update, section);

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
