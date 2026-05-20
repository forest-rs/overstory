// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Built-in dependency properties that every Overstory element understands.

use kurbo::Rect;
use peniko::Brush;
use understory_property::{Property, PropertyMetadataBuilder, PropertyRegistry};

use crate::channels::{LAYOUT, PAINT};

/// Handles to the dependency properties Overstory registers in every
/// [`crate::Ui`]'s `PropertyRegistry`.
///
/// Properties are registered once at `Ui` construction; their type-safe
/// [`Property<T>`] handles are exposed here so application code can read and
/// write them without re-registering.
#[derive(Copy, Clone, Debug)]
pub struct BuiltInProperties {
    /// Fill brush for the element's body. Default `None` (no fill).
    pub background: Property<Option<Brush>>,

    /// Bounds in viewport coordinates the element paints into. Default
    /// [`Rect::ZERO`] — a zero-area element paints nothing.
    ///
    /// Will become parent-relative + layout-driven once Phase 1e wires taffy.
    pub bounds: Property<Rect>,

    /// Corner radius for the background fill. Default `0.0` (rectangular).
    pub corner_radius: Property<f64>,

    /// `taffy::Style` driving this element's contribution to layout. Default
    /// is `taffy::Style::default()` (an empty leaf). Writes mark the
    /// `LAYOUT` channel so the next `Ui::compute_layout` picks them up.
    ///
    /// Phase 1e exposes the whole `taffy::Style` as one property. Later
    /// phases will likely split into per-attribute properties so selectors
    /// can target individual layout attributes.
    pub layout: Property<taffy::Style>,
}

impl BuiltInProperties {
    /// Registers every built-in property into `registry` and returns their
    /// type-safe handles.
    pub(crate) fn register(registry: &mut PropertyRegistry) -> Self {
        Self {
            background: registry.register(
                "Background",
                PropertyMetadataBuilder::<Option<Brush>>::new(None)
                    .affects_channels(PAINT.into_set())
                    .build(),
            ),
            bounds: registry.register(
                "Bounds",
                PropertyMetadataBuilder::new(Rect::ZERO)
                    .affects_channels(PAINT.into_set())
                    .build(),
            ),
            corner_radius: registry.register(
                "CornerRadius",
                PropertyMetadataBuilder::new(0.0_f64)
                    .affects_channels(PAINT.into_set())
                    .build(),
            ),
            layout: registry.register(
                "Layout",
                PropertyMetadataBuilder::new(taffy::Style::default())
                    .affects_channels(LAYOUT.into_set())
                    .build(),
            ),
        }
    }
}
