// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Native Overstory host built on `winit`, `wgpu`, and `imaging_vello`.
//!
//! This crate owns the platform event loop, window creation, normalized
//! input translation, GPU surface setup, and presentation path. Application
//! crates only need to provide a function that builds an [`overstory::Ui`].

extern crate alloc;

use alloc::{boxed::Box, format, string::String, sync::Arc};
use core::error::Error;

use accesskit_winit::{
    Adapter as AccessKitAdapter, Event as AccessKitEvent, WindowEvent as AccessKitWindowEvent,
};
use frameclock::FrameDemand;
use imaging_vello::VelloRenderer;
use imaging_wgpu::{TextureRenderer, TextureViewTarget};
use overstory::{Ui, UiRendererTimings};
use overstory_host::{FrameViewport, HostFrameSubmission, HostFrameTiming, UiHost};
use ui_events_winit::{WindowEventReducer, WindowEventTranslation};
use web_time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};

const DEFAULT_INITIAL_SIZE: (u32, u32) = (1024, 768);
const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// WGSL for the surface blit: one oversized triangle in clip space that the
/// fragment shader samples from the vello offscreen texture.
const BLIT_SHADER: &str = r#"
@group(0) @binding(0) var scene_texture: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 3.0,  1.0),
    );
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 2.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(2.0, 0.0),
    );
    var out: VertexOut;
    out.position = vec4<f32>(positions[index], 0.0, 1.0);
    out.uv = uvs[index];
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return textureSample(scene_texture, scene_sampler, in.uv);
}
"#;

/// Result type used by the winit host.
pub type HostResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug)]
struct SurfaceValidationError;

impl core::fmt::Display for SurfaceValidationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("wgpu surface acquisition reported a validation error")
    }
}

impl Error for SurfaceValidationError {}

/// Window options for [`run`].
#[derive(Clone, Debug)]
pub struct WindowOptions {
    title: String,
    initial_size: (u32, u32),
}

impl WindowOptions {
    /// Creates options with `title` and a default logical window size.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            initial_size: DEFAULT_INITIAL_SIZE,
        }
    }

    /// Sets the initial logical window size.
    pub fn initial_size(mut self, width: u32, height: u32) -> Self {
        self.initial_size = (width, height);
        self
    }

    /// Returns the platform window title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the initial logical window size.
    pub fn size(&self) -> (u32, u32) {
        self.initial_size
    }
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self::new("Overstory")
    }
}

/// Runs a native winit Overstory app.
///
/// `build_ui` is called once, after the platform window and GPU surface have
/// been created successfully.
pub fn run<F>(options: WindowOptions, build_ui: F) -> HostResult<()>
where
    F: FnOnce() -> Ui + 'static,
{
    let event_loop = EventLoop::<HostEvent>::with_user_event().build()?;
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
    let accesskit_proxy = event_loop.create_proxy();

    let mut app = App {
        options,
        build_ui: Some(build_ui),
        accesskit_proxy,
        state: None,
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}

#[derive(Debug)]
enum HostEvent {
    AccessKit(AccessKitEvent),
}

impl From<AccessKitEvent> for HostEvent {
    fn from(event: AccessKitEvent) -> Self {
        Self::AccessKit(event)
    }
}

struct App<F>
where
    F: FnOnce() -> Ui + 'static,
{
    options: WindowOptions,
    build_ui: Option<F>,
    accesskit_proxy: EventLoopProxy<HostEvent>,
    state: Option<WindowState>,
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

        state.accesskit.process_event(&state.window, &event);
        if state.host.pump_timers_now() {
            state.window.request_redraw();
        }

        let event_time = state.host.frame_now().ticks();
        if let Some(translation) =
            state
                .event_reducer
                .reduce(state.window.scale_factor(), &event, event_time)
        {
            match translation {
                WindowEventTranslation::Pointer(pointer) => {
                    let result = state.host.dispatch_pointer_event(&pointer);
                    if result.needs_redraw {
                        state.request_frame(result.frame_demand);
                    }
                    state.sync_cursor();
                }
                WindowEventTranslation::Keyboard(key) => {
                    let result = state.host.dispatch_key(&key);
                    if result.needs_redraw {
                        state.request_frame(result.frame_demand);
                    }
                }
            }
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                state.resize(new_size);
                state.request_frame(FrameDemand::CONTINUOUS_INPUT);
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                state.request_frame(FrameDemand::CONTINUOUS_INPUT);
            }
            WindowEvent::RedrawRequested => {
                if let Err(err) = state.render() {
                    eprintln!("overstory_host_winit: render error: {err}");
                }
            }
            _ => {}
        }

        state.arm_next_wakeup(event_loop);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: HostEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        match event {
            HostEvent::AccessKit(event) if event.window_id == state.window.id() => {
                state.handle_accesskit_event(event.window_event);
            }
            HostEvent::AccessKit(_) => {}
        }
        state.arm_next_wakeup(event_loop);
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: winit::event::StartCause) {
        if matches!(cause, winit::event::StartCause::ResumeTimeReached { .. })
            && let Some(state) = self.state.as_mut()
        {
            let now = state.host.frame_now();
            let timer_due = state.host.pump_timers(now.ticks());
            let frame_due = state
                .host
                .pending_frame_start()
                .is_some_and(|frame_start| frame_start <= now);
            if timer_due || frame_due {
                state.window.request_redraw();
                event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
            } else {
                state.arm_next_wakeup(event_loop);
            }
        }
    }
}

struct WindowState {
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
    fn request_frame(&mut self, demand: FrameDemand) {
        self.host.request_frame(demand);
        self.window.request_redraw();
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

    fn handle_accesskit_event(&mut self, event: AccessKitWindowEvent) {
        match event {
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

    fn arm_next_wakeup(&self, event_loop: &ActiveEventLoop) {
        let now = self.host.frame_now();
        if let Some(wait) = self.host.duration_until_next_wakeup_at(now.ticks()) {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
                Instant::now() + wait,
            ));
        } else {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
        }
    }

    fn render(&mut self) -> HostResult<()> {
        let (width, height) = self.size();
        if width == 0 || height == 0 {
            return Ok(());
        }

        self.blit.ensure_offscreen(&self.device, width, height);

        let viewport = FrameViewport::from_physical_size(width, height, self.window.scale_factor());
        let timing = HostFrameTiming::pacing_only(self.host.frame_now());
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
}

fn init_window<F>(
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

struct BlitState {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    offscreen: Option<OffscreenTarget>,
}

struct OffscreenTarget {
    #[allow(dead_code, reason = "kept alive for the view and bind group")]
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

impl BlitState {
    fn new(device: &wgpu::Device, output_format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("overstory_host_winit blit layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("overstory_host_winit blit sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("overstory_host_winit blit shader"),
            source: wgpu::ShaderSource::Wgsl(BLIT_SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("overstory_host_winit blit pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("overstory_host_winit blit pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            layout,
            sampler,
            offscreen: None,
        }
    }

    fn invalidate_offscreen(&mut self) {
        self.offscreen = None;
    }

    fn ensure_offscreen(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let needs_new = self
            .offscreen
            .as_ref()
            .is_none_or(|t| t.width != width || t.height != height);
        if needs_new {
            self.offscreen = Some(OffscreenTarget::new(
                device,
                &self.layout,
                &self.sampler,
                width,
                height,
            ));
        }
    }

    fn offscreen_view(&self) -> Option<&wgpu::TextureView> {
        self.offscreen.as_ref().map(|t| &t.view)
    }

    fn blit(&self, device: &wgpu::Device, queue: &wgpu::Queue, output: &wgpu::TextureView) {
        let Some(target) = self.offscreen.as_ref() else {
            return;
        };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("overstory_host_winit blit"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("overstory_host_winit blit pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &target.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit([encoder.finish()]);
    }
}

impl OffscreenTarget {
    fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        width: u32,
        height: u32,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("overstory_host_winit offscreen scene"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OFFSCREEN_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("overstory_host_winit offscreen bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        Self {
            texture,
            view,
            bind_group,
            width,
            height,
        }
    }
}
