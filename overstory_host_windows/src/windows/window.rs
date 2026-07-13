// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Win32 window class registration, `WNDPROC`, and message loop.
//!
//! Per-window state is a [`WindowState`], stored in a `Box`.
//! Since a `WNDPROC` is a plain `extern "system" fn`, that `Box`'s raw pointer is stashed
//! in the window's `GWLP_USERDATA` slot (via [`SetWindowLongPtrW`]) right after creation.
//!
//! Accessing this state object involves a PPYP (pre-poop-your-pants) technique that temporarily
//! replaces the `GWLP_USERDATA` with `null` (0) and puts it back in the drop handler of the accessor
//! ([`WindowStateMut`]).
//! //! This eliminates manual re-entrance checking and keeps the data structures simple.

#![expect(unsafe_code, reason = "Win32 FFI")]

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::UI::HiDpi;
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW,
    DispatchMessageW, GetClientRect, GetMessageW, KillTimer, MSG, PostQuitMessage,
    RegisterClassExW, SW_SHOW, SetTimer, ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WM_DESTROY,
    WNDCLASSEXW, WS_EX_OVERLAPPEDWINDOW, WS_OVERLAPPEDWINDOW,
};

const WINDOW_CLASS_NAME: windows::core::PCWSTR = windows::core::w!("OverstoryWin32Window");

/// Enables per-monitor-v2 DPI awareness for this process.
///
/// Must run before any window is created.
pub(crate) fn enable_per_monitor_dpi_awareness() {
    // SAFETY: `SetProcessDpiAwarenessContext` has no preconditions beyond being called early
    //          in process startup, before any window is created.
    let _ = unsafe {
        HiDpi::SetProcessDpiAwarenessContext(HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
    };
}

/// Registers this crate's window class, idempotently.
#[expect(clippy::cast_possible_truncation, reason = "impossible")]
fn register_window_class() {
    let class = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as _,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wndproc),
        lpszClassName: WINDOW_CLASS_NAME,
        ..Default::default()
    };

    // SAFETY: `class` is fully initialized, and registering the same class name twice
    //          (e.g. across repeated `run` calls in tests) is harmless.
    //          `RegisterClassExW` simply fails and this crate does not rely on the returned atom.
    let _ = unsafe { RegisterClassExW(&class) };
}

/// Creates this crate's top-level window.
///
/// # Safety
///
/// The calling thread must be the thread that will run the message loop ([`run_message_loop`]).
pub(crate) unsafe fn create_window(options: super::WindowOptions) -> windows::core::Result<HWND> {
    register_window_class();

    let (width, height) = options.size();
    let title = windows::core::HSTRING::from(options.title());

    // The requested size is a client-area (logical) size.
    // grow the window rect so the client area matches after the non-client frame is added.
    //
    // Uses the default (96 DPI) frame metrics since the window does not exist yet to query its own DPI.
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: width.cast_signed(),
        bottom: height.cast_signed(),
    };
    // SAFETY: `rect` is a valid, fully initialized `RECT`
    let _ = unsafe {
        AdjustWindowRectEx(
            &mut rect,
            WS_OVERLAPPEDWINDOW,
            false,
            WINDOW_EX_STYLE::default(),
        )
    };
    let adjusted_width = rect.right - rect.left;
    let adjusted_height = rect.bottom - rect.top;

    // SAFETY: `title` is valid null-terminated wide strings for the duration of this call.
    unsafe {
        CreateWindowExW(
            WS_EX_OVERLAPPEDWINDOW,
            WINDOW_CLASS_NAME,
            &title,
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            adjusted_width,
            adjusted_height,
            None,
            None,
            None,
            None,
        )
    }
}

/// Shows a created window.
///
/// # Safety
///
/// `hwnd` must be a valid window handle.
pub(crate) unsafe fn show_window(hwnd: HWND) {
    // SAFETY: `hwnd` is valid.
    let _ = unsafe { ShowWindow(hwnd, SW_SHOW) };
}

/// Returns the client-area size of `hwnd` in physical pixels.
///
/// # Safety
///
/// `hwnd` must be a valid window handle.
pub(crate) unsafe fn client_size(hwnd: HWND) -> (u32, u32) {
    let mut rect = RECT::default();
    // SAFETY: `hwnd` is valid, `rect` is zero initialized.
    let _ = unsafe { GetClientRect(hwnd, &mut rect) };
    (
        (rect.right - rect.left).cast_unsigned().max(1),
        (rect.bottom - rect.top).cast_unsigned().max(1),
    )
}

/// Runs the Win32 message loop until `WM_QUIT`.
///
/// # Safety
///
/// Must be called on the thread that created the window this loop is pumping messages for.
pub(crate) unsafe fn run_message_loop() {
    let mut msg = MSG::default();

    // SAFETY: `msg` is zero initialized and this is called on the window's owning thread.
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Returns the DPI scale factor for `hwnd` (1.0 at 96 DPI).
///
/// # Safety
///
/// `hwnd` must be a valid window handle.
pub(crate) unsafe fn dpi_scale_factor(hwnd: HWND) -> f64 {
    const BASE_DPI: f64 = 96.;
    // SAFETY: `hwnd` is valid.
    let dpi = unsafe { HiDpi::GetDpiForWindow(hwnd) };
    if dpi == 0 { 1. } else { dpi as f64 / BASE_DPI }
}

/// Arms (or re-arms) the Win32 timer used to wake the message loop
/// for the next `frameclock`-scheduled frame, or clears it if nothing is pending.
///
/// Called by [`run`] once at startup and by the window procedure after every message it dispatches.
pub(crate) fn arm_next_wakeup(hwnd: HWND, duration: Option<std::time::Duration>) {
    /// The Win32 timer ID this crate uses to wake the message loop for `frameclock`-driven pacing,
    /// when no other message has arrived.
    const WAKEUP_TIMER_ID: usize = 1;

    match duration {
        Some(duration) => {
            let millis = u32::try_from(duration.as_millis())
                .unwrap_or(u32::MAX)
                .max(1);
            // SAFETY: `hwnd` is a window owned by this crate, on this thread.
            unsafe { SetTimer(Some(hwnd), WAKEUP_TIMER_ID, millis, None) };
        }
        None => {
            // SAFETY: `hwnd` is a window owned by this crate, on this thread.
            //          Killing a timer that isn't currently set is a no-op.
            let _ = unsafe { KillTimer(Some(hwnd), WAKEUP_TIMER_ID) };
        }
    }
}

/// The `WNDPROC` for this crate's window class.
///
/// # Safety
///
/// This function is only ever called by the Win32 message dispatcher via the function pointer
/// installed in [`register_window_class`], which guarantees the standard `WNDPROC` contract
/// (valid `hwnd`, message dispatched on the window's owning thread).
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let result = if super::state::would_handle_message(msg)
        // SAFETY: `hwnd` is valid.
        && let Some(mut state) = unsafe { super::WindowState::fetch(hwnd) }
    {
        state.handle_message(msg, wparam, lparam)
    } else if msg == WM_DESTROY {
        // SAFETY: `hwnd` is valid.
        if let Some(state) = unsafe { super::WindowState::fetch(hwnd) } {
            state.destroy();
        }
        // SAFETY: always valid to call from a window procedure.
        unsafe { PostQuitMessage(0) };
        Some(0)
    } else {
        None
    };

    result.map_or_else(
        // SAFETY: always valid to call from a window procedure.
        || unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        LRESULT,
    )
}
