# overstory_demo

`overstory_demo` is a native control-gallery app for exercising the current
Overstory control set. It is intentionally practical rather than minimal: the
demo uses real controls, default templates, styled parts, routed input, timers,
focus, popups, tooltips, scrolling, and overlays together.

Run it with:

```sh
cargo run -p overstory_demo
```

The demo currently shows:

- text input, buttons, checkboxes, toggle switches, sliders, progress bars, and
  spinners;
- list selection and scrolling;
- splitter-driven resizing;
- routed click, pointer, key, focus, drag, and timer behavior;
- popup and tooltip overlays through the native host.

This crate is not a stable example API. It is a working surface for proving that
the public control-building path feels coherent.

## License

Licensed under either Apache-2.0 or MIT, at your option. See the repository root
for license details.
