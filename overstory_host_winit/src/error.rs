// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Error types for the winit host.

use core::error::Error;

#[derive(Debug)]
pub(crate) struct SurfaceValidationError;

impl core::fmt::Display for SurfaceValidationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("wgpu surface acquisition reported a validation error")
    }
}

impl Error for SurfaceValidationError {}
