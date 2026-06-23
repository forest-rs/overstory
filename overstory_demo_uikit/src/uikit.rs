// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! UIKit application glue for the Overstory control gallery.

use core::cell::RefCell;
use core::ptr;
use std::path::Path;

use objc2::rc::{Allocated, Retained};
use objc2::runtime::AnyObject;
use objc2::{ClassType, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_foundation::{NSDictionary, NSObject, NSObjectProtocol, NSString};
use objc2_ui_kit::{
    UIApplication, UIApplicationDelegate, UIApplicationLaunchOptionsKey, UIScreen,
    UIViewController, UIWindow,
};
use overstory_host_uikit::ViewOptions;

#[derive(Debug, Default)]
struct AppDelegateIvars {
    window: RefCell<Option<Retained<UIWindow>>>,
}

define_class! {
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "OverstoryDemoUIKitAppDelegate"]
    #[ivars = AppDelegateIvars]
    struct AppDelegate;

    impl AppDelegate {
        #[unsafe(method_id(init))]
        fn init(this: Allocated<Self>) -> Retained<Self> {
            let this = this.set_ivars(AppDelegateIvars::default());
            unsafe { msg_send![super(this), init] }
        }
    }

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl UIApplicationDelegate for AppDelegate {
        #[unsafe(method(application:didFinishLaunchingWithOptions:))]
        #[expect(
            non_snake_case,
            reason = "objc2 protocol method names mirror Objective-C selector pieces"
        )]
        unsafe fn application_didFinishLaunchingWithOptions(
            &self,
            _application: &UIApplication,
            _launch_options: Option<&NSDictionary<UIApplicationLaunchOptionsKey, AnyObject>>,
        ) -> bool {
            let mtm = MainThreadMarker::from(self);
            let window = create_window(mtm);
            match create_demo_root_view_controller() {
                Ok(root_controller) => {
                    window.setRootViewController(Some(&root_controller));
                    window.makeKeyAndVisible();
                    self.ivars().window.replace(Some(window));
                    true
                }
                Err(error) => {
                    eprintln!("failed to create Overstory demo root view controller: {error}");
                    false
                }
            }
        }
    }
}

/// Create the Overstory demo root view controller.
pub fn create_demo_root_view_controller()
-> overstory_host_uikit::HostResult<Retained<UIViewController>> {
    let (width, height) = overstory_demo::initial_window_size();
    overstory_host_uikit::create_root_view_controller(
        ViewOptions::new().initial_size(width, height),
        overstory_demo::build_ui,
    )
}

/// Create the Overstory demo root view controller for Objective-C or Swift.
///
/// The returned pointer follows Core Foundation create-rule ownership: callers
/// own a +1 retained `UIViewController`.
#[unsafe(no_mangle)]
pub extern "C" fn overstory_demo_uikit_create_root_view_controller() -> *mut UIViewController {
    match create_demo_root_view_controller() {
        Ok(controller) => Retained::into_raw(controller),
        Err(error) => {
            eprintln!("failed to create Overstory demo root view controller: {error}");
            ptr::null_mut()
        }
    }
}

/// Run the Overstory demo as a standalone UIKit app bundle.
pub fn run_standalone() -> ! {
    exit_if_not_app_bundle();

    let mtm = MainThreadMarker::new().expect("UIKit apps must start on the main thread");
    let delegate_class = NSString::from_class(AppDelegate::class());
    UIApplication::main(None, Some(&delegate_class), mtm)
}

#[expect(
    deprecated,
    reason = "The demo wrapper deliberately uses the minimal app-delegate lifecycle before adding UIScene support"
)]
fn create_window(mtm: MainThreadMarker) -> Retained<UIWindow> {
    let frame = UIScreen::mainScreen(mtm).bounds();
    UIWindow::initWithFrame(mtm.alloc(), frame)
}

fn exit_if_not_app_bundle() {
    let Ok(current_exe) = std::env::current_exe() else {
        return;
    };
    if has_app_bundle_ancestor(&current_exe) {
        return;
    }

    eprintln!(
        "overstory_demo_uikit must be launched from a UIKit app bundle.\n\
         Use: ./overstory_demo_uikit/scripts/run-macabi.sh\n\
         Direct `cargo run -p overstory_demo_uikit --target aarch64-apple-ios-macabi` \
         launches a raw Mach-O and UIKit aborts before the app delegate starts."
    );
    std::process::exit(2);
}

fn has_app_bundle_ancestor(path: &Path) -> bool {
    path.ancestors().any(|ancestor| {
        ancestor
            .extension()
            .is_some_and(|extension| extension == "app")
    })
}
