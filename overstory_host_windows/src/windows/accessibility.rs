// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! AccessKit bridge for the native Win32 host window.

#![expect(unsafe_code, reason = "Win32 FFI")]

use accesskit::{ActionHandler, ActionRequest, ActivationHandler, TreeUpdate};

pub(crate) struct AccessKitActivation(pub TreeUpdate);

impl ActivationHandler for AccessKitActivation {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        Some(self.0.clone())
    }
}

pub(crate) struct AccessKitActions(pub accesskit_windows::HWND);

unsafe impl Send for AccessKitActions {}

impl ActionHandler for AccessKitActions {
    fn do_action(&mut self, request: ActionRequest) {
        // SAFETY: `self.0` is the window this adapter was created for.
        if let Some(mut state) = unsafe { super::WindowState::fetch(self.0) } {
            state.dispatch_accesskit_action(&request);
        }
    }
}
