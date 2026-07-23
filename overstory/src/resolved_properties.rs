// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use understory_property::Property;

use crate::AccessibilityContext;
use crate::behavior::{MeasureContext, PresentContext, WidgetEventContext};
use crate::element::{Element, ElementId};
use crate::properties::BuiltInProperties;
use crate::ui::Ui;

pub(crate) trait ResolvedPropertySource {
    fn properties(&self) -> &BuiltInProperties;

    /// Returns a live element by id.
    fn element(&self, id: ElementId) -> Option<&Element>;

    /// Resolves a typed property for `element` through this source's cascade,
    /// match state, theme, inheritance lookup, and local/animation stores.
    fn resolve_value<T>(&self, element: &Element, property: Property<T>) -> T
    where
        T: Clone + 'static;
}

impl ResolvedPropertySource for PresentContext<'_> {
    fn properties(&self) -> &BuiltInProperties {
        self.properties
    }

    fn element(&self, id: ElementId) -> Option<&Element> {
        PresentContext::element(self, id)
    }

    fn resolve_value<T>(&self, element: &Element, property: Property<T>) -> T
    where
        T: Clone + 'static,
    {
        PresentContext::get_value(self, element, property)
    }
}

impl ResolvedPropertySource for MeasureContext<'_> {
    fn properties(&self) -> &BuiltInProperties {
        self.properties
    }

    fn element(&self, id: ElementId) -> Option<&Element> {
        MeasureContext::element(self, id)
    }

    fn resolve_value<T>(&self, element: &Element, property: Property<T>) -> T
    where
        T: Clone + 'static,
    {
        MeasureContext::get_value(self, element, property)
    }
}

impl ResolvedPropertySource for WidgetEventContext<'_> {
    fn properties(&self) -> &BuiltInProperties {
        self.ui().properties()
    }

    fn element(&self, id: ElementId) -> Option<&Element> {
        self.ui().get(id)
    }

    fn resolve_value<T>(&self, element: &Element, property: Property<T>) -> T
    where
        T: Clone + 'static,
    {
        self.ui().resolve_value_for(element, property)
    }
}

impl ResolvedPropertySource for AccessibilityContext<'_> {
    fn properties(&self) -> &BuiltInProperties {
        self.properties()
    }

    fn element(&self, id: ElementId) -> Option<&Element> {
        AccessibilityContext::element(self, id)
    }

    fn resolve_value<T>(&self, element: &Element, property: Property<T>) -> T
    where
        T: Clone + 'static,
    {
        self.get_value(element, property)
    }
}

impl ResolvedPropertySource for Ui {
    fn properties(&self) -> &BuiltInProperties {
        self.properties()
    }

    fn element(&self, id: ElementId) -> Option<&Element> {
        self.get(id)
    }

    fn resolve_value<T>(&self, element: &Element, property: Property<T>) -> T
    where
        T: Clone + 'static,
    {
        self.resolve_value_for(element, property)
    }
}
