// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Built-in dependency properties that every Overstory element understands.
//!
//! Roadmap:
//! - Keep built-in properties small and broadly semantic. Control-specific
//!   state should become built-in only when multiple controls need the same
//!   property, invalidation, and styling behavior. See ticket `ove-x5ci`.
//! - Pair semantic state with pseudoclasses deliberately: property values are
//!   data, pseudoclasses are selector state, and controls must keep them in
//!   sync.
//! - Add richer text, accessibility, automation, range, and content properties
//!   only with clear invalidation metadata and host integration expectations.
//! - Avoid turning this into a mirror of WPF dependency properties; Overstory
//!   should expose calm primitives backed by Understory registries.

use alloc::string::String;

use kurbo::Rect;
use peniko::Brush;
use understory_presentation::TextWrapMode;
use understory_property::{Property, PropertyMetadataBuilder, PropertyRegistry};

use crate::channels::{LAYOUT, PAINT};
use crate::layouts::Layout;

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

    /// Bounds in viewport coordinates the element paints into. Layout writes
    /// this after computing the frame. Default [`Rect::ZERO`] — a zero-area
    /// element paints nothing.
    pub bounds: Property<Rect>,

    /// Corner radius for the background fill. Default `0.0` (rectangular).
    pub corner_radius: Property<f64>,

    /// Layout contribution for this element. Writes mark the
    /// `LAYOUT` channel so the next `Ui::compute_layout` picks them
    /// up.
    pub layout: Property<Layout>,

    /// Textual content for elements that render strings (e.g. [`crate::Text`]).
    /// Default `String::new()`. Writes mark both `LAYOUT` (text size depends on
    /// content) and `PAINT`.
    pub content: Property<String>,

    /// Foreground brush used to paint text. Default solid black.
    pub foreground: Property<Brush>,

    /// Font size in pixels per em. Default `14.0`.
    pub font_size: Property<f64>,

    /// Text line wrapping policy. Default [`TextWrapMode::NoWrap`].
    /// Writes mark `LAYOUT`, which cascades through paint because wrapping
    /// can change both measured size and final glyph placement.
    pub text_wrap_mode: Property<TextWrapMode>,

    /// Border brush. `None` (the default) means the element has no
    /// border. Stroked at [`Self::border_width`] around the same
    /// rounded rect [`Self::background`] fills.
    pub border_brush: Property<Option<Brush>>,

    /// Border stroke width in logical pixels. Default `0.0` — the
    /// border isn't drawn when either the brush is `None` or the
    /// width is zero.
    pub border_width: Property<f64>,

    /// Boolean toggled state for two-state controls (`ToggleSwitch`,
    /// `CheckBox`, …). Default `false`. Writers should pair the value
    /// change with adding / removing the `:checked` pseudoclass so
    /// the cascade reflects the new state.
    pub is_checked: Property<bool>,

    /// Whether an element can participate in control interaction. Default
    /// `true`. Controls should keep this in sync with the `:disabled`
    /// pseudoclass through [`crate::Ui::set_enabled`].
    pub is_enabled: Property<bool>,

    /// Whether a text-editing control allows content mutation. Default
    /// `false`. Read-only controls remain focusable and selectable but should
    /// reject text insertion and deletion.
    pub is_read_only: Property<bool>,

    /// Numeric value for value-bearing controls (`Slider`,
    /// `ProgressBar`, …). Default `0.0`. The semantic range is
    /// `[Self::minimum, Self::maximum]`; consumers should clamp
    /// writes that go outside it.
    pub value: Property<f64>,

    /// Small keyboard increment for value-bearing controls. `Slider` uses this
    /// for unmodified arrow keys. Default `0.0`, which lets the control derive
    /// a range-relative increment.
    pub small_change: Property<f64>,

    /// Large keyboard increment for value-bearing controls. `Slider` uses this
    /// for `PageUp` / `PageDown` and Shift-modified arrow keys. Default `0.0`,
    /// which lets the control derive a range-relative increment.
    pub large_change: Property<f64>,

    /// Lower bound for [`Self::value`]. Default `0.0`.
    pub minimum: Property<f64>,

    /// Upper bound for [`Self::value`]. Default `1.0`.
    pub maximum: Property<f64>,
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
                PropertyMetadataBuilder::new(Layout::default())
                    .affects_channels(LAYOUT.into_set())
                    .build(),
            ),
            content: registry.register(
                "Content",
                PropertyMetadataBuilder::new(String::new())
                    // PAINT cascades from LAYOUT via the tracker's
                    // channel cascade — no need to declare it
                    // explicitly here.
                    .affects_channels(LAYOUT.into_set())
                    .build(),
            ),
            foreground: registry.register(
                "Foreground",
                PropertyMetadataBuilder::new(Brush::Solid(peniko::Color::from_rgba8(
                    0, 0, 0, 0xff,
                )))
                .affects_channels(PAINT.into_set())
                // Inherits so a foreground resolved on an ancestor
                // control reaches text descendants. Overstory's
                // presentation path performs style-aware inheritance
                // for this; the raw property-store inheritance helper
                // only sees animation/local ancestor values.
                .inherits(true)
                .build(),
            ),
            font_size: registry.register(
                "FontSize",
                PropertyMetadataBuilder::new(14.0_f64)
                    .affects_channels(LAYOUT.into_set())
                    // Typography should flow from controls to generated text
                    // parts and ordinary descendants unless a child overrides it.
                    .inherits(true)
                    .build(),
            ),
            text_wrap_mode: registry.register(
                "TextWrapMode",
                PropertyMetadataBuilder::new(TextWrapMode::NoWrap)
                    .affects_channels(LAYOUT.into_set())
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
            is_checked: registry.register(
                "IsChecked",
                PropertyMetadataBuilder::new(false)
                    // No direct paint impact — state lives in the
                    // pseudoclass. The control's click handler is
                    // responsible for keeping `:checked` in sync.
                    .build(),
            ),
            is_enabled: registry.register(
                "IsEnabled",
                PropertyMetadataBuilder::new(true)
                    // No direct visual impact. `Ui::set_enabled`
                    // updates `:disabled`, and the selector state
                    // drives style invalidation.
                    .build(),
            ),
            is_read_only: registry
                .register("IsReadOnly", PropertyMetadataBuilder::new(false).build()),
            value: registry.register(
                "Value",
                PropertyMetadataBuilder::new(0.0_f64)
                    // `Value` changes drive layout updates on
                    // controls like Slider (thumb position) plus
                    // the natural PAINT/COMPOSE cascade.
                    .affects_channels(LAYOUT.into_set())
                    .build(),
            ),
            small_change: registry
                .register("SmallChange", PropertyMetadataBuilder::new(0.0_f64).build()),
            large_change: registry
                .register("LargeChange", PropertyMetadataBuilder::new(0.0_f64).build()),
            minimum: registry.register(
                "Minimum",
                PropertyMetadataBuilder::new(0.0_f64)
                    .affects_channels(LAYOUT.into_set())
                    .build(),
            ),
            maximum: registry.register(
                "Maximum",
                PropertyMetadataBuilder::new(1.0_f64)
                    .affects_channels(LAYOUT.into_set())
                    .build(),
            ),
        }
    }
}
