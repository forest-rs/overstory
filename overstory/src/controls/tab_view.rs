// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Keyed tab headers composed with retained page content.

use alloc::{boxed::Box, string::String, vec::Vec};
use understory_style::TypeTag;

use crate::Panel;
use crate::behavior::{ArrangeContext, ArrangeResult};
use crate::controls::indexed_stack::{IndexedStack, set_indexed_stack_visible_index};
use crate::controls::support::{CallbackSlot, EventHandler};
use crate::controls::tab_bar::{
    TabBar, TabCloseRequestedEvent, TabItemContext, TabKey, TabOption, TabOverflow,
    replace_tab_options, select_tab_key, selected_tab_key, tab_bar_options,
};
use crate::element::{Element, ElementId};
use crate::layouts::Layout;
use crate::ui::{ChildUi, Ui};
use crate::widget::{AppendSpec, ElementConfig, ElementSpec, Widget};

/// Selector identity for [`TabView`].
pub const TYPE_TAB_VIEW: TypeTag = TypeTag(37);
/// Selector identity for a retained tab page.
pub const TYPE_TAB_PAGE: TypeTag = TypeTag(38);

type TabViewCloseRequestedHandler = EventHandler<TabViewCloseRequestedEvent>;

/// Event emitted when a retained tab page is asked to close.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabViewCloseRequestedEvent {
    /// Tab view receiving the request.
    pub tab_view: ElementId,
    /// Stable key of the page to close.
    pub key: TabKey,
    /// Retained page element associated with the request.
    pub page: ElementId,
}

/// Builder for keyed tabs with retained page content.
pub struct TabView {
    selected_key: Option<TabKey>,
    tab_bar: TabBar,
    on_close_requested: Option<TabViewCloseRequestedHandler>,
    element: ElementConfig,
}

impl core::fmt::Debug for TabView {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TabView")
            .field("selected_key", &self.selected_key)
            .finish_non_exhaustive()
    }
}

impl TabView {
    /// Creates an empty tab view.
    #[must_use]
    pub fn new() -> Self {
        let mut element = ElementConfig::default();
        element.set_layout(Layout::column(8.0));
        Self {
            selected_key: None,
            tab_bar: TabBar::new(),
            on_close_requested: None,
            element,
        }
    }

    /// Sets the initially selected page key.
    #[must_use]
    pub fn selected_key(mut self, key: impl Into<TabKey>) -> Self {
        let key = key.into();
        self.selected_key = Some(key);
        self.tab_bar = self.tab_bar.selected_key(key);
        self
    }

    /// Sets a repeatable retained recipe for tab headers.
    #[must_use]
    pub fn item_template<F>(mut self, template: F) -> Self
    where
        F: for<'ui, 'item> Fn(&mut ChildUi<'ui>, TabItemContext<'item>) + 'static,
    {
        self.tab_bar = self.tab_bar.item_template(template);
        self
    }

    /// Sets how the tab strip handles headers wider than its viewport.
    #[must_use]
    pub fn overflow(mut self, overflow: TabOverflow) -> Self {
        self.tab_bar = self.tab_bar.overflow(overflow);
        self
    }

    /// Registers a close-request callback for retained pages.
    #[must_use]
    pub fn on_close_requested<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, TabViewCloseRequestedEvent) + 'static,
    {
        self.on_close_requested = Some(Box::new(handler));
        self
    }
}

impl Default for TabView {
    fn default() -> Self {
        Self::new()
    }
}

impl AppendSpec for TabView {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget(
            parent,
            Box::new(TabViewWidget {
                tab_bar: None,
                header: None,
                leading_actions: None,
                trailing_actions: None,
                content_stack: None,
                requested_key: self.selected_key,
                close_requested: CallbackSlot::new(self.on_close_requested),
            }),
        );
        self.element.apply_to(ui, id);
        let header = ui.append(id, Panel::row(8.0));
        ui.set_layout(header, Layout::row(8.0).fill_width().align_self_stretch());
        let leading_actions = ui.append(header, Panel::row(4.0));
        let tab_bar = ui.append(
            header,
            self.tab_bar
                .on_selection_changed(move |ui, event| {
                    sync_selected_page(ui, id, event.selected_key);
                })
                .on_close_requested(move |ui, event| {
                    dispatch_tab_view_close_request(ui, id, event);
                }),
        );
        ui.set_layout(tab_bar, Layout::row(2.0).fill_width().flex_grow(1.0));
        let trailing_actions = ui.append(header, Panel::row(4.0));
        let content_stack = ui.append(id, IndexedStack::new());
        ui.set_layout(
            content_stack,
            Layout::new()
                .fill_width()
                .flex_grow(1.0)
                .align_self_stretch(),
        );
        ui.with_widget_mut(id, |widget, _| {
            let widget = widget.downcast_mut::<TabViewWidget>().unwrap();
            widget.tab_bar = Some(tab_bar);
            widget.header = Some(header);
            widget.leading_actions = Some(leading_actions);
            widget.trailing_actions = Some(trailing_actions);
            widget.content_stack = Some(content_stack);
        });
        id
    }
}

impl ElementSpec for TabView {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

/// Builder for one keyed page authored inside a [`TabView`].
#[derive(Debug)]
pub struct TabPage {
    key: TabKey,
    label: String,
    enabled: bool,
    closable: bool,
    element: ElementConfig,
}

impl TabPage {
    /// Creates a keyed tab page.
    #[must_use]
    pub fn new(key: impl Into<TabKey>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            enabled: true,
            closable: false,
            element: ElementConfig::default(),
        }
    }
    /// Sets whether the page can be selected through its header.
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    /// Disables the page header.
    #[must_use]
    pub const fn disabled(self) -> Self {
        self.enabled(false)
    }
    /// Sets whether the page header presents a close control.
    #[must_use]
    pub const fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }
}

impl AppendSpec for TabPage {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget(
            parent,
            Box::new(TabPageWidget {
                key: self.key,
                label: self.label,
                enabled: self.enabled,
                closable: self.closable,
            }),
        );
        self.element.apply_to(ui, id);
        id
    }
}
impl ElementSpec for TabPage {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

/// Scoped authoring helpers for adding keyed pages to a [`TabView`].
pub trait TabViewChildUiExt {
    /// Adds an enabled keyed page and scopes `f` to its retained content.
    fn tab<R>(
        &mut self,
        key: impl Into<TabKey>,
        label: impl Into<String>,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> (ElementId, R);
    /// Adds a configured page and scopes `f` to its retained content.
    fn tab_with<R>(
        &mut self,
        page: TabPage,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> (ElementId, R);

    /// Authors controls before the scrolling tab strip.
    fn leading_actions<R>(&mut self, f: impl FnOnce(&mut ChildUi<'_>) -> R) -> R;

    /// Authors controls after the scrolling tab strip.
    fn trailing_actions<R>(&mut self, f: impl FnOnce(&mut ChildUi<'_>) -> R) -> R;
}

impl TabViewChildUiExt for ChildUi<'_> {
    fn tab<R>(
        &mut self,
        key: impl Into<TabKey>,
        label: impl Into<String>,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> (ElementId, R) {
        self.tab_with(TabPage::new(key, label), f)
    }
    fn tab_with<R>(
        &mut self,
        page: TabPage,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> (ElementId, R) {
        let tab_view = self.parent();
        let stack = tab_view_content_stack(self.ui(), tab_view)
            .expect("TabViewChildUiExt requires a TabView parent");
        let result = self.ui_mut().children(stack, page, f);
        sync_tab_structure(self.ui_mut(), tab_view);
        result
    }

    fn leading_actions<R>(&mut self, f: impl FnOnce(&mut ChildUi<'_>) -> R) -> R {
        let tab_view = self.parent();
        let host = tab_view_leading_actions(self.ui(), tab_view)
            .expect("TabViewChildUiExt requires a TabView parent");
        self.ui_mut().with_children(host, f)
    }

    fn trailing_actions<R>(&mut self, f: impl FnOnce(&mut ChildUi<'_>) -> R) -> R {
        let tab_view = self.parent();
        let host = tab_view_trailing_actions(self.ui(), tab_view)
            .expect("TabViewChildUiExt requires a TabView parent");
        self.ui_mut().with_children(host, f)
    }
}

struct TabViewWidget {
    tab_bar: Option<ElementId>,
    header: Option<ElementId>,
    leading_actions: Option<ElementId>,
    trailing_actions: Option<ElementId>,
    content_stack: Option<ElementId>,
    requested_key: Option<TabKey>,
    close_requested: CallbackSlot<TabViewCloseRequestedEvent>,
}
impl core::fmt::Debug for TabViewWidget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TabViewWidget")
            .field("tab_bar", &self.tab_bar)
            .field("content_stack", &self.content_stack)
            .field("requested_key", &self.requested_key)
            .finish_non_exhaustive()
    }
}
impl Widget for TabViewWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_TAB_VIEW)
    }
    fn accessibility_role(
        &self,
        _: &Element,
        _: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::GenericContainer)
    }
    fn arrange(&mut self, ctx: &mut ArrangeContext<'_>) -> ArrangeResult {
        let id = ctx.id();
        ArrangeResult::from_layout_changed(sync_tab_structure(ctx.ui_mut(), id))
    }
}

#[derive(Debug)]
struct TabPageWidget {
    key: TabKey,
    label: String,
    enabled: bool,
    closable: bool,
}
impl Widget for TabPageWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_TAB_PAGE)
    }
    fn accessibility_role(
        &self,
        _: &Element,
        _: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::TabPanel)
    }
}

fn sync_tab_structure(ui: &mut Ui, tab_view: ElementId) -> bool {
    let Some((bar, stack, requested)) = tab_view_widget(ui, tab_view)
        .and_then(|widget| Some((widget.tab_bar?, widget.content_stack?, widget.requested_key)))
    else {
        return false;
    };
    let pages = ui
        .get(stack)
        .map(|element| element.children().to_vec())
        .unwrap_or_default();
    let options: Vec<_> = pages
        .iter()
        .filter_map(|page| {
            tab_page_widget(ui, *page).map(|page| {
                TabOption::new(page.key, page.label.clone())
                    .enabled(page.enabled)
                    .closable(page.closable)
            })
        })
        .collect();
    let mut changed = false;
    if tab_bar_options(ui, bar) != Some(options.as_slice()) {
        changed |= replace_tab_options(ui, bar, options);
    }
    let target = requested
        .filter(|key| {
            pages.iter().any(|page| {
                tab_page_widget(ui, *page).is_some_and(|page| page.key == *key && page.enabled)
            })
        })
        .or_else(|| selected_tab_key(ui, bar));
    if let Some(key) = target {
        changed |= select_tab_key(ui, bar, Some(key));
    }
    changed | apply_selected_page(ui, tab_view, selected_tab_key(ui, bar), false)
}

fn sync_selected_page(ui: &mut Ui, tab_view: ElementId, key: Option<TabKey>) -> bool {
    apply_selected_page(ui, tab_view, key, true)
}

fn apply_selected_page(
    ui: &mut Ui,
    tab_view: ElementId,
    key: Option<TabKey>,
    update_request: bool,
) -> bool {
    let Some(stack) = tab_view_content_stack(ui, tab_view) else {
        return false;
    };
    let pages = ui
        .get(stack)
        .map(|element| element.children().to_vec())
        .unwrap_or_default();
    let index = key.and_then(|key| {
        pages
            .iter()
            .position(|page| tab_page_widget(ui, *page).is_some_and(|page| page.key == key))
    });
    let changed = set_indexed_stack_visible_index(ui, stack, index);
    if update_request {
        ui.with_widget_mut(tab_view, |widget, _| {
            widget
                .downcast_mut::<TabViewWidget>()
                .unwrap()
                .requested_key = key;
        });
    }
    changed
}

fn tab_view_widget(ui: &Ui, id: ElementId) -> Option<&TabViewWidget> {
    ui.get(id)?.widget()?.downcast_ref()
}
fn tab_page_widget(ui: &Ui, id: ElementId) -> Option<&TabPageWidget> {
    ui.get(id)?.widget()?.downcast_ref()
}
/// Returns a tab view's internal tab bar.
#[must_use]
pub fn tab_view_tab_bar(ui: &Ui, id: ElementId) -> Option<ElementId> {
    tab_view_widget(ui, id)?.tab_bar
}
/// Returns the host for controls before the tab strip.
#[must_use]
pub fn tab_view_leading_actions(ui: &Ui, id: ElementId) -> Option<ElementId> {
    tab_view_widget(ui, id)?.leading_actions
}
/// Returns the host for controls after the tab strip.
#[must_use]
pub fn tab_view_trailing_actions(ui: &Ui, id: ElementId) -> Option<ElementId> {
    tab_view_widget(ui, id)?.trailing_actions
}
/// Returns a tab view's retained page stack.
#[must_use]
pub fn tab_view_content_stack(ui: &Ui, id: ElementId) -> Option<ElementId> {
    tab_view_widget(ui, id)?.content_stack
}
/// Returns the selected page key.
#[must_use]
pub fn selected_tab_view_key(ui: &Ui, id: ElementId) -> Option<TabKey> {
    selected_tab_key(ui, tab_view_tab_bar(ui, id)?)
}
/// Selects a page by stable key.
pub fn select_tab_view_key(ui: &mut Ui, id: ElementId, key: TabKey) -> bool {
    let Some(bar) = tab_view_tab_bar(ui, id) else {
        return false;
    };
    select_tab_key(ui, bar, Some(key))
}

/// Removes the retained page with `key` and synchronizes tab metadata.
///
/// When the selected page is removed, selection moves to the preceding
/// enabled page in logical tab order, or to the following enabled page when
/// no preceding page exists.
pub fn remove_tab_view_page(ui: &mut Ui, id: ElementId, key: TabKey) -> bool {
    let Some(stack) = tab_view_content_stack(ui, id) else {
        return false;
    };
    let pages = ui
        .get(stack)
        .map(|element| element.children().to_vec())
        .unwrap_or_default();
    let Some(index) = pages
        .iter()
        .position(|page| tab_page_widget(ui, *page).is_some_and(|page| page.key == key))
    else {
        return false;
    };
    if selected_tab_view_key(ui, id) == Some(key) {
        let replacement = pages[..index]
            .iter()
            .rev()
            .chain(pages[index + 1..].iter())
            .find_map(|page| {
                tab_page_widget(ui, *page)
                    .filter(|page| page.enabled)
                    .map(|page| page.key)
            });
        if let Some(replacement) = replacement {
            select_tab_view_key(ui, id, replacement);
        }
    }
    ui.remove(pages[index]);
    sync_tab_structure(ui, id);
    true
}

fn dispatch_tab_view_close_request(
    ui: &mut Ui,
    tab_view: ElementId,
    event: TabCloseRequestedEvent,
) {
    let Some(stack) = tab_view_content_stack(ui, tab_view) else {
        return;
    };
    let page = ui.get(stack).and_then(|element| {
        element
            .children()
            .iter()
            .copied()
            .find(|page| tab_page_widget(ui, *page).is_some_and(|page| page.key == event.key))
    });
    let Some(page) = page else {
        return;
    };
    let Some(close_requested) =
        tab_view_widget(ui, tab_view).map(|widget| widget.close_requested.clone())
    else {
        return;
    };
    close_requested.dispatch(
        ui,
        tab_view,
        TabViewCloseRequestedEvent {
            tab_view,
            key: event.key,
            page,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Badge, Button, Panel, Size, SurfaceSpecExt, Text, Visibility};
    #[test]
    fn keyed_selection_switches_retained_pages() {
        let mut ui = Ui::new();
        let root = ui.root();
        let view = ui.append(root, TabView::new().selected_key(2));
        ui.with_children(view, |tabs| {
            tabs.tab(1, "One", |page| {
                page.add(Panel::new());
            });
            tabs.tab(2, "Two", |page| {
                page.add(Panel::new());
            });
        });
        ui.compute_layout(Size::new(400.0, 240.0));
        let stack = tab_view_content_stack(&ui, view).unwrap();
        let pages = ui.get(stack).unwrap().children().to_vec();
        assert_eq!(selected_tab_view_key(&ui, view), Some(TabKey::new(2)));
        assert_eq!(ui.visibility(pages[0]), Some(Visibility::Collapsed));
        assert_eq!(ui.visibility(pages[1]), Some(Visibility::Visible));
        assert!(select_tab_view_key(&mut ui, view, TabKey::new(1)));
        assert!(ui.get(pages[1]).is_some());
    }

    #[test]
    fn action_regions_are_outside_the_tab_list() {
        let mut ui = Ui::new();
        let root = ui.root();
        let view = ui.append(root, TabView::new());
        ui.with_children(view, |tabs| {
            tabs.leading_actions(|actions| actions.add(Panel::new()));
            tabs.trailing_actions(|actions| actions.add(Panel::new()));
            tabs.tab(1, "One", |_| {});
        });
        let leading = tab_view_leading_actions(&ui, view).unwrap();
        let trailing = tab_view_trailing_actions(&ui, view).unwrap();
        let bar = tab_view_tab_bar(&ui, view).unwrap();
        assert_eq!(ui.get(leading).unwrap().children().len(), 1);
        assert_eq!(ui.get(trailing).unwrap().children().len(), 1);
        assert_eq!(ui.get(bar).unwrap().children().len(), 1);
    }

    #[test]
    fn removing_a_page_preserves_a_valid_selection() {
        let mut ui = Ui::new();
        let root = ui.root();
        let view = ui.append(root, TabView::new().selected_key(3));
        ui.with_children(view, |tabs| {
            tabs.tab(1, "One", |_| {});
            tabs.tab_with(TabPage::new(2, "Two").closable(true), |_| {});
            tabs.tab_with(TabPage::new(3, "Three").closable(true), |_| {});
        });
        assert!(remove_tab_view_page(&mut ui, view, TabKey::new(3)));
        assert_eq!(selected_tab_view_key(&ui, view), Some(TabKey::new(2)));
        assert!(!remove_tab_view_page(&mut ui, view, TabKey::new(3)));
    }

    #[test]
    fn close_callback_can_remove_its_retained_page() {
        let mut ui = Ui::new();
        let root = ui.root();
        let view = ui.append(
            root,
            TabView::new().on_close_requested(|ui, event| {
                remove_tab_view_page(ui, event.tab_view, event.key);
            }),
        );
        ui.with_children(view, |tabs| {
            tabs.tab(1, "One", |_| {});
            tabs.tab_with(TabPage::new(2, "Two").closable(true), |_| {});
        });
        let bar = tab_view_tab_bar(&ui, view).unwrap();
        let tab = ui.get(bar).unwrap().children()[1];
        let close = *ui.get(tab).unwrap().children().last().unwrap();
        let glyph = ui.get(close).unwrap().children()[0];

        ui.synthesize_click(glyph, kurbo::Point::ZERO);

        let stack = tab_view_content_stack(&ui, view).unwrap();
        assert_eq!(ui.get(stack).unwrap().children().len(), 1);
        assert_eq!(ui.get(bar).unwrap().children().len(), 1);
    }

    #[test]
    fn custom_header_and_close_control_share_one_centered_row() {
        let mut ui = Ui::new();
        let root = ui.root();
        let view = ui.append(
            root,
            TabView::new()
                .selected_key(2)
                .item_template(|header, item| {
                    header.with_child(Panel::row(6.0).align_center(), |content| {
                        content.add(Text::new(item.option().label()));
                        content.add(Badge::count(3));
                    });
                }),
        );
        ui.with_children(view, |tabs| {
            tabs.tab(1, "One", |_| {});
            tabs.tab_with(TabPage::new(2, "Two").closable(true), |_| {});
            tabs.trailing_actions(|actions| {
                actions.add(Button::new("+").font_size(14.0).padding_xy(12.0, 8.0));
            });
        });
        ui.compute_layout(Size::new(500.0, 200.0));

        let bar = tab_view_tab_bar(&ui, view).unwrap();
        let items = ui.get(bar).unwrap().children();
        let selected = items[1];
        let children = ui.get(selected).unwrap().children();
        let content_center = ui.effective_bounds(children[0]).center().y;
        let close_center = ui.effective_bounds(children[1]).center().y;
        assert!(
            (content_center - close_center).abs() <= 0.5,
            "content center {content_center}, close center {close_center}"
        );
        assert_eq!(
            ui.effective_bounds(items[0]).height(),
            ui.effective_bounds(items[1]).height()
        );
    }
}
