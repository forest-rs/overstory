// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained winit window, renderer, surface, accessibility, and UI host state.

use alloc::{boxed::Box, format, sync::Arc};
use core::error::Error;

use accesskit_winit::{
    Adapter as AccessKitAdapter, Event as AccessKitEvent, WindowEvent as AccessKitWindowEvent,
};
use frameclock::{Duration as FrameDuration, FrameDemand};
use imaging_vello::VelloRenderer;
use imaging_wgpu::{TextureRenderer, TextureViewTarget};
use overstory::{Ui, UiRendererTimings};
use overstory_host::{FrameViewport, HostFrameSubmission, HostFrameTiming, UiHost};
use ui_events_winit::{WindowEventReducer, WindowEventTranslation};
use web_time::Instant;
use winit::dpi::PhysicalSize;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::Window;

use crate::app::HostEvent;
use crate::blit::BlitState;
use crate::error::SurfaceValidationError;
use crate::{HostResult, WindowOptions};

pub(crate) struct WindowState {
    window: Arc<Window>,
    accesskit: AccessKitAdapter,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: VelloRenderer,
    blit: BlitState,
    host: UiHost,
    event_reducer: WindowEventReducer,
}

impl WindowState {
    pub(crate) fn request_frame(&mut self, demand: FrameDemand) {
        self.host.request_frame(demand);
        self.window.request_redraw();
    }

    pub(crate) fn handle_window_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        self.accesskit.process_event(&self.window, &event);
        if self.host.pump_timers_now() {
            self.window.request_redraw();
        }

        let event_time = self.host.frame_now().ticks();
        if let Some(translation) =
            self.event_reducer
                .reduce(self.window.scale_factor(), &event, event_time)
        {
            self.dispatch_translated_event(translation);
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                self.resize(new_size);
                self.request_frame(FrameDemand::CONTINUOUS_INPUT);
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                self.request_frame(FrameDemand::CONTINUOUS_INPUT);
            }
            WindowEvent::RedrawRequested => {
                if let Err(err) = self.render() {
                    eprintln!("overstory_host_winit: render error: {err}");
                }
            }
            _ => {}
        }

        self.arm_next_wakeup(event_loop);
    }

    pub(crate) fn handle_accesskit_event(&mut self, event: AccessKitEvent) {
        if event.window_id != self.window.id() {
            return;
        }
        match event.window_event {
            AccessKitWindowEvent::InitialTreeRequested => {
                self.sync_accessibility();
            }
            AccessKitWindowEvent::ActionRequested(request) => {
                if self.host.handle_accesskit_action(&request).needs_redraw {
                    self.request_frame(FrameDemand::INPUT);
                }
                self.sync_accessibility();
                self.sync_cursor();
            }
            AccessKitWindowEvent::AccessibilityDeactivated => {}
        }
    }

    pub(crate) fn handle_start_cause(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::ResumeTimeReached { .. }) {
            let now = self.host.frame_now();
            let timer_due = self.host.pump_timers(now.ticks());
            let frame_due = self
                .host
                .pending_frame_start()
                .is_some_and(|frame_start| frame_start <= now);
            if timer_due || frame_due {
                self.window.request_redraw();
                event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
            } else {
                self.arm_next_wakeup(event_loop);
            }
        }
    }

    pub(crate) fn arm_next_wakeup(&self, event_loop: &ActiveEventLoop) {
        let now = self.host.frame_now();
        if let Some(wait) = self.host.duration_until_next_wakeup_at(now.ticks()) {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
                Instant::now() + wait,
            ));
        } else {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
        }
    }

    fn dispatch_translated_event(&mut self, translation: WindowEventTranslation) {
        match translation {
            WindowEventTranslation::Pointer(pointer) => {
                let result = self.host.dispatch_pointer_event(&pointer);
                if result.needs_redraw {
                    self.request_frame(result.frame_demand);
                }
                self.sync_cursor();
            }
            WindowEventTranslation::Keyboard(key) => {
                let result = self.host.dispatch_key(&key);
                if result.needs_redraw {
                    self.request_frame(result.frame_demand);
                }
            }
        }
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        self.surface_config.width = size.width.max(1);
        self.surface_config.height = size.height.max(1);
        self.surface.configure(&self.device, &self.surface_config);
        self.blit.invalidate_offscreen();
    }

    fn size(&self) -> (u32, u32) {
        (self.surface_config.width, self.surface_config.height)
    }

    fn sync_cursor(&mut self) {
        if let Some(next) = self.host.cursor_update() {
            self.window.set_cursor(next);
        }
    }

    fn sync_accessibility(&mut self) {
        let host = &self.host;
        let scale_factor = self.window.scale_factor();
        self.accesskit
            .update_if_active(|| host.accesskit_tree_update(scale_factor));
    }

    fn render(&mut self) -> HostResult<()> {
        let (width, height) = self.size();
        if width == 0 || height == 0 {
            return Ok(());
        }

        self.blit.ensure_offscreen(&self.device, width, height);

        let viewport = FrameViewport::from_physical_size(width, height, self.window.scale_factor());
        let timing = self.host_frame_timing();
        let Some(prepared) = self.host.prepare_requested_frame(viewport, timing) else {
            if !self.host.pending_frame_demand().is_empty() {
                self.window.request_redraw();
            }
            return Ok(());
        };
        let demand = prepared.demand();
        debug_assert!(
            !demand.is_empty(),
            "requested frames should carry non-empty planned demand"
        );
        let mut scaled_scene = prepared.scene();

        let offscreen_view = self
            .blit
            .offscreen_view()
            .expect("offscreen target should exist after ensure_offscreen");
        let target = TextureViewTarget::new(offscreen_view, width, height);
        let phase_start = Instant::now();
        if let Err(err) = self
            .renderer
            .render_source_into_texture(&mut scaled_scene, target)
        {
            self.host.discard_prepared_frame();
            return Err(err.into());
        }
        let scene_to_texture = phase_start.elapsed();
        self.sync_accessibility();

        let phase_start = Instant::now();
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(output)
            | wgpu::CurrentSurfaceTexture::Suboptimal(output) => output,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.surface_config);
                self.host.discard_prepared_frame();
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                self.host.discard_prepared_frame();
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                self.host.discard_prepared_frame();
                return Err(Box::new(SurfaceValidationError));
            }
        };
        let surface_acquire = phase_start.elapsed();
        let output_view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let phase_start = Instant::now();
        self.blit.blit(&self.device, &self.queue, &output_view);
        let blit = phase_start.elapsed();
        let phase_start = Instant::now();
        output.present();
        let present = phase_start.elapsed();
        self.host
            .record_frame_submission(HostFrameSubmission::new(UiRendererTimings {
                scene_to_texture,
                surface_acquire,
                blit,
                present,
                total: scene_to_texture + surface_acquire + blit + present,
            }));
        if !self.host.pending_frame_demand().is_empty() {
            self.window.request_redraw();
        }
        Ok(())
    }

    fn host_frame_timing(&self) -> HostFrameTiming {
        let now = self.host.frame_now();
        // TODO(overstory_host_winit): This is only a nominal pacing hint.
        // Plain winit does not expose presentation timing or vsync feedback,
        // so a stronger timing model would need platform-specific host support
        // rather than more information from this adapter.
        self.window
            .current_monitor()
            .and_then(|monitor| monitor.refresh_rate_millihertz())
            .and_then(refresh_interval_from_millihertz)
            .map_or_else(
                || HostFrameTiming::pacing_only(now),
                |interval| HostFrameTiming::pacing_only_with_refresh_interval(now, interval),
            )
    }
}

fn refresh_interval_from_millihertz(millihertz: u32) -> Option<FrameDuration> {
    if millihertz == 0 {
        return None;
    }

    let millihertz = u64::from(millihertz);
    Some(FrameDuration(
        (1_000_000_000_000_u64 + millihertz / 2) / millihertz,
    ))
}

pub(crate) fn init_window<F>(
    event_loop: &ActiveEventLoop,
    options: &WindowOptions,
    build_ui: F,
    accesskit_proxy: EventLoopProxy<HostEvent>,
) -> HostResult<WindowState>
where
    F: FnOnce() -> Ui,
{
    let (width, height) = options.size();
    let window_attributes = Window::default_attributes()
        .with_title(options.title())
        .with_visible(false)
        .with_inner_size(winit::dpi::LogicalSize::new(
            f64::from(width),
            f64::from(height),
        ));
    let window = Arc::new(event_loop.create_window(window_attributes)?);
    let accesskit = AccessKitAdapter::with_event_loop_proxy(event_loop, &window, accesskit_proxy);

    let instance = wgpu::Instance::default();
    let surface = instance.create_surface(window.clone())?;

    let (adapter, device, queue) = pollster::block_on(async {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .map_err(|e| Box::<dyn Error>::from(format!("request_adapter: {e}")))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("overstory_host_winit device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::default(),
            })
            .await
            .map_err(|e| Box::<dyn Error>::from(format!("request_device: {e}")))?;
        Ok::<_, Box<dyn Error>>((adapter, device, queue))
    })?;

    let inner = window.inner_size();
    let initial_width = inner.width.max(1);
    let initial_height = inner.height.max(1);

    let mut surface_config = surface
        .get_default_config(&adapter, initial_width, initial_height)
        .ok_or("surface not compatible with adapter")?;
    let caps = surface.get_capabilities(&adapter);
    if let Some(unorm) = caps.formats.iter().copied().find(|f| {
        matches!(
            *f,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
        )
    }) {
        surface_config.format = unorm;
    }
    surface_config.usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
    surface.configure(&device, &surface_config);

    let renderer = VelloRenderer::new(device.clone(), queue.clone())?;
    let blit = BlitState::new(&device, surface_config.format);
    let host = UiHost::new(build_ui());
    window.set_visible(true);

    Ok(WindowState {
        window,
        accesskit,
        surface,
        surface_config,
        device,
        queue,
        renderer,
        blit,
        host,
        event_reducer: WindowEventReducer::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_interval_from_millihertz_rounds_to_nanosecond_ticks() {
        assert_eq!(
            refresh_interval_from_millihertz(120_000),
            Some(FrameDuration(8_333_333))
        );
        assert_eq!(
            refresh_interval_from_millihertz(60_000),
            Some(FrameDuration(16_666_667))
        );
        assert_eq!(refresh_interval_from_millihertz(0), None);
    }
}
