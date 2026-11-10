// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Reentrant event delivery for retained docking chrome.
//!
//! This module owns docking-view callback dispatch and suppression; it
//! explicitly does not own application state or semantic tiling operations.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use overstory::Ui;
use understory_tiling::PaneId;

type TabActivatedHandler = Box<dyn FnMut(&mut Ui, PaneId)>;

/// Shared delivery state used by every retained tab bar in one dock surface.
#[derive(Default)]
pub(super) struct DockTabEvents {
    suppression_depth: Cell<usize>,
    dispatching: Cell<bool>,
    pending: RefCell<Vec<PaneId>>,
    activated: RefCell<Option<TabActivatedHandler>>,
}

impl core::fmt::Debug for DockTabEvents {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DockTabEvents")
            .field("suppression_depth", &self.suppression_depth.get())
            .field("dispatching", &self.dispatching.get())
            .field("pending", &self.pending.borrow().len())
            .field("has_activated_handler", &self.activated.borrow().is_some())
            .finish()
    }
}

impl DockTabEvents {
    pub(super) fn set_activated_handler<F>(&self, handler: F)
    where
        F: FnMut(&mut Ui, PaneId) + 'static,
    {
        *self.activated.borrow_mut() = Some(Box::new(handler));
    }

    pub(super) fn suppress(&self) -> SuppressionGuard<'_> {
        self.suppression_depth
            .set(self.suppression_depth.get().saturating_add(1));
        SuppressionGuard { events: self }
    }

    pub(super) fn dispatch_activated(&self, ui: &mut Ui, pane: PaneId) {
        if self.suppression_depth.get() != 0 {
            return;
        }
        self.pending.borrow_mut().push(pane);
        if self.dispatching.replace(true) {
            return;
        }
        let _guard = DispatchGuard { events: self };
        while let Some(pane) = self.take_pending() {
            let Some(mut handler) = self.activated.borrow_mut().take() else {
                self.pending.borrow_mut().clear();
                break;
            };
            handler(ui, pane);
            if self.activated.borrow().is_none() {
                *self.activated.borrow_mut() = Some(handler);
            }
        }
    }

    fn take_pending(&self) -> Option<PaneId> {
        let mut pending = self.pending.borrow_mut();
        (!pending.is_empty()).then(|| pending.remove(0))
    }
}

pub(super) struct SuppressionGuard<'a> {
    events: &'a DockTabEvents,
}

impl Drop for SuppressionGuard<'_> {
    fn drop(&mut self) {
        self.events
            .suppression_depth
            .set(self.events.suppression_depth.get().saturating_sub(1));
    }
}

struct DispatchGuard<'a> {
    events: &'a DockTabEvents,
}

impl Drop for DispatchGuard<'_> {
    fn drop(&mut self) {
        self.events.dispatching.set(false);
    }
}

#[cfg(test)]
mod tests {
    use alloc::rc::Rc;
    use alloc::vec::Vec;
    use core::cell::RefCell;

    use overstory::{TextResources, Ui};
    use understory_tiling::PaneId;

    use super::DockTabEvents;

    #[test]
    fn nested_activation_is_queued_in_order() {
        let events = Rc::new(DockTabEvents::default());
        let observed = Rc::new(RefCell::new(Vec::new()));
        let handler_events = Rc::clone(&events);
        let handler_observed = Rc::clone(&observed);
        events.set_activated_handler(move |ui, pane| {
            handler_observed.borrow_mut().push(pane);
            if pane == PaneId(1) {
                handler_events.dispatch_activated(ui, PaneId(2));
            }
        });
        let mut ui = Ui::with_text_resources(&TextResources::empty());

        events.dispatch_activated(&mut ui, PaneId(1));

        assert_eq!(&*observed.borrow(), &[PaneId(1), PaneId(2)]);
    }

    #[test]
    fn nested_suppression_restores_delivery() {
        let events = DockTabEvents::default();
        let observed = Rc::new(RefCell::new(Vec::new()));
        let handler_observed = Rc::clone(&observed);
        events.set_activated_handler(move |_, pane| handler_observed.borrow_mut().push(pane));
        let mut ui = Ui::with_text_resources(&TextResources::empty());

        {
            let _outer = events.suppress();
            {
                let _inner = events.suppress();
                events.dispatch_activated(&mut ui, PaneId(1));
            }
            events.dispatch_activated(&mut ui, PaneId(2));
        }
        events.dispatch_activated(&mut ui, PaneId(3));

        assert_eq!(&*observed.borrow(), &[PaneId(3)]);
    }
}
