# Xcode Host App

`OverstoryDemoUIKitHost.xcodeproj` is the normal Xcode entry point for running
the Overstory demo on:

- an iPad device
- Mac Catalyst

The project keeps app lifecycle, code signing, install, launch, and debugger
integration on the Xcode side. The Rust crate keeps the actual Overstory demo
logic and exposes a single retained root view controller through a small C ABI.

## Open The Project

```sh
open overstory_demo_uikit/xcode/OverstoryDemoUIKitHost.xcodeproj
```

## First-Time Setup

1. Select the `OverstoryDemoUIKitHost` target.
2. In `Signing & Capabilities`, choose your development team.
3. Change the bundle identifier if your team or provisioning profile requires
   one.
4. Pick either:
   - `My Mac (Designed for iPad)` / Mac Catalyst
   - your connected iPad

The project intentionally supports only:

- `iphoneos`
- Mac Catalyst

It does not target the iOS simulator in this first slice. The checked-in Mac
Catalyst configuration assumes Apple Silicon and excludes the `x86_64` Catalyst
slice.

## How It Builds

The app target has a build phase that runs:

```sh
./overstory_demo_uikit/scripts/xcode-build-rust.sh
```

That script builds `overstory_demo_uikit` as a Rust `staticlib` for the active
Xcode destination and copies `liboverstory_demo_uikit.a` into
`$(BUILT_PRODUCTS_DIR)` before the app links.
