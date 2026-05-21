// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Built-in dependency properties that every Overstory element understands.

use alloc::string::String;

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

    /// Textual content for elements that render strings (e.g.
    /// `ELEMENT_KIND_TEXT`). Default `String::new()`. Writes mark both
    /// `LAYOUT` (text size depends on content) and `PAINT`.
    pub content: Property<String>,

    /// Foreground brush used to paint text. Default solid black.
    pub foreground: Property<Brush>,

    /// Font size in pixels per em. Default `14.0`.
    pub font_size: Property<f64>,

    /// Border brush. `None` (the default) means the element has no
    /// border. Stroked at [`Self::border_width`] around the same
    /// rounded rect [`Self::background`] fills.
    pub border_brush: Property<Option<Brush>>,

    /// Border stroke width in logical pixels. Default `0.0` — the
    /// border isn't drawn when either the brush is `None` or the
    /// width is zero.
    pub border_width: Property<f64>,
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
            content: registry.register(
                "Content",
                PropertyMetadataBuilder::new(String::new())
                    .affects_channels(LAYOUT.into_set() | PAINT.into_set())
                    .build(),
            ),
            foreground: registry.register(
                "Foreground",
                PropertyMetadataBuilder::new(Brush::Solid(peniko::Color::from_rgba8(
                    0, 0, 0, 0xff,
                )))
                .affects_channels(PAINT.into_set())
                .build(),
            ),
            font_size: registry.register(
                "FontSize",
                PropertyMetadataBuilder::new(14.0_f64)
                    .affects_channels(LAYOUT.into_set() | PAINT.into_set())
                    .build(),
            ),
            border_brush: registry.register(
                "BorderBrush",
                PropertyMetadataBuilder::<Option<Brush>>::new(None)
                    .affects_channels(PAINT.into_set())
                    .build(),
            ),
            border_width: registry.register(
                "BorderWidth",
                PropertyMetadataBuilder::new(0.0_f64)
                    .affects_channels(PAINT.into_set())
                    .build(),
            ),
        }
    }
}
