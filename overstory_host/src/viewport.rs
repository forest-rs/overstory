// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Host viewport and coordinate-space conversion types.

use kurbo::{Affine, Size};

/// Physical render-target size in device pixels.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PhysicalSize {
    width: u32,
    height: u32,
}

impl PhysicalSize {
    /// Creates a physical render-target size.
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Returns the width in physical device pixels.
    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }

    /// Returns the height in physical device pixels.
    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }

    /// Returns whether either physical extent is zero.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// Viewport metrics for preparing one host frame.
///
/// Overstory keeps layout, hit testing, presentation, and composition geometry
/// in logical coordinates. The host owns the final projection into a physical
/// render target, so a frame viewport carries both the logical layout size and
/// the scale factor that maps it to device pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameViewport {
    physical_size: PhysicalSize,
    logical_size: Size,
    scale_factor: f64,
}

impl FrameViewport {
    /// Creates viewport metrics from a physical render-target size and host
    /// scale factor.
    ///
    /// Platform hosts should prefer this constructor so the logical viewport
    /// is always derived from the actual target size used for rendering.
    #[must_use]
    pub fn from_physical_size(width: u32, height: u32, scale_factor: f64) -> Self {
        let scale_factor = sane_scale_factor(scale_factor);
        Self {
            physical_size: PhysicalSize::new(width, height),
            logical_size: Size::new(
                f64::from(width) / scale_factor,
                f64::from(height) / scale_factor,
            ),
            scale_factor,
        }
    }

    /// Creates viewport metrics from a logical layout size.
    ///
    /// This is useful for tests, benchmarks, and headless scenarios where the
    /// caller intentionally chooses a logical viewport first.
    #[must_use]
    pub fn from_logical_size(logical_size: Size, scale_factor: f64) -> Self {
        let scale_factor = sane_scale_factor(scale_factor);
        Self {
            physical_size: PhysicalSize::new(
                physical_extent_for_logical(logical_size.width, scale_factor),
                physical_extent_for_logical(logical_size.height, scale_factor),
            ),
            logical_size,
            scale_factor,
        }
    }

    /// Returns the physical render-target size in device pixels.
    #[must_use]
    pub const fn physical_size(self) -> PhysicalSize {
        self.physical_size
    }

    /// Returns the UI layout size in logical pixels.
    #[must_use]
    pub const fn logical_size(self) -> Size {
        self.logical_size
    }

    /// Returns the scale factor from logical pixels into physical pixels.
    #[must_use]
    pub const fn scale_factor(self) -> f64 {
        self.scale_factor
    }

    /// Returns the logical-to-physical transform for the output scene.
    #[must_use]
    pub fn logical_to_physical(self) -> Affine {
        Affine::scale(self.scale_factor)
    }

    pub(crate) fn needs_output_scale(self) -> bool {
        self.scale_factor != 1.0
    }
}

fn sane_scale_factor(scale_factor: f64) -> f64 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}

fn physical_extent_for_logical(logical_extent: f64, scale_factor: f64) -> u32 {
    let physical_extent = (logical_extent * scale_factor).ceil();
    if !physical_extent.is_finite() || physical_extent <= 0.0 {
        return 0;
    }
    if physical_extent >= f64::from(u32::MAX) {
        return u32::MAX;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "finite positive extent was clamped to u32 range"
    )]
    {
        physical_extent as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_size_constructor_derives_logical_size() {
        let viewport = FrameViewport::from_physical_size(400, 300, 2.0);

        assert_eq!(viewport.logical_size(), Size::new(200.0, 150.0));
    }

    #[test]
    fn frame_viewport_carries_physical_logical_and_scale() {
        let viewport = FrameViewport::from_physical_size(400, 300, 2.0);

        assert_eq!(viewport.physical_size(), PhysicalSize::new(400, 300));
        assert_eq!(viewport.logical_size(), Size::new(200.0, 150.0));
        assert_eq!(viewport.scale_factor(), 2.0);
        assert_eq!(viewport.logical_to_physical(), Affine::scale(2.0));
    }

    #[test]
    fn frame_viewport_sanitizes_invalid_scale_factor() {
        let viewport = FrameViewport::from_physical_size(400, 300, 0.0);

        assert_eq!(viewport.logical_size(), Size::new(400.0, 300.0));
        assert_eq!(viewport.scale_factor(), 1.0);
    }
}
