<div align="center">

# Overstory Host

**Platform-neutral runtime helpers for driving an Overstory `Ui`.**

[![Latest published version.](https://img.shields.io/crates/v/overstory_host.svg)](https://crates.io/crates/overstory_host)
[![Documentation build status.](https://img.shields.io/docsrs/overstory_host.svg)](https://docs.rs/overstory_host)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=overstory_host --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs may be evaluated here. -->

<!-- cargo-rdme start -->

Runtime helpers for driving an [`overstory::Ui`] outside of a
particular platform windowing backend.

This crate owns the reusable mechanics that every native Overstory
host needs: monotonic timer time, timer wakeup calculation, normalized
pointer/key dispatch, cursor intent tracking, layout, and preparation of
an `imaging` scene for a physical render target. Window creation, event
loop ownership, and GPU presentation are intentionally left to backend
crates such as `overstory_host_winit`.

<!-- cargo-rdme end -->

## When To Use It

Use `overstory_host` when you are building a backend host and want one place for
the shared mechanics every host needs:

- pump retained UI timers from a monotonic host clock;
- dispatch normalized `ui-events` pointer and keyboard input;
- track cursor intent reported by the UI tree;
- compute layout and produce an `imaging` scene for a physical target;
- cache per-surface scenes for compositor-friendly composition surfaces.

If you want to run a native desktop app directly, use
[`overstory_host_winit`](../overstory_host_winit/) instead.

## Example

```rust
use overstory::Ui;
use overstory_host::{logical_viewport, UiHost};

let mut host = UiHost::new(Ui::new());
let viewport = logical_viewport(1280, 720, 2.0);
let _scene = host.prepare_frame(viewport, 2.0);
```

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
