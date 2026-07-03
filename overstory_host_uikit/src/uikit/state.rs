// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained native view, renderer, display-link, and UI host state.

#![expect(
    unsafe_code,
    reason = "CAMetalLayer surface creation and UIKit view setup require Objective-C FFI"
)]

use alloc::boxed::Box;
use core::{
    cell::{Cell, RefCell},
    ffi::c_void,
    ptr,
};

use frameclock::scheduler::DegradationPolicy;
use frameclock::{
    DisplayTiming, Duration as FrameDuration, FrameDemand, FrameOpportunity, FrameTick, HostTime,
    OutputId, SchedulerConfig,
};
use frameclock_apple::DisplayLink;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_ui_kit::UIViewController;
use overstory::Ui;
use overstory_host::{FrameViewport, HostDispatchResult, UiHost};
use overstory_presenter_wgpu::{WgpuPresentResult, WgpuPresenter, WgpuPresenterDescriptor};
use ui_events::keyboard::KeyboardEvent;
use ui_events::pointer::PointerEvent;
use ui_events_uikit::UIKitInputResponder;

use super::input::create_input_responder;
use super::time::AppleTimeMap;
use super::view::{ContentView, SurfaceSize, create_metal_sublayer, physical_extent, scale_factor};
use super::{HostResult, ViewOptions, frame_from_size};

const FALLBACK_REFRESH_INTERVAL: FrameDuration = FrameDuration(16_666_667);

thread_local! {
    static VIEW_STATE: Cell<*const RefCell<Option<ViewState>>> = const { Cell::new(ptr::null()) };
}

pub(crate) fn with_view_state_cell<R>(f: impl FnOnce(&RefCell<Option<ViewState>>) -> R) -> R {
    VIEW_STATE.with(|slot| {
        let mut ptr = slot.get();
        if ptr.is_null() {
            ptr = Box::into_raw(Box::new(RefCell::new(None)));
            slot.set(ptr);
        }
        let cell = unsafe { &*ptr };
        f(cell)
    })
}

pub(crate) fn view_state_installed() -> bool {
    with_view_state_cell(|cell| cell.borrow().is_some())
}

fn install_view_state(state: ViewState) {
    with_view_state_cell(|cell| {
        *cell.borrow_mut() = Some(state);
    });
}

pub(crate) fn start_display_link() {
    with_view_state_cell(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return;
        };
        state.start_display_link();
    });
}

pub(crate) fn resize_view() {
    with_view_state_cell(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return;
        };
        state.resize_from_content_view();
        state.request_frame(FrameDemand::CONTINUOUS_INPUT);
    });
}

fn request_initial_frame() {
    with_view_state_cell(|cell| {
        let mut state = cell.borrow_mut();
        let state = state
            .as_mut()
            .expect("view state should be installed before initial frame");
        state.request_frame(FrameDemand::ANIMATION);
    });
}

pub(crate) fn handle_display_tick(tick: FrameTick) {
    with_view_state_cell(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return;
        };
        if let Err(err) = state.handle_display_tick(tick) {
            eprintln!("overstory_host_uikit: render error: {err}");
        }
    });
}

pub(crate) struct ViewState {
    presenter: WgpuPresenter,
    display_link: DisplayLink,
    display_link_started: bool,
    #[allow(
        dead_code,
        reason = "kept alive so the UIKit responder remains available to the content view"
    )]
    input_responder: Retained<UIKitInputResponder>,
    metal_layer: Retained<objc2_quartz_core::CAMetalLayer>,
    content_view: Retained<ContentView>,
    host: UiHost,
    time_map: AppleTimeMap,
}

impl ViewState {
    pub(crate) fn dispatch_keyboard_event(&mut self, event: &KeyboardEvent) {
        let result = self.host.dispatch_key(event);
        self.finish_input_dispatch(result);
    }

    pub(crate) fn dispatch_pointer_event(&mut self, event: &PointerEvent) {
        let result = self.host.dispatch_pointer_event(event);
        self.finish_input_dispatch(result);
    }

    pub(crate) fn scale_factor(&self) -> f64 {
        scale_factor(&self.content_view)
    }

    pub(crate) const fn time_map(&self) -> AppleTimeMap {
        self.time_map
    }

    pub(crate) fn current_time_ns(&self) -> u64 {
        self.current_time().ticks()
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
            self.sync_display_link_activity();
        }
    }

    fn handle_display_tick(&mut self, tick: FrameTick) -> HostResult<()> {
        let opportunity = self.frame_opportunity_from_tick(tick);
        self.host.pump_timers(opportunity.tick.now.ticks());
        let result = self.render(opportunity);
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
            .setBounds(objc2_core_foundation::CGRect::new(
                objc2_core_foundation::CGPoint::new(0.0, 0.0),
                size.bounds.size,
            ));
        self.metal_layer.setFrame(size.bounds);
        self.metal_layer.setContentsScale(size.scale_factor);
        self.metal_layer
            .setDrawableSize(objc2_core_foundation::CGSize::new(
                f64::from(size.width),
                f64::from(size.height),
            ));

        self.presenter.resize(size.width, size.height);
    }

    fn view_surface_size(&self) -> SurfaceSize {
        let bounds = self.content_view.bounds();
        let scale_factor = self.scale_factor();
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

    fn frame_opportunity_from_tick(&self, tick: FrameTick) -> FrameOpportunity {
        let tick = self.time_map.map_tick(tick);
        frameclock_apple::frame_opportunity(tick, DisplayTiming::fixed(FALLBACK_REFRESH_INTERVAL))
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

    fn render(&mut self, opportunity: FrameOpportunity) -> HostResult<()> {
        let (width, height) = self.size();
        if width == 0 || height == 0 {
            return Ok(());
        }

        let scale_factor = self.scale_factor();
        let viewport = FrameViewport::from_physical_size(width, height, scale_factor);
        let present = {
            let Some(prepared) = self.host.prepare_requested_frame(viewport, opportunity) else {
                return Ok(());
            };
            debug_assert!(
                !prepared.demand().is_empty(),
                "requested frames should carry non-empty planned demand"
            );
            if let Some(plan) = prepared.plan() {
                let _ = self
                    .display_link
                    .set_preferred_frame_interval(plan.frame_interval, opportunity.display_timing);
            }
            self.presenter.present_scene(prepared.scene())
        };

        let present = match present {
            Ok(present) => present,
            Err(err) => {
                self.host.discard_prepared_frame();
                return Err(Box::new(err));
            }
        };
        let WgpuPresentResult::Presented(timings) = present else {
            self.host.discard_prepared_frame();
            return Ok(());
        };
        self.host.record_frame_submission(
            timings,
            frameclock_apple::DEFAULT_FEEDBACK_MODE.submission(self.submitted_at()),
        );
        Ok(())
    }
}

pub(crate) fn init_view_controller<F>(
    mtm: MainThreadMarker,
    options: ViewOptions,
    build_ui: F,
) -> HostResult<Retained<UIViewController>>
where
    F: FnOnce() -> Ui,
{
    let (width, height) = options.size();
    let frame = frame_from_size(width, height);
    let input_responder = create_input_responder(mtm);
    let content_view = ContentView::new(mtm, frame, input_responder.clone());
    let initial_scale_factor = scale_factor(&content_view);
    let initial_size = SurfaceSize {
        bounds: content_view.bounds(),
        scale_factor: initial_scale_factor,
        width: physical_extent(content_view.bounds().size.width, initial_scale_factor),
        height: physical_extent(content_view.bounds().size.height, initial_scale_factor),
    };
    let root_layer = content_view.layer();
    let metal_layer = create_metal_sublayer(&root_layer, &initial_size);

    let instance = wgpu::Instance::default();
    let surface = unsafe {
        let ptr: *const objc2_quartz_core::CAMetalLayer = &*metal_layer;
        instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(
            ptr.cast_mut().cast::<c_void>(),
        ))?
    };

    let presenter = WgpuPresenter::new(
        &instance,
        surface,
        initial_size.width,
        initial_size.height,
        WgpuPresenterDescriptor::new("overstory_host_uikit"),
    )?;
    let host = UiHost::with_scheduler_config(build_ui(), uikit_scheduler_config());
    let time_map = AppleTimeMap::new();
    let display_link = DisplayLink::new(handle_display_tick, OutputId(0), mtm);

    let view_controller =
        UIViewController::initWithNibName_bundle(mtm.alloc::<UIViewController>(), None, None);
    view_controller.setView(Some(&content_view));

    install_view_state(ViewState {
        presenter,
        display_link,
        display_link_started: false,
        input_responder,
        metal_layer,
        content_view,
        host,
        time_map,
    });
    request_initial_frame();

    Ok(view_controller)
}

fn uikit_scheduler_config() -> SchedulerConfig {
    let mut config = SchedulerConfig::predictive();
    // This host uses the display-link callback itself as the frame-start wake.
    // Keep planning fixed at one frame until the UIKit host grows an explicit
    // timer path for `UiHost::pending_frame_start`.
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
    fn uikit_scheduler_uses_display_link_tick_as_frame_start_wake() {
        let config = uikit_scheduler_config();

        assert_eq!(config.minimum_frame_start_margin, FALLBACK_REFRESH_INTERVAL);
        assert_eq!(config.min_depth, 1);
        assert_eq!(config.max_depth, 1);
        assert_eq!(config.initial_depth, 1);
        assert_eq!(config.degradation_policy, DegradationPolicy::Fixed);
    }
}
