// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Minimal Overstory host: open a winit window, drive a `wgpu` surface, and
//! paint a small Overstory element tree through `imaging_vello` each frame.
//!
//! The Phase 1c tree is two elements: a full-viewport dark background and a
//! centered rounded blue panel. Both are plain `ELEMENT_KIND_GENERIC` nodes,
//! distinguished by a `ClassId`. Their backgrounds and the foreground's
//! corner radius come from an implicit stylesheet installed via
//! `Ui::set_cascade`; only bounds are still set imperatively (taffy lands in
//! Phase 1e). `Ui::paint` walks the tree, dispatches each element's
//! behavior, and assembles the per-element scene caches into one output
//! scene the host hands to vello.
//!
//! ## Render path
//!
//! `imaging_vello` writes into an `Rgba8Unorm` texture; most surfaces only
//! support `Bgra*` formats (notably Metal). The demo therefore:
//!
//! 1. Renders the imaging scene into an offscreen `Rgba8Unorm` texture.
//! 2. Runs a three-vertex full-screen WGSL pass that samples the offscreen
//!    and writes into the swapchain texture (whatever format
//!    `surface.get_default_config` negotiates).
//!
//! This mirrors the pattern used by the prior overstory prototype host on
//! `overstory_2_clean_rewrite`.

use std::sync::Arc;

use imaging::record::Scene;
use imaging_vello::VelloRenderer;
use imaging_wgpu::{TextureRenderer, TextureViewTarget, wgpu};
use kurbo::Rect;
use overstory::{ELEMENT_KIND_GENERIC, ElementId, Ui};
use peniko::{Brush, Color};
use understory_style::{
    ClassId, SelectorBuilder, SelectorStep, StyleBuilder, StyleCascadeBuilder, StyleOrigin, TypeTag,
};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

const INITIAL_SIZE: (u32, u32) = (800, 600);
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);

    let mut app = App::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}

/// Per-window state. Held lazily so we can construct it once we have the
/// resumed `Window`.
struct WindowState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: VelloRenderer,
    blit: BlitState,
    ui: Ui,
    background: ElementId,
    foreground: ElementId,
    scene: Scene,
}

#[derive(Default)]
struct App {
    state: Option<WindowState>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        match init_window(event_loop) {
            Ok(state) => {
                state.window.request_redraw();
                self.state = Some(state);
            }
            Err(err) => {
                eprintln!("overstory_demo: failed to initialize window: {err}");
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                state.resize(new_size);
                state.window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                if let Err(err) = state.render() {
                    eprintln!("overstory_demo: render error: {err}");
                }
            }
            _ => {}
        }
    }
}

impl WindowState {
    fn resize(&mut self, size: PhysicalSize<u32>) {
        self.surface_config.width = size.width.max(1);
        self.surface_config.height = size.height.max(1);
        self.surface.configure(&self.device, &self.surface_config);
        // Invalidate the offscreen target so it's reallocated at the new size.
        self.blit.invalidate_offscreen();
        // Resize the demo's elements to fill the new viewport.
        self.update_layout();
    }

    fn size(&self) -> (u32, u32) {
        (self.surface_config.width, self.surface_config.height)
    }

    fn update_layout(&mut self) {
        let (w, h) = self.size();
        let (w, h) = (f64::from(w), f64::from(h));
        let bounds_prop = self.ui.properties().bounds;
        self.ui
            .set_local(self.background, bounds_prop, Rect::new(0.0, 0.0, w, h));
        let inset = (w.min(h) * 0.15).max(24.0);
        self.ui.set_local(
            self.foreground,
            bounds_prop,
            Rect::new(inset, inset, w - inset, h - inset),
        );
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let (width, height) = self.size();
        if width == 0 || height == 0 {
            return Ok(());
        }
        // Ensure the offscreen target matches the current surface size.
        self.blit.ensure_offscreen(&self.device, width, height);

        // Drive the imaging scene from the element tree.
        self.ui.paint(&mut self.scene);

        let offscreen_view = self
            .blit
            .offscreen_view()
            .expect("offscreen target should exist after ensure_offscreen");
        let target = TextureViewTarget::new(offscreen_view, width, height);
        self.renderer
            .render_source_into_texture(&mut &self.scene, target)?;

        // Acquire the swapchain image and blit the offscreen into it.
        let output = match self.surface.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                self.surface.configure(&self.device, &self.surface_config);
                return Ok(());
            }
            Err(err) => return Err(Box::new(err)),
        };
        let output_view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.blit.blit(&self.device, &self.queue, &output_view);
        output.present();
        Ok(())
    }
}

fn init_window(event_loop: &ActiveEventLoop) -> Result<WindowState, Box<dyn std::error::Error>> {
    let window_attributes = Window::default_attributes()
        .with_title("overstory_demo")
        .with_inner_size(winit::dpi::LogicalSize::new(
            f64::from(INITIAL_SIZE.0),
            f64::from(INITIAL_SIZE.1),
        ));
    let window = Arc::new(event_loop.create_window(window_attributes)?);

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
            .map_err(|e| Box::<dyn std::error::Error>::from(format!("request_adapter: {e}")))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("overstory_demo device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::default(),
            })
            .await
            .map_err(|e| Box::<dyn std::error::Error>::from(format!("request_device: {e}")))?;
        Ok::<_, Box<dyn std::error::Error>>((adapter, device, queue))
    })?;

    let inner = window.inner_size();
    let initial_width = inner.width.max(1);
    let initial_height = inner.height.max(1);

    // Negotiate a surface format the platform actually supports. On Metal this
    // will be `Bgra8Unorm`/`Bgra8UnormSrgb`; on most other platforms similar.
    let mut surface_config = surface
        .get_default_config(&adapter, initial_width, initial_height)
        .ok_or("surface not compatible with adapter")?;
    surface_config.usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
    surface.configure(&device, &surface_config);

    let renderer = VelloRenderer::new(device.clone(), queue.clone())?;
    let blit = BlitState::new(&device, surface_config.format);

    let (ui, background, foreground) = build_ui();
    let mut state = WindowState {
        window,
        surface,
        surface_config,
        device,
        queue,
        renderer,
        blit,
        ui,
        background,
        foreground,
        scene: Scene::new(),
    };
    state.update_layout();
    Ok(state)
}

/// Marks the centered foreground panel for the stylesheet.
const CLASS_FOREGROUND: ClassId = ClassId(1);

/// Builds the Phase 1c element tree: a full-viewport dark background with a
/// blue rounded panel centered inside. Backgrounds and the foreground's
/// corner radius come from a stylesheet; bounds are still set imperatively
/// by `WindowState::update_layout` until taffy lands in Phase 1e.
fn build_ui() -> (Ui, ElementId, ElementId) {
    let mut ui = Ui::new();
    let root = ui.root();
    let foreground = ui.insert_child(root, ELEMENT_KIND_GENERIC);
    ui.add_class(foreground, CLASS_FOREGROUND);

    // Implicit style for any generic element: dark background.
    let generic_kind = TypeTag(ELEMENT_KIND_GENERIC.tag());
    let generic_style = StyleBuilder::new()
        .set(
            ui.properties().background,
            Some(Brush::Solid(Color::from_rgba8(0x1e, 0x1e, 0x24, 0xff))),
        )
        .build();
    let generic_selector = SelectorBuilder::new(SelectorStep::type_tag(generic_kind)).build();

    // Override for the foreground class: blue with a corner radius.
    let foreground_style = StyleBuilder::new()
        .set(
            ui.properties().background,
            Some(Brush::Solid(Color::from_rgba8(0x6b, 0x9d, 0xff, 0xff))),
        )
        .set(ui.properties().corner_radius, 16.0_f64)
        .build();
    let foreground_selector =
        SelectorBuilder::new(SelectorStep::type_tag(generic_kind).with_class(CLASS_FOREGROUND))
            .build();

    let cascade = StyleCascadeBuilder::new()
        .push_rule(StyleOrigin::Base, generic_selector, generic_style)
        .push_rule(StyleOrigin::Base, foreground_selector, foreground_style)
        .build();
    ui.set_cascade(cascade);

    (ui, root, foreground)
}

/// Owns the WGSL blit pipeline and the cached offscreen target it samples.
struct BlitState {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    offscreen: Option<OffscreenTarget>,
}

struct OffscreenTarget {
    // The texture must outlive the view and bind group; we never read from it
    // directly, so suppress the dead-code lint.
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
            label: Some("overstory_demo blit layout"),
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
            label: Some("overstory_demo blit sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("overstory_demo blit shader"),
            source: wgpu::ShaderSource::Wgsl(BLIT_SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("overstory_demo blit pipeline layout"),
            bind_group_layouts: &[&layout],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("overstory_demo blit pipeline"),
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
            label: Some("overstory_demo blit"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("overstory_demo blit pass"),
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
            label: Some("overstory_demo offscreen scene"),
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
            label: Some("overstory_demo offscreen bind group"),
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
