# overstory_demo_uikit

`overstory_demo_uikit` is an app-shaped UIKit wrapper around the shared
`overstory_demo` control gallery. It owns UIKit app startup, Xcode integration,
bundling scripts, and the C ABI needed by Objective-C or Swift shells. The
reusable host behavior stays in `overstory_host_uikit`.

## Preferred: Xcode

The normal way to run this demo on an iPad or under Mac Catalyst is the
checked-in Xcode host app:

```sh
open overstory_demo_uikit/xcode/OverstoryDemoUIKitHost.xcodeproj
```

That project owns:

- signing
- install and launch from Xcode
- device selection
- debugger integration

The Rust crate owns the Overstory demo logic and is built as a `staticlib` by
the Xcode build phase.

## Mac Catalyst

Do not use `cargo run` for Mac Catalyst. Build, bundle, ad-hoc sign, and launch
the app with:

```sh
./overstory_demo_uikit/scripts/run-macabi.sh
```

For a release build:

```sh
./overstory_demo_uikit/scripts/run-macabi.sh --release
```

The script stages:

- `target/aarch64-apple-ios-macabi/<profile>/OverstoryDemoUIKit.app`
- `Contents/MacOS/overstory_demo_uikit`
- `Contents/Info.plist`

## iPad

For a physical iPad without Xcode's run button, build a signed iOS app bundle
first:

```sh
./overstory_demo_uikit/scripts/build-ios-app.sh \
  --codesign-identity "Apple Development: Your Name (TEAMID)" \
  --provisioning-profile /path/to/OverstoryDemoUIKit.mobileprovision
```

The default bundle identifier is:

```text
org.forest-rs.overstory.demo-uikit
```

If your provisioning profile expects a different identifier, pass
`--bundle-id <id>`.

Then install and launch it on the device:

```sh
xcrun devicectl list devices
./overstory_demo_uikit/scripts/install-ios-device.sh --device <device-udid>
```

## Xcode Integration

An Objective-C or Swift app can link this crate as a `staticlib` and call:

```c
UIViewController *_Nullable overstory_demo_uikit_create_root_view_controller(void);
```

Set the returned controller as the `UIWindow.rootViewController`. The returned
controller is retained for the caller, matching Core Foundation create-rule
ownership.

## Build-Only Checks

These are useful when you only want compile validation:

```sh
cargo check -p overstory_demo_uikit --target aarch64-apple-ios-macabi
cargo check -p overstory_demo_uikit --target aarch64-apple-ios
```

## License

Licensed under either Apache-2.0 or MIT, at your option. See the repository root
for license details.
