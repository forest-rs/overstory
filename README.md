# Overstory

Retained UI toolkit work for forest-rs, built from small crates that keep core UI,
host integration, demos, and performance experiments separate.

Overstory is currently early, pre-release work. The public API is expected to
change while the control model, event routing, composition surfaces, and host
pipeline settle. The goal is a calm WPF / WinUI 3-inspired application API on
top of explicit, measurable internals.

## Workspace

| Crate | Purpose |
| --- | --- |
| [`overstory`](overstory/) | Core retained UI tree, controls, styling, layout intent, routed events, presentation lowering, and compositor-neutral composition data. |
| [`overstory_host`](overstory_host/) | Platform-neutral host runtime for driving a `Ui` from normalized input, timers, layout, and rendering preparation. |
| [`overstory_host_winit`](overstory_host_winit/) | Native `winit` / `wgpu` / Vello host for running Overstory apps on desktop. |
| [`overstory_demo`](overstory_demo/) | Control and motion gallery used to exercise built-in controls, routed input, timers, focus, popups, overlays, styling, and presentation transitions. |
| [`overstory_node_graph_demo`](overstory_node_graph_demo/) | Focused node graph showcase built with Overstory, `understory_node_graph`, and `understory_view2d`. |
| [`overstory_wind_tunnel`](overstory_wind_tunnel/) | Benchmarks and profiler scenarios for Overstory runtime hot paths. |

## Running Things

Native demos require a working desktop windowing and GPU stack.

```sh
cargo run -p overstory_demo
cargo run -p overstory_node_graph_demo
cargo bench -p overstory_wind_tunnel
```

For profiler-oriented loops, use the wind-tunnel binaries:

```sh
cargo run --release -p overstory_wind_tunnel --bin profile_build_dashboard -- 64 30
cargo run --release -p overstory_wind_tunnel --bin profile_ui_pipeline -- layout_dashboard 64 30
```

## Status

There is no CI for this repository yet. Until that exists, use the local checks
below before publishing or pushing release-facing work:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

The core crate is intended to stay `no_std` where practical. Host crates, demos,
and benchmark crates are allowed to depend on platform/runtime facilities that
do not belong in the core toolkit.

## Design Direction

- Keep `overstory` small enough to reason about and broad enough to express real
  applications.
- Keep windowing, GPU setup, frame pacing, IME, clipboard, accessibility bridges,
  and system compositor integration in host or integration crates.
- Grow public APIs from repeated control and host needs, rather than mirroring
  backend libraries wholesale.
- Treat diagnostics and measurement as part of the architecture.

Detailed roadmap tickets live under `.tickets/`; see epic `ove-p3tl`.

## Minimum Supported Rust Version

This workspace currently targets **Rust 1.92** and later.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>), or
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>),

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
