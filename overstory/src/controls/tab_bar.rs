// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Keyed tab-header selection.

use alloc::{boxed::Box, rc::Rc, string::String, vec::Vec};

use understory_selection::Selection;
use understory_style::TypeTag;

use crate::Text;
use crate::behavior::{ArrangeContext, ArrangeResult, WidgetEventContext};
use crate::controls::support::{CallbackSlot, EventHandler};
use crate::element::{Element, ElementId};
use crate::event_runtime::{ClickEvent, KeyEvent};
use crate::layouts::Layout;
use crate::pseudos::PSEUDO_SELECTED;
use crate::ui::{ChildUi, ControlState, Ui};
use crate::widget::{AppendSpec, ElementConfig, ElementSpec, Widget};

/// Selector identity for [`TabBar`].
pub const TYPE_TAB_BAR: TypeTag = TypeTag(35);
/// Selector identity for a generated tab header.
pub const TYPE_TAB_ITEM: TypeTag = TypeTag(36);

/// Stable application identity for a tab.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TabKey(u64);

impl TabKey {
    /// Creates a tab key.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the underlying application value.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl From<u64> for TabKey {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

/// Label and availability metadata for one tab.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabOption {
    key: TabKey,
    label: String,
    enabled: bool,
}

impl TabOption {
    /// Creates an enabled tab.
    #[must_use]
    pub fn new(key: impl Into<TabKey>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            enabled: true,
        }
    }

    /// Returns the stable tab key.
    #[must_use]
    pub const fn key(&self) -> TabKey {
        self.key
    }

    /// Returns the visible and accessible label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns whether user input may select this tab.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Sets whether user input may select this tab.
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Disables this tab.
    #[must_use]
    pub const fn disabled(self) -> Self {
        self.enabled(false)
    }
}

/// Context supplied to a tab-header template.
#[derive(Clone, Copy, Debug)]
pub struct TabItemContext<'a> {
    index: usize,
    option: &'a TabOption,
}

impl<'a> TabItemContext<'a> {
    /// Returns the tab's ordered index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.index
    }

    /// Returns the tab metadata.
    #[must_use]
    pub const fn option(self) -> &'a TabOption {
        self.option
    }
}

type SharedTabItemTemplate = Rc<dyn for<'ui, 'item> Fn(&mut ChildUi<'ui>, TabItemContext<'item>)>;
type TabSelectionChangedHandler = EventHandler<TabSelectionChangedEvent>;

/// Event emitted after keyed tab selection changes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabSelectionChangedEvent {
    /// Tab bar that changed.
    pub tab_bar: ElementId,
    /// Newly selected stable key.
    pub selected_key: Option<TabKey>,
    /// Current ordered index.
    pub selected_index: Option<usize>,
    /// Selected tab-header element.
    pub selected_tab: Option<ElementId>,
}

/// Builder for a keyed horizontal tab header strip.
pub struct TabBar {
    options: Vec<TabOption>,
    selected_key: Option<TabKey>,
    item_template: Option<SharedTabItemTemplate>,
    on_selection_changed: Option<TabSelectionChangedHandler>,
    element: ElementConfig,
}

impl core::fmt::Debug for TabBar {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TabBar")
            .field("options", &self.options)
            .field("selected_key", &self.selected_key)
            .finish_non_exhaustive()
    }
}

impl TabBar {
    /// Creates an empty tab bar.
    #[must_use]
    pub fn new() -> Self {
        let mut element = ElementConfig::default();
        element.set_layout(Layout::row(2.0));
        element.set_control_state(ControlState::interactive());
        Self {
            options: Vec::new(),
            selected_key: None,
            item_template: None,
            on_selection_changed: None,
            element,
        }
    }

    /// Replaces the keyed tab metadata.
    #[must_use]
    pub fn tabs(mut self, tabs: impl IntoIterator<Item = TabOption>) -> Self {
        self.options = tabs.into_iter().collect();
        assert_unique_keys(&self.options);
        self
    }

    /// Appends one keyed tab.
    #[must_use]
    pub fn tab(mut self, tab: TabOption) -> Self {
        assert!(
            !self.options.iter().any(|item| item.key == tab.key),
            "duplicate TabKey"
        );
        self.options.push(tab);
        self
    }

    /// Sets the initially selected stable key.
    #[must_use]
    pub fn selected_key(mut self, key: impl Into<TabKey>) -> Self {
        self.selected_key = Some(key.into());
        self
    }

    /// Sets a repeatable retained header recipe.
    #[must_use]
    pub fn item_template<F>(mut self, template: F) -> Self
    where
        F: for<'ui, 'item> Fn(&mut ChildUi<'ui>, TabItemContext<'item>) + 'static,
    {
        self.item_template = Some(Rc::new(template));
        self
    }

    /// Registers a keyed selection callback.
    #[must_use]
    pub fn on_selection_changed<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, TabSelectionChangedEvent) + 'static,
    {
        self.on_selection_changed = Some(Box::new(handler));
        self
    }
}

impl Default for TabBar {
    fn default() -> Self {
        Self::new()
    }
}

impl AppendSpec for TabBar {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        assert_unique_keys(&self.options);
        let selected = valid_key(&self.options, self.selected_key).or_else(|| {
            self.options
                .iter()
                .find(|tab| tab.enabled)
                .map(|tab| tab.key)
        });
        let mut selection = Selection::new();
        if let Some(key) = selected {
            selection.select_only(key);
        }
        let id = ui.append_widget(
            parent,
            Box::new(TabBarWidget {
                options: self.options,
                items: Vec::new(),
                selection,
                item_template: self.item_template,
                changed: CallbackSlot::new(self.on_selection_changed),
            }),
        );
        self.element.apply_to(ui, id);
        rebuild_items(ui, id);
        id
    }
}

impl ElementSpec for TabBar {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

struct TabBarWidget {
    options: Vec<TabOption>,
    items: Vec<ElementId>,
    selection: Selection<TabKey>,
    item_template: Option<SharedTabItemTemplate>,
    changed: CallbackSlot<TabSelectionChangedEvent>,
}

impl core::fmt::Debug for TabBarWidget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TabBarWidget")
            .field("options", &self.options)
            .field("items", &self.items)
            .field("selection", &self.selection)
            .finish_non_exhaustive()
    }
}

impl Widget for TabBarWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_TAB_BAR)
    }
    fn accessibility_role(
        &self,
        _: &Element,
        _: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::TabList)
    }

    fn handle_key(&mut self, ctx: &mut WidgetEventContext<'_>, event: KeyEvent<'_>) {
        use ui_events::keyboard::{Key, KeyState, NamedKey};
        if !event.is_target() || event.key.state != KeyState::Down {
            return;
        }
        let current = self.selected_index().unwrap_or(0);
        let target = match event.key.key {
            Key::Named(NamedKey::ArrowRight | NamedKey::ArrowDown) => self.next_enabled(current, 1),
            Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowUp) => {
                self.next_enabled(current, self.options.len().saturating_sub(1))
            }
            Key::Named(NamedKey::Home) => self.options.iter().position(|tab| tab.enabled),
            Key::Named(NamedKey::End) => self.options.iter().rposition(|tab| tab.enabled),
            _ => None,
        };
        let tab_bar = ctx.id();
        if let Some(index) = target
            && self.select(ctx, tab_bar, self.options[index].key)
        {
            event.mark_handled();
        }
    }

    fn arrange(&mut self, ctx: &mut ArrangeContext<'_>) -> ArrangeResult {
        let index = self.selected_index();
        let property = ctx.ui().properties().selected_index;
        if ctx
            .ui()
            .get(ctx.id())
            .is_some_and(|element| ctx.ui().resolve_value_for(element, property) == index)
        {
            return ArrangeResult::UNCHANGED;
        }
        let tab_bar = ctx.id();
        ctx.ui_mut().set_local(tab_bar, property, index);
        ArrangeResult::LAYOUT_CHANGED
    }
}

impl TabBarWidget {
    fn selected_key(&self) -> Option<TabKey> {
        self.selection.primary().copied()
    }
    fn selected_index(&self) -> Option<usize> {
        let key = self.selected_key()?;
        self.options.iter().position(|tab| tab.key == key)
    }
    fn next_enabled(&self, current: usize, step: usize) -> Option<usize> {
        let len = self.options.len();
        if len == 0 {
            return None;
        }
        (1..=len)
            .map(|offset| (current + offset * step) % len)
            .find(|index| self.options[*index].enabled)
    }
    fn select(&mut self, ui: &mut Ui, tab_bar: ElementId, key: TabKey) -> bool {
        let Some(index) = self
            .options
            .iter()
            .position(|tab| tab.key == key && tab.enabled)
        else {
            return false;
        };
        if !self.selection.select_only(key) {
            return false;
        }
        sync_selected_items(ui, &self.items, index);
        ui.set_local(tab_bar, ui.properties().selected_index, Some(index));
        self.changed.dispatch(
            ui,
            TabSelectionChangedEvent {
                tab_bar,
                selected_key: Some(key),
                selected_index: Some(index),
                selected_tab: self.items.get(index).copied(),
            },
        );
        true
    }
}

#[derive(Debug)]
struct TabItemWidget {
    index: usize,
}

impl Widget for TabItemWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_TAB_ITEM)
    }
    fn accessibility_role(
        &self,
        _: &Element,
        _: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::Tab)
    }
    fn configure_accessibility(
        &self,
        element: &Element,
        ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        ctx.add_click_action_if_enabled(element, node);
        node.set_selected(ctx.is_selected(element));
        node.set_position_in_set(self.index + 1);
    }
    fn handle_click(&mut self, ctx: &mut WidgetEventContext<'_>, event: ClickEvent<'_>) {
        if !ctx.is_enabled(ctx.id()) {
            return;
        }
        let Some(parent) = ctx.ui().get(ctx.id()).and_then(Element::parent) else {
            return;
        };
        let key = tab_key_at(ctx.ui(), parent, self.index);
        if select_tab_key(ctx, parent, key) {
            let _ = ctx.set_pointer_focus(Some(parent));
            event.mark_handled();
        }
    }
}

fn rebuild_items(ui: &mut Ui, tab_bar: ElementId) {
    let (options, template, old) = tab_bar_widget(ui, tab_bar)
        .map(|widget| {
            (
                widget.options.clone(),
                widget.item_template.clone(),
                widget.items.clone(),
            )
        })
        .unwrap();
    for item in old {
        ui.remove(item);
    }
    let selected = selected_tab_key(ui, tab_bar);
    let enabled = ui.is_enabled(tab_bar);
    let mut items = Vec::new();
    for (index, option) in options.iter().enumerate() {
        let item = ui.append_widget(tab_bar, Box::new(TabItemWidget { index }));
        ui.set_control_state(
            item,
            ControlState::interactive()
                .focusable(false)
                .enabled(enabled && option.enabled),
        );
        ui.set_local(item, ui.properties().content, option.label.clone());
        if let Some(template) = &template {
            ui.with_children(item, |children| {
                template(children, TabItemContext { index, option });
            });
        } else {
            ui.append(item, Text::new(option.label.clone()));
        }
        items.push(item);
    }
    let selected_index = selected.and_then(|key| options.iter().position(|tab| tab.key == key));
    sync_selected_items(ui, &items, selected_index.unwrap_or(usize::MAX));
    ui.with_widget_mut(tab_bar, |widget, _| {
        widget.downcast_mut::<TabBarWidget>().unwrap().items = items;
    });
}

fn sync_selected_items(ui: &mut Ui, items: &[ElementId], selected: usize) {
    for (index, item) in items.iter().copied().enumerate() {
        ui.set_local(item, ui.properties().is_checked, index == selected);
        if index == selected {
            ui.add_pseudo(item, PSEUDO_SELECTED);
        } else {
            ui.remove_pseudo(item, PSEUDO_SELECTED);
        }
    }
}
fn assert_unique_keys(options: &[TabOption]) {
    for (index, tab) in options.iter().enumerate() {
        assert!(
            !options[..index].iter().any(|other| other.key == tab.key),
            "duplicate TabKey {:?}",
            tab.key
        );
    }
}
fn valid_key(options: &[TabOption], key: Option<TabKey>) -> Option<TabKey> {
    key.filter(|key| options.iter().any(|tab| tab.key == *key && tab.enabled))
}
fn tab_bar_widget(ui: &Ui, id: ElementId) -> Option<&TabBarWidget> {
    ui.get(id)?.widget()?.downcast_ref()
}

pub(crate) fn tab_options(ui: &Ui, id: ElementId) -> Option<&[TabOption]> {
    Some(&tab_bar_widget(ui, id)?.options)
}
fn tab_key_at(ui: &Ui, id: ElementId, index: usize) -> Option<TabKey> {
    tab_bar_widget(ui, id)?
        .options
        .get(index)
        .map(|tab| tab.key)
}

/// Selects an enabled tab by stable key.
pub fn select_tab_key(ui: &mut Ui, tab_bar: ElementId, key: Option<TabKey>) -> bool {
    let Some(key) = key else {
        return false;
    };
    let mut changed = false;
    ui.with_widget_mut(tab_bar, |widget, ui| {
        if let Some(widget) = widget.downcast_mut::<TabBarWidget>() {
            changed = widget.select(ui, tab_bar, key);
        }
    });
    changed
}
/// Returns the selected stable tab key.
#[must_use]
pub fn selected_tab_key(ui: &Ui, tab_bar: ElementId) -> Option<TabKey> {
    tab_bar_widget(ui, tab_bar)?.selected_key()
}
/// Returns the selected tab index.
#[must_use]
pub fn selected_tab_index(ui: &Ui, tab_bar: ElementId) -> Option<usize> {
    tab_bar_widget(ui, tab_bar)?.selected_index()
}
/// Replaces tab metadata, preserving selection by key when possible.
pub fn replace_tab_options(
    ui: &mut Ui,
    tab_bar: ElementId,
    options: impl IntoIterator<Item = TabOption>,
) -> bool {
    let options: Vec<_> = options.into_iter().collect();
    assert_unique_keys(&options);
    let Some(current) = tab_bar_widget(ui, tab_bar).map(TabBarWidget::selected_key) else {
        return false;
    };
    ui.with_widget_mut(tab_bar, |widget, _| {
        let widget = widget.downcast_mut::<TabBarWidget>().unwrap();
        widget.options = options;
        let target = valid_key(&widget.options, current).or_else(|| {
            widget
                .options
                .iter()
                .find(|tab| tab.enabled)
                .map(|tab| tab.key)
        });
        widget.selection.clear();
        if let Some(key) = target {
            widget.selection.select_only(key);
        }
    });
    rebuild_items(ui, tab_bar);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Size;
    #[test]
    fn keys_survive_reordering() {
        let mut ui = Ui::new();
        let root = ui.root();
        let bar = ui.append(
            root,
            TabBar::new()
                .tabs([TabOption::new(1, "One"), TabOption::new(2, "Two")])
                .selected_key(2),
        );
        ui.compute_layout(Size::new(300.0, 80.0));
        assert_eq!(selected_tab_key(&ui, bar), Some(TabKey::new(2)));
        replace_tab_options(
            &mut ui,
            bar,
            [TabOption::new(2, "Two"), TabOption::new(1, "One")],
        );
        assert_eq!(selected_tab_key(&ui, bar), Some(TabKey::new(2)));
        assert_eq!(selected_tab_index(&ui, bar), Some(0));
    }
}
