// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Keyed tab headers composed with retained page content.

use alloc::{boxed::Box, string::String, vec::Vec};
use understory_style::TypeTag;

use crate::behavior::{ArrangeContext, ArrangeResult};
use crate::controls::indexed_stack::{IndexedStack, set_indexed_stack_visible_index};
use crate::controls::tab_bar::{
    TabBar, TabItemContext, TabKey, TabOption, replace_tab_options, select_tab_key,
    selected_tab_key, tab_options,
};
use crate::element::{Element, ElementId};
use crate::layouts::Layout;
use crate::ui::{ChildUi, Ui};
use crate::widget::{AppendSpec, ElementConfig, ElementSpec, Widget};

/// Selector identity for [`TabView`].
pub const TYPE_TAB_VIEW: TypeTag = TypeTag(37);
/// Selector identity for a retained tab page.
pub const TYPE_TAB_PAGE: TypeTag = TypeTag(38);

/// Builder for keyed tabs with retained page content.
pub struct TabView {
    selected_key: Option<TabKey>,
    tab_bar: TabBar,
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
                content_stack: None,
                requested_key: self.selected_key,
            }),
        );
        self.element.apply_to(ui, id);
        let tab_bar = ui.append(
            id,
            self.tab_bar.on_selection_changed(move |ui, event| {
                sync_selected_page(ui, id, event.selected_key);
            }),
        );
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
}

impl AppendSpec for TabPage {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget(
            parent,
            Box::new(TabPageWidget {
                key: self.key,
                label: self.label,
                enabled: self.enabled,
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
}

#[derive(Debug)]
struct TabViewWidget {
    tab_bar: Option<ElementId>,
    content_stack: Option<ElementId>,
    requested_key: Option<TabKey>,
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
            tab_page_widget(ui, *page)
                .map(|page| TabOption::new(page.key, page.label.clone()).enabled(page.enabled))
        })
        .collect();
    let mut changed = false;
    if tab_options(ui, bar) != Some(options.as_slice()) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Panel, Size, Visibility};
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
}
