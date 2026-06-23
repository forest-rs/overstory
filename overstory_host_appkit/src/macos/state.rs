// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained native window, renderer, display-link, and UI host state.

#![expect(
    unsafe_code,
    reason = "CAMetalLayer surface creation and NSWindow setup require Objective-C FFI"
)]

use alloc::boxed::Box;
use core::{
    cell::{Cell, RefCell},
    ffi::c_void,
    ptr,
};

use frameclock::scheduler::DegradationPolicy;
use frameclock::{
    DisplayTiming, Duration as FrameDuration, FrameDemand, FrameTick, HostTime, OutputId,
    SchedulerConfig,
};
use frameclock_apple::DisplayLink;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSBackingStoreType, NSEvent, NSWindow, NSWindowStyleMask};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::NSString;
use objc2_quartz_core::CAMetalLayer;
use overstory::Ui;
use overstory_host::{
    FrameViewport, HostDispatchResult, HostFrameSubmission, HostFrameTiming, UiHost,
};
use overstory_presenter_wgpu::{WgpuPresentResult, WgpuPresenter, WgpuPresenterDescriptor};
use ui_events::keyboard::KeyboardEvent;
use ui_events::pointer::PointerEvent;
use ui_events_appkit::AppKitInputResponder;

use super::accessibility::AccessKitState;
use super::cursor::set_cursor;
use super::input::install_input_responder;
use super::time::AppleTimeMap;
use super::window::{
    ContentView, SurfaceSize, WindowDelegate, backing_scale_factor, create_metal_sublayer,
    physical_extent,
};
use super::{AppKitHostError, HostResult, WindowOptions};

const DEFAULT_WINDOW_ORIGIN: (f64, f64) = (200.0, 200.0);
const FALLBACK_REFRESH_INTERVAL: FrameDuration = FrameDuration(16_666_667);

thread_local! {
    static WINDOW_STATE: Cell<*const RefCell<Option<WindowState>>> = const { Cell::new(ptr::null()) };
}

pub(crate) fn with_window_state_cell<R>(f: impl FnOnce(&RefCell<Option<WindowState>>) -> R) -> R {
    WINDOW_STATE.with(|slot| {
        let mut ptr = slot.get();
        if ptr.is_null() {
            ptr = Box::into_raw(Box::new(RefCell::new(None)));
            slot.set(ptr);
        }
        let cell = unsafe { &*ptr };
        f(cell)
    })
}

pub(crate) fn window_state_installed() -> bool {
    with_window_state_cell(|cell| cell.borrow().is_some())
}

pub(crate) fn install_window_state(state: WindowState) {
    with_window_state_cell(|cell| {
        *cell.borrow_mut() = Some(state);
    });
}

pub(crate) fn shutdown_window_state() {
    with_window_state_cell(|cell| {
        *cell.borrow_mut() = None;
    });
}

pub(crate) fn request_initial_frame_and_retain_window() -> Retained<NSWindow> {
    with_window_state_cell(|cell| {
        let mut state = cell.borrow_mut();
        let state = state
            .as_mut()
            .expect("window state should be installed before startup show");
        state.request_frame(FrameDemand::ANIMATION);
        state.window.clone()
    })
}

pub(crate) fn start_display_link() {
    with_window_state_cell(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return;
        };
        state.start_display_link();
    });
}

pub(crate) fn update_accessibility_focus(is_focused: bool) {
    with_window_state_cell(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return;
        };
        state.update_accessibility_focus(is_focused);
    });
}

pub(crate) fn resize_window() {
    with_window_state_cell(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return;
        };
        state.resize_from_content_view();
        state.request_frame(FrameDemand::CONTINUOUS_INPUT);
    });
}

pub(crate) fn handle_display_tick(tick: FrameTick) {
    with_window_state_cell(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return;
        };
        if let Err(err) = state.handle_display_tick(tick) {
            eprintln!("overstory_host_appkit: render error: {err}");
        }
    });
}

pub(crate) struct WindowState {
    accesskit: AccessKitState,
    presenter: WgpuPresenter,
    display_link: DisplayLink,
    display_link_started: bool,
    #[allow(
        dead_code,
        reason = "kept alive so NSWindow delegate callbacks remain installed"
    )]
    window_delegate: Retained<WindowDelegate>,
    #[allow(
        dead_code,
        reason = "kept alive so the AppKit responder chain forwards input"
    )]
    input_responder: Retained<AppKitInputResponder>,
    metal_layer: Retained<CAMetalLayer>,
    content_view: Retained<ContentView>,
    window: Retained<NSWindow>,
    host: UiHost,
    time_map: AppleTimeMap,
}

impl WindowState {
    pub(crate) fn dispatch_keyboard_event(&mut self, event: &KeyboardEvent) {
        let result = self.host.dispatch_key(event);
        self.finish_input_dispatch(result);
        self.sync_cursor();
    }

    pub(crate) fn dispatch_pointer_event(&mut self, event: &PointerEvent) {
        let result = self.host.dispatch_pointer_event(event);
        self.finish_input_dispatch(result);
        self.sync_cursor();
    }

    pub(crate) fn pointer_position(&self, event: &NSEvent) -> (f64, f64) {
        let point = self
            .content_view
            .convertPoint_fromView(event.locationInWindow(), None);
        (point.x, point.y)
    }

    pub(crate) fn scale_factor(&self) -> f64 {
        backing_scale_factor(&self.window)
    }

    pub(crate) const fn time_map(&self) -> AppleTimeMap {
        self.time_map
    }

    pub(crate) fn accesskit_tree_update(&self) -> accesskit::TreeUpdate {
        self.host.accesskit_tree_update(self.scale_factor())
    }

    pub(crate) fn dispatch_accesskit_action(&mut self, request: &accesskit::ActionRequest) {
        let result = self.host.handle_accesskit_action(request);
        if result.needs_redraw {
            self.request_frame(result.frame_demand);
        }
        self.sync_cursor();
    }

    fn start_display_link(&mut self) {
        if self.display_link_started {
            return;
        }
        self.display_link.start();
        self.display_link_started = true;
        self.sync_display_link_activity();
    }

    fn request_frame(&mut self, demand: FrameDemand) {
        self.host.request_frame(demand);
        self.sync_display_link_activity();
    }

    fn finish_input_dispatch(&mut self, result: HostDispatchResult) {
        if result.needs_redraw {
            self.request_frame(result.frame_demand);
        } else {
            // Input handlers can arm timers or presentation timelines without
            // changing an immediate visual state flag. Re-check native wakeups
            // after every dispatch so those deadlines are not stranded behind a
            // paused display link.
            self.sync_display_link_activity();
        }
    }

    fn sync_cursor(&mut self) {
        if let Some(next) = self.host.cursor_update() {
            set_cursor(next);
        }
    }

    fn sync_accessibility(&mut self) {
        let scale_factor = self.scale_factor();
        let host = &self.host;
        self.accesskit
            .update_if_active(|| host.accesskit_tree_update(scale_factor));
    }

    fn update_accessibility_focus(&mut self, is_focused: bool) {
        self.accesskit.update_view_focus_state(is_focused);
    }

    fn handle_display_tick(&mut self, tick: FrameTick) -> HostResult<()> {
        let timing = self.host_timing_from_tick(tick);
        self.host.pump_timers(timing.now.ticks());
        let result = self.render(timing);
        self.sync_display_link_activity();
        result
    }

    fn sync_display_link_activity(&self) {
        if self.display_link_started {
            self.display_link.set_paused(!self.has_pending_work());
        }
    }

    fn has_pending_work(&self) -> bool {
        !self.host.pending_frame_demand().is_empty()
            || self.host.pending_frame_start().is_some()
            || self.duration_until_next_wakeup().is_some()
    }

    fn resize_from_content_view(&mut self) {
        let size = self.view_surface_size();
        self.metal_layer
            .setBounds(CGRect::new(CGPoint::new(0.0, 0.0), size.bounds.size));
        self.metal_layer.setFrame(size.bounds);
        self.metal_layer.setContentsScale(size.scale_factor);
        self.metal_layer
            .setDrawableSize(CGSize::new(f64::from(size.width), f64::from(size.height)));

        self.presenter.resize(size.width, size.height);
    }

    fn view_surface_size(&self) -> SurfaceSize {
        let bounds = self.content_view.bounds();
        let scale_factor = backing_scale_factor(&self.window);
        SurfaceSize {
            bounds,
            scale_factor,
            width: physical_extent(bounds.size.width, scale_factor),
            height: physical_extent(bounds.size.height, scale_factor),
        }
    }

    fn size(&self) -> (u32, u32) {
        self.presenter.size()
    }

    fn host_timing_from_tick(&self, tick: FrameTick) -> HostFrameTiming {
        let tick = self.time_map.map_tick(tick);
        HostFrameTiming {
            now: tick.now,
            predicted_present: tick.predicted_present,
            refresh_interval: tick.refresh_interval,
            prev_actual_present: tick.prev_actual_present,
            hints: frameclock_apple::present_hints(&tick, FALLBACK_REFRESH_INTERVAL),
            display_timing: frameclock_apple::display_timing(
                &tick,
                DisplayTiming::fixed(FALLBACK_REFRESH_INTERVAL),
            ),
        }
    }

    fn submitted_at(&self) -> HostTime {
        self.time_map.map_host_time(frameclock_apple::now())
    }

    fn current_time(&self) -> HostTime {
        self.time_map.map_host_time(frameclock_apple::now())
    }

    fn duration_until_next_wakeup(&self) -> Option<std::time::Duration> {
        self.host
            .duration_until_next_wakeup_at(self.current_time().ticks())
    }

    fn render(&mut self, timing: HostFrameTiming) -> HostResult<()> {
        let (width, height) = self.size();
        if width == 0 || height == 0 {
            return Ok(());
        }

        let scale_factor = self.scale_factor();
        let viewport = FrameViewport::from_physical_size(width, height, scale_factor);
        let present = {
            let Some(prepared) = self.host.prepare_requested_frame(viewport, timing) else {
                return Ok(());
            };
            debug_assert!(
                !prepared.demand().is_empty(),
                "requested frames should carry non-empty planned demand"
            );
            self.presenter.present_scene(prepared.scene())
        };

        let present = match present {
            Ok(present) => present,
            Err(err) => {
                self.host.discard_prepared_frame();
                return Err(Box::new(err));
            }
        };
        self.sync_accessibility();
        let WgpuPresentResult::Presented(timings) = present else {
            self.host.discard_prepared_frame();
            return Ok(());
        };
        self.host.record_frame_submission(
            HostFrameSubmission::new(timings)
                .with_submitted_at(self.submitted_at())
                .with_deferred_present_feedback(),
        );
        Ok(())
    }
}

pub(crate) fn init_window<F>(
    mtm: MainThreadMarker,
    options: &WindowOptions,
    build_ui: F,
) -> HostResult<WindowState>
where
    F: FnOnce() -> Ui,
{
    let (width, height) = options.size();
    let style = NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Resizable
        | NSWindowStyleMask::Miniaturizable;
    let frame = CGRect::new(
        CGPoint::new(DEFAULT_WINDOW_ORIGIN.0, DEFAULT_WINDOW_ORIGIN.1),
        CGSize::new(f64::from(width), f64::from(height)),
    );

    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc::<NSWindow>(),
            frame,
            style,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    unsafe { window.setReleasedWhenClosed(false) };
    window.setTitle(&NSString::from_str(options.title()));

    let content_frame = CGRect::new(
        CGPoint::new(0.0, 0.0),
        CGSize::new(f64::from(width), f64::from(height)),
    );
    let content_view = ContentView::new(mtm, content_frame);
    window.setContentView(Some(&content_view));
    content_view.setWantsLayer(true);
    let root_layer = content_view
        .layer()
        .ok_or(AppKitHostError::MissingContentLayer)?;
    let initial_scale_factor = backing_scale_factor(&window);
    let initial_size = SurfaceSize {
        bounds: content_view.bounds(),
        scale_factor: initial_scale_factor,
        width: physical_extent(content_view.bounds().size.width, initial_scale_factor),
        height: physical_extent(content_view.bounds().size.height, initial_scale_factor),
    };
    let metal_layer = create_metal_sublayer(&root_layer, &initial_size);

    let instance = wgpu::Instance::default();
    let surface = unsafe {
        let ptr: *const CAMetalLayer = &*metal_layer;
        instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(
            ptr.cast_mut().cast::<c_void>(),
        ))?
    };

    let window_delegate = WindowDelegate::new(mtm);
    window.setDelegate(Some(ProtocolObject::from_ref(&*window_delegate)));
    let input_responder = install_input_responder(mtm, &window, &content_view);
    let accesskit = AccessKitState::new(&content_view);

    let presenter = WgpuPresenter::new(
        &instance,
        surface,
        initial_size.width,
        initial_size.height,
        WgpuPresenterDescriptor::new("overstory_host_appkit"),
    )?;
    let host = UiHost::with_scheduler_config(build_ui(), appkit_scheduler_config());
    let time_map = AppleTimeMap::new();
    let display_link = DisplayLink::new(handle_display_tick, OutputId(0), mtm);

    Ok(WindowState {
        accesskit,
        presenter,
        display_link,
        display_link_started: false,
        window_delegate,
        input_responder,
        metal_layer,
        content_view,
        window,
        host,
        time_map,
    })
}

fn appkit_scheduler_config() -> SchedulerConfig {
    let mut config = SchedulerConfig::predictive();
    // This host uses the display-link callback itself as the frame-start wake.
    // Keep planning fixed at one frame so `WaitUntil` does not require an
    // additional AppKit timer before the input/event adapter exists.
    // TODO(overstory_host_appkit): Add an AppKit wake/timer path for
    // `UiHost::pending_frame_start` so predictive scheduling can use a small
    // refresh-rate-agnostic margin instead of treating the next display-link
    // tick as the only possible frame-start wake.
    config.min_depth = 1;
    config.max_depth = 1;
    config.initial_depth = 1;
    config.degradation_policy = DegradationPolicy::Fixed;
    config.minimum_frame_start_margin = FALLBACK_REFRESH_INTERVAL;
    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appkit_scheduler_uses_display_link_tick_as_frame_start_wake() {
        let config = appkit_scheduler_config();

        assert_eq!(config.minimum_frame_start_margin, FALLBACK_REFRESH_INTERVAL);
        assert_eq!(config.min_depth, 1);
        assert_eq!(config.max_depth, 1);
        assert_eq!(config.initial_depth, 1);
        assert_eq!(config.degradation_policy, DegradationPolicy::Fixed);
    }
}
