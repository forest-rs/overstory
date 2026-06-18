// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Native AppKit cursor mapping.

use objc2::rc::Retained;
use objc2_app_kit::{NSCursor, NSCursorFrameResizeDirections, NSCursorFrameResizePosition};
use overstory::CursorIcon;

pub(crate) fn set_cursor(cursor: CursorIcon) {
    cursor_for_icon(cursor).set();
}

fn cursor_for_icon(cursor: CursorIcon) -> Retained<NSCursor> {
    match cursor {
        CursorIcon::Default
        | CursorIcon::Help
        | CursorIcon::Progress
        | CursorIcon::Wait
        | CursorIcon::Cell
        | CursorIcon::Move
        | CursorIcon::AllScroll
        | CursorIcon::DndAsk => NSCursor::arrowCursor(),
        CursorIcon::ContextMenu => NSCursor::contextualMenuCursor(),
        CursorIcon::Pointer => NSCursor::pointingHandCursor(),
        CursorIcon::Crosshair => NSCursor::crosshairCursor(),
        CursorIcon::Text => NSCursor::IBeamCursor(),
        CursorIcon::VerticalText => NSCursor::IBeamCursorForVerticalLayout(),
        CursorIcon::Alias => NSCursor::dragLinkCursor(),
        CursorIcon::Copy => NSCursor::dragCopyCursor(),
        CursorIcon::NoDrop | CursorIcon::NotAllowed => NSCursor::operationNotAllowedCursor(),
        CursorIcon::Grab => NSCursor::openHandCursor(),
        CursorIcon::Grabbing => NSCursor::closedHandCursor(),
        CursorIcon::EResize
        | CursorIcon::WResize
        | CursorIcon::EwResize
        | CursorIcon::ColResize => NSCursor::columnResizeCursor(),
        CursorIcon::NResize
        | CursorIcon::SResize
        | CursorIcon::NsResize
        | CursorIcon::RowResize => NSCursor::rowResizeCursor(),
        CursorIcon::NeResize => frame_resize_cursor(NSCursorFrameResizePosition::TopRight),
        CursorIcon::NwResize => frame_resize_cursor(NSCursorFrameResizePosition::TopLeft),
        CursorIcon::SeResize => frame_resize_cursor(NSCursorFrameResizePosition::BottomRight),
        CursorIcon::SwResize => frame_resize_cursor(NSCursorFrameResizePosition::BottomLeft),
        CursorIcon::NeswResize => frame_resize_cursor(NSCursorFrameResizePosition::TopRight),
        CursorIcon::NwseResize => frame_resize_cursor(NSCursorFrameResizePosition::TopLeft),
        CursorIcon::ZoomIn => NSCursor::zoomInCursor(),
        CursorIcon::ZoomOut => NSCursor::zoomOutCursor(),
        _ => NSCursor::arrowCursor(),
    }
}

fn frame_resize_cursor(position: NSCursorFrameResizePosition) -> Retained<NSCursor> {
    NSCursor::frameResizeCursorFromPosition_inDirections(
        position,
        NSCursorFrameResizeDirections::All,
    )
}
