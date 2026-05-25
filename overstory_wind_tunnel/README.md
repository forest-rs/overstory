# overstory_wind_tunnel

`overstory_wind_tunnel` contains benchmarks and profiler scenarios for Overstory
runtime hot paths. It keeps Criterion and profiler-only code out of the core
crates while making performance work repeatable.

Run the Criterion suite with:

```sh
cargo bench -p overstory_wind_tunnel
```

Run profiler-oriented loops with:

```sh
cargo run --release -p overstory_wind_tunnel --bin profile_build_dashboard -- 64 30
cargo run --release -p overstory_wind_tunnel --bin profile_ui_pipeline -- layout_dashboard 64 30
cargo run --release -p overstory_wind_tunnel --bin profile_ui_pipeline -- paint_dashboard 64 30
cargo run --release -p overstory_wind_tunnel --bin profile_ui_pipeline -- scroll_virtual_list 10000 30
cargo run --release -p overstory_wind_tunnel --bin profile_ui_pipeline -- layout_repeated_text_labels 512 30
cargo run --release -p overstory_wind_tunnel --bin profile_ui_pipeline -- paint_repeated_text_labels 512 30
```

The final positional argument is the run length in seconds. You can also set
`OVERSTORY_PROFILE_SECONDS` when the seconds argument is omitted.

Current scenarios cover:

- building a control-heavy dashboard tree;
- layout and paint of that dashboard;
- repeated scrolling through a virtual list;
- text-heavy layout and paint where labels share text and style.

## License

Licensed under either Apache-2.0 or MIT, at your option. See the repository root
for license details.
