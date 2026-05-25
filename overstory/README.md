<div align="center">

# Overstory

**Retained UI toolkit core for forest-rs.**

[![Latest published version.](https://img.shields.io/crates/v/overstory.svg)](https://crates.io/crates/overstory)
[![Documentation build status.](https://img.shields.io/docsrs/overstory.svg)](https://docs.rs/overstory)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=overstory --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs may be evaluated here. -->

<!-- cargo-rdme start -->

Overstory: a WPF / WinUI 3-inspired UI toolkit built on
`understory` and `imaging`.

Each control is a struct that implements [`AppendSpec`] (the
builder + entry point) and produces a [`Widget`] instance that
lives on the element. Application code uses
`Ui::append` to insert controls into the
tree:

```rust
ui.append(parent, Button::new("Click me").on_click(|_, _| {}));
ui.append(parent, ToggleSwitch::new().checked(true));
ui.append(parent, Slider::new().value(0.4));
```

`Ui::append` is the only public path for adding controls;
`insert_child` lives on the toolkit-internal side for templates
building their own parts. `set_local` remains public for power
users (bindings, dynamic theming, custom controls), but isn't
the typical app-level entry point.

Roadmap:
- Keep `overstory` itself as the no-std core toolkit: retained UI tree,
  controls, styling, layout intent, routed events, presentation lowering,
  and compositor-neutral composition data.
- Keep platform integration in host crates. Windowing, GPU setup, clipboard,
  IME, accessibility bridges, frame pacing, and native compositor surfaces
  should not leak into core controls.
- Grow public APIs from repeated control needs, not from mirroring WPF,
  WinUI, Taffy, Subduction, Parley, or any host backend wholesale.
- Make diagnostics first-class: invalidation, layout, text shaping,
  composition, timers, and event dispatch should all become measurable.
- Preserve one obvious path for app authors: construct controls, bind or
  style properties, handle routed events, and let the frame pipeline do the
  rest.

Detailed roadmap tickets live under `.tickets/`; see epic `ove-p3tl`.

<!-- cargo-rdme end -->

## Minimum supported Rust Version (MSRV)

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
