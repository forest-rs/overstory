// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::boxed::Box;

use kurbo::{Axis, Vec2};
use understory_style::TypeTag;

use crate::behavior::{ArrangeContext, ArrangeResult, WidgetEventContext};
use crate::element::{Element, ElementId};
use crate::event_runtime::ScrollEvent;
use crate::layouts::Layout;
use crate::scroll_runtime::{ScrollCommandResult, ScrollInputSource, ScrollState};
use crate::ui::{ControlState, Ui};
use crate::widget::{AppendSpec, ElementConfig, ElementSpec, Widget};

use super::events::{
    ScrollOffsetChangedEvent, ScrollOffsetChangedHandler, ScrollOffsetChangedSlot,
};
use super::ids::TYPE_SCROLL_VIEW;
use super::runtime::{
    apply_scroll_offset, scroll_by, scroll_by_accessibility_action,
    update_scroll_metrics_from_layout, wheel_scroll_delta,
};
/// Builder + spec for a clipped viewport with reusable scroll state.
pub struct ScrollView {
    size: Option<(f64, f64)>,
    state: ScrollState,
    on_offset_changed: Option<ScrollOffsetChangedHandler>,
    element: ElementConfig,
}

impl core::fmt::Debug for ScrollView {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ScrollView")
            .field("size", &self.size)
            .field("state", &self.state)
            .field(
                "has_offset_changed_handler",
                &self.on_offset_changed.is_some(),
            )
            .field("element", &self.element)
            .finish()
    }
}

impl ScrollView {
    /// Starts a new scroll view builder.
    #[must_use]
    pub fn new() -> Self {
        let mut element = ElementConfig::default();
        element.set_control_state(ControlState::interactive().focusable(false));
        Self {
            size: None,
            state: ScrollState::new(),
            on_offset_changed: None,
            element,
        }
    }

    /// Sets the viewport's fixed logical-pixel size.
    #[must_use]
    pub fn size(mut self, width: f64, height: f64) -> Self {
        self.size = Some((width, height));
        self
    }

    /// Sets the initial vertical scroll offset in logical pixels.
    #[must_use]
    pub fn scroll_offset_y(self, offset: f64) -> Self {
        self.state.set_offset_y_raw(offset);
        self
    }

    /// Registers a handler for scroll offset changes.
    ///
    /// The handler is notified for wheel/touchpad scrolling, accessibility
    /// scroll actions, programmatic offset writes, and bound scrollbar changes.
    #[must_use]
    pub fn on_offset_changed<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, ScrollOffsetChangedEvent) + 'static,
    {
        self.on_offset_changed = Some(Box::new(handler));
        self
    }

    pub(super) fn on_offset_changed_option(
        mut self,
        handler: Option<ScrollOffsetChangedHandler>,
    ) -> Self {
        self.on_offset_changed = handler;
        self
    }

    /// Sets whether the scroll view starts enabled.
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
}

impl Default for ScrollView {
    fn default() -> Self {
        Self::new()
    }
}

impl AppendSpec for ScrollView {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let state = self.state;
        let id = ui.append_widget(
            parent,
            Box::new(ScrollViewWidget {
                state: state.clone(),
                offset_changed: ScrollOffsetChangedSlot::new(self.on_offset_changed),
            }),
        );

        let mut layout = Layout::column(0.0);
        if let Some((width, height)) = self.size {
            layout = layout.logical_size(width, height);
        }
        ui.register_scroll_target(id, state);
        ui.set_layout(id, layout);
        self.element.apply_to(ui, id);
        id
    }
}

impl ElementSpec for ScrollView {
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

/// Per-element widget for `ScrollView`.
pub(crate) struct ScrollViewWidget {
    state: ScrollState,
    pub(super) offset_changed: ScrollOffsetChangedSlot,
}

impl core::fmt::Debug for ScrollViewWidget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ScrollViewWidget")
            .field("state", &self.state)
            .field(
                "has_offset_changed_handler",
                &self.offset_changed.has_handler(),
            )
            .finish()
    }
}

impl Widget for ScrollViewWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_SCROLL_VIEW)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::ScrollView)
    }

    fn configure_accessibility(
        &self,
        element: &Element,
        ctx: &crate::AccessibilityContext<'_>,
        node: &mut accesskit::Node,
    ) {
        let offset = self.state.offset();
        let max = self.state.max_offset();
        node.set_scroll_x(offset.x);
        node.set_scroll_x_min(0.0);
        node.set_scroll_x_max(max.x);
        node.set_scroll_y(offset.y);
        node.set_scroll_y_min(0.0);
        node.set_scroll_y_max(max.y);

        if !ctx.is_enabled(element.id()) {
            return;
        }
        if max.x > f64::EPSILON {
            node.add_action(accesskit::Action::ScrollLeft);
            node.add_action(accesskit::Action::ScrollRight);
        }
        if max.y > f64::EPSILON {
            node.add_action(accesskit::Action::ScrollUp);
            node.add_action(accesskit::Action::ScrollDown);
        }
        if max.x > f64::EPSILON || max.y > f64::EPSILON {
            node.add_action(accesskit::Action::SetScrollOffset);
        }
    }

    fn arrange(&mut self, ctx: &mut ArrangeContext<'_>) -> ArrangeResult {
        update_scroll_metrics_from_layout(ctx, &self.state);
        ArrangeResult::UNCHANGED
    }

    fn clips_subtree(&self, _element: &Element) -> bool {
        true
    }

    fn child_layout_offset(&self, _element: &Element) -> Vec2 {
        -self.state.offset()
    }

    fn handle_scroll(&mut self, ctx: &mut WidgetEventContext<'_>, event: ScrollEvent<'_>) {
        let viewport = ctx.id();
        let Some(delta) = wheel_scroll_delta(ctx, viewport, event) else {
            return;
        };
        let change = scroll_by(ctx, viewport, &self.state, delta, ScrollInputSource::Wheel);
        event.consume_logical_delta(delta, change.consumed_delta);
        if change.changed() {
            self.dispatch_offset_changed(ctx, viewport, change);
        }
    }

    fn handle_accessibility_action(
        &mut self,
        ctx: &mut WidgetEventContext<'_>,
        request: &accesskit::ActionRequest,
    ) -> bool {
        let viewport = ctx.id();
        if !ctx.is_enabled(viewport) {
            return false;
        }
        let change = match request.action {
            accesskit::Action::ScrollLeft => scroll_by_accessibility_action(
                ctx,
                viewport,
                &self.state,
                Axis::Horizontal,
                -1.0,
                request,
            ),
            accesskit::Action::ScrollRight => scroll_by_accessibility_action(
                ctx,
                viewport,
                &self.state,
                Axis::Horizontal,
                1.0,
                request,
            ),
            accesskit::Action::ScrollUp => scroll_by_accessibility_action(
                ctx,
                viewport,
                &self.state,
                Axis::Vertical,
                -1.0,
                request,
            ),
            accesskit::Action::ScrollDown => scroll_by_accessibility_action(
                ctx,
                viewport,
                &self.state,
                Axis::Vertical,
                1.0,
                request,
            ),
            accesskit::Action::SetScrollOffset => {
                let Some(accesskit::ActionData::SetScrollOffset(point)) = request.data.as_ref()
                else {
                    return false;
                };
                let change = apply_scroll_offset(
                    ctx,
                    viewport,
                    &self.state,
                    Vec2::new(point.x, point.y),
                    ScrollInputSource::Accessibility,
                );
                change.changed().then_some(change)
            }
            _ => return false,
        };
        if let Some(change) = change {
            self.dispatch_offset_changed(ctx, viewport, change);
            true
        } else {
            false
        }
    }
}

impl ScrollViewWidget {
    fn dispatch_offset_changed(
        &self,
        ui: &mut Ui,
        scroll_view: ElementId,
        change: ScrollCommandResult,
    ) {
        self.offset_changed.dispatch(
            ui,
            scroll_view,
            ScrollOffsetChangedEvent {
                scroll_view,
                previous_offset: change.previous_offset,
                offset: change.offset,
                source: change.source,
            },
        );
    }
}
