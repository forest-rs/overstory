<div align="center">

# Overstory Host Winit

**Native `winit`, `wgpu`, and Vello host for Overstory applications.**

[![Latest published version.](https://img.shields.io/crates/v/overstory_host_winit.svg)](https://crates.io/crates/overstory_host_winit)
[![Documentation build status.](https://img.shields.io/docsrs/overstory_host_winit.svg)](https://docs.rs/overstory_host_winit)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=overstory_host_winit --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs may be evaluated here. -->

<!-- cargo-rdme start -->

Native Overstory host built on `winit`, `wgpu`, and `imaging_vello`.

This crate owns the platform event loop, window creation, normalized
input translation, GPU surface setup, and presentation path. Application
crates only need to provide a function that builds an [`overstory::Ui`].

<!-- cargo-rdme end -->

## Running An App

`overstory_host_winit` is the quickest way to run a native Overstory UI while
keeping windowing and GPU setup out of application code.

```rust
use overstory::Ui;
use overstory_host_winit::WindowOptions;

fn main() -> overstory_host_winit::HostResult<()> {
    overstory_host_winit::run(WindowOptions::new("Overstory"), Ui::new)
}
```

Use [`overstory_host`](../overstory_host/) directly when building another host
backend or when integrating Overstory into an existing application runtime.

## What It Owns

- `winit` event-loop and window creation;
- `ui-events` translation for pointer and keyboard input;
- `wgpu` surface setup and resize handling;
- Vello rendering into an offscreen texture;
- presentation of the rendered scene into the platform surface.

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
