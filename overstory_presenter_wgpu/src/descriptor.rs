// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Construction options for [`crate::WgpuPresenter`].
#[derive(Clone, Copy, Debug)]
pub struct WgpuPresenterDescriptor<'a> {
    /// Label prefix used for the `wgpu` device request.
    pub label: &'a str,
    /// Adapter power preference passed to `wgpu`.
    pub power_preference: wgpu::PowerPreference,
    /// Whether `wgpu` should force a fallback adapter.
    pub force_fallback_adapter: bool,
}

impl<'a> WgpuPresenterDescriptor<'a> {
    /// Creates a descriptor with Overstory's default adapter preferences.
    #[must_use]
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
        }
    }
}

impl Default for WgpuPresenterDescriptor<'static> {
    fn default() -> Self {
        Self::new("overstory_presenter_wgpu")
    }
}
