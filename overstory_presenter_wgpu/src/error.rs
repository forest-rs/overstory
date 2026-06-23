// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::{error::Error, fmt};

use imaging_wgpu::TextureRendererError;

/// Errors returned by [`crate::WgpuPresenter`].
#[derive(Debug)]
pub enum WgpuPresenterError {
    /// `wgpu` could not find a compatible adapter.
    RequestAdapter(wgpu::RequestAdapterError),
    /// `wgpu` could not create the device and queue.
    RequestDevice(wgpu::RequestDeviceError),
    /// The surface could not be configured for the selected adapter.
    SurfaceIncompatible,
    /// Vello renderer initialization failed.
    Renderer(imaging_vello::Error),
    /// Rendering the scene into the offscreen texture failed.
    Texture(TextureRendererError),
    /// Surface acquisition reported a validation error.
    SurfaceValidation,
}

impl fmt::Display for WgpuPresenterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestAdapter(error) => write!(f, "failed to request wgpu adapter: {error}"),
            Self::RequestDevice(error) => write!(f, "failed to request wgpu device: {error}"),
            Self::SurfaceIncompatible => {
                f.write_str("wgpu surface is not compatible with the selected adapter")
            }
            Self::Renderer(error) => write!(f, "failed to initialize Vello renderer: {error}"),
            Self::Texture(error) => write!(f, "failed to render scene into texture: {error}"),
            Self::SurfaceValidation => {
                f.write_str("wgpu surface acquisition reported a validation error")
            }
        }
    }
}

impl Error for WgpuPresenterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RequestAdapter(error) => Some(error),
            Self::RequestDevice(error) => Some(error),
            Self::SurfaceIncompatible | Self::SurfaceValidation => None,
            Self::Renderer(error) => Some(error),
            Self::Texture(error) => Some(error),
        }
    }
}
