// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Adapters from timeline samples into dependency-property animation slots.
//!
//! The timeline runtime owns temporal sampling and knows only about erased
//! targets. This module owns the per-property lowering step: given a sampled
//! value for a `PropertyId`, write or clear the corresponding animation-layer
//! value on an element. It also owns the matching read step used by transition
//! planning: given an element and property, extract the current effective value
//! as a timeline sample when the property is animatable. Most properties can
//! use the native adapter; mapped properties, such as animating the solid color
//! inside `Surface.Background`, can register a custom adapter.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::Any;

use hashbrown::HashMap;
use peniko::Brush;
use understory_property::{DependencyObject, Property, PropertyId};
use understory_timing::TimerInstant;

use crate::element::{Element, ElementId};
use crate::motion::{Animatable, PresentationValue, PresentationValueKind};
use crate::timeline_runtime::{
    ChangeCause, PresentationRuntimeValue, TimelineRuntime, TimelineTrackKey,
};
use crate::ui::Ui;

type ReadTarget = fn(&Ui, ElementId, PropertyId) -> Option<PresentationValue>;
type SetSample = fn(&mut Element, PropertyId, PresentationValue) -> bool;
type ClearSample = fn(&mut Element, PropertyId) -> bool;
type ReadNativeTarget = fn(&Ui, ElementId, PropertyId) -> Option<Box<dyn Any>>;
type SameNativeValue = fn(&dyn Any, &dyn Any) -> Option<bool>;
type SetNativeTarget = fn(
    &mut TimelineRuntime,
    TimelineTrackKey,
    Box<dyn Any>,
    TimerInstant,
    ChangeCause,
    Option<PresentationValueKind>,
) -> Option<bool>;
type SetNativeSample = fn(&mut Element, PropertyId, &dyn Any) -> bool;

/// Per-property adapter for timeline presentation samples.
#[derive(Clone, Copy)]
pub(crate) struct TimelinePropertyAdapter {
    kind: PresentationValueKind,
    read_target: ReadTarget,
    set_sample: SetSample,
    clear_sample: ClearSample,
}

impl TimelinePropertyAdapter {
    fn presentation_value<T>() -> Self
    where
        T: PresentationRuntimeValue,
    {
        Self {
            kind: T::KIND,
            read_target: read_presentation_target::<T>,
            set_sample: set_presentation_sample::<T>,
            clear_sample: clear_property_animation_sample::<T>,
        }
    }

    fn brush_color() -> Self {
        Self {
            kind: PresentationValueKind::Color,
            read_target: read_brush_color_target,
            set_sample: set_brush_color_sample,
            clear_sample: clear_brush_color_sample,
        }
    }

    fn optional_solid_brush_color() -> Self {
        Self {
            kind: PresentationValueKind::Color,
            read_target: read_optional_solid_brush_color_target,
            set_sample: set_optional_solid_brush_color_sample,
            clear_sample: clear_optional_solid_brush_color_sample,
        }
    }

    /// Returns the presentation sample kind this adapter accepts.
    #[must_use]
    pub(crate) const fn kind(self) -> PresentationValueKind {
        self.kind
    }

    /// Reads the current effective property target as an presentation sample.
    #[must_use]
    pub(crate) fn target_value(
        self,
        ui: &Ui,
        element: ElementId,
        property: PropertyId,
    ) -> Option<PresentationValue> {
        (self.read_target)(ui, element, property)
    }

    /// Applies a sampled timeline value to `element`'s animation layer.
    pub(crate) fn set_sample(
        self,
        element: &mut Element,
        property: PropertyId,
        value: PresentationValue,
    ) -> bool {
        (self.set_sample)(element, property, value)
    }

    /// Clears `property` from `element`'s animation layer.
    pub(crate) fn clear_sample(self, element: &mut Element, property: PropertyId) -> bool {
        (self.clear_sample)(element, property)
    }
}

impl core::fmt::Debug for TimelinePropertyAdapter {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TimelinePropertyAdapter")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

/// Value read from an animatable dependency-property target.
pub(crate) enum TimelineTargetValue {
    Presentation(PresentationValue),
    Native(Box<dyn Any>),
}

/// Unified adapter for one dependency-property animation target.
#[derive(Clone, Copy, Debug)]
pub(crate) enum TimelineTargetAdapter {
    Presentation(TimelinePropertyAdapter),
    Native(NativeTimelinePropertyAdapter),
}

impl TimelineTargetAdapter {
    /// Reads the current effective property target.
    #[must_use]
    pub(crate) fn target_value(
        self,
        ui: &Ui,
        element: ElementId,
        property: PropertyId,
    ) -> Option<TimelineTargetValue> {
        match self {
            Self::Presentation(adapter) => adapter
                .target_value(ui, element, property)
                .map(TimelineTargetValue::Presentation),
            Self::Native(adapter) => adapter
                .target_value(ui, element, property)
                .map(TimelineTargetValue::Native),
        }
    }

    /// Returns whether two target values are close enough to skip transition planning.
    #[must_use]
    pub(crate) fn same_value(
        self,
        left: &TimelineTargetValue,
        right: &TimelineTargetValue,
    ) -> bool {
        match (self, left, right) {
            (
                Self::Presentation(_),
                TimelineTargetValue::Presentation(left),
                TimelineTargetValue::Presentation(right),
            ) => left == right,
            (
                Self::Native(adapter),
                TimelineTargetValue::Native(left),
                TimelineTargetValue::Native(right),
            ) => adapter
                .same_value(left.as_ref(), right.as_ref())
                .unwrap_or(false),
            _ => false,
        }
    }
}

/// Per-property adapter for native timeline samples.
#[derive(Clone, Copy)]
pub(crate) struct NativeTimelinePropertyAdapter {
    read_target: ReadNativeTarget,
    same_value: SameNativeValue,
    set_target: SetNativeTarget,
    set_sample: SetNativeSample,
    clear_sample: ClearSample,
}

impl NativeTimelinePropertyAdapter {
    fn native<T>() -> Self
    where
        T: Animatable,
    {
        Self {
            read_target: read_native_target::<T>,
            same_value: same_native_value::<T>,
            set_target: set_native_target::<T>,
            set_sample: set_native_sample::<T>,
            clear_sample: clear_property_animation_sample::<T>,
        }
    }

    /// Reads the current effective property target as a native erased value.
    #[must_use]
    pub(crate) fn target_value(
        self,
        ui: &Ui,
        element: ElementId,
        property: PropertyId,
    ) -> Option<Box<dyn Any>> {
        (self.read_target)(ui, element, property)
    }

    /// Returns whether two native erased values are close enough to be the same target.
    #[must_use]
    pub(crate) fn same_value(self, left: &dyn Any, right: &dyn Any) -> Option<bool> {
        (self.same_value)(left, right)
    }

    /// Writes a native target value into the timeline runtime.
    pub(crate) fn set_target(
        self,
        runtime: &mut TimelineRuntime,
        key: TimelineTrackKey,
        value: Box<dyn Any>,
        now: TimerInstant,
        cause: ChangeCause,
        value_kind: Option<PresentationValueKind>,
    ) -> Option<bool> {
        (self.set_target)(runtime, key, value, now, cause, value_kind)
    }

    /// Applies a native sampled timeline value to `element`'s animation layer.
    pub(crate) fn set_sample(
        self,
        element: &mut Element,
        property: PropertyId,
        value: &dyn Any,
    ) -> bool {
        (self.set_sample)(element, property, value)
    }

    /// Clears `property` from `element`'s animation layer.
    pub(crate) fn clear_sample(self, element: &mut Element, property: PropertyId) -> bool {
        (self.clear_sample)(element, property)
    }
}

impl core::fmt::Debug for NativeTimelinePropertyAdapter {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NativeTimelinePropertyAdapter")
            .finish_non_exhaustive()
    }
}

/// Registry of properties that can receive timeline samples.
#[derive(Debug, Default)]
pub(crate) struct TimelinePropertyRegistry {
    adapters: HashMap<PropertyId, TimelinePropertyAdapter>,
    native_adapters: HashMap<PropertyId, NativeTimelinePropertyAdapter>,
    properties: Vec<PropertyId>,
    native_properties: Vec<PropertyId>,
}

impl TimelinePropertyRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Registers a property whose animation sample type is the property type.
    pub(crate) fn register<T>(&mut self, property: Property<T>)
    where
        T: PresentationRuntimeValue,
    {
        self.insert(
            property.id(),
            TimelinePropertyAdapter::presentation_value::<T>(),
        );
    }

    /// Registers a property whose native animation sample type is the property type.
    pub(crate) fn register_native<T>(&mut self, property: Property<T>)
    where
        T: Animatable,
    {
        if !self.native_adapters.contains_key(&property.id()) {
            self.native_properties.push(property.id());
            self.native_properties.sort_unstable();
        }
        self.native_adapters
            .insert(property.id(), NativeTimelinePropertyAdapter::native::<T>());
    }

    /// Registers a `Brush` property that animates its solid color value.
    pub(crate) fn register_brush_color(&mut self, property: Property<Brush>) {
        self.insert(property.id(), TimelinePropertyAdapter::brush_color());
    }

    /// Registers an `Option<Brush>` property that animates solid brush color.
    pub(crate) fn register_solid_brush_color(&mut self, property: Property<Option<Brush>>) {
        self.insert(
            property.id(),
            TimelinePropertyAdapter::optional_solid_brush_color(),
        );
    }

    /// Returns the adapter for `property`, if the property is timeline-sampled.
    #[must_use]
    pub(crate) fn adapter(&self, property: PropertyId) -> Option<TimelinePropertyAdapter> {
        self.adapters.get(&property).copied()
    }

    /// Returns the native adapter for `property`, if the property accepts native value samples.
    #[must_use]
    pub(crate) fn native_adapter(
        &self,
        property: PropertyId,
    ) -> Option<NativeTimelinePropertyAdapter> {
        self.native_adapters.get(&property).copied()
    }

    /// Returns registered dependency-property animation target adapters.
    pub(crate) fn target_adapters(
        &self,
    ) -> impl Iterator<Item = (PropertyId, TimelineTargetAdapter)> + '_ {
        let presentation = self.properties.iter().copied().filter_map(|property| {
            self.adapter(property)
                .map(|adapter| (property, TimelineTargetAdapter::Presentation(adapter)))
        });
        let native = self
            .native_properties
            .iter()
            .copied()
            .filter(|property| !self.adapters.contains_key(property))
            .filter_map(|property| {
                self.native_adapter(property)
                    .map(|adapter| (property, TimelineTargetAdapter::Native(adapter)))
            });
        presentation.chain(native)
    }

    fn insert(&mut self, property: PropertyId, adapter: TimelinePropertyAdapter) {
        if !self.adapters.contains_key(&property) {
            self.properties.push(property);
            self.properties.sort_unstable();
        }
        self.adapters.insert(property, adapter);
    }
}

fn read_presentation_target<T>(
    ui: &Ui,
    element: ElementId,
    property: PropertyId,
) -> Option<PresentationValue>
where
    T: PresentationRuntimeValue,
{
    let element = ui.get(element)?;
    Some(
        ui.resolve_value_for(element, Property::<T>::from_id(property))
            .into_presentation_value(),
    )
}

fn read_brush_color_target(
    ui: &Ui,
    element: ElementId,
    property: PropertyId,
) -> Option<PresentationValue> {
    let element = ui.get(element)?;
    match ui.resolve_value_for(element, Property::<Brush>::from_id(property)) {
        Brush::Solid(color) => Some(PresentationValue::Color(color)),
        Brush::Gradient(_) | Brush::Image(_) => None,
    }
}

fn read_optional_solid_brush_color_target(
    ui: &Ui,
    element: ElementId,
    property: PropertyId,
) -> Option<PresentationValue> {
    let element = ui.get(element)?;
    match ui.resolve_value_for(element, Property::<Option<Brush>>::from_id(property)) {
        Some(Brush::Solid(color)) => Some(PresentationValue::Color(color)),
        Some(Brush::Gradient(_) | Brush::Image(_)) | None => None,
    }
}

fn set_presentation_sample<T>(
    element: &mut Element,
    property: PropertyId,
    value: PresentationValue,
) -> bool
where
    T: PresentationRuntimeValue,
{
    let Some(value) = T::from_presentation_value(value) else {
        return false;
    };
    element
        .property_store_mut()
        .set_animation(Property::<T>::from_id(property), value);
    true
}

fn set_native_sample<T>(element: &mut Element, property: PropertyId, value: &dyn Any) -> bool
where
    T: Animatable,
{
    let Some(value) = value.downcast_ref::<T>() else {
        return false;
    };
    element
        .property_store_mut()
        .set_animation(Property::<T>::from_id(property), value.clone());
    true
}

fn read_native_target<T>(ui: &Ui, element: ElementId, property: PropertyId) -> Option<Box<dyn Any>>
where
    T: Animatable,
{
    let element = ui.get(element)?;
    Some(Box::new(ui.resolve_value_for(
        element,
        Property::<T>::from_id(property),
    )))
}

fn same_native_value<T>(left: &dyn Any, right: &dyn Any) -> Option<bool>
where
    T: Animatable,
{
    Some(
        left.downcast_ref::<T>()?
            .is_close(right.downcast_ref::<T>()?),
    )
}

fn set_native_target<T>(
    runtime: &mut TimelineRuntime,
    key: TimelineTrackKey,
    value: Box<dyn Any>,
    now: TimerInstant,
    cause: ChangeCause,
    value_kind: Option<PresentationValueKind>,
) -> Option<bool>
where
    T: Animatable,
{
    let value = *value.downcast::<T>().ok()?;
    runtime.set_native_target_value(key, value, now, cause, value_kind)
}

fn clear_property_animation_sample<T>(element: &mut Element, property: PropertyId) -> bool
where
    T: Clone + 'static,
{
    element
        .property_store_mut()
        .clear_animation(Property::<T>::from_id(property))
}

fn set_brush_color_sample(
    element: &mut Element,
    property: PropertyId,
    value: PresentationValue,
) -> bool {
    let PresentationValue::Color(color) = value else {
        return false;
    };
    element
        .property_store_mut()
        .set_animation(Property::<Brush>::from_id(property), Brush::Solid(color));
    true
}

fn clear_brush_color_sample(element: &mut Element, property: PropertyId) -> bool {
    element
        .property_store_mut()
        .clear_animation(Property::<Brush>::from_id(property))
}

fn set_optional_solid_brush_color_sample(
    element: &mut Element,
    property: PropertyId,
    value: PresentationValue,
) -> bool {
    let PresentationValue::Color(color) = value else {
        return false;
    };
    element.property_store_mut().set_animation(
        Property::<Option<Brush>>::from_id(property),
        Some(Brush::Solid(color)),
    );
    true
}

fn clear_optional_solid_brush_color_sample(element: &mut Element, property: PropertyId) -> bool {
    element
        .property_store_mut()
        .clear_animation(Property::<Option<Brush>>::from_id(property))
}

#[cfg(test)]
mod tests {
    use peniko::Color;
    use understory_property::Property;

    use super::*;
    use crate::element::ElementId;

    #[test]
    fn native_adapter_writes_and_clears_animation_value() {
        let property = Property::<f32>::from_id(PropertyId::new(7));
        let mut registry = TimelinePropertyRegistry::new();
        registry.register(property);
        let adapter = registry.adapter(property.id()).unwrap();
        let mut element = Element::new(ElementId::new(1, 0), None, None, None, None);

        assert!(adapter.set_sample(
            &mut element,
            property.id(),
            PresentationValue::Scalar32(0.5)
        ));
        assert_eq!(
            element.property_store().get_animation(property).copied(),
            Some(0.5)
        );

        assert!(adapter.clear_sample(&mut element, property.id()));
        assert_eq!(element.property_store().get_animation(property), None);
    }

    #[test]
    fn solid_brush_adapter_maps_color_samples_to_brush_values() {
        let property = Property::<Option<Brush>>::from_id(PropertyId::new(9));
        let mut registry = TimelinePropertyRegistry::new();
        registry.register_solid_brush_color(property);
        let adapter = registry.adapter(property.id()).unwrap();
        let mut element = Element::new(ElementId::new(1, 0), None, None, None, None);
        let color = Color::from_rgba8(0x20, 0x40, 0x80, 0xff);

        assert!(adapter.set_sample(&mut element, property.id(), PresentationValue::Color(color)));
        assert_eq!(
            element.property_store().get_animation(property),
            Some(&Some(Brush::Solid(color)))
        );
    }
}
