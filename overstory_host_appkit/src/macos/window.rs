// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! AppKit class wrappers and window geometry helpers.

#![expect(
    unsafe_code,
    reason = "AppKit class definitions require Objective-C FFI"
)]

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSApplication, NSApplicationDelegate, NSView, NSWindow, NSWindowDelegate};
use objc2_core_foundation::{CGPoint, CGRect};
use objc2_foundation::{NSNotification, NSObject, NSObjectProtocol};
use objc2_quartz_core::{CALayer, CAMetalLayer};

use super::state::{resize_window, shutdown_window_state, start_display_link};

define_class! {
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "OverstoryAppKitAppDelegate"]
    #[ivars = ()]
    pub(crate) struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationWillTerminate:))]
        fn application_will_terminate(&self, _notification: &NSNotification) {
            shutdown_window_state();
        }

        #[unsafe(method(applicationShouldTerminateAfterLastWindowClosed:))]
        fn should_terminate_after_last_window_closed(
            &self,
            _app: &NSApplication,
        ) -> bool {
            true
        }
    }
}

impl AppDelegate {
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

define_class! {
    /// Layer-backed window content view for the rendered Overstory surface.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "OverstoryAppKitContentView"]
    #[ivars = ()]
    pub(crate) struct ContentView;

    impl ContentView {
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            false
        }

        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }
}

impl ContentView {
    pub(crate) fn new(mtm: MainThreadMarker, frame: CGRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(());
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

define_class! {
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "OverstoryAppKitWindowDelegate"]
    #[ivars = ()]
    pub(crate) struct WindowDelegate;

    unsafe impl NSObjectProtocol for WindowDelegate {}

    unsafe impl NSWindowDelegate for WindowDelegate {
        #[unsafe(method(windowDidBecomeKey:))]
        fn window_did_become_key(&self, _notification: &NSNotification) {
            start_display_link();
        }

        #[unsafe(method(windowDidResize:))]
        fn window_did_resize(&self, _notification: &NSNotification) {
            resize_window();
        }

        #[unsafe(method(windowDidChangeBackingProperties:))]
        fn window_did_change_backing_properties(&self, _notification: &NSNotification) {
            resize_window();
        }
    }
}

impl WindowDelegate {
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

pub(crate) struct SurfaceSize {
    pub(crate) bounds: CGRect,
    pub(crate) scale_factor: f64,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

pub(crate) fn create_metal_sublayer(
    parent: &CALayer,
    size: &SurfaceSize,
) -> Retained<CAMetalLayer> {
    let layer = CAMetalLayer::new();
    layer.setBounds(CGRect::new(CGPoint::new(0.0, 0.0), size.bounds.size));
    layer.setContentsScale(size.scale_factor);
    layer.setDrawableSize(objc2_core_foundation::CGSize::new(
        f64::from(size.width),
        f64::from(size.height),
    ));
    layer.setFrame(size.bounds);
    parent.addSublayer(&layer);
    layer
}

pub(crate) fn backing_scale_factor(window: &NSWindow) -> f64 {
    let scale_factor = unsafe { msg_send![window, backingScaleFactor] };
    normalized_scale_factor(scale_factor)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "AppKit logical extents are clamped before converting to pixel dimensions"
)]
pub(crate) fn physical_extent(logical: f64, scale_factor: f64) -> u32 {
    let value = (logical * normalized_scale_factor(scale_factor)).round();
    if !value.is_finite() || value <= 1.0 {
        return 1;
    }
    if value >= f64::from(u32::MAX) {
        return u32::MAX;
    }
    value as u32
}

fn normalized_scale_factor(scale_factor: f64) -> f64 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}
