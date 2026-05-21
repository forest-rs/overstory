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
mod button;
pub mod channels;
mod element;
mod generic;
pub mod layouts;
mod lookup;
mod panel;
mod properties;
pub mod pseudos;
mod slider;
mod spinner;
mod template;
mod text;
mod text_behavior;
pub mod theme;
mod toggle;
mod ui;
mod widget;

pub use behavior::{KnownDimensions, MeasureConstraints, MeasureContext, PresentContext};
pub use button::{Button, ButtonWidget, TYPE_BUTTON};
pub use element::{Element, ElementId};
pub use generic::present_surface;
pub use lookup::UiStoreLookup;
pub use panel::{Panel, PanelWidget};
pub use properties::BuiltInProperties;
pub use slider::{
    Slider, SliderThumbWidget, SliderTrackWidget, SliderWidget, TYPE_SLIDER, TYPE_SLIDER_THUMB,
    TYPE_SLIDER_TRACK, set_slider_value,
};
pub use spinner::{Spinner, SpinnerWidget, TYPE_SPINNER};
pub use template::Template;
pub use text::TextEngine;
pub use text_behavior::{TYPE_TEXT, Text, TextWidget};
pub use toggle::{
    TYPE_TOGGLE, TYPE_TOGGLE_THUMB, ToggleSwitch, ToggleThumbWidget, ToggleWidget, toggle_checked,
};
pub use ui::TimerEvent;
pub use ui::{ClickEvent, DragEvent, KeyEvent, Ui};
pub use widget::{AppendSpec, TYPE_GENERIC, Widget};

// Re-export the responder's `Phase` so callers can read
// `ClickEvent::phase` / `KeyEvent::phase` etc. without depending
// on `understory_responder` directly.
pub use understory_focus::Navigation;
pub use understory_responder::types::Phase;
pub use understory_timing::{TimerDuration, TimerId, TimerInstant};

// Re-export `taffy` so consumers don't have to add a direct
// dependency for the `Layout` property's style type.
pub use taffy;
