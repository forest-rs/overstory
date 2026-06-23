<div align="center">

# Overstory Host UIKit

**Native UIKit host for Overstory applications.**

[![Latest published version.](https://img.shields.io/crates/v/overstory_host_uikit.svg)](https://crates.io/crates/overstory_host_uikit)
[![Documentation build status.](https://img.shields.io/docsrs/overstory_host_uikit.svg)](https://docs.rs/overstory_host_uikit)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=overstory_host_uikit --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs may be evaluated here. -->

<!-- cargo-rdme start -->

Native UIKit Overstory host built on `UIViewController`, `CAMetalLayer`, and
`overstory_presenter_wgpu`.

This crate owns the UIKit view/controller shell, UIKit input translation,
native display-link ticks, and native `wgpu` surface creation. Shared
`wgpu` / Vello presentation is delegated to `overstory_presenter_wgpu`.
It explicitly does not own Overstory UI semantics, app-bundle construction,
signing, or platform-independent frame scheduling policy.

<!-- cargo-rdme end -->

## Running An App

`overstory_host_uikit` creates a UIKit root view controller for app bundles that
own their `UIApplication`, `UIWindow`, signing, and installation path.

```rust
use objc2::rc::Retained;
use objc2_ui_kit::UIViewController;
use overstory::Ui;
use overstory_host_uikit::ViewOptions;

pub fn create_root_view_controller() -> overstory_host_uikit::HostResult<Retained<UIViewController>> {
    overstory_host_uikit::create_root_view_controller(ViewOptions::new(), Ui::new)
}
```

An Objective-C or Swift app shell can expose that function through a small C ABI,
then set the returned controller as the window's root view controller. The
`ui-events` `simple_uikit` example shows that app-bundle pattern.

## What It Owns

- UIKit `UIViewController` and content `UIView` creation;
- touch, stylus, remote, and hardware-keyboard input via `ui-events-uikit`;
- `CADisplayLink` frame ticks through `frameclock_apple`;
- native `CAMetalLayer` / `wgpu::Surface` creation and resize handling.

## Current Gaps

- UIKit accessibility is not wired yet because there is no `accesskit_uikit`
  adapter in this workspace.
- `ui-events-uikit` provides a reusable `UIResponder`, but UIKit does not expose
  an AppKit-style public `setNextResponder` insertion point. This host currently
  forwards touch and press callbacks from its content `UIView`; a reusable
  `ui-events-uikit` view or forwarding helper would be the cleaner shared API.

## Minimum Supported Rust Version (MSRV)

This crate has been verified to compile with **Rust 1.92** and later.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE] or <http://www.apache.org/licenses/LICENSE-2.0>), or
- MIT license ([LICENSE-MIT] or <http://opensource.org/licenses/MIT>),

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you,
as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## Contribution

Contributions are welcome by pull request. The [Rust code of conduct] applies.
Please feel free to add your name to the [AUTHORS] file in any substantive pull request.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you,
as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

[LICENSE-APACHE]: https://github.com/forest-rs/overstory/blob/main/LICENSE-APACHE
[LICENSE-MIT]: https://github.com/forest-rs/overstory/blob/main/LICENSE-MIT
[Rust code of conduct]: https://www.rust-lang.org/policies/code-of-conduct
[AUTHORS]: https://github.com/forest-rs/overstory/blob/main/AUTHORS
