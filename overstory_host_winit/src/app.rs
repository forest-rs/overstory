// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `winit` application-handler glue.

use accesskit_winit::Event as AccessKitEvent;
use frameclock::FrameDemand;
use overstory::Ui;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::WindowId;

use crate::WindowOptions;
use crate::state::{WindowState, init_window};

#[derive(Debug)]
pub(crate) enum HostEvent {
    AccessKit(AccessKitEvent),
}

impl From<AccessKitEvent> for HostEvent {
    fn from(event: AccessKitEvent) -> Self {
        Self::AccessKit(event)
    }
}

pub(crate) struct App<F>
where
    F: FnOnce() -> Ui + 'static,
{
    options: WindowOptions,
    build_ui: Option<F>,
    accesskit_proxy: EventLoopProxy<HostEvent>,
    state: Option<WindowState>,
}

impl<F> App<F>
where
    F: FnOnce() -> Ui + 'static,
{
    pub(crate) fn new(
        options: WindowOptions,
        build_ui: F,
        accesskit_proxy: EventLoopProxy<HostEvent>,
    ) -> Self {
        Self {
            options,
            build_ui: Some(build_ui),
            accesskit_proxy,
            state: None,
        }
    }
}

impl<F> ApplicationHandler<HostEvent> for App<F>
where
    F: FnOnce() -> Ui + 'static,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let Some(build_ui) = self.build_ui.take() else {
            event_loop.exit();
            return;
        };

        match init_window(
            event_loop,
            &self.options,
            build_ui,
            self.accesskit_proxy.clone(),
        ) {
            Ok(mut state) => {
                state.request_frame(FrameDemand::ANIMATION);
                self.state = Some(state);
            }
            Err(err) => {
                eprintln!("overstory_host_winit: failed to initialize window: {err}");
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        state.handle_window_event(event_loop, event);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: HostEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        match event {
            HostEvent::AccessKit(event) => {
                state.handle_accesskit_event(event);
            }
        }
        state.arm_next_wakeup(event_loop);
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: winit::event::StartCause) {
        if let Some(state) = self.state.as_mut() {
            state.handle_start_cause(event_loop, cause);
        }
    }
}
