# System 6 — Design Tokens

Source of truth for the System 6 palette — a 1-bit black-and-white classic Mac
look. Parsed by `overstory_look_system6::design_md` into [`Seeds`]; the Rust
defaults in `palette.rs` mirror this table, and a test fails if they drift.

A true 1-bit world is just ink and paper. `desktop` and `muted` are pragmatic
grays standing in for things the original faked with 50% dither (the desktop
stipple and disabled text); a real stipple would need a pattern brush the
toolkit doesn't expose yet. There is no motion table — System 6 did not animate.

## Colors

Hex is `#rrggbb` or `#rrggbbaa`. `light` is the classic black-on-white desktop;
`dark` is an inverted "night" variant.

| role    | light     | dark      |
| ------- | --------- | --------- |
| ink     | #000000   | #ffffff   |
| paper   | #ffffff   | #101010   |
| desktop | #a0a0a0   | #303030   |
| muted   | #808080   | #909090   |

[`Seeds`]: https://docs.rs/overstory_look_system6/latest/overstory_look_system6/palette/struct.Seeds.html
