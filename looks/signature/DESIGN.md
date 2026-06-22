# Signature — Design Tokens

This file is the **source of truth** for the Signature palette. It is parsed by
`overstory_look_signature::design_md` into [`Seeds`], from which the whole theme —
the subtle surface gradients, the accent sheen, hover/pressed shades, the focus
glow, the hairline highlight — is *derived*. Edit a value here, rebuild, and the
look changes everywhere.

The Rust defaults in `palette.rs` mirror this table exactly; a crate test fails
if the two drift apart.

## Philosophy

- **Few seeds, derived ramps.** A dozen base colors and four durations; the rest
  is computed. No forty hand-tuned hex literals.
- **Gradient-forward.** Surfaces and accents carry subtle multi-stop gradients,
  authored in the unit square and fitted to each element's bounds.
- **Lively, not loud.** Interaction motion uses a springy overshoot curve;
  focus rings glow and settle.

## Colors

Each role names a base color per scheme. Hex is `#rrggbb` or `#rrggbbaa`.

| role           | dark      | light     |
| -------------- | --------- | --------- |
| accent         | #6366f1   | #5b5ef0   |
| surface        | #181a22   | #eef0f7   |
| panel          | #1c1e27   | #ffffff   |
| card           | #222530   | #ffffff   |
| control        | #1f2230   | #f6f7fb   |
| control-border | #3a3e4d   | #c9cedd   |
| track          | #2a2d3a   | #dfe3ee   |
| thumb          | #f6f7fb   | #ffffff   |
| on-accent      | #ffffff   | #ffffff   |
| on-surface     | #eceef6   | #1b1d27   |
| muted          | #a3a8bb   | #5d637a   |
| heading        | #f5f6fc   | #14161f   |
| body           | #cdd0dc   | #3a3f51   |
| focus          | #aab2ff   | #4b51e6   |

## Motion

Interaction durations, in milliseconds, shared across schemes.

| token      | ms  |
| ---------- | --- |
| fast       | 90  |
| normal     | 140 |
| emphasized | 220 |
| glow       | 420 |

[`Seeds`]: https://docs.rs/overstory_look_signature/latest/overstory_look_signature/palette/struct.Seeds.html
