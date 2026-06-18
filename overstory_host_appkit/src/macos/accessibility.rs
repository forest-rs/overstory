// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! AccessKit bridge for the native AppKit host view.

#![expect(
    unsafe_code,
    reason = "accesskit_macos subclasses the AppKit view through an Objective-C object pointer"
)]

use core::ffi::c_void;

use accesskit::{ActionHandler, ActionRequest, ActivationHandler, TreeUpdate};
use accesskit_macos::SubclassingAdapter;

use super::state::with_window_state_cell;
use super::window::ContentView;

pub(crate) struct AccessKitState {
    adapter: SubclassingAdapter,
}

impl AccessKitState {
    pub(crate) fn new(content_view: &ContentView) -> Self {
        let view = (content_view as *const ContentView)
            .cast_mut()
            .cast::<c_void>();
        let adapter =
            unsafe { SubclassingAdapter::new(view, AccessKitActivation, AccessKitActions) };
        Self { adapter }
    }

    pub(crate) fn update_if_active(&mut self, update_factory: impl FnOnce() -> TreeUpdate) {
        if let Some(events) = self.adapter.update_if_active(update_factory) {
            events.raise();
        }
    }

    pub(crate) fn update_view_focus_state(&mut self, is_focused: bool) {
        if let Some(events) = self.adapter.update_view_focus_state(is_focused) {
            events.raise();
        }
    }
}

struct AccessKitActivation;

impl ActivationHandler for AccessKitActivation {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        with_window_state_cell(|cell| {
            cell.borrow()
                .as_ref()
                .map(|state| state.accesskit_tree_update())
        })
    }
}

struct AccessKitActions;

impl ActionHandler for AccessKitActions {
    fn do_action(&mut self, request: ActionRequest) {
        with_window_state_cell(|cell| {
            let mut state = cell.borrow_mut();
            let Some(state) = state.as_mut() else {
                return;
            };
            state.dispatch_accesskit_action(&request);
        });
    }
}
