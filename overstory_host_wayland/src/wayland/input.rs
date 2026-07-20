// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Seat input: pointer, keyboard, and touch dispatch.
//!
//! The seat's devices are acquired and released as the `wl_seat` advertises and
//! withdraws its capabilities. Each device's events are normalized through the
//! shared [`SeatReducer`] into `ui-events`, dispatched into the [`UiHost`], and,
//! when the UI reacts, turned into a frame request that wakes an idle surface to
//! redraw. Event times are stamped from the host's active clock — the
//! compositor's presentation clock when offered, otherwise monotonic — so input
//! shares one timeline with the frame ticks that clock also stamps.
//!
//! [`SeatReducer`]: ui_events_wayland::seat::SeatReducer
//! [`UiHost`]: overstory_host::UiHost

use overstory::CursorIcon;
use ui_events_wayland::seat::SeatEventTranslation;
use wayland_client::protocol::{wl_keyboard, wl_pointer, wl_seat, wl_touch};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::{
    self, Shape, WpCursorShapeDeviceV1,
};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_manager_v1::{
    self, WpCursorShapeManagerV1,
};

use super::window::App;

impl App {
    /// Dispatches normalized seat events into the host and wakes the surface.
    ///
    /// Each translation is fed to the host; when one changes visible UI state,
    /// its frame demand is requested and an idle surface is woken with a frame
    /// callback so the change is drawn. When the UI's resulting cursor intent
    /// changed, the new shape is applied.
    fn dispatch_seat_translations(
        &mut self,
        translations: impl IntoIterator<Item = SeatEventTranslation>,
        qh: &QueueHandle<Self>,
    ) {
        let mut needs_redraw = false;
        let mut cursor = None;
        let now = self.now().ticks();
        if let Some(host) = self.host.as_mut() {
            for translation in translations {
                let result = match translation {
                    SeatEventTranslation::Pointer(event) => {
                        host.dispatch_pointer_event_at(&event, now)
                    }
                    SeatEventTranslation::Keyboard(event) => host.dispatch_key_at(&event, now),
                };
                if result.needs_redraw {
                    host.request_frame(result.frame_demand);
                    needs_redraw = true;
                }
            }
            cursor = host.cursor_update();
        }
        if let Some(cursor) = cursor {
            self.cursor_icon = cursor;
            self.apply_cursor_shape();
        }
        if needs_redraw {
            self.ensure_frame_callback(qh);
        }
    }

    /// Sets the pointer's cursor to the UI's current intent.
    ///
    /// A cursor can only be set while the pointer is inside the surface — the
    /// request must present the latest enter serial, which the pointer reducer
    /// tracks — and only when the compositor offers `cursor-shape-v1`;
    /// otherwise this leaves the cursor untouched.
    fn apply_cursor_shape(&self) {
        let serial = self
            .seat_reducer
            .pointer()
            .and_then(|pointer| pointer.enter_serial());
        if let (Some(device), Some(serial)) = (self.cursor_shape_device.as_ref(), serial) {
            device.set_shape(serial, cursor_shape(self.cursor_icon, device.version()));
        }
    }
}

/// Maps a UI cursor intent onto the `cursor-shape-v1` shape a device of
/// `device_version` accepts.
///
/// Both enums carry the CSS cursor names, so the mapping is one-to-one except
/// for the two non-CSS shapes added in protocol version 2, which fall back to
/// their nearest core shape on a version-1 device.
fn cursor_shape(icon: CursorIcon, device_version: u32) -> Shape {
    match icon {
        CursorIcon::ContextMenu => Shape::ContextMenu,
        CursorIcon::Help => Shape::Help,
        CursorIcon::Pointer => Shape::Pointer,
        CursorIcon::Progress => Shape::Progress,
        CursorIcon::Wait => Shape::Wait,
        CursorIcon::Cell => Shape::Cell,
        CursorIcon::Crosshair => Shape::Crosshair,
        CursorIcon::Text => Shape::Text,
        CursorIcon::VerticalText => Shape::VerticalText,
        CursorIcon::Alias => Shape::Alias,
        CursorIcon::Copy => Shape::Copy,
        CursorIcon::Move => Shape::Move,
        CursorIcon::NoDrop => Shape::NoDrop,
        CursorIcon::NotAllowed => Shape::NotAllowed,
        CursorIcon::Grab => Shape::Grab,
        CursorIcon::Grabbing => Shape::Grabbing,
        CursorIcon::EResize => Shape::EResize,
        CursorIcon::NResize => Shape::NResize,
        CursorIcon::NeResize => Shape::NeResize,
        CursorIcon::NwResize => Shape::NwResize,
        CursorIcon::SResize => Shape::SResize,
        CursorIcon::SeResize => Shape::SeResize,
        CursorIcon::SwResize => Shape::SwResize,
        CursorIcon::WResize => Shape::WResize,
        CursorIcon::EwResize => Shape::EwResize,
        CursorIcon::NsResize => Shape::NsResize,
        CursorIcon::NeswResize => Shape::NeswResize,
        CursorIcon::NwseResize => Shape::NwseResize,
        CursorIcon::ColResize => Shape::ColResize,
        CursorIcon::RowResize => Shape::RowResize,
        CursorIcon::AllScroll => Shape::AllScroll,
        CursorIcon::ZoomIn => Shape::ZoomIn,
        CursorIcon::ZoomOut => Shape::ZoomOut,
        CursorIcon::DndAsk if device_version >= 2 => Shape::DndAsk,
        CursorIcon::DndAsk => Shape::Alias,
        CursorIcon::AllResize if device_version >= 2 => Shape::AllResize,
        CursorIcon::AllResize => Shape::Move,
        // `Default`, plus whatever variants the non-exhaustive enum grows.
        _ => Shape::Default,
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for App {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let capabilities = state.seat_reducer.reduce_seat(&event);

        // Match the bound devices to the advertised capabilities: acquire a
        // device the seat now offers, and release one whose capability the seat
        // has withdrawn so any later re-advertisement starts from a fresh
        // object. The `release` destructor exists from version 3 on; below it,
        // dropping the proxy is all that can be done.
        if capabilities.pointer {
            if state.pointer.is_none() {
                let pointer = seat.get_pointer(qh, ());
                // The pointer's shape device lives exactly as long as the
                // pointer: created here, destroyed on release below.
                state.cursor_shape_device = state
                    .cursor_shape_manager
                    .as_ref()
                    .map(|manager| manager.get_pointer(&pointer, qh, ()));
                state.pointer = Some(pointer);
            }
        } else if let Some(pointer) = state.pointer.take() {
            if let Some(device) = state.cursor_shape_device.take() {
                device.destroy();
            }
            if pointer.version() >= 3 {
                pointer.release();
            }
        }

        if capabilities.keyboard {
            if state.keyboard.is_none() {
                state.keyboard = Some(seat.get_keyboard(qh, ()));
            }
        } else if let Some(keyboard) = state.keyboard.take()
            && keyboard.version() >= 3
        {
            keyboard.release();
        }

        if capabilities.touch {
            if state.touch.is_none() {
                state.touch = Some(seat.get_touch(qh, ()));
            }
        } else if let Some(touch) = state.touch.take()
            && touch.version() >= 3
        {
            touch.release();
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for App {
    fn event(
        state: &mut Self,
        _pointer: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let entered = matches!(event, wl_pointer::Event::Enter { .. });

        let time = state.now().ticks();
        let scale_factor = state.scale_factor();
        let translations = state
            .seat_reducer
            .reduce_pointer(scale_factor, &event, time);
        state.dispatch_seat_translations(translations, qh);

        // The compositor resets the cursor image on every entry, so each enter
        // re-applies the UI's current intent. This runs after the dispatch
        // above: the reducer then holds the enter serial the request must
        // present, and hit-testing the entry position has settled the intent.
        if entered {
            state.apply_cursor_shape();
        }
    }
}

impl Dispatch<WpCursorShapeManagerV1, ()> for App {
    fn event(
        _state: &mut Self,
        _manager: &WpCursorShapeManagerV1,
        _event: wp_cursor_shape_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // wp_cursor_shape_manager_v1 has no events.
    }
}

impl Dispatch<WpCursorShapeDeviceV1, ()> for App {
    fn event(
        _state: &mut Self,
        _device: &WpCursorShapeDeviceV1,
        _event: wp_cursor_shape_device_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // wp_cursor_shape_device_v1 has no events.
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for App {
    fn event(
        state: &mut Self,
        _keyboard: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        // The keyboard reducer resolves logical keys and the modifier set from
        // the compositor keymap itself (the `xkb` feature is on), so the raw
        // event is all it needs.
        let translation = state.seat_reducer.reduce_keyboard(&event);
        state.dispatch_seat_translations(translation, qh);
    }
}

impl Dispatch<wl_touch::WlTouch, ()> for App {
    fn event(
        state: &mut Self,
        _touch: &wl_touch::WlTouch,
        event: wl_touch::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let time = state.now().ticks();
        let scale_factor = state.scale_factor();
        let translations = state.seat_reducer.reduce_touch(scale_factor, &event, time);
        state.dispatch_seat_translations(translations, qh);
    }
}
