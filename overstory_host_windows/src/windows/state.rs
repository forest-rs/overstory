// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained Win32 window, renderer, and UI host state.
//!
//! Per-window state lives behind an `Box<WindowState>` whose raw pointer is stored in the window's
//! `GWLP_USERDATA` slot.

#![expect(unsafe_code, reason = "wgpu surface creation and Win32 FFI")]

use alloc::boxed::Box;
use core::ptr;

use accesskit::{ActionRequest, TreeUpdate};
use accesskit_windows::Adapter;
use frameclock::{
    Duration as FrameDuration, FrameDemand, FrameOpportunity, FrameSubmission, OutputId,
};
use overstory::{CursorIcon, Ui};
use overstory_host::{FrameViewport, HostDispatchResult, UiHost};
use overstory_presenter_wgpu::{WgpuPresentResult, WgpuPresenter, WgpuPresenterDescriptor};
use ui_events_windows::{InputEvent, MessageResponse, WindowMessageReducer};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, GWLP_USERDATA, HTCLIENT, SetWindowLongPtrW, WM_ACTIVATE, WM_CLOSE,
    WM_DPICHANGED, WM_GETOBJECT, WM_IME_COMPOSITION, WM_IME_ENDCOMPOSITION,
    WM_IME_STARTCOMPOSITION, WM_KEYDOWN, WM_KEYUP, WM_KILLFOCUS, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN,
    WM_RBUTTONUP, WM_SETCURSOR, WM_SETFOCUS, WM_SIZE, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER,
    WM_TOUCH, WM_XBUTTONDOWN, WM_XBUTTONUP,
};

use super::{HostResult, time::TimeMap, window::client_size};

const FALLBACK_REFRESH_INTERVAL: FrameDuration = FrameDuration(16_666_667);
const DEFAULT_OUTPUT: OutputId = OutputId(0);

pub(crate) struct WindowState {
    pub(crate) hwnd: HWND,
    accesskit: Adapter,
    presenter: WgpuPresenter,
    host: UiHost,
    message_reducer: WindowMessageReducer,
    scale_factor: f64,
    cursor: CursorIcon,
    time_map: TimeMap,
}

impl WindowState {
    pub(crate) fn new<F>(hwnd: HWND, build_ui: F) -> HostResult<Self>
    where
        F: FnOnce() -> Ui,
    {
        // SAFETY: `hwnd` was just created by `create_window`.
        let scale_factor = unsafe { super::window::dpi_scale_factor(hwnd) };
        let event_reducer = unsafe { WindowMessageReducer::new(hwnd.0, scale_factor) };

        let instance = wgpu::Instance::default();
        let target = wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(wgpu::rwh::RawDisplayHandle::Windows(
                wgpu::rwh::WindowsDisplayHandle::new(),
            )),
            raw_window_handle: wgpu::rwh::RawWindowHandle::Win32(
                wgpu::rwh::Win32WindowHandle::new(
                    hwnd.0
                        .addr()
                        .cast_signed()
                        .try_into()
                        .expect("window handle to be non-null"),
                ),
            ),
        };
        // SAFETY: `hwnd` is a valid, live top-level window for at least as long as the resulting surface.
        //          The surface is destroyed via dropping `WindowState` before destroying the window.
        let surface = unsafe { instance.create_surface_unsafe(target) }?;

        // SAFETY: `hwnd` is valid.
        let (width, height) = unsafe { client_size(hwnd) };
        let presenter = WgpuPresenter::new(
            &instance,
            surface,
            width,
            height,
            WgpuPresenterDescriptor::new("overstory_host_windows"),
        )?;

        let accesskit = Adapter::new(hwnd, false, super::accessibility::AccessKitActions(hwnd));
        let host = UiHost::new(build_ui());

        Ok(Self {
            hwnd,
            accesskit,
            presenter,
            host,
            message_reducer: event_reducer,
            scale_factor,
            cursor: CursorIcon::Default,
            time_map: TimeMap::new(),
        })
    }

    /// Handles one raw Win32 message for this window.
    ///
    /// Returns `Some(lresult)` when overstory has consumed the message, which should be returned
    /// from the `WNDPROC` and the message should not be forwarded to `DefWindowProcW`.
    pub(crate) fn handle_message(
        &mut self,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> MessageResponse {
        if msg == WM_GETOBJECT {
            return self
                .accesskit
                .handle_wm_getobject(
                    wparam,
                    lparam,
                    &mut super::accessibility::AccessKitActivation(self.accesskit_tree_update()),
                )
                .map(|r| r.into().0)
                .map_or(MessageResponse::Forward, MessageResponse::Consume);
        }

        let result = self.handle_overstory_message(msg, wparam, lparam);
        self.pump_frame();

        // Re-arm this crate's `frameclock`-driven wakeup timer after every message.
        super::window::arm_next_wakeup(self.hwnd, self.duration_until_next_wakeup());

        match msg {
            WM_CLOSE => {
                // TODO: handle close button/Alt+F4 behavior here, in the future.
                // SAFETY: `hwnd` is valid.
                let _ = unsafe { DestroyWindow(self.hwnd) };
                MessageResponse::Consume(0)
            }
            WM_TIMER => MessageResponse::Consume(0),
            _ => result,
        }
    }

    pub(crate) fn handle_overstory_message(
        &mut self,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> MessageResponse {
        let event_time = self.host.frame_now().ticks();
        let reduction = unsafe {
            self.message_reducer
                .reduce(msg, wparam.0, lparam.0, event_time)
        };
        for translation in reduction.events {
            self.dispatch_translated_event(translation);
        }

        match msg {
            WM_SIZE => {
                self.resize();
                self.request_frame(FrameDemand::CONTINUOUS_INPUT);
                MessageResponse::Consume(0)
            }
            WM_DPICHANGED => {
                self.update_scale_factor();
                self.resize();
                self.request_frame(FrameDemand::CONTINUOUS_INPUT);
                MessageResponse::Consume(0)
            }
            WM_SETFOCUS => {
                self.accesskit.update_window_focus_state(true);
                MessageResponse::Consume(0)
            }
            WM_KILLFOCUS => {
                self.accesskit.update_window_focus_state(false);
                MessageResponse::Consume(0)
            }
            WM_ACTIVATE => MessageResponse::Consume(0),
            WM_SETCURSOR => {
                if (lparam.0.cast_unsigned() & 0xFFFF) as u32 == HTCLIENT {
                    self.sync_cursor();
                    // SAFETY: called on this window's owning thread,
                    //          in response to a message this window received.
                    unsafe { super::cursor::set_cursor(self.cursor) };
                    MessageResponse::Consume(0)
                } else {
                    MessageResponse::Forward
                }
            }
            _ => {
                if let MessageResponse::ConsumeIfHandled(lresult) = reduction.response
                    && is_input_message(msg)
                {
                    MessageResponse::Consume(lresult)
                } else {
                    MessageResponse::Forward
                }
            }
        }
    }

    /// Requests the first frame after window/renderer initialization.
    pub(crate) fn request_initial_frame(&mut self) {
        self.request_frame(FrameDemand::ANIMATION);
    }

    pub(crate) fn accesskit_tree_update(&self) -> TreeUpdate {
        self.host.accesskit_tree_update(self.scale_factor)
    }

    pub(crate) fn dispatch_accesskit_action(&mut self, request: &ActionRequest) {
        let result = self.host.handle_accesskit_action(request);
        self.finish_dispatch(result);
    }

    /// Renders one frame if `frameclock` says a planned frame is due.
    ///
    /// Called after every message that might have produced frame demand.
    pub(crate) fn pump_frame(&mut self) {
        if self.host.pump_timers_now() {
            // Timers becoming due may have queued a frame.
            // Fall through to the render attempt below on the same pump.
        }
        if self.host.pending_frame_demand().is_empty() && self.host.pending_frame_start().is_none()
        {
            return;
        }
        if let Err(err) = self.render() {
            eprintln!("overstory_host_windows: render error: {err}");
        }
    }

    /// Returns the duration until the next pending wakeup, if any.
    ///
    /// The caller uses this to size a `SetTimer`-driven wait.
    pub(crate) fn duration_until_next_wakeup(&self) -> Option<core::time::Duration> {
        self.host.duration_until_next_wakeup()
    }

    fn dispatch_translated_event(&mut self, translation: InputEvent) {
        match translation {
            InputEvent::Pointer(pointer) => {
                let result = self.host.dispatch_pointer_event(&pointer);
                self.finish_dispatch(result);
            }
            InputEvent::Keyboard(key) => {
                let result = self.host.dispatch_key(&key);
                self.finish_dispatch(result);
            }
            InputEvent::Text(_) => {
                // Overstory does not yet expose a host-level text input dispatch path.
            }
        }
    }

    fn finish_dispatch(&mut self, result: HostDispatchResult) {
        if result.needs_redraw {
            self.request_frame(result.frame_demand);
        }
        self.sync_cursor();
    }

    fn request_frame(&mut self, demand: FrameDemand) {
        self.host.request_frame(demand);
    }

    fn sync_cursor(&mut self) {
        if let Some(cursor) = self.host.cursor_update() {
            self.cursor = cursor;
        }
    }

    fn sync_accessibility(&mut self) {
        let scale_factor = self.scale_factor;
        let host = &self.host;
        self.accesskit
            .update_if_active(|| host.accesskit_tree_update(scale_factor));
    }

    fn update_scale_factor(&mut self) {
        // SAFETY: `self.hwnd` is this window's own valid handle.
        self.scale_factor = unsafe { super::window::dpi_scale_factor(self.hwnd) };
        self.message_reducer.set_scale_factor(self.scale_factor);
    }

    fn resize(&mut self) {
        // SAFETY: `self.hwnd` is this window's own valid handle.
        let (width, height) = unsafe { client_size(self.hwnd) };
        self.presenter.resize(width, height);
    }

    fn size(&self) -> (u32, u32) {
        self.presenter.size()
    }

    fn frame_opportunity(&mut self) -> FrameOpportunity {
        // Use `subduction_backend_windows`.
        let now = self.time_map.now();
        self.refresh_rate_millihertz()
            .and_then(super::time::refresh_interval_from_millihertz)
            .map_or_else(
                || FrameOpportunity::pacing_only(now, FALLBACK_REFRESH_INTERVAL, DEFAULT_OUTPUT),
                |interval| FrameOpportunity::pacing_only(now, interval, DEFAULT_OUTPUT),
            )
    }

    #[expect(clippy::cast_possible_truncation, reason = "impossible")]
    fn refresh_rate_millihertz(&self) -> Option<u32> {
        use windows::Win32::Graphics::Gdi;

        let hmonitor = unsafe { Gdi::MonitorFromWindow(self.hwnd, Gdi::MONITOR_DEFAULTTONEAREST) };
        let mut monitor_info = Gdi::MONITORINFOEXW::default();
        monitor_info.monitorInfo.cbSize = size_of_val(&monitor_info) as _;
        if !unsafe { Gdi::GetMonitorInfoW(hmonitor, (&raw mut monitor_info).cast()) }.as_bool() {
            return None;
        };

        let device_name = monitor_info.szDevice.as_ptr();
        let mut mode = Gdi::DEVMODEW::default();
        mode.dmSize = size_of_val(&mode) as _;
        unsafe {
            Gdi::EnumDisplaySettingsExW(
                windows::core::PCWSTR(device_name),
                Gdi::ENUM_CURRENT_SETTINGS,
                &mut mode,
                Gdi::ENUM_DISPLAY_SETTINGS_FLAGS::default(),
            )
        }
        .as_bool()
        .then_some(mode.dmDisplayFrequency * 1000)
    }

    fn submitted_at(&self) -> frameclock::HostTime {
        self.time_map.now()
    }

    fn render(&mut self) -> HostResult<()> {
        let (width, height) = self.size();
        if width == 0 || height == 0 {
            return Ok(());
        }

        let viewport = FrameViewport::from_physical_size(width, height, self.scale_factor);
        let opportunity = self.frame_opportunity();
        let present = {
            let Some(prepared) = self.host.prepare_requested_frame(viewport, opportunity) else {
                return Ok(());
            };
            debug_assert!(
                !prepared.demand().is_empty(),
                "requested frames should carry non-empty planned demand"
            );
            self.presenter.present_scene(prepared.scene())
        };

        let present = match present {
            Ok(present) => present,
            Err(err) => {
                self.host.discard_prepared_frame();
                return Err(Box::new(err));
            }
        };
        self.sync_accessibility();
        let WgpuPresentResult::Presented(timings) = present else {
            self.host.discard_prepared_frame();
            return Ok(());
        };
        self.host
            .record_frame_submission(timings, FrameSubmission::new(self.submitted_at(), None));
        Ok(())
    }

    /// Installs `self` as `self.hwnd`'s window state, by leaking a `Box` reference
    /// into a [`WindowStateMut`].
    ///
    /// The leaked reference is then be placed in the `GWLP_USERDATA` slot of `self.hwnd` once
    /// the [`WindowStateMut`] is dropped or reclaimed and dropped by [`WindowStateMut::destroy`].
    pub(crate) fn attach<'w>(self) -> WindowStateMut<'w> {
        let ptr = Box::leak(Box::new(self));
        WindowStateMut { ptr }
    }

    /// Returns a mutable borrow to `hwnd`'s window state, if any is currently attached.
    ///
    /// This prevents re-entrance. For more information, see [`Self::attach`].
    ///
    /// # Safety
    ///
    /// `hwnd` must be a valid window handle. If `GWLP_USERDATA` is non-zero, it
    /// must be a pointer previously installed by [`Self::attach`].
    pub(crate) unsafe fn fetch<'w>(hwnd: HWND) -> Option<WindowStateMut<'w>> {
        // SAFETY: `hwnd` is valid.
        let ptr = unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
        let ptr = ptr::without_provenance_mut::<Self>(ptr.cast_unsigned());
        // SAFETY: `ptr` was obtained via `Box::into_raw` or is null.
        unsafe { ptr.as_mut() }
            .map(|ptr| {
                // Already borrowed further up the call stack: this is a
                // re-entrant call, so report it as unavailable rather than
                // flipping the flag back (which would let a later `Drop` clear
                // a flag it doesn't own).
                WindowStateMut { ptr }
            })
            .or_else(|| {
                eprintln!("state is unavailable");
                None
            })
    }
}

/// A mutable, re-entrance-checked borrow of a [`WindowState`]'.
pub(crate) struct WindowStateMut<'a> {
    ptr: &'a mut WindowState,
}

impl core::ops::Deref for WindowStateMut<'_> {
    type Target = WindowState;

    fn deref(&self) -> &WindowState {
        self.ptr
    }
}

impl core::ops::DerefMut for WindowStateMut<'_> {
    fn deref_mut(&mut self) -> &mut WindowState {
        self.ptr
    }
}

impl Drop for WindowStateMut<'_> {
    fn drop(&mut self) {
        let leaked_box = ptr::from_mut(self.ptr);
        // SAFETY: `hwnd` and `leaked_box` are valid.
        unsafe { SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, leaked_box.addr().cast_signed()) };
    }
}

impl<'a> WindowStateMut<'a> {
    pub(crate) fn destroy(self) {
        // SAFETY: `self.ptr` was obtained via `Box::leak`.
        drop(unsafe { Box::from_raw(self.ptr) });
        core::mem::forget(self);
    }
}

/// Returns whether `msg` is one this host feeds through [`EventReducer::reduce`].
const fn is_input_message(msg: u32) -> bool {
    // monitor `EventReducer::reduce` for change in this list
    matches!(
        msg,
        WM_KEYDOWN
            | WM_KEYUP
            | WM_SYSKEYDOWN
            | WM_SYSKEYUP
            | WM_MOUSEMOVE
            | WM_MOUSELEAVE
            | WM_LBUTTONDOWN
            | WM_LBUTTONUP
            | WM_RBUTTONDOWN
            | WM_RBUTTONUP
            | WM_MBUTTONDOWN
            | WM_MBUTTONUP
            | WM_XBUTTONDOWN
            | WM_XBUTTONUP
            | WM_MOUSEWHEEL
            | WM_MOUSEHWHEEL
            | WM_TOUCH
            | WM_IME_STARTCOMPOSITION
            | WM_IME_COMPOSITION
            | WM_IME_ENDCOMPOSITION
    )
}

/// Returns whether `msg` is one this window state handles.
pub(crate) const fn would_handle_message(msg: u32) -> bool {
    is_input_message(msg)
        // monitor `WindowState::handle_message` for change in this list
        || matches!(msg, WM_CLOSE | WM_TIMER)
        // monitor `WindowState::handle_overstory_message` for change in this list
        || matches!(
            msg,
            WM_SIZE | WM_DPICHANGED | WM_SETFOCUS | WM_KILLFOCUS | WM_SETCURSOR
        )
}
