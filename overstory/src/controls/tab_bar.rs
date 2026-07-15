// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Keyed tab-header selection.

use alloc::{boxed::Box, format, rc::Rc, string::String, vec::Vec};

use kurbo::{Axis, Vec2};
use understory_selection::Selection;
use understory_style::TypeTag;

use crate::ScrollState;
use crate::Text;
use crate::behavior::{ArrangeContext, ArrangeResult, WidgetEventContext};
use crate::controls::scroll::runtime::{
    scroll_by, scroll_by_accessibility_action, set_scroll_offset_raw,
    update_scroll_metrics_from_layout, wheel_scroll_delta,
};
use crate::controls::support::{CallbackSlot, EventHandler};
use crate::element::{Element, ElementId};
use crate::event_runtime::{ClickEvent, KeyEvent, ScrollEvent};
use crate::layouts::Layout;
use crate::pseudos::PSEUDO_SELECTED;
use crate::ui::{ChildUi, ControlState, Ui};
use crate::widget::{AppendSpec, ElementConfig, ElementSpec, Widget};

/// Selector identity for [`TabBar`].
pub const TYPE_TAB_BAR: TypeTag = TypeTag(35);
/// Selector identity for a generated tab header.
pub const TYPE_TAB_ITEM: TypeTag = TypeTag(36);
/// Selector identity for a generated tab close button.
pub const TYPE_TAB_CLOSE_BUTTON: TypeTag = TypeTag(39);

/// Behavior used when tab headers are wider than their viewport.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TabOverflow {
    /// Retains every tab at its natural width and scrolls the strip horizontally.
    #[default]
    Scroll,
    /// Allows tab headers to shrink to share the available width.
    Compress,
}

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
    closable: bool,
}

impl TabOption {
    /// Creates an enabled tab.
    #[must_use]
    pub fn new(key: impl Into<TabKey>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            enabled: true,
            closable: false,
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

    /// Returns whether the tab presents a close control.
    #[must_use]
    pub const fn is_closable(&self) -> bool {
        self.closable
    }

    /// Sets whether the tab presents a close control.
    #[must_use]
    pub const fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
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
type TabCloseRequestedHandler = EventHandler<TabCloseRequestedEvent>;

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

/// Event emitted when a tab's close control is invoked.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabCloseRequestedEvent {
    /// Tab bar receiving the request.
    pub tab_bar: ElementId,
    /// Stable key of the tab to close.
    pub key: TabKey,
    /// Tab-header element associated with the request.
    pub tab: ElementId,
}

/// Builder for a keyed horizontal tab header strip.
pub struct TabBar {
    options: Vec<TabOption>,
    selected_key: Option<TabKey>,
    item_template: Option<SharedTabItemTemplate>,
    on_selection_changed: Option<TabSelectionChangedHandler>,
    on_close_requested: Option<TabCloseRequestedHandler>,
    overflow: TabOverflow,
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
            on_close_requested: None,
            overflow: TabOverflow::Scroll,
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

    /// Sets how headers behave when they exceed the available width.
    #[must_use]
    pub const fn overflow(mut self, overflow: TabOverflow) -> Self {
        self.overflow = overflow;
        self
    }

    /// Registers a close-request callback.
    ///
    /// A request does not remove application-owned content. The owner decides
    /// whether and when to update the tab collection.
    #[must_use]
    pub fn on_close_requested<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, TabCloseRequestedEvent) + 'static,
    {
        self.on_close_requested = Some(Box::new(handler));
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
                close_requested: CallbackSlot::new(self.on_close_requested),
                overflow: self.overflow,
                scroll: ScrollState::new(),
                reveal_selection: true,
            }),
        );
        let scroll = tab_bar_widget(ui, id).unwrap().scroll.clone();
        ui.register_scroll_target(id, scroll);
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
    close_requested: CallbackSlot<TabCloseRequestedEvent>,
    overflow: TabOverflow,
    scroll: ScrollState,
    reveal_selection: bool,
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

    fn configure_accessibility(
        &self,
        element: &Element,
        ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        if self.overflow != TabOverflow::Scroll {
            return;
        }
        let offset = self.scroll.offset();
        let max = self.scroll.max_offset();
        node.set_scroll_x(offset.x);
        node.set_scroll_x_min(0.0);
        node.set_scroll_x_max(max.x);
        if ctx.is_enabled(element.id()) && max.x > f64::EPSILON {
            node.add_action(accesskit::Action::ScrollLeft);
            node.add_action(accesskit::Action::ScrollRight);
            node.add_action(accesskit::Action::SetScrollOffset);
        }
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
        let mut changed = false;
        if self.overflow == TabOverflow::Scroll {
            update_scroll_metrics_from_layout(ctx, &self.scroll);
            if self.reveal_selection {
                changed |= self.reveal_selected(ctx);
                self.reveal_selection = false;
            }
        }
        let index = self.selected_index();
        let property = ctx.ui().properties().selected_index;
        if ctx
            .ui()
            .get(ctx.id())
            .is_some_and(|element| ctx.ui().resolve_value_for(element, property) == index)
        {
            return ArrangeResult::from_layout_changed(changed);
        }
        let tab_bar = ctx.id();
        ctx.ui_mut().set_local(tab_bar, property, index);
        ArrangeResult::LAYOUT_CHANGED
    }

    fn clips_subtree(&self, _element: &Element) -> bool {
        self.overflow == TabOverflow::Scroll
    }

    fn child_layout_offset(&self, _element: &Element) -> Vec2 {
        if self.overflow == TabOverflow::Scroll {
            Vec2::new(-self.scroll.offset().x, 0.0)
        } else {
            Vec2::ZERO
        }
    }

    fn handle_scroll(&mut self, ctx: &mut WidgetEventContext<'_>, event: ScrollEvent<'_>) {
        if self.overflow != TabOverflow::Scroll {
            return;
        }
        let Some(delta) = wheel_scroll_delta(ctx, ctx.id(), event) else {
            return;
        };
        let horizontal = if delta.x.abs() > f64::EPSILON {
            delta.x
        } else {
            delta.y
        };
        let id = ctx.id();
        if scroll_by(ctx, id, &self.scroll, Vec2::new(horizontal, 0.0)).is_some() {
            event.mark_handled();
        }
    }

    fn handle_accessibility_action(
        &mut self,
        ctx: &mut WidgetEventContext<'_>,
        request: &accesskit::ActionRequest,
    ) -> bool {
        if self.overflow != TabOverflow::Scroll || !ctx.is_enabled(ctx.id()) {
            return false;
        }
        let id = ctx.id();
        match request.action {
            accesskit::Action::ScrollLeft => scroll_by_accessibility_action(
                ctx,
                id,
                &self.scroll,
                Axis::Horizontal,
                -1.0,
                request,
            )
            .is_some(),
            accesskit::Action::ScrollRight => scroll_by_accessibility_action(
                ctx,
                id,
                &self.scroll,
                Axis::Horizontal,
                1.0,
                request,
            )
            .is_some(),
            accesskit::Action::SetScrollOffset => {
                let Some(accesskit::ActionData::SetScrollOffset(point)) = request.data.as_ref()
                else {
                    return false;
                };
                set_scroll_offset_raw(ctx, id, &self.scroll, Vec2::new(point.x, 0.0)).is_some()
            }
            _ => false,
        }
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
        self.reveal_selection = true;
        sync_selected_items(ui, &self.items, index);
        ui.set_local(tab_bar, ui.properties().selected_index, Some(index));
        self.changed.dispatch(
            ui,
            tab_bar,
            TabSelectionChangedEvent {
                tab_bar,
                selected_key: Some(key),
                selected_index: Some(index),
                selected_tab: self.items.get(index).copied(),
            },
        );
        true
    }

    fn reveal_selected(&self, ctx: &mut ArrangeContext<'_>) -> bool {
        let Some(index) = self.selected_index() else {
            return false;
        };
        let Some(viewport) = ctx.effective_bounds(ctx.id()) else {
            return false;
        };
        let Some(item) = self
            .items
            .get(index)
            .and_then(|item| ctx.effective_bounds(*item))
        else {
            return false;
        };
        let current = self.scroll.offset().x;
        let logical_left = item.x0 + current - viewport.x0;
        let logical_right = item.x1 + current - viewport.x0;
        let next = if logical_left < current {
            logical_left
        } else if logical_right > current + viewport.width() {
            logical_right - viewport.width()
        } else {
            current
        };
        let id = ctx.id();
        set_scroll_offset_raw(ctx.ui_mut(), id, &self.scroll, Vec2::new(next, 0.0)).is_some()
    }
}

#[derive(Debug)]
struct TabItemWidget {
    index: usize,
    tab_bar: ElementId,
    clips_content: bool,
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
        if event.phase().is_capture() || !ctx.is_enabled(ctx.id()) {
            return;
        }
        let key = tab_key_at(ctx.ui(), self.tab_bar, self.index);
        if select_tab_key(ctx, self.tab_bar, key) {
            let _ = ctx.set_pointer_focus(Some(self.tab_bar));
            event.mark_handled();
        }
    }
    fn clips_subtree(&self, _element: &Element) -> bool {
        self.clips_content
    }
}

#[derive(Debug)]
struct TabCloseButtonWidget {
    tab_bar: ElementId,
    tab: ElementId,
    key: TabKey,
}

impl Widget for TabCloseButtonWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_TAB_CLOSE_BUTTON)
    }
    fn accessibility_role(
        &self,
        _: &Element,
        _: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::Button)
    }
    fn configure_accessibility(
        &self,
        element: &Element,
        ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        ctx.add_click_action_if_enabled(element, node);
    }
    fn handle_click(&mut self, ctx: &mut WidgetEventContext<'_>, event: ClickEvent<'_>) {
        if !ctx.is_enabled(ctx.id()) {
            return;
        }
        dispatch_close_request(ctx, self.tab_bar, self.key, self.tab);
        event.mark_handled();
    }
    fn handle_key(&mut self, ctx: &mut WidgetEventContext<'_>, event: KeyEvent<'_>) {
        use ui_events::keyboard::{Key, KeyState, NamedKey};
        if !event.is_target() || event.key.state != KeyState::Down || !ctx.is_enabled(ctx.id()) {
            return;
        }
        let activates = matches!(event.key.key, Key::Named(NamedKey::Enter))
            || matches!(&event.key.key, Key::Character(character) if character == " ");
        if activates {
            dispatch_close_request(ctx, self.tab_bar, self.key, self.tab);
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
        dispatch_close_request(ctx, self.tab_bar, self.key, self.tab);
        true
    }
}

fn dispatch_close_request(ui: &mut Ui, tab_bar: ElementId, key: TabKey, tab: ElementId) {
    let Some(close_requested) =
        tab_bar_widget(ui, tab_bar).map(|widget| widget.close_requested.clone())
    else {
        return;
    };
    close_requested.dispatch(ui, tab_bar, TabCloseRequestedEvent { tab_bar, key, tab });
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
        let overflow = tab_bar_widget(ui, tab_bar).unwrap().overflow;
        let item = ui.append_widget(
            tab_bar,
            Box::new(TabItemWidget {
                index,
                tab_bar,
                clips_content: overflow == TabOverflow::Compress,
            }),
        );
        ui.set_layout(
            item,
            Layout::row(6.0)
                .align_center()
                .min_logical_size(0.0, 0.0)
                .flex_shrink(if overflow == TabOverflow::Scroll {
                    0.0
                } else {
                    1.0
                }),
        );
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
        if option.closable {
            let close = ui.append_widget(
                item,
                Box::new(TabCloseButtonWidget {
                    tab_bar,
                    tab: item,
                    key: option.key,
                }),
            );
            ui.set_control_state(close, ControlState::interactive());
            ui.set_local(
                close,
                ui.properties().content,
                format!("Close {}", option.label),
            );
            ui.append(close, Text::new("×"));
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
        widget.reveal_selection = true;
    });
    rebuild_items(ui, tab_bar);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ElementSpecExt, Size, scroll_offset_y};
    use core::cell::Cell;
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

    #[test]
    fn overflowing_tabs_scroll_and_reveal_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let bar = ui.append(
            root,
            TabBar::new()
                .tabs((0_u64..8).map(|key| TabOption::new(key, "A naturally wide tab")))
                .layout(Layout::new().logical_size(160.0, 40.0)),
        );
        ui.compute_layout(Size::new(160.0, 80.0));
        let state = ui.scroll_target(bar).unwrap().state();
        assert!(state.max_offset().x > 0.0);
        assert_eq!(scroll_offset_y(&ui, bar), Some(0.0));

        assert!(select_tab_key(&mut ui, bar, Some(TabKey::new(7))));
        ui.compute_layout(Size::new(160.0, 80.0));
        assert!(state.offset().x > 0.0);

        set_scroll_offset_raw(&mut ui, bar, &state, Vec2::ZERO);
        ui.compute_layout(Size::new(160.0, 80.0));
        assert_eq!(state.offset().x, 0.0);
    }

    #[test]
    fn close_request_does_not_change_selection() {
        let requested = Rc::new(Cell::new(None));
        let observed = requested.clone();
        let mut ui = Ui::new();
        let root = ui.root();
        let bar = ui.append(
            root,
            TabBar::new()
                .tabs([
                    TabOption::new(1, "One"),
                    TabOption::new(2, "Two").closable(true),
                ])
                .selected_key(1)
                .on_close_requested(move |_, event| observed.set(Some(event.key))),
        );
        let tab = tab_bar_widget(&ui, bar).unwrap().items[1];
        dispatch_close_request(&mut ui, bar, TabKey::new(2), tab);
        assert_eq!(requested.get(), Some(TabKey::new(2)));
        assert_eq!(selected_tab_key(&ui, bar), Some(TabKey::new(1)));
    }

    #[test]
    fn close_descendant_click_reaches_close_control() {
        let requested = Rc::new(Cell::new(None));
        let observed = requested.clone();
        let mut ui = Ui::new();
        let root = ui.root();
        let bar = ui.append(
            root,
            TabBar::new()
                .tabs([
                    TabOption::new(1, "One"),
                    TabOption::new(2, "Two").closable(true),
                ])
                .selected_key(1)
                .on_close_requested(move |_, event| observed.set(Some(event.key))),
        );
        let tab = tab_bar_widget(&ui, bar).unwrap().items[1];
        let close = *ui.get(tab).unwrap().children().last().unwrap();
        let glyph = ui.get(close).unwrap().children()[0];

        ui.synthesize_click(glyph, kurbo::Point::ZERO);

        assert_eq!(requested.get(), Some(TabKey::new(2)));
        assert_eq!(selected_tab_key(&ui, bar), Some(TabKey::new(1)));
    }
}
