// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::{boxed::Box, rc::Rc};
use core::cell::RefCell;

use kurbo::Point;

use super::Menu;
use crate::element::ElementId;
use crate::ui::Ui;
use crate::widget::{AppendSpec, ContentSlotSpec, ElementConfig, ElementSpec};

/// Input path that requested an attached context menu.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextMenuSource {
    /// A secondary pointer press requested the menu.
    Pointer,
    /// The context-menu key or Shift+F10 requested the menu.
    Keyboard,
    /// An assistive-technology action requested the menu.
    Accessibility,
}

/// Context supplied when an attached context menu is constructed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContextMenuRequest {
    /// Element that owns the context-menu attachment and overlay lifetime.
    pub owner: ElementId,
    /// Retained element on which invocation began.
    ///
    /// For pointer input this is the physical hit element. For keyboard input
    /// it is the focused element. Accessibility requests originate on
    /// `owner` itself.
    pub invoked_on: ElementId,
    /// Scene-space point used to place the root menu.
    pub position: Point,
    /// Input path that requested the menu.
    pub source: ContextMenuSource,
}

type ContextMenuFactory = Box<dyn FnMut(&mut Ui, ContextMenuRequest) -> Option<Menu> + 'static>;

/// Reusable context-menu attachment.
///
/// A context menu owns only model construction policy. Realized menu surfaces,
/// nested submenu behavior, focus containment, and command dispatch are shared
/// with [`crate::MenuButton`].
#[derive(Clone)]
pub struct ContextMenu {
    factory: Rc<RefCell<Option<ContextMenuFactory>>>,
}

impl core::fmt::Debug for ContextMenu {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ContextMenu").finish_non_exhaustive()
    }
}

impl ContextMenu {
    /// Creates an attachment that clones `menu` for each invocation.
    #[must_use]
    pub fn new(menu: Menu) -> Self {
        Self::dynamic(move |_ui, _request| Some(menu.clone()))
    }

    /// Creates an attachment whose current menu is built at invocation time.
    ///
    /// Returning `None` declines this attachment and lets an attached retained
    /// ancestor answer the same request. The factory may inspect or mutate
    /// retained state before returning a model.
    #[must_use]
    pub fn dynamic<F>(factory: F) -> Self
    where
        F: FnMut(&mut Ui, ContextMenuRequest) -> Option<Menu> + 'static,
    {
        Self {
            factory: Rc::new(RefCell::new(Some(Box::new(factory)))),
        }
    }

    /// Attaches this context menu to `target` and returns `target` unchanged.
    pub fn attach_to(self, ui: &mut Ui, target: ElementId) -> ElementId {
        ui.set_context_menu(target, self);
        target
    }

    pub(crate) fn build(&self, ui: &mut Ui, request: ContextMenuRequest) -> Option<Menu> {
        let mut factory = self.factory.borrow_mut().take()?;
        let menu = factory(ui, request);
        if self.factory.borrow().is_none() {
            *self.factory.borrow_mut() = Some(factory);
        }
        menu
    }
}

/// Append wrapper that attaches a context menu to the inserted element.
pub struct ContextMenuSpec<S> {
    spec: S,
    context_menu: ContextMenu,
}

impl<S> core::fmt::Debug for ContextMenuSpec<S>
where
    S: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ContextMenuSpec")
            .field("spec", &self.spec)
            .field("context_menu", &self.context_menu)
            .finish()
    }
}

impl<S> ContextMenuSpec<S> {
    /// Creates a wrapper that attaches `context_menu` after `spec` is appended.
    #[must_use]
    pub fn new(spec: S, context_menu: ContextMenu) -> Self {
        Self { spec, context_menu }
    }
}

impl<S> AppendSpec for ContextMenuSpec<S>
where
    S: AppendSpec,
{
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = self.spec.append_to(ui, parent);
        ui.set_context_menu(id, self.context_menu);
        id
    }
}

impl<S> ElementSpec for ContextMenuSpec<S>
where
    S: ElementSpec,
{
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        self.spec.element_config_mut()
    }
}

impl<S> ContentSlotSpec for ContextMenuSpec<S>
where
    S: ContentSlotSpec,
{
    const CONTENT_SLOT: crate::TemplateSlot = S::CONTENT_SLOT;
}

/// Extension trait for attaching a context menu at a retained spec call-site.
pub trait ContextMenuSpecExt: AppendSpec + Sized {
    /// Attaches a reusable static menu to the inserted element.
    #[must_use]
    fn context_menu(self, menu: Menu) -> ContextMenuSpec<Self> {
        self.with_context_menu(ContextMenu::new(menu))
    }

    /// Attaches a fully configured context-menu factory.
    #[must_use]
    fn with_context_menu(self, context_menu: ContextMenu) -> ContextMenuSpec<Self> {
        ContextMenuSpec::new(self, context_menu)
    }
}

impl<S> ContextMenuSpecExt for S where S: AppendSpec {}
