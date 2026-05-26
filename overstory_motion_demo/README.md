# overstory_motion_demo

`overstory_motion_demo` is a focused native app for exercising Overstory's
presentation timeline and style-authored transition rules as visible motion
specimens.

Run it with:

```sh
cargo run -p overstory_motion_demo
```

The demo shows:

- OKLab background color interpolation;
- layer opacity transitions;
- border brush and border width transitions;
- rapid retargeting where each new transition starts from the current sampled
  presentation value.

This crate is not a stable example API. It is a small visual proving ground for
the motion system and the shape of style-authored motion policy.

## License

Licensed under either Apache-2.0 or MIT, at your option. See the repository root
for license details.
