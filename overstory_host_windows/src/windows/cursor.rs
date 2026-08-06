// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Native Win32 cursor mapping.

use overstory::CursorIcon;
use windows::Win32::UI::WindowsAndMessaging::{
    IDC_APPSTARTING, IDC_ARROW, IDC_CROSS, IDC_HAND, IDC_HELP, IDC_IBEAM, IDC_NO, IDC_SIZEALL,
    IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE, IDC_WAIT, LoadCursorW, SetCursor,
};

/// Sets the current Win32 cursor for `cursor`.
///
/// # Safety
///
/// Must be called on the thread that owns the window's message queue.
pub(crate) unsafe fn set_cursor(cursor: CursorIcon) {
    let resource = system_cursor_resource(cursor);
    // SAFETY: `resource` is one of the built-in `IDC_*` system cursor identifiers,
    //          which are always valid to pass to `LoadCursorW` with a `None` module handle.
    let handle = unsafe { LoadCursorW(None, resource) };
    if let Ok(handle) = handle {
        // SAFETY: `handle` was just loaded successfully above.
        let _ = unsafe { SetCursor(Some(handle)) };
    }
}

/// Maps a [`CursorIcon`] to the closest built-in Win32 `IDC_*` cursor resource.
fn system_cursor_resource(cursor: CursorIcon) -> windows::core::PCWSTR {
    match cursor {
        CursorIcon::Default
        | CursorIcon::Move
        | CursorIcon::Cell
        | CursorIcon::AllScroll
        | CursorIcon::DndAsk
        | CursorIcon::ContextMenu => IDC_ARROW,
        CursorIcon::Help => IDC_HELP,
        CursorIcon::Progress => IDC_APPSTARTING,
        CursorIcon::Wait => IDC_WAIT,
        CursorIcon::Pointer | CursorIcon::Alias | CursorIcon::Copy | CursorIcon::Grab => IDC_HAND,
        CursorIcon::Grabbing => IDC_SIZEALL,
        CursorIcon::Crosshair => IDC_CROSS,
        CursorIcon::Text | CursorIcon::VerticalText => IDC_IBEAM,
        CursorIcon::NoDrop | CursorIcon::NotAllowed => IDC_NO,
        CursorIcon::EResize
        | CursorIcon::WResize
        | CursorIcon::EwResize
        | CursorIcon::ColResize => IDC_SIZEWE,
        CursorIcon::NResize
        | CursorIcon::SResize
        | CursorIcon::NsResize
        | CursorIcon::RowResize => IDC_SIZENS,
        CursorIcon::NeResize | CursorIcon::SwResize | CursorIcon::NeswResize => IDC_SIZENESW,
        CursorIcon::NwResize | CursorIcon::SeResize | CursorIcon::NwseResize => IDC_SIZENWSE,
        CursorIcon::ZoomIn | CursorIcon::ZoomOut => IDC_ARROW,
        _ => IDC_ARROW,
    }
}
