// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `SegmentedControl`: compact single-selection button group.
//!
//! Roadmap:
//! - Keep segmented selection on the shared `ChoiceOption` path with
//!   `RadioGroup`, `Select`, and `ListBox`.
//! - Treat this as a visual selection control, not a second radio
//!   implementation: segmented items are templateable buttons coordinated by
//!   the owner.
//! - Grow toward typed option keys, icons, equal-width/block behavior, and item
//!   templates when the broader item-control API exists.

use alloc::{boxed::Box, string::String, vec::Vec};

use understory_style::{PartTag, TypeTag};

use crate::behavior::WidgetEventContext;
use crate::controls::choice::{
    ChoiceOption, ResolvedChoiceSelection, collect_options, first_enabled_index,
    last_enabled_index, option_label, selected_enabled_index, wrapping_next_enabled_index,
    wrapping_previous_enabled_index,
};
use crate::controls::support::{CallbackSlot, EventHandler};
use crate::element::{Element, ElementId};
use crate::event_runtime::{ClickEvent, KeyEvent};
use crate::layouts::{Layout, LayoutContainer, LayoutSpacing};
use crate::pseudos::PSEUDO_SELECTED;
use crate::template::{TemplateContract, TemplateNode, TemplateSlot, TemplateSpec};
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, ElementConfig, ElementSpec, Widget};

/// Selector identity for `SegmentedControl`.
pub const TYPE_SEGMENTED_CONTROL: TypeTag = TypeTag(26);
/// Selector identity for realized `SegmentedControl` items.
pub const TYPE_SEGMENTED_ITEM: TypeTag = TypeTag(27);
/// Owner-local part identity for a segment's visible label.
pub const PART_SEGMENTED_ITEM_LABEL: PartTag = PartTag(1);
/// Semantic slot for a segment's visible label.
pub const SLOT_SEGMENTED_ITEM_LABEL: TemplateSlot =
    TemplateSlot::new("segmented-label").with_part_tag(PART_SEGMENTED_ITEM_LABEL);

const DEFAULT_SEGMENTED_GAP: f32 = 2.0;

type SegmentedSelectionChangedHandler = EventHandler<SegmentedSelectionChangedEvent>;
type SegmentedSelectionChangedSlot = CallbackSlot<SegmentedSelectionChangedEvent>;

/// Selection change payload emitted by [`SegmentedControl`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SegmentedSelectionChangedEvent {
    /// The `SegmentedControl` element whose selection changed.
    pub segmented: ElementId,
    /// The selected item index after the change, if one is selected.
    pub selected_index: Option<usize>,
    /// The selected item text after the change, if one is selected.
    pub selected_item: Option<String>,
    /// The selected segment element, if one is selected.
    pub selected_segment: Option<ElementId>,
}

/// Builder + spec for a compact mutually-exclusive segmented control.
pub struct SegmentedControl {
    options: Vec<ChoiceOption>,
    selected_index: Option<usize>,
    gap: f32,
    element: ElementConfig,
    on_selection_changed: Option<SegmentedSelectionChangedHandler>,
}

impl core::fmt::Debug for SegmentedControl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SegmentedControl")
            .field("options_len", &self.options.len())
            .field("selected_index", &self.selected_index)
            .field("gap", &self.gap)
            .finish_non_exhaustive()
    }
}

impl SegmentedControl {
    /// Starts a new empty segmented control.
    #[must_use]
    pub fn new() -> Self {
        let mut element = ElementConfig::default();
        element.set_control_state(ControlState::interactive());
        element.set_layout(
            Layout::new()
                .container(LayoutContainer::row())
                .spacing(LayoutSpacing::gap(DEFAULT_SEGMENTED_GAP)),
        );
        Self {
            options: Vec::new(),
            selected_index: None,
            gap: DEFAULT_SEGMENTED_GAP,
            element,
            on_selection_changed: None,
        }
    }

    /// Replaces the control's options.
    #[must_use]
    pub fn options<I, O>(mut self, options: I) -> Self
    where
        I: IntoIterator<Item = O>,
        O: Into<ChoiceOption>,
    {
        self.options = collect_options(options);
        self
    }

    /// Replaces the control's item labels.
    #[must_use]
    pub fn items<I, O>(self, items: I) -> Self
    where
        I: IntoIterator<Item = O>,
        O: Into<ChoiceOption>,
    {
        self.options(items)
    }

    /// Appends one option.
    #[must_use]
    pub fn option(mut self, option: impl Into<ChoiceOption>) -> Self {
        self.options.push(option.into());
        self
    }

    /// Appends one item label.
    #[must_use]
    pub fn item(self, item: impl Into<ChoiceOption>) -> Self {
        self.option(item)
    }

    /// Sets the initially selected item index.
    #[must_use]
    pub fn selected_index(mut self, index: usize) -> Self {
        self.selected_index = Some(index);
        self
    }

    /// Sets the gap between segment items.
    #[must_use]
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self.sync_layout();
        self
    }

    /// Sets whether the control and its generated segment items start enabled.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.element
            .update_control_state(|state| state.enabled(enabled));
        self
    }

    /// Convenience for [`Self::enabled`] with `false`.
    #[must_use]
    pub fn disabled(self) -> Self {
        self.enabled(false)
    }

    /// Sets whether the control participates in focus traversal.
    #[must_use]
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.element
            .update_control_state(|state| state.focusable(focusable));
        self
    }

    /// Registers a selection-changed handler.
    #[must_use]
    pub fn on_selection_changed<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, SegmentedSelectionChangedEvent) + 'static,
    {
        self.on_selection_changed = Some(Box::new(handler));
        self
    }

    fn sync_layout(&mut self) {
        self.element.set_layout(
            Layout::new()
                .container(LayoutContainer::row())
                .spacing(LayoutSpacing::gap(self.gap)),
        );
    }
}

impl Default for SegmentedControl {
    fn default() -> Self {
        Self::new()
    }
}

impl AppendSpec for SegmentedControl {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let selected_index = selected_enabled_index(&self.options, self.selected_index);
        let enabled = self
            .element
            .control_state()
            .map(|state| state.enabled)
            .unwrap_or(true);
        let id = ui.append_widget(
            parent,
            Box::new(SegmentedControlWidget {
                options: self.options.clone(),
                segments: Vec::new(),
                selected_index,
                selection_changed: SegmentedSelectionChangedSlot::new(self.on_selection_changed),
            }),
        );
        self.element.apply_to(ui, id);

        let mut segments = Vec::with_capacity(self.options.len());
        for (index, option) in self.options.into_iter().enumerate() {
            let segment = append_segmented_item(ui, id, index, option, enabled);
            set_segmented_item_selected(ui, segment, Some(index) == selected_index);
            segments.push(segment);
        }
        if let Some(selected) = selected_index {
            ui.set_local(id, ui.properties().selected_index, Some(selected));
        }
        with_segmented_control_mut(ui, id, |widget, _ui| {
            widget.segments = segments;
        });
        id
    }
}

impl ElementSpec for SegmentedControl {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

pub(crate) struct SegmentedControlWidget {
    options: Vec<ChoiceOption>,
    segments: Vec<ElementId>,
    selected_index: Option<usize>,
    selection_changed: SegmentedSelectionChangedSlot,
}

impl core::fmt::Debug for SegmentedControlWidget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SegmentedControlWidget")
            .field("options_len", &self.options.len())
            .field("segments", &self.segments)
            .field("selected_index", &self.selected_index)
            .finish_non_exhaustive()
    }
}

impl Widget for SegmentedControlWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_SEGMENTED_CONTROL)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::RadioGroup)
    }

    fn configure_accessibility(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        node.set_size_of_set(self.options.len());
    }

    fn handle_key(&mut self, ctx: &mut WidgetEventContext<'_>, event: KeyEvent<'_>) {
        use ui_events::keyboard::{Key, KeyState, NamedKey};

        let segmented = ctx.id();
        if !event.is_target() {
            return;
        }
        if event.key.state != KeyState::Down || !ctx.is_enabled(segmented) {
            return;
        }

        let current = self.selected_index;
        let next = match &event.key.key {
            Key::Named(NamedKey::ArrowDown | NamedKey::ArrowRight) => {
                wrapping_next_enabled_index(&self.options, current)
            }
            Key::Named(NamedKey::ArrowUp | NamedKey::ArrowLeft) => {
                wrapping_previous_enabled_index(&self.options, current)
            }
            Key::Named(NamedKey::Home) => first_enabled_index(&self.options),
            Key::Named(NamedKey::End) => last_enabled_index(&self.options),
            Key::Named(NamedKey::Enter) => current.or_else(|| first_enabled_index(&self.options)),
            Key::Character(s) if s == " " => current.or_else(|| first_enabled_index(&self.options)),
            _ => None,
        };
        let Some(next) = next else {
            return;
        };
        if self.set_selected_index(ctx, segmented, Some(next)) {
            event.mark_handled();
        }
    }
}

impl SegmentedControlWidget {
    fn segment_index(&self, segment: ElementId) -> Option<usize> {
        self.segments
            .iter()
            .position(|candidate| *candidate == segment)
    }

    fn set_selected_index(
        &mut self,
        ui: &mut Ui,
        segmented: ElementId,
        index: Option<usize>,
    ) -> bool {
        if !ui.is_enabled(segmented) {
            return false;
        }
        let current = self.selected_index;
        let Some(selection) = ResolvedChoiceSelection::from_enabled_target(&self.options, index)
            .and_then(|selection| selection.changed_from(current))
        else {
            return false;
        };
        let index = selection.index;
        if let Some(previous) = self.selected_index {
            set_segmented_item_selected(ui, self.segments[previous], false);
        }
        self.selected_index = index;
        ui.set_local(segmented, ui.properties().selected_index, index);
        let selected_segment = index.map(|index| self.segments[index]);
        if let Some(selected_segment) = selected_segment {
            set_segmented_item_selected(ui, selected_segment, true);
        }
        self.selection_changed.dispatch(
            ui,
            SegmentedSelectionChangedEvent {
                segmented,
                selected_index: index,
                selected_item: selection.label,
                selected_segment,
            },
        );
        true
    }
}

#[derive(Debug)]
pub(crate) struct SegmentedItemWidget {
    index: usize,
}

impl Widget for SegmentedItemWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_SEGMENTED_ITEM)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::RadioButton)
    }

    fn configure_accessibility(
        &self,
        element: &Element,
        ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        ctx.add_click_action_if_enabled(element, node);
        ctx.apply_checked_state(element, node);
        node.set_position_in_set(self.index + 1);
    }

    fn default_template(&self) -> Option<&'static dyn crate::Template> {
        Some(&SEGMENTED_ITEM_TEMPLATE)
    }

    fn template_contract(&self) -> TemplateContract {
        TemplateContract::new().with_required_slot(SLOT_SEGMENTED_ITEM_LABEL)
    }

    fn handle_click(&mut self, ctx: &mut WidgetEventContext<'_>, event: ClickEvent<'_>) {
        let segment = ctx.id();
        if !event.is_semantic_target() {
            return;
        }
        if !ctx.is_enabled(segment) {
            return;
        }
        let Some(segmented) = segmented_parent(ctx, segment) else {
            return;
        };
        if select_segmented_item(ctx, segmented, self.index) {
            let _ = ctx.set_pointer_focus(Some(segmented));
            event.mark_handled();
        }
    }

    fn handle_accessibility_action(
        &mut self,
        ctx: &mut WidgetEventContext<'_>,
        request: &accesskit::ActionRequest,
    ) -> bool {
        if request.action != accesskit::Action::Click || !ctx.is_enabled(ctx.id()) {
            return false;
        }
        let Some(segmented) = segmented_parent(ctx, ctx.id()) else {
            return false;
        };
        select_segmented_item(ctx, segmented, self.index)
    }
}

type SegmentedItemTemplate = TemplateSpec;

static SEGMENTED_ITEM_TEMPLATE: SegmentedItemTemplate =
    TemplateSpec::new(SEGMENTED_ITEM_TEMPLATE_NODES);

const SEGMENTED_ITEM_TEMPLATE_NODES: &[TemplateNode] =
    &[TemplateNode::content_presenter(SLOT_SEGMENTED_ITEM_LABEL)];

fn append_segmented_item(
    ui: &mut Ui,
    segmented: ElementId,
    index: usize,
    option: ChoiceOption,
    segmented_enabled: bool,
) -> ElementId {
    let enabled = segmented_enabled && option.is_enabled();
    let id = ui.append_widget(segmented, Box::new(SegmentedItemWidget { index }));
    ui.set_local(id, ui.properties().content, option.into_label());
    ui.set_control_state(
        id,
        ControlState::interactive()
            .focusable(false)
            .enabled(enabled),
    );
    id
}

fn set_segmented_item_selected(ui: &mut Ui, segment: ElementId, selected: bool) {
    ui.set_local(segment, ui.properties().is_checked, selected);
    if selected {
        ui.add_pseudo(segment, PSEUDO_SELECTED);
    } else {
        ui.remove_pseudo(segment, PSEUDO_SELECTED);
    }
}

/// Selects `index` in `segmented`, replacing any previous selected item.
///
/// Returns `false` if `segmented` is stale, disabled, not a
/// [`SegmentedControl`], already has that selection, or if `index` is outside
/// the option data.
pub fn set_segmented_index(ui: &mut Ui, segmented: ElementId, index: usize) -> bool {
    set_segmented_index_internal(ui, segmented, Some(index))
}

/// Clears the selected item in `segmented`.
///
/// Returns `false` if `segmented` is stale, disabled, not a
/// [`SegmentedControl`], or if it already has no selected item.
pub fn clear_segmented_selection(ui: &mut Ui, segmented: ElementId) -> bool {
    set_segmented_index_internal(ui, segmented, None)
}

fn set_segmented_index_internal(ui: &mut Ui, segmented: ElementId, index: Option<usize>) -> bool {
    with_segmented_control_mut(ui, segmented, |widget, ui| {
        widget.set_selected_index(ui, segmented, index)
    })
    .unwrap_or(false)
}

/// Returns the currently selected segment index, if any.
#[must_use]
pub fn selected_segmented_index(ui: &Ui, segmented: ElementId) -> Option<usize> {
    segmented_control_widget(ui, segmented).and_then(|widget| widget.selected_index)
}

/// Returns the currently selected segment item text, if any.
#[must_use]
pub fn selected_segmented_item(ui: &Ui, segmented: ElementId) -> Option<String> {
    let widget = segmented_control_widget(ui, segmented)?;
    option_label(&widget.options, widget.selected_index)
}

/// Returns the generated segment element for `index`, if currently present.
#[must_use]
pub fn segmented_item(ui: &Ui, segmented: ElementId, index: usize) -> Option<ElementId> {
    segmented_control_widget(ui, segmented)?
        .segments
        .get(index)
        .copied()
        .filter(|segment| ui.get(*segment).is_some())
}

fn select_segmented_item(ui: &mut Ui, segmented: ElementId, index: usize) -> bool {
    with_segmented_control_mut(ui, segmented, |widget, ui| {
        let Some(segment) = widget.segments.get(index).copied() else {
            return false;
        };
        if widget.segment_index(segment) != Some(index) {
            return false;
        }
        widget.set_selected_index(ui, segmented, Some(index))
    })
    .unwrap_or(false)
}

fn segmented_parent(ui: &Ui, segment: ElementId) -> Option<ElementId> {
    let parent = ui.get(segment)?.parent()?;
    segmented_control_widget(ui, parent)
        .is_some()
        .then_some(parent)
}

fn with_segmented_control_mut<R>(
    ui: &mut Ui,
    segmented: ElementId,
    f: impl FnOnce(&mut SegmentedControlWidget, &mut Ui) -> R,
) -> Option<R> {
    let mut output = None;
    ui.with_widget_mut(segmented, |widget, ui| {
        if let Some(segmented_widget) = widget.downcast_mut::<SegmentedControlWidget>() {
            output = Some(f(segmented_widget, ui));
        }
    });
    output
}

fn segmented_control_widget(ui: &Ui, segmented: ElementId) -> Option<&SegmentedControlWidget> {
    ui.get(segmented)
        .and_then(|element| element.widget())
        .and_then(|widget| widget.downcast_ref::<SegmentedControlWidget>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accessibility::accesskit_node_id;
    use accesskit::Role;
    use alloc::rc::Rc;
    use core::cell::Cell;
    use kurbo::{Point, Size};
    use ui_events::keyboard::{Code, Key, KeyboardEvent, NamedKey};

    const ITEMS: [&str; 3] = ["Preview", "Code", "Metrics"];

    #[test]
    fn segmented_control_selects_one_generated_item() {
        let mut ui = Ui::new();
        let root = ui.root();
        let segmented = ui.append(root, SegmentedControl::new().items(ITEMS).selected_index(0));
        let first = segmented_item(&ui, segmented, 0).unwrap();
        let second = segmented_item(&ui, segmented, 1).unwrap();

        assert_eq!(selected_segmented_index(&ui, segmented), Some(0));
        assert!(ui.get(first).unwrap().pseudos().contains(&PSEUDO_SELECTED));

        ui.synthesize_click(second, Point::ZERO);

        assert_eq!(selected_segmented_index(&ui, segmented), Some(1));
        assert!(!ui.get(first).unwrap().pseudos().contains(&PSEUDO_SELECTED));
        assert!(ui.get(second).unwrap().pseudos().contains(&PSEUDO_SELECTED));
    }

    #[test]
    fn segmented_control_selection_changed_reports_item_text() {
        let observed = Rc::new(Cell::new(None));
        let observed_for_handler = Rc::clone(&observed);
        let mut ui = Ui::new();
        let root = ui.root();
        let segmented = ui.append(
            root,
            SegmentedControl::new()
                .items(ITEMS)
                .on_selection_changed(move |_ui, event| {
                    observed_for_handler.set(event.selected_index);
                    assert_eq!(event.selected_item.as_deref(), Some("Metrics"));
                }),
        );
        let third = segmented_item(&ui, segmented, 2).unwrap();

        ui.synthesize_click(third, Point::ZERO);

        assert_eq!(observed.get(), Some(2));
        assert_eq!(
            selected_segmented_item(&ui, segmented).as_deref(),
            Some("Metrics")
        );
    }

    #[test]
    fn segmented_control_skips_disabled_options() {
        let mut ui = Ui::new();
        let root = ui.root();
        let segmented = ui.append(
            root,
            SegmentedControl::new()
                .options([
                    ChoiceOption::new("Preview"),
                    ChoiceOption::new("Code").disabled(),
                    ChoiceOption::new("Metrics"),
                ])
                .selected_index(0),
        );
        let disabled = segmented_item(&ui, segmented, 1).unwrap();

        assert!(!ui.is_enabled(disabled));
        assert!(!set_segmented_index(&mut ui, segmented, 1));
        assert_eq!(selected_segmented_index(&ui, segmented), Some(0));

        ui.set_focus(Some(segmented));
        let key = KeyboardEvent::key_down(Key::Named(NamedKey::ArrowRight), Code::ArrowRight);
        assert!(ui.dispatch_key(&key).handled);
        assert_eq!(selected_segmented_index(&ui, segmented), Some(2));
    }

    #[test]
    fn segmented_accessibility_uses_radio_group_roles() {
        let mut ui = Ui::new();
        let root = ui.root();
        let segmented = ui.append(root, SegmentedControl::new().items(ITEMS).selected_index(0));
        ui.compute_layout(Size::new(240.0, 80.0));
        let first = segmented_item(&ui, segmented, 0).unwrap();
        let update = ui.accesskit_tree_update();

        let group_node = node_for(&update, segmented);
        let first_node = node_for(&update, first);
        assert_eq!(group_node.role(), Role::RadioGroup);
        assert_eq!(first_node.role(), Role::RadioButton);
        assert_eq!(first_node.toggled(), Some(accesskit::Toggled::True));
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
}
