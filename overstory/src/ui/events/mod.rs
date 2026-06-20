// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Routed input dispatch and host event synthesis for `Ui`.
//!
//! Event authoring has three layers:
//!
//! - Controls expose semantic callbacks such as `Button::on_activate` and
//!   `Slider::on_value_changed`. App code should use these when responding to
//!   control intent.
//! - [`Ui::events`] exposes advanced raw routed input for app code that needs
//!   route phases, handled-event observation, pointer capture, or custom
//!   gestures.
//! - [`Widget`](crate::Widget) hooks implement toolkit-side control behavior.
//!   Built-in controls translate raw routed input into semantic state changes
//!   there, rather than registering a second hidden raw-handler lane.
//!
//! Route selection and capture/target/bubble sequencing are delegated to
//! `understory_responder` through `ui::responder`; this module owns the
//! Overstory-specific work performed at each dispatch step.

mod click_drag_scroll;
mod dispatch;
mod focus;
mod keyboard;
mod pointer;
mod registration;

pub(in crate::ui) use pointer::PointerRouteData;
pub use registration::RoutedEvents;
