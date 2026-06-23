<div align="center">

# Overstory Presenter WGPU

**Shared `wgpu` and Vello presentation helpers for Overstory host crates.**

[![Latest published version.](https://img.shields.io/crates/v/overstory_presenter_wgpu.svg)](https://crates.io/crates/overstory_presenter_wgpu)
[![Documentation build status.](https://img.shields.io/docsrs/overstory_presenter_wgpu.svg)](https://docs.rs/overstory_presenter_wgpu)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=overstory_presenter_wgpu --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs may be evaluated here. -->

<!-- cargo-rdme start -->

`wgpu` and Vello presentation helpers for Overstory host crates.

This crate owns the backend work that starts after a native host has created
a [`wgpu::Surface`]: adapter/device selection, surface configuration,
rendering a prepared Overstory scene through `imaging_vello`, blitting that
offscreen texture into the presentation surface, and reporting renderer
timings.

It explicitly does not own native windows, event loops, input translation,
accessibility, timers, frame scheduling, or platform-specific surface
creation.

<!-- cargo-rdme end -->

## What It Owns

- adapter and device selection for a host-created `wgpu::Surface`;
- surface configuration and resize handling;
- rendering prepared Overstory scenes through `imaging_vello`;
- blitting the offscreen render target into the presentation surface;
- renderer timing collection for host diagnostics.

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
