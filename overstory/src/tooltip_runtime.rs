// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use hashbrown::HashMap;

use crate::controls::tooltip::Tooltip;
use crate::element::ElementId;

#[derive(Debug, Default)]
pub(crate) struct TooltipRuntime {
    entries: HashMap<ElementId, Tooltip>,
    active: Option<ActiveTooltip>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ActiveTooltip {
    pub(crate) target: ElementId,
    pub(crate) element: ElementId,
}

impl TooltipRuntime {
    pub(crate) fn set(&mut self, target: ElementId, tooltip: Tooltip) {
        self.entries.insert(target, tooltip);
    }

    pub(crate) fn tooltip_for(&self, target: ElementId) -> Option<&Tooltip> {
        self.entries.get(&target)
    }

    pub(crate) fn has_tooltip(&self, target: ElementId) -> bool {
        self.entries.contains_key(&target)
    }

    pub(crate) fn active(&self) -> Option<ActiveTooltip> {
        self.active
    }

    pub(crate) fn set_active(&mut self, target: ElementId, element: ElementId) {
        self.active = Some(ActiveTooltip { target, element });
    }

    pub(crate) fn take_active(&mut self) -> Option<ActiveTooltip> {
        self.active.take()
    }

    pub(crate) fn remove_target(&mut self, id: ElementId) -> Option<ElementId> {
        self.entries.remove(&id);
        let active = self.active?;
        if active.target == id || active.element == id {
            self.active = None;
            Some(active.element)
        } else {
            None
        }
    }
}
