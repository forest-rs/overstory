// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::format;
use core::fmt;

use imaging::record::Scene;
use imaging_vello::VelloRenderer;
use imaging_wgpu::{TextureRenderer, TextureViewTarget};
use overstory::UiRendererTimings;
use web_time::Instant;

use crate::blit::BlitState;
use crate::{WgpuPresenterDescriptor, WgpuPresenterError};

/// Result of attempting to present one prepared scene.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WgpuPresentResult {
    /// The frame was submitted to the presentation surface.
    Presented(UiRendererTimings),
    /// The presentation surface was lost or outdated and has been reconfigured.
    SurfaceReconfigured,
    /// The presentation surface was unavailable, occluded, or timed out.
    Skipped,
}

/// `wgpu` presentation state for one native host surface.
pub struct WgpuPresenter {
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: VelloRenderer,
    blit: BlitState,
}

impl WgpuPresenter {
    /// Creates a presenter for an already-created native `wgpu` surface.
    ///
    /// The platform host owns surface creation. This method selects an adapter,
    /// creates the device and queue, configures the surface, and initializes the
    /// Vello renderer and blit pipeline.
    pub fn new(
        instance: &wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
        descriptor: WgpuPresenterDescriptor<'_>,
    ) -> Result<Self, WgpuPresenterError> {
        let width = width.max(1);
        let height = height.max(1);
        let (adapter, device, queue) = pollster::block_on(async {
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: descriptor.power_preference,
                    force_fallback_adapter: descriptor.force_fallback_adapter,
                    compatible_surface: Some(&surface),
                })
                .await
                .map_err(WgpuPresenterError::RequestAdapter)?;
            let device_label = format!("{} device", descriptor.label);
            let adapter_limits = adapter.limits();
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some(device_label.as_str()),
                    required_features: wgpu::Features::empty(),
                    required_limits: presenter_required_limits(&adapter_limits),
                    memory_hints: wgpu::MemoryHints::default(),
                    trace: wgpu::Trace::Off,
                    experimental_features: wgpu::ExperimentalFeatures::default(),
                })
                .await
                .map_err(WgpuPresenterError::RequestDevice)?;
            Ok::<_, WgpuPresenterError>((adapter, device, queue))
        })?;

        let mut surface_config = surface
            .get_default_config(&adapter, width, height)
            .ok_or(WgpuPresenterError::SurfaceIncompatible)?;
        surface_config.format = preferred_surface_format(&surface, &adapter, surface_config.format);
        surface_config.usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
        surface.configure(&device, &surface_config);

        let renderer = VelloRenderer::new(device.clone(), queue.clone())
            .map_err(WgpuPresenterError::Renderer)?;
        let blit = BlitState::new(&device, surface_config.format);
        Ok(Self {
            surface,
            surface_config,
            device,
            queue,
            renderer,
            blit,
        })
    }

    /// Returns the configured physical surface size in pixels.
    #[must_use]
    pub const fn size(&self) -> (u32, u32) {
        (self.surface_config.width, self.surface_config.height)
    }

    /// Resizes and reconfigures the presentation surface.
    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.surface_config.width == width && self.surface_config.height == height {
            return;
        }
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        self.blit.invalidate_offscreen();
    }

    /// Presents `scene` into the configured surface.
    pub fn present_scene(
        &mut self,
        scene: &Scene,
    ) -> Result<WgpuPresentResult, WgpuPresenterError> {
        let (width, height) = self.size();
        if width == 0 || height == 0 {
            return Ok(WgpuPresentResult::Skipped);
        }

        self.blit.ensure_offscreen(&self.device, width, height);
        let offscreen_view = self
            .blit
            .offscreen_view()
            .expect("offscreen target should exist after ensure_offscreen");
        let target = TextureViewTarget::new(offscreen_view, width, height);

        let mut source = scene;
        let phase_start = Instant::now();
        self.renderer
            .render_source_into_texture(&mut source, target)
            .map_err(WgpuPresenterError::Texture)?;
        let scene_to_texture = phase_start.elapsed();

        let phase_start = Instant::now();
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(output)
            | wgpu::CurrentSurfaceTexture::Suboptimal(output) => output,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.reconfigure_surface();
                return Ok(WgpuPresentResult::SurfaceReconfigured);
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(WgpuPresentResult::Skipped);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(WgpuPresenterError::SurfaceValidation);
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
        Ok(WgpuPresentResult::Presented(UiRendererTimings {
            scene_to_texture,
            surface_acquire,
            blit,
            present,
            total: scene_to_texture + surface_acquire + blit + present,
        }))
    }

    fn reconfigure_surface(&mut self) {
        self.surface.configure(&self.device, &self.surface_config);
    }
}

impl fmt::Debug for WgpuPresenter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WgpuPresenter")
            .field("size", &self.size())
            .field("surface_format", &self.surface_config.format)
            .finish_non_exhaustive()
    }
}

fn preferred_surface_format(
    surface: &wgpu::Surface<'_>,
    adapter: &wgpu::Adapter,
    fallback: wgpu::TextureFormat,
) -> wgpu::TextureFormat {
    let caps = surface.get_capabilities(adapter);
    caps.formats
        .iter()
        .copied()
        .find(|format| {
            matches!(
                *format,
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
            )
        })
        .unwrap_or(fallback)
}

fn presenter_required_limits(adapter_limits: &wgpu::Limits) -> wgpu::Limits {
    let mut limits = wgpu::Limits::default()
        .using_resolution(adapter_limits.clone())
        .using_alignment(adapter_limits.clone());
    limits.max_inter_stage_shader_variables = limits
        .max_inter_stage_shader_variables
        .min(adapter_limits.max_inter_stage_shader_variables);
    limits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presenter_limits_allow_stricter_inter_stage_shader_variables() {
        let adapter_limits = wgpu::Limits {
            max_inter_stage_shader_variables: 15,
            ..wgpu::Limits::default()
        };

        let limits = presenter_required_limits(&adapter_limits);

        assert_eq!(limits.max_inter_stage_shader_variables, 15);
    }

    #[test]
    fn presenter_limits_keep_modern_defaults_for_unrelated_limits() {
        let adapter_limits = wgpu::Limits {
            max_inter_stage_shader_variables: 15,
            ..wgpu::Limits::default()
        };

        let limits = presenter_required_limits(&adapter_limits);

        assert_eq!(
            limits.max_color_attachments,
            wgpu::Limits::default().max_color_attachments
        );
        assert_eq!(
            limits.max_uniform_buffer_binding_size,
            wgpu::Limits::default().max_uniform_buffer_binding_size
        );
    }

    #[test]
    fn presenter_limits_preserve_adapter_texture_resolution() {
        let adapter_limits = wgpu::Limits {
            max_texture_dimension_2d: 16_384,
            ..wgpu::Limits::default()
        };

        let limits = presenter_required_limits(&adapter_limits);

        assert_eq!(limits.max_texture_dimension_2d, 16_384);
    }

    #[test]
    fn presenter_limits_use_adapter_buffer_alignment() {
        let adapter_limits = wgpu::Limits {
            min_uniform_buffer_offset_alignment: 512,
            ..wgpu::Limits::default()
        };

        let limits = presenter_required_limits(&adapter_limits);

        assert_eq!(limits.min_uniform_buffer_offset_alignment, 512);
    }
}
