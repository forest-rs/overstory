// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! UIKit class wrappers and view geometry helpers.

#![expect(
    unsafe_code,
    reason = "UIKit class definitions require Objective-C FFI"
)]

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::NSSet;
use objc2_quartz_core::{CALayer, CAMetalLayer};
use objc2_ui_kit::{UIEvent, UIPress, UIPressesEvent, UITouch, UIView, UIWindow};
use ui_events_uikit::UIKitInputResponder;

use super::state::{resize_view, start_display_link};

pub(crate) struct ContentViewIvars {
    input_responder: Retained<UIKitInputResponder>,
}

define_class! {
    /// UIKit content view for the rendered Overstory surface.
    #[unsafe(super(UIView))]
    #[thread_kind = MainThreadOnly]
    #[name = "OverstoryUIKitContentView"]
    #[ivars = ContentViewIvars]
    pub(crate) struct ContentView;

    impl ContentView {
        #[unsafe(method(canBecomeFirstResponder))]
        fn can_become_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(didMoveToWindow))]
        fn did_move_to_window(&self) {
            unsafe {
                let _: () = msg_send![super(self), didMoveToWindow];
            }
            if self.window().is_some() {
                let _ = self.becomeFirstResponder();
                resize_view();
                start_display_link();
            }
        }

        #[unsafe(method(layoutSubviews))]
        fn layout_subviews(&self) {
            unsafe {
                let _: () = msg_send![super(self), layoutSubviews];
            }
            resize_view();
        }

        #[unsafe(method(touchesBegan:withEvent:))]
        fn touches_began(&self, touches: &NSSet<UITouch>, event: Option<&UIEvent>) {
            self.ivars().input_responder.handle_touch_set(touches, event);
        }

        #[unsafe(method(touchesMoved:withEvent:))]
        fn touches_moved(&self, touches: &NSSet<UITouch>, event: Option<&UIEvent>) {
            self.ivars().input_responder.handle_touch_set(touches, event);
        }

        #[unsafe(method(touchesEnded:withEvent:))]
        fn touches_ended(&self, touches: &NSSet<UITouch>, event: Option<&UIEvent>) {
            self.ivars().input_responder.handle_touch_set(touches, event);
        }

        #[unsafe(method(touchesCancelled:withEvent:))]
        fn touches_cancelled(&self, touches: &NSSet<UITouch>, event: Option<&UIEvent>) {
            self.ivars().input_responder.handle_touch_set(touches, event);
        }

        #[unsafe(method(pressesBegan:withEvent:))]
        fn presses_began(&self, presses: &NSSet<UIPress>, _event: Option<&UIPressesEvent>) {
            self.ivars().input_responder.handle_press_set(presses);
        }

        #[unsafe(method(pressesChanged:withEvent:))]
        fn presses_changed(&self, presses: &NSSet<UIPress>, _event: Option<&UIPressesEvent>) {
            self.ivars().input_responder.handle_press_set(presses);
        }

        #[unsafe(method(pressesEnded:withEvent:))]
        fn presses_ended(&self, presses: &NSSet<UIPress>, _event: Option<&UIPressesEvent>) {
            self.ivars().input_responder.handle_press_set(presses);
        }

        #[unsafe(method(pressesCancelled:withEvent:))]
        fn presses_cancelled(&self, presses: &NSSet<UIPress>, _event: Option<&UIPressesEvent>) {
            self.ivars().input_responder.handle_press_set(presses);
        }
    }
}

impl ContentView {
    pub(crate) fn new(
        mtm: MainThreadMarker,
        frame: CGRect,
        input_responder: Retained<UIKitInputResponder>,
    ) -> Retained<Self> {
        let this = mtm
            .alloc::<Self>()
            .set_ivars(ContentViewIvars { input_responder });
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        view.setUserInteractionEnabled(true);
        view.setMultipleTouchEnabled(true);
        view
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
    layer.setDrawableSize(CGSize::new(f64::from(size.width), f64::from(size.height)));
    layer.setFrame(size.bounds);
    parent.addSublayer(&layer);
    layer
}

pub(crate) fn scale_factor(view: &ContentView) -> f64 {
    let scale_factor = view
        .window()
        .as_deref()
        .map_or_else(|| fallback_screen_scale_factor(view), window_scale_factor);
    normalized_scale_factor(scale_factor)
}

fn window_scale_factor(window: &UIWindow) -> f64 {
    window.screen().scale()
}

fn fallback_screen_scale_factor(view: &ContentView) -> f64 {
    let scale_factor = view.contentScaleFactor();
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "UIKit logical extents are clamped before converting to pixel dimensions"
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
