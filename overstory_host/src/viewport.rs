// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Host viewport helpers.

use kurbo::Size;

/// Converts a physical target size and scale factor into a logical viewport.
#[must_use]
pub fn logical_viewport(width: u32, height: u32, scale_factor: f64) -> Size {
    Size::new(
        f64::from(width) / scale_factor,
        f64::from(height) / scale_factor,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_viewport_divides_physical_size_by_scale_factor() {
        assert_eq!(logical_viewport(400, 300, 2.0), Size::new(200.0, 150.0));
    }
}
