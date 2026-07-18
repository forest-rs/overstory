// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Fixed-extent virtualization adapter for built-in collection controls.
//!
//! This module owns synchronization between shared scroll state and
//! `understory_virtual_list`. It explicitly does not own realized elements,
//! collection data, or selection policy.

use core::ops::Range;

use kurbo::{Size, Vec2};
use understory_property::DependencyObjectExt;
use understory_virtual_list::{FixedExtentModel, ScrollAlign, VirtualList};

use crate::channels::LAYOUT;
use crate::controls::support::action_scroll_unit;
use crate::element::ElementId;
use crate::layouts::{LayoutPlacement, LayoutSize};
use crate::scroll_runtime::{ScrollCommandResult, ScrollInputSource, ScrollState};
use crate::ui::Ui;

pub(crate) struct FixedVirtualRowsSync {
    pub(crate) viewport: Size,
    pub(crate) range: Range<usize>,
    pub(crate) changed: bool,
}

/// Keeps one fixed-extent virtualizer coherent with shared scroll state.
pub(crate) struct FixedVirtualViewport {
    virtualizer: VirtualList<FixedExtentModel<f64>>,
    state: ScrollState,
    last_viewport: Size,
}

impl FixedVirtualViewport {
    pub(crate) fn new(len: usize, item_extent: f64, scroll_offset: f64, overscan: f64) -> Self {
        let state = ScrollState::with_offset_y(scroll_offset);
        let mut virtualizer =
            VirtualList::new(FixedExtentModel::new(len, item_extent), 0.0, overscan);
        virtualizer.set_scroll_offset(state.offset_y());
        Self {
            virtualizer,
            state,
            last_viewport: Size::ZERO,
        }
    }

    pub(crate) fn state(&self) -> &ScrollState {
        &self.state
    }

    pub(crate) fn offset_y(&self) -> f64 {
        self.state.offset_y()
    }

    pub(crate) fn item_extent(&self) -> f64 {
        self.virtualizer.model().extent()
    }

    pub(crate) fn sync(&mut self, owner: ElementId, ui: &Ui, len: usize) -> FixedVirtualRowsSync {
        let bounds = ui.effective_bounds(owner);
        let viewport = Size::new(bounds.width().max(0.0), bounds.height().max(0.0));
        let viewport_changed = viewport != self.last_viewport;
        self.last_viewport = viewport;

        let len_changed = self.virtualizer.len() != len;
        if len_changed {
            self.virtualizer.set_len(len);
        }
        self.virtualizer.set_viewport_extent(viewport.height);
        let previous_offset = self.state.offset_y();
        self.refresh_metrics();

        FixedVirtualRowsSync {
            viewport,
            range: self.virtualizer.materialized_range(),
            changed: viewport_changed || len_changed || previous_offset != self.state.offset_y(),
        }
    }

    pub(crate) fn layout_rows(
        &mut self,
        ui: &mut Ui,
        rows: impl IntoIterator<Item = (usize, ElementId)>,
        viewport: Size,
    ) -> bool {
        let mut changed = false;
        let props = *ui.properties();
        let item_extent = self.item_extent();
        for (index, element) in rows {
            let size = LayoutSize::logical(viewport.width, item_extent);
            let placement = LayoutPlacement::absolute_at(0.0, self.virtualizer.offset_of(index));
            if ui
                .get(element)
                .and_then(|element| element.get_local_value(props.layout_size))
                != Some(&size)
            {
                ui.set_local(element, props.layout_size, size);
                changed = true;
            }
            if ui
                .get(element)
                .and_then(|element| element.get_local_value(props.layout_placement))
                != Some(&placement)
            {
                ui.set_local(element, props.layout_placement, placement);
                changed = true;
            }
        }
        changed
    }

    pub(crate) fn scroll_to(
        &mut self,
        owner: ElementId,
        ui: &mut Ui,
        offset: f64,
        source: ScrollInputSource,
    ) -> ScrollCommandResult {
        self.refresh_metrics();
        let change = ui
            .apply_scroll_command(owner, Vec2::new(self.state.offset().x, offset), source)
            .expect("fixed virtual scroll commands require a registered target");
        self.virtualizer.set_scroll_offset(self.state.offset_y());
        change
    }

    pub(crate) fn scroll_by(
        &mut self,
        owner: ElementId,
        ui: &mut Ui,
        delta: f64,
        source: ScrollInputSource,
    ) -> ScrollCommandResult {
        self.scroll_to(owner, ui, self.state.offset_y() + delta, source)
    }

    pub(crate) fn scroll_to_index(
        &mut self,
        owner: ElementId,
        ui: &mut Ui,
        index: usize,
        source: ScrollInputSource,
    ) -> Option<ScrollCommandResult> {
        if index >= self.virtualizer.len() {
            return None;
        }
        let offset = self
            .virtualizer
            .target_offset_for_index(index, ScrollAlign::Nearest)?;
        Some(self.scroll_to(owner, ui, offset, source))
    }

    pub(crate) fn handle_accessibility_action(
        &mut self,
        owner: ElementId,
        ui: &mut Ui,
        request: &accesskit::ActionRequest,
    ) -> bool {
        let item_extent = self.item_extent();
        let direction = match request.action {
            accesskit::Action::ScrollUp => -1.0,
            accesskit::Action::ScrollDown => 1.0,
            accesskit::Action::SetScrollOffset => {
                let Some(accesskit::ActionData::SetScrollOffset(point)) = request.data.as_ref()
                else {
                    return false;
                };
                return self
                    .scroll_to(owner, ui, point.y, ScrollInputSource::Accessibility)
                    .changed();
            }
            _ => return false,
        };
        let Some(unit) = action_scroll_unit(request) else {
            return false;
        };
        let amount = match unit {
            accesskit::ScrollUnit::Item => item_extent,
            accesskit::ScrollUnit::Page => self.virtualizer.viewport_extent().max(item_extent),
        };
        self.scroll_by(
            owner,
            ui,
            amount * direction,
            ScrollInputSource::Accessibility,
        )
        .changed()
    }

    pub(crate) fn set_len(&mut self, owner: ElementId, ui: &mut Ui, len: usize) {
        self.virtualizer.set_len(len);
        self.refresh_metrics();
        ui.mark_channels(owner, LAYOUT.into());
    }

    pub(crate) fn visible_item_count(&self) -> usize {
        let item_extent = self.item_extent();
        let viewport_extent = self.virtualizer.viewport_extent();
        if item_extent <= f64::EPSILON || viewport_extent <= 0.0 {
            return 1;
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "positive viewport/item ratio is clamped to at least one visible item"
        )]
        {
            (viewport_extent / item_extent).floor().max(1.0) as usize
        }
    }

    fn refresh_metrics(&mut self) {
        let content_height = self.virtualizer.total_extent();
        self.state.set_metrics(
            self.last_viewport,
            Size::new(self.last_viewport.width, content_height),
        );
        self.virtualizer.set_scroll_offset(self.state.offset_y());
    }
}
