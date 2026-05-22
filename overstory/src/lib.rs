// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Overstory: a WPF / `WinUI` 3-inspired UI toolkit built on
//! `understory` and `imaging`.
//!
//! Each control is a struct that implements [`AppendSpec`] (the
//! builder + entry point) and produces a [`Widget`] instance that
//! lives on the element. Application code uses
//! [`Ui::append`](crate::Ui::append) to insert controls into the
//! tree:
//!
//! ```ignore
//! ui.append(parent, Button::new("Click me").on_click(|ui, _| { … }));
//! ui.append(parent, ToggleSwitch::new().checked(true));
//! ui.append(parent, Slider::new().value(0.4));
//! ```
//!
//! `Ui::append` is the only public path for adding controls;
//! `insert_child` lives on the toolkit-internal side for templates
//! building their own parts. `set_local` remains public for power
//! users (bindings, dynamic theming, custom controls), but isn't
//! the typical app-level entry point.

extern crate alloc;

mod behavior;
pub mod channels;
mod controls;
mod element;
mod generic;
pub mod layouts;
mod lookup;
mod properties;
pub mod pseudos;
pub mod selectors;
mod template;
mod text;
pub mod theme;
mod ui;
mod widget;

pub use behavior::{KnownDimensions, MeasureConstraints, MeasureContext, PresentContext};
pub use controls::button::{
    Button, ButtonWidget, PART_BUTTON_LABEL, SLOT_BUTTON_CONTENT, TYPE_BUTTON,
};
pub use controls::panel::{Panel, PanelWidget};
pub use controls::slider::{
    CLASS_SLIDER_MAJOR_TICK, CLASS_SLIDER_MINOR_TICK, PART_SLIDER_AXIS, PART_SLIDER_THUMB,
    PART_SLIDER_TICK, PART_SLIDER_TICK_LABEL, PART_SLIDER_TRACK, SLOT_SLIDER_AXIS,
    SLOT_SLIDER_THUMB, SLOT_SLIDER_TICK, SLOT_SLIDER_TICK_LABEL, SLOT_SLIDER_TRACK, Slider,
    SliderWidget, TYPE_SLIDER, set_slider_value,
};
pub use controls::spinner::{Spinner, SpinnerWidget, TYPE_SPINNER};
pub use controls::text::{TYPE_TEXT, Text, TextWidget};
pub use controls::toggle::{
    PART_TOGGLE_THUMB, SLOT_TOGGLE_THUMB, TYPE_TOGGLE, ToggleSwitch, ToggleWidget, toggle_checked,
};
pub use element::{Element, ElementId};
pub use generic::present_surface;
pub use layouts::Layout;
pub use lookup::{UiResolveLookup, UiStoreLookup};
pub use properties::BuiltInProperties;
pub use template::{
    Template, TemplateBinding, TemplateBuiltInProperty, TemplateContext, TemplateInstance,
    TemplateItemKey, TemplateNode, TemplatePart, TemplateSlot, TemplateSpec, TemplateWidgetFactory,
};
pub use text::TextEngine;
pub use ui::TimerEvent;
pub use ui::{
    ClickEvent, ControlState, DragEvent, KeyDispatchResult, KeyEvent, RoutedDispatchResult, Ui,
};
pub use widget::{AppendSpec, TYPE_GENERIC, Widget};

// Re-export the responder's `Phase` so callers can read
// `ClickEvent::phase` / `KeyEvent::phase` etc. without depending
// on `understory_responder` directly.
pub use understory_focus::Navigation;
pub use understory_responder::types::Phase;
pub use understory_timing::{TimerDuration, TimerId, TimerInstant};
