// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Minimal Overstory host: open a winit window, drive a `wgpu` surface, and
//! paint a small Overstory element tree through `imaging_vello` each frame.
//!
//! The tree is a dark backdrop with two templated Buttons, a
//! `ToggleSwitch`, and a `Spinner` side by side. Each button cycles
//! its own label on click or Space / Enter while focused; the toggle
//! flips its `:checked` state; the spinner runs an 8-dot animation
//! driven by a repeating timer from `understory_timing`. Tab and
//! Shift+Tab walk focus across the interactive controls; the
//! focused element paints a white border ring. Layout is driven by
//! taffy via the `Layout` property; text is shaped by parley and
//! rendered as glyph runs through imaging.
//!
//! The host translates winit events through
//! `ui_events_winit::WindowEventReducer` into normalized
//! `ui-events` pointers and keys, which
//! `Ui::dispatch_pointer_event` / `Ui::dispatch_key` consume to drive
//! `:hover`, `:pressed`, `:focused`, and `:checked` plus the
//! registered click / key handlers.
//!
//! For animated controls, the host arms `ControlFlow::WaitUntil` at
//! `Ui::next_timer_deadline()` after each event so the runtime sleeps
//! between ticks instead of polling. `Ui::pump_timers(now)` drains
//! expired timers each wake-up and dispatches them to registered
//! handlers.
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
use std::time::{Duration, Instant};

use imaging::record::Scene;
use imaging_vello::VelloRenderer;
use imaging_wgpu::{TextureRenderer, TextureViewTarget, wgpu};
use kurbo::{self, Size};
use overstory::{
    Button, Navigation, Panel, Slider, Spinner, Text, ToggleSwitch, Ui, layouts, theme,
};
use ui_events_winit::{WindowEventReducer, WindowEventTranslation};
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
    /// Logical-coordinate scene produced by `Ui::paint`. Built each
    /// frame; the `scaled_scene` below wraps it with the high-DPI
    /// scale before going to vello.
    scene: Scene,
    /// Per-frame scaled view of `scene` for the renderer. Kept on
    /// the state so the inner `Vec`s amortize across frames
    /// instead of allocating fresh each render.
    scaled_scene: Scene,
    event_reducer: WindowEventReducer,
    /// Anchor for the monotonic clock we feed to `Ui::pump_timers`.
    /// Subtract from `Instant::now()` to get nanoseconds since
    /// startup.
    start_instant: Instant,
}

impl WindowState {
    /// Returns the host's current monotonic time as nanoseconds
    /// since `start_instant`. Matches the `TimerInstant` unit the
    /// `Ui` timer queue uses.
    fn now_nanos(&self) -> u64 {
        u64::try_from(self.start_instant.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }

    /// Sets winit's control flow to wake at the next pending timer
    /// deadline, or to Wait indefinitely if no timers are pending.
    fn arm_next_wakeup(&self, event_loop: &ActiveEventLoop) {
        if let Some(deadline_nanos) = self.ui.next_timer_deadline() {
            let elapsed = self.start_instant.elapsed().as_nanos();
            let target = u128::from(deadline_nanos);
            let until = target.saturating_sub(elapsed);
            // Clamp huge u128s down before turning back into a
            // Duration; anything beyond u64 nanoseconds is "very far
            // away" and Wait would be fine, but WaitUntil with a far
            // future instant is identical.
            let wait = Duration::from_nanos(u64::try_from(until).unwrap_or(u64::MAX));
            event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
                Instant::now() + wait,
            ));
        } else {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
        }
    }
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

        // Drain any timers that fired between events first, so
        // handler-driven state changes are visible before any new
        // event is processed.
        let now = state.now_nanos();
        state.ui.pump_timers(now);

        // Translate winit events into normalized `ui-events` types. Hover /
        // focus state lives inside `Ui`, so we just forward the translation.
        if let Some(translation) = state
            .event_reducer
            .reduce(state.window.scale_factor(), &event)
        {
            match translation {
                WindowEventTranslation::Pointer(pointer) => {
                    if state.ui.dispatch_pointer_event(&pointer) {
                        state.window.request_redraw();
                    }
                }
                WindowEventTranslation::Keyboard(key) => {
                    // Built-in Tab / Shift+Tab → focus navigation.
                    // App-defined keys (Space/Enter on a button etc.)
                    // are handled by element key handlers below.
                    use ui_events::keyboard::{Key, KeyState, NamedKey};
                    if key.state == KeyState::Down && matches!(key.key, Key::Named(NamedKey::Tab)) {
                        let dir = if key.modifiers.shift() {
                            Navigation::Prev
                        } else {
                            Navigation::Next
                        };
                        state.ui.focus_navigate(dir);
                    }
                    state.ui.dispatch_key(&key);
                    // Keyboard input is rare enough that a redraw on
                    // every key is fine; paint/style/binding flushes
                    // make it a no-op when nothing changed.
                    state.window.request_redraw();
                }
            }
        }

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
        // After every event, arm the next wakeup based on the
        // soonest pending timer. The spinner's repeating timer is
        // what keeps the event loop ticking ~11 times a second.
        state.arm_next_wakeup(event_loop);
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: winit::event::StartCause) {
        // ResumeTimeReached fires when our `WaitUntil` hits its
        // deadline — pump timers and request a redraw so animated
        // controls update on schedule even without other input.
        if matches!(cause, winit::event::StartCause::ResumeTimeReached { .. })
            && let Some(state) = self.state.as_mut()
        {
            let now = state.now_nanos();
            state.ui.pump_timers(now);
            state.window.request_redraw();
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
    }

    fn size(&self) -> (u32, u32) {
        (self.surface_config.width, self.surface_config.height)
    }

    /// Logical-pixel viewport that `Ui` should lay out into.
    /// `Ui::compute_layout` works in logical units; the render pass
    /// scales the painted scene by `scale_factor` so the
    /// logical-coordinate draws fill the physical offscreen target.
    fn logical_viewport(&self) -> Size {
        let (w, h) = self.size();
        let scale = self.window.scale_factor();
        Size::new(f64::from(w) / scale, f64::from(h) / scale)
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let (width, height) = self.size();
        if width == 0 || height == 0 {
            return Ok(());
        }
        // Ensure the offscreen target matches the current surface size.
        self.blit.ensure_offscreen(&self.device, width, height);

        // Compute layout against the logical viewport, then paint.
        // All draws end up at logical world coordinates.
        self.ui.compute_layout(self.logical_viewport());
        self.ui.paint(&mut self.scene);

        // Build a scaled view of the scene that the renderer sends
        // to vello. Multiplying every draw by `scale` maps the
        // logical-coord scene onto the physical-sized offscreen
        // target, so a 200-logical button looks 200×scale physical
        // pixels — properly sized on HiDPI.
        let scale = self.window.scale_factor();
        self.scaled_scene.clear();
        self.scaled_scene
            .append_transformed(&self.scene, kurbo::Affine::scale(scale));

        let offscreen_view = self
            .blit
            .offscreen_view()
            .expect("offscreen target should exist after ensure_offscreen");
        let target = TextureViewTarget::new(offscreen_view, width, height);
        self.renderer
            .render_source_into_texture(&mut &self.scaled_scene, target)?;

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

    // Negotiate a surface format the platform actually supports. On Metal the
    // default is `Bgra8UnormSrgb`, but vello renders sRGB-encoded 8-bit
    // values into the offscreen `Rgba8Unorm`; pushing those through an
    // sRGB-encoding swapchain double-encodes and washes the picture out.
    // Prefer the non-sRGB Bgra variant when available.
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

    let ui = build_ui();
    let state = WindowState {
        window,
        surface,
        surface_config,
        device,
        queue,
        renderer,
        blit,
        ui,
        scene: Scene::new(),
        scaled_scene: Scene::new(),
        event_reducer: WindowEventReducer::default(),
        start_instant: Instant::now(),
    };
    Ok(state)
}

/// Caption-sized, muted `Text` spec used for the labels under the
/// toggle / spinner / slider.
fn caption(text: &str) -> Text {
    Text::new(text)
        .font_size(13.0)
        .foreground(peniko::Color::from_rgba8(0xc8, 0xc8, 0xd2, 0xff))
}

/// Builds a click handler that cycles the button's label each time
/// it fires. `event.current` points at the element the handler is
/// registered on — i.e. the button — so we don't need the id
/// captured separately.
fn cycling_click_handler() -> impl FnMut(&mut Ui, overstory::ClickEvent) + 'static {
    let activations = std::cell::Cell::new(0_u32);
    move |ui: &mut Ui, event| {
        if !matches!(
            event.phase,
            overstory::Phase::Target | overstory::Phase::Bubble
        ) {
            return;
        }
        let n = activations.get() + 1;
        activations.set(n);
        let label = if n == 1 {
            "Clicked!".to_owned()
        } else {
            format!("Clicked {n}×")
        };
        let content_prop = ui.properties().content;
        ui.set_local(event.current, content_prop, label);
    }
}

/// Builds the demo tree. Two `Button`s, a `ToggleSwitch`, a
/// `Spinner`, and a `Slider` in a row, each interactive control
/// labeled with a `Text` caption (where it isn't already labeled).
fn build_ui() -> Ui {
    let mut ui = Ui::new();
    // Install the palette early so any spec that reads from the
    // theme sees real values rather than fallbacks.
    ui.set_theme(theme::default_theme());
    let root = ui.root();

    // Root layout: full-viewport row-flex with padding + gap.
    let root_layout = overstory::taffy::Style {
        justify_content: Some(overstory::taffy::JustifyContent::Center),
        align_items: Some(overstory::taffy::AlignItems::Center),
        size: overstory::taffy::Size {
            width: overstory::taffy::Dimension::percent(1.0),
            height: overstory::taffy::Dimension::percent(1.0),
        },
        padding: layouts::uniform_padding(50.0),
        ..layouts::row_with_gap(32.0)
    };
    let layout_prop = ui.properties().layout;
    ui.set_local(root, layout_prop, root_layout);

    // Two buttons. Each owns its own activation counter via the
    // closure returned by `cycling_click_handler`. `event.current`
    // gives the handler the button id at fire time, so the
    // closure stays self-contained.
    ui.append(
        root,
        Button::new("Click me").on_click(cycling_click_handler()),
    );
    ui.append(root, Button::new("Or me").on_click(cycling_click_handler()));

    // A labeled `ToggleSwitch` — toggle on top, "Notifications"
    // caption beneath, wrapped in a column.
    let toggle_col = ui.append(root, Panel::column(6.0).align_center());
    ui.append(toggle_col, ToggleSwitch::new());
    ui.append(toggle_col, caption("Notifications"));

    // A labeled `Spinner`.
    let spinner_col = ui.append(root, Panel::column(6.0).align_center());
    ui.append(spinner_col, Spinner::new());
    ui.append(spinner_col, caption("Loading…"));

    // A labeled `Slider`.
    let slider_col = ui.append(root, Panel::column(6.0).align_center());
    ui.append(slider_col, Slider::new().value(0.4));
    ui.append(slider_col, caption("Volume"));

    ui.set_cascade(theme::default_cascade(ui.properties()));

    ui
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
